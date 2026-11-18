//! What the server and the browser say to each other over the WebSocket
//! (bincode), and the per-tick view of the game both of them draw from.

use serde::{Deserialize, Serialize};

use crate::game::Input;
use crate::geom::V2;

/// The tank colours, by sprite name (`tank_<color>.png`).
pub const COLORS: [&str; 10] = [
    "blue", "red", "yellow", "green", "white", "pink", "orange", "purple", "teal", "brown",
];

pub fn color_name(c: u8) -> &'static str {
    COLORS.get(c as usize).copied().unwrap_or("white")
}

/// Host settings (HostMenu): times in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub koth: bool,
    pub rounds: i32,
    pub lives: i32,
    pub spawn_time_ms: i32,
    pub round_time_ms: i32,
    pub game_time_ms: i32,
    pub score_limit: i32,
}

impl Default for Settings {
    /// The host menu's slider defaults.
    fn default() -> Self {
        Settings {
            koth: false,
            rounds: 4,
            lives: 5,
            spawn_time_ms: 5_000,
            round_time_ms: 300_000,
            game_time_ms: 900_000,
            score_limit: 15,
        }
    }
}

impl Settings {
    /// The test arena's settings (Play.enter).
    pub fn playground() -> Self {
        Settings {
            koth: true,
            rounds: 4,
            lives: 0,
            spawn_time_ms: 5_000,
            round_time_ms: 500_000,
            game_time_ms: 1_500_000,
            score_limit: 15,
        }
    }

    /// Kept within the host menu's slider ranges.
    pub fn clamped(mut self) -> Self {
        self.rounds = self.rounds.clamp(1, 30);
        self.lives = self.lives.clamp(0, 10);
        self.spawn_time_ms = self.spawn_time_ms.clamp(1_000, 20_000);
        self.round_time_ms = self.round_time_ms.clamp(30_000, 600_000);
        self.game_time_ms = self.game_time_ms.clamp(60_000, 1_800_000);
        self.score_limit = self.score_limit.clamp(1, 100);
        self
    }

    /// King of the hill has no lives: tanks respawn forever.
    pub fn player_lives(&self) -> i32 {
        if self.koth {
            0
        } else {
            self.lives.max(1)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weapon {
    Default,
    Bounce,
    Flamethrower,
    Shockwave,
    Shotgun,
    Slowspeedy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Power {
    Health,
    Shield,
    Speed,
    FireRate,
    Damage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickupKind {
    Weapon(Weapon),
    Power(Power),
}

impl PickupKind {
    pub fn sprite(self) -> &'static str {
        match self {
            PickupKind::Weapon(Weapon::Bounce) => "BounceGunIcon",
            PickupKind::Weapon(Weapon::Flamethrower) => "FlamethrowerIcon",
            PickupKind::Weapon(Weapon::Shockwave) => "ShockwaveIcon",
            PickupKind::Weapon(Weapon::Shotgun) => "ShotgunIcon",
            PickupKind::Weapon(Weapon::Slowspeedy) | PickupKind::Weapon(Weapon::Default) => "SlowspeedyIcon",
            PickupKind::Power(Power::Health) => "health_powerup",
            PickupKind::Power(Power::Shield) => "shield_powerup",
            PickupKind::Power(Power::Speed) => "speed_powerup",
            PickupKind::Power(Power::FireRate) => "firerate_powerup",
            PickupKind::Power(Power::Damage) => "damage_powerup",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjKind {
    Default,
    Bounce,
    Flame,
    Shockwave,
    ShockwaveSecondary,
    Shotgun,
    Slowspeedy,
}

impl ProjKind {
    pub fn sprite(self) -> &'static str {
        match self {
            ProjKind::Default => "bullet",
            ProjKind::Bounce | ProjKind::Flame => "proj_fire",
            ProjKind::Shockwave => "shockwave_proj",
            ProjKind::ShockwaveSecondary | ProjKind::Slowspeedy => "proj_energy",
            ProjKind::Shotgun => "proj_shotgun",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sound {
    TankDestroyed,
    TankHit,
    DefaultFire,
    Flamethrower,
    Shotgun,
    SlowspeedyFire,
    SlowspeedySecondary,
    ShockwaveDetonate,
}

impl Sound {
    pub const ALL: [Sound; 8] = [
        Sound::TankDestroyed,
        Sound::TankHit,
        Sound::DefaultFire,
        Sound::Flamethrower,
        Sound::Shotgun,
        Sound::SlowspeedyFire,
        Sound::SlowspeedySecondary,
        Sound::ShockwaveDetonate,
    ];

    /// SoundHandler.playSound's mapping of events to files.
    pub fn file(self) -> &'static str {
        match self {
            Sound::TankDestroyed => "Tank_Destroyed",
            Sound::TankHit => "Tank_Hit",
            Sound::DefaultFire => "Default_Firing",
            Sound::Flamethrower => "Flamethrower_Firing",
            Sound::Shotgun => "Shotgun_Firing",
            Sound::SlowspeedySecondary => "Slowspeedy_Firing",
            Sound::SlowspeedyFire => "Shockwave_Firing",
            Sound::ShockwaveDetonate => "Shockwave_Detonate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Sound(Sound),
    Explosion(V2),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Lobby,
    Countdown,
    Playing,
    GameOver,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TankView {
    pub id: u32,
    pub player: u32,
    pub color: u8,
    pub pos: V2,
    pub theta: f32,
    pub health: f32,
    pub shield: Option<f32>,
    pub turret_pos: V2,
    pub turret_angle: f32,
    pub weapon: Weapon,
    pub ammo: i32,
    pub power: Option<Power>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjView {
    pub id: u32,
    pub kind: ProjKind,
    pub pos: V2,
    pub theta: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PickupView {
    pub id: u32,
    pub kind: PickupKind,
    pub pos: V2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u32,
    pub name: String,
    pub color: u8,
    pub score: i32,
    pub lives: i32,
    pub active: bool,
    pub eliminated: bool,
}

/// Everything drawn in one tick (the original's Pck100_WorldState and friends).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub tick: u32,
    pub phase: Phase,
    pub delay_ms: i32,
    pub round: i32,
    pub round_ms_left: i32,
    pub game_ms_left: i32,
    pub tanks: Vec<TankView>,
    pub projectiles: Vec<ProjView>,
    pub pickups: Vec<PickupView>,
    pub players: Vec<PlayerView>,
    pub zone: Option<V2>,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// Open a new room and join it as its host.
    Create { name: String, settings: Settings },
    /// Join an open room by its code.
    Join { code: String, name: String },
    Input(Input),
    Chat(String),
    /// Host only: start the game.
    Start,
    /// Host only: back to the lobby with the same players.
    Restart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgLevel {
    Chat,
    Info,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome { code: String, you: u32, host: bool, settings: Settings },
    Rejected(String),
    Message { text: String, level: MsgLevel },
    Host(bool),
    View(GameView),
    Kicked,
}

pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    bincode::serialize(msg).expect("serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Option<T> {
    bincode::deserialize(bytes).ok()
}
