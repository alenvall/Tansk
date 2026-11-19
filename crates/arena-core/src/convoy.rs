//! Convoy: team PvP over a fuel tanker truck.
//!
//! Round 1: team 0 (Rustbacks) escorts the truck along the map's route
//! ([`crate::map::Map::convoy_route`], following the road), team 1 (Dust
//! Saints) must destroy it. Round 2 swaps sides on the same route. A round
//! ends when the truck is delivered (the escorts score), destroyed, or the
//! round clock runs out (both count as a stop at the distance reached).
//!
//! The match: a team that delivered beats one that didn't; if both delivered
//! the faster delivery wins; if neither did, the truck that got further wins
//! (a draw within a metre or half a second). The end is `Phase::TeamWon` and
//! `Event::TeamMatchEnded`, like Team Brawl.
//!
//! The truck is a player slot (`VehicleKind::FuelTanker`, no crew, no brain:
//! the sim drives it) on the escort team, so every weapon, hit box, ram and
//! kill-feed rule just works. It rolls on its own, stops when attackers are
//! close and no escort is, and escorts standing next to it patch it up.

use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::events::Event;
use crate::map::Map;
use crate::parts::VehicleKind;
use crate::sim::{Phase, Player, Sim};
use crate::tank::{HullPose, Tank};
use crate::tuning::Tuning;
use crate::{Input, PlayerId};

/// Where a Convoy round is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConvoyPhase {
    /// Both sides set up; the truck waits (`ConvoySnap::ticks_left` counts down).
    Setup,
    /// The truck is on the road (moving or stopped).
    Rolling,
    /// Between the rounds.
    RoundOver,
    /// The match is decided (see `Phase::TeamWon`).
    Done,
}

/// One finished round.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConvoyRound {
    pub escort_team: u8,
    pub delivered: bool,
    /// Metres along the route the truck got.
    pub distance: f32,
    /// Seconds from rolling to the end of the round.
    pub seconds: f32,
}

/// The Convoy state for the client: the truck, the round and the score line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConvoySnap {
    /// The truck's player id (also in `Snapshot::tanks`, `vehicle == FuelTanker`).
    pub truck: PlayerId,
    pub pos: [f32; 3],
    pub alive: bool,
    pub health: u16,
    pub max_health: u16,
    /// Distance along the route, 0..1 (and in metres, of `length`).
    pub progress: f32,
    pub distance: f32,
    pub length: f32,
    /// 1 or 2.
    pub round: u8,
    /// The team escorting this round (the other attacks).
    pub side: u8,
    pub phase: ConvoyPhase,
    /// Ticks left in the setup countdown, the round clock or the intermission.
    pub ticks_left: u32,
    pub moving: bool,
    /// Attackers near and no escort: the truck is held.
    pub contested: bool,
    /// Escorts are patching it this tick.
    pub repairing: bool,
    /// Finished rounds, in order.
    pub rounds: Vec<ConvoyRound>,
}

/// What a bot needs to know to escort or hunt the truck.
#[derive(Clone, Copy, Debug)]
pub struct ConvoyInfo {
    pub truck: PlayerId,
    pub alive: bool,
    pub pos: Vec3,
    pub escort_team: u8,
    pub health_frac: f32,
    /// A point on the route `ahead` m in front of the truck, and the route direction there.
    pub ahead: Vec3,
    pub dir: Vec2,
    pub phase: ConvoyPhase,
}

#[derive(Clone, Debug)]
pub struct ConvoyState {
    /// The drivable route (the map's control points joined on the nav grid) and
    /// the distance along it at each point.
    pub path: Vec<Vec3>,
    pub cum: Vec<f32>,
    pub length: f32,
    pub truck: PlayerId,
    pub round: u8,
    pub escort_team: u8,
    pub phase: ConvoyPhase,
    /// Countdown of the setup or the intermission, s.
    pub timer: f32,
    /// Seconds the truck has been rolling this round.
    pub clock: f32,
    /// Metres along the route (best so far this round).
    pub progress: f32,
    seg: usize,
    pub moving: bool,
    pub contested: bool,
    pub repairing: bool,
    halfway: bool,
    pub results: Vec<ConvoyRound>,
    /// Set when the truck dies, handled by `step_convoy`.
    destroyed: bool,
    since_switch: f32,
    stuck: f32,
    reverse: f32,
    stall: f32,
    stall_mark: f32,
    /// Statistics: how often the truck was held this match.
    pub stops: u32,
}

impl ConvoyState {
    /// Point and direction `d` metres along the route.
    pub fn at(&self, d: f32) -> (Vec3, Vec2) {
        let n = self.path.len();
        if n < 2 {
            return (self.path.first().copied().unwrap_or(Vec3::ZERO), Vec2::X);
        }
        let d = d.clamp(0.0, self.length);
        let k = match self.cum.binary_search_by(|c| c.total_cmp(&d)) {
            Ok(k) => k.min(n - 2),
            Err(k) => k.saturating_sub(1).min(n - 2),
        };
        let (a, b) = (self.path[k], self.path[k + 1]);
        let seg = (self.cum[k + 1] - self.cum[k]).max(1e-3);
        let f = ((d - self.cum[k]) / seg).clamp(0.0, 1.0);
        let dir = Vec2::new(b.x - a.x, b.z - a.z).normalize_or(Vec2::X);
        (a + (b - a) * f, dir)
    }
}

fn yaw_toward(d: Vec2) -> f32 {
    (-d.y).atan2(d.x)
}

fn wrap(a: f32) -> f32 {
    let mut a = a % std::f32::consts::TAU;
    if a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    if a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

impl Sim {
    /// A Convoy match on `map`. Add crews with `add_player` / `add_bot`: they go
    /// to the smaller team (the truck isn't counted), Rustbacks first.
    pub fn new_convoy(seed: u64, map: Map, tuning: Tuning) -> Sim {
        Sim::with_mode(seed, map, tuning, crate::raid::ModeKind::Convoy)
    }

    /// Offline Convoy practice: you and `per_team - 1` bot allies (Rustbacks,
    /// escorting first) against `per_team` bots.
    pub fn practice_convoy_on(id: crate::map::MapId, seed: u64, per_team: usize, difficulty: f32) -> (Sim, PlayerId) {
        let mut sim = Sim::new_convoy(seed, Map::by_id(id, seed), Tuning::default());
        let me = sim.add_player("You");
        for i in 0..(per_team * 2).saturating_sub(1) {
            let names = crate::bots::BOT_NAMES;
            sim.add_bot(names[i % names.len()], difficulty);
        }
        (sim, me)
    }

    /// Is this player the Convoy truck?
    pub fn is_convoy_truck(&self, id: PlayerId) -> bool {
        self.convoy.as_ref().is_some_and(|c| c.truck == id)
    }

    /// The drivable Convoy route (empty in other modes): the map's control
    /// points joined on the nav grid, for drawing the road ahead.
    pub fn convoy_path(&self) -> &[Vec3] {
        self.convoy.as_ref().map(|c| c.path.as_slice()).unwrap_or(&[])
    }

    pub(crate) fn init_convoy(&mut self) {
        let mut ctrl: Vec<Vec3> = self.map.convoy_route.clone();
        if ctrl.len() < 2 {
            // A map without a route: across the middle, west to east.
            let h = self.map.play_half - 30.0;
            ctrl = vec![Vec3::new(-h, 0.0, 0.0), Vec3::new(h, 0.0, 0.0)];
        }
        let flat = |v: Vec3| Vec2::new(v.x, v.z);
        let mut pts2 = vec![flat(ctrl[0])];
        for w in ctrl.windows(2) {
            let from = *pts2.last().unwrap();
            let leg = self.nav.path(from, flat(w[1]));
            pts2.extend(leg);
        }
        pts2.dedup_by(|a, b| a.distance(*b) < 0.5);
        let path: Vec<Vec3> = pts2.iter().map(|p| Vec3::new(p.x, self.map.height_at(p.x, p.y), p.y)).collect();
        let mut cum = vec![0.0];
        for w in path.windows(2) {
            let d = Vec2::new(w[1].x - w[0].x, w[1].z - w[0].z).length();
            cum.push(cum.last().unwrap() + d);
        }
        let length = *cum.last().unwrap();
        // The truck: a crewless player on the escort team.
        let id = (0..=255u8).find(|i| self.players.iter().all(|p| p.id != *i)).expect("a free slot for the truck");
        self.convoy = Some(ConvoyState {
            path,
            cum,
            length,
            truck: id,
            round: 1,
            escort_team: 0,
            phase: ConvoyPhase::Setup,
            timer: self.tuning.convoy.setup_time,
            clock: 0.0,
            progress: 0.0,
            seg: 0,
            moving: false,
            contested: false,
            repairing: false,
            halfway: false,
            results: Vec::new(),
            destroyed: false,
            since_switch: 0.0,
            stuck: 0.0,
            reverse: 0.0,
            stall: 0.0,
            stall_mark: 0.0,
            stops: 0,
        });
        let (pos, yaw) = self.truck_start();
        let t = self.tuning.clone();
        let mut tank = Tank::new_vehicle(&mut self.phys, &t, pos, yaw, VehicleKind::FuelTanker);
        tank.protect = 0.0;
        self.players.push(Player {
            id,
            name: "Fuel tanker".to_string(),
            input: Input::default(),
            tank,
            kills: 0,
            deaths: 0,
            brain: None,
            ram_cd: 0.0,
            team: Some(0),
            counts_as_human: false,
            loadout: crate::garage::Loadout::default(),
        });
        self.time_left = self.tuning.convoy.setup_time;
        self.events.push(Event::ConvoyRoundStarted { round: 1, escort_team: 0, truck: id });
    }

    fn truck_start(&self) -> (Vec3, f32) {
        let c = self.convoy.as_ref().unwrap();
        let (p, dir) = c.at(0.0);
        (Vec3::new(p.x, self.map.height_at(p.x, p.z) + 1.6, p.z), yaw_toward(dir))
    }

    /// Restart the Convoy from round 1 (`reset_match`): the truck goes back to the start.
    pub(crate) fn reset_convoy(&mut self) {
        let Some(c) = self.convoy.as_mut() else { return };
        c.round = 1;
        c.escort_team = 0;
        c.results.clear();
        c.stops = 0;
        let truck = c.truck;
        self.begin_round();
        if let Some(j) = self.idx(truck) {
            self.players[j].team = Some(0);
        }
    }

    /// Fresh round state (the caller sets round and side, then respawns everyone).
    fn begin_round(&mut self) {
        let setup = self.tuning.convoy.setup_time;
        let c = self.convoy.as_mut().unwrap();
        c.phase = ConvoyPhase::Setup;
        c.timer = setup;
        c.clock = 0.0;
        c.progress = 0.0;
        c.seg = 0;
        c.moving = false;
        c.contested = false;
        c.repairing = false;
        c.halfway = false;
        c.destroyed = false;
        c.since_switch = 0.0;
        c.stuck = 0.0;
        c.reverse = 0.0;
        c.stall = 0.0;
        c.stall_mark = 0.0;
        let (round, escort_team, truck) = (c.round, c.escort_team, c.truck);
        self.time_left = setup;
        self.events.push(Event::ConvoyRoundStarted { round, escort_team, truck });
    }

    /// Convoy spawns: the truck at the route start; escorts just behind the
    /// truck; attackers well ahead of it, off to the sides of the road.
    pub(crate) fn convoy_spawn(&mut self, except: Option<usize>) -> Option<(Vec3, f32)> {
        let c = self.convoy.as_ref()?;
        let ct = self.tuning.convoy.clone();
        let (truck, escort_team, progress, length) = (c.truck, c.escort_team, c.progress, c.length);
        if let Some(j) = except {
            if self.players[j].id == truck {
                return Some(self.truck_start());
            }
        }
        let team = match except {
            Some(j) => self.players[j].team,
            None => self.pending_team,
        }?;
        let escort = team == escort_team;
        let truck_pos = self.idx(truck).map(|j| self.phys.body(self.players[j].tank.body).translation());
        for attempt in 0..40 {
            let (along, lat) = if escort {
                let a = (progress - ct.escort_spawn_behind + self.rng.range(-8.0, 8.0)).max(0.0);
                let l = self.rng.range(5.0, ct.escort_spawn_side.max(6.0)) * if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                (a, l)
            } else {
                let lo = (progress + ct.attacker_spawn_ahead_min).min(length - 10.0);
                let hi = (progress + ct.attacker_spawn_ahead_max).min(length - 5.0).max(lo + 1.0);
                let a = self.rng.range(lo, hi);
                let l = self.rng.range(12.0, ct.attacker_spawn_side.max(13.0)) * if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                (a, l)
            };
            let c = self.convoy.as_ref().unwrap();
            let (p, dir) = c.at(along);
            let normal = Vec2::new(-dir.y, dir.x);
            let q = Vec2::new(p.x, p.z) + normal * lat * if attempt > 25 { 0.5 } else { 1.0 };
            let edge = self.map.play_half - 12.0;
            if q.x.abs() > edge || q.y.abs() > edge || self.map.blocked_xz(q, 5.0) || self.map.normal_at(q.x, q.y).y < 0.9 {
                continue;
            }
            if self.nav.is_blocked(q) {
                continue;
            }
            let want = if escort {
                yaw_toward(dir)
            } else {
                let tp = truck_pos.map(|t| Vec2::new(t.x, t.z)).unwrap_or(Vec2::new(p.x, p.z));
                yaw_toward(tp - q)
            };
            // Face open, drivable ground (within 75 degrees of the route or the
            // truck), so a crew that just drives off doesn't hit a bank or a stack.
            let clear = (0..=15).flat_map(|k| [1.0f32, -1.0].map(|s| want + s * k as f32 * 5f32.to_radians())).find(|y| self.map.heading_clear(q, *y, crate::map::SPAWN_CLEAR));
            let yaw = match clear {
                Some(y) => y,
                // Nothing open here: try another spot (the last tries take what they get).
                None if attempt < 34 => continue,
                None => want,
            };
            return Some((Vec3::new(q.x, self.map.height_at(q.x, q.y) + 1.3, q.y), yaw));
        }
        None
    }

    /// For bots: where the truck is and which side the asking crew is on.
    pub fn convoy_info(&self) -> Option<ConvoyInfo> {
        let c = self.convoy.as_ref()?;
        let j = self.idx(c.truck)?;
        let tank = &self.players[j].tank;
        let pos = self.phys.body(tank.body).translation();
        let (ahead, dir) = c.at(c.progress + 22.0);
        Some(ConvoyInfo {
            truck: c.truck,
            alive: tank.alive,
            pos,
            escort_team: c.escort_team,
            health_frac: tank.health / tank.stats.max_health.max(1.0),
            ahead,
            dir,
            phase: c.phase,
        })
    }

    /// The truck's controls: follow the route while rolling and not held.
    pub(crate) fn convoy_drive(&mut self, dt: f32) {
        let Some(c) = self.convoy.as_ref() else { return };
        let Some(j) = self.idx(c.truck) else { return };
        if !self.players[j].tank.alive {
            return;
        }
        let hp = HullPose::of(&self.phys, self.players[j].tank.body);
        let fwd = hp.rot * Vec3::X;
        let mut input = Input { aim_point: hp.pos + fwd * 30.0, ..Input::default() };
        let go = c.phase == ConvoyPhase::Rolling && c.moving;
        if go {
            let speed = self.phys.body(self.players[j].tank.body).linvel().length();
            let (target, _) = c.at(c.progress + 11.0);
            let to = Vec2::new(target.x - hp.pos.x, target.z - hp.pos.z);
            let hull_yaw = (-fwd.z).atan2(fwd.x);
            let err = wrap(yaw_toward(to) - hull_yaw);
            input.steer = (-err * 2.0).clamp(-1.0, 1.0);
            input.throttle = if err.abs() > 1.0 { 0.15 } else { 1.0 - 0.5 * err.abs() };
            // Ease off behind a tank parked right in front.
            for p in &self.players {
                if p.id == c.truck || !p.tank.alive {
                    continue;
                }
                let rel = self.phys.body(p.tank.body).translation() - hp.pos;
                if rel.dot(fwd) > 0.0 && rel.dot(fwd) < 9.0 && rel.dot(hp.rot * Vec3::Z).abs() < 3.5 {
                    input.throttle = input.throttle.min(0.3);
                }
            }
            let c = self.convoy.as_mut().unwrap();
            if c.reverse > 0.0 {
                c.reverse -= dt;
                input.throttle = -0.8;
                input.steer = -input.steer;
            } else if input.throttle > 0.3 && speed < 0.4 {
                c.stuck += dt;
                if c.stuck > 2.5 {
                    c.stuck = 0.0;
                    c.reverse = 1.5;
                }
            } else {
                c.stuck = (c.stuck - dt).max(0.0);
            }
        }
        self.players[j].input = input;
    }

    /// The tanker's armour: everything does `truck_armour` of its damage, MG
    /// bullets only `truck_mg_mult` of that (they plink off the tank).
    pub(crate) fn truck_armour(&self, j: usize, w: crate::events::Weapon) -> f32 {
        if self.players[j].tank.vehicle != VehicleKind::FuelTanker {
            return 1.0;
        }
        let ct = &self.tuning.convoy;
        ct.truck_armour * if w == crate::events::Weapon::Alt(crate::parts::AltKind::Mg) { ct.truck_mg_mult } else { 1.0 }
    }

    /// The truck died (from `kill`): no respawn; the round ends this tick.

    pub(crate) fn on_convoy_death(&mut self, j: usize, killer: Option<PlayerId>, pos: Vec3) {
        let Some(c) = self.convoy.as_mut() else { return };
        if self.players[j].id != c.truck {
            return;
        }
        self.players[j].tank.respawn_in = f32::INFINITY;
        if c.phase != ConvoyPhase::Rolling && c.phase != ConvoyPhase::Setup {
            return;
        }
        c.destroyed = true;
        let (truck, distance, progress) = (c.truck, c.progress, c.progress / c.length.max(1.0));
        self.events.push(Event::ConvoyDestroyed { truck, killer, pos, distance, progress });
    }

    /// Per tick: the round clock, the truck's progress, stops, repairs, round ends.
    pub(crate) fn step_convoy(&mut self, dt: f32) {
        let Some(c) = self.convoy.as_ref() else { return };
        if self.phase != Phase::Playing {
            return;
        }
        let ct = self.tuning.convoy.clone();
        let truck = c.truck;
        let Some(tj) = self.idx(truck) else { return };
        match c.phase {
            ConvoyPhase::Done => return,
            ConvoyPhase::RoundOver => {
                let c = self.convoy.as_mut().unwrap();
                c.timer -= dt;
                self.time_left = c.timer.max(0.0);
                if c.timer <= 0.0 {
                    self.next_round();
                }
                return;
            }
            ConvoyPhase::Setup => {
                let c = self.convoy.as_mut().unwrap();
                c.timer -= dt;
                self.time_left = c.timer.max(0.0) + ct.round_time;
                if c.destroyed {
                    self.end_round(false);
                    return;
                }
                if c.timer <= 0.0 {
                    c.phase = ConvoyPhase::Rolling;
                    c.moving = true;
                    c.since_switch = 0.0;
                    let pos = self.phys.body(self.players[tj].tank.body).translation();
                    self.events.push(Event::ConvoyMoving { truck, pos });
                }
                return;
            }
            ConvoyPhase::Rolling => {}
        }
        if self.convoy.as_ref().unwrap().destroyed {
            self.end_round(false);
            return;
        }
        let tank_body = self.players[tj].tank.body;
        let pos = self.phys.body(tank_body).translation();
        let escort_team = self.convoy.as_ref().unwrap().escort_team;
        // Who's around the truck.
        let mut enemies = false;
        let mut escorts = false;
        let mut menders = 0u32;
        for p in &self.players {
            if p.id == truck || !p.tank.alive {
                continue;
            }
            let d = self.phys.body(p.tank.body).translation().distance(pos);
            if p.team == Some(escort_team) {
                escorts |= d < ct.escort_radius;
                if d < ct.repair_radius {
                    menders += 1;
                }
            } else if p.team.is_some() {
                enemies |= d < ct.stop_radius;
            }
        }
        // Repairs: escorts next to the truck patch it (not while it's being hit).
        let tank = &mut self.players[tj].tank;
        let repairing = menders > 0 && tank.since_damage > ct.repair_delay && tank.health < tank.stats.max_health;
        if repairing {
            let n = menders.min(ct.repair_max_crews) as f32;
            tank.health = (tank.health + ct.repair_hps * n * dt).min(tank.stats.max_health);
        }
        // Progress along the route (the best projection near the current leg).
        let c = self.convoy.as_mut().unwrap();
        c.repairing = repairing;
        c.clock += dt;
        c.since_switch += dt;
        let n = c.path.len();
        let p2 = Vec2::new(pos.x, pos.z);
        let mut best: Option<(f32, f32, usize)> = None;
        for k in c.seg.saturating_sub(1)..(c.seg + 4).min(n.saturating_sub(1)) {
            let a = Vec2::new(c.path[k].x, c.path[k].z);
            let b = Vec2::new(c.path[k + 1].x, c.path[k + 1].z);
            let ab = b - a;
            let f = ((p2 - a).dot(ab) / ab.length_squared().max(1e-4)).clamp(0.0, 1.0);
            let q = a + ab * f;
            let off = q.distance(p2);
            let along = c.cum[k] + f * ab.length();
            if best.is_none_or(|x| off < x.0) {
                best = Some((off, along, k));
            }
        }
        if let Some((off, along, k)) = best {
            if off < 18.0 && along > c.progress {
                c.progress = along.min(c.length);
                c.seg = k;
            }
        }
        // Held or rolling.
        let contested = enemies && !escorts;
        c.contested = contested;
        if c.moving == contested && c.since_switch > 0.75 {
            c.moving = !contested;
            c.since_switch = 0.0;
            if contested {
                c.stops += 1;
                self.events.push(Event::ConvoyStopped { truck, pos });
            } else {
                self.events.push(Event::ConvoyMoving { truck, pos });
            }
        }
        // A truck that's meant to roll but makes no headway for long gets a nudge up the road.
        if c.moving {
            if c.progress > c.stall_mark + 1.0 {
                c.stall_mark = c.progress;
                c.stall = 0.0;
            } else {
                c.stall += dt;
            }
        } else {
            c.stall = 0.0;
            c.stall_mark = c.progress;
        }
        let nudge = c.stall > 20.0;
        if nudge {
            c.stall = 0.0;
            let (p, dir) = c.at(c.progress + 8.0);
            let body = self.phys.body_mut(tank_body);
            body.set_position(crate::physics::pose(Vec3::new(p.x, p.y + 1.8, p.z), Quat::from_rotation_y(yaw_toward(dir))), true);
            body.set_linvel(Vec3::ZERO, true);
            body.set_angvel(Vec3::ZERO, true);
        }
        let c = self.convoy.as_mut().unwrap();
        if !c.halfway && c.progress >= c.length * 0.5 {
            c.halfway = true;
            self.events.push(Event::ConvoyHalfway { truck, escort_team });
        }
        let end = *c.path.last().unwrap();
        let arrived = c.length - c.progress < ct.arrive_distance || Vec2::new(end.x - pos.x, end.z - pos.z).length() < ct.arrive_distance;
        self.time_left = (ct.round_time - c.clock).max(0.0);
        if arrived {
            c.progress = c.length;
            let seconds = c.clock;
            self.events.push(Event::ConvoyDelivered { truck, escort_team, seconds });
            self.end_round(true);
        } else if c.clock >= ct.round_time {
            self.end_round(false);
        }
    }

    fn end_round(&mut self, delivered: bool) {
        let intermission = self.tuning.convoy.intermission;
        let c = self.convoy.as_mut().unwrap();
        let r = ConvoyRound { escort_team: c.escort_team, delivered, distance: c.progress, seconds: c.clock };
        c.results.push(r);
        c.moving = false;
        c.contested = false;
        c.repairing = false;
        let round = c.round;
        self.events.push(Event::ConvoyRoundEnded { round, escort_team: r.escort_team, delivered, distance: r.distance, seconds: r.seconds });
        if c.results.len() >= 2 {
            c.phase = ConvoyPhase::Done;
            let team = convoy_winner(&c.results);
            self.time_left = 0.0;
            self.phase = Phase::TeamWon { team };
            self.events.push(Event::TeamMatchEnded { team });
        } else {
            c.phase = ConvoyPhase::RoundOver;
            c.timer = intermission;
            self.time_left = intermission;
        }
    }

    /// Round 2: swap sides, the truck back at the start, everyone respawned for their new role.
    fn next_round(&mut self) {
        let c = self.convoy.as_mut().unwrap();
        c.round += 1;
        c.escort_team = 1 - c.escort_team;
        let (truck, side) = (c.truck, c.escort_team);
        self.begin_round();
        if let Some(j) = self.idx(truck) {
            self.players[j].team = Some(side);
        }
        self.projectiles.clear();
        // The truck first (the others spawn around it), then the crews.
        let order: Vec<usize> = {
            let mut v: Vec<usize> = (0..self.players.len()).collect();
            v.sort_by_key(|&j| self.players[j].id != truck);
            v
        };
        for j in order {
            self.respawn(j);
        }
    }
}

/// Who won a finished Convoy match (None: a draw). See the module docs.
pub fn convoy_winner(results: &[ConvoyRound]) -> Option<u8> {
    let (a, b) = match results {
        [a, b, ..] => (a, b),
        _ => return None,
    };
    match (a.delivered, b.delivered) {
        (true, false) => Some(a.escort_team),
        (false, true) => Some(b.escort_team),
        (true, true) => {
            if (a.seconds - b.seconds).abs() < 0.5 {
                None
            } else if a.seconds < b.seconds {
                Some(a.escort_team)
            } else {
                Some(b.escort_team)
            }
        }
        (false, false) => {
            if (a.distance - b.distance).abs() < 1.0 {
                None
            } else if a.distance > b.distance {
                Some(a.escort_team)
            } else {
                Some(b.escort_team)
            }
        }
    }
}

impl Sim {
    pub(crate) fn convoy_snap(&self) -> Option<ConvoySnap> {
        let c = self.convoy.as_ref()?;
        let j = self.idx(c.truck)?;
        let tank = &self.players[j].tank;
        let ticks = |s: f32| (s.max(0.0) * 60.0) as u32;
        let ct = &self.tuning.convoy;
        Some(ConvoySnap {
            truck: c.truck,
            pos: self.phys.body(tank.body).translation().to_array(),
            alive: tank.alive,
            health: tank.health.max(0.0).round() as u16,
            max_health: tank.stats.max_health.round() as u16,
            progress: c.progress / c.length.max(1.0),
            distance: c.progress,
            length: c.length,
            round: c.round,
            side: c.escort_team,
            phase: c.phase,
            ticks_left: match c.phase {
                ConvoyPhase::Setup | ConvoyPhase::RoundOver => ticks(c.timer),
                ConvoyPhase::Rolling => ticks(ct.round_time - c.clock),
                ConvoyPhase::Done => 0,
            },
            moving: c.moving,
            contested: c.contested,
            repairing: c.repairing,
            rounds: c.results.clone(),
        })
    }
}
