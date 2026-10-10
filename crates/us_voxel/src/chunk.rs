use serde::{Deserialize, Serialize};
use us_core::{ChunkPos, CELL};

use crate::field::Field;

/// Quantization step for SDF samples (i8 represents range ≈ ±3.97 cells).
pub const SDF_QUANT: f32 = CELL / 32.0;

#[inline]
pub fn quantize_sdf(dist: f32) -> i8 {
    let q = (dist / SDF_QUANT).round();
    q.clamp(-128.0, 127.0) as i8
}

#[inline]
pub fn dequantize_sdf(raw: i8) -> f32 {
    raw as f32 * SDF_QUANT
}

/// Bit flags tracking chunk mutation and update state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChunkFlags(pub u32);

impl ChunkFlags {
    pub const EMPTY: Self = Self(0);
    pub const GENERATED: Self = Self(1 << 0);
    pub const EDITED: Self = Self(1 << 1);
    pub const MESH_DIRTY: Self = Self(1 << 2);
    pub const LIGHT_DIRTY: Self = Self(1 << 3);
    pub const NAV_DIRTY: Self = Self(1 << 4);
    pub const SAVE_DIRTY: Self = Self(1 << 5);

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl std::ops::BitOr for ChunkFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ChunkFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// The core terrain chunk representing 32³ SDF samples and material IDs (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainChunk {
    pub pos: ChunkPos,
    pub sdf: Field<i8>,
    pub mat: Field<u8>,
    pub version: u32,
    pub flags: ChunkFlags,
}

impl TerrainChunk {
    /// Creates a uniform chunk (e.g. completely solid or completely air).
    pub fn new_uniform(pos: ChunkPos, sdf_val: i8, mat_val: u8) -> Self {
        Self {
            pos,
            sdf: Field::new_uniform(sdf_val),
            mat: Field::new_uniform(mat_val),
            version: 0,
            flags: ChunkFlags::GENERATED | ChunkFlags::MESH_DIRTY,
        }
    }

    /// Creates an all-air chunk.
    pub fn new_air(pos: ChunkPos) -> Self {
        Self::new_uniform(pos, quantize_sdf(CELL * 3.5), 0)
    }

    /// Creates an all-solid chunk.
    pub fn new_solid(pos: ChunkPos, mat: u8) -> Self {
        Self::new_uniform(pos, quantize_sdf(-CELL * 3.5), mat)
    }

    /// Reads continuous signed distance at a local cell coordinate.
    #[inline]
    pub fn get_sdf(&self, x: usize, y: usize, z: usize) -> f32 {
        dequantize_sdf(self.sdf.get(x, y, z))
    }

    /// Reads material ID at a local cell coordinate.
    #[inline]
    pub fn get_material(&self, x: usize, y: usize, z: usize) -> u8 {
        self.mat.get(x, y, z)
    }

    /// Writes continuous signed distance at a local cell coordinate.
    pub fn set_sdf(&mut self, x: usize, y: usize, z: usize, dist: f32) {
        let q = quantize_sdf(dist);
        self.sdf.set(x, y, z, q);
        self.flags
            .insert(ChunkFlags::EDITED | ChunkFlags::MESH_DIRTY);
        self.version = self.version.wrapping_add(1);
    }

    /// Writes material ID at a local cell coordinate.
    pub fn set_material(&mut self, x: usize, y: usize, z: usize, mat: u8) {
        self.mat.set(x, y, z, mat);
        self.flags
            .insert(ChunkFlags::EDITED | ChunkFlags::MESH_DIRTY);
        self.version = self.version.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_chunk_size_under_64_bytes() {
        // Acceptance criteria for TER-001: uniform chunk struct size < 64 bytes
        assert!(std::mem::size_of::<TerrainChunk>() < 64);
    }

    #[test]
    fn test_quantization_roundtrip() {
        for dist in [-3.0, -1.5, -0.5, 0.0, 0.25, 1.0, 2.5] {
            let q = quantize_sdf(dist);
            let deq = dequantize_sdf(q);
            assert!((deq - dist).abs() <= SDF_QUANT);
        }
    }
}
