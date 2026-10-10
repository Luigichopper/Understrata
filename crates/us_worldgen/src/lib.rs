//! `us_worldgen`: Deterministic world generation (§4.4, §8).
//! Pure crate: NO Bevy dependencies.

use glam::{Vec2, Vec3};
use us_core::noise::{fbm_2d, fbm_3d};
use us_core::{ChunkPos, CELL};
use us_mesh::{MAT_AIR, MAT_DIRT, MAT_STONE};
use us_voxel::{TerrainChunk, CHUNK_DIM_USIZE};

/// Pure, deterministic test chunk generator for M1 (§4.4).
pub fn gen_test_chunk(seed: u64, pos: ChunkPos) -> TerrainChunk {
    let mut chunk = TerrainChunk::new_air(pos);

    let min_world = pos.min_world();
    let mut all_air = true;
    let mut all_solid = true;

    for lz in 0..CHUNK_DIM_USIZE {
        let wz = min_world.0.z + lz as f32 * CELL;
        for lx in 0..CHUNK_DIM_USIZE {
            let wx = min_world.0.x + lx as f32 * CELL;

            // 2D surface rolling hills
            let hill = 18.0 * fbm_2d(seed, Vec2::new(wx * 0.012, wz * 0.012), 4, 2.0, 0.5);
            let surface_y = hill;

            for ly in 0..CHUNK_DIM_USIZE {
                let wy = min_world.0.y + ly as f32 * CELL;

                // Base surface distance (negative = solid, positive = air)
                let base_sdf = wy - surface_y;

                // 3D cave carving below surface
                let is_cave = if wy < surface_y - 3.0 {
                    let c = fbm_3d(
                        seed.wrapping_add(777),
                        Vec3::new(wx * 0.035, wy * 0.035, wz * 0.035),
                        3,
                        2.0,
                        0.5,
                    );
                    c.abs() < 0.12
                } else {
                    false
                };

                let final_sdf = if is_cave { 2.5 } else { base_sdf };

                let mat = if final_sdf > 0.0 {
                    MAT_AIR
                } else if wy < surface_y - 5.0 {
                    MAT_STONE
                } else {
                    MAT_DIRT
                };

                chunk.set_sdf(lx, ly, lz, final_sdf);
                chunk.set_material(lx, ly, lz, mat);

                if final_sdf <= 0.0 {
                    all_air = false;
                } else {
                    all_solid = false;
                }
            }
        }
    }

    if all_air || all_solid {
        chunk.sdf.try_collapse();
        chunk.mat.try_collapse();
    }

    chunk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_generation_is_pure_and_deterministic() {
        let seed = 12345;
        let pos = ChunkPos::new(2, -1, 3);
        let c1 = gen_test_chunk(seed, pos);
        let c2 = gen_test_chunk(seed, pos);

        assert_eq!(c1.sdf, c2.sdf);
        assert_eq!(c1.mat, c2.mat);
    }
}
