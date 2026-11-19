//! The upgrade tracks, their parts, and hit boxes.
//!
//! Frames (all right-handed, **Y up**, metres):
//! - **Hull**: the tank body. +X forward, +Y up, +Z to the tank's right.
//!   Origin at the hull box centre (about 0.9 m above flat ground at rest).
//! - **Turret**: origin at `TurretTuning::pivot` in the hull frame, rotated by
//!   `turret_yaw` about +Y (positive yaw turns the turret to the left).
//! - **Gun**: origin at `TurretTuning::gun_pivot` in the turret frame, rotated by
//!   `gun_pitch` about +Z (positive pitch raises the barrel). The barrel lies
//!   along +X.
//!
//! A [`HitBox`] is an axis-aligned box in its part's frame (centre + half
//! extents). The client can draw a part as its boxes (or a model fitted to them).

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::map::ScrapKind;
use crate::tuning::{PartStat, Tuning};

/// An upgrade track. Same order as [`ScrapKind`].
pub type Track = ScrapKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    Hull,
    Turret,
    Gun,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitBox {
    pub center: Vec3,
    pub half: Vec3,
}

const fn hb(c: [f32; 3], h: [f32; 3]) -> HitBox {
    HitBox { center: Vec3::new(c[0], c[1], c[2]), half: Vec3::new(h[0], h[1], h[2]) }
}

/// Where a part mounts. Branch alternatives may share a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slot {
    /// The main gun (gun frame). Replaced, not stacked.
    Gun,
    /// Mast on the rear-left deck (hull frame).
    Mast,
    /// Right side of the turret (turret frame).
    TurretSide,
    /// Rear-right deck (hull frame).
    RearRight,
    /// Hull sides (hull frame).
    Flanks,
    /// Glacis / bow (hull frame).
    Front,
    /// Skirts all round (hull frame).
    Skirts,
    /// Hanging on the rear plate (hull frame).
    RearPlate,
    /// Engine deck, rear centre (hull frame).
    RearDeck,
    /// Flamethrower nozzle under the barrel (gun frame).
    Nozzle,
    /// Shield emitter dish on the turret roof (turret frame).
    Emitter,
    /// Rocket pod on the turret's left (turret frame).
    RocketPod,
    /// Spikes on the bow (hull frame).
    RamSpikes,
    /// The one rare-part mount (frame depends on the part).
    Rare,
    /// Boss modules (War Rig): each has its own mount.
    BossTurret,
    BossPods,
    BossFront,
    BossPlate,
    BossEngine,
    BossMast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartKind {
    // Powder
    StubGun,
    HeavyCannon,
    BigBore,
    TwinBarrel,
    LongGun,
    // Volts
    SensorMast,
    Coilgun,
    TeslaSpire,
    // Steel
    FlankPlates,
    Bulwark,
    SpacedArmour,
    // Fuel
    Nitro,
    Afterburner,
    TwinEngines,
    // Combos (two tracks at tier 2+; bolt on with the next upgrade).
    Flamethrower,
    ShieldGenerator,
    RocketRack,
    SpikedRam,
    // Rare parts (golden crates).
    Mortar,
    Harpoon,
    SmokeLauncher,
    MineLayer,
    RepairRig,
    // Boss modules: THE WAR RIG (hull frame of the rig, which is about 3x a tank).
    RigCannons,
    RigRockets,
    RigFlamer,
    RigPlateLeft,
    RigPlateRight,
    RigPlateFront,
    RigEngineLeft,
    RigEngineRight,
    RigMast,
}

pub struct PartDef {
    pub kind: PartKind,
    pub track: Track,
    pub slot: Slot,
    pub frame: Frame,
    pub hitboxes: &'static [HitBox],
}

const STUB_BOXES: &[HitBox] = &[hb([1.0, 0.0, 0.0], [1.0, 0.11, 0.11])];
const HEAVY_BOXES: &[HitBox] = &[hb([1.7, 0.0, 0.0], [1.5, 0.16, 0.16]), hb([0.15, 0.0, 0.0], [0.3, 0.3, 0.45])];
const BIG_BORE_BOXES: &[HitBox] = &[hb([1.5, 0.0, 0.0], [1.3, 0.32, 0.32]), hb([0.15, 0.0, 0.0], [0.35, 0.38, 0.55])];
const TWIN_BOXES: &[HitBox] = &[
    hb([1.6, 0.0, -0.28], [1.4, 0.14, 0.14]),
    hb([1.6, 0.0, 0.28], [1.4, 0.14, 0.14]),
    hb([0.15, 0.0, 0.0], [0.3, 0.3, 0.55]),
];
const LONG_BOXES: &[HitBox] = &[
    hb([2.7, 0.0, 0.0], [2.5, 0.13, 0.13]),
    hb([0.15, 0.0, 0.0], [0.3, 0.28, 0.4]),
    hb([0.4, 0.38, -0.35], [0.45, 0.11, 0.11]),
];
const MAST_BOXES: &[HitBox] = &[hb([-2.4, 2.0, -1.1], [0.12, 1.6, 0.12]), hb([-2.4, 3.7, -1.1], [0.35, 0.2, 0.35])];
const COIL_BOXES: &[HitBox] = &[hb([0.5, 0.55, 1.15], [1.2, 0.18, 0.22])];
const TESLA_BOXES: &[HitBox] = &[hb([-2.4, 1.2, 1.1], [0.25, 0.75, 0.25]), hb([-2.4, 2.25, 1.1], [0.4, 0.35, 0.4])];
const FLANK_BOXES: &[HitBox] = &[hb([0.0, 0.1, -1.92], [2.6, 0.5, 0.1]), hb([0.0, 0.1, 1.92], [2.6, 0.5, 0.1])];
const BULWARK_BOXES: &[HitBox] = &[hb([3.2, 0.1, 0.0], [0.18, 0.75, 1.85]), hb([3.5, -0.55, 0.0], [0.25, 0.25, 1.9])];
const SPACED_BOXES: &[HitBox] = &[
    hb([0.0, -0.15, -2.05], [3.0, 0.5, 0.08]),
    hb([0.0, -0.15, 2.05], [3.0, 0.5, 0.08]),
    hb([3.15, 0.1, 0.0], [0.08, 0.4, 1.6]),
    hb([-3.15, 0.1, 0.0], [0.08, 0.4, 1.6]),
];
const NITRO_BOXES: &[HitBox] = &[hb([-3.2, 0.2, 0.0], [0.2, 0.3, 0.9])];
const AFTERBURNER_BOXES: &[HitBox] = &[hb([-2.5, 0.8, 0.0], [0.55, 0.3, 0.45])];
const TWIN_ENGINE_BOXES: &[HitBox] = &[hb([-2.4, 0.8, -0.45], [0.5, 0.35, 0.35]), hb([-2.4, 0.8, 0.45], [0.5, 0.35, 0.35])];
const FLAME_BOXES: &[HitBox] = &[hb([1.1, -0.3, 0.32], [0.8, 0.12, 0.12])];
const SHIELD_BOXES: &[HitBox] = &[hb([-0.5, 1.0, 0.0], [0.38, 0.25, 0.38])];
const ROCKET_BOXES: &[HitBox] = &[hb([0.2, 0.6, -1.3], [0.8, 0.3, 0.3])];
const SPIKE_BOXES: &[HitBox] = &[hb([3.3, -0.35, 0.0], [0.35, 0.3, 1.7])];
const MORTAR_BOXES: &[HitBox] = &[hb([-1.7, 0.85, -1.35], [0.3, 0.45, 0.3])];
const HARPOON_BOXES: &[HitBox] = &[hb([0.3, 0.95, 0.5], [0.9, 0.15, 0.15])];
const SMOKE_BOXES: &[HitBox] = &[hb([0.9, 0.65, -0.95], [0.2, 0.15, 0.2]), hb([0.9, 0.65, 0.95], [0.2, 0.15, 0.2])];
const MINE_BOXES: &[HitBox] = &[hb([-3.2, -0.35, 0.0], [0.2, 0.15, 1.2])];
const RIG_BOXES: &[HitBox] = &[hb([1.9, 0.75, -0.8], [0.5, 0.3, 0.4])];

pub fn part_def(kind: PartKind) -> PartDef {
    use PartKind::*;
    let (track, slot, frame, hitboxes) = match kind {
        StubGun => (Track::Powder, Slot::Gun, Frame::Gun, STUB_BOXES),
        HeavyCannon => (Track::Powder, Slot::Gun, Frame::Gun, HEAVY_BOXES),
        BigBore => (Track::Powder, Slot::Gun, Frame::Gun, BIG_BORE_BOXES),
        TwinBarrel => (Track::Powder, Slot::Gun, Frame::Gun, TWIN_BOXES),
        LongGun => (Track::Powder, Slot::Gun, Frame::Gun, LONG_BOXES),
        SensorMast => (Track::Volts, Slot::Mast, Frame::Hull, MAST_BOXES),
        Coilgun => (Track::Volts, Slot::TurretSide, Frame::Turret, COIL_BOXES),
        TeslaSpire => (Track::Volts, Slot::RearRight, Frame::Hull, TESLA_BOXES),
        FlankPlates => (Track::Steel, Slot::Flanks, Frame::Hull, FLANK_BOXES),
        Bulwark => (Track::Steel, Slot::Front, Frame::Hull, BULWARK_BOXES),
        SpacedArmour => (Track::Steel, Slot::Skirts, Frame::Hull, SPACED_BOXES),
        Nitro => (Track::Fuel, Slot::RearPlate, Frame::Hull, NITRO_BOXES),
        Afterburner => (Track::Fuel, Slot::RearDeck, Frame::Hull, AFTERBURNER_BOXES),
        TwinEngines => (Track::Fuel, Slot::RearDeck, Frame::Hull, TWIN_ENGINE_BOXES),
        Flamethrower => (Track::Powder, Slot::Nozzle, Frame::Gun, FLAME_BOXES),
        ShieldGenerator => (Track::Volts, Slot::Emitter, Frame::Turret, SHIELD_BOXES),
        RocketRack => (Track::Powder, Slot::RocketPod, Frame::Turret, ROCKET_BOXES),
        SpikedRam => (Track::Steel, Slot::RamSpikes, Frame::Hull, SPIKE_BOXES),
        Mortar => (Track::Powder, Slot::Rare, Frame::Hull, MORTAR_BOXES),
        Harpoon => (Track::Steel, Slot::Rare, Frame::Turret, HARPOON_BOXES),
        SmokeLauncher => (Track::Volts, Slot::Rare, Frame::Turret, SMOKE_BOXES),
        MineLayer => (Track::Fuel, Slot::Rare, Frame::Hull, MINE_BOXES),
        RepairRig => (Track::Steel, Slot::Rare, Frame::Hull, RIG_BOXES),
        RigCannons => (Track::Powder, Slot::BossTurret, Frame::Turret, RIG_CANNON_BOXES),
        RigRockets => (Track::Powder, Slot::BossPods, Frame::Hull, RIG_ROCKET_BOXES),
        RigFlamer => (Track::Fuel, Slot::BossFront, Frame::Hull, RIG_FLAMER_BOXES),
        RigPlateLeft => (Track::Steel, Slot::BossPlate, Frame::Hull, RIG_PLATE_L_BOXES),
        RigPlateRight => (Track::Steel, Slot::BossPlate, Frame::Hull, RIG_PLATE_R_BOXES),
        RigPlateFront => (Track::Steel, Slot::BossPlate, Frame::Hull, RIG_PLATE_F_BOXES),
        RigEngineLeft => (Track::Fuel, Slot::BossEngine, Frame::Hull, RIG_ENGINE_L_BOXES),
        RigEngineRight => (Track::Fuel, Slot::BossEngine, Frame::Hull, RIG_ENGINE_R_BOXES),
        RigMast => (Track::Volts, Slot::BossMast, Frame::Hull, RIG_MAST_BOXES),
    };
    PartDef { kind, track, slot, frame, hitboxes }
}

/// Base hit boxes that are not parts.
pub const HULL_BOX: HitBox = hb([0.0, 0.0, 0.0], [2.95, 0.45, 1.1]);
/// Track boxes, hull frame: index 0 = left (-Z), 1 = right (+Z).
pub const TRACK_BOXES: [HitBox; 2] = [hb([0.0, -0.4, -1.45], [3.05, 0.55, 0.36]), hb([0.0, -0.4, 1.45], [3.05, 0.55, 0.36])];
/// Turret box, turret frame.
pub const TURRET_BOX: HitBox = hb([0.0, 0.38, 0.0], [1.25, 0.38, 1.1]);

/// The branch options offered when a track reaches tier 2 (first = default).
pub fn branch_options(track: Track) -> &'static [PartKind] {
    match track {
        Track::Powder => &[PartKind::BigBore, PartKind::TwinBarrel, PartKind::LongGun],
        Track::Volts => &[PartKind::Coilgun, PartKind::TeslaSpire],
        Track::Steel => &[PartKind::Bulwark, PartKind::SpacedArmour],
        Track::Fuel => &[PartKind::Afterburner, PartKind::TwinEngines],
    }
}

/// The tier-1 part of a track.
pub fn tier1_part(track: Track) -> PartKind {
    match track {
        Track::Powder => PartKind::HeavyCannon,
        Track::Volts => PartKind::SensorMast,
        Track::Steel => PartKind::FlankPlates,
        Track::Fuel => PartKind::Nitro,
    }
}

/// Parts present on a track at `tier` with `branch` chosen, as (kind, tier, mk).
///
/// Powder replaces its gun each tier; the other tracks stack their tier-1
/// part with the branch part. Tier 3 is the branch part's Mk II.
/// TODO(M2): combo parts (two tracks at tier 2+) and rare finds.
pub fn parts_for(track: Track, tier: u8, branch: Option<PartKind>) -> Vec<(PartKind, u8, u8)> {
    let mut out = Vec::new();
    if tier == 0 {
        return out;
    }
    let branch = branch.unwrap_or(branch_options(track)[0]);
    let mk = if tier >= 3 { 2 } else { 1 };
    if track == Track::Powder {
        if tier == 1 {
            out.push((PartKind::HeavyCannon, 1, 1));
        } else {
            out.push((branch, tier.min(3), mk));
        }
    } else {
        out.push((tier1_part(track), 1, 1));
        if tier >= 2 {
            out.push((branch, tier.min(3), mk));
        }
    }
    out
}

pub fn part_stat(t: &Tuning, kind: PartKind) -> PartStat {
    let p = &t.parts;
    match kind {
        PartKind::StubGun => PartStat { health: f32::INFINITY, mass: 0.0 },
        PartKind::HeavyCannon => p.heavy_cannon,
        PartKind::BigBore => p.big_bore,
        PartKind::TwinBarrel => p.twin_barrel,
        PartKind::LongGun => p.long_gun,
        PartKind::SensorMast => p.sensor_mast,
        PartKind::Coilgun => p.coilgun,
        PartKind::TeslaSpire => p.tesla_spire,
        PartKind::FlankPlates => p.flank_plates,
        PartKind::Bulwark => p.bulwark,
        PartKind::SpacedArmour => p.spaced_armour,
        PartKind::Nitro => p.nitro,
        PartKind::Afterburner => p.afterburner,
        PartKind::TwinEngines => p.twin_engines,
        PartKind::Flamethrower => p.flamethrower,
        PartKind::ShieldGenerator => p.shield_generator,
        PartKind::RocketRack => p.rocket_rack,
        PartKind::SpikedRam => p.spiked_ram,
        PartKind::Mortar => p.mortar,
        PartKind::Harpoon => p.harpoon,
        PartKind::SmokeLauncher => p.smoke_launcher,
        PartKind::MineLayer => p.mine_layer,
        PartKind::RepairRig => p.repair_rig,
        PartKind::RigCannons => p.rig_cannons,
        PartKind::RigRockets => p.rig_rockets,
        PartKind::RigFlamer => p.rig_flamer,
        PartKind::RigPlateLeft | PartKind::RigPlateRight | PartKind::RigPlateFront => p.rig_plate,
        PartKind::RigEngineLeft | PartKind::RigEngineRight => p.rig_engine,
        PartKind::RigMast => p.rig_mast,
    }
}

/// A part bolted onto a tank.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub kind: PartKind,
    pub slot: Slot,
    pub track: Track,
    /// The track tier this part represents (losing it drops the track below this).
    pub tier: u8,
    /// 1, or 2 for a tier-3 "Mk II".
    pub mk: u8,
    pub health: f32,
    pub max_health: f32,
    pub mass: f32,
}

impl Part {
    pub fn new(t: &Tuning, kind: PartKind, tier: u8, mk: u8) -> Part {
        let def = part_def(kind);
        let st = part_stat(t, kind);
        let (hm, mm) = if mk >= 2 { (t.parts.mk2_health, t.parts.mk2_mass) } else { (1.0, 1.0) };
        Part {
            kind,
            slot: def.slot,
            track: def.track,
            tier,
            mk,
            health: st.health * hm,
            max_health: st.health * hm,
            mass: st.mass * mm,
        }
    }

    pub fn def(&self) -> PartDef {
        part_def(self.kind)
    }
}

/// Main gun kinds (the Powder track's output).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GunKind {
    Stub,
    HeavyCannon,
    BigBore,
    TwinBarrel,
    LongGun,
    /// THE WAR RIG's twin cannon (boss only).
    RigCannons,
}

/// Alt-fire kinds (the Volts track's output; MG when none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AltKind {
    Mg,
    Coilgun,
    Tesla,
    /// Flamethrower combo (replaces the Volts alt while bolted on).
    Flamethrower,
    /// Rocket rack combo.
    Rockets,
}

/// What a tank's parts add up to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TankStats {
    pub mass: f32,
    pub max_health: f32,
    pub gun: GunKind,
    pub gun_mk: u8,
    pub alt: AltKind,
    pub alt_mk: u8,
    /// Eye height above the hull centre for sight checks (bots, spotting).
    pub sight_height: f32,
    pub speed_mult: f32,
    pub traverse_mult: f32,
    pub can_boost: bool,
    pub fuel_max: f32,
    pub ram_mult: f32,
    pub boost_ram_mult: f32,
    pub splash_mult: f32,
    /// Centre of mass, hull frame.
    /// Shield bubble capacity (0 = no shield generator).
    pub shield_max: f32,
    /// Spiked ram: ram damage grows with speed.
    pub spiked: bool,
    /// The rare part bolted on, if any.
    pub rare: Option<PartKind>,
    /// Passive repair-rig regeneration, HP/s.
    pub regen: f32,
    pub com: Vec3,
}

pub fn compute_stats(t: &Tuning, parts: &[Part], tiers: &[u8; 4]) -> TankStats {
    compute_stats_on(t, &t.tank, parts, tiers)
}

/// Like [`compute_stats`] on a given chassis.
pub fn compute_stats_on(t: &Tuning, tt: &crate::tuning::TankTuning, parts: &[Part], tiers: &[u8; 4]) -> TankStats {
    let mut s = TankStats {
        mass: tt.base_mass,
        max_health: tt.base_health + t.parts.steel_health_per_tier * tiers[Track::Steel.index()] as f32,
        gun: GunKind::Stub,
        gun_mk: 1,
        alt: AltKind::Mg,
        alt_mk: 1,
        sight_height: 1.6,
        speed_mult: 1.0,
        traverse_mult: 1.0,
        can_boost: tiers[Track::Fuel.index()] >= 1,
        fuel_max: tt.fuel_max,
        ram_mult: tt.ram_mult,
        boost_ram_mult: 1.0,
        splash_mult: 1.0,
        com: Vec3::new(0.0, tt.com_height, 0.0),
        shield_max: 0.0,
        spiked: false,
        rare: None,
        regen: 0.0,
    };
    let mut moment = s.com * s.mass;
    for p in parts {
        let def = p.def();
        let c = def.hitboxes.iter().map(|h| h.center).sum::<Vec3>() / def.hitboxes.len().max(1) as f32;
        let c = match def.frame {
            Frame::Hull => c,
            _ => Vec3::from(t.turret.pivot) + Vec3::Y * 0.4,
        };
        moment += c * p.mass;
        s.mass += p.mass;
        match p.kind {
            PartKind::HeavyCannon => s.gun = GunKind::HeavyCannon,
            PartKind::BigBore => s.gun = GunKind::BigBore,
            PartKind::TwinBarrel => s.gun = GunKind::TwinBarrel,
            PartKind::LongGun => s.gun = GunKind::LongGun,
            PartKind::SensorMast => s.sight_height += t.parts.mast_sight,
            PartKind::Coilgun => s.alt = AltKind::Coilgun,
            PartKind::TeslaSpire => s.alt = AltKind::Tesla,
            PartKind::Bulwark => s.ram_mult *= t.parts.bulwark_ram * if p.mk >= 2 { 1.3 } else { 1.0 },
            PartKind::SpacedArmour => {
                s.splash_mult *= t.parts.spaced_splash * if p.mk >= 2 { 0.8 } else { 1.0 }
            }
            PartKind::Afterburner => {
                s.fuel_max *= t.parts.afterburner_fuel * if p.mk >= 2 { 1.3 } else { 1.0 };
                s.boost_ram_mult *= t.parts.afterburner_ram;
            }
            PartKind::Flamethrower => s.alt = AltKind::Flamethrower,
            PartKind::RocketRack => s.alt = AltKind::Rockets,
            PartKind::ShieldGenerator => s.shield_max = t.special.shield_hp,
            PartKind::SpikedRam => s.spiked = true,
            PartKind::Mortar | PartKind::Harpoon | PartKind::SmokeLauncher | PartKind::MineLayer => s.rare = Some(p.kind),
            PartKind::RigCannons => s.gun = GunKind::RigCannons,
            PartKind::RigRockets => s.alt = AltKind::Rockets,
            PartKind::RigMast => s.sight_height += t.parts.mast_sight * 2.0,
            PartKind::RepairRig => {
                s.rare = Some(p.kind);
                s.regen = t.special.repair_rig_hps;
            }
            PartKind::TwinEngines => {
                let mk = if p.mk >= 2 { 1.08 } else { 1.0 };
                s.speed_mult *= t.parts.twin_engines_speed * mk;
                s.traverse_mult *= t.parts.twin_engines_traverse * mk;
            }
            _ => {}
        }
        if p.track == Track::Powder && p.tier > 0 {
            s.gun_mk = p.mk;
        }
        if p.track == Track::Volts && p.tier >= 2 {
            s.alt_mk = p.mk;
        }
    }
    s.com = moment / s.mass;
    s
}

/// The four combo parts, keyed by the two tracks that unlock them.
pub const COMBOS: [(Track, Track, PartKind); 4] = [
    (Track::Powder, Track::Fuel, PartKind::Flamethrower),
    (Track::Volts, Track::Steel, PartKind::ShieldGenerator),
    (Track::Powder, Track::Volts, PartKind::RocketRack),
    (Track::Steel, Track::Fuel, PartKind::SpikedRam),
];
// TODO(M3): Powder + Steel and Volts + Fuel combos.

/// Rare parts found in golden crates.
pub const RARE_PARTS: [PartKind; 5] =
    [PartKind::Mortar, PartKind::Harpoon, PartKind::SmokeLauncher, PartKind::MineLayer, PartKind::RepairRig];

/// Combo and rare parts are not tier parts: they sit on a tank with `tier == 0`
/// and breaking one off costs no tier.
pub fn is_special(kind: PartKind) -> bool {
    BOSS_MODULES.contains(&kind) || COMBOS.iter().any(|c| c.2 == kind) || RARE_PARTS.contains(&kind)
}

/// Combos a tank with these tiers qualifies for (both tracks at tier 2+).
pub fn combos_unlocked(tiers: &[u8; 4]) -> Vec<PartKind> {
    COMBOS
        .iter()
        .filter(|(a, b, _)| tiers[a.index()] >= 2 && tiers[b.index()] >= 2)
        .map(|c| c.2)
        .collect()
}

/// THE WAR RIG's modules. All are shot off one by one; the core (the hull)
/// only takes full damage once the three plates are gone.
pub const BOSS_MODULES: [PartKind; 9] = [
    PartKind::RigCannons,
    PartKind::RigRockets,
    PartKind::RigFlamer,
    PartKind::RigPlateLeft,
    PartKind::RigPlateRight,
    PartKind::RigPlateFront,
    PartKind::RigEngineLeft,
    PartKind::RigEngineRight,
    PartKind::RigMast,
];

pub const RIG_PLATES: [PartKind; 3] = [PartKind::RigPlateLeft, PartKind::RigPlateRight, PartKind::RigPlateFront];

// War Rig module boxes, in the rig's frames (hull half extents 7.5 x 1.4 x 3.5;
// turret pivot at (-0.75, 1.4, 0)).
const RIG_CANNON_BOXES: &[HitBox] = &[
    hb([0.0, 1.2, 0.0], [3.2, 1.25, 2.25]),
    hb([5.0, 1.3, -0.8], [2.6, 0.3, 0.3]),
    hb([5.0, 1.3, 0.8], [2.6, 0.3, 0.3]),
];
const RIG_ROCKET_BOXES: &[HitBox] = &[hb([-4.6, 2.2, -2.4], [1.2, 0.8, 0.7]), hb([-4.6, 2.2, 2.4], [1.2, 0.8, 0.7])];
const RIG_FLAMER_BOXES: &[HitBox] = &[hb([8.1, 0.3, 0.0], [0.45, 0.5, 1.0])];
const RIG_PLATE_L_BOXES: &[HitBox] = &[hb([0.0, 0.2, -3.75], [6.6, 1.3, 0.25])];
const RIG_PLATE_R_BOXES: &[HitBox] = &[hb([0.0, 0.2, 3.75], [6.6, 1.3, 0.25])];
const RIG_PLATE_F_BOXES: &[HitBox] = &[hb([7.7, 0.5, 0.0], [0.25, 1.2, 3.3])];
const RIG_ENGINE_L_BOXES: &[HitBox] = &[hb([-6.5, 1.9, -1.6], [1.0, 0.8, 1.2])];
const RIG_ENGINE_R_BOXES: &[HitBox] = &[hb([-6.5, 1.9, 1.6], [1.0, 0.8, 1.2])];
const RIG_MAST_BOXES: &[HitBox] = &[hb([-2.5, 4.4, -2.6], [0.25, 2.4, 0.25]), hb([-2.5, 7.0, -2.6], [0.5, 0.4, 0.5])];

/// What a machine is built on. Tanks are the players' machines; the rest are raiders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VehicleKind {
    #[default]
    Tank,
    /// Fast, light, flips easily; MG only; rams and kamikazes the outpost.
    Buggy,
    /// A pickup with a rocket rack in the bed.
    Technical,
    /// THE WAR RIG boss.
    WarRig,
}
