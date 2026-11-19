//! Every tank's engine summed into one stereo stream, each panned and
//! attenuated from where it is. Platform-free: the browser layer runs it on
//! the audio callback, and the tests and preview run it natively.

use crate::dsp::{one_pole_k, OnePole};
use crate::engine::{EngineInput, EngineKind, TankEngine};
use crate::spatial::{pan_gains, Spatial};

/// Most engines heard at once. One per visible tank; 8 crews plus bots.
pub const MAX_ENGINES: usize = 16;

/// A handle to one engine voice in an [`EngineMixer`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EngineHandle {
    slot: u16,
    generation: u16,
}

#[derive(Clone, Debug)]
struct Slot {
    generation: u16,
    kind: EngineKind,
    seed: u64,
    voice: Option<TankEngine>,
    /// Target and current (left, right) gains.
    target: (f32, f32),
    current: (f32, f32),
    cutoff: f32,
    lp: OnePole,
    alive: bool,
    used: bool,
}

/// The engine bus.
#[derive(Clone, Debug)]
pub struct EngineMixer {
    sr: f32,
    slots: Vec<Slot>,
    scratch: Vec<f32>,
    /// Master gain of the bus.
    pub gain: f32,
}

impl EngineMixer {
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self { sr: sample_rate, slots: Vec::new(), scratch: Vec::new(), gain: 1.0 }
    }

    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// Changes the output rate; running voices are rebuilt at the new rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (sample_rate - self.sr).abs() < 0.5 {
            return;
        }
        self.sr = sample_rate;
        for s in &mut self.slots {
            if s.used {
                let input = s.voice.as_ref().map(TankEngine::input);
                let mut v = TankEngine::new(s.kind, s.seed, sample_rate);
                if let Some(i) = input {
                    v.set_input(i);
                }
                s.voice = Some(v);
            }
        }
    }

    /// Starts an engine. `None` when [`MAX_ENGINES`] are already running.
    pub fn add(&mut self, kind: EngineKind, seed: u64) -> Option<EngineHandle> {
        let index = match self.slots.iter().position(|s| !s.used) {
            Some(i) => i,
            None if self.slots.len() < MAX_ENGINES => {
                self.slots.push(Slot {
                    generation: 0,
                    kind,
                    seed,
                    voice: None,
                    target: (0.0, 0.0),
                    current: (0.0, 0.0),
                    cutoff: 20_000.0,
                    lp: OnePole::default(),
                    alive: false,
                    used: false,
                });
                self.slots.len() - 1
            }
            None => return None,
        };
        let s = &mut self.slots[index];
        s.generation = s.generation.wrapping_add(1);
        s.kind = kind;
        s.seed = seed;
        s.voice = Some(TankEngine::new(kind, seed, self.sr));
        s.target = (0.0, 0.0);
        s.current = (0.0, 0.0);
        s.cutoff = 20_000.0;
        s.lp = OnePole::default();
        s.alive = true;
        s.used = true;
        Some(EngineHandle { slot: index as u16, generation: s.generation })
    }

    fn slot(&mut self, h: EngineHandle) -> Option<&mut Slot> {
        self.slots.get_mut(usize::from(h.slot)).filter(|s| s.used && s.alive && s.generation == h.generation)
    }

    /// Updates what an engine is doing and how it is heard. `gain` is the
    /// tank's own level (e.g. 1, or quieter for a far-off bot).
    pub fn set(&mut self, h: EngineHandle, input: EngineInput, heard: Spatial, gain: f32) {
        if let Some(s) = self.slot(h) {
            if let Some(v) = &mut s.voice {
                v.set_input(input);
            }
            let (l, r) = pan_gains(heard.pan);
            let g = (heard.gain * gain).clamp(0.0, 4.0);
            let g = if g.is_finite() { g } else { 0.0 };
            s.target = (l * g, r * g);
            s.cutoff = heard.cutoff_hz;
        }
    }

    /// Fades an engine out and frees it.
    pub fn remove(&mut self, h: EngineHandle) {
        if let Some(s) = self.slot(h) {
            s.alive = false;
            s.target = (0.0, 0.0);
        }
    }

    /// Engines still sounding (including ones fading out).
    #[must_use]
    pub fn active(&self) -> usize {
        self.slots.iter().filter(|s| s.used).count()
    }

    /// Writes (not adds) the next block of the bus into `left` and `right`.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        left[..n].fill(0.0);
        right[..n].fill(0.0);
        if n == 0 {
            return;
        }
        if self.scratch.len() < n {
            self.scratch.resize(n, 0.0);
        }
        let bus = if self.gain.is_finite() { self.gain.clamp(0.0, 4.0) } else { 0.0 };
        // Gains move at most this far per block, toward the target: ~30 ms.
        let follow = (n as f32 / (0.03 * self.sr)).min(1.0);
        for s in &mut self.slots {
            if !s.used {
                continue;
            }
            let Some(voice) = &mut s.voice else { continue };
            let from = s.current;
            let to = (from.0 + (s.target.0 - from.0) * follow, from.1 + (s.target.1 - from.1) * follow);
            s.current = to;
            if !s.alive && to.0 + to.1 < 1e-4 {
                s.used = false;
                s.voice = None;
                continue;
            }
            if from.0 + from.1 + to.0 + to.1 < 1e-5 {
                // Out of earshot: not rendered at all (it picks up where it
                // left off, smoothed, when it comes back).
                continue;
            }
            let buf = &mut self.scratch[..n];
            voice.render(buf);
            let k = one_pole_k(s.cutoff, self.sr);
            for i in 0..n {
                let x = i as f32 / n as f32;
                let y = s.lp.lp(buf[i], k);
                left[i] += y * (from.0 + (to.0 - from.0) * x);
                right[i] += y * (from.1 + (to.1 - from.1) * x);
            }
        }
        for i in 0..n {
            left[i] = crate::dsp::soft_clip(left[i] * bus);
            right[i] = crate::dsp::soft_clip(right[i] * bus);
        }
    }
}
