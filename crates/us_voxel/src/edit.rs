use glam::{IVec3, IVec4, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use us_core::{CellPos, ChunkPos, CELL, CHUNK_DIM};

use crate::chunk::{dequantize_sdf, quantize_sdf, ChunkFlags, TerrainChunk};

pub const MAT_BEDROCK: u8 = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrushMode {
    Subtract,
    Add,
    Smooth,
    Flatten { plane_q: IVec4 },
    Paint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrushShape {
    Sphere,
    Cylinder { axis: u8, half_h_q: u16 },
    Cube,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoxMode {
    Subtract,
    Add,
    Set,
}

/// Authoritative, deterministic, serializable edit operation (§5.6).
/// Uses fixed-point integers (1/16 m quantization) to guarantee bit-exact replay across machines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditOp {
    TerrainBrush {
        center_q: IVec3, // 1/16 m
        radius_q: u16,   // 1/16 m
        shape: BrushShape,
        mode: BrushMode,
        material: u8,
        strength: u8,
    },
    TerrainBox {
        min: IVec3,
        max: IVec3,
        mode: BoxMode,
        material: u8,
    },
    BlockSet {
        cell: IVec3,
        state: u16,
    },
    BlockFill {
        min: IVec3,
        max: IVec3,
        state: u16,
        replace: bool,
    },
    FluidSet {
        cell: IVec3,
        kind: u8,
        level: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggedOp {
    pub seq: u64,
    pub tick: u64,
    pub actor: u32,
    pub op: EditOp,
}

#[derive(Debug, Default, Clone)]
pub struct AppliedOp {
    pub dirty_chunks: Vec<ChunkPos>,
    pub removed_volume_per_mat: HashMap<u8, f32>, // m³ removed
    pub added_volume_per_mat: HashMap<u8, f32>,   // m³ added
}

/// Applies an EditOp authoritatively to a set of chunks (§5.6).
pub fn apply_edit_op(chunks: &mut HashMap<ChunkPos, TerrainChunk>, op: &EditOp) -> AppliedOp {
    let mut applied = AppliedOp::default();

    if let EditOp::TerrainBrush {
        center_q,
        radius_q,
        shape,
        mode,
        material,
        ..
    } = op
    {
        let center = center_q.as_vec3() / 16.0;

        let radius = *radius_q as f32 / 16.0;

        // Bounding AABB of brush in world coordinates
        let min_world = center - Vec3::splat(radius + CELL);
        let max_world = center + Vec3::splat(radius + CELL);

        let min_cell = CellPos::new(
            (libm::floorf(min_world.x / CELL)) as i32,
            (libm::floorf(min_world.y / CELL)) as i32,
            (libm::floorf(min_world.z / CELL)) as i32,
        );
        let max_cell = CellPos::new(
            (libm::ceilf(max_world.x / CELL)) as i32,
            (libm::ceilf(max_world.y / CELL)) as i32,
            (libm::ceilf(max_world.z / CELL)) as i32,
        );

        let min_chunk = min_cell.to_chunk();
        let max_chunk = max_cell.to_chunk();

        let mut dirty_set = HashSet::new();

        for cz in min_chunk.0.z..=max_chunk.0.z {
            for cy in min_chunk.0.y..=max_chunk.0.y {
                for cx in min_chunk.0.x..=max_chunk.0.x {
                    let cpos = ChunkPos::new(cx, cy, cz);
                    let Some(chunk) = chunks.get_mut(&cpos) else {
                        continue;
                    };

                    let chunk_min = cpos.min_world().0;
                    let mut chunk_edited = false;

                    for lz in 0..CHUNK_DIM {
                        let wz = chunk_min.z + lz as f32 * CELL;
                        for ly in 0..CHUNK_DIM {
                            let wy = chunk_min.y + ly as f32 * CELL;
                            for lx in 0..CHUNK_DIM {
                                let wx = chunk_min.x + lx as f32 * CELL;
                                let sample_pos = Vec3::new(wx, wy, wz);

                                // Distance to brush
                                let brush_sdf = match shape {
                                    BrushShape::Sphere => (sample_pos - center).length() - radius,
                                    BrushShape::Cube => {
                                        let d = (sample_pos - center).abs() - Vec3::splat(radius);
                                        d.max_element()
                                    }
                                    BrushShape::Cylinder { half_h_q, .. } => {
                                        let half_h = *half_h_q as f32 / 16.0;
                                        let d_xz = (Vec2::new(sample_pos.x, sample_pos.z)
                                            - Vec2::new(center.x, center.z))
                                        .length()
                                            - radius;
                                        let d_y = (sample_pos.y - center.y).abs() - half_h;
                                        d_xz.max(d_y)
                                    }
                                };

                                // Check if brush touches this sample
                                if brush_sdf <= 0.0 {
                                    let old_raw =
                                        chunk.sdf.get(lx as usize, ly as usize, lz as usize);
                                    let old_sdf = dequantize_sdf(old_raw);
                                    let old_mat =
                                        chunk.mat.get(lx as usize, ly as usize, lz as usize);

                                    // Bedrock is indestructible (§5.8, TER-012)
                                    if old_mat == MAT_BEDROCK {
                                        continue;
                                    }

                                    let new_sdf = match mode {
                                        BrushMode::Subtract => {
                                            // Subtract: new_d = max(old_d, -brush_sdf)
                                            old_sdf.max(-brush_sdf)
                                        }
                                        BrushMode::Add => {
                                            // Add: new_d = min(old_d, brush_sdf)
                                            old_sdf.min(brush_sdf)
                                        }
                                        BrushMode::Paint => old_sdf,
                                        _ => old_sdf,
                                    };

                                    if (new_sdf - old_sdf).abs() > 1e-4 {
                                        chunk.sdf.set(
                                            lx as usize,
                                            ly as usize,
                                            lz as usize,
                                            quantize_sdf(new_sdf),
                                        );
                                        chunk_edited = true;

                                        // Yield tracking (§5.8)
                                        if old_sdf <= 0.0 && new_sdf > 0.0 {
                                            // Cell transitioned from solid to air
                                            let cell_vol = CELL * CELL * CELL;
                                            *applied
                                                .removed_volume_per_mat
                                                .entry(old_mat)
                                                .or_insert(0.0) += cell_vol;
                                        } else if old_sdf > 0.0 && new_sdf <= 0.0 {
                                            let cell_vol = CELL * CELL * CELL;
                                            *applied
                                                .added_volume_per_mat
                                                .entry(*material)
                                                .or_insert(0.0) += cell_vol;
                                        }

                                        // Border dirty propagation (§5.6)
                                        if lx == 0 {
                                            dirty_set.insert(ChunkPos::new(cx - 1, cy, cz));
                                        } else if lx == CHUNK_DIM - 1 {
                                            dirty_set.insert(ChunkPos::new(cx + 1, cy, cz));
                                        }
                                        if ly == 0 {
                                            dirty_set.insert(ChunkPos::new(cx, cy - 1, cz));
                                        } else if ly == CHUNK_DIM - 1 {
                                            dirty_set.insert(ChunkPos::new(cx, cy + 1, cz));
                                        }
                                        if lz == 0 {
                                            dirty_set.insert(ChunkPos::new(cx, cy, cz - 1));
                                        } else if lz == CHUNK_DIM - 1 {
                                            dirty_set.insert(ChunkPos::new(cx, cy, cz + 1));
                                        }
                                    }

                                    // Set material for Add or Paint
                                    if matches!(mode, BrushMode::Add | BrushMode::Paint) {
                                        chunk.mat.set(
                                            lx as usize,
                                            ly as usize,
                                            lz as usize,
                                            *material,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    if chunk_edited {
                        chunk
                            .flags
                            .insert(ChunkFlags::EDITED | ChunkFlags::MESH_DIRTY);
                        chunk.version = chunk.version.wrapping_add(1);
                        dirty_set.insert(cpos);
                    }
                }
            }
        }

        applied.dirty_chunks = dirty_set.into_iter().collect();
    }

    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ter_009_edit_op_replay_determinism() {
        let mut chunks1 = HashMap::new();
        let mut chunks2 = HashMap::new();

        let pos = ChunkPos::new(0, 0, 0);
        chunks1.insert(pos, TerrainChunk::new_solid(pos, 1));
        chunks2.insert(pos, TerrainChunk::new_solid(pos, 1));

        let op = EditOp::TerrainBrush {
            center_q: IVec3::new(16 * 16, 16 * 16, 16 * 16), // center at (16.0, 16.0, 16.0)
            radius_q: 4 * 16,                                // radius 4.0 m
            shape: BrushShape::Sphere,
            mode: BrushMode::Subtract,
            material: 0,
            strength: 255,
        };

        let res1 = apply_edit_op(&mut chunks1, &op);
        let res2 = apply_edit_op(&mut chunks2, &op);

        assert_eq!(res1.dirty_chunks, res2.dirty_chunks);
        assert_eq!(
            chunks1.get(&pos).unwrap().sdf,
            chunks2.get(&pos).unwrap().sdf
        );
    }

    #[test]
    fn test_ter_012_bedrock_indestructible() {
        let mut chunks = HashMap::new();
        let pos = ChunkPos::new(0, 0, 0);
        let chunk = TerrainChunk::new_solid(pos, MAT_BEDROCK);
        chunks.insert(pos, chunk);

        let op = EditOp::TerrainBrush {
            center_q: IVec3::new(16 * 16, 16 * 16, 16 * 16),
            radius_q: 8 * 16,
            shape: BrushShape::Sphere,
            mode: BrushMode::Subtract,
            material: 0,
            strength: 255,
        };

        let applied = apply_edit_op(&mut chunks, &op);
        assert!(
            applied.removed_volume_per_mat.is_empty(),
            "Bedrock must never be mined!"
        );
    }
}
