//! The recorded sounds: which Ogg files back each [`SoundId`], the engine
//! and weather loops, the order they load in, and how the engine loops are
//! crossfaded from what the engine is doing. Platform-free (the browser layer
//! in `web` fetches and plays them; the tests check the tables here).
//!
//! Every file is a real recording (Sonniss #GameAudioGDC bundles; sources
//! and licences in `crates/arena-client/assets/sounds/LICENSES.md`), cut,
//! layered and normalised offline to -1 dBFS peak. A sound with no files
//! stays synthesized, and every sound falls back to its synthesized takes
//! until (or unless) its files have loaded.

use crate::engine::{EngineInput, EngineKind};
use crate::sounds::SoundId;

/// Where the files are served from, relative to the page (trunk copies
/// `crates/arena-client/assets/sounds` to `dist/sounds`).
pub const BASE_URL: &str = "sounds/";

/// One recorded take: its file stem and a level trim (linear) that evens out
/// the loudness of files that were all peak-normalised.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Take {
    pub file: &'static str,
    pub trim: f32,
}

const fn t(file: &'static str, trim: f32) -> Take {
    Take { file, trim }
}

/// The recorded takes of `id`; empty when it stays synthesized.
#[must_use]
pub fn takes(id: SoundId) -> &'static [Take] {
    use SoundId::*;
    match id {
        Cannon => &CANNON,
        StubGun => &STUB,
        BigBore => &BIG_BORE,
        TwinBarrel => &TWIN,
        LongGun => &LONG_GUN,
        MachineGun => &MG,
        AutocannonBurst => &AUTOCANNON,
        CoilgunZap => &COIL_ZAP,
        TeslaCrackle => &TESLA,
        RocketLaunch => &ROCKET,
        MortarShot => &MORTAR,
        Flamethrower => &FLAME,
        HitHull => &HIT_HULL,
        HitTurret => &HIT_TURRET,
        HitBarrel => &HIT_BARREL,
        HitMast => &HIT_MAST,
        HitTrack => &HIT_TRACK,
        ArmourThud => &ARMOUR_THUD,
        PartBreak => &PART_BREAK,
        ExplosionSmall => &EXPLOSION_SMALL,
        ExplosionLarge => &EXPLOSION_LARGE,
        TankDestroyed => &TANK_DESTROYED,
        FuelBlast => &FUEL_BLAST,
        SandImpact => &SAND,
        ModuleBlowout => &MODULE_BLOWOUT,
        FlarePop => &FLARE,
        ScrapSteel => &SCRAP_STEEL,
        ScrapPowder => &SCRAP_POWDER,
        ScrapVolts => &SCRAP_VOLTS,
        ScrapFuel => &SCRAP_FUEL,
        UpgradeBolt => &UPGRADE_BOLT,
        Repair => &REPAIR,
        UiTick => &UI_TICK,
        UiConfirm => &UI_CONFIRM,
        UiError => &UI_ERROR,
        RadioSquelchOn => &SQUELCH_ON,
        RadioSquelchOff => &SQUELCH_OFF,
        AirRaidSiren => &SIREN,
        BomberFlyover => &BOMBER,
        Horn0 => &HORN_TRUCK,
        Horn2 => &HORN_BIG,
        Horn4 => &HORN_TOOT,
        Horn5 => &HORN_WHISTLE,
        // Synthesized on purpose: the coilgun's rising whine, the shell and
        // bomb whistles, the ricochet whine, the radio beeps, the War Rig's
        // roar and wave horn, and the two novelty horns (the "La Cucaracha"
        // trumpet and the "aooga" klaxon) have no fitting free recording.
        CoilgunCharge | ShellWhistle | Ricochet | CountdownBeep | CountdownGo | BombWhistle | WaveHorn | BossRoar
        | Horn1 | Horn3 => &[],
    }
}

// Trims: from the processing report (the loudest 400 ms of each file against
// its category's target loudness), clamped to 0.3..1.25.
const CANNON: [Take; 3] = [t("cannon_1", 1.25), t("cannon_2", 1.25), t("cannon_3", 0.99)];
const STUB: [Take; 2] = [t("stub_1", 1.25), t("stub_2", 1.25)];
const BIG_BORE: [Take; 2] = [t("big_bore_1", 1.06), t("big_bore_2", 0.99)];
const TWIN: [Take; 2] = [t("twin_barrel_1", 1.25), t("twin_barrel_2", 1.16)];
const LONG_GUN: [Take; 2] = [t("long_gun_1", 1.04), t("long_gun_2", 1.12)];
const MG: [Take; 4] = [t("machine_gun_1", 1.25), t("machine_gun_2", 1.25), t("machine_gun_3", 1.25), t("machine_gun_4", 0.77)];
const AUTOCANNON: [Take; 2] = [t("autocannon_1", 0.99), t("autocannon_2", 1.11)];
const COIL_ZAP: [Take; 2] = [t("coilgun_zap_1", 1.25), t("coilgun_zap_2", 1.25)];
const TESLA: [Take; 3] = [t("tesla_crackle_1", 1.25), t("tesla_crackle_2", 1.25), t("tesla_crackle_3", 1.25)];
const ROCKET: [Take; 2] = [t("rocket_1", 0.72), t("rocket_2", 0.78)];
const MORTAR: [Take; 2] = [t("mortar_1", 0.90), t("mortar_2", 0.93)];
const FLAME: [Take; 2] = [t("flamethrower_1", 0.93), t("flamethrower_2", 0.69)];
const HIT_HULL: [Take; 3] = [t("hit_hull_1", 0.78), t("hit_hull_2", 0.76), t("hit_hull_3", 0.86)];
const HIT_TURRET: [Take; 3] = [t("hit_turret_1", 0.65), t("hit_turret_2", 1.25), t("hit_turret_3", 1.23)];
const HIT_BARREL: [Take; 2] = [t("hit_barrel_1", 1.19), t("hit_barrel_2", 1.25)];
const HIT_MAST: [Take; 2] = [t("hit_mast_1", 1.25), t("hit_mast_2", 1.25)];
const HIT_TRACK: [Take; 3] = [t("hit_track_1", 1.25), t("hit_track_2", 1.07), t("hit_track_3", 1.25)];
const ARMOUR_THUD: [Take; 4] =
    [t("armour_thud_1", 0.62), t("armour_thud_2", 1.25), t("armour_thud_3", 0.74), t("armour_thud_4", 1.25)];
const PART_BREAK: [Take; 3] = [t("part_break_1", 1.02), t("part_break_2", 0.84), t("part_break_3", 1.25)];
const EXPLOSION_SMALL: [Take; 3] = [t("explosion_small_1", 0.80), t("explosion_small_2", 0.75), t("explosion_small_3", 1.25)];
const EXPLOSION_LARGE: [Take; 3] = [t("explosion_large_1", 1.06), t("explosion_large_2", 1.04), t("explosion_large_3", 0.84)];
const TANK_DESTROYED: [Take; 2] = [t("tank_destroyed_1", 1.06), t("tank_destroyed_2", 0.94)];
const FUEL_BLAST: [Take; 2] = [t("fuel_blast_1", 1.11), t("fuel_blast_2", 1.16)];
const SAND: [Take; 2] = [t("sand_impact_1", 0.97), t("sand_impact_2", 0.99)];
const MODULE_BLOWOUT: [Take; 1] = [t("module_blowout", 1.15)];
const FLARE: [Take; 1] = [t("flare_pop", 1.25)];
const SCRAP_STEEL: [Take; 2] = [t("scrap_steel_1", 0.89), t("scrap_steel_2", 1.06)];
const SCRAP_POWDER: [Take; 1] = [t("scrap_powder_1", 1.25)];
const SCRAP_VOLTS: [Take; 2] = [t("scrap_volts_1", 1.25), t("scrap_volts_2", 1.25)];
const SCRAP_FUEL: [Take; 1] = [t("scrap_fuel_1", 1.22)];
const UPGRADE_BOLT: [Take; 2] = [t("upgrade_bolt_1", 1.15), t("upgrade_bolt_2", 1.01)];
const REPAIR: [Take; 2] = [t("repair_1", 1.23), t("repair_2", 1.06)];
const UI_TICK: [Take; 3] = [t("ui_tick_1", 1.25), t("ui_tick_2", 1.25), t("ui_tick_3", 1.25)];
const UI_CONFIRM: [Take; 1] = [t("ui_confirm", 0.71)];
const UI_ERROR: [Take; 1] = [t("ui_error", 0.71)];
const SQUELCH_ON: [Take; 2] = [t("radio_squelch_on_1", 0.60), t("radio_squelch_on_2", 1.25)];
const SQUELCH_OFF: [Take; 2] = [t("radio_squelch_off_1", 0.72), t("radio_squelch_off_2", 0.80)];
const SIREN: [Take; 1] = [t("air_raid_siren", 0.91)];
const BOMBER: [Take; 1] = [t("bomber_flyover", 0.99)];
const HORN_TRUCK: [Take; 2] = [t("horn_truck_1", 0.85), t("horn_truck_2", 1.17)];
const HORN_BIG: [Take; 1] = [t("horn_big", 1.07)];
const HORN_TOOT: [Take; 1] = [t("horn_toot", 0.77)];
const HORN_WHISTLE: [Take; 1] = [t("horn_whistle", 1.08)];

/// The continuous beds: the engine layers and the weather.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bed {
    /// A diesel at idle (Chieftain, exhaust).
    EngineIdle,
    /// Pulling at medium revs (Sherman M4A3, on board).
    EngineLoad,
    /// Flat out (Stuart M5A1, on board).
    EngineHigh,
    /// Track clatter (Panzer III, at the right track).
    Tracks,
    /// The everyday desert wind (stereo).
    Wind,
    /// The sandstorm (stereo).
    Storm,
}

impl Bed {
    pub const ALL: [Bed; 6] = [Bed::EngineIdle, Bed::EngineLoad, Bed::EngineHigh, Bed::Tracks, Bed::Wind, Bed::Storm];
    /// The four layers of a sampled engine, in [`EngineMix`] order.
    pub const ENGINE: [Bed; 4] = [Bed::EngineIdle, Bed::EngineLoad, Bed::EngineHigh, Bed::Tracks];

    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Bed::EngineIdle => "engine_idle",
            Bed::EngineLoad => "engine_load",
            Bed::EngineHigh => "engine_high",
            Bed::Tracks => "tracks",
            Bed::Wind => "wind",
            Bed::Storm => "storm",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// One file to fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asset {
    /// Take `k` of a one-shot.
    Shot(SoundId, usize),
    Bed(Bed),
}

impl Asset {
    #[must_use]
    pub fn file(self) -> &'static str {
        match self {
            Asset::Shot(id, k) => takes(id)[k].file,
            Asset::Bed(b) => b.file(),
        }
    }

    /// The URL relative to the page.
    #[must_use]
    pub fn url(self) -> String {
        format!("{BASE_URL}{}.ogg", self.file())
    }
}

/// How soon a sound is needed: 0 first.
fn urgency(id: SoundId) -> u8 {
    use SoundId::*;
    match id {
        Cannon | StubGun | MachineGun | HitHull | HitTurret | ArmourThud | ExplosionSmall | SandImpact => 0,
        BigBore | TwinBarrel | LongGun | HitBarrel | HitMast | HitTrack | PartBreak | ExplosionLarge
        | TankDestroyed | UiTick | UiConfirm => 1,
        _ => 2,
    }
}

/// Every file, in the order to fetch it: the first take of the most
/// frequent combat sounds, then the engine layers, then everything else's
/// first take, the weather, and the remaining takes last.
#[must_use]
pub fn load_order() -> Vec<Asset> {
    let firsts = |u: u8| {
        SoundId::ALL.iter().filter(move |id| urgency(**id) == u && !takes(**id).is_empty()).map(|id| Asset::Shot(*id, 0))
    };
    let mut out: Vec<Asset> = firsts(0).collect();
    out.extend(Bed::ENGINE.iter().map(|b| Asset::Bed(*b)));
    out.extend(firsts(1));
    out.extend(firsts(2));
    out.extend([Asset::Bed(Bed::Wind), Asset::Bed(Bed::Storm)]);
    for u in 0..=2 {
        for id in SoundId::ALL.iter().filter(|id| urgency(**id) == u) {
            out.extend((1..takes(*id).len()).map(|k| Asset::Shot(*id, k)));
        }
    }
    out
}

/// The levels and playback rates of the four sampled engine layers
/// ([`Bed::ENGINE`] order: idle, load, high, tracks).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineMix {
    pub gain: [f32; 4],
    pub rate: [f32; 4],
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Crossfades the recorded engine by revs and load, and the track clatter
/// by ground speed. The idle, load and high loops sit at about 0.15, 0.55
/// and 0.9 rpm; each is resampled around its own recorded speed, so the
/// sweep never strays far from a real recording. `variant` (`0..1`, from the
/// tank's seed) detunes one tank from the next.
#[must_use]
pub fn engine_mix(kind: EngineKind, input: EngineInput, variant: f32) -> EngineMix {
    let clean = |v: f32, hi: f32| if v.is_finite() { v.clamp(0.0, hi) } else { 0.0 };
    let rpm = clean(input.rpm, 1.0);
    let load = clean(input.load, 1.0);
    let boost = clean(input.boost, 1.0);
    let speed = clean(input.track_speed, 40.0);
    let size = match kind {
        EngineKind::Scout => 1.14,
        EngineKind::Brawler => 1.0,
        EngineKind::Hauler => 0.86,
    } * (0.96 + 0.08 * clean(variant, 1.0));
    // Where between the three recordings the revs sit; pulling hard leans
    // toward the busier ones.
    let x = (rpm + 0.25 * load + 0.15 * boost).clamp(0.0, 1.2);
    let w_idle = 1.0 - smoothstep(0.15, 0.5, x);
    let w_high = smoothstep(0.55, 0.95, x);
    let w_load = (1.0 - w_idle - w_high).max(0.0);
    // Equal power across the crossfade.
    let norm = (w_idle * w_idle + w_load * w_load + w_high * w_high).sqrt().max(1e-3);
    let level = 0.6 + 0.3 * load + 0.1 * boost;
    let g = |w: f32| w / norm * level;
    let track_level = smoothstep(0.2, 3.0, speed) * (0.45 + 0.55 * (speed / 10.0).min(1.0));
    EngineMix {
        gain: [g(w_idle), g(w_load), g(w_high), track_level],
        rate: [
            size * (0.92 + 0.5 * rpm),
            size * (0.78 + 0.45 * rpm + 0.06 * boost),
            size * (0.72 + 0.32 * rpm + 0.06 * boost),
            0.6 + 0.55 * (speed / 10.0).min(1.3),
        ],
    }
}

/// Every file name (stems), for the licence check and the loader.
#[must_use]
pub fn all_files() -> Vec<&'static str> {
    load_order().into_iter().map(Asset::file).collect()
}
