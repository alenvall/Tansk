//! Things that happened during a tick, for juice, sound, kill feed and radio.
//! Drain them with `Sim::drain_events()` after each `step()`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::map::ScrapKind;
use crate::parts::{AltKind, GunKind, PartKind};
use crate::PlayerId;

/// Which part of a tank a hit landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitLocation {
    Hull,
    Turret,
    /// 0 = left (-Z), 1 = right (+Z).
    Track(u8),
    Part(PartKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weapon {
    Gun(GunKind),
    Alt(AltKind),
    Ram,
    /// Splash from your own shell, the world, etc.
    World,
    /// A rare part: Mortar, Harpoon, MineLayer.
    Special(PartKind),
    AirStrike,
    /// Exploding silo, fuel tank or oil drum; the wrecking ball.
    Hazard,
    /// Burning after a flamethrower hit.
    Burn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairKind {
    /// The wrench pickup: heals the hull and patches parts.
    Kit,
    /// The crew patching a blown track (0 = left, 1 = right).
    Track(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickupKind {
    Scrap(ScrapKind),
    Repair,
    /// A golden crate holding a rare part.
    Rare(PartKind),
    /// A golden flare crate: one air strike.
    AirStrike,
}

/// What a harpoon caught.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookTarget {
    Tank(PlayerId),
    Prop(u16),
    Pickup(u16),
    /// Hooked into the ground or a wall: the tank winches itself over.
    World,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A weapon went off. `pos` is the muzzle, `dir` the shot direction.
    Fired { player: PlayerId, weapon: Weapon, pos: Vec3, dir: Vec3 },
    /// A hitscan trace (MG, coilgun): from muzzle to where it stopped.
    Tracer { player: PlayerId, weapon: Weapon, from: Vec3, to: Vec3 },
    /// Damage landed on a tank. `ricochet` = a glancing hit on a steep plate.
    Hit { target: PlayerId, attacker: Option<PlayerId>, location: HitLocation, pos: Vec3, damage: f32, weapon: Weapon },
    /// A shell hit the world (terrain, rock, prop) without touching a tank.
    Impact { pos: Vec3, normal: Vec3, weapon: Weapon },
    Explosion { pos: Vec3, radius: f32, weapon: Weapon },
    TeslaArc { player: PlayerId, from: Vec3, targets: Vec<(PlayerId, Vec3)> },
    CoilCharging { player: PlayerId },
    PartBrokeOff { player: PlayerId, part: PartKind, pos: Vec3, prop_id: Option<u16> },
    TrackBlown { player: PlayerId, side: u8 },
    TrackRepaired { player: PlayerId, side: u8 },
    Destroyed { victim: PlayerId, killer: Option<PlayerId>, pos: Vec3, weapon: Weapon },
    Respawned { player: PlayerId, pos: Vec3 },
    PickedUp { player: PlayerId, kind: PickupKind, pos: Vec3 },
    /// A tier went up: `part` is the part that bolted on.
    UpgradeApplied { player: PlayerId, track: ScrapKind, tier: u8, part: Option<PartKind> },
    /// Tier 2: pick one of `options` (input.choice = index) before the timer runs out.
    BranchOffered { player: PlayerId, track: ScrapKind, options: Vec<PartKind>, default: PartKind },
    BranchChosen { player: PlayerId, track: ScrapKind, part: PartKind },
    RepairStarted { player: PlayerId, kind: RepairKind },
    RepairFinished { player: PlayerId, kind: RepairKind },
    BoostStarted { player: PlayerId },
    /// The crew rocked the tank back onto its tracks.
    Righted { player: PlayerId },
    Ram { a: PlayerId, b: PlayerId, pos: Vec3, speed: f32 },
    MatchEnded { winner: Option<PlayerId> },
    // --- Combo and rare parts.
    /// A combo or rare part bolted on (weld flash, clank).
    PartAttached { player: PlayerId, part: PartKind },
    ShieldHit { player: PlayerId, pos: Vec3, damage: f32 },
    ShieldBroken { player: PlayerId },
    ShieldRestored { player: PlayerId },
    Ignited { player: PlayerId, by: Option<PlayerId> },
    LockedOn { player: PlayerId, target: PlayerId },
    SmokeDeployed { player: PlayerId, pos: Vec3, radius: f32 },
    MineLaid { player: PlayerId, id: u16, pos: Vec3 },
    MineTriggered { id: u16, pos: Vec3, victim: Option<PlayerId> },
    HarpoonHooked { player: PlayerId, target: HookTarget, pos: Vec3 },
    HarpoonReleased { player: PlayerId },
    // --- Air strike (announced to everyone).
    /// Red flare pops at `target`, siren starts. Bombs fall `delay` seconds later
    /// along the line `from` -> `to`.
    AirStrikeCalled { player: PlayerId, target: Vec3, from: Vec3, to: Vec3, delay: f32 },
    /// The bombers are overhead: the run starts now.
    BombingRun { player: PlayerId, from: Vec3, to: Vec3 },
    // --- World.
    /// Destructible cover broke: hide `obstacle` (index into `Map::obstacles`).
    ObstacleDestroyed { obstacle: u16, kind: crate::map::ObstacleKind, pos: Vec3 },
    /// One container box of a stack was knocked loose and is now prop `prop_id`.
    ContainerToppled { obstacle: u16, box_index: u8, prop_id: u16 },
    /// A fuel silo, fuel tank or oil drum went up.
    HazardExploded { pos: Vec3, radius: f32 },
    /// A hydraulic launch plate threw a tank.
    Launched { player: PlayerId, pos: Vec3 },
    /// The wrecking ball hit a tank.
    CraneHit { player: PlayerId, pos: Vec3, speed: f32 },
    // --- Raider Waves.
    WaveStarted { wave: u16, raiders: u16, boss: bool },
    WaveCleared { wave: u16 },
    /// Aggregated outpost damage (about twice a second while it is under attack).
    OutpostHit { damage: f32, health: f32 },
    OutpostRepaired { amount: f32 },
    RaidLost { waves_survived: u16, reason: crate::raid::LossReason },
    BossSpawned { player: PlayerId, kind: crate::raid::BossKind, pos: Vec3 },
    /// A boss module was shot off (its behaviour changes; see `raid.rs`).
    BossModuleLost { player: PlayerId, part: PartKind, pos: Vec3 },
    /// The last plate is gone: the core takes full damage now.
    BossCoreExposed { player: PlayerId },
    BossDefeated { player: PlayerId, killer: Option<PlayerId>, seconds: f32 },
}
