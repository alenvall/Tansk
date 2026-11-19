//! Every one-shot in the game, as a recipe of layers. `render(id, seed)`
//! gives a different take for every seed: pitch, ring times, the order of the
//! debris and the grain of the noise all move, so a sound played twice is
//! never the same sound twice.
//!
//! The tone is the desert junkyard's (docs/DESIGN.md, pillar 3): plonks and
//! clangs of welded plate, dull thuds on armour, crunches, booms with a slap
//! of echo off the dunes, and electricity that sounds like a shorted battery,
//! not a laser.

use crate::dsp::{finish, finish_channels, Rng, SR};
use crate::horns::{aooga, foghorn, melody, squeeze, train, truck};
use crate::raid::{air_raid_siren, bomb_whistle, bomber_flyover, boss_roar, flare_pop, module_blowout, wave_horn};
use crate::layers::{
    buzz, clatter, crackle, echo, m, modes, noise, ratchet, strike, thump, tone, Crackle, Noise,
    Thump, Tone, BAR, PLATE, SHARD,
};

macro_rules! sound_ids {
    ($($id:ident => $name:literal),* $(,)?) => {
        /// Every one-shot sound. `name()` is the stable string id (also the
        /// preview WAV's file name).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum SoundId { $($id),* }

        impl SoundId {
            /// Every sound, in id order.
            pub const ALL: &'static [SoundId] = &[$(SoundId::$id),*];

            #[must_use]
            pub const fn name(self) -> &'static str {
                match self { $(SoundId::$id => $name),* }
            }

            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name { $($name => Some(SoundId::$id),)* _ => None }
            }
        }
    };
}

sound_ids! {
    // Weapons.
    Cannon => "cannon",
    BigBore => "big_bore",
    TwinBarrel => "twin_barrel",
    LongGun => "long_gun",
    MachineGun => "machine_gun",
    AutocannonBurst => "autocannon_burst",
    CoilgunCharge => "coilgun_charge",
    CoilgunZap => "coilgun_zap",
    TeslaCrackle => "tesla_crackle",
    ShellWhistle => "shell_whistle",
    // Impacts, by what was hit.
    HitHull => "hit_hull",
    HitTurret => "hit_turret",
    HitBarrel => "hit_barrel",
    HitMast => "hit_mast",
    HitTrack => "hit_track",
    ArmourThud => "armour_thud",
    PartBreak => "part_break",
    Ricochet => "ricochet",
    // Explosions and the ground.
    ExplosionSmall => "explosion_small",
    ExplosionLarge => "explosion_large",
    TankDestroyed => "tank_destroyed",
    SandImpact => "sand_impact",
    // Pickups and the crew's work.
    ScrapSteel => "scrap_steel",
    ScrapPowder => "scrap_powder",
    ScrapVolts => "scrap_volts",
    ScrapFuel => "scrap_fuel",
    UpgradeBolt => "upgrade_bolt",
    Repair => "repair",
    // Interface and radio.
    UiTick => "ui_tick",
    UiConfirm => "ui_confirm",
    UiError => "ui_error",
    CountdownBeep => "countdown_beep",
    CountdownGo => "countdown_go",
    RadioSquelchOn => "radio_squelch_on",
    RadioSquelchOff => "radio_squelch_off",
    // The air strike and the War Rig.
    AirRaidSiren => "air_raid_siren",
    BomberFlyover => "bomber_flyover",
    BombWhistle => "bomb_whistle",
    FlarePop => "flare_pop",
    WaveHorn => "wave_horn",
    BossRoar => "boss_roar",
    ModuleBlowout => "module_blowout",
    // The Garage horns; `SoundId::horn(i)` picks one.
    Horn0 => "horn_0",
    Horn1 => "horn_1",
    Horn2 => "horn_2",
    Horn3 => "horn_3",
    Horn4 => "horn_4",
    Horn5 => "horn_5",
}

/// How a sound is rendered and played.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundInfo {
    /// The longest a take may be before the tail is trimmed, seconds.
    pub seconds: f32,
    pub stereo: bool,
    /// The peak a take is normalised to. The relative loudness of the bank.
    pub peak: f32,
    /// Most copies playing at once; the oldest is cut when one more starts.
    pub voices: u8,
    /// Distinct takes pre-rendered for playback.
    pub variants: u8,
    /// Playback-rate jitter on top of the take's own, `±` share.
    pub pitch_jitter: f32,
    /// Whether it sits in the world (panned, attenuated) by default.
    pub positional: bool,
}

const fn info(seconds: f32, stereo: bool, peak: f32, voices: u8, variants: u8, pitch_jitter: f32, positional: bool) -> SoundInfo {
    SoundInfo { seconds, stereo, peak, voices, variants, pitch_jitter, positional }
}

impl SoundId {
    /// The Garage horns, in order: truck air horn, "La Cucaracha" trumpet,
    /// foghorn, klaxon "aooga", squeeze horn, train whistle.
    pub const HORNS: [SoundId; 6] =
        [SoundId::Horn0, SoundId::Horn1, SoundId::Horn2, SoundId::Horn3, SoundId::Horn4, SoundId::Horn5];

    /// Garage horn `index` (wraps, so any stored choice plays something).
    #[must_use]
    pub const fn horn(index: usize) -> SoundId {
        Self::HORNS[index % Self::HORNS.len()]
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn info(self) -> SoundInfo {
        use SoundId::*;
        match self {
            Cannon => info(1.8, true, 0.95, 6, 3, 0.03, true),
            BigBore => info(2.6, true, 0.97, 4, 2, 0.03, true),
            TwinBarrel => info(1.8, true, 0.95, 4, 3, 0.03, true),
            LongGun => info(2.0, true, 0.95, 4, 3, 0.03, true),
            MachineGun => info(0.45, false, 0.8, 12, 4, 0.05, true),
            AutocannonBurst => info(1.2, false, 0.9, 4, 3, 0.03, true),
            CoilgunCharge => info(1.0, false, 0.6, 4, 2, 0.01, true),
            CoilgunZap => info(1.0, false, 0.92, 4, 3, 0.03, true),
            TeslaCrackle => info(0.8, false, 0.8, 6, 3, 0.05, true),
            ShellWhistle => info(1.6, false, 0.55, 6, 3, 0.06, true),
            HitHull => info(0.9, false, 0.85, 8, 4, 0.06, true),
            HitTurret => info(1.0, false, 0.85, 8, 4, 0.06, true),
            HitBarrel => info(1.4, false, 0.8, 6, 3, 0.05, true),
            HitMast => info(0.9, false, 0.75, 6, 3, 0.06, true),
            HitTrack => info(0.7, false, 0.85, 6, 3, 0.06, true),
            ArmourThud => info(0.6, false, 0.9, 8, 4, 0.06, true),
            PartBreak => info(1.8, false, 0.95, 4, 3, 0.04, true),
            Ricochet => info(1.0, false, 0.7, 6, 4, 0.08, true),
            ExplosionSmall => info(1.6, true, 0.95, 6, 3, 0.05, true),
            ExplosionLarge => info(3.2, true, 0.97, 4, 2, 0.04, true),
            TankDestroyed => info(4.5, true, 0.97, 3, 2, 0.03, true),
            SandImpact => info(1.0, false, 0.8, 8, 3, 0.06, true),
            ScrapSteel => info(0.6, false, 0.6, 4, 3, 0.04, true),
            ScrapPowder => info(0.6, false, 0.6, 4, 3, 0.04, true),
            ScrapVolts => info(0.6, false, 0.55, 4, 3, 0.04, true),
            ScrapFuel => info(0.7, false, 0.6, 4, 3, 0.04, true),
            UpgradeBolt => info(1.8, false, 0.85, 3, 2, 0.02, true),
            Repair => info(2.0, false, 0.75, 3, 2, 0.02, true),
            UiTick => info(0.08, false, 0.35, 4, 3, 0.03, false),
            UiConfirm => info(0.4, false, 0.45, 2, 1, 0.0, false),
            UiError => info(0.4, false, 0.45, 2, 1, 0.0, false),
            CountdownBeep => info(0.3, false, 0.5, 2, 1, 0.0, false),
            CountdownGo => info(0.7, false, 0.55, 2, 1, 0.0, false),
            RadioSquelchOn => info(0.3, false, 0.45, 2, 3, 0.03, false),
            RadioSquelchOff => info(0.35, false, 0.45, 2, 3, 0.03, false),
            AirRaidSiren => info(4.3, false, 0.7, 1, 2, 0.02, false),
            BomberFlyover => info(5.3, false, 0.8, 2, 2, 0.03, true),
            BombWhistle => info(2.0, false, 0.6, 6, 3, 0.05, true),
            FlarePop => info(1.4, false, 0.75, 4, 3, 0.05, true),
            WaveHorn => info(3.0, false, 0.8, 1, 1, 0.0, false),
            BossRoar => info(3.8, false, 0.95, 1, 2, 0.02, true),
            ModuleBlowout => info(2.8, true, 0.97, 3, 2, 0.03, true),
            Horn0 => info(1.6, false, 0.75, 2, 2, 0.01, true),
            Horn1 => info(1.6, false, 0.75, 2, 2, 0.0, true),
            Horn2 => info(2.0, false, 0.8, 2, 2, 0.01, true),
            Horn3 => info(1.3, false, 0.75, 2, 2, 0.01, true),
            Horn4 => info(1.0, false, 0.7, 2, 2, 0.02, true),
            Horn5 => info(2.0, false, 0.7, 2, 2, 0.01, true),
        }
    }
}

/// A rendered take: one channel (mono) or two (left, right), at
/// [`crate::SAMPLE_RATE`].
#[derive(Clone, Debug)]
pub struct Rendered {
    pub channels: Vec<Vec<f32>>,
}

impl Rendered {
    #[must_use]
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }

    #[must_use]
    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / SR
    }
}

type Recipe = fn(&mut [f32], &mut Rng, &mut Rng);

/// Renders take `seed` of `id`.
#[must_use]
pub fn render(id: SoundId, seed: u64) -> Rendered {
    let inf = id.info();
    let recipe = recipe(id);
    let key = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((id.index() as u64 + 1).wrapping_mul(0xD1B5_4A32_D192_ED03));
    let len = (inf.seconds * SR) as usize;
    let chans = if inf.stereo { 2 } else { 1 };
    let mut channels: Vec<Vec<f32>> = (0..chans)
        .map(|c| {
            let mut b = vec![0.0f32; len];
            // The same parameters on both sides; the noise and the debris
            // differ, which is the width.
            let mut p = Rng::new(key);
            let mut n = Rng::new(key ^ (0xA5A5_0000_0000_0000 | (c as u64 * 0x1234_5678_9ABC)));
            recipe(&mut b, &mut p, &mut n);
            b
        })
        .collect();
    if chans == 1 {
        finish(&mut channels[0], inf.peak);
    } else {
        // Keep some of the middle: the two sides share their parameters, so
        // a blend keeps the boom centred and the debris wide.
        let (l, r) = (channels[0].clone(), channels[1].clone());
        for (i, (a, b)) in l.iter().zip(&r).enumerate() {
            channels[0][i] = 0.75 * a + 0.25 * b;
            channels[1][i] = 0.75 * b + 0.25 * a;
        }
        finish_channels(&mut channels, inf.peak);
    }
    Rendered { channels }
}

fn recipe(id: SoundId) -> Recipe {
    use SoundId::*;
    match id {
        Cannon => cannon,
        BigBore => big_bore,
        TwinBarrel => twin_barrel,
        LongGun => long_gun,
        MachineGun => machine_gun,
        AutocannonBurst => autocannon_burst,
        CoilgunCharge => coilgun_charge,
        CoilgunZap => coilgun_zap,
        TeslaCrackle => tesla_crackle,
        ShellWhistle => shell_whistle,
        HitHull => hit_hull,
        HitTurret => hit_turret,
        HitBarrel => hit_barrel,
        HitMast => hit_mast,
        HitTrack => hit_track,
        ArmourThud => armour_thud,
        PartBreak => part_break,
        Ricochet => ricochet,
        ExplosionSmall => explosion_small,
        ExplosionLarge => explosion_large,
        TankDestroyed => tank_destroyed,
        SandImpact => sand_impact,
        ScrapSteel => scrap_steel,
        ScrapPowder => scrap_powder,
        ScrapVolts => scrap_volts,
        ScrapFuel => scrap_fuel,
        UpgradeBolt => upgrade_bolt,
        Repair => repair,
        UiTick => ui_tick,
        UiConfirm => ui_confirm,
        UiError => ui_error,
        CountdownBeep => countdown_beep,
        CountdownGo => countdown_go,
        RadioSquelchOn => radio_squelch_on,
        RadioSquelchOff => radio_squelch_off,
        AirRaidSiren => air_raid_siren,
        BomberFlyover => bomber_flyover,
        BombWhistle => bomb_whistle,
        FlarePop => flare_pop,
        WaveHorn => wave_horn,
        BossRoar => boss_roar,
        ModuleBlowout => module_blowout,
        Horn0 => truck,
        Horn1 => melody,
        Horn2 => foghorn,
        Horn3 => aooga,
        Horn4 => squeeze,
        Horn5 => train,
    }
}

// ── Guns ─────────────────────────────────────────────────────────────────

/// What every gun shares, scaled: the muzzle blast (a bright crack that
/// dulls fast), the body thump, a rumble tail, the breech clanking back, and
/// the slap off the dunes.
struct Gun {
    /// Overall pitch, 1 is the main cannon.
    pitch: f32,
    /// Overall length, 1 is the main cannon.
    size: f32,
    thump: f32,
    blast: f32,
    tail: f32,
    clank: f32,
    echo_delay: f32,
    echo: f32,
}

fn gun(b: &mut [f32], t0: f32, g: &Gun, p: &mut Rng, n: &mut Rng) {
    let pitch = g.pitch * p.vary(0.05);
    let size = g.size * p.vary(0.06);
    // The crack: wide open for a few milliseconds, closing fast.
    noise(
        b,
        t0,
        Noise::lp(9000.0, 700.0 * pitch, 0.03 * size, 0.045 * size).attack(0.0004).drive(1.5),
        g.blast,
        n,
    );
    // The fireball's roar under it.
    noise(b, t0 + 0.002, Noise::lp(2500.0 * pitch, 250.0 * pitch, 0.12 * size, 0.12 * size), g.blast * 0.6, n);
    thump(
        b,
        t0,
        Thump::new(150.0 * pitch, 52.0 * pitch, 0.04 * size, 0.15 * size).drive(1.8),
        g.thump * 0.8,
    );
    // The punch a small speaker can make: a bark in the low mids.
    noise(b, t0, Noise::bp(420.0 * pitch, 0.9, 0.05 * size).attack(0.0006).drive(1.5), g.blast * 0.7, n);
    // Rumble tail.
    noise(
        b,
        t0 + 0.01,
        Noise::lp(600.0 * pitch, 120.0, 0.3 * size, 0.38 * size).attack(0.03),
        g.tail,
        n,
    );
    // Breech and recoil: a heavy clank a beat after the shot.
    if g.clank > 0.0 {
        let md = modes(300.0 * pitch * p.vary(0.08), &PLATE, 0.25, 0.7, 0.7, p);
        strike(b, t0 + 0.09 * size * p.vary(0.15), &md, 0.0012, 0.5, g.clank, n);
    }
    if g.echo > 0.0 {
        echo(b, g.echo_delay * p.vary(0.15), g.echo, 900.0);
        echo(b, g.echo_delay * 1.9 * p.vary(0.15), g.echo * 0.45, 600.0);
    }
}

fn cannon(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    gun(
        b,
        0.0,
        &Gun { pitch: 1.0, size: 1.0, thump: 1.0, blast: 0.9, tail: 0.5, clank: 0.25, echo_delay: 0.24, echo: 0.22 },
        p,
        n,
    );
}

fn big_bore(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    gun(
        b,
        0.0,
        &Gun { pitch: 0.68, size: 1.6, thump: 1.3, blast: 0.85, tail: 0.75, clank: 0.35, echo_delay: 0.3, echo: 0.28 },
        p,
        n,
    );
    // The huge shell's chamber "whoomp".
    thump(b, 0.004, Thump::new(60.0, 28.0, 0.1, 0.32).drive(0.8), 0.6);
}

fn twin_barrel(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let g = Gun { pitch: 1.18, size: 0.8, thump: 0.8, blast: 0.95, tail: 0.4, clank: 0.0, echo_delay: 0.22, echo: 0.0 };
    gun(b, 0.0, &g, p, n);
    gun(b, 0.055 * p.vary(0.2), &Gun { pitch: 1.1, ..g }, p, n);
    // A wide shotgun bark in the mids.
    noise(b, 0.0, Noise::bp(900.0, 0.8, 0.05).drive(1.0), 0.5, n);
    let md = modes(340.0 * p.vary(0.05), &PLATE, 0.22, 0.8, 0.7, p);
    strike(b, 0.12, &md, 0.0012, 0.5, 0.25, n);
    echo(b, 0.22 * p.vary(0.1), 0.24, 900.0);
}

fn long_gun(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // The supersonic crack first: a very short bright snap.
    noise(b, 0.0, Noise::new(crate::layers::Band::High, 2500.0, 2500.0, 1.0, 0.004).attack(0.0002).drive(2.0), 1.0, n);
    gun(
        b,
        0.001,
        &Gun { pitch: 1.35, size: 0.85, thump: 0.6, blast: 0.85, tail: 0.35, clank: 0.15, echo_delay: 0.38, echo: 0.3 },
        p,
        n,
    );
    // The long barrel rings.
    let md = modes(980.0 * p.vary(0.04), &BAR, 0.5, 1.0, 0.5, p);
    strike(b, 0.002, &md, 0.0004, 0.1, 0.12, n);
}

fn machine_gun(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let pitch = p.vary(0.07);
    noise(b, 0.0, Noise::lp(7000.0, 900.0 * pitch, 0.008, 0.012).attack(0.0003).drive(1.5), 0.9, n);
    thump(b, 0.0, Thump::new(220.0 * pitch, 95.0 * pitch, 0.01, 0.03).drive(1.5), 0.7);
    noise(b, 0.003, Noise::lp(1800.0, 300.0, 0.05, 0.06), 0.25, n);
    // The bolt cycling.
    let md = [m(1900.0 * pitch, 0.03, 1.0), m(3100.0 * pitch, 0.02, 0.5)];
    strike(b, 0.035 * p.vary(0.1), &md, 0.0002, 0.3, 0.12, n);
    echo(b, 0.11 * p.vary(0.15), 0.15, 1200.0);
}

fn autocannon_burst(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let rounds = 4;
    let mut t = 0.0;
    for i in 0..rounds {
        let pitch = p.vary(0.04) * (1.0 - 0.02 * i as f32);
        noise(b, t, Noise::lp(8000.0, 700.0 * pitch, 0.012, 0.022).attack(0.0003).drive(1.5), 0.9, n);
        thump(b, t, Thump::new(170.0 * pitch, 60.0 * pitch, 0.015, 0.06).drive(1.4), 0.9);
        noise(b, t + 0.004, Noise::lp(1500.0, 200.0, 0.1, 0.12), 0.25, n);
        // A spent casing pinging off the hull somewhere after.
        let md = modes(2600.0 * p.vary(0.1), &SHARD, 0.12, 1.0, 0.5, n);
        strike(b, t + p.range(0.18, 0.4), &md, 0.0002, 0.1, 0.06, n);
        t += 0.105 * p.vary(0.06);
    }
    echo(b, 0.2 * p.vary(0.1), 0.2, 800.0);
}

fn coilgun_charge(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A junk capacitor bank winding up: a transformer hum that climbs into a
    // whine, with the bank's buzz getting angrier.
    let f0 = 90.0 * p.vary(0.05);
    let dur = 0.85;
    tone(b, 0.0, Tone::new(dur, f0, f0 * 9.0, &[1.0, 0.5, 0.3, 0.15]).env(0.25, 0.01).wobble(0.3, 30.0), 0.45);
    tone(b, 0.1, Tone::new(dur - 0.1, f0 * 6.0, f0 * 22.0, &[1.0, 0.2]).env(0.6, 0.01), 0.12);
    buzz(b, 0.0, dur, f0 * 0.66, 0.4, 0.15, n);
    crackle(b, 0.3, Crackle { dur: 0.55, rate0: 5.0, rate1: 60.0, centre: 3000.0, q: 1.2, spike: 2.0 }, 0.3, n);
}

fn coilgun_zap(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // The bank dumping: a bang of current, the slug's ka-thunk, a falling
    // ring off the rails, and the arc crackling out.
    noise(b, 0.0, Noise::lp(12000.0, 1500.0, 0.01, 0.018).attack(0.0002).hp(800.0).drive(2.0), 1.0, n);
    thump(b, 0.0, Thump::new(110.0, 40.0, 0.03, 0.12).drive(1.5), 0.9);
    tone(b, 0.0, Tone::new(0.22, 2100.0 * p.vary(0.05), 180.0, &[1.0, 0.4, 0.2]).env(0.001, 0.1), 0.35);
    let md = modes(780.0 * p.vary(0.05), &BAR, 0.6, 1.2, 0.6, p);
    strike(b, 0.003, &md, 0.0005, 0.2, 0.3, n);
    crackle(b, 0.01, Crackle { dur: 0.3, rate0: 400.0, rate1: 20.0, centre: 3500.0, q: 0.9, spike: 1.5 }, 0.5, n);
    buzz(b, 0.0, 0.12, 120.0, 0.5, 0.25, n);
    echo(b, 0.25 * p.vary(0.1), 0.18, 1000.0);
}

fn tesla_crackle(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Arcing between tanks: a sputtering hum, dense snaps, a few fat arcs.
    let dur = 0.6 * p.vary(0.1);
    buzz(b, 0.0, dur, 100.0 * p.vary(0.04), 0.7, 0.35, n);
    crackle(b, 0.0, Crackle { dur, rate0: 700.0, rate1: 150.0, centre: 3200.0, q: 0.8, spike: 2.5 }, 0.7, n);
    let arcs = 3 + (p.unit() * 3.0) as usize;
    for _ in 0..arcs {
        let t = p.range(0.0, dur - 0.05);
        noise(b, t, Noise::bp(n.log_range(1500.0, 4500.0), 1.5, 0.02).hold(0.015).drive(1.5), 0.6, n);
    }
}

fn shell_whistle(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A shell going over: a breathy whistle falling as it passes, with the
    // air tearing under it. Swells, then falls away.
    let f0 = 1500.0 * p.vary(0.1);
    let f1 = f0 * 0.5;
    let dur = 1.4;
    let len = (dur * SR) as usize;
    let mut svf = crate::dsp::Svf::new(f0, 6.0);
    let mut phase = 0.0f32;
    for i in 0..len.min(b.len()) {
        let x = i as f32 / len as f32;
        // Doppler: holds high, drops through the pass, settles low.
        let pass = 1.0 / (1.0 + (-(x - 0.55) * 12.0).exp());
        let f = f0 + (f1 - f0) * pass;
        if i % 16 == 0 {
            svf.set(f, 6.0);
        }
        phase = (phase + f / SR).fract();
        let swell = (x * std::f32::consts::PI).sin().powf(1.5) * (1.0 - 0.6 * x);
        let air = svf.tick(n.bi()).bp;
        let whistle = crate::dsp::sin01(phase);
        b[i] += (whistle * 0.35 + air * 1.2) * swell;
    }
    noise(b, 0.3, Noise::lp(900.0, 400.0, 0.5, 0.35).attack(0.4), 0.25, n);
}

// ── Impacts ─────────────────────────────────────────────────────────────

/// A shell or round on welded plate: the ringing body, a dull blow under it,
/// and the snap of contact.
struct Hit {
    base: f32,
    ratios: &'static [f32],
    t60: f32,
    damp: f32,
    tilt: f32,
    contact: f32,
    ring: f32,
    blow_hz: f32,
    blow: f32,
    snap: f32,
}

fn hit(b: &mut [f32], h: &Hit, p: &mut Rng, n: &mut Rng) {
    let base = h.base * p.vary(0.1);
    let md = modes(base, h.ratios, h.t60 * p.vary(0.15), h.damp, h.tilt, p);
    strike(b, 0.0, &md, h.contact * p.vary(0.2), 0.6, h.ring, n);
    if h.blow > 0.0 {
        let hz = h.blow_hz * p.vary(0.15);
        thump(b, 0.0, Thump::new(hz * 1.6, hz, 0.01, 0.05 * p.vary(0.3)), h.blow);
    }
    noise(b, 0.0, Noise::lp(8000.0, 2000.0, 0.004, 0.006).attack(0.0002), h.snap, n);
    // Sparks and flecks of paint: a few tiny ticks just after.
    crackle(b, 0.005, Crackle { dur: 0.08, rate0: 200.0, rate1: 0.0, centre: 5000.0, q: 1.0, spike: 2.0 }, h.snap * 0.3, n);
}

fn hit_hull(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // The plonk: a big hollow box of plate.
    hit(b, &Hit { base: 190.0, ratios: &PLATE, t60: 0.55, damp: 0.6, tilt: 0.75, contact: 0.0015, ring: 1.0, blow_hz: 90.0, blow: 0.5, snap: 0.25 }, p, n);
}

fn hit_turret(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Thicker cast steel, smaller: a higher, longer clang.
    hit(b, &Hit { base: 330.0, ratios: &PLATE, t60: 0.8, damp: 0.5, tilt: 0.7, contact: 0.0009, ring: 1.0, blow_hz: 120.0, blow: 0.35, snap: 0.3 }, p, n);
}

fn hit_barrel(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A tube: the bell-like "tang".
    hit(b, &Hit { base: 620.0, ratios: &BAR, t60: 1.1, damp: 0.9, tilt: 0.55, contact: 0.0004, ring: 0.9, blow_hz: 0.0, blow: 0.0, snap: 0.35 }, p, n);
}

fn hit_mast(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A thin pole and its rattling bits: a "tink" and a wobble.
    hit(b, &Hit { base: 1150.0, ratios: &BAR, t60: 0.6, damp: 1.0, tilt: 0.5, contact: 0.0003, ring: 0.8, blow_hz: 0.0, blow: 0.0, snap: 0.3 }, p, n);
    clatter(b, 0.03, 0.25, 3, 1800.0, 3200.0, 0.2, n);
}

fn hit_track(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A clunk into the running gear and the links chattering.
    hit(b, &Hit { base: 150.0, ratios: &SHARD, t60: 0.2, damp: 1.0, tilt: 0.6, contact: 0.002, ring: 1.0, blow_hz: 80.0, blow: 0.6, snap: 0.25 }, p, n);
    clatter(b, 0.01, 0.2, 6, 600.0, 1400.0, 0.35, n);
}

fn armour_thud(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Bolted-on slab soaking it up: a dull, heavily damped "dunk".
    let pitch = p.vary(0.15);
    thump(b, 0.0, Thump::new(150.0 * pitch, 75.0 * pitch, 0.015, 0.07 * p.vary(0.25)).drive(1.2), 1.0);
    noise(b, 0.0, Noise::lp(2500.0, 400.0, 0.01, 0.03).attack(0.0005), 0.7, n);
    let md = modes(160.0 * pitch, &PLATE, 0.09, 1.0, 0.6, p);
    strike(b, 0.0, &md, 0.003, 0.5, 0.4, n);
}

fn part_break(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Welds tearing: a gritty crunch, a snap, then the part clattering away
    // in slowing bounces.
    noise(b, 0.0, Noise::lp(5000.0, 800.0, 0.06, 0.07).hold(0.04).drive(3.0), 0.8, n);
    for _ in 0..10 {
        let t = p.range(0.0, 0.14);
        noise(b, t, Noise::bp(n.log_range(500.0, 3500.0), 1.2, 0.008).drive(2.0), 0.6, n);
    }
    thump(b, 0.0, Thump::new(140.0, 60.0, 0.02, 0.08).drive(1.0), 0.6);
    // The snap of the last weld.
    let md = modes(1400.0 * p.vary(0.1), &SHARD, 0.08, 1.0, 0.6, p);
    strike(b, 0.12 * p.vary(0.2), &md, 0.0002, 0.4, 0.4, n);
    // The part itself bouncing: a heavy body, gaps shrinking.
    let body = modes(260.0 * p.vary(0.15), &PLATE, 0.3, 0.8, 0.7, p);
    let mut t = 0.3 * p.vary(0.15);
    let mut gap = 0.28 * p.vary(0.15);
    let mut a = 0.7;
    for _ in 0..5 {
        strike(b, t, &body, 0.001, 0.5, a, n);
        t += gap;
        gap *= 0.62;
        a *= 0.6;
    }
    // Bolts and bits.
    clatter(b, 0.15, 1.0, 7, 900.0, 3000.0, 0.25, n);
}

fn ricochet(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // The "pyeww": a glancing ping and the round whining away.
    let md = modes(1900.0 * p.vary(0.12), &BAR, 0.3, 1.0, 0.5, p);
    strike(b, 0.0, &md, 0.0003, 0.3, 0.6, n);
    noise(b, 0.0, Noise::lp(9000.0, 3000.0, 0.003, 0.004), 0.3, n);
    let f0 = 3400.0 * p.vary(0.1);
    tone(b, 0.004, Tone::new(0.45 * p.vary(0.15), f0, f0 * 0.55, &[1.0, 0.15]).env(0.01, 0.25).wobble(0.25, 37.0), 0.45);
    noise(b, 0.004, Noise::bp(f0 * 0.6, 3.0, 0.15).attack(0.01), 0.25, n);
}

// ── Explosions and the ground ───────────────────────────────────────────

pub(crate) fn boom(b: &mut [f32], t0: f32, size: f32, p: &mut Rng, n: &mut Rng) {
    let pitch = p.vary(0.06) / size.sqrt();
    thump(b, t0, Thump::new(105.0 * pitch, 38.0 * pitch, 0.06 * size, 0.22 * size).drive(2.0), 0.85);
    noise(b, t0, Noise::bp(480.0 * pitch, 0.8, 0.07 * size).attack(0.0008).drive(2.0), 0.6, n);
    noise(b, t0, Noise::lp(6000.0, 300.0 * pitch, 0.04 * size, 0.08 * size).attack(0.0005).drive(2.5), 0.9, n);
    noise(b, t0 + 0.01, Noise::lp(1200.0 * pitch, 140.0, 0.4 * size, 0.35 * size).attack(0.02), 0.6, n);
    // A low roll that hangs.
    noise(b, t0 + 0.05, Noise::lp(220.0, 90.0, 0.8 * size, 0.6 * size).attack(0.12 * size), 0.5, n);
    crackle(b, t0 + 0.02, Crackle { dur: 0.5 * size, rate0: 90.0, rate1: 5.0, centre: 2200.0, q: 0.7, spike: 2.5 }, 0.35, n);
}

fn explosion_small(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    boom(b, 0.0, 0.7, p, n);
    clatter(b, 0.08, 0.6, 4, 900.0, 2600.0, 0.18, n);
    echo(b, 0.22 * p.vary(0.15), 0.18, 700.0);
}

fn explosion_large(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    boom(b, 0.0, 1.5, p, n);
    thump(b, 0.0, Thump::new(55.0, 26.0, 0.15, 0.4).drive(1.0), 0.7);
    clatter(b, 0.15, 1.3, 8, 500.0, 2400.0, 0.2, n);
    echo(b, 0.3 * p.vary(0.15), 0.25, 600.0);
    echo(b, 0.62 * p.vary(0.15), 0.12, 400.0);
}

fn tank_destroyed(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // The hull lets go, the ammo cooks off, the turret comes down, and the
    // rest of it rains out of the sky.
    boom(b, 0.0, 1.7, p, n);
    thump(b, 0.0, Thump::new(50.0, 24.0, 0.2, 0.5).drive(1.0), 0.8);
    // The hull shell itself, ringing as it splits.
    let hull = modes(120.0 * p.vary(0.1), &PLATE, 0.9, 0.5, 0.75, p);
    strike(b, 0.01, &hull, 0.004, 0.8, 0.35, n);
    // Ammo cook-off: one or two secondary pops.
    let pops = 1 + (p.unit() * 2.0) as usize;
    for i in 0..pops {
        boom(b, 0.3 + 0.25 * i as f32 * p.vary(0.3), 0.45, p, n);
    }
    // The turret landing.
    let turret = modes(170.0 * p.vary(0.1), &PLATE, 0.7, 0.6, 0.7, p);
    let land = 1.0 * p.vary(0.15);
    strike(b, land, &turret, 0.004, 0.6, 0.9, n);
    thump(b, land, Thump::new(110.0, 55.0, 0.02, 0.09), 0.7);
    strike(b, land + 0.22, &turret, 0.003, 0.6, 0.35, n);
    // Debris rain.
    clatter(b, 0.45, 2.8, 26, 500.0, 3200.0, 0.22, n);
    clatter(b, 0.6, 2.0, 6, 220.0, 500.0, 0.3, n);
    echo(b, 0.32 * p.vary(0.15), 0.25, 500.0);
    echo(b, 0.7 * p.vary(0.15), 0.12, 350.0);
}

fn sand_impact(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A shell into a dune: a soft whump and sand pattering back down.
    let pitch = p.vary(0.15);
    thump(b, 0.0, Thump::new(100.0 * pitch, 48.0 * pitch, 0.03, 0.1 * p.vary(0.25)).drive(1.2).attack(0.003), 0.9);
    noise(b, 0.0, Noise::lp(1600.0, 350.0, 0.08, 0.1).attack(0.003), 0.8, n);
    noise(b, 0.04, Noise::lp(3500.0, 1500.0, 0.3, 0.2).attack(0.05).hp(400.0), 0.15, n);
    crackle(b, 0.06, Crackle { dur: 0.6, rate0: 900.0, rate1: 30.0, centre: 2800.0, q: 0.5, spike: 3.0 }, 0.22, n);
}

// ── Pickups and crew work ───────────────────────────────────────────────

fn scrap_steel(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Two grey plates clinking together as they are slung on.
    let base = 560.0 * p.vary(0.05);
    let a = modes(base, &PLATE, 0.3, 0.8, 0.6, p);
    let c = modes(base * 1.34, &PLATE, 0.25, 0.8, 0.6, p);
    strike(b, 0.0, &a, 0.0005, 0.3, 0.8, n);
    strike(b, 0.07 * p.vary(0.15), &c, 0.0004, 0.3, 0.6, n);
}

fn scrap_powder(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A shell crate: a wooden knock and brass shell cases rattling inside.
    let base = 210.0 * p.vary(0.06);
    let wood = [m(base, 0.07, 1.0), m(base * 2.3, 0.05, 0.5), m(base * 3.9, 0.03, 0.3)];
    strike(b, 0.0, &wood, 0.002, 0.8, 1.0, n);
    noise(b, 0.0, Noise::lp(2000.0, 600.0, 0.01, 0.015), 0.3, n);
    for i in 0..3 {
        let md = modes(2300.0 * p.vary(0.12), &SHARD, 0.12, 1.0, 0.5, p);
        strike(b, 0.04 + 0.05 * i as f32 * p.vary(0.3), &md, 0.0002, 0.2, 0.25, n);
    }
}

fn scrap_volts(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A battery and a coil: a hard plastic-and-copper clack, a "zzt" as the
    // terminals touch, and the coil's springy ring.
    let base = 820.0 * p.vary(0.05);
    let md = [m(base, 0.08, 1.0), m(base * 2.1, 0.05, 0.5)];
    strike(b, 0.0, &md, 0.0006, 0.4, 0.8, n);
    buzz(b, 0.02, 0.07, 140.0 * p.vary(0.1), 0.5, 0.25, n);
    crackle(b, 0.02, Crackle { dur: 0.07, rate0: 300.0, rate1: 100.0, centre: 3500.0, q: 1.0, spike: 2.0 }, 0.25, n);
    let coil = modes(1450.0 * p.vary(0.05), &BAR, 0.35, 1.0, 0.4, p);
    strike(b, 0.1, &coil, 0.0003, 0.1, 0.35, n);
}

fn scrap_fuel(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A jerrycan: a hollow "bonk" and the fuel sloshing.
    let base = 230.0 * p.vary(0.06);
    let md = [m(base, 0.25, 1.0), m(base * 1.52, 0.18, 0.6), m(base * 2.47, 0.1, 0.35), m(base * 3.6, 0.06, 0.2)];
    strike(b, 0.0, &md, 0.0015, 0.4, 1.0, n);
    // Slosh: low noise with a slow lumpy envelope.
    let start = (0.03 * SR) as usize;
    let len = (0.35 * SR) as usize;
    let mut svf = crate::dsp::Svf::new(700.0, 1.5);
    let rate = 9.0 * p.vary(0.2);
    for i in 0..len {
        let Some(out) = b.get_mut(start + i) else { break };
        let x = i as f32 / len as f32;
        let lump = (0.5 + 0.5 * crate::dsp::sin01(x * rate)).powi(3);
        *out += svf.tick(n.bi()).bp * lump * (1.0 - x) * 0.6;
    }
}

fn weld(b: &mut [f32], t0: f32, dur: f32, n: &mut Rng) {
    // An arc welder: hiss, spatter, and the transformer's buzz.
    noise(b, t0, Noise::bp(2600.0, 0.8, 0.08).hold(dur).attack(0.01), 0.3, n);
    crackle(b, t0, Crackle { dur, rate0: 400.0, rate1: 300.0, centre: 2200.0, q: 0.9, spike: 2.0 }, 0.5, n);
    buzz(b, t0, dur, 100.0, 0.35, 0.18, n);
}

fn upgrade_bolt(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Ratchet it down, the part clanks home, a weld zap, a last settle.
    ratchet(b, 0.0, 9, 0.065, 0.035, 1500.0 * p.vary(0.05), 0.45, n);
    let md = modes(260.0 * p.vary(0.06), &PLATE, 0.6, 0.6, 0.75, p);
    strike(b, 0.48, &md, 0.0012, 0.5, 1.0, n);
    thump(b, 0.48, Thump::new(140.0, 70.0, 0.02, 0.08).drive(0.8), 0.6);
    weld(b, 0.72, 0.35, n);
    let small = modes(700.0 * p.vary(0.06), &SHARD, 0.2, 1.0, 0.6, p);
    strike(b, 1.2, &small, 0.0005, 0.3, 0.3, n);
}

fn repair(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Two strokes of a wrench, a hiss of the welder, a tap to say done.
    ratchet(b, 0.0, 5, 0.05, 0.045, 1350.0 * p.vary(0.05), 0.45, n);
    ratchet(b, 0.42, 5, 0.05, 0.045, 1350.0 * p.vary(0.05), 0.45, n);
    weld(b, 0.85, 0.6, n);
    let tap = modes(900.0 * p.vary(0.05), &SHARD, 0.18, 1.0, 0.6, p);
    strike(b, 1.6, &tap, 0.0004, 0.2, 0.5, n);
}

// ── Interface and radio ─────────────────────────────────────────────────

fn ui_tick(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A toggle switch on a dashboard.
    let f = 2200.0 * p.vary(0.03);
    strike(b, 0.0, &[m(f, 0.02, 1.0), m(f * 2.3, 0.012, 0.4), m(f * 0.5, 0.03, 0.4)], 0.0002, 0.2, 1.0, n);
}

fn ui_confirm(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A heavy switch thrown, and a small bell answering.
    strike(b, 0.0, &[m(900.0, 0.03, 1.0), m(1900.0, 0.02, 0.5), m(300.0, 0.05, 0.6)], 0.0004, 0.3, 0.8, n);
    let bell = [m(1320.0, 0.25, 1.0), m(1320.0 * 2.0, 0.15, 0.3), m(1320.0 * 2.76, 0.1, 0.25)];
    strike(b, 0.06, &bell, 0.0003, 0.0, 0.6, p);
}

fn ui_error(b: &mut [f32], _p: &mut Rng, n: &mut Rng) {
    // Two flat buzzes of a dead relay.
    for i in 0..2 {
        let t = i as f32 * 0.14;
        tone(b, t, Tone::new(0.1, 150.0, 140.0, &[1.0, 0.0, 0.4, 0.0, 0.2, 0.0, 0.1]).env(0.003, 0.02), 0.6);
        strike(b, t, &[m(600.0, 0.02, 1.0)], 0.0005, 0.2, 0.3, n);
    }
}

fn radio_band(b: &mut [f32]) {
    // A field radio's narrow voice band.
    let mut svf = crate::dsp::Svf::new(1500.0, 0.6);
    let mut hp = crate::dsp::OnePole::default();
    let k = crate::dsp::one_pole_k(350.0, SR);
    for s in b.iter_mut() {
        *s = crate::dsp::drive(svf.tick(hp.hp(*s, k)).bp * 1.4, 1.0);
    }
}

fn countdown_beep(b: &mut [f32], _p: &mut Rng, n: &mut Rng) {
    tone(b, 0.0, Tone::new(0.16, 880.0, 880.0, &[1.0, 0.0, 0.25, 0.0, 0.1]).env(0.004, 0.03), 0.7);
    noise(b, 0.0, Noise::lp(4000.0, 4000.0, 1.0, 0.06).hold(0.12), 0.04, n);
    radio_band(b);
}

fn countdown_go(b: &mut [f32], _p: &mut Rng, n: &mut Rng) {
    tone(b, 0.0, Tone::new(0.5, 1320.0, 1320.0, &[1.0, 0.0, 0.25, 0.0, 0.1]).env(0.004, 0.12), 0.7);
    tone(b, 0.0, Tone::new(0.5, 660.0, 660.0, &[1.0, 0.0, 0.2]).env(0.004, 0.12), 0.35);
    noise(b, 0.0, Noise::lp(4000.0, 4000.0, 1.0, 0.1).hold(0.45), 0.04, n);
    radio_band(b);
}

fn radio_squelch_on(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Key down: a click, a burst of static, a blip of carrier.
    strike(b, 0.0, &[m(1800.0, 0.01, 1.0), m(3600.0, 0.008, 0.4)], 0.0002, 0.2, 0.5, n);
    noise(b, 0.004, Noise::bp(2000.0, 0.5, 0.05).hold(0.06 * p.vary(0.2)), 0.9, n);
    tone(b, 0.01, Tone::new(0.05, 1000.0 * p.vary(0.03), 1000.0, &[1.0]).env(0.003, 0.02), 0.25);
    radio_band(b);
}

fn radio_squelch_off(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // Key up: the "kssht" tail, cut by the click.
    let dur = 0.16 * p.vary(0.2);
    noise(b, 0.0, Noise::bp(2200.0, 0.5, 0.05).hold(dur).attack(0.01), 0.8, n);
    crackle(b, 0.0, Crackle { dur, rate0: 300.0, rate1: 600.0, centre: 2500.0, q: 0.8, spike: 2.0 }, 0.4, n);
    strike(b, dur + 0.01, &[m(1700.0, 0.012, 1.0), m(3300.0, 0.008, 0.4)], 0.0002, 0.2, 0.5, n);
    radio_band(b);
}
