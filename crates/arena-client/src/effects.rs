//! Effects: CPU-simulated, GPU-instanced sprites in a stylized, puffy look.
//! Exhaust puffs, dust plumes, track ruts, muzzle flashes, impact dust and
//! sparks, explosions (fireball, smoke, debris), burning wrecks, smoke
//! screens, shell tracers, weld flashes. Fed by simulation events and by
//! the tanks' state each frame.

use std::collections::HashMap;

use arena_core::{Event, PlayerId, PropKind, Weapon};
use glam::{Vec3, Vec4};

use crate::renderer::Light;
use crate::world::RenderWorld;

/// Sprite kinds, matching `fx.wgsl`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Lit, puffy: smoke and dust.
    Puff = 0,
    /// Self-lit soft blob: fire, flashes.
    Glow = 1,
    /// Self-lit streak along its velocity: sparks, tracers.
    Streak = 2,
    /// Dark lit chunk with a hard edge: debris.
    Chunk = 3,
    /// Self-lit fresnel shell: shield bubbles.
    Bubble = 4,
    /// Dark line between two points: cables, ropes.
    Cable = 5,
}

/// One sprite on the GPU. Matches `SpriteIn` in `fx.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Sprite {
    /// xyz centre, w radius.
    pub pos_size: [f32; 4],
    /// rgb colour (emitted for glow kinds), a opacity.
    pub color: [f32; 4],
    /// x kind, y rotation, z seed, w softness/heat.
    pub params: [f32; 4],
    /// Streak direction * length (xyz).
    pub axis: [f32; 4],
}

/// A ground decal strip (track ruts), matching `RutIn` in `fx.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rut {
    /// Segment start (xyz), w half width.
    pub a: [f32; 4],
    /// Segment end (xyz), w opacity.
    pub b: [f32; 4],
}

#[derive(Clone, Copy)]
struct Particle {
    pos: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    size0: f32,
    size1: f32,
    col0: Vec3,
    col1: Vec3,
    alpha: f32,
    kind: Kind,
    rot: f32,
    spin: f32,
    drag: f32,
    /// Upward acceleration (buoyancy, + up) minus gravity.
    lift: f32,
    seed: f32,
    /// Heat 0..1: glow strength over life for fire.
    heat: f32,
}

struct RutMark {
    a: Vec3,
    b: Vec3,
    width: f32,
    age: f32,
}

#[derive(Default)]
struct TankFx {
    exhaust_t: f32,
    dust_t: f32,
    last_rut: [Option<Vec3>; 2],
}

struct Flash {
    pos: Vec3,
    color: Vec3,
    radius: f32,
    age: f32,
    life: f32,
}

pub struct Fx {
    parts: Vec<Particle>,
    ruts: Vec<RutMark>,
    tanks: HashMap<PlayerId, TankFx>,
    flashes: Vec<Flash>,
    /// Burning spots (destroyed tanks): position, time left.
    fires: Vec<(Vec3, f32)>,
    rng: u32,
    timers: HashMap<u32, f32>,
    max_particles: usize,
    /// Where each tank was last seen (for events without a position).
    last_pos: HashMap<PlayerId, Vec3>,
}

const MAX_PARTICLES: usize = 5000;
const MAX_RUTS: usize = 2500;
const RUT_LIFE: f32 = 30.0;

impl Default for Fx {
    fn default() -> Self {
        Self::new()
    }
}

impl Fx {
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            ruts: Vec::new(),
            tanks: HashMap::new(),
            flashes: Vec::new(),
            fires: Vec::new(),
            rng: 0x1234_5678,
            timers: HashMap::new(),
            max_particles: MAX_PARTICLES,
            last_pos: HashMap::new(),
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng & 0xffffff) as f32 / 16777215.0
    }

    fn rand_dir(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.rand() * 2.0 - 1.0, self.rand() * 2.0 - 1.0, self.rand() * 2.0 - 1.0);
            let l = v.length_squared();
            if l > 0.01 && l <= 1.0 {
                return v / l.sqrt();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn(&mut self, kind: Kind, pos: Vec3, vel: Vec3, life: f32, size: (f32, f32), col: (Vec3, Vec3), alpha: f32, drag: f32, lift: f32, heat: f32) {
        if self.parts.len() >= self.max_particles {
            return;
        }
        let rot = self.rand() * 6.28;
        let spin = (self.rand() - 0.5) * 1.5;
        let seed = self.rand();
        let life = life * (0.8 + 0.4 * self.rand());
        self.parts.push(Particle { pos, vel, age: 0.0, life, size0: size.0, size1: size.1, col0: col.0, col1: col.1, alpha, kind, rot, spin, drag, lift, seed, heat });
    }

    fn smoke(&mut self, pos: Vec3, vel: Vec3, size: f32, dark: f32, life: f32) {
        let c = Vec3::new(0.62, 0.58, 0.54).lerp(Vec3::new(0.12, 0.1, 0.09), dark);
        self.spawn(Kind::Puff, pos, vel, life, (size * 0.5, size * 1.6), (c, c * 1.1), 0.85, 1.2, 1.2, 0.0);
    }

    fn dust(&mut self, pos: Vec3, vel: Vec3, size: f32, life: f32) {
        let c = Vec3::new(0.78, 0.58, 0.38);
        self.spawn(Kind::Puff, pos, vel, life, (size * 0.4, size * 1.5), (c, c * 1.08), 0.7, 2.2, 0.25, 0.0);
    }

    fn fire(&mut self, pos: Vec3, vel: Vec3, size: f32, life: f32) {
        self.spawn(Kind::Glow, pos, vel, life, (size, size * 0.4), (Vec3::new(6.0, 3.0, 0.9), Vec3::new(1.5, 0.35, 0.08)), 1.0, 1.5, 2.5, 1.0);
    }

    fn sparks(&mut self, pos: Vec3, normal: Vec3, n: u32, speed: f32) {
        for _ in 0..n {
            let d = (normal + self.rand_dir() * 0.9).normalize_or(Vec3::Y);
            let s = speed * (0.4 + self.rand());
            self.spawn(Kind::Streak, pos, d * s, 0.35, (0.05, 0.02), (Vec3::new(8.0, 5.0, 2.0), Vec3::new(3.0, 0.8, 0.1)), 1.0, 0.6, -14.0, 1.0);
        }
    }

    fn debris(&mut self, pos: Vec3, n: u32, speed: f32, dark: bool) {
        for _ in 0..n {
            let mut d = self.rand_dir();
            d.y = d.y.abs() + 0.4;
            let c = if dark { Vec3::new(0.12, 0.1, 0.09) } else { Vec3::new(0.5, 0.36, 0.24) };
            let s = speed * (0.5 + self.rand());
            let size = 0.12 + 0.22 * self.rand();
            self.spawn(Kind::Chunk, pos, d.normalize() * s, 1.6, (size, size), (c, c), 1.0, 0.2, -14.0, 0.0);
        }
    }

    fn flash(&mut self, pos: Vec3, color: Vec3, radius: f32, life: f32) {
        self.flashes.push(Flash { pos, color, radius, age: 0.0, life });
    }

    fn explosion(&mut self, pos: Vec3, radius: f32) {
        let r = radius.max(1.0);
        for _ in 0..(10.0 + r * 4.0) as u32 {
            let d = self.rand_dir();
            let v = (d + Vec3::Y * 0.6) * r * (1.5 + 2.0 * self.rand());
            let size = r * (0.5 + 0.4 * self.rand());
            self.fire(pos + d * r * 0.3, v, size, 0.55);
        }
        for _ in 0..(8.0 + r * 3.0) as u32 {
            let d = self.rand_dir();
            let v = (d * 1.6 + Vec3::Y * 1.8) * r * 0.5;
            let dark = 0.55 + 0.35 * self.rand();
            self.smoke(pos + d * r * 0.4, v, r * 0.9, dark, 3.5);
        }
        for _ in 0..(6.0 + r * 2.0) as u32 {
            let mut d = self.rand_dir();
            d.y = d.y.abs() * 0.3;
            self.dust(pos - Vec3::Y * 0.3 + d * r * 0.5, d * r * 2.5, r * 0.9, 2.5);
        }
        self.debris(pos, (6.0 + r * 2.0) as u32, 9.0 + r * 1.5, true);
        self.sparks(pos, Vec3::Y, 14, 16.0);
        self.flash(pos + Vec3::Y, Vec3::new(1.0, 0.55, 0.2) * 40.0, r * 6.0 + 6.0, 0.35);
    }

    /// Spawns effects for a simulation event; returns trauma for a camera
    /// at `listener`.
    pub fn event(&mut self, e: &Event, listener: Vec3) -> f32 {
        let near = |p: Vec3, r: f32| (1.0 - p.distance(listener) / r).max(0.0);
        match e {
            Event::Fired { pos, dir, weapon, .. } => {
                let big = matches!(weapon, Weapon::Gun(arena_core::GunKind::BigBore));
                let main = matches!(weapon, Weapon::Gun(_));
                let s = if big { 1.6 } else if main { 1.0 } else { 0.35 };
                self.spawn(Kind::Glow, *pos + *dir * 0.3 * s, *dir * 6.0, 0.09, (0.9 * s, 1.4 * s), (Vec3::new(9.0, 6.0, 2.5), Vec3::new(4.0, 1.4, 0.3)), 1.0, 4.0, 0.0, 1.0);
                if main {
                    for k in 0..6 {
                        let side = self.rand_dir();
                        self.smoke(*pos + *dir * (0.5 + k as f32 * 0.35), *dir * (4.0 + k as f32) + side * 1.5 + Vec3::Y * 0.6, 0.6 * s, 0.15, 1.8);
                    }
                    for _ in 0..5 {
                        let mut d = self.rand_dir();
                        d.y = 0.0;
                        self.dust(*pos - Vec3::Y * 1.6 + d * 0.8, d * 7.0 * s, 0.9 * s, 1.4);
                    }
                    self.flash(*pos, Vec3::new(1.0, 0.7, 0.35) * 30.0 * s, 9.0 * s, 0.12);
                }
                near(*pos, 25.0) * if main { 0.35 } else { 0.05 }
            }
            Event::Tracer { from, to, .. } => {
                let d = *to - *from;
                self.spawn(Kind::Streak, *from + d * 0.5, d.normalize_or_zero() * 0.01, 0.06, (0.05, 0.05), (Vec3::new(8.0, 6.0, 2.0), Vec3::new(8.0, 6.0, 2.0)), 1.0, 0.0, 0.0, d.length());
                self.sparks(*to, -d.normalize_or_zero(), 3, 6.0);
                0.0
            }
            Event::Hit { pos, .. } => {
                self.sparks(*pos, Vec3::Y, 16, 12.0);
                self.smoke(*pos, Vec3::Y * 1.5, 0.9, 0.5, 1.4);
                self.flash(*pos, Vec3::new(1.0, 0.75, 0.4) * 15.0, 6.0, 0.1);
                near(*pos, 18.0) * 0.4
            }
            Event::Impact { pos, normal, .. } => {
                for _ in 0..8 {
                    let d = (*normal * 1.2 + self.rand_dir()).normalize_or(Vec3::Y);
                    self.dust(*pos, d * 6.0, 1.3, 2.2);
                }
                self.debris(*pos, 6, 7.0, false);
                self.sparks(*pos, *normal, 5, 8.0);
                near(*pos, 20.0) * 0.2
            }
            Event::Explosion { pos, radius, .. } => {
                self.explosion(*pos, *radius);
                near(*pos, 45.0) * 0.8
            }
            Event::PartBrokeOff { pos, .. } => {
                self.sparks(*pos, Vec3::Y, 20, 10.0);
                self.smoke(*pos, Vec3::Y * 2.0, 1.2, 0.7, 2.5);
                0.1
            }
            Event::Destroyed { pos, .. } => {
                self.explosion(*pos + Vec3::Y * 0.8, 4.0);
                self.fires.push((*pos, 14.0));
                near(*pos, 60.0)
            }
            Event::HazardExploded { pos, radius } => {
                self.explosion(*pos, radius.max(5.0));
                self.explosion(*pos + Vec3::Y * radius * 0.4, radius * 0.7);
                near(*pos, 80.0)
            }
            Event::ObstacleDestroyed { pos, .. } => {
                for _ in 0..14 {
                    let mut d = self.rand_dir();
                    d.y = d.y.abs();
                    self.dust(*pos + d * 1.5, d * 5.0, 2.2, 3.0);
                }
                self.debris(*pos + Vec3::Y, 14, 9.0, false);
                near(*pos, 40.0) * 0.5
            }
            Event::Launched { pos, .. } => {
                for k in 0..16 {
                    let a = k as f32 / 16.0 * std::f32::consts::TAU;
                    let d = Vec3::new(a.cos(), 0.15, a.sin());
                    self.dust(*pos + d, d * 9.0, 1.4, 1.6);
                }
                self.sparks(*pos, Vec3::Y, 10, 8.0);
                near(*pos, 25.0) * 0.3
            }
            Event::ShieldHit { pos, .. } => {
                for _ in 0..10 {
                    let d = self.rand_dir();
                    self.spawn(Kind::Streak, *pos, d * 7.0, 0.3, (0.05, 0.02), (Vec3::new(2.0, 5.0, 9.0), Vec3::new(0.5, 1.5, 4.0)), 1.0, 1.0, 0.0, 1.0);
                }
                self.flash(*pos, Vec3::new(0.4, 0.8, 1.0) * 14.0, 6.0, 0.15);
                0.05
            }
            Event::ShieldBroken { .. } => 0.1,
            Event::SmokeDeployed { pos, radius, .. } => {
                for _ in 0..12 {
                    let d = self.rand_dir() * Vec3::new(1.0, 0.3, 1.0);
                    let c = Vec3::new(0.85, 0.83, 0.8);
                    let s = radius * 0.3;
                    self.spawn(Kind::Puff, *pos + d * radius * 0.4, d * 5.0 + Vec3::Y, 3.0, (s * 0.5, s * 1.2), (c, c), 0.9, 1.5, 0.1, 0.0);
                }
                0.0
            }
            Event::MineTriggered { pos, .. } => {
                self.explosion(*pos, 3.0);
                near(*pos, 35.0) * 0.6
            }
            Event::CraneHit { pos, speed, .. } => {
                self.sparks(*pos, Vec3::Y, 24, 12.0);
                self.debris(*pos, 6, 6.0, true);
                near(*pos, 30.0) * (speed / 10.0).min(1.0)
            }
            Event::AirStrikeCalled { .. } | Event::BombingRun { .. } => 0.0,
            Event::WaveStarted { .. } => 0.25,
            Event::BossSpawned { pos, .. } => {
                for k in 0..24 {
                    let a = k as f32 / 24.0 * std::f32::consts::TAU;
                    let d = Vec3::new(a.cos(), 0.2, a.sin());
                    self.dust(*pos + d * 6.0, d * 10.0, 3.0, 3.0);
                }
                near(*pos, 120.0) * 0.6
            }
            Event::BossModuleLost { pos, .. } => {
                self.explosion(*pos, 4.5);
                self.debris(*pos, 18, 14.0, true);
                near(*pos, 70.0) * 0.8
            }
            Event::BossCoreExposed { .. } => 0.3,
            Event::BossDefeated { player, .. } => {
                if let Some(p) = self.last_pos.get(player).copied() {
                    self.explosion(p + Vec3::Y * 2.0, 9.0);
                    self.explosion(p + Vec3::new(4.0, 1.0, 2.0), 6.0);
                    self.explosion(p + Vec3::new(-4.0, 1.5, -2.0), 6.0);
                    self.fires.push((p, 30.0));
                    near(p, 150.0) + 0.3
                } else {
                    0.6
                }
            }
            Event::UpgradeApplied { .. } | Event::RepairFinished { .. } => 0.0,
            _ => 0.0,
        }
    }

    /// Bolt-on weld flash at a point (UpgradeApplied, from the game layer
    /// that knows where the tank is).
    pub fn weld(&mut self, pos: Vec3) {
        self.sparks(pos, Vec3::Y, 30, 9.0);
        self.flash(pos, Vec3::new(0.6, 0.8, 1.0) * 25.0, 7.0, 0.4);
    }

    fn every(&mut self, key: u32, period: f32, dt: f32) -> u32 {
        let t = self.timers.entry(key).or_insert(0.0);
        *t += dt;
        let mut n = 0;
        while *t >= period {
            *t -= period;
            n += 1;
        }
        n
    }

    /// Ages everything and runs the continuous emitters for this frame.
    pub fn update(&mut self, dt: f32, world: &RenderWorld) {
        if dt <= 0.0 {
            return;
        }
        for p in &mut self.parts {
            p.age += dt;
            p.vel *= (-p.drag * dt).exp();
            p.vel.y += p.lift * dt;
            p.pos += p.vel * dt;
            p.rot += p.spin * dt;
        }
        // Chunks bounce once and settle on the ground plane they fell to.
        self.parts.retain(|p| p.age < p.life);
        for f in &mut self.flashes {
            f.age += dt;
        }
        self.flashes.retain(|f| f.age < f.life);
        for r in &mut self.ruts {
            r.age += dt;
        }
        self.ruts.retain(|r| r.age < RUT_LIFE);
        if self.ruts.len() > MAX_RUTS {
            let drop = self.ruts.len() - MAX_RUTS;
            self.ruts.drain(..drop);
        }

        for t in &world.tanks {
            self.last_pos.insert(t.player, t.pos);
            if !t.alive {
                continue;
            }
            let hull = t.hull_transform();
            let mut st = self.tanks.remove(&t.player).unwrap_or_default();
            // Exhaust: lazy puffs at idle, chugging under load, flame on boost.
            st.exhaust_t += dt * (2.5 + 9.0 * t.throttle + if t.boosting { 20.0 } else { 0.0 });
            while st.exhaust_t >= 1.0 {
                st.exhaust_t -= 1.0;
                let tip = hull.transform_point3(crate::tank::EXHAUST);
                let up = hull.transform_vector3(Vec3::new(-0.6, 1.0, 0.0)).normalize();
                if t.boosting {
                    self.fire(tip, up * 4.0, 0.35, 0.25);
                }
                let jitter = self.rand_dir() * 0.3;
                self.smoke(tip, up * 2.2 + jitter, 0.35 + 0.25 * t.throttle, 0.55 + 0.3 * t.throttle, 1.6);
            }
            // Dust plumes off the tracks when moving.
            if t.speed > 1.2 {
                st.dust_t += dt * (t.speed * 1.6).min(24.0);
                while st.dust_t >= 1.0 {
                    st.dust_t -= 1.0;
                    for side in [-1.42f32, 1.42] {
                        let p = hull.transform_point3(Vec3::new(-2.6 + self.rand() * 0.6, -0.85, side));
                        let back = hull.transform_vector3(Vec3::new(-1.0, 0.6, 0.0)) * t.speed.min(12.0) * 0.25;
                        let j = self.rand_dir() * 0.6;
                        self.dust(p, back + j + Vec3::Y * 0.8, 0.9 + t.speed * 0.06, 2.4);
                    }
                }
            }
            // Ruts under both tracks.
            for (k, side) in [-1.42f32, 1.42].into_iter().enumerate() {
                let p = hull.transform_point3(Vec3::new(0.0, -0.9, side));
                match st.last_rut[k] {
                    Some(a) if a.distance(p) > 0.8 => {
                        if a.distance(p) < 4.0 {
                            self.ruts.push(RutMark { a, b: p, width: 0.36, age: 0.0 });
                        }
                        st.last_rut[k] = Some(p);
                    }
                    None => st.last_rut[k] = Some(p),
                    _ => {}
                }
            }
            if t.burning {
                let p = hull.transform_point3(Vec3::new(-1.0, 0.8, 0.0));
                let n = self.every(t.player as u32 + 1000, 0.08, dt);
                for _ in 0..n {
                    let j = self.rand_dir() * 0.6;
                    self.fire(p + j, Vec3::Y * 2.5, 0.6, 0.5);
                    self.smoke(p + Vec3::Y, Vec3::Y * 3.0 + j, 1.0, 0.85, 3.0);
                }
            }
            self.tanks.insert(t.player, st);
        }

        // Burning wrecks: fire at the base, a dark column above.
        let wrecks: Vec<Vec3> = world.props.iter().filter(|p| p.kind == PropKind::Wreck).map(|p| p.pos).collect();
        let fires: Vec<Vec3> = self.fires.iter().map(|f| f.0).collect();
        for (i, p) in wrecks.iter().chain(fires.iter()).enumerate() {
            let n = self.every(5000 + i as u32, 0.07, dt);
            for _ in 0..n {
                let j = self.rand_dir() * Vec3::new(0.9, 0.3, 0.9);
                self.fire(*p + Vec3::Y * 0.8 + j, Vec3::Y * 2.2 + j, 0.7, 0.55);
                let dark = 0.8 + 0.15 * self.rand();
                self.smoke(*p + Vec3::Y * 2.0 + j, Vec3::new(1.2, 3.5, 0.4) + j, 1.4, dark, 5.0);
            }
        }
        for f in &mut self.fires {
            f.1 -= dt;
        }
        self.fires.retain(|f| f.1 > 0.0);

        // Flamethrowers: a cone of fire from the nozzle.
        for t in world.tanks.iter().filter(|t| t.alive && world.flaming.contains(&t.player)) {
            let g = t.gun_transform();
            let nozzle = g.transform_point3(Vec3::new(2.05, -0.3, 0.32));
            let fwd = g.transform_vector3(Vec3::X).normalize();
            let n = self.every(7000 + t.player as u32, 0.012, dt);
            for _ in 0..n {
                let j = self.rand_dir() * 0.18;
                let speed = 14.0 + 6.0 * self.rand();
                self.spawn(Kind::Glow, nozzle, (fwd + j) * speed, 0.45, (0.25, 1.4), (Vec3::new(7.0, 3.5, 1.0), Vec3::new(1.6, 0.3, 0.05)), 1.0, 2.2, 3.0, 1.0);
            }
            if self.every(7100 + t.player as u32, 0.05, dt) > 0 {
                self.smoke(nozzle + fwd * 6.0, fwd * 4.0 + Vec3::Y * 2.0, 0.9, 0.75, 1.6);
            }
        }
        // Rocket and mortar trails.
        for (i, s) in world.shells.iter().enumerate() {
            match s.kind {
                arena_core::snapshot::ShotKind::Rocket => {
                    if self.every(8000 + i as u32, 0.02, dt) > 0 {
                        let back = -s.vel.normalize_or_zero();
                        self.fire(s.pos + back * 0.5, back * 3.0, 0.25, 0.15);
                        self.smoke(s.pos + back * 0.8, back * 1.0 + Vec3::Y * 0.3, 0.45, 0.25, 2.2);
                    }
                }
                arena_core::snapshot::ShotKind::MortarShell => {
                    if self.every(8500 + i as u32, 0.05, dt) > 0 {
                        self.smoke(s.pos, Vec3::Y * 0.2, 0.3, 0.3, 1.2);
                    }
                }
                _ => {}
            }
        }
        // Air-strike flares: red smoke pouring from the target until the run.
        for s in &world.strikes {
            let n = self.every(9500 + s.id as u32, 0.05, dt);
            for _ in 0..n {
                let j = self.rand_dir() * 0.3;
                let c = Vec3::new(0.85, 0.12, 0.08);
                self.spawn(Kind::Puff, s.target + Vec3::Y * 0.3, Vec3::new(1.0, 4.0, 0.5) + j, 4.0, (0.5, 2.6), (c, c * 1.3), 0.85, 0.8, 0.6, 0.0);
            }
        }
        // Exposed boss cores: a glowing, smoking hole behind the turret.
        for t in world.tanks.iter().filter(|t| t.alive && world.exposed_cores.contains(&t.player)) {
            let core = t.hull_transform().transform_point3(Vec3::new(-2.0, 1.9, 0.0));
            let n = self.every(9900 + t.player as u32, 0.05, dt);
            for _ in 0..n {
                let j = self.rand_dir() * 0.6;
                self.fire(core + j, Vec3::Y * 3.0 + j, 0.9, 0.5);
                self.smoke(core + Vec3::Y * 1.2 + j, Vec3::Y * 4.0 + j, 1.6, 0.85, 4.0);
                if self.rand() < 0.3 {
                    self.sparks(core, Vec3::Y, 3, 8.0);
                }
            }
            self.flash(core, Vec3::new(1.0, 0.45, 0.15) * 8.0, 10.0, 0.06);
        }
        // The outpost smokes more as it takes damage.
        if let Some((centre, _, health)) = world.raid {
            let damage = (1.0 - health).clamp(0.0, 1.0);
            if damage > 0.15 {
                let n = self.every(9950, 0.4 / damage, dt);
                for _ in 0..n {
                    let j = self.rand_dir() * Vec3::new(4.0, 0.0, 4.0);
                    self.smoke(centre + Vec3::Y * 2.5 + j, Vec3::new(1.0, 3.0, 0.4), 1.8, 0.6 + 0.3 * damage, 5.0);
                    if damage > 0.5 {
                        self.fire(centre + Vec3::Y * 1.0 + j, Vec3::Y * 2.0, 0.8, 0.5);
                    }
                }
            }
        }
        // Smoke screens: keep each cloud filled while it lives.
        for (id, c, r, life) in &world.smokes {
            let n = self.every(9000 + *id as u32, 0.06, dt);
            for _ in 0..n {
                let d = self.rand_dir() * Vec3::new(1.0, 0.4, 1.0) * r * 0.7;
                let c2 = *c + d + Vec3::Y * r * 0.2;
                let s = r * (0.35 + 0.2 * self.rand());
                self.spawn(Kind::Puff, c2, Vec3::Y * 0.3, 2.5, (s, s * 1.3), (Vec3::new(0.82, 0.8, 0.76), Vec3::new(0.9, 0.88, 0.85)), 0.8 * life, 0.5, 0.05, 0.0);
            }
        }
    }

    /// Lights from flashes and fires (the strongest few).
    pub fn lights(&self) -> Vec<Light> {
        self.lights_with(&[])
    }

    /// Lights from flashes and fires plus `extra` (flares), strongest first.
    pub fn lights_with(&self, extra: &[Light]) -> Vec<Light> {
        let mut l: Vec<Light> = self
            .flashes
            .iter()
            .map(|f| {
                let k = 1.0 - f.age / f.life;
                Light { pos: f.pos, radius: f.radius, color: f.color * k * k }
            })
            .collect();
        for f in &self.fires {
            l.push(Light { pos: f.0 + Vec3::Y * 1.5, radius: 9.0, color: Vec3::new(1.0, 0.5, 0.15) * 6.0 });
        }
        l.extend_from_slice(extra);
        l.sort_by(|a, b| b.color.length().partial_cmp(&a.color.length()).unwrap_or(std::cmp::Ordering::Equal));
        l.truncate(8);
        l
    }

    /// This frame's sprites, back to front from `eye`, including shells.
    pub fn sprites(&self, world: &RenderWorld, eye: Vec3, cables: &[(Vec3, Vec3)]) -> Vec<Sprite> {
        let mut out: Vec<(f32, Sprite)> = Vec::with_capacity(self.parts.len() + world.shells.len());
        for p in &self.parts {
            let t = (p.age / p.life).clamp(0.0, 1.0);
            let size = p.size0 + (p.size1 - p.size0) * (1.0 - (1.0 - t) * (1.0 - t));
            let c = p.col0.lerp(p.col1, t);
            // Puffs fade in quickly and out slowly; glows burn out.
            let a = match p.kind {
                Kind::Puff => p.alpha * (t / 0.08).min(1.0) * (1.0 - t).powf(1.4),
                Kind::Glow => p.alpha * (1.0 - t),
                Kind::Streak => p.alpha * (1.0 - t),
                Kind::Chunk => p.alpha * (1.0 - ((t - 0.85) / 0.15).clamp(0.0, 1.0)),
                Kind::Bubble | Kind::Cable => p.alpha,
            };
            let axis = match p.kind {
                Kind::Streak if p.heat > 1.5 => p.vel.normalize_or_zero() * p.heat,
                Kind::Streak => p.vel * 0.03,
                _ => Vec3::ZERO,
            };
            let d = p.pos.distance_squared(eye);
            out.push((
                d,
                Sprite {
                    pos_size: p.pos.extend(size).to_array(),
                    color: c.extend(a).to_array(),
                    params: [p.kind as u32 as f32, p.rot, p.seed, if p.kind == Kind::Glow { p.heat * (1.0 - t) } else { 0.0 }],
                    axis: axis.extend(0.0).to_array(),
                },
            ));
        }
        for s in &world.shells {
            let len = (s.vel.length() * 0.025).clamp(0.5, 3.5);
            let r = if s.big { 0.22 } else { 0.12 };
            out.push((
                s.pos.distance_squared(eye),
                Sprite {
                    pos_size: s.pos.extend(r).to_array(),
                    color: Vec4::new(9.0, 6.0, 2.2, 1.0).to_array(),
                    params: [Kind::Streak as u32 as f32, 0.0, 0.0, 1.0],
                    axis: (s.vel.normalize_or_zero() * len).extend(0.0).to_array(),
                },
            ));
        }
        for t in world.tanks.iter().filter(|t| t.alive) {
            let Some(&(_, shield)) = world.shields.iter().find(|s| s.0 == t.player) else { continue };
            let c = t.pos + Vec3::Y * 0.6;
            let flicker = 0.85 + 0.15 * (world.time * 9.0 + t.player as f32).sin();
            out.push((
                c.distance_squared(eye),
                Sprite {
                    pos_size: c.extend(4.3).to_array(),
                    color: [0.6, 1.4, 2.6, (0.35 + 0.65 * shield) * flicker],
                    params: [Kind::Bubble as u32 as f32, 0.0, t.player as f32 * 0.37, 0.0],
                    axis: [0.0; 4],
                },
            ));
        }
        for (a, b) in cables {
            let mid = (*a + *b) * 0.5;
            out.push((
                mid.distance_squared(eye),
                Sprite {
                    pos_size: mid.extend(0.045).to_array(),
                    color: [0.08, 0.07, 0.06, 1.0],
                    params: [Kind::Cable as u32 as f32, 0.0, 0.0, 0.0],
                    axis: (*b - *a).extend(0.0).to_array(),
                },
            ));
        }
        for (id, p, armed) in &world.mines {
            let blink = if *armed { ((world.time * 3.0 + *id as f32 * 0.31).fract() < 0.25) as u32 as f32 } else { 0.0 };
            if blink > 0.0 {
                let c = *p + Vec3::Y * 0.25;
                out.push((c.distance_squared(eye), Sprite { pos_size: c.extend(0.25).to_array(), color: [8.0, 0.8, 0.3, 1.0], params: [Kind::Glow as u32 as f32, 0.0, 0.0, 1.0], axis: [0.0; 4] }));
            }
        }
        for s in &world.strikes {
            if s.delay > 0.0 {
                let c = s.target + Vec3::Y * 0.4;
                let pulse = 0.7 + 0.3 * (world.time * 12.0).sin();
                out.push((c.distance_squared(eye), Sprite { pos_size: c.extend(0.6).to_array(), color: [9.0, 1.2, 0.5, pulse], params: [Kind::Glow as u32 as f32, 0.0, 0.0, 1.0], axis: [0.0; 4] }));
            }
        }
        out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        out.into_iter().map(|(_, s)| s).collect()
    }

    pub fn ruts(&self) -> Vec<Rut> {
        self.ruts
            .iter()
            .map(|r| {
                let k = 1.0 - r.age / RUT_LIFE;
                Rut { a: r.a.extend(r.width).to_array(), b: r.b.extend(0.55 * k.min(1.0) * k.sqrt()).to_array() }
            })
            .collect()
    }

    /// Caps live particles (quality presets).
    pub fn set_max_particles(&mut self, n: usize) {
        self.max_particles = n.min(MAX_PARTICLES);
    }

    pub fn particle_count(&self) -> usize {
        self.parts.len()
    }
}
