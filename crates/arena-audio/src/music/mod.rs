//! Procedural music: a small band synthesized live, playing the scores in
//! [`score`]. No samples, no recordings, nothing pre-rendered but a few
//! drum hits (about 10 ms of work).
//!
//! - **Baritone guitar**: Karplus-Strong plucks (a noise burst ringing round
//!   a tuned delay line), the pick near the bridge for twang, palm mutes for
//!   the chug, a little amp grit, and a spring reverb.
//! - **Bass**: a low saw with a sub, through an enveloped low-pass, driven.
//! - **Harmonica-ish lead**: a reedy wavetable through a mouth formant, with
//!   breath, scoops into long notes and delayed vibrato.
//! - **Drone**: two detuned saws, dark, swelling.
//! - **Kit**: kick, snare, three toms, a stomp, a clap, a shaker and a
//!   scrap-metal clank, rendered once with the same layers as the sound
//!   effects and played back with a little variation.
//!
//! [`Music`] is a real-time stereo generator: `set(track, intensity)`,
//! `stinger(s)`, `render(left, right)`. Tracks crossfade; the battle's layers
//! are mixed by intensity; stingers play over the music and duck it.

pub mod score;

use crate::dsp::{drive, fast_tanh, one_pole_k, sin01, OnePole, Rng, Svf, SR};
use crate::layers::{modes, noise, strike, thump, Band, Noise, Thump, Wavetable, PLATE};
use score::{Ev, Inst, Song, LAYERS};

const E1: f32 = 41.203;
const E2: f32 = 82.407;
const E4: f32 = 329.63;
const PLUCKS: usize = 10;
const HITS: usize = 16;
const PADS: usize = 3;
/// Samples between gain refreshes.
const BLOCK: usize = 64;

/// What is playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MusicTrack {
    Silence,
    /// The title theme: 80 s, loops.
    Title,
    /// The battle loop: 34 s, mixed by intensity.
    Battle,
}

/// Short cues played over the music.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stinger {
    RoundStart,
    Victory,
    Defeat,
    WaveStart,
    BossAppears,
}

impl Stinger {
    pub const ALL: [Self; 5] = [Self::RoundStart, Self::Victory, Self::Defeat, Self::WaveStart, Self::BossAppears];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RoundStart => "round_start",
            Self::Victory => "victory",
            Self::Defeat => "defeat",
            Self::WaveStart => "wave_start",
            Self::BossAppears => "boss_appears",
        }
    }

    fn song(self) -> Song {
        match self {
            Self::RoundStart => score::round_start(),
            Self::Victory => score::victory(),
            Self::Defeat => score::defeat(),
            Self::WaveStart => score::wave_start(),
            Self::BossAppears => score::boss_appears(),
        }
    }
}

fn semis(base: f32, n: i8) -> f32 {
    base * 2f32.powf(f32::from(n) / 12.0)
}

// ── The kit ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drum {
    Kick,
    Snare,
    TomHi,
    TomMid,
    TomLo,
    Stomp,
    Clap,
    Clank,
    Shaker,
}

const DRUMS: [Drum; 9] =
    [Drum::Kick, Drum::Snare, Drum::TomHi, Drum::TomMid, Drum::TomLo, Drum::Stomp, Drum::Clap, Drum::Clank, Drum::Shaker];
const TAKES: usize = 2;

impl Drum {
    fn of(inst: Inst) -> Option<Self> {
        Some(match inst {
            Inst::Kick => Self::Kick,
            Inst::Snare => Self::Snare,
            Inst::TomHi => Self::TomHi,
            Inst::TomMid => Self::TomMid,
            Inst::TomLo => Self::TomLo,
            Inst::Stomp => Self::Stomp,
            Inst::Clap => Self::Clap,
            Inst::Clank => Self::Clank,
            Inst::Shaker => Self::Shaker,
            _ => return None,
        })
    }

    /// (pan, reverb send)
    fn place(self) -> (f32, f32) {
        match self {
            Self::Kick | Self::Stomp => (0.0, 0.05),
            Self::Snare | Self::Clap => (0.05, 0.15),
            Self::TomHi => (-0.35, 0.12),
            Self::TomMid => (0.0, 0.12),
            Self::TomLo => (0.35, 0.12),
            Self::Clank => (0.45, 0.3),
            Self::Shaker => (-0.4, 0.05),
        }
    }

    /// One dusty take, mono at 48 kHz.
    fn render(self, rng: &mut Rng) -> Vec<f32> {
        let len = (0.7 * SR) as usize;
        let mut b = vec![0.0f32; len];
        let v = rng.vary(0.04);
        let peak = match self {
            Self::Kick => {
                thump(&mut b, 0.0, Thump::new(130.0 * v, 46.0, 0.025, 0.12).drive(1.8), 1.0);
                noise(&mut b, 0.0, Noise::lp(4000.0, 800.0, 0.004, 0.004), 0.4, rng);
                0.95
            }
            Self::Snare => {
                thump(&mut b, 0.0, Thump::new(210.0 * v, 175.0, 0.02, 0.05), 0.6);
                noise(&mut b, 0.0, Noise::bp(1900.0 * v, 0.6, 0.09).attack(0.0005), 0.9, rng);
                noise(&mut b, 0.0, Noise::lp(6000.0, 3000.0, 0.03, 0.05), 0.35, rng);
                0.7
            }
            Self::TomHi | Self::TomMid | Self::TomLo => {
                let f = match self {
                    Self::TomHi => 165.0,
                    Self::TomMid => 125.0,
                    _ => 92.0,
                } * v;
                thump(&mut b, 0.0, Thump::new(f * 1.35, f, 0.03, 0.2).drive(0.8), 1.0);
                noise(&mut b, 0.0, Noise::lp(1500.0, 500.0, 0.02, 0.03), 0.3, rng);
                0.75
            }
            Self::Stomp => {
                // A boot on a wooden platform.
                thump(&mut b, 0.0, Thump::new(95.0 * v, 58.0, 0.02, 0.07).drive(0.8), 1.0);
                noise(&mut b, 0.0, Noise::lp(900.0, 300.0, 0.02, 0.04), 0.7, rng);
                let wood = [crate::layers::m(190.0 * v, 0.07, 1.0), crate::layers::m(430.0 * v, 0.05, 0.5)];
                strike(&mut b, 0.0, &wood, 0.002, 0.6, 0.6, rng);
                0.85
            }
            Self::Clap => {
                for (k, t) in [0.0, 0.011, 0.021].into_iter().enumerate() {
                    noise(&mut b, t, Noise::bp(1250.0, 0.9, 0.006), 1.0 - 0.15 * k as f32, rng);
                }
                noise(&mut b, 0.028, Noise::bp(1150.0, 0.8, 0.07), 0.7, rng);
                0.55
            }
            Self::Clank => {
                // A length of scrap pipe or a brake drum, struck.
                let md = modes(rng.range(380.0, 520.0), &PLATE, 0.5, 0.7, 0.75, rng);
                strike(&mut b, 0.0, &md, 0.0006, 0.6, 1.0, rng);
                noise(&mut b, 0.0, Noise::lp(7000.0, 2000.0, 0.003, 0.004), 0.3, rng);
                0.5
            }
            Self::Shaker => {
                // A tin of sand.
                noise(&mut b, 0.0, Noise::new(Band::Band, 4200.0, 4200.0, 1.0, 0.03).attack(0.008).q(0.9), 1.0, rng);
                0.22
            }
        };
        crate::dsp::finish(&mut b, peak);
        b
    }
}

// ── Voices ──────────────────────────────────────────────────────────────

/// Which song (slot generation) and layer a voice belongs to, for its gain.
#[derive(Clone, Copy, Debug, Default)]
struct Owner {
    gen: u32,
    layer: u8,
}

#[derive(Clone, Debug)]
struct Pluck {
    buf: Vec<f32>,
    len: f32,
    w: usize,
    last: f32,
    damp: f32,
    loss: f32,
    hold: u32,
    delay: u32,
    release_loss: f32,
    active: bool,
    age: u32,
    owner: Owner,
    gain: f32,
}

impl Pluck {
    fn new() -> Self {
        Self {
            buf: vec![0.0; 4096],
            len: 100.0,
            w: 0,
            last: 0.0,
            damp: 0.3,
            loss: 1.0,
            hold: 0,
            delay: 0,
            release_loss: 0.9,
            active: false,
            age: 0,
            owner: Owner::default(),
            gain: 0.0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn start(&mut self, f: f32, vel: f32, hold: u32, muted: bool, delay: u32, sr: f32, rng: &mut Rng) {
        let period = sr / f.clamp(30.0, 2000.0);
        self.damp = if muted { 0.62 } else { 0.18 + 0.12 * (1.0 - vel) };
        // The loop's low-pass delays it; take that out to stay in tune.
        self.len = (period - self.damp / (1.0 - self.damp)).clamp(4.0, (self.buf.len() - 4) as f32);
        let t60 = if muted { 0.22 } else { 3.2 * (110.0 / f).powf(0.3) };
        self.loss = 10f32.powf(-3.0 * (period / sr) / t60);
        self.release_loss = 10f32.powf(-3.0 * (period / sr) / 0.09);
        // The pick: brighter for harder notes, near the bridge for twang.
        let n = self.len.ceil() as usize + 1;
        let k = 0.35 + 0.55 * vel;
        let mut lp = 0.0f32;
        let mut burst = vec![0.0f32; n];
        for v in &mut burst {
            lp += (rng.bi() - lp) * k;
            *v = lp;
        }
        let pick = ((n as f32) * 0.14) as usize;
        self.buf.fill(0.0);
        for i in 0..n {
            let comb = burst[i] - if i >= pick { burst[i - pick] } else { 0.0 };
            self.buf[i] = comb * vel;
        }
        self.w = n % self.buf.len();
        self.last = 0.0;
        self.hold = hold;
        self.delay = delay;
        self.active = true;
        self.age = 0;
    }

    fn tick(&mut self) -> f32 {
        if self.delay > 0 {
            self.delay -= 1;
            return 0.0;
        }
        let size = self.buf.len();
        let read = self.w as f32 - self.len;
        let read = if read < 0.0 { read + size as f32 } else { read };
        let i = (read as usize).min(size - 1);
        let frac = read - i as f32;
        let j = if i + 1 == size { 0 } else { i + 1 };
        let a = self.buf[i];
        let b = self.buf[j];
        let y = a + (b - a) * frac;
        self.last = y * (1.0 - self.damp) + self.last * self.damp;
        let loss = if self.hold > 0 {
            self.hold -= 1;
            self.loss
        } else {
            self.release_loss
        };
        self.buf[self.w] = self.last * loss;
        self.w += 1;
        if self.w == size {
            self.w = 0;
        }
        self.age += 1;
        if self.hold == 0 && self.age.is_multiple_of(512) && y.abs() < 1e-4 && self.last.abs() < 1e-4 {
            self.active = false;
        }
        y
    }
}

#[derive(Clone, Debug, Default)]
struct Bass {
    f: f32,
    target: f32,
    phase: f32,
    sub: f32,
    env: f32,
    hold: u32,
    vel: f32,
    filt: Svf,
    owner: Owner,
    gain: f32,
}

impl Bass {
    fn tick(&mut self, sr: f32, clock: usize) -> f32 {
        let gate = self.hold > 0;
        if gate {
            self.hold -= 1;
            self.env += (self.vel - self.env) * (1.0 / (0.004 * sr)).min(1.0);
        } else {
            self.env *= 1.0 - 1.0 / (0.05 * sr);
        }
        if self.env < 1e-5 {
            return 0.0;
        }
        self.f += (self.target - self.f) * (1.0 / (0.015 * sr)).min(1.0);
        self.phase = (self.phase + self.f / sr).fract();
        self.sub = (self.sub + self.f / sr).fract();
        if clock.is_multiple_of(16) {
            self.filt.set_sr(250.0 + 1100.0 * self.env, 0.9, sr);
        }
        let saw = 2.0 * self.phase - 1.0;
        let x = self.filt.tick(saw * 0.7 + sin01(self.sub) * 0.6).lp;
        drive(x * 1.6, 2.5) * self.env * 0.55
    }
}

#[derive(Clone, Debug)]
struct Lead {
    table: Wavetable,
    f: f32,
    target: f32,
    glide: f32,
    phase: f32,
    vib: f32,
    since: f32,
    env: f32,
    hold: u32,
    vel: f32,
    mouth: Svf,
    breath: Svf,
    owner: Owner,
    gain: f32,
}

impl Lead {
    fn new() -> Self {
        Self {
            table: Wavetable::new(&[1.0, 0.75, 0.6, 0.5, 0.42, 0.3, 0.24, 0.16, 0.12, 0.08]),
            f: E4,
            target: E4,
            glide: 0.03,
            phase: 0.0,
            vib: 0.0,
            since: 0.0,
            env: 0.0,
            hold: 0,
            vel: 0.0,
            mouth: Svf::new(1400.0, 1.4),
            breath: Svf::new(1300.0, 2.0),
            owner: Owner::default(),
            gain: 0.0,
        }
    }

    fn start(&mut self, f: f32, vel: f32, hold: u32, len_steps: u16, sr: f32) {
        // Long notes are scooped up into, as a bent harp note is.
        if len_steps >= 6 {
            self.f = f * 2f32.powf(-1.6 / 12.0);
            self.glide = 0.13;
        } else {
            if self.env < 0.01 {
                self.f = f;
            }
            self.glide = 0.03;
        }
        self.target = f;
        self.vel = vel;
        self.hold = hold;
        self.since = 0.0;
        self.mouth.set_sr(1400.0, 1.4, sr);
    }

    fn tick(&mut self, sr: f32, clock: usize, rng: &mut Rng) -> f32 {
        let dt = 1.0 / sr;
        if self.hold > 0 {
            self.hold -= 1;
            self.env += (self.vel - self.env) * (dt / 0.03).min(1.0);
        } else {
            self.env *= 1.0 - dt / 0.07;
        }
        if self.env < 1e-5 {
            return 0.0;
        }
        self.since += dt;
        self.f += (self.target - self.f) * (dt / self.glide).min(1.0);
        self.vib = (self.vib + 5.6 * dt).fract();
        let depth = 0.011 * ((self.since - 0.2) / 0.3).clamp(0.0, 1.0);
        let f = self.f * (1.0 + depth * sin01(self.vib));
        self.phase = (self.phase + f / sr).fract();
        if clock.is_multiple_of(16) {
            self.breath.set_sr(f * 2.0, 2.0, sr);
        }
        let reed = self.table.at(self.phase) + self.breath.tick(rng.bi()).bp * 0.35;
        let y = self.mouth.tick(reed);
        let x = reed * 0.35 + y.bp * 1.2;
        drive(x * self.env, 1.2) * 0.5
    }
}

#[derive(Clone, Debug, Default)]
struct Pad {
    f: f32,
    ph: [f32; 4],
    env: f32,
    vel: f32,
    hold: u32,
    filt: [Svf; 2],
    owner: Owner,
    gain: f32,
    active: bool,
}

impl Pad {
    fn tick(&mut self, sr: f32) -> (f32, f32) {
        let dt = 1.0 / sr;
        if self.hold > 0 {
            self.hold -= 1;
            self.env += (self.vel - self.env) * (dt / 1.2).min(1.0);
        } else {
            self.env *= 1.0 - dt / 0.9;
            if self.env < 1e-4 {
                self.active = false;
                return (0.0, 0.0);
            }
        }
        let det = [0.997, 1.003, 1.4983, 2.002];
        let mut out = [0.0f32; 2];
        for (k, ph) in self.ph.iter_mut().enumerate() {
            *ph = (*ph + self.f * det[k] / sr).fract();
            let saw = 2.0 * *ph - 1.0;
            out[k % 2] += saw * if k >= 2 { 0.3 } else { 0.5 };
        }
        let l = self.filt[0].tick(out[0]).lp;
        let r = self.filt[1].tick(out[1]).lp;
        (l * self.env * 0.35, r * self.env * 0.35)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Hit {
    drum: usize,
    take: usize,
    pos: f32,
    rate: f32,
    vel: f32,
    active: bool,
    owner: Owner,
    gain: f32,
}

/// A spring tank: dispersive all-passes (the "boing") feeding a damped
/// feedback delay, one per side with different lengths.
#[derive(Clone, Debug)]
struct Spring {
    ap: [Vec<f32>; 4],
    ap_i: [usize; 4],
    line: Vec<f32>,
    line_i: usize,
    damp: OnePole,
    k: f32,
}

impl Spring {
    fn new(sr: f32, side: usize) -> Self {
        let scale = sr / 48_000.0;
        let aps = [[113, 337, 509, 761], [131, 373, 541, 809]][side];
        let line = [1693, 1931][side];
        Self {
            ap: aps.map(|n| vec![0.0; ((n as f32 * scale) as usize).max(2)]),
            ap_i: [0; 4],
            line: vec![0.0; ((line as f32 * scale) as usize).max(2)],
            line_i: 0,
            damp: OnePole::default(),
            k: one_pole_k(2600.0, sr),
        }
    }

    fn tick(&mut self, x: f32) -> f32 {
        let fb = self.line[self.line_i];
        let mut y = x + fb * 0.72;
        for (k, buf) in self.ap.iter_mut().enumerate() {
            let i = self.ap_i[k];
            let d = buf[i];
            let g = 0.62;
            let v = y + g * d;
            buf[i] = v;
            y = d - g * v;
            self.ap_i[k] = (i + 1) % buf.len();
        }
        let y = self.damp.lp(y, self.k);
        self.line[self.line_i] = y;
        self.line_i = (self.line_i + 1) % self.line.len();
        y
    }
}

// ── The player ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Seq {
    song: Song,
    t: f64,
    next: usize,
    step_samples: f64,
    done: bool,
}

impl Seq {
    fn new(song: Song, sr: f32) -> Self {
        let step_samples = f64::from(sr) * 60.0 / f64::from(song.bpm) / 4.0;
        Self { song, t: 0.0, next: 0, step_samples, done: false }
    }

    fn time_of(&self, e: &Ev) -> f64 {
        let swing = if e.step % 2 == 1 { f64::from(self.song.swing) } else { 0.0 };
        (f64::from(e.step) + swing) * self.step_samples
    }

    fn length(&self) -> f64 {
        f64::from(self.song.steps) * self.step_samples
    }
}

#[derive(Clone, Debug)]
struct Slot {
    gen: u32,
    seq: Option<Seq>,
    gain: f32,
    target: f32,
    /// Seconds for a full 0..1 move.
    fade: f32,
    layers: [f32; LAYERS],
    stinger: bool,
    track: MusicTrack,
}

/// The band. One per game; feed it with [`Music::set`] and
/// [`Music::stinger`], pull stereo with [`Music::render`].
#[derive(Clone, Debug)]
pub struct Music {
    sr: f32,
    rng: Rng,
    kit: Vec<Vec<Vec<f32>>>,
    /// Per drum: left gain, right gain, reverb send.
    kit_place: Vec<(f32, f32, f32)>,
    /// Guitar left/right, lead left/right.
    pans: [f32; 4],
    plucks: Vec<Pluck>,
    bass: Bass,
    lead: Lead,
    pads: Vec<Pad>,
    hits: Vec<Hit>,
    springs: [Spring; 2],
    slots: Vec<Slot>,
    gen: u32,
    track: MusicTrack,
    intensity: f32,
    target_intensity: f32,
    clock: usize,
    strum: (u32, f64, u32),
    dc: [(f32, f32); 2],
    /// Output level.
    pub gain: f32,
}

impl Music {
    /// A silent band at `sample_rate`. Renders the drum kit (~10 ms).
    #[must_use]
    pub fn new(seed: u64, sample_rate: f32) -> Self {
        let sr = if sample_rate.is_finite() && sample_rate >= 8000.0 { sample_rate.min(192_000.0) } else { 48_000.0 };
        let mut rng = Rng::new(seed ^ 0x6D75_7369_6321);
        let kit = DRUMS.iter().map(|d| (0..TAKES).map(|_| d.render(&mut rng)).collect()).collect();
        Self {
            sr,
            rng,
            kit,
            kit_place: DRUMS
                .iter()
                .map(|d| {
                    let (pan, send) = d.place();
                    let (l, r) = crate::spatial::pan_gains(pan);
                    (l, r, send)
                })
                .collect(),
            pans: {
                let (gl, gr) = crate::spatial::pan_gains(-0.3);
                let (ll, lr) = crate::spatial::pan_gains(0.25);
                [gl, gr, ll, lr]
            },
            plucks: (0..PLUCKS).map(|_| Pluck::new()).collect(),
            bass: Bass::default(),
            lead: Lead::new(),
            pads: vec![Pad::default(); PADS],
            hits: vec![Hit::default(); HITS],
            springs: [Spring::new(sr, 0), Spring::new(sr, 1)],
            slots: Vec::new(),
            gen: 0,
            track: MusicTrack::Silence,
            intensity: 0.0,
            target_intensity: 0.0,
            clock: 0,
            strum: (u32::MAX, -1.0, 0),
            dc: [(0.0, 0.0); 2],
            gain: 1.0,
        }
    }

    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    #[must_use]
    pub fn track(&self) -> MusicTrack {
        self.track
    }

    /// Chooses the track and, for the battle, how intense it is (`0` calm ..
    /// `1` full band; glides over a couple of seconds). Changing track fades
    /// the old one out over about a second and starts the new one from its
    /// top. Calling it every frame with the same track only moves the
    /// intensity.
    pub fn set(&mut self, track: MusicTrack, intensity: f32) {
        if intensity.is_finite() {
            self.target_intensity = intensity.clamp(0.0, 1.0);
        }
        if track == self.track {
            return;
        }
        self.track = track;
        for s in self.slots.iter_mut().filter(|s| !s.stinger) {
            s.target = 0.0;
            s.fade = 1.2;
            if let Some(q) = &mut s.seq {
                q.done = true;
            }
        }
        let song = match track {
            MusicTrack::Silence => return,
            MusicTrack::Title => score::title(),
            MusicTrack::Battle => {
                self.intensity = self.target_intensity;
                score::battle()
            }
        };
        self.gen += 1;
        self.slots.push(Slot {
            gen: self.gen,
            seq: Some(Seq::new(song, self.sr)),
            gain: 0.0,
            target: 1.0,
            fade: 0.3,
            layers: [1.0; LAYERS],
            stinger: false,
            track,
        });
    }

    /// Plays a stinger over the music, ducking it while it plays. A new
    /// stinger cuts the one before short (with a quick fade).
    pub fn stinger(&mut self, s: Stinger) {
        for slot in self.slots.iter_mut().filter(|x| x.stinger) {
            slot.target = 0.0;
            slot.fade = 0.25;
            if let Some(q) = &mut slot.seq {
                q.done = true;
            }
        }
        self.gen += 1;
        self.slots.push(Slot {
            gen: self.gen,
            seq: Some(Seq::new(s.song(), self.sr)),
            gain: 1.0,
            target: 1.0,
            fade: 0.2,
            layers: [1.0; LAYERS],
            stinger: true,
            track: MusicTrack::Silence,
        });
    }

    /// Whether a stinger's notes are still being played.
    #[must_use]
    pub fn stinger_playing(&self) -> bool {
        self.slots.iter().any(|s| s.stinger && s.seq.as_ref().is_some_and(|q| !q.done))
    }

    /// Seconds in one pass of a track (its loop length).
    #[must_use]
    pub fn track_seconds(track: MusicTrack) -> f32 {
        let song = match track {
            MusicTrack::Silence => return 0.0,
            MusicTrack::Title => score::title(),
            MusicTrack::Battle => score::battle(),
        };
        song.steps as f32 * 15.0 / song.bpm
    }

    /// Seconds of a stinger's notes (its ring-out comes after).
    #[must_use]
    pub fn stinger_seconds(s: Stinger) -> f32 {
        let song = s.song();
        song.steps as f32 * 15.0 / song.bpm
    }

    /// Writes (not adds) the next block into `left` and `right`.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        let mut i = 0;
        while i < n {
            if self.clock.is_multiple_of(BLOCK) {
                self.control();
            }
            let (l, r) = self.next_pair();
            left[i] = l;
            right[i] = r;
            i += 1;
        }
    }

    /// Per-block bookkeeping: fades, intensity, voice gains.
    fn control(&mut self) {
        let dt = BLOCK as f32 / self.sr;
        self.intensity += (self.target_intensity - self.intensity) * (dt / 1.5).min(1.0);
        let ducked = self.stinger_playing();
        for s in &mut self.slots {
            let target = if !s.stinger && ducked && s.target > 0.0 { 0.35 } else { s.target };
            let step = dt / s.fade.max(0.01);
            s.gain = if s.gain < target { (s.gain + step).min(target) } else { (s.gain - step * 1.5).max(target) };
            if s.track == MusicTrack::Battle {
                s.layers = score::battle_layers(self.intensity);
            }
        }
        // A slot is gone when it is silent and finished; its voices go with it.
        self.slots.retain(|s| !(s.gain <= 0.0 && s.target <= 0.0));
        let gain_of = |slots: &[Slot], o: Owner| -> f32 {
            slots.iter().find(|s| s.gen == o.gen).map_or(0.0, |s| s.gain * s.layers[usize::from(o.layer).min(LAYERS - 1)])
        };
        for p in &mut self.plucks {
            p.gain = gain_of(&self.slots, p.owner);
        }
        for h in &mut self.hits {
            h.gain = gain_of(&self.slots, h.owner);
        }
        for p in &mut self.pads {
            p.gain = gain_of(&self.slots, p.owner);
        }
        self.bass.gain = gain_of(&self.slots, self.bass.owner);
        self.lead.gain = gain_of(&self.slots, self.lead.owner);
        // Stingers end: once all their notes are out, mark them to fade
        // with their last ring.
        for s in &mut self.slots {
            if s.stinger && s.seq.as_ref().is_some_and(|q| q.done) && s.target > 0.0 {
                s.target = 0.0;
                s.fade = 2.5;
            }
        }
    }

    fn trigger(&mut self, e: Ev, gen: u32, step_samples: f64, at: f64) {
        let owner = Owner { gen, layer: e.layer };
        let sr = self.sr;
        // A new voice's gain is set now, not at the next block.
        let g = self
            .slots
            .iter()
            .find(|s| s.gen == gen)
            .map_or(0.0, |s| s.gain * s.layers[usize::from(e.layer).min(LAYERS - 1)]);
        let hold = (f64::from(e.len) * step_samples) as u32;
        let vel = (e.vel * self.rng.vary(0.06)).clamp(0.05, 1.0);
        match e.inst {
            Inst::Guitar | Inst::Mute => {
                // Strings struck on one step are strummed 9 ms apart.
                if self.strum.0 == gen && (self.strum.1 - at).abs() < 1.0 {
                    self.strum.2 += 1;
                } else {
                    self.strum = (gen, at, 0);
                }
                let delay = (self.strum.2 as f32 * 0.009 * sr) as u32;
                let slot = self.free_pluck();
                let p = &mut self.plucks[slot];
                p.start(semis(E2, e.note), vel, hold, e.inst == Inst::Mute, delay, sr, &mut self.rng);
                p.owner = owner;
                p.gain = g;
            }
            Inst::Bass => {
                self.bass.target = semis(E1, e.note);
                if self.bass.env < 0.01 {
                    self.bass.f = self.bass.target;
                }
                self.bass.vel = vel;
                self.bass.hold = hold.saturating_sub((0.01 * sr) as u32).max(1);
                self.bass.owner = owner;
                self.bass.gain = g;
            }
            Inst::Lead => {
                self.lead.start(semis(E4, e.note), vel, hold.saturating_sub((0.02 * sr) as u32).max(1), e.len, sr);
                self.lead.owner = owner;
                self.lead.gain = g;
            }
            Inst::Drone => {
                let k = self.pads.iter().position(|p| !p.active).unwrap_or(0);
                let p = &mut self.pads[k];
                p.f = semis(E1, e.note);
                p.vel = vel;
                p.hold = hold;
                p.env = if p.active { p.env } else { 0.0 };
                p.active = true;
                p.owner = owner;
                p.gain = g;
                for f in &mut p.filt {
                    f.set_sr(380.0, 0.8, sr);
                }
            }
            _ => {
                let Some(drum) = Drum::of(e.inst) else { return };
                let d = DRUMS.iter().position(|x| *x == drum).unwrap_or(0);
                let k = self.hits.iter().position(|h| !h.active).unwrap_or_else(|| {
                    // Steal the one furthest along.
                    let mut best = 0;
                    for (i, h) in self.hits.iter().enumerate() {
                        if h.pos > self.hits[best].pos {
                            best = i;
                        }
                    }
                    best
                });
                let take = (self.rng.unit() * TAKES as f32) as usize % TAKES;
                self.hits[k] = Hit {
                    drum: d,
                    take,
                    pos: 0.0,
                    rate: 48_000.0 / sr * self.rng.vary(0.02),
                    vel,
                    active: true,
                    owner,
                    gain: g,
                };
            }
        }
    }

    fn free_pluck(&self) -> usize {
        if let Some(i) = self.plucks.iter().position(|p| !p.active) {
            return i;
        }
        let mut best = 0;
        for (i, p) in self.plucks.iter().enumerate() {
            if p.age > self.plucks[best].age {
                best = i;
            }
        }
        best
    }

    fn next_pair(&mut self) -> (f32, f32) {
        let sr = self.sr;
        // ── Sequencers ────────────────────────────────────────────────
        let mut due: Vec<(Ev, u32, f64, f64)> = Vec::new();
        for s in &mut self.slots {
            let gen = s.gen;
            let quiet = s.target <= 0.0;
            let Some(q) = &mut s.seq else { continue };
            if q.done {
                continue;
            }
            while let Some(e) = q.song.events.get(q.next) {
                let t = q.time_of(e);
                if t > q.t {
                    break;
                }
                // A layer that cannot be heard does not play.
                let audible = s.layers[usize::from(e.layer).min(LAYERS - 1)] > 0.002 || s.track != MusicTrack::Battle;
                if !quiet && audible {
                    due.push((*e, gen, q.step_samples, t));
                }
                q.next += 1;
            }
            q.t += 1.0;
            if q.t >= q.length() {
                if q.song.looping {
                    q.t -= q.length();
                    q.next = 0;
                } else {
                    q.done = true;
                }
            }
        }
        for (e, gen, step, t) in due {
            self.trigger(e, gen, step, t);
        }
        self.clock = self.clock.wrapping_add(1);
        let clock = self.clock;

        // ── Voices ────────────────────────────────────────────────────
        let mut guitar = 0.0;
        for p in &mut self.plucks {
            if p.active {
                guitar += p.tick() * p.gain;
            }
        }
        // Amp grit, then a touch of bridge-pickup brightness.
        let guitar = fast_tanh(guitar * 1.6) * 0.55;
        let bass = if self.bass.env > 1e-5 || self.bass.hold > 0 { self.bass.tick(sr, clock) * self.bass.gain } else { 0.0 };
        let lead = if self.lead.env > 1e-5 || self.lead.hold > 0 { self.lead.tick(sr, clock, &mut self.rng) * self.lead.gain } else { 0.0 };
        let (mut pl, mut pr) = (0.0, 0.0);
        for p in &mut self.pads {
            if p.active {
                let (a, b) = p.tick(sr);
                pl += a * p.gain;
                pr += b * p.gain;
            }
        }
        let (mut dl, mut dr, mut dsend) = (0.0, 0.0, 0.0);
        for h in &mut self.hits {
            if !h.active {
                continue;
            }
            let buf = &self.kit[h.drum][h.take];
            let i = h.pos as usize;
            if i + 1 >= buf.len() {
                h.active = false;
                continue;
            }
            let f = h.pos - i as f32;
            let x = (buf[i] + (buf[i + 1] - buf[i]) * f) * h.vel * h.gain;
            h.pos += h.rate;
            let (gl, gr, send) = self.kit_place[h.drum];
            dl += x * gl;
            dr += x * gr;
            dsend += x * send;
        }

        // ── Mix ───────────────────────────────────────────────────────
        let [gl, gr, ll, lr] = self.pans;
        let send = guitar * 0.45 + lead * 0.3 + (pl + pr) * 0.1 + dsend;
        let wet = [self.springs[0].tick(send), self.springs[1].tick(send)];
        let mut out = [
            guitar * gl + bass * 0.75 + lead * ll + pl + dl + wet[0] * 0.5,
            guitar * gr + bass * 0.75 + lead * lr + pr + dr + wet[1] * 0.5,
        ];
        let g = if self.gain.is_finite() { self.gain.clamp(0.0, 4.0) } else { 0.0 };
        for (c, o) in out.iter_mut().enumerate() {
            let x = *o * 0.6 * g;
            let (xi, yo) = &mut self.dc[c];
            let y = x - *xi + 0.999 * *yo;
            *xi = x;
            *yo = y;
            *o = crate::dsp::soft_clip(y) * 0.97;
        }
        (out[0], out[1])
    }
}

/// Renders `seconds` of a track at a fixed intensity (preview and tests).
#[must_use]
pub fn render_track(track: MusicTrack, intensity: f32, seconds: f32) -> [Vec<f32>; 2] {
    render_with(seconds, |m, _| m.set(track, intensity))
}

/// Renders a stinger alone, with its ring-out.
#[must_use]
pub fn render_stinger(s: Stinger) -> [Vec<f32>; 2] {
    let secs = Music::stinger_seconds(s) + 2.0;
    let mut started = false;
    render_with(secs, |m, _| {
        if !started {
            m.stinger(s);
            started = true;
        }
    })
}

/// Runs a fresh 48 kHz band for `seconds`, calling `steer(music, time)`
/// every 512 samples.
pub fn render_with(seconds: f32, mut steer: impl FnMut(&mut Music, f32)) -> [Vec<f32>; 2] {
    let mut m = Music::new(1, SR);
    let n = (seconds.max(0.0) * SR) as usize;
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    let mut i = 0;
    while i < n {
        steer(&mut m, i as f32 / SR);
        let end = (i + 512).min(n);
        m.render(&mut l[i..end], &mut r[i..end]);
        i = end;
    }
    [l, r]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guitar is in tune: a plucked A2 repeats every 1/110 s.
    #[test]
    fn a_pluck_is_in_tune() {
        for f in [55.0f32, 82.407, 110.0, 220.0] {
            let mut p = Pluck::new();
            let mut rng = Rng::new(1);
            p.start(f, 0.8, 48_000, false, 0, SR, &mut rng);
            let x: Vec<f32> = (0..24_000).map(|_| p.tick()).collect();
            // Autocorrelation peak near the expected period.
            let want = SR / f;
            let lag = |l: usize| -> f32 { x[4000..20_000].iter().zip(&x[4000 + l..]).map(|(a, b)| a * b).sum() };
            let lo = (want * 0.9) as usize;
            let hi = (want * 1.1) as usize;
            let best = (lo..hi).max_by(|a, b| lag(*a).total_cmp(&lag(*b))).unwrap();
            let err = (best as f32 - want).abs() / want;
            assert!(err < 0.01, "{f} Hz plays at period {best}, want {want:.1}");
        }
    }
}
