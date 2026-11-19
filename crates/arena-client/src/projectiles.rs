//! Projectiles in 3D: low-poly models for every round in flight, turned to
//! their velocity and scaled up a little so they read at play distance (the
//! tracers and trails from the effects stay on top). A stubby brass-and-
//! steel shell with a burning tracer base, a rocket with fins and a
//! flickering exhaust, a finned mortar bomb, the harpoon, and the coilgun's
//! bolt: a glowing slug crackling with an arc, flown along the hitscan line.

use arena_core::snapshot::ShotKind;
use arena_core::{AltKind, Event, Weapon};
use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{MeshBuilder, Paint};
use crate::renderer::{Instance, Renderer};
use crate::shield::{GlassInstance, Shell, Style};
use crate::world::{Library, RenderWorld};

/// Readability scale over true size.
const SHELL_SCALE: f32 = 1.8;
const ROCKET_SCALE: f32 = 1.5;
const MORTAR_SCALE: f32 = 1.6;
/// The coil bolt's speed along its line, m/s.
const BOLT_SPEED: f32 = 520.0;

#[derive(Clone, Copy)]
struct Bolt {
    from: Vec3,
    to: Vec3,
    born: f32,
}

/// Client-side projectile visuals (coil bolts live only here).
#[derive(Default)]
pub struct Projectiles {
    bolts: Vec<Bolt>,
    clock: f32,
}

impl Projectiles {
    pub fn event(&mut self, e: &Event) {
        if let Event::Tracer { weapon: Weapon::Alt(AltKind::Coilgun), from, to, .. } = e {
            if self.bolts.len() < 32 {
                self.bolts.push(Bolt { from: *from, to: *to, born: self.clock });
            }
        }
    }

    /// Queues this frame's rounds: models into `instances`, glows into `glass`.
    pub fn draw(&mut self, r: &mut Renderer, lib: &mut Library, instances: &mut Vec<Instance>, world: &RenderWorld) {
        self.clock = world.time;
        let t = world.time;
        for (i, s) in world.shells.iter().enumerate() {
            let dir = s.vel.normalize_or(Vec3::X);
            let aim = Quat::from_rotation_arc(Vec3::X, dir);
            let jitter = hash(i as f32 * 1.7 + t * 61.0);
            match s.kind {
                ShotKind::Shell => {
                    let id = lib.get(r, "shot:shell", shell);
                    let k = SHELL_SCALE * if s.big { 1.45 } else { 1.0 };
                    let spin = Quat::from_rotation_x(t * 30.0 + i as f32);
                    instances.push(Instance::new(id, Mat4::from_scale_rotation_translation(Vec3::splat(k), aim * spin, s.pos)));
                }
                ShotKind::Rocket => {
                    let id = lib.get(r, "shot:rocket3d", rocket);
                    let roll = Quat::from_rotation_x(t * 9.0 + i as f32);
                    let xf = Mat4::from_scale_rotation_translation(Vec3::splat(ROCKET_SCALE), aim * roll, s.pos);
                    instances.push(Instance::new(id, xf));
                    // The exhaust: a flickering cone of flame out the back.
                    let flame = Mat4::from_scale_rotation_translation(Vec3::new(0.9 + 0.6 * jitter, 0.85 + 0.3 * jitter, 0.85 + 0.3 * jitter), Quat::IDENTITY, Vec3::ZERO);
                    let fid = lib.get(r, "shot:flame", flame_cone);
                    instances.push(Instance::new(fid, xf * Mat4::from_translation(Vec3::X * -0.55) * flame));
                    let glow = Mat4::from_scale_rotation_translation(Vec3::new(0.3, 0.12, 0.12) * ROCKET_SCALE, aim, s.pos - dir * 1.0);
                    r.glass.push(Shell::Sphere, GlassInstance::new(glow, Style::Glow, Vec3::new(1.6, 0.6, 0.12) * (0.8 + 0.4 * jitter), 1.0, 1.0, 0.0));
                }
                ShotKind::MortarShell => {
                    let id = lib.get(r, "shot:mortar3d", mortar);
                    instances.push(Instance::new(id, Mat4::from_scale_rotation_translation(Vec3::splat(MORTAR_SCALE), aim, s.pos)));
                }
                ShotKind::Harpoon => {
                    let id = lib.get(r, "shot:harpoon", crate::props::harpoon);
                    instances.push(Instance::new(id, Mat4::from_scale_rotation_translation(Vec3::splat(1.3), aim, s.pos)));
                }
            }
        }
        // Coil bolts: the slug runs the line, an arc crackling round it,
        // a fading ionised wake behind.
        self.bolts.retain(|b| {
            let len = b.from.distance(b.to);
            t - b.born < len / BOLT_SPEED + 0.25 && t >= b.born
        });
        for (k, b) in self.bolts.iter().enumerate() {
            let line = b.to - b.from;
            let len = line.length().max(0.1);
            let dir = line / len;
            let age = t - b.born;
            let run = (age * BOLT_SPEED).min(len);
            let head = b.from + dir * run;
            let arrived = age * BOLT_SPEED >= len;
            let fade = if arrived { 1.0 - ((age - len / BOLT_SPEED) / 0.25).clamp(0.0, 1.0) } else { 1.0 };
            let aim = Quat::from_rotation_arc(Vec3::X, dir);
            let blue = Vec3::new(0.55, 0.85, 1.6);
            if !arrived {
                let slug = Mat4::from_scale_rotation_translation(Vec3::new(0.9, 0.22, 0.22), aim, head);
                r.glass.push(Shell::Sphere, GlassInstance::new(slug, Style::Glow, blue * 6.0, 1.0, 1.0, 0.0));
            }
            // The wake: a long thin glow from the muzzle to the slug.
            let wake_len = run.min(30.0);
            let mid = head - dir * wake_len * 0.5;
            let wake = Mat4::from_scale_rotation_translation(Vec3::new(wake_len * 0.5, 0.06, 0.06), aim, mid);
            r.glass.push(Shell::Sphere, GlassInstance::new(wake, Style::Glow, blue * 2.5 * fade, 1.0, 1.0, 0.0));
            // The arc: a jagged chain round the slug, re-struck every frame.
            let side = dir.any_orthonormal_vector();
            let up = dir.cross(side);
            let mut prev = head;
            for j in 0..6 {
                let h = hash(t * 97.0 + k as f32 * 13.0 + j as f32 * 3.1);
                let h2 = hash(t * 53.0 + k as f32 * 7.0 + j as f32 * 5.7);
                let a = h * std::f32::consts::TAU;
                let next = head - dir * (j as f32 + 1.0) * 0.9 + (side * a.cos() + up * a.sin()) * (0.25 + 0.35 * h2);
                let seg = next - prev;
                let sl = seg.length().max(0.01);
                let m = Mat4::from_scale_rotation_translation(Vec3::new(sl * 0.5, 0.03, 0.03), Quat::from_rotation_arc(Vec3::X, seg / sl), (prev + next) * 0.5);
                r.glass.push(Shell::Sphere, GlassInstance::new(m, Style::Glow, Vec3::new(1.2, 1.6, 2.6) * 4.0 * fade * (1.0 - j as f32 / 7.0), 1.0, 1.0, 0.0));
                prev = next;
            }
        }
    }
}

fn hash(x: f32) -> f32 {
    ((x * 12.9898).sin() * 43758.547).fract().abs()
}

fn along_x(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p)
}

/// A main-gun round, nose along +X, centred: a steel ogive, a brass driving
/// band, a steel body and a burning tracer base.
pub fn shell() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let steel = Paint::steel(0x6a6660, 0.3);
    let brass = Paint::steel(0xc8963c, 0.25);
    // Lathe profile along +Y (turned to +X): base to tip.
    let body = [
        (0.0f32, -0.2f32, Vec2::NEG_Y),
        (0.075, -0.2, Vec2::new(0.5, -0.85)),
        (0.08, -0.16, Vec2::X),
        (0.08, 0.04, Vec2::X),
        (0.07, 0.12, Vec2::new(0.9, 0.4)),
        (0.045, 0.2, Vec2::new(0.75, 0.65)),
        (0.0, 0.27, Vec2::Y),
    ];
    m.lathe(&body, 10, false, steel, along_x(Vec3::ZERO));
    m.cylinder(0.088, 0.025, 0.01, 1, 10, brass, along_x(Vec3::X * -0.1));
    m.cylinder(0.083, 0.02, 0.005, 1, 10, brass, along_x(Vec3::X * 0.0));
    m.cylinder(0.055, 0.02, 0.0, 1, 8, Paint::glow(0xffb040, 6.0), along_x(Vec3::X * -0.21));
    m
}

/// A rocket along +X, centred: a long painted body with a red nose, four
/// swept fins and a steel nozzle.
pub fn rocket() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let body = Paint::painted(0xd8d2c4, 0.3);
    m.cylinder(0.1, 0.42, 0.03, 1, 10, body, along_x(Vec3::ZERO));
    let tip = [(0.1f32, 0.0f32, Vec2::new(0.85, 0.3)), (0.06, 0.16, Vec2::new(0.8, 0.5)), (0.0, 0.28, Vec2::Y)];
    m.lathe(&tip, 10, false, Paint::painted(0xb8281c, 0.3), along_x(Vec3::X * 0.42));
    m.cylinder(0.105, 0.03, 0.01, 1, 10, Paint::with(crate::meshkit::Material::Hazard, 0xe8b419, 0.6), along_x(Vec3::X * 0.2));
    let nozzle = [(0.07f32, 0.0f32, Vec2::NEG_Y), (0.085, 0.0, Vec2::new(0.2, -1.0)), (0.06, 0.1, Vec2::X)];
    m.lathe(&nozzle, 8, true, Paint::steel(0x2a2725, 0.5), Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::X * -0.42));
    for k in 0..4 {
        let rot = Quat::from_rotation_x(k as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4);
        m.wedge(Vec3::new(0.13, 0.11, 0.012), 0.004, 1, 0.3, Paint::painted(0x3a3e34, 0.4), Mat4::from_rotation_translation(rot, Vec3::new(-0.3, 0.0, 0.0)) * Mat4::from_translation(Vec3::Y * 0.17));
    }
    m
}

/// The rocket's exhaust flame, a self-lit cone pointing back along -X from
/// the origin.
fn flame_cone() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let p = [(0.0f32, 0.0f32, Vec2::NEG_Y), (0.075, 0.0, Vec2::new(0.6, -0.4)), (0.06, 0.18, Vec2::X), (0.0, 0.6, Vec2::Y)];
    m.lathe(&p, 8, false, Paint::glow(0xff9a30, 5.0), Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2));
    m
}

/// A mortar bomb, centred, nose along +X: a fat teardrop body, a nose fuze
/// and a finned tail.
pub fn mortar() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let body = [
        (0.0f32, -0.32f32, Vec2::NEG_Y),
        (0.05, -0.32, Vec2::new(0.3, -0.95)),
        (0.06, -0.18, Vec2::new(0.9, -0.4)),
        (0.15, -0.02, Vec2::X),
        (0.13, 0.14, Vec2::new(0.85, 0.5)),
        (0.05, 0.27, Vec2::new(0.6, 0.8)),
        (0.0, 0.3, Vec2::Y),
    ];
    m.lathe(&body, 10, false, Paint::painted(0x4a5038, 0.4), along_x(Vec3::ZERO));
    m.cylinder(0.152, 0.02, 0.005, 1, 10, Paint::painted(0xe0c030, 0.3), along_x(Vec3::X * 0.06));
    m.cylinder(0.03, 0.04, 0.01, 1, 6, Paint::steel(0xc8963c, 0.3), along_x(Vec3::X * 0.31));
    for k in 0..4 {
        let rot = Quat::from_rotation_x(k as f32 * std::f32::consts::FRAC_PI_2);
        m.bevel_box(Vec3::new(0.07, 0.07, 0.008), 0.003, 1, Paint::steel(0x3a3632, 0.4), Mat4::from_rotation_translation(rot, Vec3::new(-0.27, 0.0, 0.0)) * Mat4::from_translation(Vec3::Y * 0.08));
    }
    m
}
