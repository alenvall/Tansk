//! The tank engine: a continuous voice, generated block by block, never a
//! buffer. brick-racers' pulse-and-pipe model (exhaust pulses at the real
//! firing rate, ringing through a pipe with a muffler in its feedback loop)
//! tuned as a big junkyard diesel, plus the parts of a tank a car does not
//! have: injector knock, a growl that comes up with load, a nitro roar and
//! turbo whistle on boost, and two tracks clattering at the rate their links
//! hit the sprockets.

use crate::dsp::{drive, one_pole_k, soft_clip, OnePole, Resonator, Rng, Svf};

const MAX_CYL: usize = 12;
const MAX_PULSES: usize = 16;
const SOUND_SPEED: f32 = 450.0;
const MAX_PIPE_M: f32 = 5.0;

/// Which engine a chassis carries (docs/DESIGN.md, "Garage").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EngineKind {
    /// A light, rev-happy straight six.
    Scout,
    /// A V8 diesel: the default growl.
    Brawler,
    /// A slow, huge V12 diesel.
    Hauler,
}

impl EngineKind {
    pub const ALL: [Self; 3] = [Self::Scout, Self::Brawler, Self::Hauler];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scout => "scout",
            Self::Brawler => "brawler",
            Self::Hauler => "hauler",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Spec {
    cylinders: usize,
    idle_rpm: f32,
    max_rpm: f32,
    /// Relative gap after each firing (uneven is a V's burble).
    gaps: [f32; MAX_CYL],
    pipe_m: f32,
    feedback: f32,
    muffler_hz: f32,
    pulse_on_ms: f32,
    pulse_off_ms: f32,
    strength_jitter: f32,
    knock: f32,
    /// Track link pitch, metres; and the link's ring.
    link_m: f32,
    link_hz: f32,
    level: f32,
}

impl Spec {
    fn of(kind: EngineKind) -> Self {
        let even = [1.0; MAX_CYL];
        match kind {
            EngineKind::Scout => Self {
                cylinders: 6,
                idle_rpm: 800.0,
                max_rpm: 3400.0,
                gaps: even,
                pipe_m: 2.2,
                feedback: 0.5,
                muffler_hz: 1800.0,
                pulse_on_ms: 1.3,
                pulse_off_ms: 2.6,
                strength_jitter: 0.18,
                knock: 0.5,
                link_m: 0.13,
                link_hz: 900.0,
                level: 0.8,
            },
            EngineKind::Brawler => Self {
                cylinders: 8,
                idle_rpm: 620.0,
                max_rpm: 2700.0,
                // Cross-plane: uneven gaps, the burble.
                gaps: [1.0, 0.8, 1.3, 0.9, 1.0, 1.2, 0.8, 1.0, 1.0, 1.0, 1.0, 1.0],
                pipe_m: 3.0,
                feedback: 0.5,
                muffler_hz: 1300.0,
                pulse_on_ms: 1.7,
                pulse_off_ms: 3.2,
                strength_jitter: 0.22,
                knock: 0.6,
                link_m: 0.16,
                link_hz: 700.0,
                level: 1.1,
            },
            EngineKind::Hauler => Self {
                cylinders: 12,
                idle_rpm: 520.0,
                max_rpm: 2200.0,
                gaps: even,
                pipe_m: 4.0,
                feedback: 0.45,
                muffler_hz: 1000.0,
                pulse_on_ms: 2.0,
                pulse_off_ms: 3.8,
                strength_jitter: 0.25,
                knock: 0.3,
                link_m: 0.2,
                link_hz: 520.0,
                level: 1.6,
            },
        }
    }
}

/// What the engine is doing, written by the game every frame. Every field
/// is smoothed inside the voice, so stepping it is fine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineInput {
    /// Engine speed, `0` idle to `1` governed maximum.
    pub rpm: f32,
    /// How hard it is pulling, `0..1` (throttle against resistance).
    pub load: f32,
    /// Nitro / afterburner, `0..1`.
    pub boost: f32,
    /// Ground speed of the tracks, m/s (the faster track; sign ignored).
    pub track_speed: f32,
}

impl Default for EngineInput {
    fn default() -> Self {
        Self { rpm: 0.0, load: 0.0, boost: 0.0, track_speed: 0.0 }
    }
}

#[derive(Clone, Copy, Debug)]
struct Pulse {
    age: f32,
    width: f32,
    amp: f32,
    /// `e^(1 - age/width)`, kept by a per-sample multiply.
    e: f32,
    k: f32,
}

/// One track's links striking the sprocket and wheels.
#[derive(Clone, Debug)]
struct Track {
    phase: f32,
    modes: [Resonator; 3],
    gains: [f32; 3],
    clunk: Resonator,
}

/// One tank's engine, mono. The caller pans it.
#[derive(Clone, Debug)]
pub struct TankEngine {
    spec: Spec,
    sr: f32,
    dt: f32,
    rng: Rng,
    target: EngineInput,
    s: EngineInput,
    positions: [f32; MAX_CYL],
    strength: [f32; MAX_CYL],
    timing: [f32; MAX_CYL],
    cycle: f32,
    next_cyl: usize,
    pulses: [Pulse; MAX_PULSES],
    delay: Vec<f32>,
    write: usize,
    muffler: f32,
    outlet: f32,
    knock_env: f32,
    knock_bp: Svf,
    intake: OnePole,
    roar: OnePole,
    roar2: OnePole,
    hunt: f32,
    hunt_target: f32,
    hunt_clock: f32,
    spool: f32,
    whistle_phase: f32,
    tracks: [Track; 2],
    rumble: OnePole,
    rumble2: OnePole,
    dc_in: f32,
    dc_out: f32,
    dc_in2: f32,
    dc_out2: f32,
    dc_r: f32,
    k_slow: f32,
    k_fast: f32,
    /// Fixed filter coefficients: the roar, the rumble.
    kc: [f32; 2],
    /// Load-dependent coefficients refreshed at a control rate: muffler,
    /// intake.
    mk: f32,
    ik: f32,
    clock: u32,
}

impl TankEngine {
    /// A voice for `kind` at `sample_rate`, parked at idle. `seed` gives the
    /// engine its own character (cylinder strengths, timing, link rings),
    /// so two tanks of one kind still sound like two tanks.
    #[must_use]
    pub fn new(kind: EngineKind, seed: u64, sample_rate: f32) -> Self {
        let sr = if sample_rate.is_finite() && sample_rate >= 8000.0 { sample_rate.min(192_000.0) } else { 48_000.0 };
        let spec = Spec::of(kind);
        let mut rng = Rng::new(seed ^ 0x7A4C_E11E);
        let n = spec.cylinders;
        let mut positions = [0.0; MAX_CYL];
        let total: f32 = spec.gaps[..n].iter().sum();
        let mut acc = 0.0;
        for (p, gap) in positions.iter_mut().zip(&spec.gaps[..n]) {
            *p = acc / total;
            acc += gap;
        }
        let mut strength = [1.0; MAX_CYL];
        let mut timing = [0.0; MAX_CYL];
        for i in 0..n {
            strength[i] = 1.0 + spec.strength_jitter * rng.bi();
            timing[i] = 0.06 * rng.bi();
        }
        let track = |rng: &mut Rng| {
            let f = spec.link_hz * rng.vary(0.08);
            Track {
                phase: rng.unit(),
                modes: [
                    Resonator::new(f, 0.05, sr),
                    Resonator::new(f * 2.37 * rng.vary(0.03), 0.035, sr),
                    Resonator::new(f * 4.1 * rng.vary(0.03), 0.02, sr),
                ],
                gains: [1.0, 0.55, 0.3],
                clunk: Resonator::new(spec.link_hz * 0.28 * rng.vary(0.05), 0.06, sr),
            }
        };
        let tracks = [track(&mut rng), track(&mut rng)];
        let delay_len = (sr * 2.0 * MAX_PIPE_M / SOUND_SPEED) as usize + 2;
        let dt = 1.0 / sr;
        Self {
            spec,
            sr,
            dt,
            rng,
            target: EngineInput::default(),
            s: EngineInput::default(),
            positions,
            strength,
            timing,
            cycle: 0.0,
            next_cyl: 0,
            pulses: [Pulse { age: -1.0, width: 1.0, amp: 0.0, e: 0.0, k: 0.0 }; MAX_PULSES],
            delay: vec![0.0; delay_len],
            write: 0,
            muffler: 0.0,
            outlet: 0.0,
            knock_env: 0.0,
            knock_bp: {
                let mut f = Svf::default();
                f.set_sr(1600.0, 0.8, sr);
                f
            },
            intake: OnePole::default(),
            roar: OnePole::default(),
            roar2: OnePole::default(),
            hunt: 0.0,
            hunt_target: 0.0,
            hunt_clock: 0.0,
            spool: 0.0,
            whistle_phase: 0.0,
            tracks,
            rumble: OnePole::default(),
            rumble2: OnePole::default(),
            dc_in: 0.0,
            dc_out: 0.0,
            dc_in2: 0.0,
            dc_out2: 0.0,
            dc_r: 1.0 - std::f32::consts::TAU * 15.0 / sr,
            k_slow: 1.0 - (-dt / 0.12).exp(),
            k_fast: 1.0 - (-dt / 0.04).exp(),
            kc: [one_pole_k(900.0, sr), one_pole_k(140.0, sr)],
            mk: one_pole_k(spec.muffler_hz, sr),
            ik: one_pole_k(250.0, sr),
            clock: 0,
        }
    }

    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// Sets what the engine is doing; non-finite values are ignored.
    pub fn set_input(&mut self, input: EngineInput) {
        let f = |v: f32, lo: f32, hi: f32, old: f32| if v.is_finite() { v.clamp(lo, hi) } else { old };
        let t = self.target;
        self.target = EngineInput {
            rpm: f(input.rpm, 0.0, 1.0, t.rpm),
            load: f(input.load, 0.0, 1.0, t.load),
            boost: f(input.boost, 0.0, 1.0, t.boost),
            track_speed: f(input.track_speed.abs(), 0.0, 40.0, t.track_speed),
        };
    }

    #[must_use]
    pub fn input(&self) -> EngineInput {
        self.target
    }

    /// Firings per second right now (for tests and debugging).
    #[must_use]
    pub fn firing_hz(&self) -> f32 {
        self.rpm_now() / 120.0 * self.spec.cylinders as f32
    }

    fn rpm_now(&self) -> f32 {
        let share = (self.s.rpm + 0.08 * self.s.boost).min(1.1);
        let rpm = self.spec.idle_rpm + (self.spec.max_rpm - self.spec.idle_rpm) * share;
        rpm * (1.0 + self.hunt)
    }

    /// Writes the next `out.len()` samples.
    pub fn render(&mut self, out: &mut [f32]) {
        for s in out.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Adds the next `out.len()` samples, times `gain`.
    pub fn render_add(&mut self, out: &mut [f32], gain: f32) {
        for s in out.iter_mut() {
            *s += self.next_sample() * gain;
        }
    }

    /// One sample, bounded in `-1..1`.
    pub fn next_sample(&mut self) -> f32 {
        let dt = self.dt;
        let sr = self.sr;
        // ── Smooth the inputs ─────────────────────────────────────────
        let (ks, kf) = (self.k_slow, self.k_fast);
        self.s.rpm += (self.target.rpm - self.s.rpm) * ks;
        self.s.load += (self.target.load - self.s.load) * kf;
        self.s.boost += (self.target.boost - self.s.boost) * kf;
        self.s.track_speed += (self.target.track_speed - self.s.track_speed) * ks;
        let s = self.s;
        let spec = self.spec;

        // ── An idle that hunts, as an old diesel's does ──────────────
        self.hunt_clock += dt;
        if self.hunt_clock > 0.35 {
            self.hunt_clock = 0.0;
            self.hunt_target = 0.025 * self.rng.bi() * (1.0 - s.rpm).powi(2);
        }
        self.hunt += (self.hunt_target - self.hunt) * (dt / 0.25);

        // ── Firings ───────────────────────────────────────────────────
        let n = spec.cylinders;
        let rpm = self.rpm_now();
        self.cycle += rpm / 120.0 * dt;
        let load = (s.load + 0.5 * s.boost).min(1.0);
        let mut guard = 0;
        loop {
            let due = self.positions[self.next_cyl] + self.timing[self.next_cyl] / n as f32;
            if self.cycle < due || guard >= n * 2 {
                break;
            }
            guard += 1;
            self.fire(load, s.rpm);
            self.next_cyl += 1;
            if self.next_cyl >= n {
                self.next_cyl = 0;
                self.cycle -= 1.0;
            }
        }
        if self.cycle > 2.0 {
            self.cycle = 0.0;
        }
        let mut raw = 0.0;
        for p in &mut self.pulses {
            if p.age < 0.0 {
                continue;
            }
            let x = p.age / p.width;
            if x > 6.0 {
                p.age = -1.0;
                continue;
            }
            raw += p.amp * x * p.e;
            p.e *= p.k;
            p.age += dt;
        }

        // ── The pipe ──────────────────────────────────────────────────
        let delay = ((sr * 2.0 * spec.pipe_m / SOUND_SPEED) as usize).clamp(1, self.delay.len() - 1);
        let len = self.delay.len();
        let echo = self.delay[(self.write + len - delay) % len];
        if self.clock.is_multiple_of(32) {
            self.mk = one_pole_k(spec.muffler_hz * (1.0 + 0.6 * load), sr);
            self.ik = one_pole_k(250.0 + 1400.0 * s.rpm, sr);
        }
        self.clock = self.clock.wrapping_add(1);
        let mk = self.mk;
        self.muffler += (echo - self.muffler) * mk;
        let into = raw - spec.feedback * self.muffler;
        self.delay[self.write] = into;
        self.write = (self.write + 1) % len;
        self.outlet += (into - self.outlet) * mk;
        // The growl: the pipe driven harder under load and boost.
        let growl = 0.6 + 2.0 * load + 1.5 * s.boost;
        let exhaust = drive(self.outlet * (1.1 + 0.5 * load), growl) * 0.5;

        // ── Diesel knock: injector clatter on each firing ─────────────
        let noise = self.rng.bi();
        self.knock_env *= 1.0 - dt / 0.003;
        let knock = self.knock_bp.tick(noise).bp * self.knock_env;

        // ── Intake and the boost roar ─────────────────────────────────
        let intake = self.intake.lp(noise, self.ik) * (0.05 + 0.25 * load) * (0.4 + s.rpm);
        let r1 = self.roar.lp(noise, self.kc[0]);
        let r2 = self.roar2.lp(r1, self.kc[0]);
        let roar = (r1 - 0.5 * r2) * s.boost * 0.5;
        // Turbo whistle, lagging the boost.
        let tau = if s.boost > self.spool { 0.5 } else { 0.25 };
        self.spool += (s.boost.max(0.3 * load * s.rpm) - self.spool) * (dt / tau);
        let wf = 1600.0 + 1800.0 * self.spool;
        self.whistle_phase = (self.whistle_phase + wf * dt).fract();
        let whistle = crate::dsp::sin01(self.whistle_phase) * self.spool * self.spool * 0.04;

        // ── Tracks ────────────────────────────────────────────────────
        let speed = s.track_speed;
        let clat = (speed / 10.0).min(1.3).powf(0.8);
        let mut track = 0.0;
        if speed > 0.05 || clat > 1e-4 {
            let rate = speed / spec.link_m;
            for (i, t) in self.tracks.iter_mut().enumerate() {
                t.phase += rate * dt * (1.0 + 0.031 * i as f32);
                let mut x = 0.0;
                if t.phase >= 1.0 {
                    t.phase -= 1.0;
                    // Each link lands a little differently.
                    x = clat * (0.55 + 0.45 * self.rng.unit());
                }
                let mut y = 0.0;
                for (r, g) in t.modes.iter_mut().zip(t.gains) {
                    y += r.tick(x) * g;
                }
                y += t.clunk.tick(x) * 0.8;
                track += y;
            }
        }
        let r = self.rumble.lp(noise, self.kc[1]);
        let rumble = self.rumble2.lp(r, self.kc[1]) * clat * 3.0;

        // ── Mix, DC block, bound ──────────────────────────────────────
        let mixed = exhaust + knock * spec.knock * 1.6 + intake * 0.25 + roar + whistle + track * 0.09 + rumble * 0.6;
        let y = mixed - self.dc_in + self.dc_r * self.dc_out;
        self.dc_in = mixed;
        self.dc_out = y;
        let z = y - self.dc_in2 + self.dc_r * self.dc_out2;
        self.dc_in2 = y;
        self.dc_out2 = z;
        soft_clip(z * spec.level) * 0.97
    }

    fn fire(&mut self, load: f32, rpm: f32) {
        let spec = self.spec;
        let c = self.next_cyl;
        // A chugging idle: big per-firing variation that settles up the revs.
        let jitter = 0.08 + 0.12 * (1.0 - rpm);
        let amp = self.strength[c] * (1.0 + jitter * self.rng.bi()) * (0.7 + 0.3 * load);
        let width = spec.pulse_off_ms + (spec.pulse_on_ms - spec.pulse_off_ms) * load;
        // Knock is loudest at idle and light load.
        self.knock_env = self.knock_env.max(amp * (1.0 - 0.5 * load));
        let width = (width * 1e-3).max(self.dt);
        let slot = self.pulses.iter().position(|p| p.age < 0.0).unwrap_or_else(|| {
            let mut oldest = 0;
            for (i, p) in self.pulses.iter().enumerate() {
                if p.age > self.pulses[oldest].age {
                    oldest = i;
                }
            }
            oldest
        });
        self.pulses[slot] = Pulse { age: 0.0, width, amp: amp * 0.35, e: std::f32::consts::E, k: (-self.dt / width).exp() };
    }
}
