//! Destructible terrain and props ("more destructible terrain and props!").
//!
//! - **Staged cover**: shacks, tin fences, market stalls, tents, watchtowers,
//!   water towers, wind pumps and power poles take damage like the old
//!   destructibles. Below 60% health they are **dented** (stage 1: holes and
//!   soot on the client); at zero they become **rubble** (stage 2): a low
//!   heap that still gives low cover (shacks), flattened sheets you drive
//!   over (fences, stalls, tents), or the thing **toppled** over (towers,
//!   poles), falling away from whoever brought it down. The old
//!   destructibles (barriers, sandbags, crates, wrecks, pipes) dent too.
//! - **Ramming**: fences, stalls, tents, wind pumps and power poles give way
//!   to a tank driving into them fast; it ploughs on through (oil drums are
//!   loose props already).
//! - **Craters**: big blasts on the ground (air-strike bombs, fuel trailers
//!   and tanks, the mortar, the Big Bore) dig a real dent in the terrain:
//!   the map's heights and the physics heightfield both change, capped so
//!   tanks always drive out. The crater list rides in every snapshot (late
//!   joiners get all of them), quantised so the server and every client dig
//!   exactly the same.
//!
//! Nothing is rebuilt during a match; a match reset puts everything back.
//! The bots' nav grid is patched round each change (a few dozen cells).
//!
//! Hooks: [`Sim::init_destruct`] after the world is built, [`Sim::reset_destruct`]
//! after a match reset, [`Sim::step_destruct`] each tick, [`Sim::rubble_instead`]
//! at the top of `destroy_obstacle`, [`Sim::crater_from_blast`] in `explode`.

use glam::{Quat, Vec2, Vec3};
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::events::{Event, Weapon};
use crate::map::{Cover, ObstacleKind};
use crate::parts::{GunKind, PartKind};
use crate::physics::{debris_groups, pose, static_groups};
use crate::sim::{Prop, PropKind, Sim};
use crate::snapshot::RemovedSnap;
use crate::PlayerId;

/// A crater, quantised for the wire (and dug from the quantised values on
/// both ends, so everyone agrees).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraterSnap {
    /// Centre, decimetres.
    pub x: i16,
    pub z: i16,
    /// Radius, decimetres.
    pub r: u8,
    /// Depth, centimetres.
    pub d: u8,
}

impl CraterSnap {
    pub fn new(c: Vec2, radius: f32, depth: f32) -> Self {
        CraterSnap {
            x: (c.x * 10.0).round().clamp(-32000.0, 32000.0) as i16,
            z: (c.y * 10.0).round().clamp(-32000.0, 32000.0) as i16,
            r: (radius * 10.0).round().clamp(1.0, 255.0) as u8,
            d: (depth * 100.0).round().clamp(1.0, 255.0) as u8,
        }
    }
    pub fn centre(&self) -> Vec2 {
        Vec2::new(self.x as f32 * 0.1, self.z as f32 * 0.1)
    }
    pub fn radius(&self) -> f32 {
        self.r as f32 * 0.1
    }
    pub fn depth(&self) -> f32 {
        self.d as f32 * 0.01
    }
}

/// What a destroyed obstacle left behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RubbleKind {
    /// A low heap (still low cover).
    Pile,
    /// Flattened (drive over it).
    Flat,
    /// Toppled over, lying along `RubbleSnap::dir`.
    Topple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubbleSnap {
    pub obstacle: u16,
    pub kind: RubbleKind,
    /// Which way it fell (256ths of a turn; yaw, 0 = +X).
    pub dir: u8,
}

impl RubbleSnap {
    /// The fall direction as a yaw (0 faces +X, like obstacle yaws).
    pub fn yaw(&self) -> f32 {
        self.dir as f32 / 256.0 * std::f32::consts::TAU
    }
}

/// At most this many craters a match (the oldest stay; later blasts just burn).
pub const MAX_CRATERS: usize = 40;
/// A tank this fast (m/s) bursts through light cover.
pub const RAM_SPEED: f32 = 6.5;
/// Dented below this share of health.
pub const DENT_AT: f32 = 0.6;

/// The staged kinds: (health, what they become, rammable).
fn staged(kind: ObstacleKind) -> Option<(f32, RubbleKind, bool)> {
    use ObstacleKind::*;
    Some(match kind {
        Shack => (260.0, RubbleKind::Pile, false),
        Fence => (150.0, RubbleKind::Flat, true),
        Stall => (70.0, RubbleKind::Flat, true),
        Tent => (150.0, RubbleKind::Flat, true),
        Tower => (220.0, RubbleKind::Topple, false),
        WaterTower => (380.0, RubbleKind::Topple, false),
        Windmill => (150.0, RubbleKind::Topple, true),
        PowerPole => (90.0, RubbleKind::Topple, true),
        _ => return None,
    })
}

/// What a fast tank ploughs through: the light staged cover (fences,
/// stalls, tents). Oil drums are loose props already; crates and wrecks
/// hold (they are the low cover the fights lean on).
fn rammable(kind: ObstacleKind) -> bool {
    staged(kind).is_some_and(|s| s.2)
}

#[derive(Clone, Debug, Default)]
pub struct Destruct {
    /// Per obstacle: 0 whole, 1 dented, 2 rubble or gone.
    pub stage: Vec<u8>,
    max_hp: Vec<f32>,
    pub rubble: Vec<RubbleSnap>,
    /// The rubble's colliders (taken out again on a reset).
    colliders: Vec<ColliderHandle>,
    pub craters: Vec<CraterSnap>,
    /// The untouched heights (kept once the first crater digs in).
    base: Vec<f32>,
    /// Each tank's velocity last tick (rams are judged on it).
    prev_vel: Vec<(PlayerId, Vec3)>,
}

impl Destruct {
    /// Obstacles dented right now (for the snapshot).
    pub fn dented(&self) -> Vec<u16> {
        self.stage.iter().enumerate().filter(|(_, s)| **s == 1).map(|(i, _)| i as u16).collect()
    }
}

impl Sim {
    /// Health for the staged kinds; everything starts whole.
    pub(crate) fn init_destruct(&mut self) {
        let n = self.map.obstacles.len();
        let mut d = Destruct { stage: vec![0; n], max_hp: vec![0.0; n], ..Default::default() };
        for (i, o) in self.map.obstacles.iter().enumerate() {
            if let Some((hp, _, _)) = staged(o.kind) {
                if self.obstacle_hp[i].is_none() {
                    self.obstacle_hp[i] = Some(hp);
                }
            }
            // (Fuel hazards burn and blow instead of denting.)
            if !o.kind.is_fuel() && o.kind != ObstacleKind::FuelTrailer {
                d.max_hp[i] = self.obstacle_hp[i].unwrap_or(0.0);
            }
        }
        // Keep the craters' ground record across a reset (it is the map's own).
        d.base = std::mem::take(&mut self.destruct.base);
        self.destruct = d;
    }

    /// A new match: rubble cleared away (`reset_world` already put the
    /// obstacles back), craters filled in, nav restored.
    pub(crate) fn reset_destruct(&mut self) {
        for h in std::mem::take(&mut self.destruct.colliders) {
            self.phys.world.remove_collider(h);
        }
        let had_craters = !self.destruct.craters.is_empty();
        if had_craters && !self.destruct.base.is_empty() {
            std::sync::Arc::make_mut(&mut self.map).heights = self.destruct.base.clone();
            self.refresh_heightfield();
        }
        let rubble: Vec<u16> = self.destruct.rubble.iter().map(|r| r.obstacle).collect();
        self.init_destruct();
        for oi in rubble {
            let o = &self.map.obstacles[oi as usize];
            let (c, r) = (Vec2::new(o.pos.x, o.pos.z), o.radius + 16.0);
            self.refresh_nav(c, r);
        }
    }

    pub(crate) fn step_destruct(&mut self, _dt: f32) {
        if self.destruct.stage.len() != self.map.obstacles.len() {
            return;
        }
        // Dents (every few ticks: a cheap scan).
        if self.tick % 6 == 0 {
            for oi in 0..self.destruct.stage.len() {
                if self.destruct.stage[oi] != 0 || self.destruct.max_hp[oi] <= 0.0 {
                    continue;
                }
                let hp = self.obstacle_hp[oi].unwrap_or(0.0);
                if hp > 0.0 && hp < self.destruct.max_hp[oi] * DENT_AT {
                    self.destruct.stage[oi] = 1;
                    let o = &self.map.obstacles[oi];
                    let c = o.pos + o.rot * o.boxes[0].center;
                    self.events.push(Event::Impact { pos: c, normal: Vec3::Y, weapon: Weapon::World });
                }
            }
        }
        self.step_rams();
    }

    /// Fast tanks burst through light cover: judged on last tick's speed
    /// (physics has already slowed them by now), and they keep most of it.
    fn step_rams(&mut self) {
        let mut prev = std::mem::take(&mut self.destruct.prev_vel);
        for j in 0..self.players.len() {
            let p = &self.players[j];
            if !p.tank.alive {
                continue;
            }
            let id = p.id;
            let body = self.phys.body(p.tank.body);
            let c = body.translation();
            let v_now = body.linvel();
            let v = prev.iter().find(|(pid, _)| *pid == id).map(|x| x.1).unwrap_or(v_now);
            let flat_v = Vec2::new(v.x, v.z);
            if flat_v.length() > RAM_SPEED {
                let c2 = Vec2::new(c.x, c.z);
                let mut hit = None;
                for (oi, o) in self.map.obstacles.iter().enumerate() {
                    if !rammable(o.kind) || self.destruct.stage.get(oi).is_some_and(|s| *s >= 2) || !self.obstacle_hp[oi].is_some_and(|h| h > 0.0) {
                        continue;
                    }
                    if Vec2::new(o.pos.x, o.pos.z).distance(c2) > o.radius + 4.0 {
                        continue;
                    }
                    let touching = o.world_boxes().any(|(bc, r, h)| {
                        let l = r.inverse() * (Vec3::new(c.x, bc.y, c.z) - bc);
                        let toward = flat_v.dot(Vec2::new(bc.x - c.x, bc.z - c.z)) > 0.0;
                        toward && l.x.abs() < h.x + 3.4 && l.z.abs() < h.z + 3.4
                    });
                    if touching {
                        hit = Some(oi);
                        break;
                    }
                }
                if let Some(oi) = hit {
                    self.damage_obstacle(oi, f32::MAX, Some(id));
                    let b = self.phys.body_mut(self.players[j].tank.body);
                    b.set_linvel(v * 0.8, true);
                }
            }
        }
        prev.clear();
        for p in &self.players {
            if p.tank.alive {
                prev.push((p.id, self.phys.body(p.tank.body).linvel()));
            }
        }
        self.destruct.prev_vel = prev;
    }

    /// Called at the top of `destroy_obstacle`: a staged kind becomes rubble
    /// (or topples) instead of vanishing. Returns true when it handled it.
    pub(crate) fn rubble_instead(&mut self, oi: usize, owner: Option<PlayerId>) -> bool {
        let Some((_, kind, _)) = staged(self.map.obstacles[oi].kind) else { return false };
        if self.destruct.stage.get(oi).is_some_and(|s| *s >= 2) {
            return true;
        }
        let o = self.map.obstacles[oi].clone();
        if let Some(Some(hp)) = self.obstacle_hp.get_mut(oi) {
            *hp = 0.0;
        }
        // The standing pieces go...
        for (bi, _) in o.boxes.iter().enumerate() {
            let removed = self.removed.iter().any(|r| r.obstacle as usize == oi && (r.box_index == 255 || r.box_index as usize == bi));
            if !removed {
                let h = self.phys.obstacle_colliders[oi][bi];
                self.phys.world.remove_collider(h);
            }
        }
        self.removed.retain(|r| r.obstacle as usize != oi);
        self.removed.push(RemovedSnap { obstacle: oi as u16, box_index: 255 });
        // ...and what's left lies there. Toppled things fall away from
        // whoever brought them down (or a way of their own).
        let from = owner.and_then(|id| self.idx(id)).map(|j| self.phys.body(self.players[j].tank.body).translation());
        let away = from.map(|f| Vec2::new(o.pos.x - f.x, o.pos.z - f.z)).filter(|d| d.length() > 0.5).map(|d| d.normalize());
        let away = away.unwrap_or_else(|| Vec2::from_angle(self.rng.range(0.0, std::f32::consts::TAU)));
        let yaw = (-away.y).atan2(away.x);
        let dir = ((yaw.rem_euclid(std::f32::consts::TAU)) / std::f32::consts::TAU * 256.0).round() as u32 % 256;
        let snap = RubbleSnap { obstacle: oi as u16, kind, dir: dir as u8 };
        let yaw = snap.yaw();
        let ground: Vec<(Vec3, Quat, Vec3)> = o.world_boxes().filter(|(c, _, h)| c.y - h.y < o.pos.y + 4.0).collect();
        let add = |phys: &mut crate::physics::Physics, c: Vec3, r: Quat, h: Vec3| {
            phys.world.insert_collider(ColliderBuilder::cuboid(h.x, h.y, h.z).position(pose(c, r)).friction(0.7).collision_groups(static_groups()), None)
        };
        let mut handles = Vec::new();
        match kind {
            RubbleKind::Pile => {
                for (c, r, h) in &ground {
                    let c = Vec3::new(c.x, o.pos.y + 0.45, c.z);
                    handles.push(add(&mut self.phys, c, *r, Vec3::new(h.x * 0.95, 0.45, h.z * 0.95)));
                }
            }
            RubbleKind::Flat => {
                for (c, r, h) in &ground {
                    let c = Vec3::new(c.x, o.pos.y + 0.12, c.z);
                    handles.push(add(&mut self.phys, c, *r, Vec3::new(h.x, 0.12, h.z)));
                }
            }
            RubbleKind::Topple => {
                let (len, half_w, half_h) = topple_size(o.kind);
                let d = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
                let c = o.pos + d * (len * 0.5 + 0.5);
                let c = Vec3::new(c.x, self.map.height_at(c.x, c.z) + half_h, c.z);
                handles.push(add(&mut self.phys, c, Quat::from_rotation_y(yaw), Vec3::new(len * 0.5, half_h, half_w)));
            }
        }
        self.destruct.colliders.extend(handles);
        self.destruct.rubble.push(snap);
        if let Some(s) = self.destruct.stage.get_mut(oi) {
            *s = 2;
        }
        // Debris flies.
        let w = self.tuning.world.clone();
        for (c, r, h) in ground.iter().take(3) {
            for k in 0..2 {
                let half = (*h * 0.3).clamp(Vec3::splat(0.15), Vec3::splat(0.9));
                let off = Vec3::new(self.rng.range(-1.0, 1.0), 0.6 + k as f32 * 0.5, self.rng.range(-1.0, 1.0));
                let body = self.phys.world.insert_body(
                    RigidBodyBuilder::dynamic()
                        .pose(pose(*c + off, *r))
                        .linvel(Vec3::new(self.rng.range(-5.0, 5.0), self.rng.range(3.0, 8.0), self.rng.range(-5.0, 5.0)))
                        .angvel(Vec3::new(self.rng.range(-4.0, 4.0), self.rng.range(-4.0, 4.0), self.rng.range(-4.0, 4.0))),
                );
                self.phys.world.insert_collider(ColliderBuilder::cuboid(half.x, half.y, half.z).mass(150.0).collision_groups(debris_groups()), Some(body));
                let id = self.alloc_id();
                self.props.push(Prop { id, kind: PropKind::Rubble(o.kind), body, ttl: Some(w.rubble_lifetime), health: None });
            }
        }
        let centre = ground.first().map(|g| g.0).unwrap_or(o.pos);
        self.events.push(Event::ObstacleDestroyed { obstacle: oi as u16, kind: o.kind, pos: centre });
        // The bots see the change: flattened ground opens up, a toppled
        // tower closes its strip.
        let reach = match kind {
            RubbleKind::Topple => topple_size(o.kind).0 + 6.0,
            _ => o.radius + 6.0,
        };
        self.refresh_nav(Vec2::new(o.pos.x, o.pos.z), reach);
        true
    }

    /// Re-judges the nav cells round `c` (radius `r`): steep ground (and the
    /// ring next to it) stays blocked, the rest follows what stands there now.
    pub(crate) fn refresh_nav(&mut self, c: Vec2, r: f32) {
        let nav = &self.nav;
        let (cell, n, origin) = (nav.cell, nav.n, nav.origin);
        let lo = |v: f32| (((v - r - origin) / cell).floor().max(0.0) as usize).min(n - 1);
        let hi = |v: f32| (((v + r - origin) / cell).ceil().max(0.0) as usize).min(n - 1);
        let centre = |x: usize, z: usize| Vec2::new(origin + (x as f32 + 0.5) * cell, origin + (z as f32 + 0.5) * cell);
        let steep = |p: Vec2| {
            let q = cell * 0.5;
            [(0.0, 0.0), (-q, -q), (q, -q), (-q, q), (q, q), (0.0, -q), (0.0, q), (-q, 0.0), (q, 0.0)]
                .iter()
                .any(|(dx, dz)| self.map.normal_at(p.x + dx, p.y + dz).y < 0.85)
        };
        let mut updates = Vec::new();
        for z in lo(c.y)..=hi(c.y) {
            for x in lo(c.x)..=hi(c.x) {
                let p = centre(x, z);
                // Steep here or next door: leave it as the build had it.
                let near_steep = (-1i32..=1).any(|dz| {
                    (-1i32..=1).any(|dx| {
                        let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                        nx >= 0 && nz >= 0 && (nx as usize) < n && (nz as usize) < n && steep(centre(nx as usize, nz as usize))
                    })
                });
                if near_steep {
                    continue;
                }
                updates.push((z * n + x, self.blocked_now(p, 2.8)));
            }
        }
        if updates.iter().any(|(i, b)| self.nav.blocked[*i] != *b) {
            let nav = std::sync::Arc::make_mut(&mut self.nav);
            for (i, b) in updates {
                nav.blocked[i] = b;
            }
        }
    }

    /// `Map::blocked_xz` as things stand now: gone obstacles don't block,
    /// rubble blocks as its heap (flattened rubble doesn't), toppled things
    /// block where they fell.
    fn blocked_now(&self, p: Vec2, margin: f32) -> bool {
        if p.x.abs() > self.map.play_half - margin || p.y.abs() > self.map.play_half - margin {
            return true;
        }
        for (oi, o) in self.map.obstacles.iter().enumerate() {
            if o.cover == Cover::Flat {
                continue;
            }
            let near = Vec2::new(o.pos.x, o.pos.z).distance(p) <= o.radius + margin + 2.0;
            let rubble = self.destruct.rubble.iter().find(|r| r.obstacle as usize == oi);
            if let Some(rb) = rubble {
                match rb.kind {
                    RubbleKind::Flat => continue,
                    RubbleKind::Pile => {}
                    RubbleKind::Topple => {
                        let (len, half_w, half_h) = topple_size(o.kind);
                        if half_h < 0.5 {
                            continue; // a pole or a wind pump: drive over it
                        }
                        let yaw = rb.yaw();
                        let d = Vec2::new(yaw.cos(), -yaw.sin());
                        let c = Vec2::new(o.pos.x, o.pos.z) + d * (len * 0.5 + 0.5);
                        let l = p - c;
                        let along = l.dot(d);
                        let across = l.dot(Vec2::new(-d.y, d.x));
                        if along.abs() < len * 0.5 + margin && across.abs() < half_w + margin {
                            return true;
                        }
                        continue;
                    }
                }
            } else if self.removed.iter().any(|r| r.obstacle as usize == oi && r.box_index == 255) {
                continue;
            }
            if !near {
                continue;
            }
            for (c, r, h) in o.world_boxes() {
                if c.y - h.y > o.pos.y + 4.0 {
                    continue;
                }
                let local = r.inverse() * (Vec3::new(p.x, c.y, p.y) - c);
                if local.x.abs() < h.x + margin && local.z.abs() < h.z + margin {
                    return true;
                }
            }
        }
        false
    }

    /// Called from `explode`: a big blast on the ground digs a crater.
    pub(crate) fn crater_from_blast(&mut self, pos: Vec3, r: f32, w: Weapon) {
        let big = match w {
            Weapon::AirStrike => true,
            Weapon::Hazard => r >= 10.0,
            Weapon::Special(PartKind::Mortar) => true,
            Weapon::Gun(GunKind::BigBore) => true,
            _ => false,
        };
        if !big || r < 2.5 || self.destruct.craters.len() >= MAX_CRATERS {
            return;
        }
        let ground = self.map.height_at(pos.x, pos.z);
        if pos.y - ground > 3.5 || pos.x.abs() > self.map.play_half || pos.z.abs() > self.map.play_half {
            return;
        }
        // A Big Bore shell scoops a small pit; bombs and fuel blow big holes.
        let (radius, depth) = if matches!(w, Weapon::Gun(_)) {
            (2.6, 0.3)
        } else {
            let radius = (r * 0.5).clamp(3.0, 8.0);
            (radius, (radius * 0.16).min(1.1))
        };
        let c = CraterSnap::new(Vec2::new(pos.x, pos.z), radius, depth);
        if self.destruct.base.is_empty() {
            self.destruct.base = self.map.heights.clone();
        }
        let base = std::mem::take(&mut self.destruct.base);
        std::sync::Arc::make_mut(&mut self.map).apply_crater(c.centre(), c.radius(), c.depth(), &base);
        self.destruct.base = base;
        self.destruct.craters.push(c);
        self.refresh_heightfield();
        self.refresh_nav(c.centre(), c.radius() + 6.0);
    }

    /// Digs the crater a bomb of blast radius `r` would at `pos` (tests, debug).
    pub fn dig_crater(&mut self, pos: Vec3, r: f32) {
        self.crater_from_blast(pos, r, Weapon::AirStrike);
    }

    /// Hands the map's current heights to the physics terrain.
    fn refresh_heightfield(&mut self) {
        let n = self.map.resolution;
        let heights = Array2::from_fn(n, n, |i, j| self.map.heights[i * n + j]);
        let shape = SharedShape::heightfield_with_flags(heights, Vector::new(self.map.size, 1.0, self.map.size), HeightFieldFlags::FIX_INTERNAL_EDGES);
        let handle = self.phys.world.colliders.iter().find(|(_, c)| c.shape().as_heightfield().is_some()).map(|(h, _)| h);
        if let Some(h) = handle {
            if let Some(c) = self.phys.world.colliders.get_mut(h) {
                c.set_shape(shape);
            }
        }
    }
}

/// A toppled thing's footprint: (length, half width, half height).
pub fn topple_size(kind: ObstacleKind) -> (f32, f32, f32) {
    match kind {
        ObstacleKind::WaterTower => (14.0, 3.0, 1.6),
        ObstacleKind::Tower => (9.0, 2.2, 0.9),
        ObstacleKind::Windmill => (11.0, 1.4, 0.4),
        _ => (9.0, 0.5, 0.25),
    }
}
