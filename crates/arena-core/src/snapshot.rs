//! What the client draws: a compact, serde + bincode (varint) world state.
//!
//! The map itself is not in here: build it from the seed with
//! `Map::the_sink(seed)`. Quantisation: rotations are `[x, y, z, w]` * 32767
//! as i16; wheel compression is 0..255 of `suspension_travel + wheel_radius`;
//! health-like fractions are 0..255.

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::events::{PickupKind, RepairKind};
use crate::map::ScrapKind;
use crate::parts::{AltKind, GunKind, PartKind};
use crate::sim::{Phase, PropKind};
use crate::PlayerId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u32,
    pub tanks: Vec<TankSnap>,
    pub projectiles: Vec<ProjectileSnap>,
    pub pickups: Vec<PickupSnap>,
    pub props: Vec<PropSnap>,
    pub scores: Vec<ScoreSnap>,
    pub mode: ModeSnap,
    // --- Added in wave 2 (combos, rares, air strike, destruction, hazards).
    pub smokes: Vec<SmokeSnap>,
    pub mines: Vec<MineSnap>,
    pub strikes: Vec<StrikeSnap>,
    pub hazards: Vec<HazardSnap>,
    /// Destroyed obstacles and loose container boxes: hide these from the static map.
    pub removed: Vec<RemovedSnap>,
    pub tethers: Vec<TetherSnap>,
    // --- Added in wave 3 (Raider Waves).
    /// Outpost, wave number and countdown (None in Scrap Brawl).
    pub raid: Option<crate::raid::RaidSnap>,
    /// Living bosses and their modules.
    pub bosses: Vec<crate::raid::BossSnap>,
    /// Team Brawl scores (empty in other modes).
    pub teams: Vec<TeamScoreSnap>,
    /// Sandstorm intensity, 0 (clear) ..= 1 (full storm).
    pub storm: f32,
    // --- Added in wave 7.
    /// Convoy: the truck, the round and the score line (None in other modes).
    pub convoy: Option<crate::convoy::ConvoySnap>,
    // --- Added in wave 8.
    /// Wellhead: the derricks and both fuel tanks (None in other modes).
    pub wellhead: Option<crate::wellhead::WellheadSnap>,
    /// Last Crew Rolling: the sandstorm wall (None in other modes).
    pub storm_circle: Option<crate::lastcrew::StormCircleSnap>,
    // --- Destruction (`destruct.rs`).
    /// Obstacles dented (stage 1).
    pub dented: Vec<u16>,
    /// What destroyed staged cover left behind.
    pub rubble: Vec<crate::destruct::RubbleSnap>,
    /// Craters dug this match, in order (dig them in this order).
    pub craters: Vec<crate::destruct::CraterSnap>,
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TankSnap {
    pub player: PlayerId,
    pub alive: bool,
    pub pos: [f32; 3],
    pub rot: [i16; 4],
    pub linvel: [f32; 3],
    pub angvel: [f32; 3],
    pub turret_yaw: f32,
    pub gun_pitch: f32,
    /// Per road wheel, left side first, front to back. 0..255 of full ray length.
    pub wheels: Vec<u8>,
    /// Track surface speeds (left, right), cm/s.
    pub track_speed: [i16; 2],
    pub health: u16,
    pub max_health: u16,
    pub tracks: [TrackSnap; 2],
    pub parts: Vec<PartSnap>,
    pub tiers: [u8; 4],
    /// Scrap held toward the next tier, per kind.
    pub scrap: [u8; 4],
    pub gun: GunKind,
    pub alt: AltKind,
    /// 0..255 of fuel_max.
    pub fuel: u8,
    pub boosting: bool,
    /// Aim bloom 0..255 (reticle size).
    pub bloom: u8,
    /// Coilgun charge 0..255.
    pub charge: u8,
    /// Main gun reload remaining, 0..255 of the full reload.
    pub reload: u8,
    pub repair: Option<RepairSnap>,
    pub choice: Option<ChoiceSnap>,
    /// Ticks until respawn (when dead).
    pub respawn_in: u16,
    pub protected: bool,
    pub mass: f32,
    // --- Added in wave 2.
    /// Shield bubble 0..255 of capacity (0 with no shield generator or when down).
    pub shield: u8,
    pub burning: bool,
    /// Flamethrower jetting this tick (draw the flame cone).
    pub flaming: bool,
    /// Rocket rack lock: who, and progress 0..255 (255 = locked).
    pub lock: Option<PlayerId>,
    pub lock_progress: u8,
    /// Charges left on the rare part (mortar rounds, smoke, mines).
    pub rare_ammo: u8,
    /// Holding an air-strike flare (press special to call it on the aim point).
    pub air_strike: bool,
    // --- Added in wave 3.
    /// 255 = no team (free-for-all); 0 defenders, 1 raiders.
    pub team: u8,
    /// Chassis to draw (tank, buggy, technical, War Rig).
    pub vehicle: crate::parts::VehicleKind,
    // --- Added in wave 6 (Garage).
    pub chassis: crate::garage::ChassisClass,
    pub paint: u8,
    pub emblem: u8,
    // --- Power-ups.
    /// The timed buffs this tank has now (empty almost always).
    pub powerups: Vec<crate::powerups::PowerUpSnap>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackSnap {
    /// 0..255 of track health.
    pub health: u8,
    pub blown: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartSnap {
    pub kind: PartKind,
    pub mk: u8,
    /// 0..255 of max health.
    pub health: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepairSnap {
    pub kind: RepairKind,
    /// 0..255 progress.
    pub progress: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceSnap {
    pub track: ScrapKind,
    pub options: Vec<PartKind>,
    /// The option applied right now (the default until the player picks).
    pub current: PartKind,
    pub ticks_left: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectileSnap {
    pub id: u16,
    pub owner: PlayerId,
    pub gun: GunKind,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    /// Added in wave 2: what the projectile is (a shell of `gun`, or a special).
    pub shot: ShotKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PickupSnap {
    pub id: u16,
    pub kind: PickupKind,
    pub pos: [f32; 3],
    /// Spilled from a tank (vs sitting on a site).
    pub dropped: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropSnap {
    pub id: u16,
    pub kind: PropKind,
    pub pos: [f32; 3],
    pub rot: [i16; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoreSnap {
    pub player: PlayerId,
    pub kills: u16,
    pub deaths: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModeSnap {
    pub phase: Phase,
    pub ticks_left: u32,
    pub kill_limit: u16,
}

pub fn q_rot(q: Quat) -> [i16; 4] {
    let q = q.normalize();
    [q.x, q.y, q.z, q.w].map(|c| (c.clamp(-1.0, 1.0) * 32767.0).round() as i16)
}

pub fn dq_rot(r: [i16; 4]) -> Quat {
    Quat::from_xyzw(r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32).normalize()
}

pub fn q01(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn opts() -> impl bincode::Options {
    use bincode::Options;
    bincode::DefaultOptions::new().with_varint_encoding()
}

impl Snapshot {
    /// bincode with varint integers.
    pub fn encode(&self) -> Vec<u8> {
        use bincode::Options;
        opts().serialize(self).expect("snapshot serializes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Snapshot, String> {
        use bincode::Options;
        opts().deserialize(bytes).map_err(|e| e.to_string())
    }

    pub fn tank(&self, player: PlayerId) -> Option<&TankSnap> {
        self.tanks.iter().find(|t| t.player == player)
    }
}

impl TankSnap {
    pub fn position(&self) -> Vec3 {
        Vec3::from(self.pos)
    }
    pub fn rotation(&self) -> Quat {
        dq_rot(self.rot)
    }
}

/// What a projectile is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShotKind {
    /// A main-gun shell (see `ProjectileSnap::gun`).
    Shell,
    Rocket,
    MortarShell,
    Harpoon,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmokeSnap {
    pub id: u16,
    pub pos: [f32; 3],
    pub radius: f32,
    /// 0..255 of the cloud's life left (fade it out).
    pub life: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MineSnap {
    pub id: u16,
    pub owner: PlayerId,
    pub pos: [f32; 3],
    pub armed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrikeSnap {
    pub id: u16,
    pub owner: PlayerId,
    /// The flare.
    pub target: [f32; 3],
    /// The carpet line the bombs fall along.
    pub from: [f32; 3],
    pub to: [f32; 3],
    /// Ticks until the first bomb (0 while the run is under way).
    pub ticks_left: u16,
    /// Bombs dropped so far.
    pub dropped: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HazardKind {
    FuelSilo,
    FuelTank,
    LaunchPlate,
    Crane,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HazardSnap {
    /// Index into `Map::obstacles`.
    pub obstacle: u16,
    pub kind: HazardKind,
    /// 0..255 health (fuel hazards); launch plates: 255 when charged.
    pub health: u8,
    /// Plates: true when charged. Fuel hazards: false once blown.
    pub ready: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemovedSnap {
    pub obstacle: u16,
    /// Box index, or 255 for the whole obstacle.
    pub box_index: u8,
}

/// A taut harpoon line from a tank to what it hooked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TetherSnap {
    pub player: PlayerId,
    pub from: [f32; 3],
    pub to: [f32; 3],
}

/// One team's score in Team Brawl (team 0 Rustbacks, team 1 Dust Saints).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeamScoreSnap {
    pub team: u8,
    pub kills: u16,
}
