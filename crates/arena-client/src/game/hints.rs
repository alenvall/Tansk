//! Onboarding: short crew-chief tips in the HUD, each shown once per
//! browser and only when the situation calls for it (first spawn, the first
//! scrap in sight, the first branch offer...). The game says what it sees
//! with [`Hints::want`]; [`Hints::tick`] shows one tip at a time. Which tips
//! were seen is saved by the platform layer (web: localStorage).
//!
//! Tip text marks keys as `[W]`; the UI draws them as key caps. Each tip
//! has a keyboard and a gamepad wording.

use std::collections::{HashSet, VecDeque};

use crate::ui::Ui;

/// One tip: its save key, keyboard wording, gamepad wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tip {
    pub key: &'static str,
    pub keys: &'static str,
    pub pad: &'static str,
}

pub const DRIVE: Tip = Tip {
    key: "drive",
    keys: "[W][A][S][D] to drive \u{b7} [Mouse] to aim \u{b7} [Click] to fire",
    pad: "[LS] to drive \u{b7} [RS] to aim \u{b7} [RT] to fire",
};
pub const SCRAP: Tip = Tip {
    key: "scrap",
    keys: "Drive over scrap. Three of a kind bolts on an upgrade.",
    pad: "Drive over scrap. Three of a kind bolts on an upgrade.",
};
pub const BRANCH: Tip = Tip {
    key: "branch",
    keys: "Pick [1] [2] or [3] before the timer runs out",
    pad: "Pick with [\u{25c0}] [\u{25b2}] [\u{25b6}] before the timer runs out",
};
pub const SPECIAL: Tip = Tip {
    key: "special",
    keys: "Something special aboard: [F] to use it",
    pad: "Something special aboard: [Y] to use it",
};
pub const LOW_HEALTH: Tip = Tip {
    key: "lowhp",
    keys: "Hull's failing. Drive over a wrench to patch it up.",
    pad: "Hull's failing. Drive over a wrench to patch it up.",
};
pub const PART_LOST: Tip = Tip {
    key: "partlost",
    keys: "Your part fell off. Its scrap is on the ground: grab it back.",
    pad: "Your part fell off. Its scrap is on the ground: grab it back.",
};
pub const CONVOY: Tip = Tip {
    key: "convoy",
    keys: "Convoy: escorts keep the tanker rolling to the depot, the other crew stops it. Then you swap.",
    pad: "Convoy: escorts keep the tanker rolling to the depot, the other crew stops it. Then you swap.",
};
pub const WELLHEAD: Tip = Tip {
    key: "wellhead",
    keys: "Wellhead: park inside a derrick's ring to take it. Held wells pump fuel. Fill the tank first.",
    pad: "Wellhead: park inside a derrick's ring to take it. Held wells pump fuel. Fill the tank first.",
};
pub const LAST_CREW: Tip = Tip {
    key: "lastcrew",
    keys: "Last Crew Rolling: no respawns. Grab scrap early, stay inside the storm wall, be the last one rolling.",
    pad: "Last Crew Rolling: no respawns. Grab scrap early, stay inside the storm wall, be the last one rolling.",
};
pub const RAID: Tip = Tip {
    key: "raid",
    keys: "Raid: hold the outpost against the waves. Stay close and stick together.",
    pad: "Raid: hold the outpost against the waves. Stay close and stick together.",
};

/// How long a tip stays up, seconds.
const SHOW_SECS: f32 = 6.5;
/// A breather between tips.
const GAP_SECS: f32 = 0.8;

#[derive(Default)]
pub struct Hints {
    seen: HashSet<String>,
    queue: VecDeque<Tip>,
    showing: Option<(Tip, f32)>,
    gap: f32,
    /// Something new was seen: the platform should save [`Hints::saved`].
    pub dirty: bool,
}

impl Hints {
    /// Restores the seen set from [`Hints::saved`]'s format.
    pub fn load(&mut self, saved: &str) {
        self.seen = saved.split(',').map(str::trim).filter(|k| !k.is_empty()).map(String::from).collect();
    }

    /// The seen set, comma separated.
    pub fn saved(&self) -> String {
        let mut keys: Vec<&str> = self.seen.iter().map(String::as_str).collect();
        keys.sort_unstable();
        keys.join(",")
    }

    pub fn seen(&self, tip: Tip) -> bool {
        self.seen.contains(tip.key)
    }

    /// Queues `tip` unless it was seen, is up, or is already waiting.
    pub fn want(&mut self, tip: Tip) {
        if self.seen(tip) || self.queue.contains(&tip) || self.showing.is_some_and(|(t, _)| t == tip) {
            return;
        }
        self.queue.push_back(tip);
    }

    /// Like `want`, but it jumps the queue and replaces what is up (a tip
    /// that only makes sense right now, like the branch timer).
    pub fn want_now(&mut self, tip: Tip) {
        if self.seen(tip) {
            return;
        }
        self.queue.retain(|t| *t != tip);
        self.queue.push_front(tip);
        if self.showing.is_some_and(|(t, _)| t != tip) {
            self.showing = None;
            self.gap = 0.0;
        }
    }

    /// Takes back one tip that no longer applies (waiting or up); it can
    /// come again later.
    pub fn drop_tip(&mut self, tip: Tip, ui: &mut Ui) {
        self.queue.retain(|t| *t != tip);
        if self.showing.is_some_and(|(t, _)| t == tip) {
            self.showing = None;
            self.seen.remove(tip.key);
            self.dirty = true;
            ui.hide_hint();
        }
    }

    /// Drops waiting tips (leaving a match); what is up fades out.
    pub fn clear_queue(&mut self, ui: &mut Ui) {
        self.queue.clear();
        if self.showing.take().is_some() {
            ui.hide_hint();
        }
    }

    /// Shows the next tip when `can_show` (alive, not paused, not at the
    /// end screen); `pad` picks the gamepad wording.
    pub fn tick(&mut self, dt: f32, can_show: bool, pad: bool, ui: &mut Ui) {
        if let Some((_, left)) = &mut self.showing {
            *left -= dt;
            if *left <= 0.0 || !can_show {
                self.showing = None;
                self.gap = GAP_SECS;
                ui.hide_hint();
            }
            return;
        }
        self.gap -= dt;
        if !can_show || self.gap > 0.0 {
            return;
        }
        if let Some(tip) = self.queue.pop_front() {
            ui.show_hint(if pad { tip.pad } else { tip.keys });
            self.seen.insert(tip.key.to_string());
            self.dirty = true;
            self.showing = Some((tip, SHOW_SECS));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tips_show_once_and_round_trip() {
        let mut h = Hints::default();
        h.want(DRIVE);
        h.want(DRIVE);
        h.want(SCRAP);
        assert_eq!(h.queue.len(), 2);
        h.seen.insert(DRIVE.key.into());
        h.seen.insert(SCRAP.key.into());
        let saved = h.saved();
        let mut back = Hints::default();
        back.load(&saved);
        assert!(back.seen(DRIVE) && back.seen(SCRAP) && !back.seen(BRANCH));
        back.want(DRIVE);
        assert!(back.queue.is_empty());
        back.want(SCRAP);
        back.want_now(BRANCH);
        assert_eq!(back.queue.front(), Some(&BRANCH));
    }
}
