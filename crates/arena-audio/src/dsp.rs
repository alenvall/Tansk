//! The small DSP kit every sound is built from: a seeded random source,
//! filters, a modal resonator for struck metal, and the finishing pass that
//! makes a buffer safe to ship (no DC, no clicks, a known peak).
//!
//! Per-sample work avoids `exp`/`tan` where it can: envelopes are
//! multiplicative recurrences and filter coefficients are refreshed at a
//! control rate, so a whole sound bank renders in well under a second even in
//! WASM.

use std::f32::consts::{PI, TAU};

/// The rate every one-shot is rendered at.
pub const SAMPLE_RATE: u32 = 48_000;
/// [`SAMPLE_RATE`] as a float.
pub const SR: f32 = SAMPLE_RATE as f32;

/// Samples between filter-coefficient refreshes in a sweep.
pub const CONTROL: usize = 16;

/// splitmix64: tiny, seedable, and good enough for audio jitter.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut r = Self(seed ^ 0x5DEE_CE66_D1CE_4E5B);
        r.next_u64();
        r
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..1`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in `-1..1`.
    pub fn bi(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    /// Uniform in `a..b`.
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }

    /// `1 ± amount`, uniform.
    pub fn vary(&mut self, amount: f32) -> f32 {
        1.0 + amount * self.bi()
    }

    /// Log-uniform in `a..b` (for frequencies).
    pub fn log_range(&mut self, a: f32, b: f32) -> f32 {
        a * (b / a).powf(self.unit())
    }

    /// An independent stream split off this one.
    pub fn fork(&mut self) -> Self {
        Self::new(self.next_u64())
    }
}

/// Seconds to a sample index.
#[must_use]
pub fn at(t: f32) -> usize {
    (t.max(0.0) * SR) as usize
}

/// Per-sample multiplier for an exponential decay with time constant `tau`.
#[must_use]
pub fn decay_k(tau: f32) -> f32 {
    (-1.0 / (tau.max(1e-4) * SR)).exp()
}

/// One-pole low-pass coefficient for `cutoff` Hz.
#[must_use]
pub fn one_pole_k(cutoff: f32, sr: f32) -> f32 {
    let c = if cutoff.is_finite() { cutoff.max(1.0) } else { 1000.0 };
    1.0 - (-TAU * c / sr).exp()
}

/// A one-pole low-pass; `x - lp` is the matching high-pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    pub s: f32,
}

impl OnePole {
    pub fn lp(&mut self, x: f32, k: f32) -> f32 {
        self.s += (x - self.s) * k;
        self.s
    }

    pub fn hp(&mut self, x: f32, k: f32) -> f32 {
        x - self.lp(x, k)
    }
}

/// Simper's trapezoidal state-variable filter: stable under fast sweeps.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

/// The three outputs of [`Svf::tick`]. `bp` is normalised to unity gain at
/// the centre.
#[derive(Clone, Copy, Debug)]
pub struct SvfOut {
    pub lp: f32,
    pub bp: f32,
    pub hp: f32,
}

impl Svf {
    #[must_use]
    pub fn new(cutoff: f32, q: f32) -> Self {
        let mut f = Self::default();
        f.set(cutoff, q);
        f
    }

    pub fn set(&mut self, cutoff: f32, q: f32) {
        self.set_sr(cutoff, q, SR);
    }

    pub fn set_sr(&mut self, cutoff: f32, q: f32, sr: f32) {
        let g = (PI * cutoff.clamp(10.0, sr * 0.45) / sr).tan();
        self.k = 1.0 / q.max(0.05);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    pub fn tick(&mut self, x: f32) -> SvfOut {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut {
            lp: v2,
            bp: self.k * v1,
            hp: x - self.k * v1 - v2,
        }
    }
}

/// A two-pole resonator: one mode of a struck object. Normalised so an
/// impulse of area 1 rings at amplitude 1.
#[derive(Clone, Copy, Debug)]
pub struct Resonator {
    a1: f32,
    a2: f32,
    gain: f32,
    y1: f32,
    y2: f32,
}

impl Resonator {
    /// `t60` is the time to ring down by 60 dB.
    #[must_use]
    pub fn new(freq: f32, t60: f32, sr: f32) -> Self {
        let f = freq.clamp(20.0, sr * 0.45);
        let w = TAU * f / sr;
        let r = (-6.9078 / (t60.max(0.002) * sr)).exp();
        Self {
            a1: 2.0 * r * w.cos(),
            a2: -r * r,
            gain: w.sin(),
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.gain * x + self.a1 * self.y1 + self.a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Bounded in `-1..1`, linear below 0.5.
#[must_use]
pub fn soft_clip(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    let a = x.abs();
    if a < 0.5 {
        x
    } else {
        let over = a.min(8.0) - 0.5;
        x.signum() * (0.5 + 0.5 * (1.0 - (-2.0 * over).exp()))
    }
}

/// A saturator with unity slope at zero and a ceiling of `1 / drive`-ish,
/// renormalised so a full-scale sine stays near full scale.
#[must_use]
pub fn drive(x: f32, amount: f32) -> f32 {
    if amount <= 0.0 {
        return x;
    }
    let d = 1.0 + amount;
    fast_tanh(x * d) / fast_tanh(d)
}

/// A rational tanh: within 2% everywhere, exact at 0 and in the limits, and
/// much cheaper than `f32::tanh` (which matters in WASM).
#[must_use]
pub fn fast_tanh(x: f32) -> f32 {
    if x >= 3.0 {
        1.0
    } else if x <= -3.0 {
        -1.0
    } else {
        let x2 = x * x;
        x * (27.0 + x2) / (27.0 + 9.0 * x2)
    }
}

/// `sin(2π·p)` for a phase `p` in turns, by a polynomial: error under 1e-4.
#[must_use]
pub fn sin01(p: f32) -> f32 {
    // Reduce to -0.5..0.5 turns, then fold to -0.25..0.25.
    let mut x = p - p.floor();
    if x > 0.5 {
        x -= 1.0;
    }
    if x > 0.25 {
        x = 0.5 - x;
    } else if x < -0.25 {
        x = -0.5 - x;
    }
    let t = x * TAU;
    let t2 = t * t;
    t * (1.0 - t2 / 6.0 * (1.0 - t2 / 20.0 * (1.0 - t2 / 42.0 * (1.0 - t2 / 72.0))))
}

/// A sine oscillator by rotation: one complex multiply per sample, its
/// frequency settable at a control rate.
#[derive(Clone, Copy, Debug)]
pub struct Rotor {
    re: f32,
    im: f32,
    c: f32,
    s: f32,
}

impl Rotor {
    /// Starts at phase zero (output 0, rising).
    #[must_use]
    pub fn new(freq: f32) -> Self {
        let mut r = Self { re: 1.0, im: 0.0, c: 1.0, s: 0.0 };
        r.set(freq);
        r
    }

    pub fn set(&mut self, freq: f32) {
        let w = TAU * freq.clamp(0.0, SR * 0.49) / SR;
        self.c = w.cos();
        self.s = w.sin();
        // Hold the magnitude at one against rounding drift.
        let m = (self.re * self.re + self.im * self.im).sqrt();
        if m > 0.0 {
            self.re /= m;
            self.im /= m;
        }
    }

    /// The next sample of the sine.
    pub fn tick(&mut self) -> f32 {
        let re = self.re * self.c - self.im * self.s;
        let im = self.re * self.s + self.im * self.c;
        self.re = re;
        self.im = im;
        im
    }
}

/// Makes a rendered buffer shippable: removes rumble below 20 Hz and any DC,
/// trims the silent tail, fades the edges, and scales to `peak`.
pub fn finish(buf: &mut Vec<f32>, peak: f32) {
    if buf.is_empty() {
        return;
    }
    // Two cascaded one-pole high-passes at 20 Hz.
    let k = one_pole_k(20.0, SR);
    let (mut h1, mut h2) = (OnePole::default(), OnePole::default());
    for s in buf.iter_mut() {
        let x = if s.is_finite() { *s } else { 0.0 };
        *s = h2.hp(h1.hp(x, k), k);
    }
    // Trim the tail below -66 dB of the peak, keeping a short ring-out.
    let top = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if top <= 1e-9 {
        return;
    }
    let floor = top * 5e-4;
    let last = buf.iter().rposition(|s| s.abs() > floor).unwrap_or(0);
    let keep = (last + at(0.02)).min(buf.len());
    buf.truncate(keep.max(at(0.02)).min(buf.len()));
    // No DC.
    let mean = buf.iter().sum::<f32>() / buf.len() as f32;
    for s in buf.iter_mut() {
        *s -= mean;
    }
    // Edges: a 0.3 ms fade-in keeps transients, a 12 ms fade-out.
    let fade_in = at(0.0003).min(buf.len());
    for (i, s) in buf.iter_mut().take(fade_in).enumerate() {
        *s *= i as f32 / fade_in as f32;
    }
    let n = buf.len();
    let fade_out = at(0.012).min(n);
    for i in 0..fade_out {
        let g = i as f32 / fade_out as f32;
        buf[n - 1 - i] *= g * g;
    }
    let top = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if top > 1e-9 {
        let g = peak.min(0.98) / top;
        for s in buf.iter_mut() {
            *s *= g;
        }
    }
}

/// Scales every channel together so the loudest sample is `peak`, after
/// [`finish`]-style cleaning of each channel. Channels are padded to one
/// length.
pub fn finish_channels(chans: &mut [Vec<f32>], peak: f32) {
    for c in chans.iter_mut() {
        finish(c, 1.0);
    }
    let n = chans.iter().map(Vec::len).max().unwrap_or(0);
    let mut top = 0.0f32;
    for c in chans.iter_mut() {
        c.resize(n, 0.0);
        top = c.iter().fold(top, |m, s| m.max(s.abs()));
    }
    if top > 1e-9 {
        let g = peak.min(0.98) / top;
        for c in chans.iter_mut() {
            for s in c.iter_mut() {
                *s *= g;
            }
        }
    }
}
