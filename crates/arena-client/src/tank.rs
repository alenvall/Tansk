//! The tank, built from the mesh kit in two modular parts: the hull (with
//! running gear and hull bolt-ons) and the turret (with mantlet and gun),
//! so the turret can traverse and later be swapped for upgrade variants.
//! Aimed at the target reference: bevelled riveted plates with dark panel
//! lines, dark metal trim bands, a chunky barrel with a reinforced muzzle,
//! bolted-on crates and boxes, team paint with a soft two-tone camo.

use glam::{Mat4, Quat, Vec3};

use crate::meshkit::{bevelled_cylinder_profile, Material, MeshBuilder, Paint};

/// The hull frame's origin (the simulation's hull centre) above the ground.
/// Meshes are modelled standing on y = 0 and shifted down by this.
pub const HULL_Y: f32 = 0.95;
/// The turret pivot in the hull frame (arena-core: (-0.3, 0.45, 0)).
pub const TURRET_PIVOT: Vec3 = Vec3::new(-0.3, 0.45, 0.0);
/// The gun frame in the turret frame (arena-core: (1.05, 0.42, 0)), plus a
/// small visual lift so the mantlet clears the deck.
pub const GUN_PIVOT: Vec3 = Vec3::new(1.05, 0.6, 0.0);
/// Exhaust stack tip, hull frame (for exhaust puffs).
pub const EXHAUST: Vec3 = Vec3::new(-2.57, 3.35 - HULL_Y, -0.8);
/// Road wheels: x of each of the five per side, |z| of the outer hub face.
pub const WHEEL_X: [f32; 5] = [2.2, 1.1, 0.0, -1.1, -2.2];
pub const WHEEL_Z: f32 = 1.81;
/// Wheel centre height in the hull frame for a compression (metres).
pub fn wheel_y(compression: f32) -> f32 {
    -0.7 + compression
}
// Where the old ground-frame turret ring sat, for the hull deck ring.
const TURRET_RING: Vec3 = Vec3::new(-0.3, HULL_Y + 0.45, 0.0);

pub const RUSTBACK: u32 = 0xe2661a;
pub const DUST_SAINT: u32 = 0x1b9a92;
const TRIM: u32 = 0x2a2725;
const BARE: u32 = 0x9a958c;
const TRACK: u32 = 0x2b2725;
const CRATE: u32 = 0x9a6a3e;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}
fn along_z(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), p)
}

/// The standard armour plate: a bevelled slab whose thin axis is `axis`,
/// facing `sign` along it, with a rivet row round its rim and a dark lip
/// under its edge, which reads as the panel line between plates.
pub fn plate(m: &mut MeshBuilder, centre: Vec3, half: Vec3, axis: usize, sign: f32, paint: Paint, rivet_paint: Paint) {
    let bevel = (half[axis] * 0.9).min(0.05);
    m.bevel_box(half, bevel, 2, paint, at(centre));
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let mut normal = Vec3::ZERO;
    normal[axis] = sign;
    // The panel line: a dark slab a touch larger than the plate, under it.
    let mut lip = half;
    lip[u] += 0.03;
    lip[v] += 0.03;
    lip[axis] *= 0.5;
    m.bevel_box(lip, lip[axis] * 0.8, 1, Paint::steel(0x1a1716, 0.7), at(centre - normal * half[axis] * 0.6));
    let inset = 0.1;
    let spacing = 0.3;
    let face = centre + normal * half[axis];
    for (along, across) in [(u, v), (v, u)] {
        let len = 2.0 * (half[along] - inset);
        if len <= 0.0 || half[across] <= inset {
            continue;
        }
        let count = (len / spacing).round().max(1.0) as u32;
        for side in [-1.0f32, 1.0] {
            for k in 0..=count {
                if along == v && (k == 0 || k == count) {
                    continue;
                }
                let mut p = face;
                p[along] += -half[along] + inset + len * k as f32 / count as f32;
                p[across] += side * (half[across] - inset);
                m.rivet(p, normal, 0.045, rivet_paint);
            }
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

/// The hull: running gear, track guards, deck and hull bolt-ons. +X
/// forward, +Z right, standing on y = 0.
pub fn hull_mesh(team: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::team(team, 0.35);
    let rivet = Paint::team(team, 0.2);
    let trim = Paint::steel(TRIM, 0.5);
    let bare = Paint::steel(BARE, 0.35);
    let track = Paint::rubber(TRACK);

    for side in [-1.0f32, 1.0] {
        let z = side * 1.42;
        m.bevel_box(Vec3::new(2.95, 0.45, 0.38), 0.44, 5, track, at(Vec3::new(0.0, 0.45, z)));
        for x in [-2.6f32, 2.6] {
            let w = Vec3::new(x, 0.45, z + side * 0.39);
            m.cylinder(0.3, 0.05, 0.02, 1, 8, trim, along_z(w));
            m.cylinder(0.1, 0.07, 0.02, 1, 8, bare, along_z(w));
        }
        m.bevel_box(Vec3::new(2.3, 0.4, 0.46), 0.06, 2, paint, at(Vec3::new(-0.1, 1.15, z)));
        m.wedge(Vec3::new(0.42, 0.4, 0.46), 0.06, 2, 0.4, paint, at(Vec3::new(2.62, 1.15, z)));
        m.wedge(
            Vec3::new(0.3, 0.4, 0.46),
            0.06,
            2,
            0.55,
            paint,
            Mat4::from_rotation_translation(Quat::from_rotation_y(std::f32::consts::PI), Vec3::new(-2.7, 1.15, z)),
        );
        for k in 0..3 {
            let x = -1.6 + k as f32 * 1.5;
            plate(&mut m, Vec3::new(x, 1.57, z), Vec3::new(0.7, 0.025, 0.38), 1, 1.0, paint, rivet);
            plate(&mut m, Vec3::new(x, 1.13, z + side * 0.47), Vec3::new(0.7, 0.3, 0.02), 2, side, paint, rivet);
        }
        m.bevel_box(Vec3::new(2.32, 0.06, 0.06), 0.035, 2, trim, at(Vec3::new(-0.1, 1.55, z + side * 0.47)));
        m.bevel_box(Vec3::new(2.32, 0.06, 0.06), 0.035, 2, trim, at(Vec3::new(-0.1, 0.76, z + side * 0.47)));
        m.bevel_box(Vec3::new(0.26, 0.16, 0.26), 0.04, 2, trim, at(Vec3::new(1.7, 1.76, z)));
        m.rivet(Vec3::new(1.7, 1.92, z), Vec3::Y, 0.06, bare);
        crate_box(&mut m, Vec3::new(-1.7, 1.83, z), Vec3::new(0.42, 0.24, 0.32));
    }

    m.bevel_box(Vec3::new(2.45, 0.48, 0.98), 0.06, 2, paint, at(Vec3::new(0.0, 1.06, 0.0)));
    m.wedge(Vec3::new(0.45, 0.48, 0.98), 0.06, 2, 0.3, paint, at(Vec3::new(2.85, 1.06, 0.0)));
    m.bevel_box(Vec3::new(0.07, 0.11, 1.0), 0.045, 2, trim, at(Vec3::new(3.28, 0.72, 0.0)));
    for k in 0..5 {
        m.rivet(Vec3::new(3.35, 0.72, -0.8 + k as f32 * 0.4), Vec3::X, 0.05, bare);
    }
    plate(&mut m, Vec3::new(1.75, 1.56, 0.0), Vec3::new(0.6, 0.03, 0.9), 1, 1.0, paint, rivet);
    plate(&mut m, Vec3::new(-1.95, 1.62, 0.0), Vec3::new(0.48, 0.09, 0.9), 1, 1.0, paint, rivet);
    for i in 0..5 {
        m.bevel_box(Vec3::new(0.05, 0.03, 0.6), 0.02, 1, trim, at(Vec3::new(-2.2 + i as f32 * 0.13, 1.73, 0.0)));
    }
    plate(&mut m, Vec3::new(-2.47, 1.06, 0.0), Vec3::new(0.03, 0.38, 0.9), 0, -1.0, paint, rivet);
    // The bent octagonal exhaust stack.
    let base = Vec3::new(-2.15, 1.7, -0.75);
    let bend = base + Vec3::Y * 1.25;
    let tip = bend + Vec3::new(-0.42, 0.38, -0.05);
    let out = (tip - bend).normalize();
    let soot = Paint::steel(0x2b2725, 0.75);
    octo_pipe(&mut m, base, bend, 0.13, soot);
    octo_pipe(&mut m, bend - Vec3::Y * 0.05, tip, 0.13, soot);
    octo_pipe(&mut m, bend - Vec3::Y * 0.09, bend + Vec3::Y * 0.09, 0.165, trim);
    octo_pipe(&mut m, base + Vec3::Y * 0.3, base + Vec3::Y * 0.45, 0.17, trim);
    octo_pipe(&mut m, tip - out * 0.06, tip + out * 0.01, 0.1, Paint::with(Material::BareSteel, 0x0c0a09, 0.9));
    // Turret ring.
    m.cylinder(1.05, 0.06, 0.02, 1, 24, trim, at(TURRET_RING + Vec3::Y * 0.22));
    m.transform_from(0, at(Vec3::Y * -HULL_Y));
    m
}

/// One road wheel, centred on its hub, axle along Z, outer face at +Z.
pub fn road_wheel_mesh() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let trim = Paint::steel(TRIM, 0.5);
    let bare = Paint::steel(BARE, 0.35);
    m.cylinder(0.35, 0.05, 0.025, 1, 8, trim, along_z(Vec3::ZERO));
    m.cylinder(0.14, 0.07, 0.02, 1, 8, bare, along_z(Vec3::Z * 0.02));
    for k in 0..5 {
        let a = k as f32 / 5.0 * std::f32::consts::TAU;
        m.rivet(Vec3::new(a.cos() * 0.24, a.sin() * 0.24, 0.05), Vec3::Z, 0.035, bare);
    }
    m
}

/// The standard turret with its gun, in turret space (origin on the ring).
pub fn turret_mesh(team: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::team(team, 0.35);
    let rivet = Paint::team(team, 0.2);
    let trim = Paint::steel(TRIM, 0.5);
    let t = Vec3::Y * 0.12;
    m.wedge(Vec3::new(1.3, 0.6, 1.12), 0.09, 2, 0.8, paint, at(t + Vec3::new(0.05, 0.72, 0.0)));
    for side in [-1.0f32, 1.0] {
        plate(&mut m, t + Vec3::new(-0.2, 0.66, side * 1.13), Vec3::new(0.85, 0.34, 0.02), 2, side, paint, rivet);
    }
    plate(&mut m, t + Vec3::new(-0.5, 1.33, -0.4), Vec3::new(0.6, 0.02, 0.52), 1, 1.0, paint, rivet);
    m.bevel_box(Vec3::new(0.06, 0.09, 1.15), 0.035, 2, trim, at(t + Vec3::new(-1.26, 1.05, 0.0)));
    m.bevel_box(Vec3::new(1.32, 0.05, 0.05), 0.03, 2, trim, at(t + Vec3::new(0.0, 0.17, 1.1)));
    m.bevel_box(Vec3::new(1.32, 0.05, 0.05), 0.03, 2, trim, at(t + Vec3::new(0.0, 0.17, -1.1)));
    m.bevel_box(Vec3::new(0.22, 0.13, 0.17), 0.04, 2, trim, at(t + Vec3::new(-0.85, 1.43, 0.6)));
    m.cylinder(0.38, 0.05, 0.03, 2, 16, paint, at(t + Vec3::new(-0.4, 1.36, 0.42)));
    m.cylinder(0.27, 0.025, 0.015, 1, 12, paint, at(t + Vec3::new(-0.4, 1.43, 0.42)));
    m.cylinder(0.05, 0.05, 0.01, 1, 6, trim, at(t + Vec3::new(-1.05, 1.37, -0.8)));
    m.cylinder(0.012, 0.6, 0.0, 1, 5, trim, at(t + Vec3::new(-1.05, 1.96, -0.8)));
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
    m.bevel_box(half, 0.01, 1, Paint::painted(BONE, 0.7), at(centre));
}

/// A light dune buggy: open tub, roll cage, big balloon tyres (instanced
/// separately), spare wheel, scrap plates. Hull half (2.2, 0.35, 1.1).
pub fn buggy_mesh(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(colour, 0.6);
    let rivet = Paint::painted(colour, 0.7);
    let trim = Paint::steel(TRIM, 0.5);
    let tube = Paint::steel(0x4a4440, 0.5);
    // Floor pan and tub.
    m.bevel_box(Vec3::new(2.0, 0.12, 0.8), 0.05, 2, trim, at(Vec3::new(0.0, -0.2, 0.0)));
    m.wedge(Vec3::new(0.9, 0.28, 0.8), 0.06, 2, 0.35, paint, at(Vec3::new(1.2, 0.05, 0.0)));
    m.bevel_box(Vec3::new(0.9, 0.3, 0.85), 0.06, 2, paint, at(Vec3::new(-1.1, 0.08, 0.0)));
    for side in [-1.0f32, 1.0] {
        plate(&mut m, Vec3::new(0.1, 0.05, side * 0.82), Vec3::new(0.65, 0.22, 0.03), 2, side, paint, rivet);
        stripe(&mut m, Vec3::new(1.3, 0.2, side * 0.81), Vec3::new(0.5, 0.04, 0.01));
    }
    // Roll cage.
    for side in [-1.0f32, 1.0] {
        let z = side * 0.7;
        m.tube(&[Vec3::new(0.9, -0.05, z), Vec3::new(0.4, 1.05, z * 0.9), Vec3::new(-0.6, 1.1, z * 0.9), Vec3::new(-1.2, 0.2, z)], 0.05, 6, tube);
    }
    m.tube(&[Vec3::new(0.4, 1.05, -0.63), Vec3::new(0.4, 1.05, 0.63)], 0.05, 6, tube);
    m.tube(&[Vec3::new(-0.6, 1.1, -0.63), Vec3::new(-0.6, 1.1, 0.63)], 0.05, 6, tube);
    // Seats, a spare wheel, a jerrycan.
    m.bevel_box(Vec3::new(0.25, 0.25, 0.25), 0.08, 2, Paint::matte(0x4a3a2a), at(Vec3::new(-0.2, 0.25, 0.32)));
    m.torus(0.32, 0.12, 0.22, 10, Paint::rubber(0x24211f), Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::new(-2.05, 0.35, 0.0)));
    m.bevel_box(Vec3::new(0.09, 0.22, 0.17), 0.03, 2, Paint::painted(0xb02a20, 0.6), at(Vec3::new(-1.6, 0.6, -0.5)));
    // Bull bar.
    m.tube(&[Vec3::new(2.15, -0.2, -0.7), Vec3::new(2.3, 0.3, -0.5), Vec3::new(2.3, 0.3, 0.5), Vec3::new(2.15, -0.2, 0.7)], 0.06, 6, tube);
    m
}

/// A pickup technical: cab, hood, bed (the rocket rack is its turret).
/// Hull half (2.6, 0.45, 1.2).
pub fn technical_mesh(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(colour, 0.7);
    let rivet = Paint::painted(colour, 0.8);
    let trim = Paint::steel(TRIM, 0.5);
    m.bevel_box(Vec3::new(2.5, 0.18, 1.0), 0.05, 2, trim, at(Vec3::new(0.0, -0.3, 0.0)));
    // Hood and grille.
    m.wedge(Vec3::new(0.8, 0.3, 1.05), 0.08, 2, 0.75, paint, at(Vec3::new(1.75, 0.15, 0.0)));
    for k in 0..5 {
        m.bevel_box(Vec3::new(0.03, 0.22, 0.08), 0.01, 1, trim, at(Vec3::new(2.56, 0.1, -0.6 + k as f32 * 0.3)));
    }
    // Cab with welded-up windows.
    m.bevel_box(Vec3::new(0.65, 0.55, 1.08), 0.08, 2, paint, at(Vec3::new(0.55, 0.65, 0.0)));
    for side in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.4, 0.18, 0.02), 0.02, 1, Paint::steel(0x1a2228, 0.4), at(Vec3::new(0.55, 0.85, side * 1.09)));
        plate(&mut m, Vec3::new(-1.3, 0.25, side * 1.08), Vec3::new(1.1, 0.3, 0.03), 2, side, paint, rivet);
        stripe(&mut m, Vec3::new(0.55, 0.5, side * 1.1), Vec3::new(0.5, 0.05, 0.01));
    }
    m.bevel_box(Vec3::new(0.05, 0.25, 0.95), 0.02, 1, Paint::steel(0x1a2228, 0.4), at(Vec3::new(1.21, 0.85, 0.0)));
    // The bed.
    m.bevel_box(Vec3::new(1.15, 0.08, 1.05), 0.03, 1, trim, at(Vec3::new(-1.3, -0.05, 0.0)));
    m.bevel_box(Vec3::new(0.05, 0.3, 1.05), 0.02, 1, paint, at(Vec3::new(-2.45, 0.25, 0.0)));
    m
}

/// The technical's rocket rack, in its turret frame.
pub fn technical_turret(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let trim = Paint::steel(TRIM, 0.5);
    m.cylinder(0.35, 0.08, 0.02, 1, 10, trim, at(Vec3::ZERO));
    m.bevel_box(Vec3::new(0.7, 0.32, 0.55), 0.05, 2, Paint::painted(colour, 0.6), at(Vec3::new(0.1, 0.5, 0.0)));
    for row in 0..2 {
        for col in 0..4 {
            let p = Vec3::new(0.81, 0.36 + row as f32 * 0.28, -0.39 + col as f32 * 0.26);
            m.cylinder(0.09, 0.02, 0.0, 1, 8, Paint::steel(0x141210, 0.9), Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p));
            m.cylinder(0.06, 0.03, 0.0, 1, 8, Paint::painted(0xb02a20, 0.3), Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p + Vec3::X * 0.02));
        }
    }
    stripe(&mut m, Vec3::new(0.1, 0.83, 0.0), Vec3::new(0.6, 0.01, 0.06));
    m
}

/// The buggy's pintle MG, in its turret frame.
pub fn buggy_turret() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let trim = Paint::steel(TRIM, 0.5);
    m.cylinder(0.04, 0.35, 0.0, 1, 6, trim, at(Vec3::Y * 0.35));
    m.bevel_box(Vec3::new(0.35, 0.08, 0.08), 0.02, 1, trim, at(Vec3::new(0.1, 0.72, 0.0)));
    m.cylinder(0.035, 0.45, 0.0, 1, 6, Paint::steel(0x2a2725, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), Vec3::new(0.85, 0.74, 0.0)));
    m.bevel_box(Vec3::new(0.08, 0.1, 0.08), 0.02, 1, Paint::painted(0x5e6a3a, 0.6), at(Vec3::new(0.0, 0.62, 0.12)));
    m
}

/// THE WAR RIG: an armoured truck-tank. Hull half (7.5, 1.4, 3.5). Its
/// plates, engines, guns, rockets, flamer and mast are separate parts.
pub fn war_rig_mesh(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(colour, 0.85);
    let rivet = Paint::painted(colour, 0.9);
    let trim = Paint::steel(TRIM, 0.6);
    // Chassis rails and the inner armoured box.
    m.bevel_box(Vec3::new(7.2, 0.35, 2.4), 0.08, 2, trim, at(Vec3::new(0.0, -0.9, 0.0)));
    m.bevel_box(Vec3::new(6.4, 1.1, 3.2), 0.15, 2, paint, at(Vec3::new(-0.4, 0.2, 0.0)));
    // The cab: a raked armoured nose.
    m.wedge(Vec3::new(1.2, 1.1, 3.1), 0.12, 2, 0.55, paint, at(Vec3::new(6.2, 0.2, 0.0)));
    for k in 0..3 {
        m.bevel_box(Vec3::new(0.05, 0.12, 0.7), 0.02, 1, Paint::steel(0x1a2228, 0.4), at(Vec3::new(6.1, 1.0, -2.0 + k as f32 * 2.0)));
    }
    // Deck plates with rivets, bone stencil bands.
    for k in 0..4 {
        let x = -5.0 + k as f32 * 3.0;
        plate(&mut m, Vec3::new(x, 1.33, 0.0), Vec3::new(1.4, 0.04, 2.9), 1, 1.0, paint, rivet);
    }
    for side in [-1.0f32, 1.0] {
        stripe(&mut m, Vec3::new(5.2, 0.9, side * 3.22), Vec3::new(0.9, 0.12, 0.01));
        // Wheel arches.
        m.bevel_box(Vec3::new(7.0, 0.12, 0.6), 0.05, 1, trim, at(Vec3::new(-0.4, -0.15, side * 3.0)));
    }
    // The core, under the deck behind the turret.
    m.bevel_box(Vec3::new(1.0, 0.6, 1.0), 0.1, 2, Paint::steel(0x2a2422, 0.6), at(Vec3::new(-2.0, 1.2, 0.0)));
    m.cylinder(0.5, 0.05, 0.02, 1, 12, Paint::glow(0xff6020, 1.2), at(Vec3::new(-2.0, 1.82, 0.0)));
    m
}

/// A road wheel for the light chassis or the rig: a fat tyre on a hub,
/// axle along Z, outer face at +Z.
pub fn truck_wheel(radius: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let w = radius * 0.6;
    m.torus(radius * 0.72, radius * 0.28, w, 12, Paint::rubber(0x24211f), Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2));
    m.cylinder(radius * 0.48, w * 0.45, 0.02, 1, 8, Paint::steel(0x5a5450, 0.5), Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2));
    m.cylinder(radius * 0.18, w * 0.55, 0.01, 1, 6, Paint::steel(BARE, 0.35), Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2));
    m
}
