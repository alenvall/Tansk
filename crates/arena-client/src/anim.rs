//! Cartoon juice for the machines: spring-damper state per vehicle that
//! makes them move like toys with weight. Purely visual and client-side:
//! driven from the render view of each vehicle (pose, speed, throttle,
//! parts) and from the sim's events (shots, hits, boosts), and applied as
//! extra transforms when the instances are built.
//!
//! - firing: the barrel slides back and eases home, the muzzle pops
//!   (squash then stretch), the hull rocks back away from the shot on its
//!   suspension (more for the Big Bore) and the turret jolts; the
//!   Land-Ship's deck guns recoil and its hull heels on a broadside;
//! - driving: the hull pitches on throttle and brake, leans out of turns,
//!   bounces with overshoot on bumps and landings;
//! - idle: a gentle engine chug in step with the exhaust puffs;
//! - hits: a squash and a jolt away from the impact, the struck part wobbles;
//! - upgrades: a new part pops in (0 -> ~1.15 -> 1) and the tank hops;
//! - the antenna, the sensor mast and the tesla spire whip on springs;
//! - boost: the tail squats and the nose lifts.
//!
//! Each vehicle kind has its own [`Feel`]: buggies bounce and roll a lot,
//! the fuel tanker's trailer sways behind the cab, the War Rig and the
//! Land-Ship move slowly and heavily.

use arena_core::parts::VehicleKind;
use arena_core::{Event, GunKind, HitLocation, PartKind, PlayerId, Weapon};
use glam::{Mat4, Quat, Vec3};

use crate::tank;
use crate::world::TankView;

const TAU: f32 = std::f32::consts::TAU;

/// A damped spring on one value (`x`, its velocity `v`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Spring {
    pub x: f32,
    pub v: f32,
}

impl Spring {
    /// Steps towards `target` at natural frequency `hz` and damping ratio
    /// `zeta` (substepped, so it stays stable on long frames).
    pub fn step(&mut self, target: f32, hz: f32, zeta: f32, dt: f32) {
        let w = TAU * hz;
        let n = ((dt * w / 0.4).ceil() as u32).clamp(1, 16);
        let h = dt / n as f32;
        for _ in 0..n {
            let a = -w * w * (self.x - target) - 2.0 * zeta * w * self.v;
            self.v += a * h;
            self.x += self.v * h;
        }
    }

    fn settled(&self, target: f32) -> bool {
        (self.x - target).abs() < 0.002 && self.v.abs() < 0.02
    }
}

/// How a vehicle kind moves on its springs.
#[derive(Clone, Copy, Debug)]
pub struct Feel {
    /// Pitch per m/s^2 of fore-aft acceleration, roll per m/s^2 sideways.
    pitch_gain: f32,
    roll_gain: f32,
    /// The most it tips, rad.
    tilt_max: f32,
    /// Rock and bob springs: frequency (Hz) and damping.
    rock_hz: f32,
    rock_zeta: f32,
    bob_hz: f32,
    bob_zeta: f32,
    /// Impulses from shots, hits and landings are scaled by this (mass).
    kick: f32,
    /// Squash multiplier.
    squash: f32,
    /// Height of the rock pivot (the ground under the hull), hull frame.
    pivot_y: f32,
    /// Engine chug amplitude, m.
    chug: f32,
}

impl Feel {
    pub fn of(kind: VehicleKind) -> Feel {
        let tank = Feel { pitch_gain: 0.007, roll_gain: 0.009, tilt_max: 0.07, rock_hz: 1.9, rock_zeta: 0.32, bob_hz: 2.6, bob_zeta: 0.3, kick: 1.0, squash: 1.0, pivot_y: -tank::HULL_Y + 0.15, chug: 0.008 };
        match kind {
            VehicleKind::Tank => tank,
            VehicleKind::Buggy => Feel { pitch_gain: 0.016, roll_gain: 0.03, tilt_max: 0.16, rock_hz: 1.6, rock_zeta: 0.2, bob_hz: 1.9, bob_zeta: 0.18, kick: 1.6, squash: 1.4, pivot_y: -0.75, chug: 0.012 },
            VehicleKind::Technical => Feel { pitch_gain: 0.012, roll_gain: 0.022, tilt_max: 0.12, rock_hz: 1.6, rock_zeta: 0.24, bob_hz: 2.0, bob_zeta: 0.22, kick: 1.3, squash: 1.2, pivot_y: -0.9, chug: 0.01 },
            VehicleKind::FuelTanker => Feel { pitch_gain: 0.006, roll_gain: 0.014, tilt_max: 0.07, rock_hz: 1.3, rock_zeta: 0.28, bob_hz: 1.7, bob_zeta: 0.28, kick: 0.7, squash: 0.6, pivot_y: -1.3, chug: 0.01 },
            VehicleKind::WarRig => Feel { pitch_gain: 0.004, roll_gain: 0.007, tilt_max: 0.045, rock_hz: 0.9, rock_zeta: 0.35, bob_hz: 1.1, bob_zeta: 0.35, kick: 0.35, squash: 0.35, pivot_y: -2.2, chug: 0.02 },
            VehicleKind::LandShip => Feel { pitch_gain: 0.002, roll_gain: 0.004, tilt_max: 0.03, rock_hz: 0.45, rock_zeta: 0.4, bob_hz: 0.6, bob_zeta: 0.4, kick: 0.2, squash: 0.15, pivot_y: -3.0, chug: 0.015 },
        }
    }
}

/// How hard a weapon kicks the machine that fires it (1 = Heavy Cannon).
pub fn kick(weapon: Weapon) -> f32 {
    match weapon {
        Weapon::Gun(GunKind::Stub) => 0.6,
        Weapon::Gun(GunKind::HeavyCannon) => 1.0,
        Weapon::Gun(GunKind::BigBore) => 1.7,
        Weapon::Gun(GunKind::TwinBarrel) => 0.75,
        Weapon::Gun(GunKind::LongGun) => 1.15,
        Weapon::Gun(GunKind::DeckGun) => 2.5,
        Weapon::Gun(GunKind::RigCannons) => 2.0,
        Weapon::Special(PartKind::ShipMortar) => 1.2,
        _ => 0.35,
    }
}

/// The machine an event animates, if any.
pub fn subject(e: &Event) -> Option<PlayerId> {
    match e {
        Event::Fired { player, .. } | Event::BoostStarted { player } | Event::PartAttached { player, .. } => Some(*player),
        Event::Hit { target, .. } => Some(*target),
        _ => None,
    }
}

/// A machine's animated frames this frame, for the effects to spawn from
/// (muzzle flashes and exhaust puffs ride the rocking body, not the raw
/// sim pose).
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    /// The juiced hull frame.
    pub body: Mat4,
    /// The juiced gun frame (tanks; the turret frame for other vehicles).
    pub gun: Mat4,
    /// The main gun's muzzle tip and its direction, world space.
    pub muzzle: Vec3,
    pub muzzle_dir: Vec3,
}

/// One machine's juice.
#[derive(Clone, Debug, Default)]
pub struct Juice {
    init: bool,
    kind: VehicleKind,
    /// Hull rock: pitch (+ = nose up) and roll (+ = top leans to +Z), rad.
    pitch: Spring,
    roll: Spring,
    /// Hull bob, m (+ = up).
    heave: Spring,
    /// Hit jolt slide in the hull frame, m.
    slide_x: Spring,
    slide_z: Spring,
    /// Squash (+ = flattened), fraction.
    squash: Spring,
    /// Gun slide back, m, and the muzzle pop (+ = fat and short).
    recoil: Spring,
    pop: Spring,
    turret_jolt: Spring,
    /// Whip tilts (about Z, about X) of the antenna and the masts.
    antenna: [Spring; 2],
    mast: [Spring; 2],
    /// The fuel tanker's trailer roll, lagging the cab, rad.
    trailer: Spring,
    /// Deck guns sliding back after a shot: (gun, recoil m).
    deck_recoil: Vec<(PartKind, Spring)>,
    prev_pos: Vec3,
    prev_vel: Vec3,
    prev_comp: f32,
    hull: Mat4,
    /// The engine's chug phase (cycles), its amplitude, the clock.
    chug: f32,
    chug_amp: f32,
    clock: f32,
    parts: Vec<PartKind>,
    /// New parts popping in: (part, scale spring heading for 1).
    pops: Vec<(PartKind, Spring)>,
    /// Struck parts wobbling: (part, wobble spring heading for 0).
    wobbles: Vec<(PartKind, Spring)>,
}

impl Juice {
    fn feel(&self) -> Feel {
        Feel::of(self.kind)
    }

    /// Advances the springs by `dt` from the machine's latest view. With a
    /// zero `dt` (paused, line-up shots) nothing moves. `comp` is the mean
    /// suspension compression (m) of its wheels.
    pub fn update(&mut self, t: &TankView, kind: VehicleKind, comp: f32, dt: f32) {
        self.hull = t.hull_transform();
        if !self.init || self.kind != kind {
            *self = Juice { init: true, kind, prev_pos: t.pos, prev_comp: comp, hull: self.hull, parts: t.parts.clone(), ..Default::default() };
            return;
        }
        let f = self.feel();
        // New parts pop in and the whole tank hops (tanks only: boss
        // modules only ever come off).
        if kind == VehicleKind::Tank {
            for &p in &t.parts {
                if !self.parts.contains(&p) {
                    self.pops.retain(|(q, _)| *q != p);
                    self.pops.push((p, Spring::default()));
                    self.heave.v += 1.6;
                    self.squash.v -= 1.5;
                }
            }
        }
        self.parts = t.parts.clone();
        if dt <= 0.0 {
            return;
        }
        let mut vel = (t.pos - self.prev_pos) / dt;
        if vel.length() > 60.0 {
            vel = self.prev_vel; // a respawn or a teleport
        }
        let acc = ((vel - self.prev_vel) / dt).clamp_length_max(80.0);
        let local = t.rot.inverse() * acc;
        // Landings: a hard stop of a fall slams the hull down.
        let dvy = vel.y - self.prev_vel.y;
        if self.prev_vel.y < -2.5 && dvy > 2.0 {
            self.heave.v -= (dvy * 0.45 * f.kick).min(3.5);
            self.squash.v += (dvy * 0.6 * f.squash).min(4.5);
            self.pitch.v += (dvy * 0.04 * f.kick).min(0.4) * if t.speed >= 0.0 { -1.0 } else { 1.0 };
        }
        // Bumps: the suspension compressing fast kicks the hull up a bit.
        let dc = (comp - self.prev_comp) / dt;
        if dc.abs() > 0.4 {
            self.heave.v += (dc * 0.12 * f.kick).clamp(-0.6, 0.6);
            self.roll.v += (dc * 0.05 * f.kick * (self.clock * 13.7).sin()).clamp(-0.2, 0.2);
        }
        self.prev_pos = t.pos;
        self.prev_vel = vel;
        self.prev_comp = comp;

        // Throttle and brake pitch, lean out of turns, boost squat.
        let pitch_target = (local.x * f.pitch_gain).clamp(-f.tilt_max, f.tilt_max) + if t.boosting { 0.06 * f.kick.min(1.0) } else { 0.0 };
        let roll_target = (-local.z * f.roll_gain).clamp(-f.tilt_max, f.tilt_max);
        self.pitch.step(pitch_target, f.rock_hz, f.rock_zeta, dt);
        self.roll.step(roll_target, f.rock_hz * 1.1, f.rock_zeta, dt);
        self.heave.step(0.0, f.bob_hz, f.bob_zeta, dt);
        self.slide_x.step(0.0, 3.5, 0.45, dt);
        self.slide_z.step(0.0, 3.5, 0.45, dt);
        self.squash.step(0.0, 4.5, 0.28, dt);
        self.recoil.step(0.0, 2.6, 0.95, dt);
        self.pop.step(0.0, 6.5, 0.25, dt);
        self.turret_jolt.step(0.0, 4.0, 0.3, dt);
        // The trailer leans further out of a turn than the cab, late.
        self.trailer.step(roll_target * 1.6, 0.9, 0.22, dt);
        // The whips trail the hull's motion.
        let whip = [(local.x * 0.012 + self.pitch.v * 0.08).clamp(-0.35, 0.35), (-local.z * 0.012 - self.roll.v * 0.08).clamp(-0.35, 0.35)];
        for k in 0..2 {
            self.antenna[k].step(whip[k] * 1.4, 2.4, 0.1, dt);
            self.mast[k].step(whip[k], 1.6, 0.14, dt);
        }
        for (_, s) in &mut self.pops {
            s.step(1.0, 2.6, 0.38, dt);
        }
        self.pops.retain(|(_, s)| !s.settled(1.0));
        for (_, s) in &mut self.wobbles {
            s.step(0.0, 5.0, 0.18, dt);
        }
        self.wobbles.retain(|(_, s)| !s.settled(0.0));
        for (_, s) in &mut self.deck_recoil {
            s.step(0.0, 1.2, 0.9, dt);
        }
        self.deck_recoil.retain(|(_, s)| !s.settled(0.0));
        // The engine chugs in step with the exhaust puffs (see effects).
        let rate = 2.5 + 9.0 * t.throttle + if t.boosting { 20.0 } else { 0.0 };
        self.chug = (self.chug + dt * rate).fract();
        self.chug_amp = f.chug * (1.0 + 1.25 * t.throttle.clamp(0.0, 1.0));
        self.clock += dt;
    }

    /// Reacts to a sim event about this machine.
    pub fn event(&mut self, e: &Event) {
        let f = self.feel();
        let inv = self.hull.inverse();
        match e {
            Event::Fired { weapon, pos, dir, .. } => {
                let k = kick(*weapon) * f.kick;
                // The shot's direction in the hull frame, flattened.
                let d = (inv.transform_vector3(*dir) * Vec3::new(1.0, 0.0, 1.0)).normalize_or(Vec3::X);
                if self.kind == VehicleKind::Tank && matches!(weapon, Weapon::Gun(_)) {
                    self.recoil.x = 0.22 * kick(*weapon);
                    self.recoil.v = 0.0;
                    self.pop.x = 0.9;
                    self.pop.v = 0.0;
                }
                if *weapon == Weapon::Gun(GunKind::DeckGun) {
                    // The gun nearest the muzzle slides back.
                    let local = inv.transform_point3(*pos);
                    let gun = arena_core::parts::SHIP_GUNS.iter().min_by(|a, b| a.1.distance(local).total_cmp(&b.1.distance(local))).map(|g| g.0);
                    if let Some(g) = gun {
                        self.deck_recoil.retain(|(q, _)| *q != g);
                        self.deck_recoil.push((g, Spring { x: 1.1, v: 0.0 }));
                    }
                }
                // The hull rocks back away from the shot and squats.
                self.pitch.v += d.x * 0.75 * k;
                self.roll.v -= d.z * 0.75 * k;
                self.heave.v -= 0.35 * k;
                self.squash.v += 0.8 * k * f.squash;
                let side = if (self.clock * 7.3).fract() > 0.5 { 1.0 } else { -1.0 };
                self.turret_jolt.v += side * 0.35 * k;
                for s in self.antenna.iter_mut().chain(self.mast.iter_mut()) {
                    s.v -= d.x * 0.9 * k;
                }
                self.antenna[1].v += d.z * 0.9 * k;
                self.trailer.v -= d.z * 0.6 * k;
            }
            Event::Hit { pos, damage, location, .. } => {
                let s = (damage / 40.0).clamp(0.3, 1.6) * f.kick;
                let p = inv.transform_point3(*pos);
                let d = Vec3::new(p.x, 0.0, p.z).normalize_or(Vec3::X);
                // Shoved away from the impact, the top tipping away, squashed.
                self.slide_x.v -= d.x * 1.6 * s;
                self.slide_z.v -= d.z * 1.6 * s;
                self.pitch.v += d.x * 0.5 * s;
                self.roll.v -= d.z * 0.5 * s;
                self.squash.v += 2.2 * s * f.squash;
                self.trailer.v -= d.z * 0.8 * s;
                for sp in self.antenna.iter_mut() {
                    sp.v += d.x * 1.2 * s;
                }
                if let HitLocation::Part(part) = location {
                    match self.wobbles.iter_mut().find(|(q, _)| q == part) {
                        Some((_, w)) => w.v += 6.0 * s,
                        None => self.wobbles.push((*part, Spring { x: 0.0, v: 6.0 * s })),
                    }
                }
            }
            Event::BoostStarted { .. } => {
                self.pitch.v += 0.5 * f.kick.min(1.0);
                self.heave.v += 0.4 * f.kick.min(1.0);
            }
            _ => {}
        }
    }

    /// The juiced hull transform for the raw one (the wheels stay on the
    /// raw hull, so the body rocks over its running gear).
    pub fn hull(&self, raw: Mat4) -> Mat4 {
        let f = self.feel();
        let pulse = (self.chug * TAU).sin().max(0.0).powi(2);
        let chug = self.chug_amp * pulse;
        let buzz = 0.0025 * (self.clock * TAU * 23.0).sin() * self.chug_amp / 0.018;
        let pivot = Vec3::new(0.0, f.pivot_y, 0.0);
        let sq = self.squash.x.clamp(-0.25, 0.25);
        let scale = Vec3::new(1.0 + 0.45 * sq, 1.0 - sq, 1.0 + 0.45 * sq);
        let lim = (f.tilt_max * 4.0).min(0.3);
        let rot = Quat::from_rotation_z(self.pitch.x.clamp(-lim, lim) + buzz) * Quat::from_rotation_x(self.roll.x.clamp(-lim, lim));
        raw * Mat4::from_translation(Vec3::new(self.slide_x.x, self.heave.x.clamp(-0.5, 0.8) + chug, self.slide_z.x))
            * Mat4::from_translation(pivot)
            * Mat4::from_scale_rotation_translation(scale, rot, Vec3::ZERO)
            * Mat4::from_translation(-pivot)
    }

    /// The juiced turret transform on a juiced hull.
    pub fn turret(&self, hull: Mat4, yaw: f32) -> Mat4 {
        hull * tank::turret_local(yaw + self.turret_jolt.x.clamp(-0.2, 0.2))
    }

    /// The juiced gun transform: the barrel slid back and the muzzle pop
    /// (fat and short, then thin and long, about the breech).
    pub fn gun(&self, turret: Mat4, pitch: f32) -> Mat4 {
        let p = self.pop.x.clamp(-0.6, 1.0);
        let scale = Vec3::new(1.0 - 0.07 * p, 1.0 + 0.22 * p, 1.0 + 0.22 * p);
        turret * tank::gun_local(pitch) * Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, Vec3::X * -self.recoil.x.clamp(0.0, 0.6))
    }

    /// The turret antenna (its foot at `foot` in the turret frame), whipping.
    pub fn antenna(&self, turret: Mat4, foot: Vec3) -> Mat4 {
        turret * Mat4::from_rotation_translation(Quat::from_rotation_z(self.antenna[0].x) * Quat::from_rotation_x(self.antenna[1].x), foot)
    }

    /// The fuel tanker's trailer on the juiced hull: it rolls about its own
    /// axis, late and further than the cab.
    pub fn trailer(&self, body: Mat4, axis_y: f32) -> Mat4 {
        let pivot = Vec3::new(0.0, axis_y, 0.0);
        body * Mat4::from_translation(pivot) * Mat4::from_quat(Quat::from_rotation_x((self.trailer.x - self.roll.x).clamp(-0.12, 0.12))) * Mat4::from_translation(-pivot)
    }

    /// How far a deck gun's barrel has slid back (m, its own frame).
    pub fn deck_recoil(&self, gun: PartKind) -> f32 {
        self.deck_recoil.iter().find(|(q, _)| *q == gun).map_or(0.0, |(_, s)| s.x.clamp(0.0, 1.5))
    }

    /// A part on its (juiced) frame: popping in, wobbling when struck, the
    /// masts whipping about their feet.
    pub fn part(&self, p: PartKind, frame: Mat4) -> Mat4 {
        let mut xf = frame;
        let foot = match p {
            PartKind::SensorMast => Some(Vec3::new(-2.4, 0.55, -1.1)),
            PartKind::TeslaSpire => Some(Vec3::new(-2.4, 0.55, 1.1)),
            PartKind::RigMast => Some(Vec3::new(-2.5, 2.0, -2.6)),
            _ => None,
        };
        if let Some(b) = foot {
            let tilt = Quat::from_rotation_z(self.mast[0].x) * Quat::from_rotation_x(self.mast[1].x);
            xf = xf * Mat4::from_translation(b) * Mat4::from_quat(tilt) * Mat4::from_translation(-b);
        }
        let pop = self.pops.iter().find(|(q, _)| *q == p).map(|(_, s)| s.x.max(0.0));
        let wob = self.wobbles.iter().find(|(q, _)| *q == p).map(|(_, s)| s.x);
        if pop.is_none() && wob.is_none() {
            return xf;
        }
        let boxes = arena_core::part_def(p).hitboxes;
        let c = boxes.iter().map(|b| b.center).sum::<Vec3>() / boxes.len().max(1) as f32;
        let s = pop.unwrap_or(1.0);
        let w = wob.unwrap_or(0.0).clamp(-1.5, 1.5);
        let rot = Quat::from_rotation_z(w * 0.07) * Quat::from_rotation_x(w * 0.05);
        let scale = Vec3::new(s * (1.0 - 0.04 * w), s * (1.0 + 0.06 * w), s * (1.0 - 0.04 * w));
        xf * Mat4::from_translation(c) * Mat4::from_scale_rotation_translation(scale, rot, Vec3::ZERO) * Mat4::from_translation(-c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> TankView {
        crate::world::parked_tank(1, tank::RUSTBACK, Vec3::ZERO, 0.0, PartKind::BigBore, vec![])
    }

    #[test]
    fn springs_settle_and_overshoot() {
        let mut s = Spring { x: 0.0, v: 0.0 };
        let mut peak = 0.0f32;
        for _ in 0..240 {
            s.step(1.0, 2.6, 0.38, 1.0 / 60.0);
            peak = peak.max(s.x);
        }
        assert!(peak > 1.08 && peak < 1.3, "overshoot to {peak}");
        assert!(s.settled(1.0));
        // A huge frame stays stable.
        s.step(0.0, 6.5, 0.25, 0.5);
        assert!(s.x.is_finite() && s.x.abs() < 2.0);
    }

    #[test]
    fn firing_kicks_and_returns_to_rest() {
        let mut j = Juice::default();
        let mut t = view();
        j.update(&t, VehicleKind::Tank, 0.15, 1.0 / 60.0);
        j.event(&Event::Fired { player: 1, weapon: Weapon::Gun(GunKind::BigBore), pos: Vec3::ZERO, dir: Vec3::X });
        let mut rocked = 0.0f32;
        for _ in 0..240 {
            j.update(&t, VehicleKind::Tank, 0.15, 1.0 / 60.0);
            rocked = rocked.max(j.pitch.x.abs());
        }
        assert!(rocked > 0.03, "big bore rocks the hull ({rocked})");
        assert!(j.recoil.x.abs() < 0.01 && j.pitch.x.abs() < 0.01);
        // A new part pops in with an overshoot.
        t.parts.push(PartKind::FlankPlates);
        let mut peak = 0.0f32;
        for _ in 0..120 {
            j.update(&t, VehicleKind::Tank, 0.15, 1.0 / 60.0);
            if let Some((_, s)) = j.pops.first() {
                peak = peak.max(s.x);
            }
        }
        assert!(peak > 1.08 && peak < 1.3, "pop overshoot {peak}");
        assert!(j.pops.is_empty());
    }

    #[test]
    fn heavy_machines_rock_less_and_slower() {
        let fire = Event::Fired { player: 1, weapon: Weapon::Gun(GunKind::HeavyCannon), pos: Vec3::ZERO, dir: Vec3::X };
        let peak = |kind: VehicleKind| {
            let mut j = Juice::default();
            let t = view();
            j.update(&t, kind, 0.15, 1.0 / 60.0);
            j.event(&fire);
            let (mut peak, mut at) = (0.0f32, 0);
            for k in 0..600 {
                j.update(&t, kind, 0.15, 1.0 / 60.0);
                if j.pitch.x.abs() > peak {
                    peak = j.pitch.x.abs();
                    at = k;
                }
            }
            (peak, at)
        };
        let (buggy, tb) = peak(VehicleKind::Buggy);
        let (tank, _) = peak(VehicleKind::Tank);
        let (ship, ts) = peak(VehicleKind::LandShip);
        assert!(buggy > tank && tank > ship, "{buggy} {tank} {ship}");
        assert!(ts > tb, "the ship sways slower ({ts} vs {tb} frames to peak)");
    }

    #[test]
    fn at_rest_the_pose_is_the_raw_pose() {
        let mut j = Juice::default();
        let t = view();
        j.update(&t, VehicleKind::Tank, 0.15, 0.0);
        j.update(&t, VehicleKind::Tank, 0.15, 0.0);
        let raw = t.hull_transform();
        assert!(j.hull(raw).abs_diff_eq(raw, 1e-5));
        assert!(j.gun(raw, 0.1).abs_diff_eq(raw * tank::gun_local(0.1), 1e-5));
    }
}
