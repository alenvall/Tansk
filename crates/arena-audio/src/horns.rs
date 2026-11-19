//! The Garage horns (docs/DESIGN.md, "Customisation"): six junkyard horns,
//! each a blown or squeezed thing with a real body (reed, bell, pipe) rather
//! than an oscillator beep. `SoundId::horn(index)` picks one.

use crate::dsp::{drive, one_pole_k, sin01, OnePole, Rng, Svf, CONTROL, SR};
use crate::layers::{echo, Wavetable};

/// One blown note: a harmonic stack at a pitch that follows `pitch(t)`
/// (seconds from the note's start), a breath of noise around the
/// fundamental, an attack and release, and `rasp` of saturation.
struct Blow<'a> {
    dur: f32,
    pitch: &'a dyn Fn(f32) -> f32,
    table: &'a Wavetable,
    attack: f32,
    release: f32,
    breath: f32,
    rasp: f32,
    /// Vibrato depth (share of pitch) and rate.
    vibrato: (f32, f32),
}

fn blow(b: &mut [f32], t0: f32, v: &Blow<'_>, amp: f32, n: &mut Rng) {
    let start = (t0 * SR) as usize;
    let len = (v.dur * SR) as usize;
    let att = (v.attack * SR).max(1.0);
    let rel = (v.release * SR).max(1.0);
    let mut phase = 0.0f32;
    let mut f = (v.pitch)(0.0);
    let mut breath = Svf::new(f, 3.0);
    let mut vib = n.unit();
    for i in 0..len {
        let Some(out) = b.get_mut(start + i) else { break };
        let t = i as f32 / SR;
        if i % CONTROL == 0 {
            f = (v.pitch)(t) * (1.0 + v.vibrato.0 * sin01(vib));
            breath.set(f * 2.0, 3.0);
        }
        vib = (vib + v.vibrato.1 / SR).fract();
        phase = (phase + f / SR).fract();
        let e = (i as f32 / att).min(1.0) * ((len - i) as f32 / rel).min(1.0);
        let s = v.table.at(phase) + breath.tick(n.bi()).bp * v.breath;
        *out += drive(s * e, v.rasp) * amp;
    }
}

/// A resonant body over the whole buffer: a horn bell, a squeeze bulb.
fn body(b: &mut [f32], centre: f32, q: f32, mix: f32) {
    let mut svf = Svf::new(centre, q);
    for s in b.iter_mut() {
        let y = svf.tick(*s).bp;
        *s = *s * (1.0 - mix) + y * mix * 2.0;
    }
}

/// Brass-ish: strong, slowly falling harmonics.
const BRASS: [f32; 10] = [1.0, 0.9, 0.75, 0.6, 0.5, 0.4, 0.3, 0.22, 0.15, 0.1];
/// A reed or a diaphragm: odd-heavy and buzzy.
const REED: [f32; 9] = [1.0, 0.35, 0.8, 0.3, 0.6, 0.25, 0.45, 0.2, 0.3];

/// Horn 0: a truck's dual-tone air horn. Two trumpets a minor third
/// apart, sagging as the tank's air runs down.
pub(crate) fn truck(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let t = Wavetable::new(&BRASS);
    let lo = 290.0 * p.vary(0.02);
    let dur = 1.0;
    for (k, f) in [lo, lo * 1.19].into_iter().enumerate() {
        let pitch = move |t: f32| f * (1.0 - 0.02 * t);
        blow(
            b,
            0.0,
            &Blow { dur, pitch: &pitch, table: &t, attack: 0.03, release: 0.12, breath: 0.08, rasp: 1.5, vibrato: (0.002, 5.0 + k as f32) },
            0.45,
            n,
        );
    }
    body(b, 900.0, 0.8, 0.4);
    echo(b, 0.3 * p.vary(0.1), 0.18, 1500.0);
}

/// Horn 1: the opening of "La Cucaracha" (a traditional Mexican folk song,
/// public domain) on a dented trumpet: C C C F A, the last one held.
pub(crate) fn melody(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let t = Wavetable::new(&BRASS);
    let c = 392.0 * p.vary(0.01); // Played in G: sits better on a small horn.
    let notes = [(1.0, 0.11), (1.0, 0.11), (1.0, 0.11), (4.0 / 3.0, 0.2), (5.0 / 3.0, 0.42)];
    let mut at = 0.0;
    for (ratio, len) in notes {
        let f = c * ratio;
        // Each note scoops up into pitch, as a sloppy player's does.
        let pitch = move |t: f32| f * (1.0 - 0.03 * (-t / 0.02).exp());
        blow(
            b,
            at,
            &Blow { dur: len, pitch: &pitch, table: &t, attack: 0.012, release: 0.03, breath: 0.15, rasp: 2.0, vibrato: (0.006, 6.0) },
            0.55,
            n,
        );
        at += len + 0.025;
    }
    body(b, 1300.0, 1.0, 0.45);
    echo(b, 0.25 * p.vary(0.1), 0.15, 1500.0);
}

/// Horn 2: a ship's foghorn off a wreck in the Sink: low, huge, slow.
pub(crate) fn foghorn(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let t = Wavetable::new(&BRASS);
    let f = 98.0 * p.vary(0.02);
    let pitch = move |t: f32| f * (0.94 + 0.06 * (1.0 - (-t / 0.12).exp()));
    blow(
        b,
        0.0,
        &Blow { dur: 1.35, pitch: &pitch, table: &t, attack: 0.12, release: 0.3, breath: 0.12, rasp: 1.2, vibrato: (0.0, 1.0) },
        0.6,
        n,
    );
    let fifth = move |t: f32| f * 1.5 * (0.95 + 0.05 * (1.0 - (-t / 0.15).exp()));
    let t2 = Wavetable::new(&BRASS[..4]);
    blow(
        b,
        0.02,
        &Blow { dur: 1.3, pitch: &fifth, table: &t2, attack: 0.15, release: 0.3, breath: 0.0, rasp: 0.5, vibrato: (0.0, 1.0) },
        0.15,
        n,
    );
    body(b, 400.0, 0.9, 0.35);
    echo(b, 0.45 * p.vary(0.1), 0.3, 600.0);
}

/// Horn 3: an old klaxon, "aa-OO-ga": a diaphragm buzz whose pitch is
/// cranked up, then drops.
pub(crate) fn aooga(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let t = Wavetable::new(&REED);
    let f = 210.0 * p.vary(0.03);
    let pitch = move |t: f32| {
        if t < 0.5 {
            let x = t / 0.5;
            f * (1.0 + 1.0 * x * x * (3.0 - 2.0 * x))
        } else {
            f * (2.0 - 0.6 * ((t - 0.5) / 0.15).min(1.0))
        }
    };
    blow(
        b,
        0.0,
        &Blow { dur: 0.68, pitch: &pitch, table: &t, attack: 0.03, release: 0.08, breath: 0.1, rasp: 2.5, vibrato: (0.004, 28.0) },
        0.5,
        n,
    );
    // The bell's "aa → oo": a formant that closes as it goes up.
    let mut svf = Svf::new(1100.0, 1.5);
    let len = (0.75 * SR) as usize;
    for (i, s) in b.iter_mut().enumerate().take(len) {
        if i % CONTROL == 0 {
            let t = i as f32 / SR;
            svf.set(1100.0 - 500.0 * (t / 0.5).min(1.0) + 400.0 * ((t - 0.5) / 0.15).clamp(0.0, 1.0), 1.5);
        }
        *s = *s * 0.4 + svf.tick(*s).bp * 1.4;
    }
    echo(b, 0.28 * p.vary(0.1), 0.15, 1500.0);
}

/// Horn 4: a rubber-bulb squeeze horn bolted to the hull: two nasal honks,
/// the reed choking and the bulb wheezing. Scrappy, not a cartoon.
pub(crate) fn squeeze(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let t = Wavetable::new(&REED);
    let f = 370.0 * p.vary(0.04);
    for (k, at) in [0.0f32, 0.3].into_iter().enumerate() {
        let lift = if k == 0 { 1.0 } else { 0.94 };
        let pitch = move |t: f32| f * lift * (0.92 + 0.08 * (t / 0.05).min(1.0)) * (1.0 - 0.05 * (t / 0.22).min(1.0));
        blow(
            b,
            at,
            &Blow { dur: 0.22, pitch: &pitch, table: &t, attack: 0.015, release: 0.06, breath: 0.4, rasp: 3.0, vibrato: (0.012, 17.0) },
            0.45,
            n,
        );
    }
    body(b, 1500.0, 2.0, 0.55);
    // A rasp of air as the bulb refills.
    let start = (0.55 * SR) as usize;
    let mut lp = OnePole::default();
    let k = one_pole_k(2500.0, SR);
    for i in 0..(0.12 * SR) as usize {
        let Some(s) = b.get_mut(start + i) else { break };
        let x = i as f32 / (0.12 * SR);
        *s += lp.lp(n.bi(), k) * (x * (1.0 - x)) * 0.25;
    }
}

/// Horn 5: a steam train whistle: three pipes in a chord, breathy, with the
/// steam catching at the start.
pub(crate) fn train(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    let t = Wavetable::new(&[1.0, 0.25, 0.12, 0.05]);
    let root = 440.0 * p.vary(0.02);
    for (k, ratio) in [1.0f32, 1.26, 1.5].into_iter().enumerate() {
        let f = root * ratio;
        let pitch = move |t: f32| f * (0.96 + 0.04 * (1.0 - (-t / 0.08).exp()));
        blow(
            b,
            0.01 * k as f32,
            &Blow { dur: 1.3, pitch: &pitch, table: &t, attack: 0.08, release: 0.3, breath: 0.9, rasp: 0.6, vibrato: (0.003, 4.5 + k as f32) },
            0.3,
            n,
        );
    }
    echo(b, 0.4 * p.vary(0.1), 0.25, 1200.0);
}
