use glam::{IVec3, Vec3};
use us_core::{CELL, CHUNK_DIM};
use us_voxel::PaddedChunkArray;

pub const MAT_AIR: u8 = 0;
pub const MAT_DIRT: u8 = 1;
pub const MAT_GRASS: u8 = 2;
pub const MAT_STONE: u8 = 3;

/// Mesh buffers containing unindexed, flat-shaded vertex data for rendering.
#[derive(Debug, Default, Clone)]
pub struct TerrainMeshBuffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub materials: Vec<u8>,
    pub ao: Vec<f32>,
}

impl TerrainMeshBuffers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Emits a flat-shaded triangle into the buffers.
    pub fn push_triangle(&mut self, p0: Vec3, p1: Vec3, p2: Vec3, material: u8) {
        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let cross = edge1.cross(edge2);
        let normal = if cross.length_squared() > 1e-6 {
            cross.normalize()
        } else {
            Vec3::Y
        };

        // Surface skin (TER-011): Up-facing dirt becomes grass
        let final_material = if normal.y > 0.7 && (material == MAT_DIRT || material == MAT_STONE) {
            MAT_GRASS
        } else {
            material
        };

        let norm_arr = normal.to_array();

        // Push 3 unindexed vertices
        for &p in &[p0, p1, p2] {
            self.positions.push(p.to_array());
            self.normals.push(norm_arr);
            self.uvs.push([p.x / CELL, p.z / CELL]);
            self.materials.push(final_material);
            self.ao.push(1.0);
        }
    }
}

// 33 cells along each axis in padded space (0..33)
const CELLS_DIM: usize = 33;
const CELLS_VOLUME: usize = CELLS_DIM * CELLS_DIM * CELLS_DIM;

#[inline]
const fn cell_index(cx: usize, cy: usize, cz: usize) -> usize {
    cx + CELLS_DIM * (cy + CELLS_DIM * cz)
}

/// Meshes a 32³ chunk into flat-shaded triangles using naive Surface Nets (§5.2).
pub fn mesh_terrain(padded: &PaddedChunkArray) -> TerrainMeshBuffers {
    let mut buffers = TerrainMeshBuffers::new();

    // 1. Compute isosurface vertices for all cells in padded space (0..33)
    let mut cell_verts: Vec<Option<Vec3>> = vec![None; CELLS_VOLUME];
    let mut cell_mats: Vec<u8> = vec![0; CELLS_VOLUME];

    // Corner relative offsets within a cell (0..2 in x, y, z)
    let corner_offsets: [IVec3; 8] = [
        IVec3::new(0, 0, 0),
        IVec3::new(1, 0, 0),
        IVec3::new(0, 1, 0),
        IVec3::new(1, 1, 0),
        IVec3::new(0, 0, 1),
        IVec3::new(1, 0, 1),
        IVec3::new(0, 1, 1),
        IVec3::new(1, 1, 1),
    ];

    // 12 cube edges connecting corner indices (u, v)
    let edge_indices: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7), // X-parallel
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7), // Y-parallel
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // Z-parallel
    ];

    for cz in 0..CELLS_DIM {
        for cy in 0..CELLS_DIM {
            for cx in 0..CELLS_DIM {
                let mut s = [0.0f32; 8];
                let mut mats = [0u8; 8];
                let mut has_solid = false;
                let mut has_air = false;

                for (i, off) in corner_offsets.iter().enumerate() {
                    let px = cx + off.x as usize;
                    let py = cy + off.y as usize;
                    let pz = cz + off.z as usize;
                    let sdf_val = padded.get_sdf(px, py, pz);
                    s[i] = sdf_val;
                    mats[i] = padded.get_material(px, py, pz);

                    if sdf_val <= 0.0 {
                        has_solid = true;
                    } else {
                        has_air = true;
                    }
                }

                // If surface crosses this cell, interpolate vertex
                if has_solid && has_air {
                    let mut sum_pos = Vec3::ZERO;
                    let mut count = 0.0f32;

                    for &(i0, i1) in &edge_indices {
                        let s0 = s[i0];
                        let s1 = s[i1];
                        if (s0 <= 0.0 && s1 > 0.0) || (s0 > 0.0 && s1 <= 0.0) {
                            let t = -s0 / (s1 - s0);
                            let p0 = corner_offsets[i0].as_vec3();
                            let p1 = corner_offsets[i1].as_vec3();
                            sum_pos += p0 + t * (p1 - p0);
                            count += 1.0;
                        }
                    }

                    if count > 0.0 {
                        let avg_pos = sum_pos / count;
                        // World offset relative to chunk origin:
                        // cx in padded space corresponds to local (cx as i32 - 1)
                        let local_x = (cx as i32 - 1) as f32 + avg_pos.x;
                        let local_y = (cy as i32 - 1) as f32 + avg_pos.y;
                        let local_z = (cz as i32 - 1) as f32 + avg_pos.z;

                        let vert = Vec3::new(local_x * CELL, local_y * CELL, local_z * CELL);
                        let idx = cell_index(cx, cy, cz);
                        cell_verts[idx] = Some(vert);

                        // Majority material among solid corners
                        let mut mat_counts = [0u8; 256];
                        let mut best_mat = mats[0];
                        let mut max_count = 0;
                        for i in 0..8 {
                            if s[i] <= 0.0 {
                                let m = mats[i];
                                mat_counts[m as usize] += 1;
                                if mat_counts[m as usize] > max_count {
                                    max_count = mat_counts[m as usize];
                                    best_mat = m;
                                }
                            }
                        }
                        cell_mats[idx] = best_mat;
                    }
                }
            }
        }
    }

    // 2. Emit quads for every edge owned by this chunk (local coordinates 0..CHUNK_DIM)
    // Local 0..32 corresponds to padded index 1..=32
    for lz in 0..CHUNK_DIM {
        for ly in 0..CHUNK_DIM {
            for lx in 0..CHUNK_DIM {
                let px = (lx + 1) as usize;
                let py = (ly + 1) as usize;
                let pz = (lz + 1) as usize;

                // --- X-Parallel Edge from (px, py, pz) to (px + 1, py, pz) ---
                let s_start = padded.get_sdf(px, py, pz);
                let s_end = padded.get_sdf(px + 1, py, pz);
                if (s_start <= 0.0 && s_end > 0.0) || (s_start > 0.0 && s_end <= 0.0) {
                    // 4 cells sharing this edge in the YZ plane: (py, pz), (py - 1, pz), (py - 1, pz - 1), (py, pz - 1)
                    let c0 = cell_verts[cell_index(px, py, pz)];
                    let c1 = cell_verts[cell_index(px, py, pz - 1)];
                    let c2 = cell_verts[cell_index(px, py - 1, pz - 1)];
                    let c3 = cell_verts[cell_index(px, py - 1, pz)];

                    if let (Some(v0), Some(v1), Some(v2), Some(v3)) = (c0, c1, c2, c3) {
                        let mat = cell_mats[cell_index(px, py, pz)];
                        if s_start <= 0.0 {
                            buffers.push_triangle(v0, v3, v2, mat);
                            buffers.push_triangle(v0, v2, v1, mat);
                        } else {
                            buffers.push_triangle(v0, v1, v2, mat);
                            buffers.push_triangle(v0, v2, v3, mat);
                        }
                    }
                }

                // --- Y-Parallel Edge from (px, py, pz) to (px, py + 1, pz) ---
                let s_start = padded.get_sdf(px, py, pz);
                let s_end = padded.get_sdf(px, py + 1, pz);
                if (s_start <= 0.0 && s_end > 0.0) || (s_start > 0.0 && s_end <= 0.0) {
                    // 4 cells sharing this edge in the XZ plane: (px, pz), (px - 1, pz), (px - 1, pz - 1), (px, pz - 1)
                    let c0 = cell_verts[cell_index(px, py, pz)];
                    let c1 = cell_verts[cell_index(px - 1, py, pz)];
                    let c2 = cell_verts[cell_index(px - 1, py, pz - 1)];
                    let c3 = cell_verts[cell_index(px, py, pz - 1)];

                    if let (Some(v0), Some(v1), Some(v2), Some(v3)) = (c0, c1, c2, c3) {
                        let mat = cell_mats[cell_index(px, py, pz)];
                        if s_start <= 0.0 {
                            buffers.push_triangle(v0, v3, v2, mat);
                            buffers.push_triangle(v0, v2, v1, mat);
                        } else {
                            buffers.push_triangle(v0, v1, v2, mat);
                            buffers.push_triangle(v0, v2, v3, mat);
                        }
                    }
                }

                // --- Z-Parallel Edge from (px, py, pz) to (px, py, pz + 1) ---
                let s_start = padded.get_sdf(px, py, pz);
                let s_end = padded.get_sdf(px, py, pz + 1);
                if (s_start <= 0.0 && s_end > 0.0) || (s_start > 0.0 && s_end <= 0.0) {
                    // 4 cells sharing this edge in the XY plane: (px, py), (px - 1, py), (px - 1, py - 1), (px, py - 1)
                    let c0 = cell_verts[cell_index(px, py, pz)];
                    let c1 = cell_verts[cell_index(px, py - 1, pz)];
                    let c2 = cell_verts[cell_index(px - 1, py - 1, pz)];
                    let c3 = cell_verts[cell_index(px - 1, py, pz)];

                    if let (Some(v0), Some(v1), Some(v2), Some(v3)) = (c0, c1, c2, c3) {
                        let mat = cell_mats[cell_index(px, py, pz)];
                        if s_start <= 0.0 {
                            buffers.push_triangle(v0, v3, v2, mat);
                            buffers.push_triangle(v0, v2, v1, mat);
                        } else {
                            buffers.push_triangle(v0, v1, v2, mat);
                            buffers.push_triangle(v0, v2, v3, mat);
                        }
                    }
                }
            }
        }
    }

    buffers
}
