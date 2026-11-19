//! Wave 2: combo parts, rare parts, the air strike, destructible cover and
//! map hazards. All of it hangs off `Sim` (see the hooks in `sim.rs`).
//!
//! Fun beats balance: these are rare, loud and telegraphed rather than fair.

use glam::{Quat, Vec2, Vec3};
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::events::{Event, HitLocation, HookTarget, PickupKind, Weapon};
use crate::map::ObstacleKind;
use crate::parts::{AltKind, Frame, GunKind, PartKind};
use crate::physics::{debris_groups, obstacle_of, pose, prop_groups, static_groups, OBSTACLE_TAG};
use crate::sim::{Projectile, Prop, PropKind, Sim, Site};
use crate::snapshot::*;
use crate::tank::HullPose;
use crate::tuning::GunStats;
use crate::{Input, PlayerId, DT};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Smoke {
    pub id: u16,
    pub pos: Vec3,
    pub radius: f32,
    pub life: f32,
    pub max_life: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mine {
    pub id: u16,
    pub owner: PlayerId,
    pub pos: Vec3,
    pub age: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Strike {
    pub id: u16,
    pub owner: PlayerId,
    pub target: Vec3,
    pub from: Vec3,
    pub to: Vec3,
    pub timer: f32,
    pub started: bool,
    pub dropped: u32,
    pub bomb_timer: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tether {
    pub owner: PlayerId,
    pub target: HookTarget,
    /// Hook point (World target).
    pub point: Vec3,
    pub time: f32,
}

#[derive(Clone, Debug)]
pub struct Hazard {
    /// Index into `Map::obstacles`.
    pub obstacle: usize,
    pub kind: HazardKind,
    pub ready: bool,
    pub timer: f32,
    pub max_health: f32,
    /// Crane: the wrecking ball body and its velocity last tick.
    pub ball: Option<RigidBodyHandle>,
    pub ball_vel: Vec3,
    pub hit_cd: f32,
    /// Fuel hazards lit by a neighbour's blast: `timer` counts the fuse
    /// down; who set off the first one gets the credit.
    pub fuse_owner: Option<PlayerId>,
}

fn blast(splash: f32, radius: f32, knockback: f32) -> GunStats {
    GunStats { damage: 0.0, splash_damage: splash, splash_radius: radius, knockback, ..GunStats::default() }
}

fn flat(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

/// Distance from point `p` to segment a-b.
fn seg_dist(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// How far a tank's blast centre sits from its own skin (its widest box).
fn boxes_reach(o: &crate::map::Obstacle) -> f32 {
    o.boxes.iter().map(|b| b.half.x.max(b.half.z)).fold(0.0, f32::max)
}

/// Distance from `p` to an oriented box.
fn box_dist(c: Vec3, r: Quat, h: Vec3, p: Vec3) -> f32 {
    let l = r.inverse() * (p - c);
    (l.abs() - h).max(Vec3::ZERO).length()
}

impl Sim {
    // ------------------------------------------------------------ set-up

    pub(crate) fn init_world(&mut self) {
        let map = self.map.clone();
        let w = self.tuning.world.clone();
        self.obstacle_hp = map
            .obstacles
            .iter()
            .map(|o| match o.kind {
                ObstacleKind::Barrier => Some(w.barrier_health),
                ObstacleKind::Sandbags => Some(w.sandbags_health),
                ObstacleKind::CarWreck => Some(w.car_wreck_health),
                ObstacleKind::Crates => Some(w.crates_health),
                ObstacleKind::Pipe => Some(w.pipe_health),
                ObstacleKind::Silo => Some(w.fuel_silo_health),
                ObstacleKind::FuelTank => Some(w.fuel_tank_health),
                ObstacleKind::StorageTank => Some(w.storage_tank_health),
                ObstacleKind::TankCar => Some(w.tank_car_health),
                _ => None,
            })
            .collect();
        self.hazards.clear();
        for (i, o) in map.obstacles.iter().enumerate() {
            let (kind, max) = match o.kind {
                ObstacleKind::Silo => (HazardKind::FuelSilo, w.fuel_silo_health),
                ObstacleKind::FuelTank => (HazardKind::FuelTank, w.fuel_tank_health),
                ObstacleKind::StorageTank => (HazardKind::FuelTank, w.storage_tank_health),
                ObstacleKind::TankCar => (HazardKind::FuelTank, w.tank_car_health),
                ObstacleKind::LaunchPlate => (HazardKind::LaunchPlate, 1.0),
                ObstacleKind::Crane => (HazardKind::Crane, 1.0),
                _ => continue,
            };
            self.hazards.push(Hazard {
                obstacle: i,
                kind,
                ready: true,
                timer: 0.0,
                max_health: max,
                ball: None,
                ball_vel: Vec3::ZERO,
                hit_cd: 0.0,
                fuse_owner: None,
            });
        }
        self.spawn_balls();
        for p in &map.rare_sites {
            let t = self.rng.range(25.0, 45.0);
            self.sites.push(Site { pos: *p, kind: PickupKind::Rare(PartKind::Mortar), timer: t, live: None });
        }
        for p in &map.strike_sites {
            self.sites.push(Site { pos: *p, kind: PickupKind::AirStrike, timer: 70.0, live: None });
        }
    }

    fn spawn_balls(&mut self) {
        let map = self.map.clone();
        let w = self.tuning.world.clone();
        for c in &map.cranes {
            let Some(hz) = self.hazards.iter().position(|h| h.obstacle == c.obstacle) else { continue };
            let o = &map.obstacles[c.obstacle];
            let anchor = self.phys.world.insert_body(RigidBodyBuilder::fixed().translation(c.anchor));
            let side = o.rot * Vec3::Z;
            let ball = self.phys.world.insert_body(
                RigidBodyBuilder::dynamic()
                    .translation(c.anchor - Vec3::Y * c.rope)
                    .linvel(side * 7.0)
                    .angular_damping(1.0),
            );
            self.phys.world.insert_collider(
                ColliderBuilder::ball(w.crane_ball_radius).mass(w.crane_ball_mass).collision_groups(prop_groups()),
                Some(ball),
            );
            let joint = SphericalJointBuilder::new().local_anchor1(Vec3::ZERO).local_anchor2(Vec3::Y * c.rope);
            self.phys.world.insert_impulse_joint(anchor, ball, joint);
            let id = self.alloc_id();
            self.props.push(Prop { id, kind: PropKind::WreckingBall, body: ball, ttl: None, health: None });
            self.hazards[hz].ball = Some(ball);
        }
    }

    /// Put the map back together for a new match.
    pub(crate) fn reset_world(&mut self) {
        let map = self.map.clone();
        for r in std::mem::take(&mut self.removed) {
            let o = &map.obstacles[r.obstacle as usize];
            for (bi, (c, rot, h)) in o.world_boxes().enumerate() {
                if r.box_index != 255 && r.box_index as usize != bi {
                    continue;
                }
                let builder = if o.kind.is_cylinder() {
                    ColliderBuilder::cylinder(h.y, h.x)
                } else {
                    ColliderBuilder::cuboid(h.x, h.y, h.z)
                };
                let handle = self.phys.world.insert_collider(
                    builder
                        .position(pose(c, rot))
                        .friction(0.6)
                        .user_data(OBSTACLE_TAG | (r.obstacle as u128) << 8 | bi as u128)
                        .collision_groups(static_groups()),
                    None,
                );
                self.phys.obstacle_colliders[r.obstacle as usize][bi] = handle;
            }
        }
        let mut k = 0;
        while k < self.props.len() {
            if matches!(self.props[k].kind, PropKind::Container | PropKind::Rubble(_) | PropKind::WreckingBall) {
                let p = self.props.swap_remove(k);
                self.phys.world.remove_body(p.body);
            } else {
                k += 1;
            }
        }
        self.smokes.clear();
        self.mines.clear();
        self.strikes.clear();
        self.tethers.clear();
        self.sites.retain(|s| !matches!(s.kind, PickupKind::Rare(_) | PickupKind::AirStrike));
        let t = self.tuning.clone();
        for p in &mut self.players {
            p.tank.parts.retain(|q| q.tier > 0);
            p.tank.refresh_stats(&mut self.phys, &t);
            p.tank.air_strike = false;
            p.tank.shield = 0.0;
        }
        self.init_world();
    }

    // ------------------------------------------------------------ public helpers

    /// Bolt a combo or rare part onto player index `j` (debug, tests, server cheats).
    pub fn give_part(&mut self, j: usize, kind: PartKind) {
        self.attach_special(j, kind);
    }

    /// Drop a pickup at `pos` (it lives `dropped_lifetime`). Returns its id.
    pub fn spawn_pickup(&mut self, kind: PickupKind, pos: Vec3) -> u16 {
        let id = self.alloc_id();
        let ttl = self.tuning.pickups.dropped_lifetime;
        self.pickups.push(crate::sim::Pickup { id, kind, pos, vel: Vec3::ZERO, age: 10.0, ttl: Some(ttl), site: None, settled: true });
        id
    }

    /// An ownerless explosion (splash damage, radius, knockback) affecting tanks, props and cover.
    pub fn blast(&mut self, pos: Vec3, splash: f32, radius: f32, knockback: f32) {
        self.explode(pos, None, &blast(splash, radius, knockback), 1.0, None, Weapon::World);
    }

    // ------------------------------------------------------------ tank-side

    /// Bolt a combo or rare part on, with its event.
    pub(crate) fn attach_special(&mut self, j: usize, kind: PartKind) {
        let t = self.tuning.clone();
        if self.players[j].tank.add_special(&mut self.phys, &t, kind) {
            let id = self.players[j].id;
            self.events.push(Event::PartAttached { player: id, part: kind });
        }
    }

    /// Shield bubble: soaks damage first. Returns what's left to apply.
    pub(crate) fn absorb(&mut self, j: usize, amount: f32, pos: Vec3, w: Weapon) -> Option<f32> {
        let id = self.players[j].id;
        let tank = &mut self.players[j].tank;
        tank.since_damage = 0.0;
        if tank.shield <= 0.0 || w == Weapon::Burn {
            return Some(amount);
        }
        let a = tank.shield.min(amount);
        tank.shield -= a;
        self.events.push(Event::ShieldHit { player: id, pos, damage: a });
        if tank.shield <= 0.0 {
            tank.shield = 0.0;
            self.events.push(Event::ShieldBroken { player: id });
        }
        let left = amount - a;
        if left > 0.001 {
            Some(left)
        } else {
            None
        }
    }

    /// Per-tick upkeep: shield recharge, repair rig, burning, charges, rocket salvos.
    pub(crate) fn tick_special_passive(&mut self, i: usize, dt: f32) {
        let t = self.tuning.clone();
        let s = &t.special;
        let id = self.players[i].id;
        {
            let tank = &mut self.players[i].tank;
            tank.since_damage += dt;
            tank.flaming = false;
            tank.rare_cooldown = (tank.rare_cooldown - dt).max(0.0);
            if tank.stats.shield_max > 0.0 && tank.since_damage > s.shield_recharge_delay && tank.shield < tank.stats.shield_max {
                let was = tank.shield;
                tank.shield = (tank.shield + s.shield_recharge_rate * dt).min(tank.stats.shield_max);
                if was <= 0.0 {
                    self.events.push(Event::ShieldRestored { player: id });
                }
            }
            if tank.stats.regen > 0.0 && tank.since_damage > s.repair_rig_delay {
                tank.health = (tank.health + tank.stats.regen * dt).min(tank.stats.max_health);
                for tr in &mut tank.tracks {
                    if tr.blown > 0.0 {
                        tr.blown -= dt;
                    }
                }
            }
            let max = match tank.stats.rare {
                Some(PartKind::Mortar) => s.mortar_ammo,
                Some(PartKind::SmokeLauncher) => s.smoke_charges,
                Some(PartKind::MineLayer) => s.mine_charges,
                _ => 0,
            };
            if tank.rare_ammo < max {
                tank.rare_regen += dt;
                if tank.rare_regen >= s.charge_regen {
                    tank.rare_regen = 0.0;
                    tank.rare_ammo += 1;
                }
            }
            if tank.stats.alt != AltKind::Rockets {
                tank.lock = None;
                tank.lock_progress = 0.0;
            }
        }
        // Burning.
        if self.players[i].tank.burning > 0.0 {
            self.players[i].tank.burning -= dt;
            if self.tick % 15 == 0 {
                let by = self.players[i].tank.burn_attacker;
                let pos = self.phys.body(self.players[i].tank.body).translation();
                self.damage(i, by, HitLocation::Hull, s.burn_dps * 0.25, pos, Vec3::Y, Weapon::Burn);
            }
        }
        // Rocket salvo in progress.
        if self.players[i].tank.alive && self.players[i].tank.salvo_left > 0 {
            self.players[i].tank.salvo_timer -= dt;
            if self.players[i].tank.salvo_timer <= 0.0 {
                let hp = HullPose::of(&self.phys, self.players[i].tank.body);
                let (o, r) = self.players[i].tank.frame(&hp, &t, Frame::Turret);
                let side = if self.players[i].tank.salvo_left % 2 == 0 { -0.12 } else { 0.12 };
                let from = o + r * Vec3::new(1.0, 0.6 + side, -1.3);
                let dir = (self.players[i].tank.barrel_dir(&hp, &t) + Vec3::Y * 0.12).normalize();
                let target = self.players[i].tank.salvo_target;
                let pid = self.alloc_id();
                self.projectiles.push(Projectile {
                    id: pid,
                    owner: id,
                    gun: GunKind::Stub,
                    pos: from,
                    vel: dir * s.rocket_speed,
                    age: 0.0,
                    travelled: 0.0,
                    shot: ShotKind::Rocket,
                    target,
                });
                self.events.push(Event::Fired { player: id, weapon: Weapon::Alt(AltKind::Rockets), pos: from, dir });
                let tank = &mut self.players[i].tank;
                tank.salvo_left -= 1;
                tank.salvo_timer = s.rocket_interval;
            }
        }
    }

    /// The special button: air strike first, else the rare part.
    pub(crate) fn special_input(&mut self, i: usize, input: &Input, _dt: f32) {
        let pressed = input.special && !self.players[i].tank.prev_special;
        if !pressed || !self.players[i].tank.alive {
            return;
        }
        if self.players[i].tank.air_strike {
            self.call_strike(i, input.aim_point);
            return;
        }
        if self.players[i].tank.rare_cooldown > 0.0 {
            return;
        }
        match self.players[i].tank.stats.rare {
            Some(PartKind::Mortar) => self.fire_mortar(i, input.aim_point),
            Some(PartKind::Harpoon) => self.fire_harpoon(i),
            Some(PartKind::SmokeLauncher) => self.deploy_smoke(i),
            Some(PartKind::MineLayer) => self.lay_mine(i),
            _ => {}
        }
    }

    pub(crate) fn flamethrower(&mut self, i: usize, input: &Input, dt: f32) {
        let t = self.tuning.clone();
        let s = &t.special;
        let id = self.players[i].id;
        if !input.alt_fire || self.players[i].tank.fuel <= 0.0 {
            return;
        }
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let (o, r) = self.players[i].tank.frame(&hp, &t, Frame::Gun);
        let nozzle = o + r * Vec3::new(1.9, -0.3, 0.32);
        let dir = r * Vec3::X;
        {
            let tank = &mut self.players[i].tank;
            if !tank.prev_alt {
                self.events.push(Event::Fired { player: id, weapon: Weapon::Alt(AltKind::Flamethrower), pos: nozzle, dir });
            }
            tank.flaming = true;
            tank.protect = 0.0;
            tank.fuel = (tank.fuel - s.flame_fuel_per_s * dt).max(0.0);
        }
        if self.tick % 6 != 0 {
            return;
        }
        let cone = s.flame_cone_deg.to_radians();
        let w = Weapon::Alt(AltKind::Flamethrower);
        for j in 0..self.players.len() {
            if j == i || !self.players[j].tank.alive {
                continue;
            }
            let c = self.phys.body(self.players[j].tank.body).translation();
            let v = c - nozzle;
            let d = v.length();
            if d > s.flame_range + 2.5 || d < 0.1 {
                continue;
            }
            let ang = v.angle_between(dir);
            if ang > cone + (2.5 / d).atan() || !self.sight_clear(nozzle, c) {
                continue;
            }
            self.damage(j, Some(id), HitLocation::Hull, s.flame_dps * 0.1, c, v / d, w);
            let tank = &mut self.players[j].tank;
            if tank.alive {
                if tank.burning <= 0.0 {
                    let pid = self.players[j].id;
                    self.events.push(Event::Ignited { player: pid, by: Some(id) });
                }
                let tank = &mut self.players[j].tank;
                tank.burning = s.burn_time;
                tank.burn_attacker = Some(id);
            }
        }
        // Scorch whatever burns in the cone: drums and crates.
        let tip = nozzle + dir * s.flame_range * 0.7;
        self.explode_world(tip, s.flame_range * 0.4, s.flame_dps * 0.1, 0.0, Some(id));
    }

    pub(crate) fn rocket_rack(&mut self, i: usize, input: &Input) {
        let t = self.tuning.clone();
        let s = &t.special;
        let id = self.players[i].id;
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let pivot = hp.point(Vec3::from(t.turret.pivot)) + Vec3::Y * 0.6;
        let aim_dir = (input.aim_point - pivot).normalize_or(hp.rot * Vec3::X);
        let cone = s.rocket_lock_cone_deg.to_radians();
        let mut best: Option<(f32, PlayerId)> = None;
        for (j, p) in self.players.iter().enumerate() {
            if j == i || !p.tank.alive {
                continue;
            }
            let c = self.phys.body(p.tank.body).translation() + Vec3::Y * 0.5;
            let v = c - pivot;
            if v.length() > s.rocket_lock_range {
                continue;
            }
            let ang = v.angle_between(aim_dir);
            if ang < cone && best.is_none_or(|b| ang < b.0) && self.sight_clear(pivot, c) {
                best = Some((ang, p.id));
            }
        }
        {
            let tank = &mut self.players[i].tank;
            match best {
                Some((_, tid)) if tank.lock == Some(tid) => {
                    let was = tank.lock_progress;
                    tank.lock_progress = (tank.lock_progress + DT / s.rocket_lock_time).min(1.0);
                    if was < 1.0 && tank.lock_progress >= 1.0 {
                        self.events.push(Event::LockedOn { player: id, target: tid });
                    }
                }
                Some((_, tid)) => {
                    tank.lock = Some(tid);
                    tank.lock_progress = 0.0;
                }
                None => {
                    tank.lock = None;
                    tank.lock_progress = 0.0;
                }
            }
        }
        let tank = &mut self.players[i].tank;
        if input.alt_fire && !tank.prev_alt && tank.alt_cooldown <= 0.0 && tank.salvo_left == 0 {
            tank.salvo_left = s.rocket_count;
            tank.salvo_timer = 0.0;
            tank.salvo_target = if tank.lock_progress >= 1.0 { tank.lock } else { None };
            tank.alt_cooldown = s.rocket_cooldown;
            tank.protect = 0.0;
        }
    }

    fn fire_mortar(&mut self, i: usize, aim: Vec3) {
        let t = self.tuning.clone();
        let s = &t.special;
        let id = self.players[i].id;
        if self.players[i].tank.rare_ammo == 0 {
            return;
        }
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let origin = hp.point(Vec3::new(-1.7, 1.4, -1.35));
        let d = aim - origin;
        let h = flat(d);
        let dh = h.length().max(1.0);
        // A fixed 55-degree lob; the charge (speed) is set for the range.
        let g = 9.81;
        let theta: f32 = 55f32.to_radians();
        let denom = 2.0 * theta.cos().powi(2) * (dh * theta.tan() - d.y);
        let v = if denom > 0.0 { (g * dh * dh / denom).sqrt().min(s.mortar_speed) } else { s.mortar_speed };
        let hd = h / dh;
        let dir = Vec3::new(hd.x * theta.cos(), theta.sin(), hd.y * theta.cos());
        let pid = self.alloc_id();
        self.projectiles.push(Projectile {
            id: pid,
            owner: id,
            gun: GunKind::Stub,
            pos: origin,
            vel: dir * v,
            age: 0.0,
            travelled: 0.0,
            shot: ShotKind::MortarShell,
            target: None,
        });
        let tank = &mut self.players[i].tank;
        tank.rare_ammo -= 1;
        tank.rare_cooldown = s.mortar_cooldown;
        tank.protect = 0.0;
        self.events.push(Event::Fired { player: id, weapon: Weapon::Special(PartKind::Mortar), pos: origin, dir });
    }

    fn fire_harpoon(&mut self, i: usize) {
        let t = self.tuning.clone();
        let s = &t.special;
        let id = self.players[i].id;
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let (o, r) = self.players[i].tank.frame(&hp, &t, Frame::Turret);
        let origin = o + r * Vec3::new(1.3, 0.95, 0.5);
        let dir = self.hitscan_dir(i, &t, origin);
        self.tethers.retain(|x| x.owner != id);
        let pid = self.alloc_id();
        self.projectiles.push(Projectile {
            id: pid,
            owner: id,
            gun: GunKind::Stub,
            pos: origin,
            vel: dir * s.harpoon_speed,
            age: 0.0,
            travelled: 0.0,
            shot: ShotKind::Harpoon,
            target: None,
        });
        self.players[i].tank.rare_cooldown = s.harpoon_cooldown;
        self.players[i].tank.protect = 0.0;
        self.events.push(Event::Fired { player: id, weapon: Weapon::Special(PartKind::Harpoon), pos: origin, dir });
    }

    fn deploy_smoke(&mut self, i: usize) {
        let t = self.tuning.clone();
        let s = &t.special;
        let id = self.players[i].id;
        if self.players[i].tank.rare_ammo == 0 {
            return;
        }
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let fwd = self.players[i].tank.barrel_dir(&hp, &t);
        let f = flat(fwd).normalize_or(Vec2::X);
        let c = flat(hp.pos) + f * s.smoke_distance;
        let pos = Vec3::new(c.x, self.map.height_at(c.x, c.y) + 3.0, c.y);
        let sid = self.alloc_id();
        self.smokes.push(Smoke { id: sid, pos, radius: s.smoke_radius, life: s.smoke_life, max_life: s.smoke_life });
        let tank = &mut self.players[i].tank;
        tank.rare_ammo -= 1;
        tank.rare_cooldown = s.smoke_cooldown;
        self.events.push(Event::SmokeDeployed { player: id, pos, radius: s.smoke_radius });
    }

    fn lay_mine(&mut self, i: usize) {
        let t = self.tuning.clone();
        let s = &t.special;
        let id = self.players[i].id;
        let active = self.mines.iter().filter(|m| m.owner == id).count();
        if self.players[i].tank.rare_ammo == 0 || active >= s.mine_max_active as usize {
            return;
        }
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let p = hp.point(Vec3::new(-3.8, 0.0, 0.0));
        let pos = Vec3::new(p.x, self.map.height_at(p.x, p.z) + 0.1, p.z);
        let mid = self.alloc_id();
        self.mines.push(Mine { id: mid, owner: id, pos, age: 0.0 });
        if self.mines.len() > 48 {
            self.mines.remove(0);
        }
        let tank = &mut self.players[i].tank;
        tank.rare_ammo -= 1;
        tank.rare_cooldown = s.mine_cooldown;
        self.events.push(Event::MineLaid { player: id, id: mid, pos });
    }

    fn call_strike(&mut self, i: usize, aim: Vec3) {
        let t = self.tuning.clone();
        let s = &t.special;
        let id = self.players[i].id;
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let mut off = flat(aim - hp.pos);
        if off.length() > s.strike_max_range {
            off = off.normalize() * s.strike_max_range;
        }
        let c = flat(hp.pos) + off;
        let ph = self.map.play_half - 5.0;
        let c = Vec2::new(c.x.clamp(-ph, ph), c.y.clamp(-ph, ph));
        let dir = if off.length() > 5.0 { off.normalize() } else { flat(hp.rot * Vec3::X).normalize_or(Vec2::X) };
        let a = c - dir * s.strike_length / 2.0;
        let b = c + dir * s.strike_length / 2.0;
        let at = |p: Vec2| Vec3::new(p.x, self.map.height_at(p.x, p.y), p.y);
        let (target, from, to) = (at(c), at(a), at(b));
        let sid = self.alloc_id();
        self.strikes.push(Strike {
            id: sid,
            owner: id,
            target,
            from,
            to,
            timer: s.strike_delay,
            started: false,
            dropped: 0,
            bomb_timer: 0.0,
        });
        self.players[i].tank.air_strike = false;
        self.events.push(Event::AirStrikeCalled { player: id, target, from, to, delay: s.strike_delay });
    }

    /// On death: the rare part and any flare drop as golden crates; combos go.
    pub(crate) fn death_specials(&mut self, j: usize, pos: Vec3, vel: Vec3) {
        let t = self.tuning.clone();
        let id = self.players[j].id;
        if let Some(r) = self.players[j].tank.stats.rare {
            self.spill(PickupKind::Rare(r), pos + Vec3::Y, vel);
        }
        if self.players[j].tank.air_strike {
            self.spill(PickupKind::AirStrike, pos + Vec3::Y, vel);
        }
        let tank = &mut self.players[j].tank;
        tank.air_strike = false;
        tank.parts.retain(|p| p.tier > 0);
        tank.refresh_stats(&mut self.phys, &t);
        tank.shield = 0.0;
        tank.burning = 0.0;
        tank.flaming = false;
        tank.lock = None;
        tank.lock_progress = 0.0;
        tank.salvo_left = 0;
        tank.rare_ammo = 0;
        self.tethers.retain(|x| x.owner != id && x.target != HookTarget::Tank(id));
    }

    /// Whether a sight line is clear of walls and smoke.
    pub fn sight_clear(&self, a: Vec3, b: Vec3) -> bool {
        self.phys.clear_line(a, b) && self.smokes.iter().all(|s| seg_dist(a, b, s.pos) > s.radius * 0.85)
    }

    // ------------------------------------------------------------ projectiles

    pub(crate) fn step_special_projectile(&mut self, mut pr: Projectile, dt: f32) -> Option<Projectile> {
        let t = self.tuning.clone();
        let s = &t.special;
        let owner = self.idx(pr.owner);
        pr.age += dt;
        let (life, grav) = match pr.shot {
            ShotKind::Rocket => (5.0, 0.0),
            ShotKind::MortarShell => (20.0, 1.0),
            ShotKind::Harpoon => (s.harpoon_range / s.harpoon_speed, 0.3),
            ShotKind::Shell => (0.0, 1.0),
        };
        if pr.age > life {
            return None;
        }
        if pr.shot == ShotKind::Rocket {
            if let Some(tj) = pr.target.and_then(|x| self.idx(x)) {
                if self.players[tj].tank.alive {
                    let c = self.phys.body(self.players[tj].tank.body).translation() + Vec3::Y * 0.4;
                    let v = pr.vel.normalize_or(Vec3::X);
                    let want = (c - pr.pos).normalize_or(v);
                    let ang = v.angle_between(want);
                    if ang > 1e-4 {
                        let step = ang.min(s.rocket_turn_rate * dt);
                        let axis = v.cross(want).normalize_or(Vec3::Y);
                        pr.vel = Quat::from_axis_angle(axis, step) * v * s.rocket_speed;
                    }
                }
            }
        }
        pr.vel.y -= 9.81 * grav * dt;
        let seg = pr.vel * dt;
        let len = seg.length();
        let dir = seg / len.max(1e-6);
        // Harpoon: snag a pickup passing near the line.
        if pr.shot == ShotKind::Harpoon {
            let end = pr.pos + seg;
            if let Some(k) = self.pickups.iter().position(|p| seg_dist(pr.pos, end, p.pos) < 1.8) {
                let pid = self.pickups[k].id;
                let at = self.pickups[k].pos;
                self.tethers.push(Tether { owner: pr.owner, target: HookTarget::Pickup(pid), point: at, time: s.harpoon_reel_time });
                self.events.push(Event::HarpoonHooked { player: pr.owner, target: HookTarget::Pickup(pid), pos: at });
                return None;
            }
        }
        let Some(h) = self.trace(pr.pos, dir, len, owner) else {
            pr.pos += seg;
            pr.travelled += len;
            return if pr.pos.y < -100.0 { None } else { Some(pr) };
        };
        match pr.shot {
            ShotKind::Rocket | ShotKind::MortarShell => {
                let (w, direct, b) = if pr.shot == ShotKind::Rocket {
                    (Weapon::Alt(AltKind::Rockets), s.rocket_damage, blast(s.rocket_splash, s.rocket_radius, 40_000.0))
                } else {
                    (Weapon::Special(PartKind::Mortar), s.mortar_damage, blast(s.mortar_splash, s.mortar_radius, s.mortar_knockback))
                };
                let mut hit_tank = None;
                if let Some((vi, loc)) = h.tank {
                    self.damage(vi, Some(pr.owner), loc, direct, h.pos, dir, w);
                    hit_tank = Some(vi);
                } else {
                    self.events.push(Event::Impact { pos: h.pos, normal: h.normal, weapon: w });
                    self.hit_collider(h.collider, direct, b.knockback, h.pos, dir, Some(pr.owner));
                }
                self.explode(h.pos + h.normal * 0.3, Some(pr.owner), &b, 1.0, hit_tank, w);
            }
            ShotKind::Harpoon => {
                let w = Weapon::Special(PartKind::Harpoon);
                let target = if let Some((vi, loc)) = h.tank {
                    self.damage(vi, Some(pr.owner), loc, s.harpoon_damage, h.pos, dir, w);
                    HookTarget::Tank(self.players[vi].id)
                } else {
                    let prop = h.collider.and_then(|c| self.phys.world.colliders[c].parent()).and_then(|b| {
                        self.props.iter().find(|p| p.body == b).map(|p| p.id)
                    });
                    match prop {
                        Some(id) => HookTarget::Prop(id),
                        None => HookTarget::World,
                    }
                };
                self.tethers.push(Tether { owner: pr.owner, target, point: h.pos, time: s.harpoon_reel_time });
                self.events.push(Event::HarpoonHooked { player: pr.owner, target, pos: h.pos });
            }
            ShotKind::Shell => {}
        }
        None
    }

    // ------------------------------------------------------------ world damage

    /// A direct hit on a collider: obstacles take damage, props get shoved (drums may blow).
    pub(crate) fn hit_collider(
        &mut self,
        c: Option<ColliderHandle>,
        damage: f32,
        knockback: f32,
        pos: Vec3,
        dir: Vec3,
        owner: Option<PlayerId>,
    ) {
        let Some(c) = c else { return };
        let Some(coll) = self.phys.world.colliders.get(c) else { return };
        if let Some((oi, _)) = obstacle_of(coll.user_data) {
            self.damage_obstacle(oi, damage, owner);
            return;
        }
        let Some(b) = coll.parent() else { return };
        if self.phys.body(b).is_dynamic() && knockback > 0.0 {
            let m = self.phys.body(b).mass();
            self.phys.body_mut(b).apply_impulse_at_point(dir * (m * 6.0).min(knockback), pos, true);
        }
        if let Some(k) = self.props.iter().position(|p| p.body == b) {
            if let Some(hp) = self.props[k].health.as_mut() {
                *hp -= damage;
                if *hp <= 0.0 {
                    self.blow_drum(k, owner);
                }
            }
        }
    }

    pub(crate) fn damage_obstacle(&mut self, oi: usize, amount: f32, owner: Option<PlayerId>) {
        let Some(Some(hp)) = self.obstacle_hp.get_mut(oi) else { return };
        if *hp <= 0.0 {
            return;
        }
        *hp -= amount;
        if *hp <= 0.0 {
            self.destroy_obstacle(oi, owner);
        }
    }

    fn is_removed(&self, oi: usize, bi: usize) -> bool {
        self.removed
            .iter()
            .any(|r| r.obstacle as usize == oi && (r.box_index == 255 || r.box_index as usize == bi))
    }

    fn destroy_obstacle(&mut self, oi: usize, owner: Option<PlayerId>) {
        let map = self.map.clone();
        let w = self.tuning.world.clone();
        let o = &map.obstacles[oi];
        if self.removed.iter().any(|r| r.obstacle as usize == oi && r.box_index == 255) {
            return;
        }
        if let Some(Some(hp)) = self.obstacle_hp.get_mut(oi) {
            *hp = 0.0;
        }
        let boxes: Vec<(Vec3, Quat, Vec3)> = o.world_boxes().collect();
        for (bi, _) in boxes.iter().enumerate() {
            if !self.is_removed(oi, bi) {
                let h = self.phys.obstacle_colliders[oi][bi];
                self.phys.world.remove_collider(h);
            }
        }
        self.removed.retain(|r| r.obstacle as usize != oi);
        self.removed.push(RemovedSnap { obstacle: oi as u16, box_index: 255 });
        // Rubble.
        for (c, r, h) in boxes.iter().take(5) {
            for k in 0..2 {
                let half = (*h * 0.45).clamp(Vec3::splat(0.2), Vec3::splat(1.3));
                let off = Vec3::new(self.rng.range(-1.0, 1.0), 0.3 + k as f32 * 0.4, self.rng.range(-1.0, 1.0));
                let body = self.phys.world.insert_body(
                    RigidBodyBuilder::dynamic()
                        .pose(pose(*c + off, *r))
                        .linvel(Vec3::new(self.rng.range(-4.0, 4.0), self.rng.range(3.0, 7.0), self.rng.range(-4.0, 4.0)))
                        .angvel(Vec3::new(self.rng.range(-3.0, 3.0), self.rng.range(-3.0, 3.0), self.rng.range(-3.0, 3.0))),
                );
                self.phys.world.insert_collider(
                    ColliderBuilder::cuboid(half.x, half.y, half.z).mass(300.0).collision_groups(debris_groups()),
                    Some(body),
                );
                let id = self.alloc_id();
                self.props.push(Prop { id, kind: PropKind::Rubble(o.kind), body, ttl: Some(w.rubble_lifetime), health: None });
            }
        }
        let center = boxes.iter().map(|b| b.0).sum::<Vec3>() / boxes.len().max(1) as f32;
        self.events.push(Event::ObstacleDestroyed { obstacle: oi as u16, kind: o.kind, pos: center });
        if o.kind.is_fuel() {
            if let Some(hz) = self.hazards.iter_mut().find(|h| h.obstacle == oi) {
                hz.ready = false;
                hz.timer = 0.0;
            }
            let big = o.kind == ObstacleKind::StorageTank;
            let (dmg, radius) = if big { (w.storage_blast_damage, w.storage_blast_radius) } else { (w.fuel_blast_damage, w.fuel_blast_radius) };
            self.events.push(Event::HazardExploded { pos: center, radius });
            let b = blast(dmg, radius, w.fuel_blast_knockback * if big { 1.4 } else { 1.0 });
            self.explode(center, owner, &b, 1.0, None, Weapon::Hazard);
            // Chain reactions: a storage tank or tank car lights the fuel
            // hazards next to it, each on a short fuse.
            if matches!(o.kind, ObstacleKind::StorageTank | ObstacleKind::TankCar) {
                for hz in 0..self.hazards.len() {
                    let h = &self.hazards[hz];
                    if h.kind != HazardKind::FuelTank && h.kind != HazardKind::FuelSilo || !h.ready || h.timer > 0.0 || h.obstacle == oi {
                        continue;
                    }
                    let other = &map.obstacles[h.obstacle];
                    let near = other.world_boxes().map(|(c, r, hh)| box_dist(c, r, hh, center)).fold(f32::MAX, f32::min);
                    // Reach from the blast centre to the neighbour's skin.
                    let reach = if big { boxes_reach(o) + w.chain_reach } else { w.chain_reach + 6.0 };
                    if near < reach {
                        let d = flat(other.pos).distance(flat(o.pos));
                        self.hazards[hz].timer = w.chain_fuse * (0.7 + d / 40.0);
                        self.hazards[hz].fuse_owner = owner;
                    }
                }
            }
        }
    }

    fn topple(&mut self, oi: usize, bi: usize, from: Vec3, impulse: f32) {
        let map = self.map.clone();
        let w = self.tuning.world.clone();
        let boxes: Vec<(Vec3, Quat, Vec3)> = map.obstacles[oi].world_boxes().collect();
        let base = boxes[bi].0;
        let mut release: Vec<usize> =
            (0..boxes.len()).filter(|&k| boxes[k].0.y >= base.y - 0.1 && flat(boxes[k].0).distance(flat(base)) < 2.0).collect();
        release.retain(|&k| !self.is_removed(oi, k));
        for k in release {
            let (c, r, h) = boxes[k];
            let handle = self.phys.obstacle_colliders[oi][k];
            self.phys.world.remove_collider(handle);
            self.removed.push(RemovedSnap { obstacle: oi as u16, box_index: k as u8 });
            let body = self.phys.world.insert_body(RigidBodyBuilder::dynamic().pose(pose(c, r)));
            self.phys.world.insert_collider(
                ColliderBuilder::cuboid(h.x, h.y, h.z).mass(w.container_mass).friction(0.6).collision_groups(prop_groups()),
                Some(body),
            );
            let dir = (c - from + Vec3::Y * 2.0).normalize_or(Vec3::Y);
            let j = (impulse * if k == bi { 0.5 } else { 0.3 }).min(w.container_mass * 14.0);
            self.phys.body_mut(body).apply_impulse_at_point(dir * j, c + Vec3::Y * h.y * 0.8, true);
            let id = self.alloc_id();
            self.props.push(Prop { id, kind: PropKind::Container, body, ttl: None, health: None });
            self.events.push(Event::ContainerToppled { obstacle: oi as u16, box_index: k as u8, prop_id: id });
        }
    }

    fn blow_drum(&mut self, k: usize, owner: Option<PlayerId>) {
        let w = self.tuning.world.clone();
        let p = self.props.swap_remove(k);
        let pos = self.phys.body(p.body).translation();
        self.phys.world.remove_body(p.body);
        self.events.push(Event::HazardExploded { pos, radius: w.drum_blast_radius });
        let b = blast(w.drum_blast_damage, w.drum_blast_radius, 60_000.0);
        self.explode(pos, owner, &b, 1.0, None, Weapon::Hazard);
    }

    /// Blast effects on the world: destructible cover, containers, oil drums.
    pub(crate) fn explode_world(&mut self, pos: Vec3, r: f32, dmg: f32, knockback: f32, owner: Option<PlayerId>) {
        let map = self.map.clone();
        let topple_at = self.tuning.world.container_topple_impulse;
        for (oi, o) in map.obstacles.iter().enumerate() {
            if flat(o.pos).distance(flat(pos)) > o.radius + r + 4.0 {
                continue;
            }
            let destructible = self.obstacle_hp[oi].is_some_and(|h| h > 0.0);
            let container = o.kind == ObstacleKind::ContainerStack;
            if !destructible && !container {
                continue;
            }
            let mut best_f: f32 = 0.0;
            let mut topple: Option<(usize, f32)> = None;
            for (bi, (c, rot, h)) in o.world_boxes().enumerate() {
                if self.is_removed(oi, bi) {
                    continue;
                }
                let d = box_dist(c, rot, h, pos);
                if d >= r {
                    continue;
                }
                let f = 1.0 - d / r;
                best_f = best_f.max(f);
                if container && knockback * f >= topple_at && topple.is_none_or(|t| c.y < map.obstacles[oi].boxes[t.0].center.y + o.pos.y) {
                    topple = Some((bi, knockback * f));
                }
            }
            if destructible && best_f > 0.0 {
                self.damage_obstacle(oi, dmg * best_f, owner);
            }
            if let Some((bi, imp)) = topple {
                self.topple(oi, bi, pos, imp);
            }
        }
        // Oil drums.
        let mut blow = Vec::new();
        for p in &mut self.props {
            if let Some(hp) = p.health.as_mut() {
                let c = self.phys.world.bodies[p.body].translation();
                let d = c.distance(pos);
                if d < r {
                    *hp -= dmg * (1.0 - d / r);
                    if *hp <= 0.0 {
                        blow.push(p.id);
                    }
                }
            }
        }
        for id in blow {
            if let Some(k) = self.props.iter().position(|p| p.id == id) {
                self.blow_drum(k, owner);
            }
        }
    }

    // ------------------------------------------------------------ world tick

    pub(crate) fn step_world(&mut self, dt: f32) {
        let t = self.tuning.clone();
        let s = &t.special;
        // Smoke.
        for sm in &mut self.smokes {
            sm.life -= dt;
        }
        self.smokes.retain(|sm| sm.life > 0.0);

        // Mines.
        let mut k = 0;
        while k < self.mines.len() {
            self.mines[k].age += dt;
            let m = self.mines[k].clone();
            let mut victim = None;
            if m.age >= s.mine_arm_time {
                for p in &self.players {
                    if p.id == m.owner || !p.tank.alive {
                        continue;
                    }
                    let c = self.phys.body(p.tank.body).translation();
                    if flat(c).distance(flat(m.pos)) < s.mine_trigger_radius && (c.y - m.pos.y).abs() < 3.5 {
                        victim = Some(p.id);
                        break;
                    }
                }
            }
            if let Some(v) = victim {
                self.mines.remove(k);
                self.events.push(Event::MineTriggered { id: m.id, pos: m.pos, victim: Some(v) });
                let b = blast(s.mine_damage, s.mine_radius, s.mine_knockback);
                self.explode(m.pos + Vec3::Y * 0.2, Some(m.owner), &b, 1.0, None, Weapon::Special(PartKind::MineLayer));
                continue;
            }
            k += 1;
        }

        // Air strikes.
        let mut k = 0;
        while k < self.strikes.len() {
            let n = s.strike_bombs.max(1);
            self.strikes[k].timer -= dt;
            if self.strikes[k].timer <= 0.0 {
                if !self.strikes[k].started {
                    self.strikes[k].started = true;
                    let st = &self.strikes[k];
                    self.events.push(Event::BombingRun { player: st.owner, from: st.from, to: st.to });
                }
                self.strikes[k].bomb_timer -= dt;
                while self.strikes[k].bomb_timer <= 0.0 && self.strikes[k].dropped < n {
                    let st = self.strikes[k].clone();
                    let f = (st.dropped as f32 + 0.5) / n as f32;
                    let line = st.to - st.from;
                    let side = Vec3::new(-line.z, 0.0, line.x).normalize_or(Vec3::X) * self.rng.range(-3.0, 3.0);
                    let p = st.from + line * f + side;
                    let p = Vec3::new(p.x, self.map.height_at(p.x, p.z) + 0.3, p.z);
                    let b = blast(s.strike_damage, s.strike_radius, s.strike_knockback);
                    self.explode(p, Some(st.owner), &b, 1.0, None, Weapon::AirStrike);
                    self.strikes[k].dropped += 1;
                    self.strikes[k].bomb_timer += s.strike_duration / n as f32;
                }
                if self.strikes[k].dropped >= n {
                    self.strikes.remove(k);
                    continue;
                }
            }
            k += 1;
        }

        self.step_tethers(dt);
        self.step_hazards(dt);
    }

    fn step_tethers(&mut self, dt: f32) {
        let s = self.tuning.special.clone();
        let mut k = 0;
        while k < self.tethers.len() {
            self.tethers[k].time -= dt;
            let tt = self.tethers[k].clone();
            let Some(oi) = self.idx(tt.owner).filter(|&o| self.players[o].tank.alive) else {
                self.tethers.remove(k);
                continue;
            };
            let ob = self.players[oi].tank.body;
            let op = self.phys.body(ob).translation();
            let om = self.phys.body(ob).mass();
            let mut done = tt.time <= 0.0;
            match tt.target {
                HookTarget::Tank(id) => match self.idx(id).filter(|&j| self.players[j].tank.alive) {
                    Some(j) => {
                        let tb = self.players[j].tank.body;
                        let tp = self.phys.body(tb).translation();
                        let d = tp - op;
                        if d.length() < 7.0 {
                            done = true;
                        } else {
                            let dir = d.normalize();
                            let f = s.harpoon_reel_accel * om.min(self.phys.body(tb).mass());
                            self.phys.body_mut(tb).apply_impulse(-dir * f * dt, true);
                            self.phys.body_mut(ob).apply_impulse(dir * f * 0.3 * dt, true);
                        }
                    }
                    None => done = true,
                },
                HookTarget::Prop(id) => match self.props.iter().find(|p| p.id == id).map(|p| p.body) {
                    Some(b) => {
                        let d = self.phys.body(b).translation() - op;
                        if d.length() < 5.0 {
                            done = true;
                        } else {
                            let m = self.phys.body(b).mass();
                            self.phys.body_mut(b).apply_impulse(-d.normalize() * m * s.harpoon_reel_accel * 1.5 * dt, true);
                        }
                    }
                    None => done = true,
                },
                HookTarget::Pickup(id) => match self.pickups.iter_mut().find(|p| p.id == id) {
                    Some(p) => {
                        let d = op - p.pos;
                        let step = (25.0 * dt).min(d.length());
                        p.pos += d.normalize_or_zero() * step;
                        p.settled = true;
                        p.age = p.age.max(10.0);
                    }
                    None => done = true,
                },
                HookTarget::World => {
                    let d = tt.point - op;
                    if d.length() < 6.0 {
                        done = true;
                    } else {
                        self.phys.body_mut(ob).apply_impulse(d.normalize() * om * s.harpoon_reel_accel * dt, true);
                    }
                }
            }
            if done {
                self.tethers.remove(k);
                self.events.push(Event::HarpoonReleased { player: tt.owner });
                continue;
            }
            k += 1;
        }
    }

    fn step_hazards(&mut self, dt: f32) {
        let map = self.map.clone();
        let w = self.tuning.world.clone();
        for hz in 0..self.hazards.len() {
            match self.hazards[hz].kind {
                HazardKind::LaunchPlate => {
                    if !self.hazards[hz].ready {
                        self.hazards[hz].timer -= dt;
                        if self.hazards[hz].timer <= 0.0 {
                            self.hazards[hz].ready = true;
                        }
                        continue;
                    }
                    let o = &map.obstacles[self.hazards[hz].obstacle];
                    for j in 0..self.players.len() {
                        if !self.players[j].tank.alive {
                            continue;
                        }
                        let b = self.players[j].tank.body;
                        let c = self.phys.body(b).translation();
                        let l = o.rot.inverse() * (c - o.pos);
                        let v = self.phys.body(b).linvel();
                        let hv = Vec3::new(v.x, 0.0, v.z);
                        if l.x.abs() < 3.2 && l.z.abs() < 2.8 && l.y > -1.0 && l.y < 2.5 && hv.length() > w.plate_min_speed {
                            let nv = v + Vec3::Y * w.plate_up_speed + hv.normalize() * w.plate_forward_speed;
                            self.phys.body_mut(b).set_linvel(nv, true);
                            let id = self.players[j].id;
                            self.events.push(Event::Launched { player: id, pos: c });
                            self.hazards[hz].ready = false;
                            self.hazards[hz].timer = w.plate_recharge;
                            break;
                        }
                    }
                }
                HazardKind::Crane => {
                    let Some(ball) = self.hazards[hz].ball else { continue };
                    let o = &map.obstacles[self.hazards[hz].obstacle];
                    let bv = self.phys.body(ball).linvel();
                    let bp = self.phys.body(ball).translation();
                    let hv = Vec3::new(bv.x, 0.0, bv.z);
                    // Keep it swinging: pump along its motion, or kick it off along the jib's side.
                    let push = if hv.length() > 0.3 { hv.normalize() } else { o.rot * Vec3::Z };
                    if hv.length() < 8.0 {
                        self.phys.body_mut(ball).apply_impulse(push * w.crane_push * dt, true);
                    }
                    self.hazards[hz].hit_cd -= dt;
                    let prev = self.hazards[hz].ball_vel;
                    if self.hazards[hz].hit_cd <= 0.0 {
                        for j in 0..self.players.len() {
                            if !self.players[j].tank.alive {
                                continue;
                            }
                            let tb = self.players[j].tank.body;
                            let tp = self.phys.body(tb).translation();
                            let n = tp - bp;
                            if n.length() > w.crane_ball_radius + 3.8 {
                                continue;
                            }
                            let n = n.normalize_or(Vec3::X);
                            let rel = (prev - self.phys.body(tb).linvel()).dot(n);
                            if rel > w.crane_hit_speed {
                                let id = self.players[j].id;
                                self.events.push(Event::CraneHit { player: id, pos: bp + n * w.crane_ball_radius, speed: rel });
                                let imp = (n + Vec3::Y * 0.4).normalize() * w.crane_ball_mass * rel * 0.8;
                                self.phys.body_mut(tb).apply_impulse(imp, true);
                                self.damage(j, None, HitLocation::Hull, (rel - w.crane_hit_speed) * w.crane_damage_per_ms, bp, n, Weapon::Hazard);
                                self.hazards[hz].hit_cd = 1.0;
                                break;
                            }
                        }
                    }
                    self.hazards[hz].ball_vel = self.phys.body(ball).linvel();
                }
                HazardKind::FuelSilo | HazardKind::FuelTank => {
                    if self.hazards[hz].ready && self.hazards[hz].timer > 0.0 {
                        self.hazards[hz].timer -= dt;
                        if self.hazards[hz].timer <= 0.0 {
                            let oi = self.hazards[hz].obstacle;
                            let owner = self.hazards[hz].fuse_owner;
                            self.damage_obstacle(oi, f32::MAX, owner);
                        }
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ snapshot parts

    pub(crate) fn smoke_snaps(&self) -> Vec<SmokeSnap> {
        self.smokes
            .iter()
            .map(|s| SmokeSnap { id: s.id, pos: s.pos.to_array(), radius: s.radius, life: q01(s.life / s.max_life) })
            .collect()
    }

    pub(crate) fn mine_snaps(&self) -> Vec<MineSnap> {
        let arm = self.tuning.special.mine_arm_time;
        self.mines
            .iter()
            .map(|m| MineSnap { id: m.id, owner: m.owner, pos: m.pos.to_array(), armed: m.age >= arm })
            .collect()
    }

    pub(crate) fn strike_snaps(&self) -> Vec<StrikeSnap> {
        self.strikes
            .iter()
            .map(|s| StrikeSnap {
                id: s.id,
                owner: s.owner,
                target: s.target.to_array(),
                from: s.from.to_array(),
                to: s.to.to_array(),
                ticks_left: (s.timer.max(0.0) * 60.0) as u16,
                dropped: s.dropped as u8,
            })
            .collect()
    }

    pub(crate) fn hazard_snaps(&self) -> Vec<HazardSnap> {
        self.hazards
            .iter()
            .map(|h| {
                let health = match h.kind {
                    HazardKind::FuelSilo | HazardKind::FuelTank => {
                        q01(self.obstacle_hp[h.obstacle].unwrap_or(0.0) / h.max_health)
                    }
                    _ => {
                        if h.ready {
                            255
                        } else {
                            0
                        }
                    }
                };
                HazardSnap { obstacle: h.obstacle as u16, kind: h.kind, health, ready: h.ready }
            })
            .collect()
    }

    pub(crate) fn tether_snaps(&self) -> Vec<TetherSnap> {
        self.tethers
            .iter()
            .filter_map(|x| {
                let o = self.idx(x.owner)?;
                let from = self.phys.body(self.players[o].tank.body).translation() + Vec3::Y * 1.4;
                let to = match x.target {
                    HookTarget::Tank(id) => self.phys.body(self.players[self.idx(id)?].tank.body).translation(),
                    HookTarget::Prop(id) => self.phys.body(self.props.iter().find(|p| p.id == id)?.body).translation(),
                    HookTarget::Pickup(id) => self.pickups.iter().find(|p| p.id == id)?.pos,
                    HookTarget::World => x.point,
                };
                Some(TetherSnap { player: x.owner, from: from.to_array(), to: to.to_array() })
            })
            .collect()
    }
}
