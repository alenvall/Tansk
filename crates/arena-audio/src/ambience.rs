//! The desert's own sound, a continuous stereo generator: a wind bed with
//! slow gusts and a faint hiss of blowing sand, and a sandstorm layer
//! (a howl through the wrecks, a low roar, sand rattling on the hull) that
//! comes up with storm intensity. Both sides use their own noise, so the bed
//! is wide; the gusts are shared, so it moves as one wind.

use crate::dsp::{one_pole_k, soft_clip, OnePole, Rng, Svf};

/// Paul Kellet's economy pink filter: wind is pink, not white.
#[derive(Clone, Copy, Debug, Default)]
struct Pink([f32; 3]);

impl Pink {
    fn tick(&mut self, w: f32) -> f32 {
        let b = &mut self.0;
        b[0] = 0.99765 * b[0] + w * 0.099_046;
        b[1] = 0.963 * b[1] + w * 0.296_516_4;
        b[2] = 0.57 * b[2] + w * 1.052_691_3;
        (b[0] + b[1] + b[2] + w * 0.1848) * 0.2
    }
}

#[derive(Clone, Debug)]
struct Side {
    rng: Rng,
    pink: Pink,
    body: Svf,
    hiss_hp: OnePole,
    hiss_lp: OnePole,
    howl: [Svf; 2],
    roar: [OnePole; 2],
    rattle: Svf,
    click: f32,
    dc_in: [f32; 2],
    dc_out: [f32; 2],
}

impl Side {
    fn new(seed: u64, sr: f32) -> Self {
        let mut body = Svf::default();
        body.set_sr(400.0, 0.7, sr);
        let mut rattle = Svf::default();
        rattle.set_sr(3000.0, 1.0, sr);
        let mut howl = [Svf::default(); 2];
        for h in &mut howl {
            h.set_sr(500.0, 10.0, sr);
        }
        Self {
            rng: Rng::new(seed),
            pink: Pink::default(),
            body,
            hiss_hp: OnePole::default(),
            hiss_lp: OnePole::default(),
            howl,
            roar: [OnePole::default(); 2],
            rattle,
            click: 0.0,
            dc_in: [0.0; 2],
            dc_out: [0.0; 2],
        }
    }
}

/// A slow random wander in `0..1`: a new target every few seconds, glided to.
#[derive(Clone, Debug)]
struct Gust {
    value: f32,
    target: f32,
    clock: f32,
    every: (f32, f32),
    tau: f32,
}

impl Gust {
    fn tick(&mut self, dt: f32, rng: &mut Rng) -> f32 {
        self.clock -= dt;
        if self.clock <= 0.0 {
            self.clock = rng.range(self.every.0, self.every.1);
            // Mostly moderate, now and then a real gust.
            let u = rng.unit();
            self.target = u * u * 0.8 + 0.2 * rng.unit();
        }
        self.value += (self.target - self.value) * (dt / self.tau).min(1.0);
        self.value
    }
}

/// The ambience generator. `set` it every frame or whenever the weather
/// changes; it glides to new settings over about half a second.
#[derive(Clone, Debug)]
pub struct Ambience {
    sr: f32,
    dt: f32,
    target: (f32, f32),
    wind: f32,
    storm: f32,
    k: f32,
    rng: Rng,
    gust: Gust,
    squall: Gust,
    sides: [Side; 2],
    clock: u32,
    hiss_k: (f32, f32),
    roar_k: f32,
    dc_r: f32,
}

impl Ambience {
    #[must_use]
    pub fn new(seed: u64, sample_rate: f32) -> Self {
        let sr = if sample_rate.is_finite() && sample_rate >= 8000.0 { sample_rate.min(192_000.0) } else { 48_000.0 };
        let dt = 1.0 / sr;
        Self {
            sr,
            dt,
            target: (0.0, 0.0),
            wind: 0.0,
            storm: 0.0,
            k: 1.0 - (-dt / 0.5).exp(),
            rng: Rng::new(seed ^ 0x00D0_5E57),
            gust: Gust { value: 0.4, target: 0.4, clock: 0.0, every: (1.5, 4.0), tau: 0.9 },
            squall: Gust { value: 0.5, target: 0.5, clock: 0.0, every: (0.4, 1.4), tau: 0.35 },
            sides: [Side::new(seed.wrapping_add(1), sr), Side::new(seed.wrapping_add(2), sr)],
            clock: 0,
            hiss_k: (one_pole_k(2500.0, sr), one_pole_k(6500.0, sr)),
            roar_k: one_pole_k(170.0, sr),
            dc_r: 1.0 - std::f32::consts::TAU * 25.0 / sr,
        }
    }

    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// `wind`: the everyday desert wind, `0` still .. `1` blustery.
    /// `storm`: the sandstorm layer, `0` none .. `1` full storm.
    /// Non-finite values are ignored.
    pub fn set(&mut self, wind: f32, storm: f32) {
        let f = |v: f32, old: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { old };
        self.target = (f(wind, self.target.0), f(storm, self.target.1));
    }

    /// `(wind, storm)` as last set.
    #[must_use]
    pub fn settings(&self) -> (f32, f32) {
        self.target
    }

    /// Writes (not adds) the next block into `left` and `right`.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        for i in 0..n {
            let (l, r) = self.next_pair();
            left[i] = l;
            right[i] = r;
        }
    }

    /// One stereo sample, each side bounded in `-1..1`.
    pub fn next_pair(&mut self) -> (f32, f32) {
        self.wind += (self.target.0 - self.wind) * self.k;
        self.storm += (self.target.1 - self.storm) * self.k;
        if self.wind < 1e-4 && self.storm < 1e-4 && self.target == (0.0, 0.0) {
            return (0.0, 0.0);
        }
        let dt = self.dt;
        let gust = self.gust.tick(dt, &mut self.rng);
        let squall = self.squall.tick(dt, &mut self.rng);
        let (wind, storm) = (self.wind, self.storm);
        // The storm drives the wind up too.
        let blow = (wind * (0.45 + 0.55 * gust) + storm * 0.6).min(1.3);
        let refresh = self.clock.is_multiple_of(32);
        self.clock = self.clock.wrapping_add(1);
        let sr = self.sr;
        let howl_hz = 330.0 + 420.0 * squall + 120.0 * gust;
        let rattle_rate = storm * storm * (250.0 + 500.0 * squall) / sr;
        let mut out = [0.0f32; 2];
        for (c, side) in self.sides.iter_mut().enumerate() {
            if refresh {
                side.body.set_sr(220.0 + 520.0 * gust + 60.0 * c as f32, 0.7, sr);
                let h = howl_hz * if c == 0 { 1.0 } else { 1.03 };
                side.howl[0].set_sr(h, 9.0, sr);
                side.howl[1].set_sr(h * 1.47, 12.0, sr);
            }
            let w = side.rng.bi();
            let pink = side.pink.tick(w);
            // The bed: pink noise through a band that opens with the gusts.
            let bed = side.body.tick(pink).bp * blow * 0.9;
            // Sand hiss: soft, only really there in a gust.
            let hiss = side.hiss_lp.lp(side.hiss_hp.hp(w, self.hiss_k.0), self.hiss_k.1)
                * blow
                * gust
                * gust
                * 0.12;
            // The storm: a howl through the wrecks, a roar under it.
            let howl = (side.howl[0].tick(pink).bp + 0.5 * side.howl[1].tick(pink).bp) * storm * (0.4 + 0.6 * squall) * 2.2;
            let r = side.roar[0].lp(pink, self.roar_k);
            let roar = side.roar[1].lp(r, self.roar_k) * storm * 1.8;
            // Sand rattling on plate: grains of grit through a metallic band.
            if side.rng.unit() < rattle_rate {
                side.click = side.rng.unit().powi(2) * if side.rng.unit() < 0.5 { -1.0 } else { 1.0 };
            }
            let rattle = side.rattle.tick(side.click + w * side.click.abs() * 0.3).bp * 1.2;
            side.click *= 0.93;
            let x = bed + hiss + howl + roar + rattle;
            // Two high-passes near 25 Hz: pink noise wanders below that.
            let y = x - side.dc_in[0] + self.dc_r * side.dc_out[0];
            side.dc_in[0] = x;
            side.dc_out[0] = y;
            let z = y - side.dc_in[1] + self.dc_r * side.dc_out[1];
            side.dc_in[1] = y;
            side.dc_out[1] = z;
            out[c] = soft_clip(z * 0.8) * 0.97;
        }
        (out[0], out[1])
    }
}
