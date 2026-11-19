//! Wellhead: team PvP over three derricks.
//!
//! Each map names three wells ([`crate::map::Map::wells`]): one on each team's
//! side and one in the middle. A well is taken by having more crews inside its
//! ring than the other side: the ring fills toward the bigger side (faster the
//! more it outnumbers them), an enemy well must be emptied before it fills for
//! you, and a ring with both sides in it is *contested*. A held well pumps
//! `pump_rate` fuel per second into its team's tank; holding all three pumps
//! `all_wells_mult` times as fast. First team to `target` fuel wins, or the
//! most fuel at `time_limit` (a draw on equal tanks). Kills only matter for
//! what they clear off the wells. The end is `Phase::TeamWon` and
//! `Event::TeamMatchEnded`, like Team Brawl.

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::events::Event;
use crate::sim::{Phase, Sim};

/// One derrick, for the client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WellSnap {
    pub pos: [f32; 3],
    /// The team that holds it, if any.
    pub owner: Option<u8>,
    /// Who the ring belongs to: -1 all Rustbacks (team 0) ..= 1 all Dust
    /// Saints (team 1); a well is held at either end and changes hands at 0.
    pub progress: f32,
    /// Crews of both sides in the ring.
    pub contested: bool,
    /// Crews in the ring per team (for the HUD).
    pub crews: [u8; 2],
}

/// The Wellhead state for the client: the wells and both fuel tanks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WellheadSnap {
    pub wells: Vec<WellSnap>,
    /// Fuel pumped per team (Rustbacks, Dust Saints).
    pub fuel: [u32; 2],
    /// The tank size: first to it wins.
    pub target: u32,
    /// The capture ring radius, m.
    pub radius: f32,
    /// The team holding all three (double pumping), if any.
    pub all_held: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct Well {
    pub pos: Vec3,
    pub owner: Option<u8>,
    /// -1 (team 0) ..= 1 (team 1).
    pub control: f32,
    pub contested: bool,
    pub crews: [u8; 2],
    /// Seconds since the last "contested" call.
    pub since_call: f32,
}

#[derive(Clone, Debug)]
pub struct WellheadState {
    pub wells: Vec<Well>,
    pub fuel: [f32; 2],
    pub all_held: Option<u8>,
    /// Statistics: the biggest fuel lead each team ever had, and how many
    /// times one team held all three.
    pub max_lead: [f32; 2],
    pub all_holds: u32,
    pub captures: u32,
}

/// What a bot needs to pick a well.
#[derive(Clone, Copy, Debug)]
pub struct WellInfo {
    pub pos: Vec3,
    pub owner: Option<u8>,
    pub control: f32,
    pub contested: bool,
    pub crews: [u8; 2],
}

impl Sim {
    /// A Wellhead match on `map`: crews go to the smaller team as they join.
    pub fn new_wellhead(seed: u64, map: crate::map::Map, tuning: crate::tuning::Tuning) -> Sim {
        Sim::with_mode(seed, map, tuning, crate::raid::ModeKind::Wellhead)
    }

    /// Offline Wellhead: you (a Rustback) and `per_team - 1` bot allies against `per_team` bots.
    pub fn practice_wellhead_on(id: crate::map::MapId, seed: u64, per_team: usize, difficulty: f32) -> (Sim, crate::PlayerId) {
        let mut sim = Sim::new_wellhead(seed, crate::map::Map::by_id(id, seed), crate::tuning::Tuning::default());
        let me = sim.add_player("You");
        for i in 0..(per_team * 2).saturating_sub(1) {
            let names = crate::bots::BOT_NAMES;
            sim.add_bot(names[i % names.len()], difficulty);
        }
        (sim, me)
    }

    pub(crate) fn init_wellhead(&mut self) {
        let mut sites: Vec<Vec3> = self.map.wells.iter().map(|w| w.pos).collect();
        if sites.len() < 3 {
            // A map without wells: three across the middle.
            let h = self.map.play_half * 0.55;
            sites = [-h, 0.0, h].iter().map(|x| Vec3::new(*x, self.map.height_at(*x, 0.0), 0.0)).collect();
        }
        self.wellhead = Some(WellheadState {
            wells: sites
                .into_iter()
                .map(|pos| Well { pos, owner: None, control: 0.0, contested: false, crews: [0; 2], since_call: 1e6 })
                .collect(),
            fuel: [0.0; 2],
            all_held: None,
            max_lead: [0.0; 2],
            all_holds: 0,
            captures: 0,
        });
        self.time_left = self.tuning.wellhead.time_limit;
    }

    /// Back to neutral wells and empty tanks (`reset_match`).
    pub(crate) fn reset_wellhead(&mut self) {
        if self.wellhead.is_some() {
            self.init_wellhead();
        }
    }

    /// Which side of the field a team spawns on: the side of its own well.
    /// Returns (origin, axis): team 0 spawns where `(p - origin) . axis < 0`.
    pub fn wellhead_split(&self) -> Option<(Vec2, Vec2)> {
        let w = self.wellhead.as_ref()?;
        if w.wells.len() < 3 {
            return None;
        }
        let f = |v: Vec3| Vec2::new(v.x, v.z);
        let axis = (f(w.wells[2].pos) - f(w.wells[0].pos)).normalize_or(Vec2::X);
        Some((f(w.wells[1].pos), axis))
    }

    /// For bots: every well.
    pub fn well_info(&self) -> Vec<WellInfo> {
        self.wellhead
            .as_ref()
            .map(|w| {
                w.wells
                    .iter()
                    .map(|x| WellInfo { pos: x.pos, owner: x.owner, control: x.control, contested: x.contested, crews: x.crews })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Per tick: who's in each ring, capture progress, pumping, the win.
    pub(crate) fn step_wellhead(&mut self, dt: f32) {
        if self.wellhead.is_none() || self.phase != Phase::Playing {
            return;
        }
        let wt = self.tuning.wellhead.clone();
        let elapsed = self.match_ticks as f32 / crate::TICK_HZ as f32;
        self.time_left = (wt.time_limit - elapsed).max(0.0);
        // Crews in each ring.
        let n = self.wellhead.as_ref().unwrap().wells.len();
        let mut counts = vec![[0u8; 2]; n];
        for p in &self.players {
            let Some(team) = p.team.filter(|t| *t < 2) else { continue };
            if !p.tank.alive {
                continue;
            }
            let c = self.phys.body(p.tank.body).translation();
            for (k, w) in self.wellhead.as_ref().unwrap().wells.iter().enumerate() {
                let d = Vec2::new(c.x - w.pos.x, c.z - w.pos.z).length();
                if d < wt.radius && (c.y - w.pos.y).abs() < 8.0 {
                    counts[k][team as usize] = counts[k][team as usize].saturating_add(1);
                }
            }
        }
        let mut events = Vec::new();
        let wh = self.wellhead.as_mut().unwrap();
        for (k, w) in wh.wells.iter_mut().enumerate() {
            let [a, b] = counts[k];
            w.crews = [a, b];
            w.since_call += dt;
            let contested = a > 0 && b > 0;
            if contested && !w.contested && w.since_call > wt.contested_call {
                w.since_call = 0.0;
                events.push(Event::WellContested { well: k as u8 });
            }
            w.contested = contested;
            let rate = 1.0 / wt.capture_time.max(0.1);
            let before = w.control;
            if a != b {
                // The bigger side pushes the ring its way.
                let diff = a.abs_diff(b).min(wt.max_crowd as u8) as f32;
                let speed = rate * (1.0 + wt.crowd_bonus * (diff - 1.0));
                let dir = if b > a { 1.0 } else { -1.0 };
                w.control = (w.control + dir * speed * dt).clamp(-1.0, 1.0);
            } else if a == 0 {
                // Empty: a held well heals back to its owner, a loose one drifts to neutral.
                let home = match w.owner {
                    Some(0) => -1.0,
                    Some(_) => 1.0,
                    None => 0.0,
                };
                let step = wt.decay * dt;
                w.control += (home - w.control).clamp(-step, step);
            }
            // Changing hands.
            if let Some(o) = w.owner {
                let lost = if o == 0 { w.control >= 0.0 && before < 0.0 } else { w.control <= 0.0 && before > 0.0 };
                if lost || (w.control == 0.0 && before != 0.0) {
                    w.owner = None;
                    events.push(Event::WellLost { well: k as u8, team: o });
                }
            }
            if w.owner.is_none() {
                if w.control <= -1.0 {
                    w.owner = Some(0);
                    wh.captures += 1;
                    events.push(Event::WellCaptured { well: k as u8, team: 0 });
                } else if w.control >= 1.0 {
                    w.owner = Some(1);
                    wh.captures += 1;
                    events.push(Event::WellCaptured { well: k as u8, team: 1 });
                }
            }
        }
        // Pumping.
        let held = |t: u8| wh.wells.iter().filter(|w| w.owner == Some(t)).count();
        let all = (0..2u8).find(|t| held(*t) == wh.wells.len() && !wh.wells.is_empty());
        if all.is_some() && all != wh.all_held {
            wh.all_holds += 1;
            events.push(Event::AllWellsHeld { team: all.unwrap() });
        }
        wh.all_held = all;
        for t in 0..2u8 {
            let mult = if all == Some(t) { wt.all_wells_mult } else { 1.0 };
            wh.fuel[t as usize] += held(t) as f32 * wt.pump_rate * mult * dt;
        }
        for t in 0..2 {
            wh.max_lead[t] = wh.max_lead[t].max(wh.fuel[t] - wh.fuel[1 - t]);
        }
        let fuel = wh.fuel;
        self.events.extend(events);
        // The whistle: a full tank, or the clock.
        let full = fuel.iter().any(|f| *f >= wt.target);
        if full || self.time_left <= 0.0 {
            let team = if (fuel[0] - fuel[1]).abs() < 0.5 {
                None
            } else if fuel[0] > fuel[1] {
                Some(0)
            } else {
                Some(1)
            };
            self.phase = Phase::TeamWon { team };
            self.events.push(Event::TeamMatchEnded { team });
        }
    }

    pub(crate) fn wellhead_snap(&self) -> Option<WellheadSnap> {
        let w = self.wellhead.as_ref()?;
        let wt = &self.tuning.wellhead;
        Some(WellheadSnap {
            wells: w
                .wells
                .iter()
                .map(|x| WellSnap { pos: x.pos.to_array(), owner: x.owner, progress: x.control, contested: x.contested, crews: x.crews })
                .collect(),
            fuel: [w.fuel[0].min(wt.target) as u32, w.fuel[1].min(wt.target) as u32],
            target: wt.target as u32,
            radius: wt.radius,
            all_held: w.all_held,
        })
    }
}
