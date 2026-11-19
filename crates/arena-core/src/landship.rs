//! THE LAND-SHIP: Raider Waves' second boss (every other boss wave, wave 10,
//! 20, ...). A beached ship hull welded onto four giant track units, about
//! 30 x 9 x 8 m, crawling at walking pace and bristling with guns.
//!
//! Modules (all [`crate::parts::SHIP_MODULES`], shot off one by one like the
//! War Rig's; see `parts.rs` for their boxes in the ship's hull frame):
//! - **Two deck gun turrets** (fore, aft): heavy cannons that each pick their
//!   own target and traverse on their own (`BossSnap::turrets`).
//! - **The mortar battery**: a salvo of shells lobbed at the biggest cluster of
//!   defenders every few seconds. Spread out!
//! - **Two broadside MG nests**: hose anything close on their side.
//! - **The bridge** with its sensor mast: lose it and every gun's aim scatters.
//! - **Four track units**: lose one and it slows; lose both on a side and it
//!   crawls and slews toward the dead side.
//! - **Two plate sets** (deck and belt armour, fore and aft): the hull core
//!   shrugs off hits until both are gone.
//! - **The hangar**: while it stands, the stern ramp drops a raider buggy every
//!   `ship_hangar_interval` seconds.
//!
//! The ship drives toward the outpost (its bot brain) but parks at
//! `ship_standoff` and bombards it; its gunnery here runs every tick.

use glam::{Quat, Vec2, Vec3};

use crate::events::{Event, HitLocation, Weapon};
use crate::parts::{AltKind, GunKind, PartKind, VehicleKind, SHIP_GUNS, SHIP_MGS, SHIP_MORTAR_AT, SHIP_RAMP_AT, SHIP_TRACKS_LEFT, SHIP_TRACKS_RIGHT};
use crate::raid::{TurretSnap, TEAM_RAIDERS};
use crate::sim::{Projectile, Sim};
use crate::snapshot::ShotKind;
use crate::tank::HullPose;
use crate::{Input, PlayerId};

/// One Land-Ship's gunnery state.
#[derive(Clone, Debug)]
pub struct ShipState {
    pub id: PlayerId,
    /// Deck guns (fore, aft): reload left, turret yaw and gun pitch (relative to the hull).
    pub gun_reload: [f32; 2],
    pub gun_yaw: [f32; 2],
    pub gun_pitch: [f32; 2],
    pub gun_target: [Option<PlayerId>; 2],
    pub mortar_timer: f32,
    pub mg_cooldown: [f32; 2],
    pub hangar_timer: f32,
    /// Aim noise per gun (re-rolled now and then), as an angle offset.
    noise: [Vec2; 2],
    noise_t: f32,
}

impl ShipState {
    pub fn new(id: PlayerId, hangar_timer: f32) -> ShipState {
        ShipState {
            id,
            gun_reload: [2.0, 3.5],
            gun_yaw: [0.0, std::f32::consts::PI],
            gun_pitch: [0.0; 2],
            gun_target: [None; 2],
            mortar_timer: 6.0,
            mg_cooldown: [0.0; 2],
            hangar_timer,
            noise: [Vec2::ZERO; 2],
            noise_t: 0.0,
        }
    }
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

/// Low-arc elevation (world) to put a shell of `speed` on a point `dh` away and `dy` up.
fn ballistic(speed: f32, gravity: f32, dh: f32, dy: f32) -> f32 {
    let v2 = speed * speed;
    let disc = v2 * v2 - gravity * (gravity * dh * dh + 2.0 * dy * v2);
    if disc < 0.0 {
        std::f32::consts::FRAC_PI_4
    } else {
        ((v2 - disc.sqrt()) / (gravity * dh.max(0.1))).atan()
    }
}

impl Sim {
    fn ship_has(&self, j: usize, kind: PartKind) -> bool {
        self.players[j].tank.parts.iter().any(|p| p.kind == kind)
    }

    /// Living crews the ship's guns may shoot at: (index, position, velocity).
    fn ship_targets(&self) -> Vec<(usize, Vec3, Vec3)> {
        self.players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.team != Some(TEAM_RAIDERS) && p.tank.alive)
            .map(|(j, p)| {
                let b = self.phys.body(p.tank.body);
                (j, b.translation(), b.linvel())
            })
            .collect()
    }

    /// Every Land-Ship's guns, mortar, MG nests and hangar, once per tick.
    pub(crate) fn ship_weapons(&mut self, dt: f32) {
        let Some(raid) = self.raid.as_ref() else { return };
        if raid.ships.is_empty() || raid.lost.is_some() {
            return;
        }
        let mut ships = std::mem::take(&mut self.raid.as_mut().unwrap().ships);
        for s in &mut ships {
            let Some(j) = self.idx(s.id) else { continue };
            if !self.players[j].tank.alive || self.players[j].tank.vehicle != VehicleKind::LandShip {
                continue;
            }
            self.ship_guns(j, s, dt);
            self.ship_mortar(j, s, dt);
            self.ship_mgs(j, s, dt);
            self.ship_hangar(j, s, dt);
        }
        // Spawning buggies may not touch `ships`, but keep any added meanwhile.
        let r = self.raid.as_mut().unwrap();
        ships.append(&mut r.ships);
        r.ships = ships;
    }

    fn ship_error(&self, j: usize) -> f32 {
        let r = &self.tuning.raid;
        if self.ship_has(j, PartKind::ShipBridge) {
            r.ship_aim_error_deg.to_radians()
        } else {
            r.ship_blind_error_deg.to_radians()
        }
    }

    fn ship_guns(&mut self, j: usize, s: &mut ShipState, dt: f32) {
        let t = self.tuning.clone();
        let g = t.raid.deck_gun.clone();
        let id = s.id;
        let hp = HullPose::of(&self.phys, self.players[j].tank.body);
        let err = self.ship_error(j);
        s.noise_t -= dt;
        if s.noise_t <= 0.0 {
            s.noise_t = self.rng.range(0.5, 1.2);
            for k in 0..2 {
                s.noise[k] = Vec2::new(self.rng.range(-1.0, 1.0), self.rng.range(-0.5, 0.5)) * err;
            }
        }
        let targets = self.ship_targets();
        let range = g.shell_speed * g.lifetime * 0.85;
        let grav = 9.81 * g.gravity_scale;
        for k in 0..2 {
            let (part, ring_local) = SHIP_GUNS[k];
            s.gun_reload[k] = (s.gun_reload[k] - dt).max(0.0);
            if !self.ship_has(j, part) {
                s.gun_target[k] = None;
                continue;
            }
            let ring = hp.point(ring_local);
            let eye = ring + hp.rot * Vec3::Y * 1.2;
            // Fore covers all but a wedge astern (the bridge), aft all but a wedge ahead.
            let arc_ok = |local: Vec3| {
                let yaw = (-local.z).atan2(local.x);
                let rel = if k == 0 { yaw } else { wrap(yaw - std::f32::consts::PI) };
                rel.abs() < 2.7
            };
            let other = s.gun_target[1 - k];
            let mut best: Option<(f32, usize, Vec3, Vec3)> = None;
            for &(q, c, v) in &targets {
                let d = c.distance(ring);
                if d > range || !arc_ok(hp.rot.inverse() * (c - ring)) {
                    continue;
                }
                if !self.sight_clear(eye, c + Vec3::Y * 0.5) {
                    continue;
                }
                // Each turret prefers its own target: the two guns split up.
                let mut score = d;
                if Some(self.players[q].id) == other {
                    score += 60.0;
                }
                if Some(self.players[q].id) == s.gun_target[k] {
                    score -= 15.0;
                }
                if best.is_none_or(|b| score < b.0) {
                    best = Some((score, q, c, v));
                }
            }
            let aim = match best {
                Some((_, q, c, v)) => {
                    s.gun_target[k] = Some(self.players[q].id);
                    let mut a = c + Vec3::Y * 0.4;
                    let mut tof = a.distance(ring) / g.shell_speed;
                    for _ in 0..2 {
                        a = c + Vec3::Y * 0.4 + v * tof;
                        tof = a.distance(ring) / g.shell_speed;
                    }
                    Some(a)
                }
                None => {
                    s.gun_target[k] = None;
                    // Nobody in sight: shell the outpost if it's in range.
                    let o = self.raid.as_ref().map(|r| r.outpost + Vec3::Y);
                    o.filter(|o| o.distance(ring) < range && arc_ok(hp.rot.inverse() * (*o - ring)))
                }
            };
            let Some(aim) = aim else { continue };
            let local = hp.rot.inverse() * (aim - ring);
            let want_yaw = (-local.z).atan2(local.x) + s.noise[k].x;
            let flat = Vec2::new(local.x, local.z).length();
            let want_pitch = ballistic(g.shell_speed, grav, flat, local.y) + s.noise[k].y;
            let step = t.raid.ship_gun_traverse * dt;
            let e = wrap(want_yaw - s.gun_yaw[k]);
            s.gun_yaw[k] = wrap(s.gun_yaw[k] + e.clamp(-step, step));
            s.gun_pitch[k] += (want_pitch - s.gun_pitch[k]).clamp(-step, step);
            if s.gun_reload[k] > 0.0 || e.abs() > 0.06 {
                continue;
            }
            let dir_local = Quat::from_rotation_y(s.gun_yaw[k]) * Quat::from_rotation_z(s.gun_pitch[k]) * Vec3::X;
            let dir = self.rng.cone(hp.rot * dir_local, g.spread_deg.to_radians());
            let muzzle = ring + hp.rot * (dir_local * g.barrel_length) + hp.rot * Vec3::Y * 0.2;
            let pid = self.alloc_id();
            self.projectiles.push(Projectile {
                id: pid,
                owner: id,
                gun: GunKind::DeckGun,
                pos: muzzle,
                vel: dir * g.shell_speed,
                age: 0.0,
                travelled: 0.0,
                shot: ShotKind::Shell,
                target: None,
            });
            s.gun_reload[k] = g.reload * self.rng.range(0.9, 1.15);
            self.events.push(Event::Fired { player: id, weapon: Weapon::Gun(GunKind::DeckGun), pos: muzzle, dir });
        }
    }

    fn ship_mortar(&mut self, j: usize, s: &mut ShipState, dt: f32) {
        let r = self.tuning.raid.clone();
        let sp = self.tuning.special.clone();
        s.mortar_timer -= dt;
        if s.mortar_timer > 0.0 || !self.ship_has(j, PartKind::ShipMortar) {
            return;
        }
        let id = s.id;
        let hp = HullPose::of(&self.phys, self.players[j].tank.body);
        let origin = hp.point(SHIP_MORTAR_AT);
        let targets = self.ship_targets();
        // The biggest cluster in range (ties: the nearest).
        let mut best: Option<(f32, Vec3)> = None;
        for &(_, c, _) in &targets {
            let d = c.distance(origin);
            if d < r.ship_mortar_min_range || d > r.ship_mortar_max_range {
                continue;
            }
            let near: Vec<Vec3> = targets.iter().map(|x| x.1).filter(|o| o.distance(c) < r.ship_mortar_cluster).collect();
            let centre = near.iter().copied().sum::<Vec3>() / near.len().max(1) as f32;
            let score = near.len() as f32 * 1000.0 - d;
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, centre));
            }
        }
        let aim = best.map(|b| b.1).or_else(|| {
            let o = self.raid.as_ref()?.outpost;
            (o.distance(origin) < r.ship_mortar_max_range).then_some(o)
        });
        let Some(aim) = aim else {
            s.mortar_timer = 1.0;
            return;
        };
        s.mortar_timer = r.ship_mortar_interval * self.rng.range(0.9, 1.1);
        let scatter = r.ship_mortar_scatter * if self.ship_has(j, PartKind::ShipBridge) { 1.0 } else { 2.0 };
        for k in 0..r.ship_mortar_shells {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let rr = self.rng.range(0.0, scatter);
            let p = aim + Vec3::new(a.cos() * rr, 0.0, a.sin() * rr);
            let d = p - origin;
            let h = Vec2::new(d.x, d.z);
            let dh = h.length().max(1.0);
            let theta: f32 = 60f32.to_radians();
            let g = 9.81;
            let denom = 2.0 * theta.cos().powi(2) * (dh * theta.tan() - d.y);
            let v = if denom > 0.0 { (g * dh * dh / denom).sqrt().min(sp.mortar_speed * 1.2) } else { sp.mortar_speed };
            let hd = h / dh;
            let dir = Vec3::new(hd.x * theta.cos(), theta.sin(), hd.y * theta.cos());
            let side = hp.rot * Vec3::Z * (k as f32 - 1.0) * 0.8;
            let pid = self.alloc_id();
            self.projectiles.push(Projectile {
                id: pid,
                owner: id,
                gun: GunKind::Stub,
                pos: origin + side,
                vel: dir * v,
                age: 0.0,
                travelled: 0.0,
                shot: ShotKind::MortarShell,
                target: None,
            });
            self.events.push(Event::Fired { player: id, weapon: Weapon::Special(PartKind::ShipMortar), pos: origin + side, dir });
        }
    }

    fn ship_mgs(&mut self, j: usize, s: &mut ShipState, dt: f32) {
        let r = self.tuning.raid.clone();
        let id = s.id;
        let hp = HullPose::of(&self.phys, self.players[j].tank.body);
        let err = self.ship_error(j) * 1.5;
        let targets = self.ship_targets();
        for k in 0..2 {
            let (part, at) = SHIP_MGS[k];
            s.mg_cooldown[k] = (s.mg_cooldown[k] - dt).max(0.0);
            if s.mg_cooldown[k] > 0.0 || !self.ship_has(j, part) {
                continue;
            }
            let muzzle = hp.point(at);
            let out = hp.rot * Vec3::Z * if k == 0 { -1.0 } else { 1.0 };
            let mut best: Option<(f32, usize, Vec3)> = None;
            for &(q, c, _) in &targets {
                let v = c + Vec3::Y * 0.4 - muzzle;
                let d = v.length();
                if d > r.ship_mg_range || v.dot(out) < d * 0.2 {
                    continue;
                }
                if best.is_none_or(|b| d < b.0) && self.sight_clear(muzzle, c + Vec3::Y * 0.5) {
                    best = Some((d, q, c));
                }
            }
            let Some((_, _, c)) = best else { continue };
            s.mg_cooldown[k] = 1.0 / r.ship_mg_rate.max(0.1);
            let dir = self.rng.cone((c + Vec3::Y * 0.4 - muzzle).normalize_or(out), err);
            let w = Weapon::Alt(AltKind::Mg);
            let hit = self.trace(muzzle, dir, r.ship_mg_range, Some(j));
            let end = hit.map(|h| h.pos).unwrap_or(muzzle + dir * r.ship_mg_range);
            self.events.push(Event::Tracer { player: id, weapon: w, from: muzzle, to: end });
            if let Some(h) = hit {
                if let Some((vi, loc)) = h.tank {
                    self.damage(vi, Some(id), loc, r.ship_mg_damage, h.pos, dir, w);
                } else {
                    self.events.push(Event::Impact { pos: h.pos, normal: h.normal, weapon: w });
                }
            }
        }
    }

    fn ship_hangar(&mut self, j: usize, s: &mut ShipState, dt: f32) {
        if !self.ship_has(j, PartKind::ShipHangar) {
            return;
        }
        s.hangar_timer -= dt;
        if s.hangar_timer > 0.0 {
            return;
        }
        s.hangar_timer = self.tuning.raid.ship_hangar_interval;
        let alive = self.players.iter().filter(|p| p.team == Some(TEAM_RAIDERS) && p.tank.alive).count();
        if alive >= self.tuning.raid.max_alive + 2 {
            return;
        }
        let hp = HullPose::of(&self.phys, self.players[j].tank.body);
        let fwd = hp.rot * Vec3::X;
        let yaw = (-fwd.z).atan2(fwd.x) + std::f32::consts::PI;
        // Down the stern ramp, or off to a side if the ground behind is blocked.
        let mut spot = None;
        for off in [SHIP_RAMP_AT, SHIP_RAMP_AT + Vec3::Z * 6.0, SHIP_RAMP_AT - Vec3::Z * 6.0, Vec3::new(0.0, -1.5, 9.0), Vec3::new(0.0, -1.5, -9.0)] {
            let p = hp.point(off);
            if !self.map.blocked_xz(Vec2::new(p.x, p.z), 3.0) {
                spot = Some(p);
                break;
            }
        }
        let Some(p) = spot else { return };
        let pos = Vec3::new(p.x, self.map.height_at(p.x, p.z) + 1.3, p.z);
        if let Some(buggy) = self.spawn_raider_at(VehicleKind::Buggy, Some((pos, yaw))) {
            if let Some(b) = self.idx(buggy) {
                // A shove down the ramp.
                let body = self.players[b].tank.body;
                self.phys.body_mut(body).set_linvel(-fwd * 6.0, true);
            }
            self.events.push(Event::ShipLaunchedBuggy { ship: s.id, buggy, pos });
        }
    }

    /// Land-Ship driving on top of its bot brain: no main gun or MG from the
    /// brain (the gunnery above does the shooting), park at the standoff range,
    /// and slew toward a side whose track units are both gone.
    pub(crate) fn ship_inputs(&mut self) {
        let Some(raid) = self.raid.as_ref() else { return };
        let outpost = raid.outpost;
        let r = self.tuning.raid.clone();
        for i in 0..self.players.len() {
            let p = &self.players[i];
            if p.tank.vehicle != VehicleKind::LandShip || !p.tank.alive {
                continue;
            }
            let hp = HullPose::of(&self.phys, p.tank.body);
            let fwd = hp.rot * Vec3::X;
            let mut input = Input { aim_point: hp.pos + fwd * 60.0, ..p.input };
            input.fire = false;
            input.alt_fire = false;
            input.special = false;
            input.boost = false;
            let d = Vec2::new(hp.pos.x - outpost.x, hp.pos.z - outpost.z).length();
            if d < r.ship_standoff {
                // Parked: swing broadside to the outpost so both deck guns and a
                // nest bear on it.
                input.throttle = 0.0;
                let to = Vec2::new(outpost.x - hp.pos.x, outpost.z - hp.pos.z);
                let bearing = wrap((-to.y).atan2(to.x) - (-fwd.z).atan2(fwd.x));
                let err = if bearing > 0.0 { bearing - std::f32::consts::FRAC_PI_2 } else { bearing + std::f32::consts::FRAC_PI_2 };
                input.steer = if err.abs() < 0.12 { 0.0 } else { (-err * 2.0).clamp(-1.0, 1.0) };
            }

            let has = |k: &PartKind| p.tank.parts.iter().any(|q| q.kind == *k);
            let left = SHIP_TRACKS_LEFT.iter().filter(|k| has(k)).count();
            let right = SHIP_TRACKS_RIGHT.iter().filter(|k| has(k)).count();
            if input.throttle.abs() > 0.05 {
                if left == 0 && right > 0 {
                    input.steer = (input.steer - r.ship_slew).clamp(-1.0, 1.0);
                } else if right == 0 && left > 0 {
                    input.steer = (input.steer + r.ship_slew).clamp(-1.0, 1.0);
                }
            }
            self.players[i].input = input;
        }
    }

    /// The deck guns' aim for the snapshot (empty for other machines).
    pub(crate) fn ship_turrets(&self, id: PlayerId) -> Vec<TurretSnap> {
        let Some(s) = self.raid.as_ref().and_then(|r| r.ships.iter().find(|s| s.id == id)) else { return Vec::new() };
        (0..2).map(|k| TurretSnap { part: SHIP_GUNS[k].0, yaw: s.gun_yaw[k], pitch: s.gun_pitch[k] }).collect()
    }

    /// The Land-Ship's base track boxes don't exist (its tracks are modules):
    /// a stray `Track` location on it counts as the hull. The Convoy truck's
    /// wheels can't be blown off either (its run would just stall).
    pub(crate) fn ship_location(&self, j: usize, loc: HitLocation) -> HitLocation {
        match (self.players[j].tank.vehicle, loc) {
            (VehicleKind::LandShip, HitLocation::Track(_) | HitLocation::Turret) => HitLocation::Hull,
            (VehicleKind::FuelTanker, HitLocation::Track(_)) => HitLocation::Hull,
            (_, l) => l,
        }
    }

}
