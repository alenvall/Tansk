//! Last Crew Rolling: free-for-all, no respawns, a sandstorm wall closing in.
//!
//! The safe zone is a circle. It holds for a while, then the wall closes in
//! (linearly, centre and radius) to the next circle: a random spot inside the
//! current one, `factor` times its radius. Four closes, then the last squeeze
//! to nothing. Outside the wall a tank takes `damage_base + damage_per_minute *
//! minutes` per second (no attacker: the storm). Scrap respawns
//! `scrap_boost` times faster early so tanks grow fast; a wreck spills extra.
//! A dead crew is out for good (`Event::Eliminated` with its place) and
//! watches; the last tank rolling wins (`Event::LastCrew`, then
//! `Phase::Ended` / `Event::MatchEnded` like Scrap Brawl).

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::events::{Event, Weapon};
use crate::sim::{Phase, Sim};
use crate::PlayerId;

/// The sandstorm wall for the client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StormCircleSnap {
    /// The wall now (x, z) and its radius, m.
    pub center: [f32; 2],
    pub radius: f32,
    /// Where it closes to next (equal to the current circle once fully closed).
    pub next_center: [f32; 2],
    pub next_radius: f32,
    /// 1-based phase of the next (or current) close; `phases + 1` once it's all closed.
    pub phase: u8,
    /// Ticks until the wall starts moving (holding) or stops (shrinking).
    pub ticks_left: u32,
    /// Crews still rolling.
    pub alive: u8,
    /// The wall is moving right now.
    pub shrinking: bool,
    /// Damage per second outside the wall right now.
    pub dps: f32,
    /// How many closes the match has (the last is the squeeze).
    pub phases: u8,
}

#[derive(Clone, Debug)]
pub struct LastCrewState {
    /// The circle the current phase started from, and the one it closes to.
    pub from: (Vec2, f32),
    pub to: (Vec2, f32),
    /// The wall now.
    pub center: Vec2,
    pub radius: f32,
    /// 0-based index of the phase in progress (== phases when all closed).
    pub phase: usize,
    pub shrinking: bool,
    /// Seconds left of the hold or the close.
    pub timer: f32,
    /// Crews out, in order (first out first).
    pub out: Vec<PlayerId>,
    /// Statistics: every kill as (seconds in, metres inside the wall; negative
    /// = outside it) and storm deaths.
    pub kills: Vec<(f32, f32)>,
    pub storm_deaths: u32,
    pub winner: Option<PlayerId>,
}

impl LastCrewState {
    /// Signed distance from the wall: positive inside the circle.
    pub fn inside(&self, p: Vec3) -> f32 {
        self.radius - Vec2::new(p.x, p.z).distance(self.center)
    }

    /// The circle bots should make for: the next one once its close is
    /// announced or under way, else the current.
    pub fn safe(&self) -> (Vec2, f32) {
        if self.shrinking || self.timer < 15.0 {
            self.to
        } else {
            (self.center, self.radius)
        }
    }
}

impl Sim {
    /// A Last Crew Rolling match on `map`.
    pub fn new_last_crew(seed: u64, map: crate::map::Map, tuning: crate::tuning::Tuning) -> Sim {
        Sim::with_mode(seed, map, tuning, crate::raid::ModeKind::LastCrew)
    }

    /// Offline Last Crew Rolling: you and `bots` bots.
    pub fn practice_last_crew_on(id: crate::map::MapId, seed: u64, bots: usize, difficulty: f32) -> (Sim, PlayerId) {
        let mut sim = Sim::new_last_crew(seed, crate::map::Map::by_id(id, seed), crate::tuning::Tuning::default());
        let me = sim.add_player("You");
        for i in 0..bots {
            let names = crate::bots::BOT_NAMES;
            sim.add_bot(names[i % names.len()], difficulty);
        }
        (sim, me)
    }

    pub(crate) fn init_last_crew(&mut self) {
        let t = self.tuning.last_crew.clone();
        let r0 = self.map.play_half * t.start_radius;
        let c0 = Vec2::ZERO;
        let to = self.next_circle(c0, r0, t.factor.first().copied().unwrap_or(0.5));
        self.last_crew = Some(LastCrewState {
            from: (c0, r0),
            to,
            center: c0,
            radius: r0,
            phase: 0,
            shrinking: false,
            timer: t.hold.first().copied().unwrap_or(30.0),
            out: Vec::new(),
            kills: Vec::new(),
            storm_deaths: 0,
            winner: None,
        });
        // The wall is this mode's storm: no random sandstorm on top.
        self.storm = crate::storm::StormState { phase: crate::storm::StormPhase::Over, ..Default::default() };
        self.time_left = 0.0;
    }

    pub(crate) fn reset_last_crew(&mut self) {
        if self.last_crew.is_some() {
            self.init_last_crew();
        }
    }

    /// A random circle of `factor * r` inside (c, r), on open ground and inside the play area.
    fn next_circle(&mut self, c: Vec2, r: f32, factor: f32) -> (Vec2, f32) {
        let nr = r * factor;
        let edge = (self.map.play_half - nr * 0.6 - 10.0).max(0.0);
        let mut best = (c, f32::MIN);
        for _ in 0..24 {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let d = (r - nr).max(0.0) * self.rng.range(0.0, 1.0).sqrt() * 0.9;
            let mut p = c + Vec2::new(a.cos(), a.sin()) * d;
            p = p.clamp(Vec2::splat(-edge), Vec2::splat(edge));
            // Keep it inside the old circle after the clamp.
            if p.distance(c) > (r - nr).max(0.0) {
                p = c + (p - c).normalize_or_zero() * (r - nr).max(0.0);
            }
            // Prefer open, drivable ground for the middle (the last fight happens there).
            let open = !self.nav.is_blocked(p) && !self.map.blocked_xz(p, 6.0);
            let score = if open { 1.0 } else { 0.0 } + self.rng.range(0.0, 0.5);
            if score > best.1 {
                best = (p, score);
            }
        }
        (best.0, nr)
    }

    /// Crews still rolling (not out of the match).
    pub fn crews_alive(&self) -> usize {
        let Some(lc) = &self.last_crew else { return self.players.iter().filter(|p| p.tank.alive).count() };
        self.players.iter().filter(|p| !lc.out.contains(&p.id) && (p.tank.alive || p.tank.respawn_in.is_finite())).count()
    }

    /// For bots and the client: the wall now, and the circle to make for.
    pub fn storm_zone(&self) -> Option<((Vec2, f32), (Vec2, f32))> {
        let lc = self.last_crew.as_ref()?;
        Some(((lc.center, lc.radius), lc.safe()))
    }

    /// A crew joining after `late_join` waits out this match.
    pub(crate) fn last_crew_late_join(&mut self, j: usize) {
        let Some(lc) = &self.last_crew else { return };
        let elapsed = self.match_ticks as f32 / crate::TICK_HZ as f32;
        if elapsed < self.tuning.last_crew.late_join && lc.out.is_empty() {
            return;
        }
        let id = self.players[j].id;
        let tank = &mut self.players[j].tank;
        tank.alive = false;
        tank.respawn_in = f32::INFINITY;
        let b = tank.body;
        self.phys.body_mut(b).set_enabled(false);
        self.last_crew.as_mut().unwrap().out.push(id);
    }

    /// A crew died (from `kill`): out for good, its place called.
    pub(crate) fn on_last_crew_death(&mut self, j: usize, killer: Option<PlayerId>, pos: Vec3) {
        let Some(lc) = &self.last_crew else { return };
        let id = self.players[j].id;
        self.players[j].tank.respawn_in = f32::INFINITY;
        if lc.out.contains(&id) || self.phase != Phase::Playing {
            return;
        }
        let elapsed = self.match_ticks as f32 / crate::TICK_HZ as f32;
        let inside = lc.inside(pos);
        let lc = self.last_crew.as_mut().unwrap();
        if killer.is_some_and(|k| k != id) {
            lc.kills.push((elapsed, inside));
        } else {
            lc.storm_deaths += 1;
        }
        lc.out.push(id);
        let place = self.crews_alive() + 1;
        self.events.push(Event::Eliminated { player: id, place: place.min(255) as u8 });
    }

    /// Per tick: the wall's clock and movement, the storm's bite, the last crew.
    pub(crate) fn step_last_crew(&mut self, dt: f32) {
        if self.last_crew.is_none() || self.phase != Phase::Playing {
            return;
        }
        let t = self.tuning.last_crew.clone();
        let phases = t.hold.len().min(t.shrink.len()).min(t.factor.len());
        // The wall.
        let mut ev = Vec::new();
        let mut next: Option<(Vec2, f32, f32)> = None;
        {
            let lc = self.last_crew.as_mut().unwrap();
            if lc.phase < phases {
                lc.timer -= dt;
                if !lc.shrinking {
                    if lc.timer <= 0.0 {
                        lc.shrinking = true;
                        lc.timer = t.shrink[lc.phase];
                        let c = Vec3::new(lc.to.0.x, 0.0, lc.to.0.y);
                        ev.push(Event::CircleShrinking { phase: lc.phase as u8 + 1, seconds: t.shrink[lc.phase], center: c, radius: lc.to.1 });
                    }
                } else {
                    let f = (1.0 - lc.timer / t.shrink[lc.phase].max(0.1)).clamp(0.0, 1.0);
                    lc.center = lc.from.0.lerp(lc.to.0, f);
                    lc.radius = lc.from.1 + (lc.to.1 - lc.from.1) * f;
                    if lc.timer <= 0.0 {
                        lc.center = lc.to.0;
                        lc.radius = lc.to.1;
                        lc.shrinking = false;
                        ev.push(Event::CircleClosed { phase: lc.phase as u8 + 1 });
                        lc.phase += 1;
                        lc.from = (lc.center, lc.radius);
                        if lc.phase < phases {
                            lc.timer = t.hold[lc.phase];
                            next = Some((lc.center, lc.radius, t.factor[lc.phase]));
                        } else {
                            lc.timer = 0.0;
                            lc.to = lc.from;
                        }
                    }
                }
            }
        }
        if let Some((c, r, f)) = next {
            let to = self.next_circle(c, r, f);
            self.last_crew.as_mut().unwrap().to = to;
        }
        self.events.extend(ev);
        // The storm bites whoever is outside.
        let minutes = self.match_ticks as f32 / crate::TICK_HZ as f32 / 60.0;
        let dps = t.damage_base + t.damage_per_minute * minutes;
        let mut dead = Vec::new();
        for j in 0..self.players.len() {
            if !self.players[j].tank.alive {
                continue;
            }
            let pos = self.phys.body(self.players[j].tank.body).translation();
            if self.last_crew.as_ref().unwrap().inside(pos) >= 0.0 {
                continue;
            }
            let tank = &mut self.players[j].tank;
            tank.health -= dps * dt;
            tank.since_damage = 0.0;
            if tank.health <= 0.0 {
                dead.push(j);
            }
        }
        for j in dead {
            if self.players[j].tank.alive {
                // The storm takes them: nobody gets the kill.
                self.kill(j, None, Weapon::Storm);
            }
        }
        // The last tank rolling.
        let alive: Vec<PlayerId> = self.players.iter().filter(|p| p.tank.alive).map(|p| p.id).collect();
        let lc = self.last_crew.as_ref().unwrap();
        let total = self.players.len();
        if total >= 2 && alive.len() <= 1 {
            let winner = alive.first().copied().or_else(|| lc.out.last().copied());
            if let Some(w) = winner {
                self.events.push(Event::LastCrew { player: w });
            }
            self.last_crew.as_mut().unwrap().winner = winner;
            self.phase = Phase::Ended { winner };
            self.events.push(Event::MatchEnded { winner });
        }
    }

    pub(crate) fn last_crew_snap(&self) -> Option<StormCircleSnap> {
        let lc = self.last_crew.as_ref()?;
        let t = &self.tuning.last_crew;
        let phases = t.hold.len().min(t.shrink.len()).min(t.factor.len());
        let minutes = self.match_ticks as f32 / crate::TICK_HZ as f32 / 60.0;
        Some(StormCircleSnap {
            center: lc.center.to_array(),
            radius: lc.radius,
            next_center: lc.to.0.to_array(),
            next_radius: lc.to.1,
            phase: (lc.phase + 1).min(255) as u8,
            ticks_left: (lc.timer.max(0.0) * 60.0) as u32,
            alive: self.players.iter().filter(|p| p.tank.alive).count().min(255) as u8,
            shrinking: lc.shrinking,
            dps: t.damage_base + t.damage_per_minute * minutes,
            phases: phases as u8,
        })
    }
}
