//! Tansk's game rules, ported from the 2013 Java/Slick2D original.
//!
//! No rendering, sockets or clocks here: the server steps a [`Game`] for each
//! room, and the browser steps one itself in the offline test arena.

pub mod game;
pub mod geom;
pub mod map;
pub mod proto;
pub mod rng;

pub use game::{Game, Input, MAX_PLAYERS, TICK_MS};
pub use geom::V2;
