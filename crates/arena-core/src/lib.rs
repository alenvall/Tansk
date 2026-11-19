//! TANSK 2026 shared simulation.
//!
//! The authoritative game: a rapier world on "The Sink", physics tanks with
//! bolt-on parts, ballistic and hitscan weapons, scrap pickups and upgrades,
//! Scrap Brawl, and bot brains. Runs on the server (authoritative) and in the
//! browser (prediction, offline Practice); no OS entropy, threads or clocks.
//!
//! Conventions (see README section in the crate docs of each module):
//! - **Y is up**, metres, seconds, kg. Fixed step [`DT`] = 1/60 s.
//! - Tank hull frame: **+X forward, +Y up, +Z right**. Hull yaw 0 faces +X;
//!   positive yaw turns left (counter-clockwise seen from above).
//! - Turret yaw is relative to the hull (+ = left), gun pitch relative to the
//!   turret (+ = up). See [`parts`] for frames and hit boxes.
//!
//! ```no_run
//! use arena_core::*;
//! let mut sim = Sim::new(7, Map::the_sink(7), Tuning::default());
//! let me = sim.add_player("crew");
//! sim.add_bot("Rusty", 0.5);
//! sim.set_input(me, Input { throttle: 1.0, ..Input::default() });
//! sim.step();
//! let bytes = sim.snapshot().encode();
//! for e in sim.drain_events() { let _ = e; }
//! # let _ = bytes;
//! ```

pub mod bots;
pub mod convoy;
pub mod events;
pub mod garage;
pub mod landship;
pub mod lastcrew;
pub mod map;
pub mod net;
pub mod parts;
pub mod physics;
pub mod raid;
pub mod refinery;
pub mod rng;
pub mod sim;
pub mod sink;
pub mod snapshot;
pub mod storm;
pub mod specials;
pub mod tank;
pub mod tuning;
pub mod wellhead;

pub use events::{Event, HitLocation, PickupKind, RepairKind, Weapon};
pub use glam::{Quat, Vec3};
pub use map::{Cover, Map, MapId, Obstacle, ObstacleBox, ObstacleKind, ScrapKind};
pub use parts::{part_def, VehicleKind, AltKind, Frame, GunKind, HitBox, Part, PartKind, Slot};
pub use sim::{Phase, PlayerInfo, PropKind, Sim};
pub use raid::{BossKind, ModeKind, RaidPhase};
pub use convoy::{ConvoyPhase, ConvoySnap};
pub use lastcrew::StormCircleSnap;
pub use wellhead::{WellSnap, WellheadSnap};
pub use garage::{ChassisClass, Loadout};
pub use snapshot::Snapshot;
pub use tuning::Tuning;

use serde::{Deserialize, Serialize};

/// Fixed simulation step, seconds.
pub const DT: f32 = 1.0 / 60.0;
/// Simulation ticks per second.
pub const TICK_HZ: u32 = 60;

/// A player slot in a match (0..=255).
pub type PlayerId = u8;

/// One tick of controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Input {
    /// -1 (full reverse) ..= 1 (full ahead).
    pub throttle: f32,
    /// -1 (left) ..= 1 (right). Skid steering: works standing still.
    pub steer: f32,
    /// World point the turret should put a shell on (the reticle's ground/target point).
    pub aim_point: Vec3,
    /// Main gun.
    pub fire: bool,
    /// Alt weapon: MG (hold), Coilgun (hold to charge, release to fire), Tesla (press).
    pub alt_fire: bool,
    pub boost: bool,
    /// Branch pick while a choice is open: index into the offered options.
    pub choice: Option<u8>,
    /// Added in wave 2: use the rare part (mortar, harpoon, smoke, mines) or, when
    /// holding a flare, call the air strike on `aim_point`.
    #[serde(default)]
    pub special: bool,
    /// Added in wave 6: sound the horn (an edge sends `Event::Horn`).
    #[serde(default)]
    pub horn: bool,
}
