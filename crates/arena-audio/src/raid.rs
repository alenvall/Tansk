//! The air strike and the War Rig: siren, bombers, bombs, flares, the wave
//! horn, the boss's engine and its modules tearing off.

use crate::dsp::{drive, fast_tanh, one_pole_k, sin01, OnePole, Rng, Svf, SR};
use crate::engine::{EngineInput, EngineKind, TankEngine};
use crate::layers::{
    clatter, crackle, echo, modes, noise, strike, thump, tone, Band, Crackle, Noise, Thump, Tone, Wavetable,
    PLATE,
};

/// Smoothstep-joined lookup in `(time, value)` keyframes: a crank or a
/// throttle eases between settings rather than snapping.
fn keyed(keys: &[(f32, f32)], t: f32) -> f32 {
    let i = keys.iter().rposition(|k| k.0 <= t).unwrap_or(0);
    let a = keys[i];
    let Some(b) = keys.get(i + 1) else { return a.1 };
    let x = ((t - a.0) / (b.0 - a.0).max(1e-6)).clamp(0.0, 1.0);
    let x = x * x * (3.0 - 2.0 * x);
    a.1 + (b.1 - a.1) * x
}

pub(crate) fn air_raid_siren(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A hand-cranked siren: a rotor chopping air, wound up twice and let run
    // down, heard through a blown scrapyard PA horn.
    let top = 640.0 * p.vary(0.05);
    let keys = [
        (0.0, 110.0),
        (1.3, top),
        (1.7, top * 0.97),
        (2.15, top * 0.62),
        (2.9, top * 1.04),
        (3.2, top),
        (4.2, 120.0),
    ];
    let len = ((4.2 * SR) as usize).min(b.len());
    let mut phase = 0.0f32;
    let mut air = Svf::new(top, 4.0);
    let mut f = 110.0;
    for (i, out) in b.iter_mut().enumerate().take(len) {
        let t = i as f32 / SR;
        if i % 16 == 0 {
            // The crank is not perfectly steady.
            f = keyed(&keys, t) * (1.0 + 0.006 * sin01(t * 5.3));
            air.set(f, 4.0);
        }
        phase = (phase + f / SR).fract();
        // Ports opening and closing: a rounded pulse wave.
        let chop = fast_tanh(sin01(phase) * 3.0) + 0.35 * sin01(phase * 2.0);
        let breath = air.tick(n.bi()).bp * 1.5;
        let level = ((f - 100.0) / (top - 100.0)).clamp(0.0, 1.0).sqrt();
        let fade = ((len - i) as f32 / (0.15 * SR)).min(1.0);
        *out += (chop * 0.5 + breath) * level * fade;
    }
    // The PA: driven hard, squeezed into a horn's band.
    for s in b.iter_mut() {
        *s = drive(*s, 2.5);
    }
    pa_band(b);
    echo(b, 0.42 * p.vary(0.1), 0.3, 1500.0);
}

/// A cheap horn loudspeaker: no lows, no highs, a honky middle.
fn pa_band(b: &mut [f32]) {
    let mut svf = Svf::new(1300.0, 0.55);
    let mut hp = OnePole::default();
    let k = one_pole_k(280.0, SR);
    for s in b.iter_mut() {
        *s = drive(svf.tick(hp.hp(*s, k)).bp * 1.5, 1.0);
    }
}

pub(crate) fn bomber_flyover(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A twin-engine prop bomber passing over: two detuned radial drones
    // beating, Doppler from high to low, swelling overhead and dulling as it
    // leaves.
    let len = ((5.2 * SR) as usize).min(b.len());
    let base = 72.0 * p.vary(0.05);
    let centre = 2.6 * p.vary(0.05);
    let engines = [base, base * 1.013];
    let mut phase = [0.0f32; 2];
    let (mut lp, mut lp2, mut rumble) = (OnePole::default(), OnePole::default(), OnePole::default());
    let rk = one_pole_k(180.0, SR);
    let (mut k, mut dop, mut near, mut edge) = (0.1, 1.0, 0.0, 0.0);
    let drone = Wavetable::new(&[1.0, 0.7, 0.55, 0.4, 0.3, 0.22, 0.16, 0.12]);
    for (i, out) in b.iter_mut().enumerate().take(len) {
        let t = i as f32 / SR;
        if i % 32 == 0 {
            let x = (t - centre) / 0.9;
            // Doppler: +7% coming, -7% going.
            dop = 1.0 + 0.07 * (1.0 - 2.0 / (1.0 + (-x * 2.0).exp()));
            near = 1.0 / (1.0 + x * x);
            let leaving = if x > 0.0 { 0.7 } else { 1.0 };
            edge = (t / 0.3).min(1.0) * ((5.2 - t) / 0.3).clamp(0.0, 1.0);
            k = one_pole_k(350.0 + 2400.0 * near * leaving, SR);
        }
        let mut s = 0.0;
        for (e, f) in engines.iter().enumerate() {
            phase[e] = (phase[e] + f * dop / SR).fract();
            // Cylinder firings and prop blades: a buzzy harmonic stack,
            // pulsed at half the drone's rate.
            s += drone.at(phase[e]) * (0.75 + 0.25 * sin01(phase[e] * 0.5));
        }
        let air = rumble.lp(n.bi(), rk) * 6.0;
        let y = lp2.lp(lp.lp(s * 0.4 + air, k), k);
        *out += y * (0.08 + near) * edge;
    }
}

pub(crate) fn bomb_whistle(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A bomb falling: a whistle sliding down and getting louder all the way
    // to the ground (the blast is its own sound).
    let dur = 1.8 * p.vary(0.08);
    let f0 = 2300.0 * p.vary(0.08);
    let f1 = f0 * 0.32;
    let len = ((dur * SR) as usize).min(b.len());
    let mut svf = Svf::new(f0, 8.0);
    let mut phase = 0.0f32;
    let mut f = f0;
    for (i, out) in b.iter_mut().enumerate().take(len) {
        let x = i as f32 / len as f32;
        if i % 16 == 0 {
            f = f0 * (f1 / f0).powf(x.powf(1.4));
            svf.set(f, 8.0);
        }
        phase = (phase + f / SR).fract();
        let swell = (0.08 + 0.92 * x * x) * (x / 0.05).min(1.0);
        // The fins wobble it.
        let wob = 1.0 + 0.15 * sin01(x * dur * 13.0);
        *out += (sin01(phase) * 0.5 + svf.tick(n.bi()).bp * 1.2) * swell * wob;
    }
}

pub(crate) fn flare_pop(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A flare gun: a hollow pop, then the flare fizzing and sputtering.
    let pitch = p.vary(0.1);
    thump(b, 0.0, Thump::new(320.0 * pitch, 140.0 * pitch, 0.01, 0.03).drive(1.0), 0.8);
    noise(b, 0.0, Noise::lp(6000.0, 1200.0, 0.01, 0.02).attack(0.0003).drive(1.5), 0.8, n);
    let start = (0.03 * SR) as usize;
    let len = (1.1 * p.vary(0.1) * SR) as usize;
    let mut svf = Svf::new(2600.0, 0.9);
    let (mut gate, mut target) = (1.0f32, 1.0f32);
    for i in 0..len {
        let Some(out) = b.get_mut(start + i) else { break };
        if i % 360 == 0 {
            target = n.range(0.35, 1.0);
        }
        gate += (target - gate) * 0.01;
        let x = i as f32 / len as f32;
        let env = (i as f32 / (0.05 * SR)).min(1.0) * (1.0 - x).powi(2);
        *out += svf.tick(n.bi()).bp * gate * env * 0.5;
    }
    crackle(b, 0.03, Crackle { dur: 0.9, rate0: 120.0, rate1: 20.0, centre: 2000.0, q: 0.8, spike: 2.0 }, 0.35, n);
}

pub(crate) fn wave_horn(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A big low horn, a ship's or a refinery's: two reeds a hair apart and a
    // fifth above, sagging into pitch as the air comes up.
    let f = 82.0 * p.vary(0.02);
    let reed: &[f32] = &[1.0, 0.85, 0.7, 0.55, 0.45, 0.35, 0.27, 0.2, 0.15, 0.11, 0.08];
    tone(b, 0.0, Tone::new(2.3, f * 0.94, f, reed).env(0.18, 0.45), 0.5);
    tone(b, 0.01, Tone::new(2.3, f * 1.006 * 0.94, f * 1.006, reed).env(0.2, 0.45), 0.45);
    tone(b, 0.0, Tone::new(2.3, f * 1.5 * 0.95, f * 1.5, &[1.0, 0.5, 0.3]).env(0.25, 0.45), 0.18);
    noise(b, 0.0, Noise::bp(400.0, 1.2, 0.3).hold(1.9).attack(0.15), 0.08, n);
    for s in b.iter_mut() {
        *s = drive(*s, 1.2);
    }
    echo(b, 0.38 * p.vary(0.1), 0.3, 700.0);
    echo(b, 0.8 * p.vary(0.1), 0.14, 500.0);
}

pub(crate) fn boss_roar(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // The War Rig waking up: the real engine voice (the V12), cranked from a
    // cough to a flat-out roar with the nitro open, settling to a menacing
    // high idle; exhaust barks and a sub-bass shove under it.
    let mut e = TankEngine::new(EngineKind::Hauler, p.next_u64(), SR);
    let rpm = [(0.0, 0.0), (0.35, 0.05), (0.6, 0.9), (1.6, 1.0), (2.3, 0.7), (3.4, 0.25)];
    let load = [(0.0, 0.2), (0.35, 1.0), (1.6, 1.0), (2.0, 0.3), (3.4, 0.0)];
    let boost = [(0.0, 0.0), (0.5, 0.0), (0.8, 1.0), (1.7, 1.0), (2.2, 0.0)];
    let len = ((3.5 * SR) as usize).min(b.len());
    let mut i = 0;
    while i < len {
        let t = i as f32 / SR;
        e.set_input(EngineInput { rpm: keyed(&rpm, t), load: keyed(&load, t), boost: keyed(&boost, t), track_speed: 0.0 });
        let end = (i + 128).min(len);
        e.render(&mut b[i..end]);
        i = end;
    }
    // Weight under it: the size of the thing.
    let mut sub = OnePole::default();
    let k = one_pole_k(120.0, SR);
    for s in b.iter_mut().take(len) {
        let low = sub.lp(*s, k);
        *s = *s * 1.4 + low * 2.0;
    }
    thump(b, 0.33, Thump::new(70.0, 34.0, 0.08, 0.3).drive(1.0), 0.5);
    // Barks out of the stacks as it catches.
    for j in 0..4 {
        let t = 0.36 + 0.09 * j as f32 * p.vary(0.3);
        noise(b, t, Noise::lp(2200.0, 300.0, 0.03, 0.05).drive(2.0), 0.45, n);
    }
    let fade = (0.12 * SR) as usize;
    for j in 0..fade.min(len) {
        b[len - 1 - j] *= j as f32 / fade as f32;
    }
    echo(b, 0.35 * p.vary(0.1), 0.2, 500.0);
}

pub(crate) fn module_blowout(b: &mut [f32], p: &mut Rng, n: &mut Rng) {
    // A boss module torn off: plate shearing in a fast run of strikes
    // dropping in pitch, a shriek of steel on steel, then it goes up.
    let rip = 0.32 * p.vary(0.15);
    let grains = 36;
    for g in 0..grains {
        let x = g as f32 / grains as f32;
        let t = (rip * x + 0.004 * n.bi()).max(0.0);
        let base = 900.0 * 0.33f32.powf(x) * n.vary(0.08);
        let md = modes(base, &PLATE, 0.12, 1.0, 0.7, n);
        strike(b, t, &md, 0.0006, 1.0, 0.35 * (0.5 + x), n);
    }
    let mut shriek = Noise::new(Band::Band, 2400.0, 500.0, rip * 0.6, rip * 0.5).hold(rip * 0.5).drive(2.5);
    shriek.q = 3.0;
    noise(b, 0.0, shriek, 0.6, n);
    thump(b, rip * 0.4, Thump::new(160.0, 70.0, 0.03, 0.1).drive(1.2), 0.6);
    // Then it blows.
    crate::sounds::boom(b, rip, 1.2, p, n);
    let hull = modes(150.0 * p.vary(0.1), &PLATE, 0.8, 0.6, 0.75, p);
    strike(b, rip + 0.01, &hull, 0.003, 0.6, 0.35, n);
    clatter(b, rip + 0.1, 1.8, 14, 400.0, 2800.0, 0.22, n);
    echo(b, 0.3 * p.vary(0.15), 0.22, 600.0);
}
