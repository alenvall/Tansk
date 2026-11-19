//! The tank, built from the mesh kit in two modular parts: the hull (with
//! running gear and hull bolt-ons) and the turret (with mantlet and gun),
//! so the turret can traverse and be swapped for upgrade variants.
//!
//! The look is a stylized arena-game toy-soldier machine: squat and chunky,
//! a big soft-bevelled turret over a wide hull that overhangs thick round
//! tracks on big wheels, fat stubby barrels with big muzzle rings, a few big
//! rivets instead of rows of small ones, and the hand-welded scrap charm in
//! the bolt-ons (a strapped log, a jerrycan, crates, a bent exhaust).
//! Each Garage class has its own silhouette over the same hit boxes: the
//! Scout low and wedged with a dome turret, the Brawler the all-rounder, the
//! Hauler tall and boxy with a cargo rack and twin stacks.

use arena_core::ChassisClass;
use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{bevelled_cylinder_profile, Material, MeshBuilder, Paint};

/// The hull frame's origin (the simulation's hull centre) above the ground.
/// Meshes are modelled standing on y = 0 and shifted down by this.
pub const HULL_Y: f32 = 0.95;
/// The turret pivot in the hull frame (arena-core: (-0.3, 0.45, 0)).
pub const TURRET_PIVOT: Vec3 = Vec3::new(-0.3, 0.45, 0.0);
/// The gun frame in the turret frame (arena-core: (1.05, 0.42, 0)), plus a
/// small visual lift so the gun sits mid-face on the big turret.
pub const GUN_PIVOT: Vec3 = Vec3::new(1.05, 0.6, 0.0);
/// Exhaust stack tip, hull frame (for exhaust puffs).
pub const EXHAUST: Vec3 = Vec3::new(-2.72, 2.42 - HULL_Y, -1.55);
/// Road wheels: x of each of the five per side, |z| of the wheel's centre plane.
pub const WHEEL_X: [f32; 5] = [2.2, 1.1, 0.0, -1.1, -2.2];
pub const WHEEL_Z: f32 = 1.8;
/// Road wheel radius (the big toon wheels).
pub const WHEEL_R: f32 = 0.43;
/// Wheel centre height in the hull frame for a compression (metres).
pub fn wheel_y(compression: f32) -> f32 {
    -0.58 + compression
}
/// The deck height (standing frame) the turret ring sits on.
const DECK_Y: f32 = 1.46;
/// Track loops: centre height, half height, |z| centre, half width.
const TRACK_Y: f32 = 0.52;
const TRACK_HY: f32 = 0.5;
const TRACK_Z: f32 = 1.45;
const TRACK_HZ: f32 = 0.4;

pub const RUSTBACK: u32 = 0xe2661a;
pub const DUST_SAINT: u32 = 0x1b9a92;
const TRIM: u32 = 0x2f2b28;
const BARE: u32 = 0xa8a398;
const TRACK: u32 = 0x2b2725;
const CRATE: u32 = 0x9a6a3e;
const LOG: u32 = 0x8a5a32;
const JERRY: u32 = 0xc0281c;
const ROPE: u32 = 0xb89660;
/// Team paint wear: the toon machines are kept cleaner than the old ones.
const PAINT_WEAR: f32 = 0.22;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}
fn along_z(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), p)
}
fn along_x(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p)
}

/// The team paint every tank part uses.
pub fn team_paint(team: u32) -> Paint {
    Paint::team(team, PAINT_WEAR)
}
/// Dark gunmetal trim (bumpers, mantlets, rings, muzzles).
pub fn trim_paint() -> Paint {
    Paint::steel(TRIM, 0.5)
}
/// Bright bare steel (hubs, bolts).
pub fn bare_paint() -> Paint {
    Paint::steel(BARE, 0.35)
}

/// The toon armour panel: a slab with its rim fully rounded over and a few
/// fat rivets, in the corners and about every metre along its long edges
/// (a single row down the middle when it is a narrow strip).
pub fn panel(m: &mut MeshBuilder, centre: Vec3, half: Vec3, axis: usize, sign: f32, paint: Paint, rivet_paint: Paint) {
    let bevel = (half[axis] * 0.95).min(0.16);
    m.bevel_box(half, bevel, 3, paint, at(centre));
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let (long, short) = if half[u] >= half[v] { (u, v) } else { (v, u) };
    let mut normal = Vec3::ZERO;
    normal[axis] = sign;
    let face = centre + normal * half[axis];
    let r = (half[short] * 0.3).clamp(0.045, 0.085);
    let inset = (bevel + r * 1.6).max(0.12);
    let len = 2.0 * (half[long] - inset);
    if len <= 0.0 {
        return;
    }
    let count = (len / 1.0).round().max(1.0) as u32;
    let rows: &[f32] = if half[short] - inset < 0.12 { &[0.0] } else { &[-1.0, 1.0] };
    for &row in rows {
        for k in 0..=count {
            let mut p = face;
            p[long] += -half[long] + inset + len * k as f32 / count as f32;
            p[short] += row * (half[short] - inset);
            m.rivet(p, normal, r, rivet_paint);
        }
    }
}

/// An octagonal, faceted pipe from `a` to `b` with bevelled ends.
pub fn octo_pipe(m: &mut MeshBuilder, a: Vec3, b: Vec3, radius: f32, paint: Paint) {
    let d = b - a;
    let profile = bevelled_cylinder_profile(radius, 0.5 * d.length(), radius * 0.2, 1);
    let rot = Quat::from_rotation_arc(Vec3::Y, d.normalize());
    m.lathe(
        &profile,
        8,
        true,
        paint,
        Mat4::from_rotation_translation(rot, 0.5 * (a + b)) * Mat4::from_rotation_y(std::f32::consts::PI / 8.0),
    );
}

/// A smooth round pipe from `a` to `b`, its ends rounded over (toon pipes,
/// stacks and rods).
pub fn pipe(m: &mut MeshBuilder, a: Vec3, b: Vec3, radius: f32, paint: Paint) {
    let d = b - a;
    let profile = bevelled_cylinder_profile(radius, 0.5 * d.length(), radius * 0.45, 2);
    let rot = Quat::from_rotation_arc(Vec3::Y, d.normalize());
    m.lathe(&profile, 14, false, paint, Mat4::from_rotation_translation(rot, 0.5 * (a + b)));
}

/// A fat ring (muzzle rings, collars) along +X at `c`: a short cylinder
/// with well rounded rims.
pub fn ring_x(m: &mut MeshBuilder, c: Vec3, radius: f32, half_len: f32, paint: Paint) {
    m.cylinder(radius, half_len, (half_len * 0.7).min(radius * 0.3), 3, 20, paint, along_x(c));
}

/// A smooth ball of radius `r` at `c`.
pub fn ball(m: &mut MeshBuilder, c: Vec3, r: f32, paint: Paint) {
    let rings = 8;
    let profile: Vec<(f32, f32, Vec2)> = (0..=rings)
        .map(|k| {
            let a = k as f32 / rings as f32 * std::f32::consts::PI;
            let n = Vec2::new(a.sin(), a.cos());
            (r * n.x, r * n.y, n)
        })
        .collect();
    m.lathe(&profile, 16, false, paint, at(c));
}

/// A wooden crate with dark straps.
pub fn crate_box(m: &mut MeshBuilder, centre: Vec3, half: Vec3) {
    m.bevel_box(half, 0.03, 1, Paint::with(Material::Wood, CRATE, 0.8), at(centre));
    let strap = Paint::steel(TRIM, 0.5);
    for dx in [-0.55f32, 0.55] {
        m.bevel_box(
            Vec3::new(0.035, half.y + 0.01, half.z + 0.01),
            0.01,
            1,
            strap,
            at(centre + Vec3::new(dx * half.x, 0.0, 0.0)),
        );
    }
}

/// A chunky toon crate: soft-edged planks with one fat strap round it.
pub fn toon_crate(m: &mut MeshBuilder, centre: Vec3, half: Vec3) {
    m.bevel_box(half, half.min_element() * 0.3, 2, Paint::with(Material::Wood, CRATE, 0.8), at(centre));
    m.bevel_box(Vec3::new(0.06, half.y + 0.015, half.z + 0.015), 0.03, 2, trim_paint(), at(centre));
}

/// A log strapped along X: a wood cylinder with two dark straps.
pub fn strapped_log(m: &mut MeshBuilder, centre: Vec3, half_len: f32, radius: f32) {
    m.cylinder(radius, half_len, radius * 0.35, 2, 10, Paint::with(Material::Wood, LOG, 0.85), along_x(centre));
    m.cylinder(radius * 0.7, half_len + 0.03, 0.02, 1, 10, Paint::with(Material::Wood, 0xc89a62, 0.8), along_x(centre));
    for dx in [-0.6f32, 0.6] {
        m.cylinder(radius + 0.025, 0.05, 0.02, 1, 10, trim_paint(), along_x(centre + Vec3::X * dx * half_len));
    }
}

/// A red jerrycan standing at `c` (its base), face along Z.
pub fn jerrycan(m: &mut MeshBuilder, base: Vec3, yaw: f32) {
    let rot = Quat::from_rotation_y(yaw);
    let xf = |p: Vec3| Mat4::from_rotation_translation(rot, base + rot * p);
    let red = Paint::painted(JERRY, 0.45);
    m.bevel_box(Vec3::new(0.2, 0.26, 0.09), 0.06, 2, red, xf(Vec3::Y * 0.26));
    m.bevel_box(Vec3::new(0.12, 0.03, 0.02), 0.012, 1, red, xf(Vec3::new(0.0, 0.42, 0.1)));
    m.bevel_box(Vec3::new(0.025, 0.06, 0.04), 0.012, 1, trim_paint(), xf(Vec3::new(0.12, 0.56, 0.0)));
    m.cylinder(0.04, 0.05, 0.015, 1, 8, trim_paint(), xf(Vec3::new(-0.12, 0.56, 0.0)));
}

/// How each Garage class is shaped (all over the same hit boxes).
struct HullStyle {
    /// Upper hull half height (its top is the deck, `DECK_Y`).
    body_hy: f32,
    /// Nose front height as a fraction (the glacis rake).
    nose_k: f32,
    /// Half beam of the upper hull, over the tracks.
    beam: f32,
    bevel: f32,
}

fn hull_style(class: ChassisClass) -> HullStyle {
    match class {
        ChassisClass::Scout => HullStyle { body_hy: 0.24, nose_k: 0.22, beam: 1.8, bevel: 0.2 },
        ChassisClass::Brawler => HullStyle { body_hy: 0.3, nose_k: 0.42, beam: 1.86, bevel: 0.26 },
        ChassisClass::Hauler => HullStyle { body_hy: 0.34, nose_k: 0.72, beam: 1.9, bevel: 0.22 },
    }
}

/// The hull of the default class (the Brawler).
pub fn hull_mesh(team: u32) -> MeshBuilder {
    hull_mesh_class(ChassisClass::Brawler, team)
}

/// The hull for a Garage class: running gear, the upper hull overhanging
/// the tracks, deck and hull bolt-ons. +X forward, +Z right, in the hull
/// frame (modelled standing on y = 0, shifted down by `HULL_Y`).
pub fn hull_mesh_class(class: ChassisClass, team: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let st = hull_style(class);
    let paint = team_paint(team);
    let rivet = Paint::team(team, 0.15);
    let trim = trim_paint();
    let bare = bare_paint();
    let track = Paint::rubber(TRACK);
    let hy = st.body_hy;
    let yb = DECK_Y - hy;

    // Running gear: a fat rounded track loop each side, an idler and a
    // sprocket at the ends (the road wheels are separate instances).
    for side in [-1.0f32, 1.0] {
        let z = side * TRACK_Z;
        m.bevel_box(Vec3::new(2.98, TRACK_HY, TRACK_HZ), TRACK_HZ, 5, track, at(Vec3::new(0.0, TRACK_Y, z)));
        for x in [-2.62f32, 2.62] {
            let w = Vec3::new(x, TRACK_Y + 0.04, side * (TRACK_Z + TRACK_HZ - 0.03));
            m.cylinder(0.27, 0.05, 0.035, 2, 14, trim, along_z(w));
            m.cylinder(0.11, 0.07, 0.03, 2, 10, bare, along_z(w));
        }
    }

    // The belly between the tracks, seen at the nose and tail.
    m.bevel_box(Vec3::new(2.9, 0.3, 1.06), 0.22, 3, paint, at(Vec3::new(0.05, 0.74, 0.0)));
    // The upper hull: a fat rounded tub over the tracks and a raked nose.
    m.bevel_box(Vec3::new(2.0, hy, st.beam), st.bevel, 3, paint, at(Vec3::new(-0.55, yb, 0.0)));
    m.wedge(Vec3::new(0.72, hy, st.beam), st.bevel, 3, st.nose_k, paint, at(Vec3::new(2.15, yb, 0.0)));
    // The dark seam under the overhang, so the hull reads as a lid on the tracks.
    for side in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(2.65, 0.05, 0.1), 0.045, 2, trim, at(Vec3::new(0.15, yb - hy + 0.02, side * (st.beam - 0.12))));
    }
    // A few fat rivets along each side of the tub.
    for side in [-1.0f32, 1.0] {
        for x in [-2.15f32, -0.95, 0.25, 1.45] {
            m.rivet(Vec3::new(x, yb, side * st.beam), Vec3::Z * side, 0.075, rivet);
        }
    }
    // Front bumper with two big bolts and tow hooks, a pair of headlamps.
    let nose_top = yb - hy + 2.0 * hy * st.nose_k;
    let bumper_y = 0.78;
    m.bevel_box(Vec3::new(0.14, 0.14, 1.3), 0.13, 3, trim, at(Vec3::new(2.95, bumper_y, 0.0)));
    for z in [-0.8f32, 0.8] {
        m.rivet(Vec3::new(3.09, bumper_y, z), Vec3::X, 0.08, bare);
        m.torus(0.1, 0.035, 0.07, 10, trim, Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), Vec3::new(3.15, bumper_y - 0.12, z * 0.55)));
    }
    for z in [-1.4f32, 1.4] {
        let c = Vec3::new(2.9, nose_top - 0.02, z);
        ring_x(&mut m, c, 0.15, 0.07, trim);
        m.cylinder(0.11, 0.01, 0.0, 1, 14, Paint::glow(0xfff0c0, 0.7), along_x(c + Vec3::X * 0.075));
    }
    // Tail: a rear plate with two big bolts, the engine deck grille.
    m.bevel_box(Vec3::new(0.08, 0.26, 1.05), 0.07, 2, trim, at(Vec3::new(-2.82, 0.86, 0.0)));
    for z in [-0.6f32, 0.6] {
        m.rivet(Vec3::new(-2.9, 0.86, z), Vec3::NEG_X, 0.07, bare);
    }
    // A raised engine deck at the back with a fat grille on it.
    m.bevel_box(Vec3::new(0.42, 0.1, 1.3), 0.09, 3, paint, at(Vec3::new(-2.15, DECK_Y + 0.02, 0.0)));
    for i in 0..3 {
        m.bevel_box(Vec3::new(0.07, 0.04, 0.55), 0.035, 2, trim, at(Vec3::new(-2.37 + i as f32 * 0.22, DECK_Y + 0.12, 0.0)));
    }
    // The bent exhaust stack on the left rear fender.
    exhaust_stack(&mut m, -1.0);
    // Turret ring.
    m.cylinder(1.02, 0.05, 0.03, 2, 28, trim, at(Vec3::new(TURRET_PIVOT.x, DECK_Y + 0.03, 0.0)));

    // Class bolt-ons.
    match class {
        ChassisClass::Scout => {
            // Racing stripes down the nose and deck, little side fins, one jerrycan.
            let stripe = Paint::painted(0xf2ece0, 0.3);
            for z in [-0.32f32, 0.32] {
                m.bevel_box(Vec3::new(0.6, 0.012, 0.12), 0.01, 1, stripe, Mat4::from_rotation_translation(Quat::from_rotation_z(-((2.0 * hy * (1.0 - st.nose_k)) / 1.44).atan()), Vec3::new(2.1, yb + hy * st.nose_k + 0.015, z)));
                m.bevel_box(Vec3::new(0.35, 0.012, 0.12), 0.01, 1, stripe, at(Vec3::new(1.1, DECK_Y + 0.005, z)));
            }
            for side in [-1.0f32, 1.0] {
                m.wedge(Vec3::new(0.45, 0.16, 0.06), 0.05, 2, 0.2, paint, at(Vec3::new(-2.35, DECK_Y + 0.14, side * (st.beam - 0.15))));
            }
            jerrycan(&mut m, Vec3::new(-1.65, DECK_Y, 1.62), 0.0);
        }
        ChassisClass::Brawler => {
            strapped_log(&mut m, Vec3::new(0.55, DECK_Y + 0.16, -1.66), 0.75, 0.15);
            toon_crate(&mut m, Vec3::new(-1.6, DECK_Y + 0.2, 1.52), Vec3::new(0.32, 0.2, 0.26));
            jerrycan(&mut m, Vec3::new(0.65, DECK_Y, 1.68), 0.0);
            // A coil of tow rope on the nose.
            m.torus(0.22, 0.05, 0.1, 14, Paint::matte(ROPE), at(Vec3::new(2.05, yb + hy * (1.0 + st.nose_k) * 0.5 + 0.12, -0.75)));
        }
        ChassisClass::Hauler => {
            // A second stack, a cargo rack along both fenders, spare track
            // links on the nose.
            exhaust_stack(&mut m, 1.0);
            strapped_log(&mut m, Vec3::new(0.55, DECK_Y + 0.16, -1.68), 0.8, 0.15);
            strapped_log(&mut m, Vec3::new(0.55, DECK_Y + 0.16, 1.68), 0.8, 0.15);
            toon_crate(&mut m, Vec3::new(-1.6, DECK_Y + 0.22, 1.45), Vec3::new(0.3, 0.22, 0.3));
            for side in [-1.0f32, 1.0] {
                pipe(&mut m, Vec3::new(-0.4, DECK_Y + 0.02, side * 1.84), Vec3::new(1.5, DECK_Y + 0.02, side * 1.84), 0.04, trim);
                for x in [-0.4f32, 1.5] {
                    pipe(&mut m, Vec3::new(x, DECK_Y - 0.1, side * 1.84), Vec3::new(x, DECK_Y + 0.42, side * 1.84), 0.04, trim);
                }
                pipe(&mut m, Vec3::new(-0.4, DECK_Y + 0.42, side * 1.84), Vec3::new(1.5, DECK_Y + 0.42, side * 1.84), 0.04, trim);
            }
            for k in 0..3 {
                m.bevel_box(Vec3::new(0.16, 0.05, 0.5), 0.045, 2, Paint::rubber(TRACK), at(Vec3::new(2.05 + k as f32 * 0.36, nose_top + (yb + hy - nose_top) * (1.0 - (0.15 + k as f32 * 0.3)) + 0.05, 0.75)));
            }
            jerrycan(&mut m, Vec3::new(-1.0, DECK_Y, -1.62), 0.0);
        }
    }
    m.transform_from(0, at(Vec3::Y * -HULL_Y));
    m
}

/// The bent exhaust stack on a rear fender (`side` -1 left, the puff
/// point `EXHAUST`; +1 mirrors it for the Hauler's second stack).
fn exhaust_stack(m: &mut MeshBuilder, side: f32) {
    let soot = Paint::steel(0x2b2725, 0.75);
    let base = Vec3::new(-2.35, DECK_Y - 0.05, side * 1.55);
    let bend = Vec3::new(-2.38, 2.12, side * 1.55);
    let tip = Vec3::new(EXHAUST.x, EXHAUST.y + HULL_Y, side * 1.55);
    pipe(m, base, bend + Vec3::Y * 0.05, 0.15, soot);
    pipe(m, bend, tip, 0.15, soot);
    m.cylinder(0.19, 0.06, 0.04, 2, 14, trim_paint(), at(base + Vec3::Y * 0.12));
    let out = (tip - bend).normalize();
    let rot = Quat::from_rotation_arc(Vec3::Y, out);
    m.cylinder(0.19, 0.07, 0.05, 2, 14, trim_paint(), Mat4::from_rotation_translation(rot, tip - out * 0.02));
    m.cylinder(0.12, 0.01, 0.0, 1, 12, Paint::with(Material::BareSteel, 0x0c0a09, 0.9), Mat4::from_rotation_translation(rot, tip + out * 0.055));
}

/// One road wheel, centred on its hub, axle along Z, outer face at +Z: a
/// fat rubber tyre on a dished steel wheel with a bright hub and three
/// big bolts.
pub fn road_wheel_mesh() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let bare = bare_paint();
    m.cylinder(WHEEL_R, 0.09, 0.07, 3, 22, Paint::steel(0x5c5752, 0.5), along_z(Vec3::ZERO));
    m.cylinder(WHEEL_R * 0.7, 0.05, 0.035, 2, 18, Paint::steel(0x3e3a37, 0.55), along_z(Vec3::Z * 0.07));
    m.cylinder(WHEEL_R * 0.36, 0.05, 0.045, 3, 14, bare, along_z(Vec3::Z * 0.11));
    for k in 0..3 {
        let a = k as f32 / 3.0 * std::f32::consts::TAU + 0.5;
        m.rivet(Vec3::new(a.cos() * WHEEL_R * 0.53, a.sin() * WHEEL_R * 0.53, 0.12), Vec3::Z, 0.05, bare);
    }
    m
}

/// The turret of the default class (the Brawler).
pub fn turret_mesh(team: u32) -> MeshBuilder {
    turret_mesh_class(ChassisClass::Brawler, team)
}

/// The turret for a Garage class, in turret space (origin on the ring):
/// a big soft-bevelled house, a hatch cupola, an antenna with a ball tip.
/// The gun comes separately (`parts::gun_mesh`).
pub fn turret_mesh_class(class: ChassisClass, team: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = team_paint(team);
    let rivet = Paint::team(team, 0.15);
    let trim = trim_paint();
    let bare = bare_paint();
    // The ring collar.
    m.cylinder(1.0, 0.07, 0.05, 2, 28, trim, at(Vec3::Y * 0.08));
    let top = match class {
        ChassisClass::Scout => {
            // A low rounded dome.
            let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
            let profile = [
                (0.0, 0.98, Vec2::Y),
                (0.6, 0.96, n(0.12, 1.0)),
                (0.92, 0.86, n(0.45, 1.0)),
                (1.1, 0.64, n(1.0, 0.6)),
                (1.16, 0.38, n(1.0, 0.0)),
                (1.1, 0.16, n(1.0, -0.5)),
                (0.95, 0.1, Vec2::NEG_Y),
                (0.0, 0.1, Vec2::NEG_Y),
            ];
            m.lathe(&profile, 28, false, paint, Mat4::IDENTITY);
            m.cylinder(1.14, 0.04, 0.025, 2, 28, trim, at(Vec3::Y * 0.22));
            for a in [0.9f32, 2.2, 4.1, 5.4] {
                let d = Vec3::new(a.cos(), 0.0, a.sin());
                m.rivet(d * 1.16 + Vec3::Y * 0.4, d, 0.08, rivet);
            }
            0.97
        }
        ChassisClass::Brawler => {
            m.wedge(Vec3::new(1.32, 0.5, 1.25), 0.44, 4, 0.84, paint, at(Vec3::new(0.0, 0.62, 0.0)));
            for side in [-1.0f32, 1.0] {
                for x in [-0.55f32, 0.45] {
                    m.rivet(Vec3::new(x, 0.6, side * 1.25), Vec3::Z * side, 0.085, rivet);
                }
            }
            // A stowage box on the turret's back.
            m.bevel_box(Vec3::new(0.22, 0.24, 0.66), 0.1, 3, trim, at(Vec3::new(-1.36, 0.6, 0.0)));
            m.bevel_box(Vec3::new(0.05, 0.05, 0.68), 0.03, 2, bare, at(Vec3::new(-1.52, 0.68, 0.0)));
            1.11
        }
        ChassisClass::Hauler => {
            m.wedge(Vec3::new(1.32, 0.52, 1.24), 0.3, 3, 0.9, paint, at(Vec3::new(0.0, 0.64, 0.0)));
            for side in [-1.0f32, 1.0] {
                panel(&mut m, Vec3::new(-0.15, 0.6, side * 1.26), Vec3::new(0.85, 0.3, 0.07), 2, side, paint, rivet);
            }
            // A cargo basket on the turret's back with a crate in it.
            m.bevel_box(Vec3::new(0.3, 0.05, 0.8), 0.04, 2, trim, at(Vec3::new(-1.48, 0.32, 0.0)));
            for z in [-0.78f32, 0.78] {
                pipe(&mut m, Vec3::new(-1.76, 0.3, z), Vec3::new(-1.76, 0.75, z), 0.035, trim);
            }
            pipe(&mut m, Vec3::new(-1.76, 0.75, -0.78), Vec3::new(-1.76, 0.75, 0.78), 0.035, trim);
            toon_crate(&mut m, Vec3::new(-1.45, 0.55, -0.35), Vec3::new(0.24, 0.2, 0.28));
            1.15
        }
    };
    // Hatch cupola (front left, clear of the emitter and harpoon mounts).
    let hatch = Vec3::new(0.3, top, -0.5);
    m.cylinder(0.36, 0.06, 0.04, 2, 22, trim, at(hatch + Vec3::Y * 0.02));
    m.cylinder(0.29, 0.06, 0.05, 3, 22, paint, at(hatch + Vec3::Y * 0.1));
    m.bevel_box(Vec3::new(0.05, 0.035, 0.16), 0.025, 2, bare, at(hatch + Vec3::new(0.0, 0.18, 0.0)));
    m.rivet(hatch + Vec3::new(-0.3, 0.1, 0.0), Vec3::NEG_X, 0.07, bare);
    // Periscope block, front right.
    m.bevel_box(Vec3::new(0.12, 0.08, 0.16), 0.05, 2, trim, at(Vec3::new(0.75, top - 0.02, 0.62)));
    m.bevel_box(Vec3::new(0.01, 0.035, 0.11), 0.01, 1, Paint::steel(0x1a2228, 0.2), at(Vec3::new(0.875, top - 0.0, 0.62)));
    // The antenna with a ball on top.
    let foot = Vec3::new(-0.95, top - 0.12, 0.72);
    m.cylinder(0.08, 0.06, 0.03, 2, 10, trim, at(foot));
    pipe(&mut m, foot, foot + Vec3::new(-0.08, 1.15, 0.0), 0.018, trim);
    m.cylinder(0.06, 0.06, 0.055, 3, 12, Paint::painted(0xe0b030, 0.3), at(foot + Vec3::new(-0.08, 1.2, 0.0)));
    m
}

/// The turret's transform relative to the hull for a traverse (+ = left).
pub fn turret_local(traverse: f32) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_y(traverse), TURRET_PIVOT)
}

/// The gun frame relative to the turret for a pitch (+ = up).
pub fn gun_local(pitch: f32) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(pitch), GUN_PIVOT)
}

// ── Raider vehicles (wave 3) ─────────────────────────────────────────────
//
// Built in each chassis's hull frame (origin at the hull centre, +X
// forward), sized to arena-core's `hull_half` for the chassis. Wheels are
// separate instances, placed from the chassis's suspension tuning.

/// The raiders' paint: dark rust-black.
pub const RAIDER: u32 = 0x3a2a22;
/// The raiders' bone-white stencil stripes.
const BONE: u32 = 0xd8ccb0;

fn stripe(m: &mut MeshBuilder, centre: Vec3, half: Vec3) {
    m.bevel_box(half, 0.01, 1, Paint::painted(BONE, 0.5), at(centre));
}

/// A light dune buggy: a fat rounded tub, a chunky roll cage, a spare
/// balloon tyre, a bull bar (its big tyres are instanced separately).
/// Hull half (2.2, 0.35, 1.1).
pub fn buggy_mesh(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(colour, 0.45);
    let rivet = Paint::painted(colour, 0.5);
    let trim = trim_paint();
    let tube = Paint::steel(0x4a4440, 0.5);
    // Floor pan, a raked nose and a fat tub.
    m.bevel_box(Vec3::new(2.0, 0.14, 0.85), 0.12, 3, trim, at(Vec3::new(0.0, -0.2, 0.0)));
    m.wedge(Vec3::new(0.9, 0.3, 0.86), 0.2, 3, 0.4, paint, at(Vec3::new(1.2, 0.05, 0.0)));
    m.bevel_box(Vec3::new(0.95, 0.32, 0.9), 0.22, 3, paint, at(Vec3::new(-1.1, 0.08, 0.0)));
    for side in [-1.0f32, 1.0] {
        panel(&mut m, Vec3::new(0.1, 0.05, side * 0.86), Vec3::new(0.65, 0.24, 0.05), 2, side, paint, rivet);
        stripe(&mut m, Vec3::new(1.25, 0.18, side * 0.87), Vec3::new(0.45, 0.05, 0.01));
    }
    // Roll cage.
    for side in [-1.0f32, 1.0] {
        let z = side * 0.7;
        m.tube(&[Vec3::new(0.9, -0.05, z), Vec3::new(0.4, 1.05, z * 0.9), Vec3::new(-0.6, 1.1, z * 0.9), Vec3::new(-1.2, 0.2, z)], 0.075, 10, tube);
    }
    m.tube(&[Vec3::new(0.4, 1.05, -0.63), Vec3::new(0.4, 1.05, 0.63)], 0.075, 10, tube);
    m.tube(&[Vec3::new(-0.6, 1.1, -0.63), Vec3::new(-0.6, 1.1, 0.63)], 0.075, 10, tube);
    // A seat, a spare wheel, a jerrycan, headlamps.
    m.bevel_box(Vec3::new(0.25, 0.25, 0.25), 0.1, 3, Paint::matte(0x4a3a2a), at(Vec3::new(-0.2, 0.25, 0.32)));
    m.torus(0.32, 0.15, 0.26, 14, Paint::rubber(0x24211f), Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::new(-2.05, 0.35, 0.0)));
    m.cylinder(0.18, 0.1, 0.05, 2, 12, Paint::steel(0x5a5450, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::new(-2.05, 0.35, 0.0)));
    jerrycan(&mut m, Vec3::new(-1.55, 0.4, -0.5), std::f32::consts::FRAC_PI_2);
    for z in [-0.55f32, 0.55] {
        let c = Vec3::new(2.05, 0.3, z);
        ring_x(&mut m, c, 0.13, 0.06, trim);
        m.cylinder(0.095, 0.01, 0.0, 1, 12, Paint::glow(0xfff0c0, 0.7), along_x(c + Vec3::X * 0.065));
    }
    // Bull bar.
    m.tube(&[Vec3::new(2.15, -0.2, -0.7), Vec3::new(2.3, 0.3, -0.5), Vec3::new(2.3, 0.3, 0.5), Vec3::new(2.15, -0.2, 0.7)], 0.085, 10, tube);
    m
}

/// A pickup technical: a rounded cab, hood and bed (the rocket rack is its
/// turret). Hull half (2.6, 0.45, 1.2).
pub fn technical_mesh(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(colour, 0.5);
    let rivet = Paint::painted(colour, 0.55);
    let trim = trim_paint();
    let glass = Paint::steel(0x1a2228, 0.25);
    m.bevel_box(Vec3::new(2.5, 0.2, 1.0), 0.15, 3, trim, at(Vec3::new(0.0, -0.3, 0.0)));
    // Hood and grille.
    m.wedge(Vec3::new(0.8, 0.32, 1.08), 0.22, 3, 0.72, paint, at(Vec3::new(1.75, 0.15, 0.0)));
    for k in 0..4 {
        m.bevel_box(Vec3::new(0.05, 0.2, 0.08), 0.035, 2, trim, at(Vec3::new(2.56, 0.1, -0.45 + k as f32 * 0.3)));
    }
    for z in [-0.85f32, 0.85] {
        let c = Vec3::new(2.53, 0.15, z);
        ring_x(&mut m, c, 0.14, 0.06, trim);
        m.cylinder(0.1, 0.01, 0.0, 1, 12, Paint::glow(0xfff0c0, 0.7), along_x(c + Vec3::X * 0.065));
    }
    // A rounded cab with welded-up windows.
    m.bevel_box(Vec3::new(0.68, 0.56, 1.1), 0.3, 3, paint, at(Vec3::new(0.55, 0.65, 0.0)));
    for side in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.36, 0.17, 0.03), 0.03, 2, glass, at(Vec3::new(0.55, 0.85, side * 1.1)));
        panel(&mut m, Vec3::new(-1.3, 0.25, side * 1.08), Vec3::new(1.1, 0.3, 0.06), 2, side, paint, rivet);
        stripe(&mut m, Vec3::new(0.55, 0.45, side * 1.105), Vec3::new(0.45, 0.06, 0.01));
    }
    m.bevel_box(Vec3::new(0.04, 0.24, 0.85), 0.03, 2, glass, at(Vec3::new(1.23, 0.85, 0.0)));
    // The bed, its tailgate and a strapped log.
    m.bevel_box(Vec3::new(1.15, 0.08, 1.05), 0.06, 2, trim, at(Vec3::new(-1.3, -0.05, 0.0)));
    m.bevel_box(Vec3::new(0.07, 0.3, 1.08), 0.06, 2, paint, at(Vec3::new(-2.45, 0.25, 0.0)));
    strapped_log(&mut m, Vec3::new(-1.4, 0.18, -0.75), 0.75, 0.14);
    m
}

/// The technical's rocket rack, in its turret frame.
pub fn technical_turret(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let trim = trim_paint();
    m.cylinder(0.38, 0.09, 0.05, 2, 16, trim, at(Vec3::ZERO));
    m.bevel_box(Vec3::new(0.7, 0.34, 0.58), 0.2, 3, Paint::painted(colour, 0.45), at(Vec3::new(0.1, 0.5, 0.0)));
    for row in 0..2 {
        for col in 0..3 {
            let p = Vec3::new(0.81, 0.36 + row as f32 * 0.28, -0.32 + col as f32 * 0.32);
            m.cylinder(0.12, 0.03, 0.02, 1, 12, Paint::steel(0x141210, 0.9), along_x(p));
            m.cylinder(0.08, 0.06, 0.06, 2, 10, Paint::painted(0xd03a26, 0.3), along_x(p + Vec3::X * 0.03));
        }
    }
    stripe(&mut m, Vec3::new(0.1, 0.845, 0.0), Vec3::new(0.5, 0.01, 0.08));
    m
}

/// The buggy's pintle MG, in its turret frame.
pub fn buggy_turret() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let trim = trim_paint();
    pipe(&mut m, Vec3::ZERO, Vec3::Y * 0.7, 0.05, trim);
    m.bevel_box(Vec3::new(0.35, 0.1, 0.1), 0.06, 2, trim, at(Vec3::new(0.1, 0.72, 0.0)));
    m.cylinder(0.05, 0.4, 0.02, 1, 10, Paint::steel(0x2a2725, 0.4), along_x(Vec3::new(0.85, 0.74, 0.0)));
    ring_x(&mut m, Vec3::new(1.25, 0.74, 0.0), 0.08, 0.06, trim);
    m.bevel_box(Vec3::new(0.09, 0.11, 0.09), 0.04, 2, Paint::painted(0x5e6a3a, 0.5), at(Vec3::new(0.0, 0.62, 0.13)));
    m
}

/// THE WAR RIG: an armoured truck-tank. Hull half (7.5, 1.4, 3.5). Its
/// plates, engines, guns, rockets, flamer and mast are separate parts.
pub fn war_rig_mesh(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(colour, 0.7);
    let rivet = Paint::painted(colour, 0.75);
    let trim = Paint::steel(TRIM, 0.6);
    // Chassis rails and the inner armoured box.
    m.bevel_box(Vec3::new(7.2, 0.35, 2.4), 0.3, 3, trim, at(Vec3::new(0.0, -0.9, 0.0)));
    m.bevel_box(Vec3::new(6.4, 1.1, 3.2), 0.5, 3, paint, at(Vec3::new(-0.4, 0.2, 0.0)));
    // The cab: a raked armoured nose with slit windows.
    m.wedge(Vec3::new(1.2, 1.1, 3.1), 0.45, 3, 0.55, paint, at(Vec3::new(6.2, 0.2, 0.0)));
    for k in 0..3 {
        m.bevel_box(Vec3::new(0.06, 0.14, 0.7), 0.05, 2, Paint::steel(0x1a2228, 0.3), at(Vec3::new(6.12, 1.0, -2.0 + k as f32 * 2.0)));
    }
    // Deck plates with big rivets, bone stencil bands.
    for k in 0..4 {
        let x = -5.0 + k as f32 * 3.0;
        panel(&mut m, Vec3::new(x, 1.33, 0.0), Vec3::new(1.4, 0.08, 2.9), 1, 1.0, paint, rivet);
    }
    for side in [-1.0f32, 1.0] {
        stripe(&mut m, Vec3::new(5.2, 0.9, side * 3.22), Vec3::new(0.9, 0.14, 0.01));
        // Wheel arches.
        m.bevel_box(Vec3::new(7.0, 0.16, 0.6), 0.14, 3, trim, at(Vec3::new(-0.4, -0.15, side * 3.0)));
    }
    // The core, under the deck behind the turret.
    m.bevel_box(Vec3::new(1.0, 0.6, 1.0), 0.3, 3, Paint::steel(0x2a2422, 0.6), at(Vec3::new(-2.0, 1.2, 0.0)));
    m.cylinder(0.5, 0.05, 0.02, 1, 16, Paint::glow(0xff6020, 1.2), at(Vec3::new(-2.0, 1.82, 0.0)));
    m
}

/// A road wheel for the light chassis or the rig: a fat balloon tyre on a
/// dished hub, axle along Z, outer face at +Z.
pub fn truck_wheel(radius: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let w = radius * 0.7;
    m.torus(radius * 0.66, radius * 0.34, w, 18, Paint::rubber(0x24211f), Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2));
    m.cylinder(radius * 0.5, w * 0.42, radius * 0.08, 2, 14, Paint::steel(0x5a5450, 0.5), Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2));
    m.cylinder(radius * 0.2, w * 0.55, radius * 0.07, 2, 10, Paint::steel(BARE, 0.35), Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2));
    for k in 0..4 {
        let a = k as f32 / 4.0 * std::f32::consts::TAU;
        m.rivet(Vec3::new(a.cos() * radius * 0.34, a.sin() * radius * 0.34, w * 0.42), Vec3::Z, radius * 0.07, Paint::steel(BARE, 0.35));
    }
    m
}

#[cfg(test)]
mod shots {
    //! `cargo test -p arena-client --release -- --ignored shots_chassis`
    //! renders the three Garage classes up close (docs/shots/chassis.png;
    //! needs a GPU).
    use super::*;
    use crate::renderer::{Instance, Renderer};
    use crate::world::{RenderWorld, WorldRenderer};
    use arena_core::PartKind;

    /// One whole tank as a single static mesh in its own colour.
    fn whole_tank(class: ChassisClass, team: u32, gun: PartKind, parts: &[PartKind], turret_yaw: f32) -> MeshBuilder {
        let mut m = hull_mesh_class(class, team);
        let hull = Mat4::from_translation(Vec3::Y * HULL_Y);
        m.transform_from(0, hull);
        let turret = hull * turret_local(turret_yaw);
        let gun_xf = turret * gun_local(0.04);
        let mut add = |mut part: MeshBuilder, xf: Mat4| {
            part.transform_from(0, xf);
            let base = m.vertices.len() as u32;
            m.vertices.append(&mut part.vertices);
            m.indices.extend(part.indices.iter().map(|i| i + base));
        };
        add(turret_mesh_class(class, team), turret);
        add(crate::parts::gun_mesh(gun, team), gun_xf);
        for &p in parts {
            let frame = match arena_core::part_def(p).frame {
                arena_core::Frame::Hull => hull,
                arena_core::Frame::Turret => turret,
                arena_core::Frame::Gun => gun_xf,
            };
            add(crate::parts::part_mesh(p, team), frame);
        }
        for side in [-1.0f32, 1.0] {
            for x in WHEEL_X {
                let local = Mat4::from_rotation_translation(
                    Quat::from_rotation_y(if side < 0.0 { std::f32::consts::PI } else { 0.0 }),
                    Vec3::new(x, wheel_y(0.15), side * WHEEL_Z),
                );
                add(road_wheel_mesh(), hull * local);
            }
        }
        m
    }

    fn render(name: &str, build: impl FnOnce(&mut WorldRenderer) -> RenderWorld) {
        let (w, h) = (1600u32, 900u32);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
        let (device, queue) = pollster::block_on(crate::renderer::request_device(&adapter)).expect("device");
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut wr = WorldRenderer::new(Renderer::new(device, queue, format, w, h));
        let world = build(&mut wr);
        let texture = wr.renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        wr.draw(&texture.create_view(&Default::default()), &world);
        let r = &wr.renderer;
        let padded = (w * 4).div_ceil(256) * 256;
        let buffer = r.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (padded * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = r.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        r.queue.submit([enc.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        r.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let mapped = buffer.slice(..).get_mapped_range().expect("mapped");
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h as usize {
            px.extend_from_slice(&mapped[row * padded as usize..row * padded as usize + (w * 4) as usize]);
        }
        let path = format!("{}/../../docs/shots/{name}.png", env!("CARGO_MANIFEST_DIR"));
        image::save_buffer(&path, &px, w, h, image::ExtendedColorType::Rgba8).expect("save");
    }

    /// A light vehicle on its truck wheels, as one mesh in its hull frame
    /// standing on the ground (wheels at rest).
    fn whole_vehicle(kind: arena_core::VehicleKind, mut hull: MeshBuilder, turret: Option<MeshBuilder>) -> MeshBuilder {
        let tuning = arena_core::tuning::Tuning::default();
        let c = arena_core::tank::chassis_of(&tuning, kind);
        let scale = Vec3::from(c.hull_half) / Vec3::from(tuning.tank.hull_half);
        let add = |m: &mut MeshBuilder, mut part: MeshBuilder, xf: Mat4| {
            part.transform_from(0, xf);
            let base = m.vertices.len() as u32;
            m.vertices.append(&mut part.vertices);
            m.indices.extend(part.indices.iter().map(|i| i + base));
        };
        if let Some(t) = turret {
            add(&mut hull, t, at(Vec3::from(tuning.turret.pivot) * scale));
        }
        let n = c.wheels_per_side.max(1);
        let ray_len = c.suspension_travel + c.wheel_radius;
        for i in 0..2 * n {
            let side = if i < n { -1.0 } else { 1.0 };
            let k = i % n;
            let x = (n as f32 - 1.0) * 0.5 * c.wheel_spacing - k as f32 * c.wheel_spacing;
            let y = c.ray_origin_y - ray_len * 0.7 + c.wheel_radius;
            let local = Mat4::from_rotation_translation(Quat::from_rotation_y(if side < 0.0 { std::f32::consts::PI } else { 0.0 }), Vec3::new(x, y, side * c.track_half_width));
            add(&mut hull, truck_wheel(c.wheel_radius), local);
        }
        // Stand it on its wheels.
        let low = hull.vertices.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
        hull.transform_from(0, at(Vec3::Y * -low));
        hull
    }

    /// The raiders' light vehicles and the convoy tanker.
    #[test]
    #[ignore]
    fn shots_vehicles() {
        use arena_core::VehicleKind;
        render("vehicles", |wr| {
            wr.begin_scene();
            let r = &mut wr.renderer;
            let floor = r.add_mesh(&crate::terrain::flat(90.0));
            let mut statics = vec![Instance::new(floor, Mat4::IDENTITY)];
            let rows = [
                (whole_vehicle(VehicleKind::Buggy, buggy_mesh(0xc8322a), Some(buggy_turret())), Vec3::new(-8.0, 0.0, 3.0)),
                (whole_vehicle(VehicleKind::Technical, technical_mesh(RAIDER), Some(technical_turret(RAIDER))), Vec3::new(0.0, 0.0, 3.0)),
                (whole_vehicle(VehicleKind::FuelTanker, crate::ships::fuel_tanker(0x2f6ad0), None), Vec3::new(9.0, 0.0, 2.0)),
            ];
            for (m, p) in rows {
                let id = r.add_mesh(&m);
                statics.push(Instance::new(id, Mat4::from_rotation_translation(Quat::from_rotation_y(-0.55), p)));
            }
            wr.set_statics(statics);
            RenderWorld { camera: Some(crate::camera::Camera::look(Vec3::new(0.0, 7.0, 16.0), Vec3::new(0.0, 0.8, 1.0), 50.0)), ..Default::default() }
        });
    }

    /// The three Garage classes side by side, close up, each with a
    /// different gun and a couple of parts.
    #[test]
    #[ignore]
    fn shots_chassis() {
        render("chassis", |wr| {
            wr.begin_scene();
            let r = &mut wr.renderer;
            let floor = r.add_mesh(&crate::terrain::flat(90.0));
            let mut statics = vec![Instance::new(floor, Mat4::IDENTITY)];
            let rows = [
                (ChassisClass::Scout, RUSTBACK, PartKind::StubGun, vec![], -8.5),
                (ChassisClass::Brawler, DUST_SAINT, PartKind::HeavyCannon, vec![], 0.0),
                (ChassisClass::Hauler, 0x2f6ad0, PartKind::BigBore, vec![], 8.5),
                (ChassisClass::Scout, 0x5aa03a, PartKind::LongGun, vec![PartKind::Nitro, PartKind::SensorMast], -8.5),
                (ChassisClass::Brawler, 0xe0b020, PartKind::TwinBarrel, vec![PartKind::Coilgun, PartKind::Bulwark], 0.0),
                (ChassisClass::Hauler, 0xc8322a, PartKind::HeavyCannon, vec![PartKind::FlankPlates, PartKind::TwinEngines, PartKind::RocketRack], 8.5),
            ];
            for (i, (class, col, gun, parts, x)) in rows.into_iter().enumerate() {
                let z = if i < 3 { 2.0 } else { -7.0 };
                let id = r.add_mesh(&whole_tank(class, col, gun, &parts, 0.3));
                statics.push(Instance::new(id, Mat4::from_rotation_translation(Quat::from_rotation_y(-0.55), Vec3::new(x, 0.0, z))));
            }
            wr.set_statics(statics);
            RenderWorld { camera: Some(crate::camera::Camera::look(Vec3::new(0.0, 9.0, 17.0), Vec3::new(0.0, 0.5, -2.5), 50.0)), ..Default::default() }
        });
    }
}
