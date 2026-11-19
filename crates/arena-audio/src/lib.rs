//! TANSK 2026 audio. Every sound is synthesized: no audio files.
//!
//! - [`sounds`]: the one-shots ([`SoundId`]), rendered to f32 buffers at
//!   48 kHz with seeded variation ([`render`]), pre-rendered into a [`Bank`].
//! - [`engine`]: the continuous tank engine voice ([`TankEngine`]), driven by
//!   rpm / load / boost / track speed and generated in real time.
//! - [`mixer`]: every engine panned and summed into one stereo bus.
//! - [`spatial`]: pan, level and dullness of a source from the listener.
//! - `web` (wasm32 only): Web Audio playback, [`Audio`].
//! - [`wav`] and [`analysis`]: the native preview and the tests.
//!
//! The approach is brick-racers' (`docs/design/audio.md` there): engines on
//! a pulse-and-pipe model; effects built like recordings of real objects
//! (struck modes, filtered noise, debris) rather than from oscillators.

pub mod analysis;
pub mod dsp;
pub mod engine;
pub mod layers;
pub mod mixer;
mod raid;
pub mod sounds;
pub mod spatial;
pub mod wav;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use dsp::SAMPLE_RATE;
pub use engine::{EngineInput, EngineKind, TankEngine};
pub use mixer::{EngineHandle, EngineMixer, MAX_ENGINES};
pub use sounds::{render, Rendered, SoundId, SoundInfo};
pub use spatial::{spatialize, Listener, Spatial};
#[cfg(target_arch = "wasm32")]
pub use web::Audio;

/// The seed of take `k` of a sound in the [`Bank`].
#[must_use]
pub const fn take_seed(k: usize) -> u64 {
    0x7A25_6B00 + k as u64 * 7919
}

/// Every take to pre-render, in order: take 0 of each sound (enough to play
/// anything), then take 1 of each that has one, and so on.
#[must_use]
pub fn schedule() -> Vec<(SoundId, usize)> {
    let most = SoundId::ALL.iter().map(|s| s.info().variants as usize).max().unwrap_or(1);
    (0..most)
        .flat_map(|k| SoundId::ALL.iter().filter(move |s| (s.info().variants as usize) > k).map(move |s| (*s, k)))
        .collect()
}

/// Pre-rendered takes of every sound, filled a take at a time so the work
/// can be spread over frames: first one take of every sound (enough to play
/// anything), then the rest.
#[derive(Clone, Debug, Default)]
pub struct Bank {
    takes: Vec<Vec<Rendered>>,
    next: usize,
}

impl Bank {
    #[must_use]
    pub fn new() -> Self {
        Self { takes: vec![Vec::new(); SoundId::ALL.len()], next: 0 }
    }

    /// Renders the next take; returns which, or `None` when complete.
    pub fn render_next(&mut self) -> Option<(SoundId, usize)> {
        let (id, k) = schedule().get(self.next).copied()?;
        self.next += 1;
        self.takes[id.index()].push(render(id, take_seed(k)));
        Some((id, k))
    }

    /// Renders until every sound has at least one take.
    pub fn render_first_takes(&mut self) {
        while self.next < SoundId::ALL.len() && self.render_next().is_some() {}
    }

    /// Renders everything left.
    pub fn render_all(&mut self) {
        while self.render_next().is_some() {}
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.next >= schedule().len()
    }

    #[must_use]
    pub fn takes(&self, id: SoundId) -> &[Rendered] {
        &self.takes[id.index()]
    }

    /// Total frames rendered so far.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.takes.iter().flatten().map(Rendered::frames).sum()
    }
}
