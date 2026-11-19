//! The air strike you can see coming: a chunky twin-engined prop bomber
//! comes in low along the carpet line, props spinning, wing lights
//! blinking, engines trailing smoke, its shadow sweeping over the sand; the
//! bomb bay drops a stick of finned bombs that fall onto the explosions the
//! sim sets off. (The siren and flyover are in the sound layer.)
//!
//! The flight is timed to the sim: bomb `i` of `n` bursts `i * duration / n`
//! after the run starts, `(i + 0.5) / n` along the line; the plane flies
//! the line at the speed that keeps it over the bursts, and each bomb is let
//! go a fall time earlier, keeping the plane's speed as it drops.

use std::collections::HashMap;

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

use crate::effects::Fx;
use crate::meshkit::{MeshBuilder, Paint};
use crate::renderer::{Instance, Light, Renderer};
use crate::shield::{GlassInstance, Shell, Style};
use crate::world::{Library, RenderWorld};

/// Height of the pass over the line, m.
const ALTITUDE: f32 = 30.0;
/// Bomb fall (a stylised, heavier gravity: they drop fast and hit hard).
const GRAVITY: f32 = 18.0;
/// Seconds of flight shown before the first bomb and after the last.
const LEAD: f32 = 7.0;
const TAIL: f32 = 7.0;
/// Model scale over the mesh (the mesh is about 16 m span).
const SCALE: f32 = 1.35;

#[derive(Clone, Copy)]
struct Run {
    /// Renderer clock of the first bomb's burst.
    start: f32,
    from: Vec3,
    to: Vec3,
    /// Smoke spawn accumulator.
    puff: f32,
}

#[derive(Default)]
pub struct AirStrikes {
    runs: HashMap<u16, Run>,
    tuning: Option<(u32, f32)>,
}

impl AirStrikes {
    fn tuning(&mut self) -> (u32, f32) {
        *self.tuning.get_or_insert_with(|| {
            let s = arena_core::Tuning::default().special;
            (s.strike_bombs.max(1), s.strike_duration.max(0.1))
        })
    }

    /// Queues the planes, bombs, glows and lights; feeds the smoke trails.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, r: &mut Renderer, lib: &mut Library, instances: &mut Vec<Instance>, blobs: &mut Vec<Vec4>, lights: &mut Vec<Light>, fx: &mut Fx, world: &RenderWorld, ground: &dyn Fn(f32, f32) -> f32) {
        let (n, duration) = self.tuning();
        let now = world.time;
        let dt = world.dt.clamp(0.0, 0.1);
        for s in &world.strikes {
            let est = now + s.delay;
            let run = self.runs.entry(s.id).or_insert(Run { start: est, from: s.from, to: s.to, puff: 0.0 });
            // Follow the sim's countdown while it runs (snapshots jitter a little).
            if s.delay > 0.0 && (est - run.start).abs() > 0.25 {
                run.start = est;
            }
        }
        self.runs.retain(|_, run| now < run.start + duration + TAIL && now > run.start - LEAD - 30.0);
        for run in self.runs.values_mut() {
            let line = (run.to - run.from).with_y(0.0);
            let len = line.length().max(1.0);
            let dir = line / len;
            let speed = len / duration;
            let t = now - run.start;
            if t < -LEAD {
                continue;
            }
            let floor = run.from.y.max(run.to.y);
            let ground_at = |t: f32| run.from + dir * (len * 0.5 / n as f32 + speed * t);
            let g = ground_at(t);
            let pos = Vec3::new(g.x, floor + ALTITUDE, g.z);
            // A gentle bank and bob as it comes in.
            let bank = 0.06 * (now * 0.9).sin();
            let yaw = Quat::from_rotation_arc(Vec3::X, dir);
            let rot = yaw * Quat::from_rotation_x(bank) * Quat::from_rotation_z(0.03 * (now * 1.3).sin());
            let body = Mat4::from_scale_rotation_translation(Vec3::splat(SCALE), rot, pos + Vec3::Y * 0.4 * (now * 1.7).sin());
            let plane = lib.get(r, "plane:bomber", bomber);
            instances.push(Instance::new(plane, body));
            let prop = lib.get(r, "plane:prop", propeller);
            for z in [-ENGINE_Z, ENGINE_Z] {
                let spin = Quat::from_rotation_x(now * 31.0 + z);
                instances.push(Instance::new(prop, body * Mat4::from_rotation_translation(spin, Vec3::new(ENGINE_X + 1.25, ENGINE_Y, z))));
                // A faint disc where the blades blur.
                let disc = body * Mat4::from_scale_rotation_translation(Vec3::new(0.05, 1.6, 1.6), Quat::IDENTITY, Vec3::new(ENGINE_X + 1.3, ENGINE_Y, z));
                r.glass.push(Shell::Sphere, GlassInstance::new(disc, Style::Aura, Vec3::splat(0.25), 0.6, 1.0, 0.0));
            }
            // Wing lights: port red, starboard green, a white strobe on the tail.
            let blink = ((now * 1.5).fract() < 0.12) as u32 as f32;
            for (p, c) in [(Vec3::new(0.4, 0.55, -WING_Z), Vec3::new(9.0, 0.6, 0.3)), (Vec3::new(0.4, 0.55, WING_Z), Vec3::new(0.4, 9.0, 1.0)), (Vec3::new(-7.6, 2.9, 0.0), Vec3::splat(12.0) * blink)] {
                let wp = body.transform_point3(p);
                r.glass.push(Shell::Sphere, GlassInstance::new(Mat4::from_scale_rotation_translation(Vec3::splat(0.45), Quat::IDENTITY, wp), Style::Glow, c, 1.0, 1.0, 0.0));
            }
            // Its shadow sweeping the sand (the real one falls with the sun).
            let gy = ground(pos.x, pos.z);
            blobs.push(Vec3::new(pos.x, gy, pos.z).extend(7.0));
            // Smoke from both engines, a white wake off the wingtips.
            run.puff += dt;
            while run.puff > 0.03 {
                run.puff -= 0.03;
                for z in [-ENGINE_Z, ENGINE_Z] {
                    let e = body.transform_point3(Vec3::new(ENGINE_X - 1.6, ENGINE_Y, z));
                    fx.smoke_a(e, -dir * 6.0 + Vec3::Y * 0.6, 1.5, 0.55, 2.6, 0.6);
                }
                for z in [-WING_Z, WING_Z] {
                    let e = body.transform_point3(Vec3::new(-0.4, 0.55, z));
                    fx.smoke_a(e, -dir * 3.0, 0.8, 0.0, 1.6, 0.35);
                }
            }
            // The stick of bombs, each let go a fall time before its burst.
            let fall = (2.0 * ALTITUDE / GRAVITY).sqrt();
            let bomb = lib.get(r, "plane:bomb", bomb_mesh);
            for i in 0..n {
                let hit = i as f32 * duration / n as f32;
                let tau = t - (hit - fall);
                if !(0.0..fall).contains(&tau) {
                    continue;
                }
                let g0 = ground_at(hit - fall);
                let p = Vec3::new(g0.x, floor + ALTITUDE - 1.2, g0.z) + dir * speed * tau - Vec3::Y * 0.5 * GRAVITY * tau * tau;
                let v = dir * speed - Vec3::Y * GRAVITY * tau;
                let aim = Quat::from_rotation_arc(Vec3::X, v.normalize_or(Vec3::NEG_Y));
                instances.push(Instance::new(bomb, Mat4::from_scale_rotation_translation(Vec3::splat(1.6), aim, p)));
            }
            if lights.len() < 8 {
                lights.push(Light { pos: pos - Vec3::Y * 3.0, radius: 14.0, color: Vec3::new(1.0, 0.4, 0.2) * 2.0 * blink.max(0.4) });
            }
        }
    }
}

/// Engine nacelle centre, x along the fuselage.
const ENGINE_X: f32 = 1.6;
const ENGINE_Y: f32 = 0.15;
const ENGINE_Z: f32 = 3.2;
/// Wingtip z.
const WING_Z: f32 = 8.0;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}

fn along_x(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p)
}

/// A chunky twin-engined bomber, nose along +X, centred: a fat fuselage
/// with a glazed nose and red cowl stripes, thick straight wings with two
/// big radial engines, a twin-finned tail, desert paint worn to metal.
pub fn bomber() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(0xb89a6a, 0.55);
    let belly = Paint::painted(0x8a7e66, 0.6);
    let dark = Paint::steel(0x2e2c2a, 0.45);
    let red = Paint::painted(0xb8281c, 0.35);
    // Fuselage: a lathe from tail to nose.
    let hull = [
        (0.0f32, -8.0f32, Vec2::NEG_Y),
        (0.35, -7.9, Vec2::new(0.5, -0.85)),
        (0.8, -5.5, Vec2::new(0.98, -0.2)),
        (1.25, -2.0, Vec2::new(1.0, -0.05)),
        (1.3, 1.5, Vec2::X),
        (1.15, 3.6, Vec2::new(0.9, 0.4)),
        (0.7, 4.8, Vec2::new(0.7, 0.7)),
        (0.0, 5.3, Vec2::Y),
    ];
    m.lathe(&hull, 12, true, paint, along_x(Vec3::ZERO));
    // The glazed nose and the cockpit canopy.
    m.cylinder(0.72, 0.35, 0.3, 2, 10, Paint::glow(0x6a8090, 0.6), along_x(Vec3::new(4.55, 0.1, 0.0)));
    m.bevel_box(Vec3::new(1.0, 0.45, 0.65), 0.3, 2, Paint::glow(0x5a7080, 0.5), at(Vec3::new(2.4, 1.15, 0.0)));
    // Belly and bomb bay.
    m.bevel_box(Vec3::new(2.6, 0.3, 0.9), 0.15, 1, belly, at(Vec3::new(0.0, -1.1, 0.0)));
    m.bevel_box(Vec3::new(1.8, 0.1, 0.6), 0.04, 1, dark, at(Vec3::new(-0.2, -1.38, 0.0)));
    // Red nose band.
    m.cylinder(1.22, 0.18, 0.05, 1, 12, red, along_x(Vec3::new(3.6, 0.0, 0.0)));
    // Wings: thick, straight, tips raked, with a red tip stripe.
    m.wedge(Vec3::new(1.5, 0.22, WING_Z), 0.12, 2, 0.55, paint, at(Vec3::new(0.4, 0.3, 0.0)));
    for z in [-WING_Z + 0.5, WING_Z - 0.5] {
        m.bevel_box(Vec3::new(1.45, 0.24, 0.35), 0.1, 1, red, at(Vec3::new(0.4, 0.3, z)));
    }
    // Engines: fat nacelles with cowls and exhaust stubs.
    for z in [-ENGINE_Z, ENGINE_Z] {
        m.cylinder(0.75, 1.5, 0.35, 2, 10, paint, along_x(Vec3::new(ENGINE_X - 0.5, ENGINE_Y, z)));
        m.cylinder(0.85, 0.35, 0.15, 2, 12, red, along_x(Vec3::new(ENGINE_X + 0.85, ENGINE_Y, z)));
        m.cylinder(0.3, 0.2, 0.08, 1, 8, dark, along_x(Vec3::new(ENGINE_X + 1.2, ENGINE_Y, z)));
        m.cylinder(0.12, 0.4, 0.03, 1, 6, dark, along_x(Vec3::new(ENGINE_X - 1.6, ENGINE_Y - 0.5, z)));
    }
    // Tailplane and twin fins.
    m.wedge(Vec3::new(1.1, 0.12, 3.2), 0.06, 1, 0.5, paint, at(Vec3::new(-7.0, 0.5, 0.0)));
    for z in [-3.1f32, 3.1] {
        m.wedge(Vec3::new(1.0, 1.3, 0.12), 0.06, 1, 0.45, paint, at(Vec3::new(-7.2, 1.5, z)));
        m.bevel_box(Vec3::new(0.6, 0.25, 0.14), 0.05, 1, red, at(Vec3::new(-7.4, 2.4, z)));
    }
    m
}

/// A three-bladed propeller with a spinner, turning about +X.
fn propeller() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let blade = Paint::steel(0x2a2725, 0.5);
    let spinner = [(0.3f32, 0.0f32, Vec2::new(0.6, -0.8)), (0.25, 0.25, Vec2::new(0.9, 0.4)), (0.0, 0.55, Vec2::Y)];
    m.lathe(&spinner, 8, false, Paint::painted(0xb8281c, 0.3), along_x(Vec3::ZERO));
    for k in 0..3 {
        let a = k as f32 * std::f32::consts::TAU / 3.0;
        let rot = Quat::from_rotation_x(a) * Quat::from_rotation_y(0.35);
        m.bevel_box(Vec3::new(0.05, 1.5, 0.16), 0.03, 1, blade, Mat4::from_rotation_translation(rot, Vec3::ZERO) * at(Vec3::Y * 1.5));
        m.bevel_box(Vec3::new(0.055, 0.12, 0.17), 0.02, 1, Paint::painted(0xe8b419, 0.3), Mat4::from_rotation_translation(rot, Vec3::ZERO) * at(Vec3::Y * 2.9));
    }
    m
}

/// A finned bomb, nose along +X, centred: olive, a yellow nose band.
pub fn bomb_mesh() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let body = [
        (0.0f32, -0.55f32, Vec2::NEG_Y),
        (0.08, -0.55, Vec2::new(0.3, -0.95)),
        (0.18, -0.25, Vec2::new(0.95, -0.3)),
        (0.22, 0.05, Vec2::X),
        (0.18, 0.3, Vec2::new(0.8, 0.6)),
        (0.0, 0.45, Vec2::Y),
    ];
    m.lathe(&body, 10, false, Paint::painted(0x4a5038, 0.45), along_x(Vec3::ZERO));
    m.cylinder(0.205, 0.04, 0.01, 1, 10, Paint::painted(0xe0c030, 0.3), along_x(Vec3::X * 0.22));
    for k in 0..4 {
        let rot = Quat::from_rotation_x(k as f32 * std::f32::consts::FRAC_PI_2);
        m.bevel_box(Vec3::new(0.12, 0.14, 0.01), 0.004, 1, Paint::steel(0x3a3632, 0.4), Mat4::from_rotation_translation(rot, Vec3::new(-0.5, 0.0, 0.0)) * at(Vec3::Y * 0.12));
    }
    // A box tail ring.
    m.cylinder(0.24, 0.06, 0.01, 1, 4, Paint::steel(0x3a3632, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_y(std::f32::consts::FRAC_PI_4), Vec3::X * -0.58));
    m
}
