use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};

/// Lattice spacing in meters; terrain sample spacing == block size.
pub const CELL: f32 = 1.0;

/// Number of cells along one edge of a chunk.
pub const CHUNK_DIM: i32 = 32;

/// Number of boundary padding samples used for seamless meshing.
pub const CHUNK_PAD: i32 = 1;

/// Lowest world boundary (bedrock floor) in meters.
pub const WORLD_MIN_Y: i32 = -384;

/// Highest world boundary (sky ceiling) in meters.
pub const WORLD_MAX_Y: i32 = 128;

/// Sea level Y coordinate in meters.
pub const SEA_LEVEL_Y: i32 = 0;

/// Default player collision capsule dimensions in meters.
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_RADIUS: f32 = 0.35;
pub const EYE_HEIGHT: f32 = 1.62;

/// Continuous world position in meters (Y is up, right-handed).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldPos(pub Vec3);

impl WorldPos {
    pub const ZERO: Self = Self(Vec3::ZERO);

    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }

    /// Converts world position to the containing cell coordinate using floor division.
    #[inline]
    pub fn to_cell(self) -> CellPos {
        let x = (libm::floorf(self.0.x / CELL)) as i32;
        let y = (libm::floorf(self.0.y / CELL)) as i32;
        let z = (libm::floorf(self.0.z / CELL)) as i32;
        CellPos(IVec3::new(x, y, z))
    }

    /// Converts world position to the containing chunk coordinate.
    #[inline]
    pub fn to_chunk(self) -> ChunkPos {
        self.to_cell().to_chunk()
    }
}

/// Discrete integer cell coordinate on the 1 m grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellPos(pub IVec3);

impl PartialOrd for CellPos {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CellPos {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.0.x, self.0.y, self.0.z).cmp(&(other.0.x, other.0.y, other.0.z))
    }
}

impl CellPos {
    pub const ZERO: Self = Self(IVec3::ZERO);

    #[inline]
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    /// Returns chunk position using Euclidean division (`div_euclid`).
    /// Negative coordinates correctly map to their enclosing chunk.
    #[inline]
    pub fn to_chunk(self) -> ChunkPos {
        ChunkPos(IVec3::new(
            self.0.x.div_euclid(CHUNK_DIM),
            self.0.y.div_euclid(CHUNK_DIM),
            self.0.z.div_euclid(CHUNK_DIM),
        ))
    }

    /// Local cell offset within its chunk (in range `0..CHUNK_DIM`).
    #[inline]
    pub fn local_in_chunk(self) -> IVec3 {
        IVec3::new(
            self.0.x.rem_euclid(CHUNK_DIM),
            self.0.y.rem_euclid(CHUNK_DIM),
            self.0.z.rem_euclid(CHUNK_DIM),
        )
    }

    /// World center of the cell.
    #[inline]
    pub fn center_world(self) -> WorldPos {
        WorldPos(Vec3::new(
            (self.0.x as f32 + 0.5) * CELL,
            (self.0.y as f32 + 0.5) * CELL,
            (self.0.z as f32 + 0.5) * CELL,
        ))
    }

    /// World minimum corner (origin) of the cell.
    #[inline]
    pub fn min_corner_world(self) -> WorldPos {
        WorldPos(Vec3::new(
            self.0.x as f32 * CELL,
            self.0.y as f32 * CELL,
            self.0.z as f32 * CELL,
        ))
    }
}

/// Discrete integer chunk coordinate (each chunk is `CHUNK_DIM`³ cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkPos(pub IVec3);

impl PartialOrd for ChunkPos {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChunkPos {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.0.x, self.0.y, self.0.z).cmp(&(other.0.x, other.0.y, other.0.z))
    }
}

impl ChunkPos {
    pub const ZERO: Self = Self(IVec3::ZERO);

    #[inline]
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    /// Minimum cell coordinate contained in this chunk.
    #[inline]
    pub fn min_cell(self) -> CellPos {
        CellPos(self.0 * CHUNK_DIM)
    }

    /// Maximum cell coordinate contained in this chunk (inclusive).
    #[inline]
    pub fn max_cell(self) -> CellPos {
        CellPos(self.0 * CHUNK_DIM + IVec3::splat(CHUNK_DIM - 1))
    }

    /// Minimum corner world position in meters.
    #[inline]
    pub fn min_world(self) -> WorldPos {
        WorldPos(self.0.as_vec3() * (CHUNK_DIM as f32 * CELL))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_coord_conversions() {
        let cell = CellPos::new(10, 20, 31);
        let chunk = cell.to_chunk();
        assert_eq!(chunk, ChunkPos::new(0, 0, 0));
        assert_eq!(cell.local_in_chunk(), IVec3::new(10, 20, 31));

        let cell2 = CellPos::new(32, 64, 96);
        assert_eq!(cell2.to_chunk(), ChunkPos::new(1, 2, 3));
        assert_eq!(cell2.local_in_chunk(), IVec3::new(0, 0, 0));
    }

    #[test]
    fn test_negative_coord_euclidean_division() {
        // -1 should map to chunk -1, with local offset 31
        let cell_neg1 = CellPos::new(-1, -1, -1);
        assert_eq!(cell_neg1.to_chunk(), ChunkPos::new(-1, -1, -1));
        assert_eq!(cell_neg1.local_in_chunk(), IVec3::new(31, 31, 31));

        // -32 should map to chunk -1, with local offset 0
        let cell_neg32 = CellPos::new(-32, -32, -32);
        assert_eq!(cell_neg32.to_chunk(), ChunkPos::new(-1, -1, -1));
        assert_eq!(cell_neg32.local_in_chunk(), IVec3::new(0, 0, 0));

        // -33 should map to chunk -2, with local offset 31
        let cell_neg33 = CellPos::new(-33, -33, -33);
        assert_eq!(cell_neg33.to_chunk(), ChunkPos::new(-2, -2, -2));
        assert_eq!(cell_neg33.local_in_chunk(), IVec3::new(31, 31, 31));
    }

    #[test]
    fn test_world_to_cell_mapping() {
        let wp1 = WorldPos::new(0.5, 1.2, -0.1);
        assert_eq!(wp1.to_cell(), CellPos::new(0, 1, -1));

        let wp2 = WorldPos::new(-1.0, -0.01, 32.5);
        assert_eq!(wp2.to_cell(), CellPos::new(-1, -1, 32));
    }
}
