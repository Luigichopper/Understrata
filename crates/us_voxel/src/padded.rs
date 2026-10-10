use us_core::{ChunkPos, CHUNK_DIM, CHUNK_PAD};

use crate::chunk::{dequantize_sdf, TerrainChunk};

pub const PADDED_DIM: usize = (CHUNK_DIM + 2 * CHUNK_PAD) as usize; // 34
pub const PADDED_VOLUME: usize = PADDED_DIM * PADDED_DIM * PADDED_DIM; // 39,304

/// Immutable 34³ padded array gathering a chunk's data plus 1-sample border from neighbors (§5.1).
/// Passed to async meshing tasks so background workers don't lock chunks.
#[derive(Clone)]
pub struct PaddedChunkArray {
    pub chunk_pos: ChunkPos,
    pub sdf: Box<[i8; PADDED_VOLUME]>,
    pub mat: Box<[u8; PADDED_VOLUME]>,
}

impl PaddedChunkArray {
    /// Linear index in the 34³ array (coordinates in 0..34).
    #[inline]
    pub const fn index(x: usize, y: usize, z: usize) -> usize {
        x + PADDED_DIM * (y + PADDED_DIM * z)
    }

    /// Read raw quantized signed distance at (0..34, 0..34, 0..34).
    #[inline]
    pub fn get_sdf_raw(&self, x: usize, y: usize, z: usize) -> i8 {
        self.sdf[Self::index(x, y, z)]
    }

    /// Read continuous signed distance in meters at (0..34, 0..34, 0..34).
    #[inline]
    pub fn get_sdf(&self, x: usize, y: usize, z: usize) -> f32 {
        dequantize_sdf(self.get_sdf_raw(x, y, z))
    }

    /// Read material ID at (0..34, 0..34, 0..34).
    #[inline]
    pub fn get_material(&self, x: usize, y: usize, z: usize) -> u8 {
        self.mat[Self::index(x, y, z)]
    }

    /// Creates a padded snapshot from a central chunk and a neighborhood sampler closure.
    /// `sample_neighbor(chunk_coord, local_coord) -> (sdf: i8, mat: u8)`
    pub fn from_chunk_and_neighbors<F>(chunk: &TerrainChunk, mut sample_fn: F) -> Self
    where
        F: FnMut(ChunkPos, usize, usize, usize) -> (i8, u8),
    {
        let mut sdf = Box::new([0i8; PADDED_VOLUME]);
        let mut mat = Box::new([0u8; PADDED_VOLUME]);

        let center_pos = chunk.pos;

        for pz in 0..PADDED_DIM {
            for py in 0..PADDED_DIM {
                for px in 0..PADDED_DIM {
                    let idx = Self::index(px, py, pz);

                    // Map padded coord (0..34) to local chunk coord with offset -1
                    let lx = px as i32 - 1;
                    let ly = py as i32 - 1;
                    let lz = pz as i32 - 1;

                    let in_bounds = (0..CHUNK_DIM).contains(&lx)
                        && (0..CHUNK_DIM).contains(&ly)
                        && (0..CHUNK_DIM).contains(&lz);

                    if in_bounds {
                        // Interior sample from self
                        let ux = lx as usize;
                        let uy = ly as usize;
                        let uz = lz as usize;
                        sdf[idx] = chunk.sdf.get(ux, uy, uz);
                        mat[idx] = chunk.mat.get(ux, uy, uz);
                    } else {
                        // Neighbor border sample
                        let cx = lx.div_euclid(CHUNK_DIM);
                        let cy = ly.div_euclid(CHUNK_DIM);
                        let cz = lz.div_euclid(CHUNK_DIM);

                        let target_chunk = ChunkPos::new(
                            center_pos.0.x + cx,
                            center_pos.0.y + cy,
                            center_pos.0.z + cz,
                        );

                        let nx = lx.rem_euclid(CHUNK_DIM) as usize;
                        let ny = ly.rem_euclid(CHUNK_DIM) as usize;
                        let nz = lz.rem_euclid(CHUNK_DIM) as usize;

                        let (s, m) = sample_fn(target_chunk, nx, ny, nz);
                        sdf[idx] = s;
                        mat[idx] = m;
                    }
                }
            }
        }

        Self {
            chunk_pos: center_pos,
            sdf,
            mat,
        }
    }
}
