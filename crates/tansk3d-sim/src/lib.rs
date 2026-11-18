//! Tansk 3D: the same game reimagined in three dimensions. Tanks drive over
//! hilly terrain, shells fly in arcs, you aim from the turret. Shares the 2D
//! game's settings, weapons, power-ups and modes (`tansk_sim::proto`).

pub mod game;
mod ground;
pub mod proto;
pub mod terrain;

pub use game::{Game3d, MAX_PLAYERS, TICK_MS};
