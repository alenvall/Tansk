//! Layers: the building blocks a sound recipe adds into a buffer. Each adds
//! (never overwrites) starting at a time in seconds, and stops at the end of
//! the buffer.

use crate::dsp::{at, decay_k, drive, one_pole_k, sin01, OnePole, Resonator, Rng, Rotor, Svf, CONTROL, SR};

/// A sine "body" whose pitch glides from `f0` to `f1`: the thump of a
/// blast, the boom of a hull.
#[derive(Clone, Copy, Debug)]
pub struct Thump {
    pub f0: f32,
    pub f1: f32,
    /// Time constant of the pitch glide, seconds.
    pub glide: f32,
    /// Time constant of the amplitude decay, seconds.
    pub decay: f32,
    /// Saturation: adds the harmonics that make it read on small speakers.
    pub drive: f32,
    pub attack: f32,
}

impl Thump {
    #[must_use]
    pub const fn new(f0: f32, f1: f32, glide: f32, decay: f32) -> Self {
        Self { f0, f1, glide, decay, drive: 0.0, attack: 0.0015 }
    }

    #[must_use]
    pub const fn drive(mut self, d: f32) -> Self {
        self.drive = d;
        self
    }

    #[must_use]
    pub const fn attack(mut self, a: f32) -> Self {
        self.attack = a;
        self
    }
}

pub fn thump(b: &mut [f32], t0: f32, th: Thump, amp: f32) {
    let start = at(t0);
    let len = at(th.attack + th.decay * 6.0);
    let gk = decay_k(th.glide.max(1e-3)).powi(CONTROL as i32);
    let dk = decay_k(th.decay);
    let att = at(th.attack).max(1);
    let mut diff = th.f0 - th.f1;
    let mut osc = Rotor::new(th.f0);
    let mut env = 1.0;
    for i in 0..len {
        let Some(out) = b.get_mut(start + i) else { break };
        if i % CONTROL == 0 && i > 0 {
            diff *= gk;
            osc.set(th.f1 + diff);
        }
        let a = if i < att { i as f32 / att as f32 } else { 1.0 };
        let s = drive(osc.tick(), th.drive);
        *out += s * env * a * amp;
        if i >= att {
            env *= dk;
        }
    }
}

/// How a noise burst is filtered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Low,
    Band,
    High,
}

/// A burst of filtered noise with an attack, a hold, an exponential decay,
/// and a cutoff that glides from `cut0` to `cut1`.
#[derive(Clone, Copy, Debug)]
pub struct Noise {
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
    pub cut0: f32,
    pub cut1: f32,
    pub cut_tau: f32,
    pub q: f32,
    pub band: Band,
    /// An extra high-pass before the main filter, Hz; `0` is none.
    pub hp: f32,
    pub drive: f32,
}

impl Noise {
    #[must_use]
    pub const fn new(band: Band, cut0: f32, cut1: f32, cut_tau: f32, decay: f32) -> Self {
        Self {
            attack: 0.0008,
            hold: 0.0,
            decay,
            cut0,
            cut1,
            cut_tau,
            q: 0.7,
            band,
            hp: 0.0,
            drive: 0.0,
        }
    }

    #[must_use]
    pub const fn lp(cut0: f32, cut1: f32, cut_tau: f32, decay: f32) -> Self {
        Self::new(Band::Low, cut0, cut1, cut_tau, decay)
    }

    #[must_use]
    pub const fn bp(centre: f32, q: f32, decay: f32) -> Self {
        let mut n = Self::new(Band::Band, centre, centre, 1.0, decay);
        n.q = q;
        n
    }

    #[must_use]
    pub const fn attack(mut self, a: f32) -> Self {
        self.attack = a;
        self
    }

    #[must_use]
    pub const fn hold(mut self, h: f32) -> Self {
        self.hold = h;
        self
    }

    #[must_use]
    pub const fn q(mut self, q: f32) -> Self {
        self.q = q;
        self
    }

    #[must_use]
    pub const fn hp(mut self, hz: f32) -> Self {
        self.hp = hz;
        self
    }

    #[must_use]
    pub const fn drive(mut self, d: f32) -> Self {
        self.drive = d;
        self
    }
}

pub fn noise(b: &mut [f32], t0: f32, n: Noise, amp: f32, rng: &mut Rng) {
    let start = at(t0);
    let att = at(n.attack).max(1);
    let hold = at(n.hold);
    let len = att + hold + at(n.decay * 5.5);
    let dk = decay_k(n.decay);
    let ck = decay_k(n.cut_tau.max(1e-3)).powi(CONTROL as i32);
    let hk = one_pole_k(n.hp.max(1.0), SR);
    let mut hp = OnePole::default();
    let mut svf = Svf::new(n.cut0, n.q);
    let ratio = n.cut0.max(10.0) / n.cut1.max(10.0);
    let mut glide = 1.0f32;
    let mut env = 1.0;
    for i in 0..len {
        let Some(out) = b.get_mut(start + i) else { break };
        if i % CONTROL == 0 && i > 0 {
            glide *= ck;
            // Exponential glide of the cutoff in log space.
            svf.set(n.cut1.max(10.0) * ratio.powf(glide), n.q);
        }
        let mut x = rng.bi();
        if n.hp > 0.0 {
            x = hp.hp(x, hk);
        }
        let o = svf.tick(x);
        let y = match n.band {
            Band::Low => o.lp,
            Band::Band => o.bp,
            Band::High => o.hp,
        };
        let a = if i < att {
            i as f32 / att as f32
        } else {
            1.0
        };
        *out += drive(y * 1.7, n.drive) * env * a * amp;
        if i >= att + hold {
            env *= dk;
        }
    }
}

/// One mode of a struck object.
#[derive(Clone, Copy, Debug)]
pub struct Mode {
    pub f: f32,
    pub t60: f32,
    pub a: f32,
}

#[must_use]
pub const fn m(f: f32, t60: f32, a: f32) -> Mode {
    Mode { f, t60, a }
}

/// Strikes a set of modes with a contact pulse `contact` seconds long
/// (short is hard and bright, long is soft and dull), plus `grit` of noise in
/// the contact for a scrape of texture.
pub fn strike(b: &mut [f32], t0: f32, modes: &[Mode], contact: f32, grit: f32, amp: f32, rng: &mut Rng) {
    let start = at(t0);
    if start >= b.len() || modes.is_empty() {
        return;
    }
    let mut res: Vec<(Resonator, f32)> = modes
        .iter()
        .filter(|md| md.f < SR * 0.45)
        .map(|md| (Resonator::new(md.f, md.t60, SR), md.a))
        .collect();
    let t60 = modes.iter().fold(0.0f32, |acc, md| acc.max(md.t60));
    let pulse = at(contact).max(2);
    // A half-sine of unit area.
    let norm = std::f32::consts::PI / (2.0 * pulse as f32);
    let len = (pulse + at(t60 * 1.05)).min(b.len() - start);
    for i in 0..len {
        let x = if i < pulse {
            let s = (std::f32::consts::PI * (i as f32 + 0.5) / pulse as f32).sin();
            s * norm * (1.0 + grit * rng.bi() * 3.0)
        } else {
            0.0
        };
        let mut y = 0.0;
        for (r, a) in &mut res {
            y += r.tick(x) * *a;
        }
        b[start + i] += y * amp;
    }
}

/// Inharmonic ratios of a struck plate or box section: the hull "plonk".
pub const PLATE: [f32; 6] = [1.0, 1.59, 2.14, 2.87, 3.61, 4.78];
/// A free bar or tube: the barrel "clang".
pub const BAR: [f32; 4] = [1.0, 2.756, 5.404, 8.933];
/// A small loose piece: a bolt, a link, a shard.
pub const SHARD: [f32; 3] = [1.0, 2.41, 3.93];

/// Modes from a ratio table: base pitch, the base ring time, how much faster
/// each higher mode dies, and how much quieter each is. Ratios and levels are
/// jittered so no two strikes are the same object.
#[must_use]
pub fn modes(base: f32, ratios: &[f32], t60: f32, damp: f32, tilt: f32, rng: &mut Rng) -> Vec<Mode> {
    ratios
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let i = i as f32;
            m(
                base * r * if i > 0.0 { rng.vary(0.03) } else { 1.0 },
                t60 / (1.0 + damp * i) * rng.vary(0.15),
                tilt.powf(i) * rng.vary(0.25),
            )
        })
        .collect()
}

/// Sparse random clicks through a band-pass: fire crackle, arcing, sand.
#[derive(Clone, Copy, Debug)]
pub struct Crackle {
    pub dur: f32,
    /// Events per second at the start and the end.
    pub rate0: f32,
    pub rate1: f32,
    pub centre: f32,
    pub q: f32,
    /// How much a click's size varies; higher is more spiky.
    pub spike: f32,
}

pub fn crackle(b: &mut [f32], t0: f32, c: Crackle, amp: f32, rng: &mut Rng) {
    let start = at(t0);
    let len = at(c.dur);
    let mut svf = Svf::new(c.centre, c.q);
    let mut click = 0.0f32;
    let ck = decay_k(0.0004);
    for i in 0..len + at(0.02) {
        let Some(out) = b.get_mut(start + i) else { break };
        let x = i as f32 / len as f32;
        if i < len {
            let rate = c.rate0 + (c.rate1 - c.rate0) * x;
            if rng.unit() < rate / SR {
                let size = rng.unit().powf(c.spike.max(0.1));
                click = size * if rng.unit() < 0.5 { -1.0 } else { 1.0 };
            }
        }
        let y = svf.tick(click + rng.bi() * click.abs() * 0.5).bp;
        click *= ck;
        let env = (1.0 - x).clamp(0.0, 1.0).sqrt();
        *out += y * env * amp * 3.0;
    }
}

/// A pitched tone: a sum of harmonics with a log pitch glide, an attack and
/// a release.
#[derive(Clone, Copy, Debug)]
pub struct Tone<'a> {
    pub dur: f32,
    pub f0: f32,
    pub f1: f32,
    /// Level of harmonic 1, 2, 3...
    pub harmonics: &'a [f32],
    pub attack: f32,
    pub release: f32,
    /// Amplitude wobble depth and rate (buzz, warble).
    pub wobble: f32,
    pub wobble_hz: f32,
}

impl<'a> Tone<'a> {
    #[must_use]
    pub const fn new(dur: f32, f0: f32, f1: f32, harmonics: &'a [f32]) -> Self {
        Self { dur, f0, f1, harmonics, attack: 0.004, release: 0.03, wobble: 0.0, wobble_hz: 0.0 }
    }

    #[must_use]
    pub const fn env(mut self, attack: f32, release: f32) -> Self {
        self.attack = attack;
        self.release = release;
        self
    }

    #[must_use]
    pub const fn wobble(mut self, depth: f32, hz: f32) -> Self {
        self.wobble = depth;
        self.wobble_hz = hz;
        self
    }
}

pub fn tone(b: &mut [f32], t0: f32, t: Tone<'_>, amp: f32) {
    let start = at(t0);
    let len = at(t.dur).max(1);
    let att = at(t.attack).max(1);
    let rel = at(t.release).max(1).min(len);
    let ratio = (t.f1 / t.f0).max(1e-3);
    let mut phase = 0.0f32;
    let mut wob = 0.0f32;
    let mut f = t.f0;
    for i in 0..len {
        let Some(out) = b.get_mut(start + i) else { break };
        let x = i as f32 / len as f32;
        if i % CONTROL == 0 {
            f = t.f0 * ratio.powf(x);
        }
        phase = (phase + f / SR).fract();
        wob = (wob + t.wobble_hz / SR).fract();
        let mut s = 0.0;
        for (h, lvl) in t.harmonics.iter().enumerate() {
            let hf = f * (h + 1) as f32;
            if hf < SR * 0.45 {
                s += lvl * sin01(phase * (h + 1) as f32);
            }
        }
        let e_a = (i as f32 / att as f32).min(1.0);
        let e_r = ((len - i) as f32 / rel as f32).min(1.0);
        let w = 1.0 - t.wobble * (0.5 + 0.5 * sin01(wob));
        *out += s * e_a * e_r * e_r * w * amp;
    }
}

/// Bits of metal raining down: `count` pieces over `dur`, denser at the
/// start, each bouncing once or twice. `lo..hi` is the base pitch range.
#[allow(clippy::too_many_arguments)]
pub fn clatter(b: &mut [f32], t0: f32, dur: f32, count: usize, lo: f32, hi: f32, amp: f32, rng: &mut Rng) {
    for _ in 0..count {
        let u = rng.unit();
        let t = t0 + dur * u * u;
        let size = rng.range(0.3, 1.0) * (1.0 - 0.6 * u);
        let base = rng.log_range(lo, hi);
        let md = modes(base, &SHARD, rng.range(0.05, 0.18), 1.2, 0.6, rng);
        let contact = rng.range(0.0002, 0.0008);
        strike(b, t, &md, contact, 0.3, amp * size, rng);
        // A bounce or two, closer and smaller.
        let mut gap = rng.range(0.05, 0.12);
        let mut s = size * 0.45;
        let mut tt = t;
        for _ in 0..rng.range(0.0, 2.99) as usize {
            tt += gap;
            strike(b, tt, &md, contact, 0.3, amp * s, rng);
            gap *= 0.6;
            s *= 0.5;
        }
    }
}

/// A ratchet: `clicks` pawl clicks from `t0`, the gap between them easing
/// from `gap0` to `gap1`.
#[allow(clippy::too_many_arguments)]
pub fn ratchet(b: &mut [f32], t0: f32, clicks: usize, gap0: f32, gap1: f32, pitch: f32, amp: f32, rng: &mut Rng) {
    let mut t = t0;
    for i in 0..clicks {
        let x = i as f32 / clicks.max(2).saturating_sub(1) as f32;
        let p = pitch * rng.vary(0.04);
        let md = [m(p, 0.025, 1.0), m(p * 1.73, 0.018, 0.6), m(p * 2.9, 0.012, 0.35), m(p * 0.42, 0.04, 0.5)];
        strike(b, t, &md, 0.00015, 0.4, amp * rng.range(0.75, 1.0), rng);
        t += gap0 + (gap1 - gap0) * x;
    }
}

/// Adds a delayed, dulled copy of the buffer to itself: the slap of a blast
/// off a dune or a wall.
pub fn echo(b: &mut [f32], delay: f32, gain: f32, cutoff: f32) {
    let d = at(delay);
    if d == 0 || d >= b.len() {
        return;
    }
    let src = b.to_vec();
    let k = one_pole_k(cutoff, SR);
    let mut lp = OnePole::default();
    let mut lp2 = OnePole::default();
    for i in d..b.len() {
        b[i] += lp2.lp(lp.lp(src[i - d], k), k) * gain;
    }
}

/// An electric buzz: a mains-ish hum with harsh harmonics, gated by random
/// sputter. `sputter` 0 is steady, 1 is very broken.
pub fn buzz(b: &mut [f32], t0: f32, dur: f32, hz: f32, sputter: f32, amp: f32, rng: &mut Rng) {
    let start = at(t0);
    let len = at(dur);
    let mut phase = 0.0f32;
    let mut gate = 1.0f32;
    let mut target = 1.0f32;
    let mut lp = OnePole::default();
    let k = one_pole_k(3500.0, SR);
    let edge = at(0.004).max(1);
    for i in 0..len {
        let Some(out) = b.get_mut(start + i) else { break };
        if i % 240 == 0 {
            target = if rng.unit() < sputter * 0.6 { rng.range(0.0, 0.3) } else { rng.range(0.7, 1.0) };
        }
        gate += (target - gate) * 0.02;
        phase = (phase + hz * rng.vary(0.02) / SR).fract();
        // A clipped saw: rich, raspy.
        let saw = 2.0 * phase - 1.0;
        let s = drive(saw + 0.3 * rng.bi(), 2.0);
        let e = (i.min(len - i) as f32 / edge as f32).min(1.0);
        *out += lp.lp(s, k) * gate * e * amp;
    }
}
