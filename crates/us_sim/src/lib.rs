//! `us_sim`: Simulation logic (fluids, lighting, navigation, damage rules) (§4.2, §10).
//! Pure crate: NO Bevy dependencies.

pub mod controller;

pub use controller::*;

pub fn sim_version() -> &'static str {
    "0.1.0"
}
