//! A tank: rapier hull, raycast suspension, skid-steer drive, turret, and
//! the part-level hit boxes.

use glam::{Quat, Vec3};
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::events::{HitLocation, RepairKind};
use crate::parts::{compute_stats_on, VehicleKind, parts_for, Frame, HitBox, Part, PartKind, TankStats, Track, HULL_BOX, TRACK_BOXES, TURRET_BOX};
use crate::physics::{ground_query, pose, tank_groups, Physics};
use crate::tuning::{GunStats, TankTuning, Tuning};
use crate::Input;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WheelState {
    /// Metres the spring is compressed (0 = fully extended).
    pub compression: f32,
    pub grounded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackState {
    pub health: f32,
    /// Seconds until the crew has it patched; 0 = intact.
    pub blown: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Repair {
    pub kind: RepairKind,
    pub elapsed: f32,
    pub duration: f32,
    /// Health restored per second while it runs.
    pub heal_rate: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingChoice {
    pub track: Track,
    pub options: Vec<PartKind>,
    pub time_left: f32,
}

#[derive(Clone, Debug)]
pub struct Tank {
    pub body: RigidBodyHandle,
    pub alive: bool,
    pub health: f32,
    pub stats: TankStats,
    /// Turret yaw relative to the hull, rad (positive = left).
    pub turret_yaw: f32,
    /// Gun elevation relative to the turret, rad (positive = up).
    pub gun_pitch: f32,
    /// Road wheels: first `wheels_per_side` are the left side (-Z), front to back.
    pub wheels: Vec<WheelState>,
    /// Track surface speed, m/s (left, right), for tread animation.
    pub track_speed: [f32; 2],
    pub tracks: [TrackState; 2],
    pub tiers: [u8; 4],
    pub branch: [Option<PartKind>; 4],
    pub scrap: [u8; 4],
    pub parts: Vec<Part>,
    pub choice: Option<PendingChoice>,
    pub fuel: f32,
    pub boosting: bool,
    pub reload: f32,
    pub alt_cooldown: f32,
    /// Coilgun charge, 0..1.
    pub charge: f32,
    /// Accuracy bloom, 0..1.
    pub bloom: f32,
    pub repair: Option<Repair>,
    pub tipped_time: f32,
    pub respawn_in: f32,
    pub protect: f32,
    pub last_attacker: Option<u8>,
    /// Times the anti-fall-through net had to catch this tank (should stay 0).
    pub rescues: u32,
    pub twin_side: bool,
    pub prev_alt: bool,
    // --- Combo and rare parts, air strike.
    /// Shield bubble hit points left.
    pub shield: f32,
    /// Seconds since this tank last took damage.
    pub since_damage: f32,
    /// Seconds of burning left, and who lit it.
    pub burning: f32,
    pub burn_attacker: Option<u8>,
    /// Flamethrower jetting this tick.
    pub flaming: bool,
    /// Rocket lock: candidate target and lock progress 0..1.
    pub lock: Option<u8>,
    pub lock_progress: f32,
    /// Rockets still to launch in the current salvo (and the timer to the next).
    pub salvo_left: u32,
    pub salvo_timer: f32,
    pub salvo_target: Option<u8>,
    /// Charges of the rare part (mortar rounds, smoke, mines).
    pub rare_ammo: u8,
    pub rare_cooldown: f32,
    pub rare_regen: f32,
    /// Holding an air-strike flare.
    pub air_strike: bool,
    pub prev_special: bool,
    // --- Wave 3.
    /// What it's built on (tanks for players; buggies, technicals, the War Rig for raiders).
    pub vehicle: VehicleKind,
    /// Max-health multiplier (raiders get tougher each wave).
    pub health_mult: f32,
    /// Chassis size relative to a tank (scales the base hit boxes).
    pub hit_scale: Vec3,
    /// Garage chassis class (tanks only).
    pub class: crate::garage::ChassisClass,
    pub prev_horn: bool,
}

/// Hull pose helpers.
#[derive(Clone, Copy, Debug)]
pub struct HullPose {
    pub pos: Vec3,
    pub rot: Quat,
}

impl HullPose {
    pub fn of(phys: &Physics, body: RigidBodyHandle) -> HullPose {
        let b = phys.body(body);
        HullPose { pos: b.translation(), rot: *b.rotation() }
    }
    pub fn point(&self, local: Vec3) -> Vec3 {
        self.pos + self.rot * local
    }
}

pub fn gun_stats(t: &Tuning, stats: &TankStats) -> GunStats {
    use crate::parts::GunKind::*;
    let mut g = match stats.gun {
        Stub => t.guns.stub.clone(),
        HeavyCannon => t.guns.heavy_cannon.clone(),
        BigBore => t.guns.big_bore.clone(),
        TwinBarrel => t.guns.twin_barrel.clone(),
        LongGun => t.guns.long_gun.clone(),
        RigCannons => t.raid.rig_cannon.clone(),
    };
    if stats.gun_mk >= 2 {
        g.damage *= t.guns.mk2_damage;
        g.splash_damage *= t.guns.mk2_damage;
        g.reload *= t.guns.mk2_reload;
    }
    g
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

fn approach(cur: f32, target: f32, max_step: f32) -> f32 {
    cur + (target - cur).clamp(-max_step, max_step)
}

/// Ray vs box in the box's frame: (entry distance, local entry normal).
fn ray_box(o: Vec3, d: Vec3, b: &HitBox, max: f32) -> Option<(f32, Vec3)> {
    let mut tmin = 0.0f32;
    let mut tmax = max;
    let mut n = Vec3::ZERO;
    for axis in 0..3 {
        let (oa, da, ca, ha) = (o[axis], d[axis], b.center[axis], b.half[axis]);
        let lo = ca - ha;
        let hi = ca + ha;
        if da.abs() < 1e-8 {
            if oa < lo || oa > hi {
                return None;
            }
        } else {
            let mut t1 = (lo - oa) / da;
            let mut t2 = (hi - oa) / da;
            let mut sign = -1.0;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
                sign = 1.0;
            }
            if t1 > tmin {
                tmin = t1;
                n = Vec3::ZERO;
                n[axis] = sign;
            }
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some((tmin, n))
}

impl Tank {
    pub fn new(phys: &mut Physics, t: &Tuning, pos: Vec3, yaw: f32) -> Tank {
        Self::new_vehicle(phys, t, pos, yaw, VehicleKind::Tank)
    }

    /// A machine on any chassis.
    pub fn new_vehicle(phys: &mut Physics, t: &Tuning, pos: Vec3, yaw: f32, vehicle: VehicleKind) -> Tank {
        let tt = chassis_of(t, vehicle);
        let scale = Vec3::from(tt.hull_half) / Vec3::from(t.tank.hull_half);
        let rot = Quat::from_rotation_y(yaw);
        let rb = RigidBodyBuilder::dynamic()
            .pose(pose(pos, rot))
            .linear_damping(tt.linear_damping)
            .angular_damping(tt.angular_damping)
            .ccd_enabled(true);
        let body = phys.world.insert_body(rb);
        let h = tt.hull_half;
        phys.world.insert_collider(
            ColliderBuilder::cuboid(h[0], h[1], h[2])
                .density(0.0)
                .friction(tt.hull_friction)
                .collision_groups(tank_groups()),
            Some(body),
        );
        let pv = Vec3::from(t.turret.pivot) * scale;
        phys.world.insert_collider(
            ColliderBuilder::cylinder(0.38 * scale.y, 1.2 * scale.x.min(scale.z))
                .translation(pv + Vec3::Y * 0.38 * scale.y)
                .density(0.0)
                .friction(tt.hull_friction)
                .collision_groups(tank_groups()),
            Some(body),
        );
        let tiers = [0u8; 4];
        let stats = compute_stats_on(t, tt, &[], &tiers);
        let tank = Tank {
            body,
            alive: true,
            health: stats.max_health,
            stats,
            turret_yaw: 0.0,
            gun_pitch: 0.0,
            wheels: vec![WheelState::default(); tt.wheels_per_side * 2],
            track_speed: [0.0; 2],
            tracks: [TrackState { health: t.damage.track_health, blown: 0.0 }; 2],
            tiers,
            branch: [None; 4],
            scrap: [0; 4],
            parts: Vec::new(),
            choice: None,
            fuel: tt.fuel_max,
            boosting: false,
            reload: 0.0,
            alt_cooldown: 0.0,
            charge: 0.0,
            bloom: 0.0,
            repair: None,
            tipped_time: 0.0,
            respawn_in: 0.0,
            protect: t.damage.spawn_protection,
            last_attacker: None,
            rescues: 0,
            twin_side: false,
            prev_alt: false,
            shield: 0.0,
            since_damage: 100.0,
            burning: 0.0,
            burn_attacker: None,
            flaming: false,
            lock: None,
            lock_progress: 0.0,
            salvo_left: 0,
            salvo_timer: 0.0,
            salvo_target: None,
            rare_ammo: 0,
            rare_cooldown: 0.0,
            rare_regen: 0.0,
            air_strike: false,
            prev_special: false,
            vehicle,
            health_mult: 1.0,
            hit_scale: scale,
            class: crate::garage::ChassisClass::Brawler,
            prev_horn: false,
        };
        tank.apply_mass(phys, t);
        tank
    }

    pub fn mass(&self) -> f32 {
        self.stats.mass
    }

    /// Rebuild a track's parts after its tier or branch changed, keeping the
    /// health of parts that stay, then recompute stats and mass.
    pub fn rebuild_track(&mut self, phys: &mut Physics, t: &Tuning, track: Track) {
        let want = parts_for(track, self.tiers[track.index()], self.branch[track.index()]);
        let old: Vec<Part> = self.parts.iter().filter(|p| p.track == track && p.tier > 0).cloned().collect();
        self.parts.retain(|p| p.track != track || p.tier == 0);
        for (kind, tier, mk) in want {
            if !self.class.allows(crate::parts::part_def(kind).slot) {
                continue;
            }
            let mut p = Part::new(t, kind, tier, mk);
            if let Some(o) = old.iter().find(|o| o.kind == kind && o.mk == mk) {
                p.health = o.health.min(p.max_health);
            }
            self.parts.push(p);
        }
        self.refresh_stats(phys, t);
    }

    /// Bolt on a combo or rare part (tier 0: losing it costs no tier). A new rare part
    /// replaces the old one. Returns false if it was already there.
    pub fn add_special(&mut self, phys: &mut Physics, t: &Tuning, kind: PartKind) -> bool {
        if self.parts.iter().any(|p| p.kind == kind) {
            return false;
        }
        if crate::parts::RARE_PARTS.contains(&kind) {
            self.parts.retain(|p| !crate::parts::RARE_PARTS.contains(&p.kind));
            let s = &t.special;
            self.rare_ammo = match kind {
                PartKind::Mortar => s.mortar_ammo,
                PartKind::SmokeLauncher => s.smoke_charges,
                PartKind::MineLayer => s.mine_charges,
                _ => 0,
            };
            self.rare_cooldown = 0.0;
        }
        self.parts.push(Part::new(t, kind, 0, 1));
        self.refresh_stats(phys, t);
        if kind == PartKind::ShieldGenerator {
            self.shield = self.stats.shield_max;
        }
        true
    }

    /// Remove a combo or rare part.
    pub fn remove_special(&mut self, phys: &mut Physics, t: &Tuning, kind: PartKind) {
        self.parts.retain(|p| !(p.kind == kind && p.tier == 0));
        self.refresh_stats(phys, t);
        self.shield = self.shield.min(self.stats.shield_max);
    }

    pub fn refresh_stats(&mut self, phys: &mut Physics, t: &Tuning) {
        let old_max = self.stats.max_health;
        self.stats = compute_stats_on(t, self.chassis(t), &self.parts, &self.tiers);
        if self.vehicle == VehicleKind::Tank {
            let c = self.class_stat(t);
            self.stats.max_health += c.health - t.tank.base_health;
            self.stats.mass += t.tank.base_mass * (c.mass_mult - 1.0);
            self.stats.speed_mult *= c.speed_mult;
            self.stats.traverse_mult *= c.traverse_mult;
            // The Scout comes with nitro plumbed in: it can boost from the start.
            if self.class == crate::garage::ChassisClass::Scout {
                self.stats.can_boost = true;
            }
        }
        self.stats.max_health *= self.health_mult;
        if self.vehicle == VehicleKind::WarRig {
            // Lose an engine: it slows. Lose both: it crawls.
            let engines = self.parts.iter().filter(|p| matches!(p.kind, PartKind::RigEngineLeft | PartKind::RigEngineRight)).count();
            self.stats.speed_mult *= [0.25, 0.6, 1.0][engines.min(2)];
            self.stats.can_boost = false;
        }
        if self.stats.max_health > old_max {
            self.health += self.stats.max_health - old_max;
        }
        self.health = self.health.min(self.stats.max_health);
        self.fuel = self.fuel.min(self.stats.fuel_max);
        self.apply_mass(phys, t);
    }

    fn apply_mass(&self, phys: &mut Physics, t: &Tuning) {
        let m = self.stats.mass;
        let inertia = box_inertia(self.chassis(t), m);
        let props = MassProperties::new(self.stats.com, m, inertia);
        let w = &mut phys.world;
        let b = &mut w.bodies[self.body];
        b.set_additional_mass_properties(props, true);
        b.recompute_mass_properties_from_colliders(&w.colliders);
    }

    /// Garage class numbers for this tank.
    pub fn class_stat(&self, t: &Tuning) -> crate::tuning::ClassStat {
        match self.class {
            crate::garage::ChassisClass::Scout => t.chassis.scout,
            crate::garage::ChassisClass::Brawler => t.chassis.brawler,
            crate::garage::ChassisClass::Hauler => t.chassis.hauler,
        }
    }

    /// Set the Garage class (rebuilds parts and stats).
    pub fn set_class(&mut self, phys: &mut Physics, t: &Tuning, class: crate::garage::ChassisClass) {
        self.class = class;
        for k in crate::map::ScrapKind::ALL {
            self.rebuild_track(phys, t, k);
        }
        self.health = self.stats.max_health;
    }

    /// The chassis tuning this machine drives with.
    pub fn chassis<'a>(&self, t: &'a Tuning) -> &'a TankTuning {
        chassis_of(t, self.vehicle)
    }

    /// Size of this chassis relative to a tank (hull half extents ratio).
    pub fn scale(&self, t: &Tuning) -> Vec3 {
        Vec3::from(self.chassis(t).hull_half) / Vec3::from(t.tank.hull_half)
    }

    /// Turret ring centre, hull frame (scaled with the chassis).
    pub fn pivot(&self, t: &Tuning) -> Vec3 {
        Vec3::from(t.turret.pivot) * self.scale(t)
    }

    /// Gun trunnion, turret frame (scaled with the chassis).
    pub fn gun_pivot(&self, t: &Tuning) -> Vec3 {
        Vec3::from(t.turret.gun_pivot) * self.scale(t)
    }

    /// World origin and rotation of a part frame.
    pub fn frame(&self, hp: &HullPose, t: &Tuning, frame: Frame) -> (Vec3, Quat) {
        match frame {
            Frame::Hull => (hp.pos, hp.rot),
            Frame::Turret => {
                let r = hp.rot * Quat::from_rotation_y(self.turret_yaw);
                (hp.point(self.pivot(t)), r)
            }
            Frame::Gun => {
                let tr = hp.rot * Quat::from_rotation_y(self.turret_yaw);
                let o = hp.point(self.pivot(t)) + tr * self.gun_pivot(t);
                (o, tr * Quat::from_rotation_z(self.gun_pitch))
            }
        }
    }

    pub fn barrel_dir(&self, hp: &HullPose, t: &Tuning) -> Vec3 {
        let (_, r) = self.frame(hp, t, Frame::Gun);
        r * Vec3::X
    }

    pub fn muzzle(&self, hp: &HullPose, t: &Tuning, offset_z: f32) -> Vec3 {
        let (o, r) = self.frame(hp, t, Frame::Gun);
        let g = gun_stats(t, &self.stats);
        o + r * Vec3::new(g.barrel_length, 0.0, offset_z)
    }

    /// Sight point for line-of-sight checks (raised by a sensor mast).
    pub fn eye(&self, hp: &HullPose) -> Vec3 {
        hp.pos + Vec3::Y * self.stats.sight_height
    }

    /// Turret yaw and gun pitch (relative) that put a shell on `aim`.
    pub fn aim_solution(&self, hp: &HullPose, t: &Tuning, aim: Vec3, speed: f32, gravity: f32) -> (f32, f32) {
        let pivot = hp.point(self.pivot(t));
        let local = hp.rot.inverse() * (aim - pivot);
        let yaw = (-local.z).atan2(local.x);
        let tr = hp.rot * Quat::from_rotation_y(yaw);
        let gp = pivot + tr * self.gun_pivot(t);
        let d = aim - gp;
        let horiz = Vec3::new(d.x, 0.0, d.z);
        let dh = horiz.length().max(0.01);
        let dy = d.y;
        let theta = if gravity <= 0.0 {
            dy.atan2(dh)
        } else {
            let v2 = speed * speed;
            let disc = v2 * v2 - gravity * (gravity * dh * dh + 2.0 * dy * v2);
            if disc < 0.0 {
                std::f32::consts::FRAC_PI_4
            } else {
                ((v2 - disc.sqrt()) / (gravity * dh)).atan()
            }
        };
        let world_dir = horiz / dh * theta.cos() + Vec3::Y * theta.sin();
        let l = tr.inverse() * world_dir;
        let pitch = l.y.atan2((l.x * l.x + l.z * l.z).sqrt());
        (yaw, pitch.clamp(t.turret.pitch_min, t.turret.pitch_max))
    }

    /// Rotate the turret and gun toward the aim point at their limited rates.
    /// Returns how far (rad) the turret moved, for bloom.
    pub fn aim(&mut self, hp: &HullPose, t: &Tuning, aim: Vec3, dt: f32) -> f32 {
        let g = gun_stats(t, &self.stats);
        let (yaw, pitch) = self.aim_solution(hp, t, aim, g.shell_speed, 9.81 * g.gravity_scale);
        let rate = t.turret.yaw_rate * self.stats.traverse_mult * g.traverse_mult;
        let err = wrap(yaw - self.turret_yaw);
        let step = err.clamp(-rate * dt, rate * dt);
        self.turret_yaw = wrap(self.turret_yaw + step);
        self.gun_pitch = approach(self.gun_pitch, pitch, t.turret.pitch_rate * dt);
        step.abs()
    }

    /// Suspension, drive, steering, boost, righting. Call once per tick before the physics step.
    pub fn drive(&mut self, phys: &mut Physics, t: &Tuning, input: &Input, dt: f32) -> DriveReport {
        let tt = chassis_of(t, self.vehicle);
        let mut report = DriveReport::default();
        let hp = HullPose::of(phys, self.body);
        let up = hp.rot * Vec3::Y;
        let fwd = hp.rot * Vec3::X;
        let n = tt.wheels_per_side;
        let mass = self.stats.mass;
        let per_wheel = mass / (2 * n) as f32;
        let omega = std::f32::consts::TAU * tt.spring_hz;
        let k = per_wheel * omega * omega;
        let c = 2.0 * tt.damping_ratio * (k * per_wheel).sqrt();
        let ray_len = tt.suspension_travel + tt.wheel_radius;

        // 1. Rays.
        let mut hits: Vec<Option<(Vec3, f32, Vec3)>> = Vec::with_capacity(2 * n);
        for side in 0..2 {
            let z = if side == 0 { -tt.track_half_width } else { tt.track_half_width };
            for i in 0..n {
                let x = (n as f32 - 1.0) * 0.5 * tt.wheel_spacing - i as f32 * tt.wheel_spacing;
                let origin = hp.point(Vec3::new(x, tt.ray_origin_y, z));
                let hit = phys.ray(origin, -up, ray_len, ground_query(), Some(self.body));
                hits.push(hit.and_then(|(_, toi, normal)| {
                    if normal.dot(up) < 0.35 {
                        None
                    } else {
                        Some((origin, toi, normal))
                    }
                }));
            }
        }

        // 2. Springs.
        let body = phys.body_mut(self.body);
        let linvel = body.linvel();
        let angvel = body.angvel();
        let mut total_load = 0.0;
        let mut grounded = 0;
        let mut normal_sum = Vec3::ZERO;
        let mut contact_sum = Vec3::ZERO;
        for (w, hit) in self.wheels.iter_mut().zip(hits.iter()) {
            match hit {
                Some((origin, toi, normal)) => {
                    let comp = (ray_len - toi).max(0.0);
                    let contact = *origin - up * *toi;
                    let v = body.velocity_at_point(contact);
                    let rate = -v.dot(up);
                    let f = (k * comp + c * rate).max(0.0);
                    body.apply_impulse_at_point(up * f * dt, contact, true);
                    w.compression = comp;
                    w.grounded = true;
                    total_load += f;
                    grounded += 1;
                    normal_sum += *normal;
                    contact_sum += contact;
                }
                None => {
                    w.compression = (w.compression - 3.0 * dt).max(0.0);
                    w.grounded = false;
                }
            }
        }

        // 3. Track speeds from input (normalised -1..1), blown tracks dead.
        let throttle = input.throttle.clamp(-1.0, 1.0);
        let steer = input.steer.clamp(-1.0, 1.0);
        let blown = [self.tracks[0].blown > 0.0, self.tracks[1].blown > 0.0];
        let mut fwd_n = throttle * (1.0 - tt.steer_speed_loss * steer.abs());
        let mut turn_n = steer;
        if blown[0] || blown[1] {
            fwd_n *= tt.crippled_speed;
            turn_n *= 0.5;
            if blown[0] && blown[1] {
                fwd_n = 0.0;
                turn_n = 0.0;
            } else {
                // Only one track pulls: the tank swings toward the dead side.
                let pull = tt.crippled_pull * throttle.abs();
                turn_n += if blown[0] { -pull } else { pull };
            }
        }

        // Boost.
        let want_boost = input.boost && self.stats.can_boost && throttle > 0.1;
        if want_boost && (self.boosting || self.fuel >= tt.boost_min_fuel) && self.fuel > 0.0 {
            if !self.boosting {
                report.boost_started = true;
            }
            self.boosting = true;
            self.fuel = (self.fuel - tt.boost_drain * dt).max(0.0);
        } else {
            self.boosting = false;
            self.fuel = (self.fuel + tt.fuel_regen * dt).min(self.stats.fuel_max);
        }

        let mass_factor = (tt.base_mass / mass).powf(tt.mass_speed_exponent);
        let boost = if self.boosting { tt.boost_speed_mult } else { 1.0 };
        let vmax = tt.max_speed * self.stats.speed_mult * mass_factor * boost;
        let vrev = tt.reverse_speed * self.stats.speed_mult * mass_factor;

        let mut vf_report = linvel.dot(fwd);
        let mut yaw_rate = angvel.dot(up);
        if grounded > 0 {
            let gf = grounded as f32 / (2 * n) as f32;
            let nrm = (normal_sum / grounded as f32).normalize_or(Vec3::Y);
            let contact = contact_sum / grounded as f32;
            let f_dir = (fwd - nrm * fwd.dot(nrm)).normalize_or(fwd);
            let l_dir = nrm.cross(f_dir);
            let vf = linvel.dot(f_dir);
            let vl = linvel.dot(l_dir);
            vf_report = vf;
            let target = if fwd_n >= 0.0 { fwd_n * vmax } else { fwd_n * vrev };
            let dv = target - vf;
            let accel_lim = if target.abs() < 0.05 {
                tt.coast_decel
            } else if vf.abs() > 0.5 && target.signum() != vf.signum() {
                tt.brake
            } else if dv.signum() != vf.signum() && vf.abs() > 0.5 {
                // Throttling down toward a slower target in the same direction.
                tt.coast_decel.max(tt.brake * 0.5)
            } else {
                tt.accel * mass_factor * if self.boosting { tt.boost_accel_mult } else { 1.0 }
            };
            let a = (dv / tt.accel_tau).clamp(-accel_lim, accel_lim);
            let traction = tt.traction_mu * total_load;
            let f_long = (mass * a * gf).clamp(-traction, traction);
            let f_lat = (-vl * mass / tt.lateral_tau * gf).clamp(-tt.lateral_mu * total_load, tt.lateral_mu * total_load);
            let lever = contact + up * 0.25;
            body.apply_impulse_at_point((f_dir * f_long + l_dir * f_lat) * dt, lever, true);

            // Yaw: skid steering drives the yaw rate toward its target.
            let speed_frac = (vf.abs() / tt.max_speed).min(1.0);
            let yaw_max = tt.yaw_rate_in_place + (tt.yaw_rate_at_speed - tt.yaw_rate_in_place) * speed_frac;
            // Reversing: holding right still swings the nose right.
            let target_w = -turn_n * yaw_max;
            let w = angvel.dot(nrm);
            yaw_rate = w;
            let iyy = box_inertia(tt, mass).y;
            let alpha = ((target_w - w) / tt.yaw_tau).clamp(-tt.yaw_accel_max, tt.yaw_accel_max);
            body.apply_torque_impulse(nrm * iyy * alpha * gf * dt, true);
        }

        // Track surface speeds (for animation): ground speed +/- yaw.
        let hw = tt.track_half_width;
        if grounded > 0 {
            self.track_speed = [vf_report - yaw_rate * hw, vf_report + yaw_rate * hw];
        } else {
            let cmd = fwd_n * vmax;
            self.track_speed = [cmd + turn_n * 3.0, cmd - turn_n * 3.0];
        }
        report.speed = linvel.length();
        report.yaw_rate = yaw_rate;
        report.grounded = grounded;

        // Righting: tipped for a while and nearly stopped -> crew rocks it back.
        if up.y < tt.tipped_up_y {
            self.tipped_time += dt;
            if self.tipped_time > tt.righting_delay && linvel.length() < 4.0 {
                let yaw = (-fwd.z).atan2(fwd.x);
                let pos = body.translation() + Vec3::Y * 1.8;
                body.set_position(pose(pos, Quat::from_rotation_y(yaw)), true);
                body.set_linvel(Vec3::new(0.0, 1.0, 0.0), true);
                body.set_angvel(Vec3::ZERO, true);
                self.tipped_time = 0.0;
                report.righted = true;
            }
        } else {
            self.tipped_time = 0.0;
        }
        report
    }

    /// All hit locations as (location, frame, box).
    pub fn hit_boxes(&self) -> Vec<(HitLocation, Frame, HitBox)> {
        let mut v = vec![
            (HitLocation::Hull, Frame::Hull, scaled(HULL_BOX, self.hit_scale)),
            (HitLocation::Track(0), Frame::Hull, scaled(TRACK_BOXES[0], self.hit_scale)),
            (HitLocation::Track(1), Frame::Hull, scaled(TRACK_BOXES[1], self.hit_scale)),
            (HitLocation::Turret, Frame::Turret, scaled(TURRET_BOX, self.hit_scale)),
        ];
        let has_gun_part = self.parts.iter().any(|p| p.track == Track::Powder && p.tier > 0);
        if !has_gun_part {
            for b in crate::parts::part_def(PartKind::StubGun).hitboxes {
                v.push((HitLocation::Turret, Frame::Gun, *b));
            }
        }
        for p in &self.parts {
            let def = p.def();
            for b in def.hitboxes {
                v.push((HitLocation::Part(p.kind), def.frame, *b));
            }
        }
        v
    }

    /// Nearest hit box along a ray: (distance, location, world normal).
    pub fn ray_test(&self, hp: &HullPose, t: &Tuning, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, HitLocation, Vec3)> {
        // Bounding sphere first.
        let oc = origin - hp.pos;
        let b = oc.dot(dir);
        let cc = oc.length_squared() - 8.0 * 8.0;
        if cc > 0.0 && b > 0.0 {
            return None;
        }
        if b * b - cc < 0.0 {
            return None;
        }
        let frames = [
            self.frame(hp, t, Frame::Hull),
            self.frame(hp, t, Frame::Turret),
            self.frame(hp, t, Frame::Gun),
        ];
        let mut best: Option<(f32, HitLocation, Vec3)> = None;
        for (loc, frame, bx) in self.hit_boxes() {
            let (fo, fr) = frames[frame as usize];
            let inv = fr.inverse();
            let lo = inv * (origin - fo);
            let ld = inv * dir;
            if let Some((d, n)) = ray_box(lo, ld, &bx, max) {
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, loc, fr * n));
                }
            }
        }
        best
    }

    /// World-space centre of each hit location (for splash).
    pub fn location_centers(&self, hp: &HullPose, t: &Tuning) -> Vec<(HitLocation, Vec3)> {
        let mut v = Vec::new();
        for (loc, frame, bx) in self.hit_boxes() {
            let (fo, fr) = self.frame(hp, t, frame);
            if v.iter().any(|(l, _)| *l == loc) {
                continue;
            }
            v.push((loc, fo + fr * bx.center));
        }
        v
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DriveReport {
    pub speed: f32,
    pub yaw_rate: f32,
    pub grounded: usize,
    pub boost_started: bool,
    pub righted: bool,
}

/// The chassis tuning for a vehicle kind.
pub fn chassis_of(t: &Tuning, v: VehicleKind) -> &TankTuning {
    match v {
        VehicleKind::Tank => &t.tank,
        VehicleKind::Buggy => &t.buggy,
        VehicleKind::Technical => &t.technical,
        VehicleKind::WarRig => &t.war_rig,
    }
}

/// Principal inertia of a hull box (with some height for the turret), kg*m^2.
fn box_inertia(tt: &TankTuning, m: f32) -> Vec3 {
    let d = Vec3::new(tt.hull_half[0] * 2.0, tt.hull_half[1] * 2.0 + 0.6, tt.hull_half[2] * 2.0);
    Vec3::new(d.y * d.y + d.z * d.z, d.x * d.x + d.z * d.z, d.x * d.x + d.y * d.y) * (m / 12.0)
}

fn scaled(b: HitBox, s: Vec3) -> HitBox {
    HitBox { center: b.center * s, half: b.half * s }
}
