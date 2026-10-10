//! `us_core` contains engine-agnostic core primitives:
//! - Coordinate types, spatial conversions, and world constants (§4.5)
//! - Deterministic RNG and integer hashing (§4.4)
//! - Pure deterministic gradient noise and fBm using `libm` (§4.4)
//!
//! Note: This crate NEVER depends on Bevy or GPU libraries.

pub mod coords;
pub mod noise;
pub mod rng;

pub use coords::{
    CellPos, ChunkPos, WorldPos, CELL, CHUNK_DIM, CHUNK_PAD, EYE_HEIGHT, PLAYER_HEIGHT,
    PLAYER_RADIUS, SEA_LEVEL_Y, WORLD_MAX_Y, WORLD_MIN_Y,
};
pub use noise::{fbm_2d, fbm_3d, grad_noise_2d, grad_noise_3d};
pub use rng::{hash64_2, hash_coords3, Rng64};
