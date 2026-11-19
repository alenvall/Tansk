//! Spectacle hazards, from the wishlist ("a spring launches the tank", "a
//! fuel tanker that blows the whole street"). Fun beats balance.
//!
//! - **Spring pads** ([`ObstacleKind::SpringPad`]): drive over one fast along
//!   its arrow and it flings you in a high arc to its landing spot
//!   ([`Map::springs`]): over a wall, across the river bed, up onto a
//!   plateau. The flight is steered on its ballistic path (so it lands where
//!   the map says, whatever the tank's drag) until it hits something.
//! - **Fuel trailers** ([`ObstacleKind::FuelTrailer`]): shoot one up and it
//!   springs a leak and burns (sputtering fireballs, a few seconds' warning),
//!   then goes up in a huge, overpowered blast that throws tanks about and
//!   lights the other trailers nearby. It is towed back in after a while.
//!
//! - **Bank slides**: a tank wedged on ground too steep to drive (thrown onto
//!   a river cliff by a blast or a bad jump) slides off it after a moment;
//!   one driving into a cliff face slides along it instead of pinning.
//!
//! Hooks in `sim.rs`: [`Sim::init_spectacle`] after the world is built,
//! [`Sim::reset_spectacle`] on a match reset, [`Sim::step_spectacle`] each
//! tick. The client sees `Launched`, `Explosion`, `HazardExploded` and
//! `ObstacleDestroyed` events and the obstacle coming and going in `removed`.
//!
//! [`Map::springs`]: crate::map::Map::springs

use glam::{Vec2, Vec3};
use rapier3d::prelude::*;

use crate::events::{Event, Weapon};
use crate::map::ObstacleKind;
use crate::physics::{pose, static_groups, OBSTACLE_TAG};
use crate::sim::Sim;
use crate::tuning::GunStats;
use crate::PlayerId;

/// Speed along the pad's arrow that sets a spring off, m/s.
pub const SPRING_MIN_SPEED: f32 = 4.5;
/// A spring re-cocks this long after it fires, s.
pub const SPRING_RECHARGE: f32 = 2.0;
/// Horizontal speed of the throw, m/s (the flight time follows from the distance).
const SPRING_FLIGHT_SPEED: f32 = 22.0;
const GRAVITY: f32 = 9.81;
/// Share of the throw's speed a tank keeps on touchdown.
const LANDING_KEEP: f32 = 0.3;

/// Ground steeper than this (normal's up part) is a bank, not a road.
const SLIDE_NORMAL: f32 = 0.85;
/// Wedged on a bank this long, a tank gets slid off it, at this speed.
const SLIDE_AFTER: f32 = 1.5;
const SLIDE_SPEED: f32 = 5.0;
/// Ground ahead this steep (normal's up part) is a wall to slide along.
const WALL_NORMAL: f32 = 0.82;
/// Pressing into a wall this long, a tank slides along it, at this speed.
const WALL_AFTER: f32 = 0.3;
const WALL_SLIDE_SPEED: f32 = 6.0;

/// A fuel trailer's health; it starts to leak and burn below this share of it.
pub const TRAILER_HEALTH: f32 = 220.0;
pub const TRAILER_IGNITE: f32 = 0.65;
/// How long it burns before it blows, s.
pub const TRAILER_BURN: f32 = 3.2;
/// The blast: big, loud and unfair.
pub const TRAILER_BLAST_RADIUS: f32 = 26.0;
pub const TRAILER_BLAST_DAMAGE: f32 = 330.0;
pub const TRAILER_KNOCKBACK: f32 = 900_000.0;
/// A blown trailer is back after this long, s.
pub const TRAILER_RESPAWN: f32 = 90.0;

#[derive(Clone, Debug, Default)]
pub struct Spectacle {
    /// Per `Map::springs`: seconds until it can fire again.
    springs: Vec<f32>,
    flights: Vec<Flight>,
    trailers: Vec<Trailer>,
    /// Seconds each tank has sat wedged on ground too steep to drive.
    wedged: Vec<(PlayerId, f32)>,
}

/// A tank on its way through the air from a spring.
#[derive(Clone, Debug)]
struct Flight {
    player: PlayerId,
    v0: Vec3,
    t: f32,
    time: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TrailerState {
    Intact,
    Burning,
    Gone,
}

#[derive(Clone, Debug)]
struct Trailer {
    obstacle: usize,
    state: TrailerState,
    timer: f32,
    puff: f32,
}

impl Spectacle {
    /// Is this player in a spring's flight right now?
    pub fn flying(&self, player: PlayerId) -> bool {
        self.flights.iter().any(|f| f.player == player)
    }

    /// Fuel trailers burning now (obstacle indices).
    pub fn burning(&self) -> impl Iterator<Item = usize> + '_ {
        self.trailers.iter().filter(|t| t.state == TrailerState::Burning).map(|t| t.obstacle)
    }
}

impl Sim {
    /// Arms the springs and fills the trailers (after `init_world`).
    pub(crate) fn init_spectacle(&mut self) {
        self.spectacle = Spectacle { springs: vec![0.0; self.map.springs.len()], ..Default::default() };
        for (i, o) in self.map.obstacles.iter().enumerate() {
            if o.kind == ObstacleKind::FuelTrailer {
                if let Some(hp) = self.obstacle_hp.get_mut(i) {
                    *hp = Some(TRAILER_HEALTH);
                }
                self.spectacle.trailers.push(Trailer { obstacle: i, state: TrailerState::Intact, timer: 0.0, puff: 0.0 });
            }
        }
    }

    /// A new match: every trailer back (`reset_world` put their colliders back).
    pub(crate) fn reset_spectacle(&mut self) {
        self.init_spectacle();
    }

    pub(crate) fn step_spectacle(&mut self, dt: f32) {
        self.step_springs(dt);
        self.step_flights(dt);
        self.step_trailers(dt);
        self.step_slides(dt);
    }

    /// A tank thrown or tumbled onto a bank too steep to drive (a river
    /// cliff, a rim) can sit there wedged; after a moment it slides off,
    /// down the slope.
    fn step_slides(&mut self, dt: f32) {
        let mut wedged = std::mem::take(&mut self.spectacle.wedged);
        for j in 0..self.players.len() {
            let p = &self.players[j];
            let id = p.id;
            let k = wedged.iter().position(|w| w.0 == id);
            if !p.tank.alive || self.spectacle.flying(id) {
                if let Some(k) = k {
                    wedged.swap_remove(k);
                }
                continue;
            }
            let body = self.phys.body(p.tank.body);
            let c = body.translation();
            let n = self.map.normal_at(c.x, c.z);
            let on_ground = c.y - self.map.height_at(c.x, c.z) < 3.0;
            let slow = body.linvel().length() < 1.2;
            // Pressing into a wall: steep ground just ahead of the hull (or
            // behind it, backing up) while the tracks push and nothing moves.
            let throttle = p.input.throttle;
            let fwd = body.rotation() * Vec3::X;
            let fwd = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::X) * throttle.signum();
            let probe = c + fwd * 3.6;
            let wall_n = self.map.normal_at(probe.x, probe.z);
            let pressing = throttle.abs() > 0.3 && wall_n.y < WALL_NORMAL;
            let on_bank = n.y < SLIDE_NORMAL;
            if !on_ground || !slow || !(on_bank || pressing) {
                if let Some(k) = k {
                    wedged.swap_remove(k);
                }
                continue;
            }
            let t = match k {
                Some(k) => {
                    wedged[k].1 += dt;
                    wedged[k].1
                }
                None => {
                    wedged.push((id, dt));
                    dt
                }
            };
            let fire = if on_bank { t > SLIDE_AFTER } else { t > WALL_AFTER };
            if fire {
                let b = self.phys.body_mut(self.players[j].tank.body);
                if on_bank {
                    let down = Vec3::new(n.x, 0.0, n.z).normalize_or(Vec3::X);
                    b.set_linvel(down * SLIDE_SPEED + Vec3::Y * 1.5, true);
                } else {
                    // Slide along the wall the way the hull points, eased off it.
                    let away = Vec3::new(wall_n.x, 0.0, wall_n.z).normalize_or(-fwd);
                    let mut along = Vec3::new(-away.z, 0.0, away.x);
                    if along.dot(fwd) < 0.0 {
                        along = -along;
                    }
                    b.set_linvel(along * WALL_SLIDE_SPEED + away * 1.5 + Vec3::Y * 0.3, true);
                    // ...and swing the nose round to run along it.
                    let turn = fwd.cross(along).y.signum();
                    b.set_angvel(Vec3::Y * turn * 2.5, true);
                }
                if let Some(k) = wedged.iter().position(|w| w.0 == id) {
                    wedged[k].1 = 0.0;
                }
            }
        }
        self.spectacle.wedged = wedged;
    }

    fn step_springs(&mut self, dt: f32) {
        for si in 0..self.map.springs.len() {
            if self.spectacle.springs[si] > 0.0 {
                self.spectacle.springs[si] -= dt;
                continue;
            }
            let site = self.map.springs[si];
            let o = &self.map.obstacles[site.obstacle];
            let (opos, orot) = (o.pos, o.rot);
            let fwd = orot * Vec3::X;
            for j in 0..self.players.len() {
                let p = &self.players[j];
                if !p.tank.alive || self.spectacle.flying(p.id) {
                    continue;
                }
                let b = self.phys.body(p.tank.body);
                let c = b.translation();
                let l = orot.inverse() * (c - opos);
                if l.x.abs() > 3.0 || l.z.abs() > 2.6 || l.y < -1.0 || l.y > 2.8 {
                    continue;
                }
                if b.linvel().dot(fwd) < SPRING_MIN_SPEED {
                    continue;
                }
                // The throw: a ballistic arc from here to the landing spot
                // (the hull keeps its ride height above the ground).
                let ride = c.y - self.map.height_at(c.x, c.z);
                let target = site.land + Vec3::Y * ride;
                let d = target - c;
                let flat = Vec2::new(d.x, d.z).length();
                let time = (flat / SPRING_FLIGHT_SPEED).clamp(2.4, 3.4);
                let v0 = Vec3::new(d.x / time, d.y / time + 0.5 * GRAVITY * time, d.z / time);
                let body = self.phys.body_mut(self.players[j].tank.body);
                body.set_linvel(v0, true);
                body.set_angvel(Vec3::ZERO, true);
                let id = self.players[j].id;
                self.spectacle.flights.push(Flight { player: id, v0, t: 0.0, time });
                self.events.push(Event::Launched { player: id, pos: c });
                self.spectacle.springs[si] = SPRING_RECHARGE;
                break;
            }
        }
    }

    /// Holds each flying tank to its arc (drag would drop it short of the
    /// landing spot) until it lands or hits something.
    fn step_flights(&mut self, dt: f32) {
        let mut k = 0;
        while k < self.spectacle.flights.len() {
            let f = self.spectacle.flights[k].clone();
            let Some(j) = self.idx(f.player).filter(|&j| self.players[j].tank.alive) else {
                self.spectacle.flights.swap_remove(k);
                continue;
            };
            let expected = f.v0 - Vec3::Y * GRAVITY * f.t;
            let t = f.t + dt;
            let body = self.phys.body_mut(self.players[j].tank.body);
            // Hit something on the way (a wall lip, another tank): let physics have it.
            let off = (body.linvel() - expected).length() > 6.0;
            if t >= f.time || off {
                if !off || t > 0.6 * f.time {
                    // Touchdown: the springs bleed the run out of it (a thump, not a skid).
                    let v = body.linvel();
                    body.set_linvel(Vec3::new(v.x * LANDING_KEEP, v.y, v.z * LANDING_KEEP), true);
                }
                self.spectacle.flights.swap_remove(k);
                continue;
            }
            body.set_linvel(f.v0 - Vec3::Y * GRAVITY * t, true);
            let w = body.angvel();
            body.set_angvel(w * 0.85, true);
            self.spectacle.flights[k].t = t;
            k += 1;
        }
    }

    fn step_trailers(&mut self, dt: f32) {
        for ti in 0..self.spectacle.trailers.len() {
            let tr = self.spectacle.trailers[ti].clone();
            let oi = tr.obstacle;
            let hp = self.obstacle_hp.get(oi).copied().flatten().unwrap_or(0.0);
            match tr.state {
                TrailerState::Intact => {
                    if hp <= 0.0 {
                        // Blown apart in one go: no warning.
                        self.trailer_blast(ti);
                    } else if hp < TRAILER_HEALTH * TRAILER_IGNITE {
                        self.light_trailer(ti, TRAILER_BURN);
                    }
                }
                TrailerState::Burning => {
                    let t = &mut self.spectacle.trailers[ti];
                    t.timer -= dt;
                    t.puff -= dt;
                    let (timer, puff) = (t.timer, t.puff);
                    if puff <= 0.0 {
                        // Sputtering fireballs out of the leak, faster as it goes.
                        let c = self.trailer_centre(oi);
                        let k = (self.tick % 3) as f32 - 1.0;
                        let along = self.map.obstacles[oi].rot * Vec3::X * k * 2.5;
                        self.events.push(Event::Explosion { pos: c + along + Vec3::Y * 1.2, radius: 1.6 + 0.4 * (1.0 - timer / TRAILER_BURN), weapon: Weapon::Burn });
                        self.spectacle.trailers[ti].puff = 0.2 + 0.4 * (timer / TRAILER_BURN).max(0.0);
                    }
                    if timer <= 0.0 || hp <= 0.0 {
                        self.trailer_blast(ti);
                    }
                }
                TrailerState::Gone => {
                    self.spectacle.trailers[ti].timer -= dt;
                    if self.spectacle.trailers[ti].timer <= 0.0 {
                        self.restore_trailer(ti);
                    }
                }
            }
        }
    }

    fn trailer_centre(&self, oi: usize) -> Vec3 {
        let o = &self.map.obstacles[oi];
        o.pos + o.rot * o.boxes[0].center
    }

    fn light_trailer(&mut self, ti: usize, fuse: f32) {
        let t = &mut self.spectacle.trailers[ti];
        if t.state != TrailerState::Intact {
            return;
        }
        t.state = TrailerState::Burning;
        t.timer = fuse;
        t.puff = 0.0;
    }

    /// The big one: the trailer goes, everything near it gets thrown, and
    /// the other trailers in reach catch on a short fuse.
    fn trailer_blast(&mut self, ti: usize) {
        let oi = self.spectacle.trailers[ti].obstacle;
        let c = self.trailer_centre(oi);
        // Take the trailer out (rubble, the obstacle removed) if a shell hasn't yet.
        self.damage_obstacle(oi, f32::MAX, None);
        self.spectacle.trailers[ti].state = TrailerState::Gone;
        self.spectacle.trailers[ti].timer = TRAILER_RESPAWN;
        self.events.push(Event::HazardExploded { pos: c + Vec3::Y, radius: TRAILER_BLAST_RADIUS });
        let g = GunStats { damage: 0.0, splash_damage: TRAILER_BLAST_DAMAGE, splash_radius: TRAILER_BLAST_RADIUS, knockback: TRAILER_KNOCKBACK, ..GunStats::default() };
        self.explode(c + Vec3::Y, None, &g, 1.0, None, Weapon::Hazard);
        for tj in 0..self.spectacle.trailers.len() {
            let other = self.spectacle.trailers[tj].obstacle;
            let d = self.trailer_centre(other).distance(c);
            if tj != ti && d < TRAILER_BLAST_RADIUS * 1.6 {
                self.light_trailer(tj, 0.6 + d / 40.0);
            }
        }
    }

    /// Tows a fresh trailer back in, once nobody is parked where it stands.
    fn restore_trailer(&mut self, ti: usize) {
        let oi = self.spectacle.trailers[ti].obstacle;
        let o = self.map.obstacles[oi].clone();
        let blocked = self.players.iter().any(|p| {
            p.tank.alive && {
                let c = self.phys.body(p.tank.body).translation();
                Vec2::new(c.x - o.pos.x, c.z - o.pos.z).length() < o.radius + 3.0
            }
        });
        if blocked {
            self.spectacle.trailers[ti].timer = 2.0;
            return;
        }
        if self.removed.iter().any(|r| r.obstacle as usize == oi) {
            for (bi, (c, rot, h)) in o.world_boxes().enumerate() {
                let handle = self.phys.world.insert_collider(
                    ColliderBuilder::cuboid(h.x, h.y, h.z)
                        .position(pose(c, rot))
                        .friction(0.6)
                        .user_data(OBSTACLE_TAG | (oi as u128) << 8 | bi as u128)
                        .collision_groups(static_groups()),
                    None,
                );
                self.phys.obstacle_colliders[oi][bi] = handle;
            }
            self.removed.retain(|r| r.obstacle as usize != oi);
        }
        if let Some(hp) = self.obstacle_hp.get_mut(oi) {
            *hp = Some(TRAILER_HEALTH);
        }
        let t = &mut self.spectacle.trailers[ti];
        t.state = TrailerState::Intact;
        t.timer = 0.0;
    }
}
