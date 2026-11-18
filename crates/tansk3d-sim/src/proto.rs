//! What the /3d server and browser say to each other (bincode over the
//! WebSocket at `/ws3d`), and the per-tick view both draw from.
//!
//! Coordinates: x east (0..ARENA_W), z south (0..ARENA_D, the 2D game's y),
//! y up. One unit is one pixel of the 2D game. Angles are degrees.
//! Yaw 0 points +x, yaw 90 points +z (clockwise seen from above, like the 2D
//! game's theta). Pitch is up from horizontal.

use serde::{Deserialize, Serialize};
pub use tansk_sim::proto::{
    color_name, MsgLevel, Phase, PickupKind, PlayerView, Power, ProjKind, Settings, Sound, Weapon, COLORS,
};

pub type P3 = [f32; 3];

/// One player's controls this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Input3d {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub fire: bool,
    /// Where the turret should face (world yaw, degrees). The turret turns
    /// toward it at a limited rate.
    pub aim_yaw: f32,
    /// Barrel elevation wanted, degrees, clamped by the game to [MIN_PITCH, MAX_PITCH].
    pub aim_pitch: f32,
}

pub const MIN_PITCH: f32 = -8.0;
pub const MAX_PITCH: f32 = 35.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TankView3d {
    pub id: u32,
    pub player: u32,
    pub color: u8,
    /// Hull centre, on the ground.
    pub pos: P3,
    /// Hull heading.
    pub yaw: f32,
    /// Hull tilt from the terrain under it (for drawing).
    pub hull_pitch: f32,
    pub hull_roll: f32,
    /// Turret heading (world) and barrel elevation.
    pub turret_yaw: f32,
    pub barrel_pitch: f32,
    pub health: f32,
    pub shield: Option<f32>,
    pub weapon: Weapon,
    pub ammo: i32,
    pub power: Option<Power>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjView3d {
    pub id: u32,
    pub kind: ProjKind,
    pub pos: P3,
    pub vel: P3,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PickupView3d {
    pub id: u32,
    pub kind: PickupKind,
    pub pos: P3,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event3d {
    Sound(Sound, P3),
    Explosion(P3),
    /// A projectile struck terrain or a wall (dust puff).
    Impact(P3),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct View3d {
    pub tick: u32,
    pub phase: Phase,
    pub delay_ms: i32,
    pub round: i32,
    pub round_ms_left: i32,
    pub game_ms_left: i32,
    pub tanks: Vec<TankView3d>,
    pub projectiles: Vec<ProjView3d>,
    pub pickups: Vec<PickupView3d>,
    pub players: Vec<PlayerView>,
    pub zone: Option<P3>,
    pub events: Vec<Event3d>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg3d {
    Create { name: String, settings: Settings },
    Join { code: String, name: String },
    Input(Input3d),
    Chat(String),
    Start,
    Restart,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg3d {
    Welcome { code: String, you: u32, host: bool, settings: Settings },
    Rejected(String),
    Message { text: String, level: MsgLevel },
    Host(bool),
    View(View3d),
    Kicked,
}

pub use tansk_sim::proto::{decode, encode};
