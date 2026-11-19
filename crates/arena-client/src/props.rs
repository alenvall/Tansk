//! The set-dressing kit, in the same language as the tanks: bevelled
//! plates, rivets, trims, faceted rock. Each builder returns a mesh in its
//! own frame. Obstacle-shaped builders take the collider's half extents and
//! are centred on the box centre; free props stand on y = 0.

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};
use crate::shapes::hash;
use crate::tank::{crate_box, octo_pipe};

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}
fn along_x(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p)
}
fn along_z(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), p)
}

pub const CONTAINER_COLOURS: [u32; 4] = [0xb3302a, 0x2f5f9a, 0x5e6a3a, 0xc27a22];
const ROCK: Paint = Paint { color: Vec3::ONE, roughness: 0.9, material: Material::Rock, wear: 0.0 };

/// A shipping container of half extents `half` (long axis X), centred.
pub fn container(half: Vec3, colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let (hx, hy, hz) = (half.x, half.y, half.z);
    let body = Paint::painted(colour, 0.5);
    let frame = Paint::painted(colour, 0.7);
    let dark = Paint::steel(0x4a4642, 0.5);
    for side in [-1.0f32, 1.0] {
        let long = at(Vec3::new(0.0, 0.0, side * (hz - 0.05)));
        m.corrugated(2.0 * hx - 0.2, 2.0 * hy - 0.24, 0.3, 0.08, 0.01, body, long);
        let end = Mat4::from_rotation_translation(
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(side * (hx - 0.05), 0.0, 0.0),
        );
        m.corrugated(2.0 * hz - 0.2, 2.0 * hy - 0.24, 0.3, 0.06, 0.01, body, end);
    }
    m.bevel_box(Vec3::new(hx - 0.04, 0.05, hz - 0.04), 0.03, 1, body, at(Vec3::Y * (hy - 0.07)));
    for y in [-hy + 0.08, hy - 0.08] {
        for side in [-1.0f32, 1.0] {
            m.bevel_box(Vec3::new(hx, 0.08, 0.08), 0.03, 2, frame, at(Vec3::new(0.0, y, side * (hz - 0.08))));
            m.bevel_box(Vec3::new(0.08, 0.08, hz), 0.03, 2, frame, at(Vec3::new(side * (hx - 0.08), y, 0.0)));
        }
    }
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        m.bevel_box(Vec3::new(0.1, hy, 0.1), 0.03, 2, frame, at(Vec3::new(sx * (hx - 0.1), 0.0, sz * (hz - 0.1))));
        // Corner castings.
        for sy in [-1.0f32, 1.0] {
            m.bevel_box(Vec3::splat(0.13), 0.03, 1, dark, at(Vec3::new(sx * (hx - 0.1), sy * (hy - 0.1), sz * (hz - 0.1))));
        }
    }
    // Doors on +X: a seam, four locking bars with handles.
    m.bevel_box(Vec3::new(0.03, hy - 0.2, 0.02), 0.01, 1, dark, at(Vec3::new(hx + 0.01, 0.0, 0.0)));
    for z in [-0.8f32, -0.35, 0.35, 0.8] {
        let z = z * hz / 1.22;
        m.cylinder(0.03, hy - 0.2, 0.0, 1, 6, Paint::steel(0x7a766e, 0.4), at(Vec3::new(hx + 0.05, 0.0, z)));
        m.bevel_box(Vec3::new(0.05, 0.03, 0.12), 0.01, 1, dark, at(Vec3::new(hx + 0.08, -0.2, z + 0.1)));
    }
    m
}

/// A concrete jersey barrier (`hazard`: painted in yellow-black stripes),
/// half extents `half`, centred.
pub fn barrier(half: Vec3, hazard: bool) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = if hazard { Paint::with(Material::Hazard, 0xffffff, 0.6) } else { Paint::matte(0xa49a8c) };
    let (hx, hy, hz) = (half.x, half.y, half.z);
    let base_h = hy * 0.35;
    m.bevel_box(Vec3::new(hx, base_h, hz), 0.05, 2, paint, at(Vec3::new(0.0, -hy + base_h, 0.0)));
    m.bevel_box(Vec3::new(hx - 0.02, hy * 0.35, hz * 0.7), 0.05, 2, paint, at(Vec3::new(0.0, -hy + base_h * 2.0 + hy * 0.3, 0.0)));
    m.bevel_box(Vec3::new(hx - 0.04, hy * 0.18, hz * 0.45), 0.05, 2, paint, at(Vec3::new(0.0, hy - hy * 0.18, 0.0)));
    // Lifting loops.
    for x in [-hx * 0.6, hx * 0.6] {
        m.tube(&[Vec3::new(x - 0.12, hy, 0.0), Vec3::new(x, hy + 0.12, 0.0), Vec3::new(x + 0.12, hy, 0.0)], 0.025, 6, Paint::steel(0x5a5650, 0.5));
    }
    m
}

/// A line of sandbags filling `half` (long axis X), centred.
pub fn sandbags(half: Vec3, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let rows = ((2.0 * half.y) / 0.26).round().max(1.0) as u32;
    let bag = Vec3::new(0.36, 0.13, 0.24);
    for r in 0..rows {
        let y = -half.y + bag.y + r as f32 * 2.0 * bag.y * 0.92;
        let count = ((2.0 * half.x) / (2.0 * bag.x * 0.95)).floor() as i32;
        for depth in [-1.0f32, 1.0] {
            if half.z < 0.4 && depth < 0.0 {
                continue;
            }
            let z = depth * (half.z - bag.z).max(0.0) * 0.8;
            for k in 0..count {
                let i = r * 97 + k as u32 * 3 + (depth > 0.0) as u32;
                let x = -half.x + bag.x + k as f32 * 2.0 * bag.x * 0.95 + if r % 2 == 1 { bag.x * 0.9 } else { 0.0 };
                if x > half.x - bag.x * 0.5 {
                    continue;
                }
                let tone = [0xa8925f, 0x9a8656, 0xb39e6c][(hash(i, seed) * 3.0) as usize % 3];
                let rot = Quat::from_rotation_y((hash(i, seed + 1) - 0.5) * 0.3) * Quat::from_rotation_z((hash(i, seed + 2) - 0.5) * 0.12);
                m.bevel_box(bag, 0.11, 3, Paint::matte(tone), Mat4::from_rotation_translation(rot, Vec3::new(x, y, z)));
            }
        }
    }
    m
}

/// A rusted silo of radius `r`, half height `hy`, centred on its box.
pub fn silo(r: f32, hy: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(0xd6a21e, 0.75);
    let trim = Paint::painted(0x8a6418, 0.8);
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let profile = [
        (0.0, -hy, Vec2::NEG_Y),
        (r, -hy, Vec2::NEG_Y),
        (r, -hy, Vec2::X),
        (r, hy - 0.6, Vec2::X),
        (r, hy - 0.6, n(1.0, 0.6)),
        (r * 0.25, hy + r * 0.45, n(0.5, 1.0)),
        (r * 0.25, hy + r * 0.45, Vec2::Y),
        (0.0, hy + r * 0.45, Vec2::Y),
    ];
    m.lathe(&profile, 24, true, paint, Mat4::IDENTITY);
    // Bands, a top hatch, a ladder.
    let mut y = -hy + 1.2;
    while y < hy - 0.8 {
        m.cylinder(r + 0.05, 0.08, 0.03, 1, 24, trim, at(Vec3::Y * y));
        y += 2.2;
    }
    m.cylinder(r * 0.3, 0.15, 0.04, 1, 12, trim, at(Vec3::Y * (hy + r * 0.5)));
    let ladder = Paint::steel(0x4d4740, 0.6);
    for side in [-0.25f32, 0.25] {
        m.bevel_box(Vec3::new(0.04, hy, 0.04), 0.01, 1, ladder, at(Vec3::new(r + 0.25, 0.0, side)));
    }
    let mut ry = -hy + 0.4;
    while ry < hy {
        m.bevel_box(Vec3::new(0.03, 0.03, 0.27), 0.01, 1, ladder, at(Vec3::new(r + 0.25, ry, 0.0)));
        ry += 0.45;
    }
    m
}

/// A faceted sandstone block sized to a collider box, centred.
pub fn rock_block(half: Vec3, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.cliff(seed, Vec2::new(half.x * 1.12, half.z * 1.12), 2.0 * half.y + 0.4, 9, 3, ROCK, at(Vec3::Y * (-half.y - 0.2)));
    m
}

/// A mesa block: wide cliff bands with a broken crown, sized to a box.
pub fn mesa_block(half: Vec3, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let h = 2.0 * half.y + 0.4;
    m.cliff(seed, Vec2::new(half.x * 1.08, half.z * 1.08), h * 0.78, 13, 4, ROCK, at(Vec3::Y * (-half.y - 0.2)));
    for k in 0..3u32 {
        let off = Vec3::new((hash(k, seed + 5) - 0.5) * half.x, 0.0, (hash(k, seed + 6) - 0.5) * half.z);
        let r = Vec2::new(half.x, half.z) * (0.3 + 0.25 * hash(k, seed + 7));
        m.cliff(seed + 11 + k, r, h * (0.25 + 0.15 * hash(k, seed + 8)), 8, 2, ROCK, at(off + Vec3::Y * (-half.y - 0.2 + h * 0.74)));
    }
    m
}

/// Rubble: faceted boulders scattered on a ring round `radius`.
pub fn rubble(radius: f32, count: u32, size: f32, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    for i in 0..count {
        let a = hash(i, seed) * std::f32::consts::TAU;
        let r = radius * (0.85 + 0.35 * hash(i, seed + 1));
        let s = size * (0.35 + 0.9 * hash(i, seed + 2));
        let p = Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        m.rock_blob(seed + i, Vec3::new(s, s * 0.7, s * 0.85), ROCK, Mat4::from_rotation_translation(Quat::from_rotation_y(a * 3.0), p));
    }
    m
}

/// A beached ship hull from its three boxes (hull, bow, bridge), in the
/// obstacle frame.
pub fn ship_hull(boxes: &[(Vec3, Vec3)], seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let hull = Paint::painted(0x6e3a2a, 0.95);
    let upper = Paint::painted(0x2f4a5a, 0.85);
    let white = Paint::painted(0xd8d2c4, 0.8);
    let dark = Paint::steel(0x2a2624, 0.7);
    if let Some(&(c, h)) = boxes.first() {
        m.bevel_box(h, 0.4, 2, hull, at(c));
        // Waterline band and deck rail.
        m.bevel_box(Vec3::new(h.x + 0.02, 0.35, h.z + 0.02), 0.1, 1, upper, at(c + Vec3::Y * (h.y * 0.45)));
        m.bevel_box(Vec3::new(h.x, 0.08, 0.06), 0.03, 1, dark, at(c + Vec3::new(0.0, h.y + 0.5, h.z - 0.1)));
        m.bevel_box(Vec3::new(h.x, 0.08, 0.06), 0.03, 1, dark, at(c + Vec3::new(0.0, h.y + 0.5, -h.z + 0.1)));
        // Rivet rows down the side plates.
        let mut x = -h.x + 1.0;
        while x < h.x - 0.5 {
            for side in [-1.0f32, 1.0] {
                for y in [-h.y * 0.2, h.y * 0.15] {
                    m.rivet(c + Vec3::new(x, y, side * h.z), Vec3::Z * side, 0.09, hull);
                }
            }
            x += 1.4;
        }
        // Plate seams.
        let mut x = -h.x + 3.5;
        while x < h.x {
            for side in [-1.0f32, 1.0] {
                m.bevel_box(Vec3::new(0.05, h.y * 0.9, 0.04), 0.02, 1, dark, at(c + Vec3::new(x, -h.y * 0.05, side * (h.z + 0.01))));
            }
            x += 4.0 + 2.0 * hash(x as u32, seed);
        }
    }
    if let Some(&(c, h)) = boxes.get(1) {
        // The bow: a wedge narrowing forward.
        m.wedge(Vec3::new(h.x, h.y, h.z), 0.3, 2, 0.7, hull, at(c) * Mat4::from_rotation_y(0.0));
        m.wedge(Vec3::new(h.x * 0.9, h.y * 0.5, h.z * 0.6), 0.2, 1, 0.3, hull, at(c + Vec3::new(h.x * 0.6, h.y * 0.2, 0.0)));
    }
    if let Some(&(c, h)) = boxes.get(2) {
        m.bevel_box(h, 0.15, 2, white, at(c));
        // Bridge windows.
        let mut z = -h.z + 0.6;
        while z < h.z - 0.4 {
            m.bevel_box(Vec3::new(0.05, 0.4, 0.3), 0.03, 1, Paint::steel(0x1a2228, 0.3), at(c + Vec3::new(h.x + 0.02, h.y * 0.45, z)));
            z += 0.9;
        }
        m.bevel_box(Vec3::new(h.x + 0.3, 0.12, h.z + 0.3), 0.05, 1, upper, at(c + Vec3::Y * (h.y + 0.1)));
        m.cylinder(0.12, 3.0, 0.02, 1, 8, dark, at(c + Vec3::new(-h.x * 0.4, h.y + 3.0, 0.0)));
        m.cylinder(0.7, 1.4, 0.1, 1, 12, Paint::painted(0x9a2a20, 0.8), at(c + Vec3::new(-h.x * 0.6, h.y + 1.4, 0.0)));
    }
    m
}

/// A flattened, rusted car of half extents `half` (long axis X), centred.
pub fn car_wreck(half: Vec3, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let colour = [0x56707a, 0x8a7a5a, 0x7a3a2a, 0xb8b0a0][(hash(1, seed) * 4.0) as usize % 4];
    let body = Paint::painted(colour, 0.95);
    let (hx, hy, hz) = (half.x, half.y, half.z);
    m.bevel_box(Vec3::new(hx, hy * 0.6, hz), 0.08, 2, body, at(Vec3::new(0.0, -hy * 0.35, 0.0)));
    m.wedge(Vec3::new(hx * 0.48, hy * 0.45, hz * 0.92), 0.06, 2, 0.45, body, at(Vec3::new(-hx * 0.15, hy * 0.6, 0.0)));
    let hole = Paint::steel(0x231c18, 0.9);
    for side in [-1.0f32, 1.0] {
        for x in [-hx * 0.4, -hx * 0.02] {
            m.bevel_box(Vec3::new(hx * 0.16, hy * 0.18, 0.02), 0.02, 1, hole, at(Vec3::new(x, hy * 0.55, side * hz * 0.925)));
        }
        // Wheel arches with bare rims, tyres gone.
        for x in [-hx * 0.62, hx * 0.62] {
            m.cylinder(hy * 0.55, 0.08, 0.03, 1, 10, Paint::steel(0x5a4a3c, 0.8), along_z(Vec3::new(x, -hy * 0.55, side * (hz - 0.05))));
        }
    }
    m.bevel_box(Vec3::new(0.07, 0.09, hz + 0.03), 0.03, 1, Paint::steel(0x6b5a4a, 0.7), at(Vec3::new(hx + 0.03, -hy * 0.4, 0.0)));
    m.bevel_box(
        Vec3::new(0.07, 0.09, hz),
        0.03,
        1,
        Paint::steel(0x6b5a4a, 0.7),
        Mat4::from_rotation_translation(Quat::from_rotation_x(0.35), Vec3::new(-hx - 0.03, -hy * 0.6, 0.1)),
    );
    m
}

/// A big rusted pipe section lying along X, half extents `half`, centred.
pub fn pipe(half: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let r = half.y.min(half.z);
    let rust = Paint::painted(0x7a4a30, 1.0);
    m.cylinder(r * 0.95, half.x, 0.04, 1, 18, rust, along_x(Vec3::ZERO));
    for x in [-half.x + 0.15, half.x - 0.15] {
        m.cylinder(r * 1.04, 0.12, 0.04, 2, 18, Paint::painted(0x5a3a28, 1.0), along_x(Vec3::new(x, 0.0, 0.0)));
        m.cylinder(r * 0.8, 0.01, 0.0, 1, 16, Paint::steel(0x140f0c, 0.9), along_x(Vec3::new(x + x.signum() * 0.13, 0.0, 0.0)));
        for k in 0..10 {
            let a = k as f32 / 10.0 * std::f32::consts::TAU;
            let n = Vec3::new(0.0, a.sin(), a.cos());
            m.rivet(Vec3::new(x + x.signum() * 0.12, 0.0, 0.0) + n * r * 0.92, Vec3::X * x.signum(), 0.05, Paint::steel(0x8a7a6a, 0.5));
        }
    }
    m
}

/// A broken pier deck chunk, half extents `half`, centred.
pub fn pier_ruin(half: Vec3, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let wood = Paint::with(Material::Wood, 0x7a5a3a, 0.8);
    let beam = Paint::with(Material::Wood, 0x5a4028, 0.8);
    for z in [-half.z * 0.7, 0.0, half.z * 0.7] {
        m.bevel_box(Vec3::new(half.x, 0.15, 0.15), 0.03, 1, beam, at(Vec3::new(0.0, -half.y + 0.35, z)));
    }
    let mut x = -half.x + 0.15;
    let mut i = 0;
    while x < half.x - 0.1 {
        i += 1;
        if hash(i, seed) < 0.12 {
            x += 0.32;
            continue;
        }
        let len = half.z * (0.75 + 0.25 * hash(i, seed + 1));
        let rot = Quat::from_rotation_x((hash(i, seed + 2) - 0.5) * 0.08);
        m.bevel_box(Vec3::new(0.14, 0.05, len), 0.02, 1, wood, Mat4::from_rotation_translation(rot, Vec3::new(x, -half.y + 0.55, (hash(i, seed + 3) - 0.5) * 0.3)));
        x += 0.32;
    }
    for (sx, sz) in [(-0.8f32, -0.7f32), (0.8, -0.7), (-0.8, 0.7), (0.8, 0.7)] {
        let tilt = Quat::from_rotation_z((hash((sx * 10.0) as u32 + 3, seed) - 0.5) * 0.3);
        m.bevel_box(Vec3::new(0.16, half.y + 0.4, 0.16), 0.04, 1, beam, Mat4::from_rotation_translation(tilt, Vec3::new(sx * half.x, 0.2, sz * half.z)));
    }
    m
}

/// One highway deck segment, half extents `half`, centred; markings come
/// from the asphalt material in this frame.
pub fn highway(half: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(half, 0.08, 1, Paint::with(Material::Asphalt, 0x3a3836, 0.85), Mat4::IDENTITY);
    m
}

// ── Free props (stand on y = 0) ──────────────────────────────────────────

pub const DRUM_R: f32 = 0.29;
pub const DRUM_HALF: f32 = 0.44;

/// An oil drum, centred (as rapier's cylinder is).
pub fn oil_drum(colour: u32, wear: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(colour, wear);
    let (r, h) = (DRUM_R, DRUM_HALF);
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let mut profile = vec![(0.0, h - 0.02, Vec2::Y), (r - 0.03, h - 0.02, Vec2::Y), (r - 0.025, h, n(0.3, 1.0))];
    profile.push((r, h - 0.015, n(1.0, 0.4)));
    for hy in [0.15f32, -0.15] {
        let y = hy * h * 2.0;
        profile.push((r, y + 0.05, Vec2::X));
        profile.push((r + 0.014, y + 0.02, n(1.0, 0.5)));
        profile.push((r + 0.014, y - 0.02, n(1.0, -0.5)));
        profile.push((r, y - 0.05, Vec2::X));
    }
    profile.push((r, -h + 0.015, n(1.0, -0.4)));
    profile.push((r - 0.025, -h, n(0.3, -1.0)));
    profile.push((0.0, -h, Vec2::NEG_Y));
    m.lathe(&profile, 16, false, paint, Mat4::IDENTITY);
    m.cylinder(0.035, 0.012, 0.005, 1, 8, Paint::steel(0x9a958c, 0.5), at(Vec3::new(0.15, h - 0.01, 0.0)));
    m
}

/// A Czech hedgehog: three crossed riveted angle beams.
pub fn hedgehog() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let steel = Paint::painted(0x5a5048, 0.9);
    let c = Vec3::Y * 0.75;
    for (k, axis) in [Vec3::new(1.0, 1.0, 0.0), Vec3::new(-1.0, 1.0, 0.6), Vec3::new(0.2, 1.0, -1.0)].into_iter().enumerate() {
        let d = axis.normalize();
        let rot = Quat::from_rotation_arc(Vec3::X, d);
        m.bevel_box(Vec3::new(1.05, 0.06, 0.18), 0.02, 1, steel, Mat4::from_rotation_translation(rot, c));
        m.bevel_box(Vec3::new(1.05, 0.18, 0.06), 0.02, 1, steel, Mat4::from_rotation_translation(rot, c + rot * Vec3::new(0.0, 0.12, -0.12)));
        m.rivet(c + rot * Vec3::new(0.0, 0.07, 0.0), rot * Vec3::Y, 0.05 + 0.002 * k as f32, Paint::steel(0x8a7a6a, 0.5));
    }
    m
}

/// A coil of razor wire, `len` long along X, on y = 0.
pub fn razor_wire(len: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let wire = Paint::steel(0x8f8a82, 0.3);
    let r = 0.42;
    let turns = (len / 0.32) as u32;
    let pts: Vec<Vec3> = (0..=turns * 10)
        .map(|i| {
            let t = i as f32 / 10.0;
            let a = t * std::f32::consts::TAU;
            Vec3::new(-len / 2.0 + t * len / turns as f32, r + r * a.sin(), r * a.cos())
        })
        .collect();
    m.tube(&pts, 0.012, 4, wire);
    for x in [-len / 2.0, len / 2.0] {
        m.bevel_box(Vec3::new(0.04, 0.5, 0.04), 0.01, 1, Paint::steel(0x4a3a2e, 0.7), at(Vec3::new(x, 0.5, 0.0)));
    }
    m
}

/// A wooden power-line pole with a cross arm and insulators. Wire anchor
/// points are at `POLE_ANCHORS`.
pub fn power_pole() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let wood = Paint::with(Material::Wood, 0x6a4a30, 0.8);
    m.cylinder(0.16, 4.5, 0.02, 1, 8, wood, at(Vec3::Y * 4.5));
    m.bevel_box(Vec3::new(0.08, 0.08, 1.2), 0.02, 1, wood, at(Vec3::new(0.0, 8.4, 0.0)));
    for z in POLE_ANCHORS {
        m.cylinder(0.05, 0.1, 0.02, 1, 8, Paint::matte(0x6aa0b0), at(Vec3::new(0.0, 8.6, z)));
        m.cylinder(0.06, 0.03, 0.01, 1, 8, Paint::steel(0x4a4a48, 0.4), at(Vec3::new(0.0, 8.55, z)));
    }
    // A transformer can on some.
    m.cylinder(0.26, 0.4, 0.05, 1, 12, Paint::painted(0x7a7a72, 0.6), at(Vec3::new(0.3, 7.2, 0.0)));
    m
}

pub const POLE_ANCHORS: [f32; 3] = [-1.0, 0.0, 1.0];
pub const POLE_WIRE_HEIGHT: f32 = 8.7;

/// Sagging wires between two pole tops (world space).
pub fn wires(a: Vec3, b: Vec3, sag: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let cable = Paint::steel(0x1e1c1b, 0.6);
    let d = (b - a).normalize();
    let side = d.cross(Vec3::Y).normalize_or(Vec3::Z);
    for z in POLE_ANCHORS {
        let (p0, p1) = (a + side * z, b + side * z);
        let pts: Vec<Vec3> = (0..=12)
            .map(|i| {
                let t = i as f32 / 12.0;
                p0.lerp(p1, t) - Vec3::Y * sag * 4.0 * t * (1.0 - t)
            })
            .collect();
        m.tube(&pts, 0.025, 4, cable);
    }
    m
}

/// A tyre lying flat or a stack of `count`.
pub fn tyres(count: u32, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let rubber = Paint::rubber(0x24211f);
    for k in 0..count {
        let off = Vec3::new((hash(k, seed) - 0.5) * 0.12, 0.13 + k as f32 * 0.25, (hash(k, seed + 1) - 0.5) * 0.12);
        m.torus(0.36, 0.13, 0.25, 16, rubber, Mat4::from_rotation_translation(Quat::from_rotation_y(k as f32), off));
    }
    m
}

/// A stack of wooden crates.
pub fn crates(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    crate_box(&mut m, Vec3::new(0.0, 0.45, 0.0), Vec3::new(0.6, 0.45, 0.5));
    crate_box(&mut m, Vec3::new(1.2, 0.35, 0.15), Vec3::new(0.45, 0.35, 0.4));
    if hash(0, seed) > 0.3 {
        crate_box(&mut m, Vec3::new(0.15, 1.2, 0.0), Vec3::new(0.45, 0.3, 0.4));
    }
    m
}

/// A horizontal yellow fuel tank on two saddles.
pub fn fuel_tank() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(0xdcaa22, 0.6);
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let (r, h) = (1.1f32, 2.6f32);
    let profile = [
        (0.0, h + 0.4, Vec2::Y),
        (r * 0.6, h + 0.3, n(0.4, 1.0)),
        (r, h, n(1.0, 0.4)),
        (r, -h, n(1.0, -0.4)),
        (r * 0.6, -h - 0.3, n(0.4, -1.0)),
        (0.0, -h - 0.4, Vec2::NEG_Y),
    ];
    m.lathe(&profile, 18, false, paint, along_x(Vec3::Y * 1.6));
    for x in [-1.6f32, 1.6] {
        m.bevel_box(Vec3::new(0.25, 0.4, 0.9), 0.04, 1, Paint::painted(0x5a5048, 0.8), at(Vec3::new(x, 0.4, 0.0)));
        m.cylinder(r + 0.04, 0.08, 0.02, 1, 18, Paint::painted(0x8a6418, 0.8), along_x(Vec3::new(x, 1.6, 0.0)));
    }
    octo_pipe(&mut m, Vec3::new(1.0, 2.6, 0.0), Vec3::new(1.0, 3.1, 0.0), 0.12, Paint::steel(0x4a4642, 0.5));
    // Hazard plate.
    m.bevel_box(Vec3::new(0.6, 0.35, 0.02), 0.02, 1, Paint::with(Material::Hazard, 0xffffff, 0.6), at(Vec3::new(0.0, 1.6, r + 0.01)));
    m
}

/// A faceted saguaro.
pub fn cactus(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let green = Paint::matte(0x6e9a35);
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let stalk = |m: &mut MeshBuilder, r: f32, h: f32, xf: Mat4| {
        let profile = [
            (r * 0.9, 0.0, Vec2::X),
            (r, h * 0.1, Vec2::X),
            (r, h - r, Vec2::X),
            (r * 0.7, h - r * 0.3, n(0.7, 0.7)),
            (0.0, h, Vec2::Y),
        ];
        m.lathe(&profile, 8, true, green, xf);
    };
    let h = 2.8 + 1.2 * hash(seed, 1);
    stalk(&mut m, 0.28, h, Mat4::IDENTITY);
    for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        if hash(seed, 9 + k as u32) < 0.25 {
            continue;
        }
        let y = h * (0.35 + 0.2 * hash(seed, 2 + k as u32));
        let elbow = Vec3::new(side * 0.55, y, 0.0);
        let arm = Mat4::from_rotation_translation(Quat::from_rotation_z(-side * std::f32::consts::FRAC_PI_2), Vec3::new(0.0, y, 0.0));
        stalk(&mut m, 0.17, 0.6, arm);
        stalk(&mut m, 0.18, 0.8 + 0.7 * hash(seed, 4 + k as u32), at(elbow - Vec3::Y * 0.05));
    }
    m
}

/// A small faceted rock or boulder, on y = 0.
pub fn boulder(seed: u32, size: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.rock_blob(seed, Vec3::new(size, size * 0.7, size * 0.85), ROCK, Mat4::IDENTITY);
    m
}

/// A scrap pickup: a chunky icon object per kind (index 0 steel plates,
/// 1 powder crate, 2 volts coil, 3 fuel can, 4 repair wrench), on y = 0.
pub fn pickup(kind: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    match kind {
        0 => {
            for k in 0..3 {
                let rot = Quat::from_rotation_y(k as f32 * 0.5);
                m.bevel_box(Vec3::new(0.45, 0.05, 0.35), 0.03, 2, Paint::painted(0x9a9a92, 0.6), Mat4::from_rotation_translation(rot, Vec3::Y * (0.06 + k as f32 * 0.11)));
            }
        }
        1 => {
            m.bevel_box(Vec3::new(0.45, 0.3, 0.32), 0.04, 2, Paint::painted(0xb02a20, 0.4), at(Vec3::Y * 0.3));
            m.bevel_box(Vec3::new(0.46, 0.05, 0.33), 0.02, 1, Paint::steel(0x2a2725, 0.5), at(Vec3::Y * 0.45));
            m.bevel_box(Vec3::new(0.2, 0.12, 0.02), 0.01, 1, Paint::glow(0xffd060, 1.2), at(Vec3::new(0.0, 0.28, 0.33)));
        }
        2 => {
            m.cylinder(0.22, 0.35, 0.04, 2, 12, Paint::steel(0x3a3e44, 0.4), at(Vec3::Y * 0.35));
            for k in 0..4 {
                m.cylinder(0.26, 0.03, 0.01, 1, 12, Paint::glow(0x40a8ff, 2.5), at(Vec3::Y * (0.12 + k as f32 * 0.15)));
            }
        }
        3 => {
            m.bevel_box(Vec3::new(0.12, 0.3, 0.24), 0.04, 2, Paint::painted(0xe2b21e, 0.4), at(Vec3::Y * 0.3));
            octo_pipe(&mut m, Vec3::new(0.0, 0.6, 0.15), Vec3::new(0.0, 0.72, 0.2), 0.04, Paint::steel(0x2a2725, 0.5));
        }
        _ => {
            let steel = Paint::steel(0xb8b4aa, 0.3);
            m.bevel_box(Vec3::new(0.06, 0.04, 0.4), 0.02, 1, steel, at(Vec3::Y * 0.1));
            m.cylinder(0.12, 0.04, 0.01, 1, 8, steel, at(Vec3::new(0.0, 0.1, 0.45)));
            m.bevel_box(Vec3::new(0.08, 0.04, 0.06), 0.01, 1, Paint::glow(0x50ff80, 1.5), at(Vec3::new(0.0, 0.12, -0.2)));
        }
    }
    m
}

// ── Wave 2: crane, launch plate, specials ───────────────────────────────

/// A dockside crane from its two boxes (mast, jib), in the obstacle frame
/// (standing on y = 0): a lattice tower, a cab, the jib with a counterweight.
pub fn crane(mast: (Vec3, Vec3), jib: (Vec3, Vec3)) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let yellow = Paint::painted(0xd9a21c, 0.7);
    let dark = Paint::steel(0x3a3632, 0.6);
    let (mc, mh) = mast;
    let top = mc.y + mh.y;
    // Four corner legs and cross braces.
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let c = Vec3::new(mc.x + sx * (mh.x - 0.15), 0.0, mc.z + sz * (mh.z - 0.15));
        m.bevel_box(Vec3::new(0.15, top * 0.5, 0.15), 0.04, 1, yellow, at(c + Vec3::Y * top * 0.5));
    }
    let mut y = 1.5;
    let mut k = 0;
    while y < top - 1.0 {
        for side in [-1.0f32, 1.0] {
            let rot = Quat::from_rotation_z(if k % 2 == 0 { 0.78 } else { -0.78 });
            m.bevel_box(Vec3::new(mh.x * 1.38, 0.06, 0.06), 0.02, 1, yellow, Mat4::from_rotation_translation(rot, Vec3::new(mc.x, y + 1.0, mc.z + side * (mh.z - 0.15))));
            let rot = Quat::from_rotation_x(if k % 2 == 0 { 0.78 } else { -0.78 });
            m.bevel_box(Vec3::new(0.06, 0.06, mh.z * 1.38), 0.02, 1, yellow, Mat4::from_rotation_translation(rot, Vec3::new(mc.x + side * (mh.x - 0.15), y + 1.0, mc.z)));
        }
        m.bevel_box(Vec3::new(mh.x, 0.08, mh.z), 0.03, 1, yellow, at(Vec3::new(mc.x, y, mc.z)));
        y += 2.0;
        k += 1;
    }
    // Cab under the jib.
    m.bevel_box(Vec3::new(1.3, 1.0, 1.2), 0.08, 2, yellow, at(Vec3::new(mc.x + 0.6, top - 1.2, mc.z)));
    m.bevel_box(Vec3::new(0.05, 0.5, 0.9), 0.02, 1, Paint::steel(0x1a2228, 0.3), at(Vec3::new(mc.x + 1.92, top - 1.0, mc.z)));
    // Jib and counter-jib.
    let (jc, jh) = jib;
    m.bevel_box(jh, 0.1, 2, yellow, at(jc));
    for k in 0..9 {
        let x = jc.x - jh.x + 1.0 + k as f32 * 2.0;
        m.bevel_box(Vec3::new(0.05, 0.05, jh.z + 0.05), 0.02, 1, dark, at(Vec3::new(x, jc.y - jh.y, jc.z)));
    }
    m.bevel_box(Vec3::new(1.2, 0.9, 1.0), 0.08, 2, Paint::matte(0x8a857a), at(Vec3::new(mc.x - 3.4, jc.y - 0.4, jc.z)));
    m.bevel_box(Vec3::new(3.0, 0.3, 0.4), 0.06, 1, yellow, at(Vec3::new(mc.x - 2.0, jc.y, jc.z)));
    // Pulley block at the tip.
    m.bevel_box(Vec3::new(0.3, 0.35, 0.3), 0.05, 1, dark, at(Vec3::new(jc.x + jh.x - 0.3, jc.y - jh.y - 0.35, jc.z)));
    m
}

/// A launch plate set flush in the ground, half extents `half` (standing
/// on y = 0): a riveted steel deck, a hazard border and forward chevrons.
pub fn launch_plate(half: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let steel = Paint::steel(0x6d6862, 0.5);
    let hz = Paint::with(Material::Hazard, 0xffffff, 0.6);
    m.bevel_box(Vec3::new(half.x, 0.12, half.z), 0.05, 1, steel, at(Vec3::Y * 0.02));
    for side in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(half.x, 0.06, 0.2), 0.03, 1, hz, at(Vec3::new(0.0, 0.12, side * (half.z - 0.2))));
        m.bevel_box(Vec3::new(0.2, 0.06, half.z), 0.03, 1, hz, at(Vec3::new(side * (half.x - 0.2), 0.12, 0.0)));
    }
    for k in 0..3 {
        let x = -1.2 + k as f32 * 1.0;
        for side in [-1.0f32, 1.0] {
            let rot = Quat::from_rotation_y(side * 0.6);
            m.bevel_box(Vec3::new(0.12, 0.04, 0.8), 0.02, 1, Paint::glow(0xff9a20, 1.2), Mat4::from_rotation_translation(rot, Vec3::new(x, 0.15, side * 0.55)));
        }
    }
    let mut x = -half.x + 0.4;
    while x < half.x - 0.3 {
        for side in [-1.0f32, 1.0] {
            m.rivet(Vec3::new(x, 0.14, side * (half.z - 0.45)), Vec3::Y, 0.06, Paint::steel(0x9a958c, 0.4));
        }
        x += 0.6;
    }
    m
}

/// The crane's wrecking ball, centred, with a lifting eye on top.
#[allow(dead_code)] // called by the game layer (shots, end screen)
pub fn wrecking_ball(r: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.rock_blob(4, Vec3::splat(r), Paint::steel(0x3e3a36, 0.55), Mat4::IDENTITY);
    m.torus(0.3, 0.09, 0.18, 10, Paint::steel(0x6a6460, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), Vec3::Y * (r + 0.2)));
    m
}

/// A proximity mine, standing on y = 0.
pub fn mine() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.cylinder(0.32, 0.07, 0.04, 2, 10, Paint::painted(0x4a5038, 0.6), at(Vec3::Y * 0.07));
    m.cylinder(0.12, 0.05, 0.02, 1, 8, Paint::steel(0x3a3632, 0.4), at(Vec3::Y * 0.17));
    m
}

/// A rocket along +X, centred.
pub fn rocket() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.cylinder(0.09, 0.45, 0.03, 1, 8, Paint::painted(0xd8d2c4, 0.3), along_x(Vec3::ZERO));
    let tip = [(0.09f32, 0.0f32, Vec2::new(0.8, 0.3)), (0.0, 0.22, Vec2::Y)];
    m.lathe(&tip, 8, true, Paint::painted(0xb02a20, 0.3), along_x(Vec3::X * 0.45));
    for k in 0..4 {
        let rot = Quat::from_rotation_x(k as f32 * std::f32::consts::FRAC_PI_2);
        m.bevel_box(Vec3::new(0.1, 0.12, 0.01), 0.005, 1, Paint::steel(0x3a3632, 0.4), Mat4::from_rotation_translation(rot, Vec3::new(-0.38, 0.0, 0.0)) * at(Vec3::Y * 0.1));
    }
    m
}

/// A harpoon along +X, centred: a shaft and a barbed head.
pub fn harpoon() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.cylinder(0.05, 0.6, 0.0, 1, 6, Paint::steel(0x6a6460, 0.4), along_x(Vec3::ZERO));
    let tip = [(0.12f32, 0.0f32, Vec2::new(0.6, 0.6)), (0.0, 0.35, Vec2::Y)];
    m.lathe(&tip, 6, true, Paint::steel(0xb8b4aa, 0.25), along_x(Vec3::X * 0.6));
    for side in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.14, 0.02, 0.02), 0.01, 1, Paint::steel(0xb8b4aa, 0.25), Mat4::from_rotation_translation(Quat::from_rotation_z(side * 0.6), Vec3::new(0.5, side * 0.08, 0.0)));
    }
    m
}

/// A mortar shell, centred, nose along +X.
pub fn mortar_shell() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let body = [(0.0f32, -0.3f32, Vec2::NEG_Y), (0.14, -0.25, Vec2::new(0.6, -0.8)), (0.16, 0.0, Vec2::X), (0.1, 0.25, Vec2::new(0.8, 0.6)), (0.0, 0.35, Vec2::Y)];
    m.lathe(&body, 10, false, Paint::painted(0x4a5038, 0.4), Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2));
    m
}

/// A bomber silhouette for air strikes, nose along +X, centred.
pub fn plane() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let body = Paint::painted(0x5a5e52, 0.6);
    m.cylinder(0.9, 5.5, 0.6, 2, 10, body, along_x(Vec3::ZERO));
    m.wedge(Vec3::new(1.6, 0.15, 9.0), 0.08, 1, 0.5, body, at(Vec3::new(0.5, 0.2, 0.0)));
    m.wedge(Vec3::new(0.8, 0.1, 3.0), 0.05, 1, 0.5, body, at(Vec3::new(-5.0, 0.4, 0.0)));
    m.wedge(Vec3::new(0.9, 1.2, 0.1), 0.05, 1, 0.4, body, at(Vec3::new(-5.0, 1.3, 0.0)));
    for z in [-3.5f32, 3.5] {
        m.cylinder(0.45, 0.9, 0.15, 1, 8, Paint::steel(0x3a3632, 0.5), along_x(Vec3::new(1.4, -0.2, z)));
    }
    m
}

/// A podium block for the end-screen line-up: `height` tall, standing on
/// y = 0, riveted steel with a hazard-striped lip.
#[allow(dead_code)] // called by the game layer (shots, end screen)
pub fn podium_block(height: f32, colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let half = Vec3::new(3.6, height * 0.5, 2.6);
    m.bevel_box(half, 0.1, 2, Paint::painted(colour, 0.35), at(Vec3::Y * half.y));
    m.bevel_box(Vec3::new(half.x + 0.05, 0.12, half.z + 0.05), 0.04, 1, Paint::with(Material::Hazard, 0xffffff, 0.6), at(Vec3::Y * (height - 0.12)));
    let mut x = -half.x + 0.4;
    while x < half.x {
        m.rivet(Vec3::new(x, height - 0.45, half.z), Vec3::Z, 0.07, Paint::steel(0x9a958c, 0.35));
        x += 0.6;
    }
    m
}

// ── Clutter: visual-only dressing ───────────────────────────────────────

/// A wooden pallet, standing on y = 0.
pub fn pallet(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let wood = Paint::with(Material::Wood, [0x9a7448, 0x7a5a3a, 0x8a6a44][(hash(seed, 1) * 3.0) as usize % 3], 0.8);
    for z in [-0.45f32, 0.0, 0.45] {
        m.bevel_box(Vec3::new(0.6, 0.05, 0.05), 0.01, 1, wood, at(Vec3::new(0.0, 0.05, z)));
    }
    for k in 0..6 {
        let x = -0.5 + k as f32 * 0.2;
        m.bevel_box(Vec3::new(0.07, 0.015, 0.55), 0.008, 1, wood, at(Vec3::new(x, 0.115, 0.0)));
    }
    m
}

/// A few cinder blocks, one on another.
pub fn cinder_blocks(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let grey = Paint::matte(0x9a948a);
    let n = 2 + (hash(seed, 2) * 3.0) as u32;
    for k in 0..n {
        let x = (k % 2) as f32 * 0.42 - 0.2 + (hash(k, seed) - 0.5) * 0.1;
        let y = 0.1 + (k / 2) as f32 * 0.2;
        let rot = Quat::from_rotation_y((hash(k, seed + 1) - 0.5) * 0.5);
        m.bevel_box(Vec3::new(0.2, 0.1, 0.1), 0.015, 1, grey, Mat4::from_rotation_translation(rot, Vec3::new(x, y, 0.0)));
        for dx in [-0.09f32, 0.09] {
            m.bevel_box(Vec3::new(0.06, 0.101, 0.06), 0.005, 1, Paint::steel(0x2a2725, 0.9), Mat4::from_rotation_translation(rot, Vec3::new(x, y, 0.0)) * at(Vec3::new(dx, 0.0, 0.0)));
        }
    }
    m
}

/// A jerrycan (or two), standing on y = 0.
pub fn jerrycans(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    for k in 0..(1 + (hash(seed, 3) * 2.0) as u32) {
        let c = [0xb02a20, 0x5e6a3a, 0xe2b21e][((hash(seed, k + 5)) * 3.0) as usize % 3];
        let p = Vec3::new(k as f32 * 0.32, 0.3, (hash(k, seed) - 0.5) * 0.2);
        let rot = Quat::from_rotation_y(hash(k, seed + 2) * 0.6);
        m.bevel_box(Vec3::new(0.12, 0.3, 0.22), 0.04, 2, Paint::painted(c, 0.6), Mat4::from_rotation_translation(rot, p));
        m.bevel_box(Vec3::new(0.03, 0.04, 0.1), 0.015, 1, Paint::painted(c, 0.7), Mat4::from_rotation_translation(rot, p + Vec3::Y * 0.33));
    }
    m
}

/// A blue tarp draped over a lump, on y = 0.
pub fn tarp(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let blue = Paint::matte(0x2f62a0);
    let h = |x: f32, z: f32| {
        let r = (x * x / 1.4 + z * z / 0.9).sqrt();
        (0.75 * (1.0 - r).max(0.0)).powf(0.8) + 0.06 * (x * 6.0 + hash(seed, 1) * 6.0).sin() * (1.0 - r).max(0.0) + 0.02
    };
    m.heightfield(1.4, 10, h, blue);
    m
}

/// A dry grass tuft: a fan of thin blades.
pub fn grass_tuft(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let tones = [0xb8a060, 0xa08a48, 0x8a8a44];
    for k in 0..7 {
        let a = hash(k, seed) * std::f32::consts::TAU;
        let lean = 0.25 + 0.4 * hash(k, seed + 1);
        let h = 0.35 + 0.35 * hash(k, seed + 2);
        let base = Vec3::new(a.cos() * 0.06, 0.0, a.sin() * 0.06);
        let tip = base + Vec3::new(a.cos() * lean * h, h, a.sin() * lean * h);
        let side = Vec3::new(-a.sin(), 0.0, a.cos()) * 0.03;
        let paint = Paint::matte(tones[k as usize % 3]);
        m.flat_tri(base - side, base + side, tip, base - Vec3::new(a.cos(), 0.0, a.sin()), paint);
        m.flat_tri(base + side, base - side, tip, base + Vec3::new(a.cos(), 0.0, a.sin()), paint);
    }
    m
}

/// A low faceted desert shrub.
pub fn shrub(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let tones = [0x7a8040, 0x6a7a3a, 0x8a8a4a];
    for k in 0..3 {
        let off = Vec3::new((hash(k, seed) - 0.5) * 0.6, 0.0, (hash(k, seed + 1) - 0.5) * 0.6);
        let s = 0.3 + 0.25 * hash(k, seed + 2);
        m.rock_blob(seed + k, Vec3::new(s, s * 0.75, s), Paint::matte(tones[k as usize]), at(off + Vec3::Y * s * 0.3));
    }
    m
}

/// A scatter of small pebbles in a patch about a metre across.
pub fn pebbles(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    for k in 0..9 {
        let p = Vec3::new((hash(k, seed) - 0.5) * 1.4, 0.0, (hash(k, seed + 1) - 0.5) * 1.4);
        let s = 0.05 + 0.08 * hash(k, seed + 2);
        m.rock_blob(seed * 31 + k, Vec3::new(s, s * 0.6, s * 0.8), ROCK, at(p));
    }
    m
}

/// A loose corrugated sheet lying on the ground.
pub fn sheet_on_ground(seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let c = [0x7e8079, 0x8a5a3a, 0x6e7a76][(hash(seed, 4) * 3.0) as usize % 3];
    let rot = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 + 0.06) * Quat::from_rotation_y((hash(seed, 5) - 0.5) * 0.2);
    m.corrugated(1.6, 2.2, 0.15, 0.04, 0.008, Paint::painted(c, 0.95), Mat4::from_rotation_translation(rot, Vec3::Y * 0.06));
    m
}

/// The Raider Waves outpost zone: a hazard-striped ring on the ground with
/// banner poles round it in the defenders' colour, centred, on y = 0.
pub fn raid_zone(radius: f32, colour: u32, ground: &dyn Fn(f32, f32) -> f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let ring = [
        (radius - 0.35, 0.0, Vec2::NEG_Y),
        (radius - 0.35, 0.08, Vec2::new(-0.6, 0.8)),
        (radius - 0.25, 0.12, Vec2::Y),
        (radius + 0.25, 0.12, Vec2::Y),
        (radius + 0.35, 0.08, Vec2::new(0.6, 0.8)),
        (radius + 0.35, 0.0, Vec2::NEG_Y),
    ];
    // The ring follows the ground in short striped segments.
    let _ = ring;
    let segs = 96;
    for k in 0..segs {
        let a = (k as f32 + 0.5) / segs as f32 * std::f32::consts::TAU;
        let (x, z) = (a.cos() * radius, a.sin() * radius);
        let len = radius * std::f32::consts::TAU / segs as f32 * 0.55;
        let rot = Quat::from_rotation_y(-a + std::f32::consts::FRAC_PI_2);
        m.bevel_box(Vec3::new(len, 0.08, 0.35), 0.03, 1, Paint::with(Material::Hazard, 0xffffff, 0.6), Mat4::from_rotation_translation(rot, Vec3::new(x, ground(x, z) + 0.05, z)));
    }
    let poles = ((radius * std::f32::consts::TAU) / 18.0).round().max(6.0) as u32;
    for k in 0..poles {
        let a = k as f32 / poles as f32 * std::f32::consts::TAU;
        let p = Vec3::new(a.cos() * radius, ground(a.cos() * radius, a.sin() * radius), a.sin() * radius);
        m.cylinder(0.06, 2.0, 0.0, 1, 6, Paint::steel(0x4a4440, 0.5), at(p + Vec3::Y * 2.0));
        // A banner hanging off the pole, facing outwards.
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let side = Vec3::new(-a.sin(), 0.0, a.cos());
        let rot = Quat::from_rotation_arc(Vec3::Z, out);
        m.bevel_box(Vec3::new(0.45, 0.7, 0.02), 0.02, 1, Paint::painted(colour, 0.4), Mat4::from_rotation_translation(rot, p + Vec3::Y * 3.2 + side * 0.47));
        m.bevel_box(Vec3::new(0.46, 0.06, 0.025), 0.01, 1, Paint::painted(0xd8ccb0, 0.5), Mat4::from_rotation_translation(rot, p + Vec3::Y * 3.55 + side * 0.47));
    }
    m
}

// ── Outpost pieces (map rework) ─────────────────────────────────────────
// All centred on their collider box, like the other obstacle builders.

/// A concrete wall segment, long along X: cast panels with seams, a
/// capping beam, scuffed hazard paint at the foot, razor wire on top.
pub fn wall_segment(half: Vec3, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let concrete = Paint::matte(0xa8a092);
    let panels = ((2.0 * half.x) / 2.4).round().max(1.0) as u32;
    let w = 2.0 * half.x / panels as f32;
    for k in 0..panels {
        let x = -half.x + w * (k as f32 + 0.5);
        let tone = [0xa8a092, 0x9e9688, 0xb0a898][(hash(k, seed) * 3.0) as usize % 3];
        m.bevel_box(Vec3::new(w * 0.5 - 0.03, half.y - 0.1, half.z), 0.05, 1, Paint::matte(tone), at(Vec3::new(x, -0.1, 0.0)));
    }
    m.bevel_box(Vec3::new(half.x, 0.12, half.z + 0.05), 0.04, 1, concrete, at(Vec3::Y * (half.y - 0.12)));
    for side in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(half.x, 0.15, 0.02), 0.01, 1, Paint::with(Material::Hazard, 0xffffff, 0.6), at(Vec3::new(0.0, -half.y + 0.35, side * (half.z + 0.01))));
    }
    let mut wire = props_razor_on_top(half);
    let base = m.vertices.len() as u32;
    m.vertices.append(&mut wire.vertices);
    m.indices.extend(wire.indices.iter().map(|i| i + base));
    m
}

fn props_razor_on_top(half: Vec3) -> MeshBuilder {
    let mut m = razor_wire(2.0 * half.x - 0.4);
    m.transform_from(0, Mat4::from_scale_rotation_translation(Vec3::new(1.0, 0.6, 0.6), Quat::IDENTITY, Vec3::Y * half.y));
    m
}

/// A quonset tent over its box (long along X): a canvas half-cylinder on
/// ribs, end walls with a dark door.
pub fn quonset(half: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let canvas = Paint::matte(0x8a8458);
    let r = half.z.min(2.0 * half.y);
    let base_y = -half.y;
    // The shell: a lathe swept into a half-cylinder by building it as a
    // sequence of flat strips (faceted, like a canvas over ribs).
    let steps = 10;
    for k in 0..steps {
        let a0 = k as f32 / steps as f32 * std::f32::consts::PI;
        let a1 = (k + 1) as f32 / steps as f32 * std::f32::consts::PI;
        let p = |a: f32, x: f32| Vec3::new(x, base_y + a.sin() * r * (2.0 * half.y / r).min(1.0), a.cos() * half.z);
        let inside = Vec3::new(0.0, base_y, 0.0);
        m.flat_tri(p(a0, -half.x), p(a1, -half.x), p(a1, half.x), inside, canvas);
        m.flat_tri(p(a0, -half.x), p(a1, half.x), p(a0, half.x), inside, canvas);
        // End walls.
        for x in [-half.x, half.x] {
            m.flat_tri(Vec3::new(x, base_y, 0.0), p(a0, x), p(a1, x), Vec3::new(-x, base_y + 0.5, 0.0), Paint::matte(0x7a7448));
        }
    }
    let rib = Paint::steel(0x4a4440, 0.6);
    let mut x = -half.x + 0.2;
    while x < half.x {
        let pts: Vec<Vec3> = (0..=8)
            .map(|k| {
                let a = k as f32 / 8.0 * std::f32::consts::PI;
                Vec3::new(x, base_y + a.sin() * r * (2.0 * half.y / r).min(1.0) + 0.03, a.cos() * (half.z + 0.03))
            })
            .collect();
        m.tube(&pts, 0.04, 4, rib);
        x += 1.6;
    }
    m.bevel_box(Vec3::new(0.03, 0.9, 0.6), 0.02, 1, Paint::steel(0x1a1816, 0.9), at(Vec3::new(half.x + 0.02, base_y + 0.9, 0.0)));
    m
}

/// A wooden watchtower over its box: four splayed legs with braces, a
/// cabin with a plank parapet and a tin roof.
pub fn watchtower(half: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let wood = Paint::with(Material::Wood, 0x7a5a3a, 0.8);
    let dark = Paint::with(Material::Wood, 0x5a4028, 0.8);
    let base = -half.y;
    let deck = half.y - 2.4;
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let foot = Vec3::new(sx * half.x * 0.95, base, sz * half.z * 0.95);
        let top = Vec3::new(sx * half.x * 0.6, deck, sz * half.z * 0.6);
        crate::tank::octo_pipe(&mut m, foot, top + Vec3::Y * 2.2, 0.12, wood);
    }
    for y in [base + 1.8, base + 3.8] {
        let t = (y - base) / (deck - base);
        let s = 0.95 - 0.35 * t;
        for side in [-1.0f32, 1.0] {
            m.bevel_box(Vec3::new(half.x * s, 0.06, 0.06), 0.02, 1, dark, at(Vec3::new(0.0, y, side * half.z * s)));
            m.bevel_box(Vec3::new(0.06, 0.06, half.z * s), 0.02, 1, dark, at(Vec3::new(side * half.x * s, y, 0.0)));
        }
    }
    // Deck, parapet, roof posts and a tin roof.
    m.bevel_box(Vec3::new(half.x * 0.75, 0.08, half.z * 0.75), 0.03, 1, wood, at(Vec3::Y * deck));
    for side in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(half.x * 0.75, 0.45, 0.04), 0.02, 1, wood, at(Vec3::new(0.0, deck + 0.5, side * half.z * 0.73)));
        m.bevel_box(Vec3::new(0.04, 0.45, half.z * 0.75), 0.02, 1, wood, at(Vec3::new(side * half.x * 0.73, deck + 0.5, 0.0)));
    }
    let roof = Mat4::from_rotation_translation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 + 0.12), Vec3::Y * (deck + 2.1));
    m.corrugated(2.0 * half.x * 0.9, 2.0 * half.z * 0.9, 0.2, 0.05, 0.01, Paint::painted(0x8a7a6a, 0.9), roof);
    m
}

/// The gas-station shop: a block with a dark shop window, a door and a
/// faded sign band.
pub fn gas_shop(half: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(half, 0.06, 1, Paint::matte(0xd8ccb0), Mat4::IDENTITY);
    m.bevel_box(Vec3::new(half.x + 0.02, 0.3, half.z + 0.02), 0.03, 1, Paint::painted(0xb02a20, 0.6), at(Vec3::Y * (half.y - 0.35)));
    m.bevel_box(Vec3::new(half.x * 0.6, half.y * 0.35, 0.03), 0.02, 1, Paint::steel(0x1a2228, 0.3), at(Vec3::new(-half.x * 0.2, -half.y * 0.15, half.z)));
    m.bevel_box(Vec3::new(0.5, half.y * 0.55, 0.03), 0.02, 1, Paint::steel(0x2a2422, 0.7), at(Vec3::new(half.x * 0.65, -half.y * 0.4, half.z)));
    m
}

/// The canopy roof: a flat slab with a striped fascia and dangling lights.
pub fn canopy(half: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(Vec3::new(half.x, half.y * 0.6, half.z), 0.04, 1, Paint::matte(0xd8d0c0), at(Vec3::Y * half.y * 0.2));
    m.bevel_box(Vec3::new(half.x + 0.02, half.y * 0.5, half.z + 0.02), 0.03, 1, Paint::painted(0xb02a20, 0.6), at(Vec3::Y * -half.y * 0.25));
    for k in 0..4 {
        let x = -half.x * 0.6 + k as f32 * half.x * 0.4;
        m.bevel_box(Vec3::new(0.3, 0.05, 0.6), 0.02, 1, Paint::glow(0xfff0c0, 0.4), at(Vec3::new(x, -half.y, 0.0)));
    }
    m
}

/// A canopy pillar with a fuel pump at its foot.
pub fn pillar(half: Vec3, pump: bool) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(half, 0.04, 1, Paint::matte(0xd0c8b8), Mat4::IDENTITY);
    m.bevel_box(Vec3::new(half.x + 0.02, 0.2, half.z + 0.02), 0.02, 1, Paint::painted(0xb02a20, 0.6), at(Vec3::Y * (-half.y + 0.6)));
    if pump {
        m.bevel_box(Vec3::new(0.3, 0.7, 0.25), 0.06, 2, Paint::painted(0xd6a21e, 0.7), at(Vec3::new(0.7, -half.y + 0.7, 0.0)));
        m.bevel_box(Vec3::new(0.02, 0.18, 0.15), 0.01, 1, Paint::steel(0x1a2228, 0.3), at(Vec3::new(1.01, -half.y + 1.05, 0.0)));
    }
    m
}

// ── Landmarks: a tall silhouette per place ───────────────────────────────
// Whole-obstacle builders, in the obstacle frame (standing on y = 0).

/// A four-legged lattice tower tapering from `base` (half width) at the
/// foot to `top` at `height`: corner posts, girts and X-braces every
/// `bay` metres from `first` up.
#[allow(clippy::too_many_arguments)]
fn lattice_tower(m: &mut MeshBuilder, base: f32, top: f32, height: f32, first: f32, bay: f32, post: f32, paint: Paint) {
    let corner = |sx: f32, sz: f32, y: f32| {
        let t = y / height;
        let s = base * (1.0 - t) + top * t;
        Vec3::new(sx * s, y, sz * s)
    };
    let corners = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for (sx, sz) in corners {
        octo_pipe(m, corner(sx, sz, 0.0), corner(sx, sz, height), post, paint);
    }
    let mut y = first;
    while y < height - 0.5 {
        let y1 = (y + bay).min(height);
        for i in 0..4 {
            let (ax, az) = corners[i];
            let (bx, bz) = corners[(i + 1) % 4];
            octo_pipe(m, corner(ax, az, y), corner(bx, bz, y), post * 0.5, paint);
            octo_pipe(m, corner(ax, az, y), corner(bx, bz, y1), post * 0.35, paint);
            octo_pipe(m, corner(bx, bz, y), corner(ax, az, y1), post * 0.35, paint);
        }
        y = y1;
    }
    for i in 0..4 {
        let (ax, az) = corners[i];
        let (bx, bz) = corners[(i + 1) % 4];
        octo_pipe(m, corner(ax, az, height), corner(bx, bz, height), post * 0.6, paint);
    }
}

/// A water tower from its leg and tank boxes (obstacle frame): four steel
/// legs braced above tank height, a riveted tank in faded blue with a
/// painted band, a cone roof, a catwalk with a rail and a ladder.
pub fn water_tower(legs: &[(Vec3, Vec3)], tank: (Vec3, Vec3)) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let steel = Paint::painted(0x5c564e, 0.8);
    let (tc, th) = tank;
    let r = th.x.min(th.z);
    let foot = tc.y - th.y;
    for (c, h) in legs {
        let a = Vec3::new(c.x, 0.0, c.z);
        let b = Vec3::new(c.x * 0.8, foot, c.z * 0.8);
        octo_pipe(&mut m, a, b, h.x * 0.6, steel);
        m.bevel_box(Vec3::new(0.45, 0.2, 0.45), 0.04, 1, Paint::matte(0x9a948a), at(a + Vec3::Y * 0.1));
    }
    // Braces from 3.6 m up (tanks pass between the legs below).
    let pts: Vec<Vec3> = legs.iter().map(|(c, _)| Vec3::new(c.x, 0.0, c.z)).collect();
    let ring = [0usize, 2, 3, 1];
    let mid = foot * 0.62 + 1.2;
    for (y0, y1) in [(3.6f32, mid), (mid, foot)] {
        for i in 0..4 {
            let a = pts[ring[i]];
            let b = pts[ring[(i + 1) % 4]];
            let s0 = 1.0 - 0.2 * y0 / foot;
            let s1 = 1.0 - 0.2 * y1 / foot;
            let (a0, b0) = (Vec3::new(a.x * s0, y0, a.z * s0), Vec3::new(b.x * s0, y0, b.z * s0));
            let (a1, b1) = (Vec3::new(a.x * s1, y1, a.z * s1), Vec3::new(b.x * s1, y1, b.z * s1));
            octo_pipe(&mut m, a0, b0, 0.09, steel);
            octo_pipe(&mut m, a0, b1, 0.06, steel);
            octo_pipe(&mut m, b0, a1, 0.06, steel);
        }
    }
    // The tank: a painted band, a seam, a rivet row.
    let body = Paint::painted(0x6f9db0, 0.65);
    let band = Paint::painted(0xc8402c, 0.6);
    let hy = th.y * 0.8;
    let y0 = foot + 0.6;
    m.cylinder(r, hy, 0.12, 2, 20, body, at(Vec3::Y * (y0 + hy)));
    m.cylinder(r + 0.03, 0.45, 0.04, 1, 20, band, at(Vec3::Y * (y0 + hy * 1.2)));
    m.cylinder(r + 0.02, 0.06, 0.02, 1, 20, Paint::painted(0x4a5a60, 0.8), at(Vec3::Y * (y0 + hy * 0.6)));
    for k in 0..24 {
        let a = k as f32 / 24.0 * std::f32::consts::TAU;
        let n = Vec3::new(a.cos(), 0.0, a.sin());
        m.rivet(Vec3::Y * (y0 + hy * 0.6 + 0.1) + n * (r + 0.02), n, 0.05, Paint::steel(0x8a9aa0, 0.5));
    }
    // Floor plate under the tank and a cone roof with a vent.
    m.cylinder(r * 0.98, 0.25, 0.06, 1, 20, steel, at(Vec3::Y * (y0 - 0.1)));
    let top = y0 + 2.0 * hy;
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    m.lathe(&[(r + 0.15, top, n(0.5, 1.0)), (0.5, top + 1.6, n(0.5, 1.0)), (0.0, top + 1.7, Vec2::Y)], 20, true, Paint::painted(0x7e8a8a, 0.75), Mat4::IDENTITY);
    m.cylinder(0.25, 0.3, 0.05, 1, 8, steel, at(Vec3::Y * (top + 1.9)));
    // Catwalk and rail round the tank's foot.
    m.cylinder(r + 0.9, 0.06, 0.02, 1, 20, Paint::steel(0x4a4642, 0.6), at(Vec3::Y * (y0 - 0.25)));
    for k in 0..16 {
        let a = k as f32 / 16.0 * std::f32::consts::TAU;
        let p = Vec3::new(a.cos(), 0.0, a.sin()) * (r + 0.85);
        m.bevel_box(Vec3::new(0.03, 0.5, 0.03), 0.01, 1, Paint::painted(0xd6a21e, 0.6), at(p + Vec3::Y * (y0 + 0.25)));
    }
    m.torus(r + 0.85, 0.035, 0.06, 24, Paint::painted(0xd6a21e, 0.6), at(Vec3::Y * (y0 + 0.75)));
    // A ladder up the first leg.
    let (c0, _) = legs[0];
    let lad = Vec3::new(c0.x, 0.0, c0.z).normalize_or(Vec3::X);
    for side in [-0.25f32, 0.25] {
        let s = Vec3::new(-lad.z, 0.0, lad.x) * side;
        octo_pipe(&mut m, Vec3::new(c0.x, 2.2, c0.z) + s + lad * 0.4, Vec3::new(c0.x * 0.8, foot, c0.z * 0.8) + s + lad * 0.4, 0.035, steel);
    }
    m
}

/// A radio mast over its box (obstacle frame): a tapering lattice in red
/// and white aviation bands, dishes and panels, a whip, red lights.
pub fn radio_mast(half: Vec3, centre: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let height = centre.y + half.y;
    let base = half.x.min(half.z) * 0.95;
    let bands = 6;
    let seg = height / bands as f32;
    for k in 0..bands {
        let paint = if k % 2 == 0 { Paint::painted(0xc83a26, 0.55) } else { Paint::painted(0xe8e2d4, 0.6) };
        let y0 = k as f32 * seg;
        let t0 = y0 / height;
        let t1 = (y0 + seg) / height;
        let b0 = base * (1.0 - t0) + 0.3 * t0;
        let b1 = base * (1.0 - t1) + 0.3 * t1;
        let mut part = MeshBuilder::new();
        lattice_tower(&mut part, b0, b1, seg, 0.0, seg / 2.0, 0.08, paint);
        part.transform_from(0, at(Vec3::Y * y0));
        let first = m.vertices.len() as u32;
        m.vertices.extend(part.vertices);
        m.indices.extend(part.indices.iter().map(|i| i + first));
    }
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        m.bevel_box(Vec3::new(0.3, 0.15, 0.3), 0.03, 1, Paint::matte(0x9a948a), at(Vec3::new(sx * base, 0.08, sz * base)));
    }
    // Dishes and panels high up, a whip and the aviation lights.
    for (y, a, r) in [(height * 0.72, 0.4f32, 0.7f32), (height * 0.55, 2.6, 0.55)] {
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let p = Vec3::Y * y + d * 0.55;
        let rot = Quat::from_rotation_arc(Vec3::Y, d);
        m.lathe(&[(0.0, -0.1, -Vec2::Y), (r, 0.12, Vec2::new(-0.3, 1.0).normalize()), (r + 0.04, 0.18, Vec2::X)], 14, true, Paint::painted(0xd8d4c8, 0.5), Mat4::from_rotation_translation(rot, p));
    }
    for k in 0..3 {
        let a = k as f32 * 2.1;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        m.bevel_box(Vec3::new(0.08, 0.6, 0.25), 0.02, 1, Paint::painted(0xd8d4c8, 0.6), Mat4::from_rotation_translation(Quat::from_rotation_y(-a), Vec3::Y * (height * 0.86) + d * 0.4));
    }
    octo_pipe(&mut m, Vec3::Y * height, Vec3::Y * (height + 3.0), 0.04, Paint::steel(0x4a4642, 0.5));
    m.cylinder(0.16, 0.14, 0.06, 2, 10, Paint::glow(0xff2a10, 2.5), at(Vec3::Y * (height + 0.2)));
    m.cylinder(0.12, 0.1, 0.05, 2, 10, Paint::glow(0xff2a10, 2.5), at(Vec3::Y * (height * 0.5)));
    octo_pipe(&mut m, Vec3::new(0.2, 0.3, 0.0), Vec3::new(0.2, height * 0.72, 0.0), 0.05, Paint::rubber(0x1e1c1b));
    m
}

/// A wind pump over its box (obstacle frame): a steel lattice tower, a
/// many-bladed wheel facing +X with a tail vane, a pump rod down to a
/// trough.
pub fn windmill(half: Vec3, centre: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let height = centre.y + half.y;
    let steel = Paint::painted(0x6a5a4a, 0.85);
    let base = half.x.min(half.z) * 0.95;
    lattice_tower(&mut m, base, 0.35, height, 2.8, 2.6, 0.08, steel);
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        m.bevel_box(Vec3::new(0.25, 0.12, 0.25), 0.03, 1, Paint::matte(0x9a948a), at(Vec3::new(sx * base, 0.06, sz * base)));
    }
    // Platform, head, wheel, vane.
    m.bevel_box(Vec3::new(0.7, 0.05, 0.7), 0.02, 1, Paint::with(Material::Wood, 0x7a5a3a, 0.8), at(Vec3::Y * (height - 0.3)));
    let hub = Vec3::new(0.7, height + 0.6, 0.0);
    m.bevel_box(Vec3::new(0.5, 0.22, 0.2), 0.05, 2, Paint::painted(0x4a4642, 0.6), at(Vec3::new(0.1, height + 0.6, 0.0)));
    octo_pipe(&mut m, Vec3::Y * (height - 0.3), Vec3::Y * (height + 0.4), 0.12, steel);
    let blades = 16;
    let r = 2.4;
    for k in 0..blades {
        let a = k as f32 / blades as f32 * std::f32::consts::TAU;
        // Each blade radial in the wheel's YZ plane, pitched about its own axis.
        let radial = Quat::from_rotation_x(a);
        let pitch = Quat::from_rotation_y(0.35);
        let c = if k % 4 == 0 { 0xc83a26 } else { 0xd8d0bc };
        m.bevel_box(Vec3::new(0.02, r * 0.32, 0.17), 0.01, 1, Paint::painted(c, 0.7), Mat4::from_rotation_translation(radial * pitch, hub + radial * Vec3::Y * r * 0.62));
    }
    let wheel = Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), hub);
    m.torus(r * 0.95, 0.03, 0.05, 24, steel, wheel);
    m.torus(r * 0.32, 0.03, 0.05, 16, steel, wheel);
    m.cylinder(0.18, 0.15, 0.05, 1, 10, Paint::painted(0x4a4642, 0.6), along_x(hub + Vec3::X * 0.1));
    octo_pipe(&mut m, Vec3::new(-0.2, height + 0.6, 0.0), Vec3::new(-2.6, height + 0.6, 0.0), 0.05, steel);
    m.bevel_box(Vec3::new(0.7, 0.55, 0.02), 0.02, 1, Paint::painted(0xd8d0bc, 0.7), at(Vec3::new(-2.8, height + 0.7, 0.0)));
    // The pump rod and a trough beside the tower.
    octo_pipe(&mut m, Vec3::Y * 0.4, Vec3::Y * (height - 0.3), 0.03, Paint::steel(0x8a857c, 0.5));
    m.bevel_box(Vec3::new(1.1, 0.25, 0.4), 0.04, 1, Paint::painted(0x5e6a3a, 0.85), at(Vec3::new(0.2, 0.25, base + 1.2)));
    m
}

/// A sand drift heaped against a wall: `len` along X, running out `depth`
/// along +Z from `height` at the wall to nothing, its rim sunk a little
/// so it sits on uneven ground. Painted as the ground itself (`salt` as
/// the terrain's).
pub fn sand_drift(len: f32, depth: f32, height: f32, salt: f32, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint { wear: salt, ..Paint::with(Material::Sand, 0xffffff, 0.95) };
    let (nx, nz) = (10usize, 5usize);
    let wob = hash(seed, 3) * 6.0;
    let h = move |x: f32, z: f32| -> f32 {
        let u = (2.0 * x / len).clamp(-1.0, 1.0);
        let along = (1.0 - u * u).max(0.0).powf(0.6) * (1.0 + 0.15 * (u * 3.0 + wob).sin());
        let out = (1.0 - (z / depth).clamp(0.0, 1.0)).powf(1.6);
        height * along * out - 0.15
    };
    let e = 0.05;
    let mut idx = Vec::new();
    for j in 0..=nz {
        for i in 0..=nx {
            let x = -len / 2.0 + len * i as f32 / nx as f32;
            let z = -0.3 + (depth + 0.3) * j as f32 / nz as f32;
            let n = Vec3::new(h(x - e, z) - h(x + e, z), 2.0 * e, h(x, z - e) - h(x, z + e)).normalize();
            idx.push(m.push(Vec3::new(x, h(x, z), z), n, paint));
        }
    }
    for j in 0..nz {
        for i in 0..nx {
            let a = idx[j * (nx + 1) + i];
            let b = idx[j * (nx + 1) + i + 1];
            let c = idx[(j + 1) * (nx + 1) + i + 1];
            let d = idx[(j + 1) * (nx + 1) + i];
            m.tri(a, b, c);
            m.tri(a, c, d);
        }
    }
    m
}
