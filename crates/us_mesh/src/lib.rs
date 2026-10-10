//! `us_mesh`: Surface-nets terrain mesher, block mesher, flat shading, AO bake (§5.2, §6.3).
//! Pure crate: NO Bevy dependencies.

pub mod surface_nets;

pub use surface_nets::{mesh_terrain, TerrainMeshBuffers, MAT_AIR, MAT_DIRT, MAT_GRASS, MAT_STONE};

#[cfg(test)]
mod tests {
    use super::*;
    use us_core::ChunkPos;
    use us_voxel::{quantize_sdf, PaddedChunkArray, TerrainChunk};

    #[test]
    fn test_ter_002_mesh_sphere_produces_valid_triangles() {
        let chunk_pos = ChunkPos::new(0, 0, 0);
        let mut chunk = TerrainChunk::new_air(chunk_pos);

        // Carve a solid sphere at center (16, 16, 16) with radius 8
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let dx = x as f32 - 16.0;
                    let dy = y as f32 - 16.0;
                    let dz = z as f32 - 16.0;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt() - 8.0;
                    chunk.set_sdf(x, y, z, dist);
                    chunk.set_material(x, y, z, MAT_DIRT);
                }
            }
        }

        let padded = PaddedChunkArray::from_chunk_and_neighbors(&chunk, |_target, _x, _y, _z| {
            (quantize_sdf(10.0), MAT_AIR)
        });

        let mesh = mesh_terrain(&padded);
        assert!(!mesh.is_empty(), "Mesh should contain triangles for sphere");
        assert_eq!(
            mesh.positions.len() % 3,
            0,
            "Positions must be a multiple of 3 (unindexed triangles)"
        );
        assert_eq!(mesh.normals.len(), mesh.positions.len());
        assert_eq!(mesh.materials.len(), mesh.positions.len());

        // Check flat normals are normalized
        for normal in &mesh.normals {
            let len =
                (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            assert!(
                (len - 1.0).abs() < 1e-3,
                "Normal must be unit length: {len}"
            );
        }

        // TER-011: Verify grass appears on up-facing triangles
        let mut has_grass = false;
        for (i, normal) in mesh.normals.iter().enumerate() {
            if normal[1] > 0.7 && mesh.materials[i] == MAT_GRASS {
                has_grass = true;
                break;
            }
        }
        assert!(
            has_grass,
            "Up-facing dirt surface must be converted to grass (TER-011)"
        );
    }
}
