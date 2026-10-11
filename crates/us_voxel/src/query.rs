use glam::Vec3;
use us_core::{CellPos, WorldPos, CELL};

use crate::chunk::TerrainChunk;

/// Samples continuous signed distance via trilinear interpolation within a chunk's local frame.
pub fn sample_trilinear_sdf(
    chunk: &TerrainChunk,
    local_pos: Vec3, // in meters within chunk [0..32]
) -> f32 {
    let fx = local_pos.x / CELL;
    let fy = local_pos.y / CELL;
    let fz = local_pos.z / CELL;

    let x0 = (libm::floorf(fx) as usize).clamp(0, 31);
    let y0 = (libm::floorf(fy) as usize).clamp(0, 31);
    let z0 = (libm::floorf(fz) as usize).clamp(0, 31);

    let x1 = (x0 + 1).min(31);
    let y1 = (y0 + 1).min(31);
    let z1 = (z0 + 1).min(31);

    let tx = fx - (x0 as f32);
    let ty = fy - (y0 as f32);
    let tz = fz - (z0 as f32);

    let c000 = chunk.get_sdf(x0, y0, z0);
    let c100 = chunk.get_sdf(x1, y0, z0);
    let c010 = chunk.get_sdf(x0, y1, z0);
    let c110 = chunk.get_sdf(x1, y1, z0);
    let c001 = chunk.get_sdf(x0, y0, z1);
    let c101 = chunk.get_sdf(x1, y0, z1);
    let c011 = chunk.get_sdf(x0, y1, z1);
    let c111 = chunk.get_sdf(x1, y1, z1);

    let c00 = c000 + tx * (c100 - c000);
    let c10 = c010 + tx * (c110 - c010);
    let c01 = c001 + tx * (c101 - c001);
    let c11 = c011 + tx * (c111 - c011);

    let c0 = c00 + ty * (c10 - c00);
    let c1 = c01 + ty * (c11 - c01);

    c0 + tz * (c1 - c0)
}

/// Computes the surface normal / gradient vector at local position via central differences.
pub fn sample_gradient(chunk: &TerrainChunk, local_pos: Vec3) -> Vec3 {
    let eps = 0.1 * CELL;
    let dx = sample_trilinear_sdf(chunk, local_pos + Vec3::X * eps)
        - sample_trilinear_sdf(chunk, local_pos - Vec3::X * eps);
    let dy = sample_trilinear_sdf(chunk, local_pos + Vec3::Y * eps)
        - sample_trilinear_sdf(chunk, local_pos - Vec3::Y * eps);
    let dz = sample_trilinear_sdf(chunk, local_pos + Vec3::Z * eps)
        - sample_trilinear_sdf(chunk, local_pos - Vec3::Z * eps);

    let g = Vec3::new(dx, dy, dz);
    let len = g.length();
    if len > 1e-5 {
        g / len
    } else {
        Vec3::Y
    }
}

/// Checks if continuous world position is inside solid ground.
#[inline]
pub fn is_solid_at(chunk: &TerrainChunk, world_pos: WorldPos) -> bool {
    let cell = world_pos.to_cell();
    let local = cell.local_in_chunk();
    chunk.get_sdf(local.x as usize, local.y as usize, local.z as usize) <= 0.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastHit {
    pub position: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub material: u8,
}

/// Raymarches through the continuous signed distance field to find surface hits (§5.8).
pub fn raycast_terrain(
    chunks: &std::collections::HashMap<us_core::ChunkPos, TerrainChunk>,
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
) -> Option<RaycastHit> {
    let dir = dir.normalize();
    let mut dist = 0.1 * CELL;
    let step_eps = 0.08 * CELL;

    while dist < max_dist {
        let p = origin + dir * dist;
        let cell = CellPos::new(
            libm::floorf(p.x / CELL) as i32,
            libm::floorf(p.y / CELL) as i32,
            libm::floorf(p.z / CELL) as i32,
        );
        let cpos = cell.to_chunk();

        if let Some(chunk) = chunks.get(&cpos) {
            let chunk_min = cpos.min_world().0;
            let local_p = p - chunk_min;
            let sdf = sample_trilinear_sdf(chunk, local_p);

            if sdf <= 0.05 {
                let normal = sample_gradient(chunk, local_p);
                let local_cell = cell.local_in_chunk();
                let mat = chunk.get_material(
                    local_cell.x.clamp(0, 31) as usize,
                    local_cell.y.clamp(0, 31) as usize,
                    local_cell.z.clamp(0, 31) as usize,
                );

                return Some(RaycastHit {
                    position: p,
                    normal,
                    distance: dist,
                    material: mat,
                });
            }

            dist += sdf.max(step_eps);
        } else {
            dist += CELL;
        }
    }

    None
}

/// Evaluates the continuous terrain SDF across multi-chunk storage at an arbitrary world position.
pub fn sdf_at_world(
    chunks: &std::collections::HashMap<us_core::ChunkPos, TerrainChunk>,
    world_pos: Vec3,
) -> f32 {
    let cell = CellPos::new(
        libm::floorf(world_pos.x / CELL) as i32,
        libm::floorf(world_pos.y / CELL) as i32,
        libm::floorf(world_pos.z / CELL) as i32,
    );
    let cpos = cell.to_chunk();

    if let Some(chunk) = chunks.get(&cpos) {
        let chunk_min = cpos.min_world().0;
        let local_p = world_pos - chunk_min;
        sample_trilinear_sdf(chunk, local_p)
    } else {
        10.0 // Default to open air if chunk is unpopulated
    }
}

/// Computes the continuous surface normal / gradient vector at an arbitrary world position.
pub fn gradient_at_world(
    chunks: &std::collections::HashMap<us_core::ChunkPos, TerrainChunk>,
    world_pos: Vec3,
) -> Vec3 {
    let eps = 0.1 * CELL;
    let dx = sdf_at_world(chunks, world_pos + Vec3::X * eps)
        - sdf_at_world(chunks, world_pos - Vec3::X * eps);
    let dy = sdf_at_world(chunks, world_pos + Vec3::Y * eps)
        - sdf_at_world(chunks, world_pos - Vec3::Y * eps);
    let dz = sdf_at_world(chunks, world_pos + Vec3::Z * eps)
        - sdf_at_world(chunks, world_pos - Vec3::Z * eps);

    let g = Vec3::new(dx, dy, dz);
    let len = g.length();
    if len > 1e-5 {
        g / len
    } else {
        Vec3::Y
    }
}

/// Retrieves the terrain material voxel index at an arbitrary world position.
pub fn material_at_world(
    chunks: &std::collections::HashMap<us_core::ChunkPos, TerrainChunk>,
    world_pos: Vec3,
) -> u8 {
    let cell = CellPos::new(
        libm::floorf(world_pos.x / CELL) as i32,
        libm::floorf(world_pos.y / CELL) as i32,
        libm::floorf(world_pos.z / CELL) as i32,
    );
    let cpos = cell.to_chunk();
    if let Some(chunk) = chunks.get(&cpos) {
        let local_cell = cell.local_in_chunk();
        chunk.get_material(
            local_cell.x.clamp(0, 31) as usize,
            local_cell.y.clamp(0, 31) as usize,
            local_cell.z.clamp(0, 31) as usize,
        )
    } else {
        0 // Air
    }
}

/// Tests whether a vertical capsule collides with the terrain SDF (§7.1).
/// Returns penetration normal and penetration depth if overlapping solid ground.
pub fn capsule_overlap(
    chunks: &std::collections::HashMap<us_core::ChunkPos, TerrainChunk>,
    base: Vec3,
    height: f32,
    radius: f32,
) -> Option<(Vec3, f32)> {
    let bottom_center = base + Vec3::Y * radius;
    let top_center = base + Vec3::Y * (height - radius).max(radius);

    // Test multiple sample points along the capsule spine
    let sample_count = 5;
    let mut deepest_penetration = 0.0f32;
    let mut best_normal = Vec3::Y;
    let mut collided = false;

    for i in 0..sample_count {
        let t = i as f32 / (sample_count - 1) as f32;
        let pt = bottom_center.lerp(top_center, t);
        let dist = sdf_at_world(chunks, pt);

        let penetration = radius - dist;
        if penetration > 0.0 {
            collided = true;
            if penetration > deepest_penetration {
                deepest_penetration = penetration;
                best_normal = gradient_at_world(chunks, pt);
            }
        }
    }

    if collided {
        Some((best_normal, deepest_penetration))
    } else {
        None
    }
}
