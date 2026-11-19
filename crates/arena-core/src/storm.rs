//! The sandstorm: about once per brawl match a storm rolls in for 45-60 s.
//! Sight shrinks (bots obey it too), shots scatter a little, and the wind
//! uncovers salvage (scrap sites respawn faster).

use serde::{Deserialize, Serialize};

use crate::events::Event;
use crate::sim::Sim;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StormPhase {
    #[default]
    Clear,
    /// Warning given; the storm arrives when `timer` runs out.
    Incoming,
    Raging,
    /// This match's storm has blown over.
    Over,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct StormState {
    pub phase: StormPhase,
    /// 0 (clear) ..= 1 (full storm).
    pub intensity: f32,
    /// Match time (s) the warning goes out.
    pub warn_at: f32,
    /// Seconds left in the current phase.
    pub timer: f32,
    pub duration: f32,
}

impl Sim {
    /// Pick this match's storm time (brawl modes; storms off when `latest` is 0).
    pub(crate) fn schedule_storm(&mut self) {
        let t = self.tuning.storm.clone();
        self.storm = StormState::default();
        // Last Crew Rolling has its own storm (the closing wall).
        if t.latest <= 0.0 || self.last_crew.is_some() {
            self.storm.phase = StormPhase::Over;
            return;
        }
        self.storm.warn_at = self.rng.range(t.earliest, t.latest.max(t.earliest));
    }

    /// Start the storm warning now (debug, tests, server admin).
    pub fn debug_storm(&mut self) {
        let elapsed = self.match_ticks as f32 / crate::TICK_HZ as f32;
        self.storm.phase = StormPhase::Clear;
        self.storm.warn_at = elapsed;
    }

    pub(crate) fn step_storm(&mut self, dt: f32) {
        if self.raid.is_some() {
            return;
        }
        let t = self.tuning.storm.clone();
        let elapsed = self.match_ticks as f32 / crate::TICK_HZ as f32;
        let s = &mut self.storm;
        match s.phase {
            StormPhase::Clear => {
                if elapsed >= s.warn_at && self.phase == crate::sim::Phase::Playing {
                    s.phase = StormPhase::Incoming;
                    s.timer = t.warning;
                    self.events.push(Event::StormIncoming { in_seconds: t.warning });
                }
            }
            StormPhase::Incoming => {
                s.timer -= dt;
                if s.timer <= 0.0 {
                    s.phase = StormPhase::Raging;
                    s.duration = self.rng.range(t.duration_min, t.duration_max);
                    s.timer = s.duration;
                    let d = s.duration;
                    self.events.push(Event::StormStarted { duration: d });
                }
            }
            StormPhase::Raging => {
                s.timer -= dt;
                let since = s.duration - s.timer;
                s.intensity = (since / t.ramp).min(s.timer / t.ramp).clamp(0.0, 1.0);
                if s.timer <= 0.0 {
                    s.phase = StormPhase::Over;
                    s.intensity = 0.0;
                    self.events.push(Event::StormEnded);
                }
            }
            StormPhase::Over => {}
        }
    }
}
