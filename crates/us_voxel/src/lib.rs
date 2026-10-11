//! `us_voxel`: Chunk storage, SDF voxel data, block layers, and edit operations (§5).
//! Pure crate: NO Bevy dependencies.

pub mod chunk;
pub mod edit;
pub mod field;
pub mod padded;
pub mod query;

pub use chunk::{dequantize_sdf, quantize_sdf, ChunkFlags, TerrainChunk, SDF_QUANT};
pub use edit::{
    apply_edit_op, AppliedOp, BoxMode, BrushMode, BrushShape, EditOp, LoggedOp, MAT_BEDROCK,
};
pub use field::{Field, CHUNK_DIM_USIZE, CHUNK_VOLUME};
pub use padded::{PaddedChunkArray, PADDED_DIM, PADDED_VOLUME};
pub use query::{
    capsule_overlap, gradient_at_world, is_solid_at, material_at_world, raycast_terrain,
    sample_gradient, sample_trilinear_sdf, sdf_at_world, RaycastHit,
};

#[cfg(test)]
mod tests {
    use super::*;
    use us_core::ChunkPos;

    #[test]
    fn test_ter_001_chunk_uniform_collapse_and_size() {
        // TER-001 acceptance criteria:
        // 1. Uniform chunk size < 64 bytes
        assert!(std::mem::size_of::<TerrainChunk>() < 64);

        // 2. Uniform chunk creation
        let pos = ChunkPos::new(0, 0, 0);
        let mut chunk = TerrainChunk::new_solid(pos, 1);
        assert!(chunk.sdf.is_uniform());
        assert!(chunk.mat.is_uniform());

        // 3. Edits and round-trip
        chunk.set_sdf(15, 15, 15, 2.0);
        assert!(!chunk.sdf.is_uniform());
        assert_eq!(chunk.get_sdf(15, 15, 15), 2.0);

        // 4. Uniform collapse
        let solid_sdf = quantize_sdf(-us_core::CELL * 3.5);
        chunk.sdf.set(15, 15, 15, solid_sdf);
        assert!(chunk.sdf.try_collapse());
        assert!(chunk.sdf.is_uniform());
    }

    #[test]
    fn test_padded_chunk_border_mapping() {
        let center = ChunkPos::new(0, 0, 0);
        let chunk = TerrainChunk::new_solid(center, 42);

        let padded =
            PaddedChunkArray::from_chunk_and_neighbors(&chunk, |_target_chunk, _x, _y, _z| {
                (10, 99)
            });

        // Interior sample (px=1 -> chunk local 0)
        assert_eq!(padded.get_material(1, 1, 1), 42);

        // Border sample (px=0 -> neighbor)
        assert_eq!(padded.get_material(0, 1, 1), 99);
        assert_eq!(padded.get_sdf_raw(0, 1, 1), 10);
    }
}
