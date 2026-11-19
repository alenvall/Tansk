//! The Rust Belt's kit: a dead scrapyard city. Same hard-surface language
//! as the rest (bevelled plates, rivets, painted metal worn to rust), a bit
//! chunkier and more cartoonish: crushed car bales in faded car colours,
//! school buses gone the colour of old mustard, tin shacks patched from
//! whatever was lying around. Obstacle-shaped builders take the collider's
//! half extents and are centred on the box (like `props`); whole-obstacle
//! builders work in the obstacle frame (standing on y = 0).

use glam::{Mat4, Quat, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};
use crate::shapes::hash;
use crate::tank::octo_pipe;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}
fn along_x(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p)
}
fn along_z(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), p)
}
fn hazard() -> Paint {
    Paint::with(Material::Hazard, 0xffffff, 0.6)
}
fn concrete(tone: u32) -> Paint {
    Paint::matte(tone)
}
fn rust() -> Paint {
    Paint::painted(0x7a3a1e, 1.0)
}
fn glass() -> Paint {
    Paint::steel(0x1c2a30, 0.25)
}

/// Faded car colours: the bales and the stacked cars.
pub const CAR_COLOURS: [u32; 8] = [0xb8463a, 0x3f6f8f, 0xd2b04a, 0x5f7f4a, 0xcfc8b6, 0x8a5a8a, 0xd07a2a, 0x7a7a72];
/// The bales: the same cars, sun-faded and crushed.
const BALE_COLOURS: [u32; 8] = [0x9a4a3a, 0x4a6a7a, 0xb0904a, 0x5a6a42, 0x9a9282, 0x6a4a5a, 0xa8642a, 0x6a6a62];
/// Faded school-bus paints.
const BUS_COLOURS: [u32; 4] = [0xd9a52a, 0xc8862a, 0xd8c27a, 0xa04a2a];
/// Tin sheet patchwork.
const TIN: [u32; 6] = [0x8a8a82, 0x9a4a2a, 0x4a6a8a, 0xb8a060, 0x6a7a5a, 0xa8382a];

fn car_paint(seed: u32) -> Paint {
    Paint::painted(CAR_COLOURS[(hash(seed, 3) * 8.0) as usize % 8], 0.85)
}

// ── Crushed car bales ────────────────────────────────────────────────────

/// One bale, centred, of half extents `h`: a car squashed into layers of
/// its own faded paint, pressed out of line, dark crumple seams between,
/// a wheel rim and a bumper end poking out, rust in the folds.
fn bale(m: &mut MeshBuilder, h: Vec3, seed: u32, xf: Mat4) {
    let first = m.vertices.len();
    let colour = BALE_COLOURS[(hash(seed, 3) * 8.0) as usize % 8];
    let seam = Paint::painted(0x2e221a, 1.0);
    // The dark core (shows in the seams).
    m.bevel_box(h - Vec3::new(0.1, 0.02, 0.1), 0.05, 1, seam, Mat4::IDENTITY);
    // Three squashed layers, each pressed a little out of line.
    let layers = 3;
    let lh = h.y / layers as f32;
    for k in 0..layers {
        let y = -h.y + lh * (2 * k + 1) as f32;
        let off = Vec3::new((hash(k, seed + 1) - 0.5) * 0.22, y, (hash(k, seed + 2) - 0.5) * 0.18);
        // Some layers another car's colour; all of them sun-faded and rusty.
        let c = if hash(k, seed + 4) < 0.25 { BALE_COLOURS[(hash(k, seed + 5) * 8.0) as usize % 8] } else { colour };
        let shade = 0.82 + 0.18 * hash(k, seed + 6);
        let paint = Paint { color: crate::meshkit::srgb_hex(c) * shade, ..Paint::painted(c, 0.95) };
        let tilt = Quat::from_rotation_z((hash(k, seed + 7) - 0.5) * 0.05) * Quat::from_rotation_x((hash(k, seed + 8) - 0.5) * 0.05);
        m.bevel_box(Vec3::new(h.x - 0.03 * k as f32, lh - 0.06, h.z - 0.02), 0.1, 2, paint, Mat4::from_rotation_translation(tilt, off));
    }
    // A rim on one face, a bumper end on another.
    let side = if hash(seed, 5) < 0.5 { -1.0 } else { 1.0 };
    let rim_at = Vec3::new((hash(seed, 6) - 0.5) * h.x, (hash(seed, 7) - 0.5) * h.y * 0.6, side * (h.z + 0.04));
    m.cylinder(0.3, 0.06, 0.03, 1, 10, Paint::steel(0x6a6460, 0.55), along_z(rim_at));
    m.cylinder(0.11, 0.08, 0.02, 1, 8, Paint::steel(0x2a2826, 0.7), along_z(rim_at + Vec3::Z * side * 0.04));
    if hash(seed, 10) < 0.5 {
        let bump = Vec3::new(side * (h.x + 0.04), (hash(seed, 8) - 0.5) * h.y, (hash(seed, 9) - 0.5) * h.z);
        m.bevel_box(Vec3::new(0.05, 0.07, h.z * 0.5), 0.03, 1, Paint::steel(0x9a968e, 0.45), Mat4::from_rotation_translation(Quat::from_rotation_x(0.2), bump));
    }
    // Rust bleeding down the faces.
    for k in 0..4 {
        let x = (hash(k, seed + 12) - 0.5) * 1.6 * h.x;
        let len = 0.25 + 0.4 * hash(k, seed + 13);
        let s = if k % 2 == 0 { 1.0 } else { -1.0 };
        m.bevel_box(Vec3::new(0.15 + 0.15 * hash(k, seed + 14), len, 0.012), 0.01, 1, rust(), at(Vec3::new(x, h.y - len - 0.05, s * (h.z + 0.03))));
    }
    m.transform_from(first, xf);
}

/// A column of bales filling a box of half extents `half`, centred.
pub fn car_bales(half: Vec3, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let n = ((2.0 * half.y) / arena_core::map::CUBE_H).round().max(1.0) as u32;
    let bh = half.y / n as f32;
    for k in 0..n {
        let s = seed * 31 + k * 7;
        // Each bale a hair smaller than its slot, a little twisted and offset.
        let h = Vec3::new(half.x - 0.08 - 0.12 * hash(s, 1), bh - 0.04, half.z - 0.06 - 0.15 * hash(s, 2));
        let off = Vec3::new((hash(s, 3) - 0.5) * 0.18, -half.y + bh * (2 * k + 1) as f32, (hash(s, 4) - 0.5) * 0.18);
        let rot = Quat::from_rotation_y((hash(s, 5) - 0.5) * 0.1) * Quat::from_rotation_z((hash(s, 6) - 0.5) * 0.04);
        bale(&mut m, h, s, Mat4::from_rotation_translation(rot, off));
    }
    m
}

// ── A flattened car (the tower, the magnet's catch) ─────────────────────

/// A car, flattened to about `hy` half height, long along X, centred.
fn flat_car(m: &mut MeshBuilder, half: Vec3, seed: u32, crushed: f32, xf: Mat4) {
    let first = m.vertices.len();
    let body = car_paint(seed);
    let (hx, hy, hz) = (half.x, half.y, half.z);
    m.bevel_box(Vec3::new(hx, hy * 0.55, hz), 0.14, 2, body, at(Vec3::new(0.0, -hy * 0.4, 0.0)));
    // The cabin, squashed down by `crushed`.
    let ch = hy * 0.5 * (1.0 - crushed * 0.7);
    m.wedge(Vec3::new(hx * 0.5, ch, hz * 0.9), 0.1, 2, 0.55, body, at(Vec3::new(-hx * 0.12, hy * 0.15 + ch, 0.0)));
    for side in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(hx * 0.36, ch * 0.5, 0.02), 0.02, 1, glass(), at(Vec3::new(-hx * 0.15, hy * 0.15 + ch, side * hz * 0.9)));
        for x in [-hx * 0.62, hx * 0.62] {
            if hash(seed, 20 + (x > 0.0) as u32 + 2 * (side > 0.0) as u32) < 0.7 {
                m.cylinder(hy * 0.62, 0.16, 0.06, 1, 12, Paint::rubber(0x222020), along_z(Vec3::new(x, -hy * 0.6, side * (hz - 0.1))));
                m.cylinder(hy * 0.3, 0.18, 0.03, 1, 8, Paint::steel(0x8a8682, 0.5), along_z(Vec3::new(x, -hy * 0.6, side * (hz - 0.08))));
            }
        }
    }
    // Bumpers and a rust patch on the bonnet.
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.08, 0.1, hz * 0.95), 0.04, 1, Paint::steel(0x9a9a92, 0.35), at(Vec3::new(s * (hx + 0.04), -hy * 0.45, 0.0)));
    }
    m.bevel_box(Vec3::new(hx * 0.2, 0.02, hz * 0.4), 0.01, 1, rust(), at(Vec3::new(hx * 0.55, hy * 0.15 + 0.01, (hash(seed, 30) - 0.5) * hz * 0.6)));
    m.transform_from(first, xf);
}

/// The tower of stacked cars (whole obstacle, standing on y = 0): cars
/// criss-crossed two to a layer up a steel spine, a beacon on top.
pub fn car_tower(lower: (Vec3, Vec3), upper: (Vec3, Vec3)) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let top = upper.0.y + upper.1.y;
    let split = lower.0.y + lower.1.y;
    let mut y = 0.0;
    let mut k = 0u32;
    while y < top - 0.6 {
        let (c, h) = if y < split { lower } else { upper };
        let lh = 0.55 + 0.12 * hash(k, 3);
        let yaw = if k % 2 == 0 { 0.0 } else { std::f32::consts::FRAC_PI_2 } + (hash(k, 4) - 0.5) * 0.35 + if y >= split { 0.5 } else { 0.0 };
        let half_w = h.x.min(h.z);
        let car = Vec3::new(half_w * 0.98, lh, half_w * 0.46);
        for s in [-1.0f32, 1.0] {
            let local = Vec3::new((hash(k, 5) - 0.5) * 0.4, y + lh, s * half_w * 0.5);
            let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_x((hash(k + s as u32, 6) - 0.5) * 0.08);
            let p = Vec3::new(c.x, 0.0, c.z) + Quat::from_rotation_y(yaw) * Vec3::new(local.x, 0.0, local.z) + Vec3::Y * local.y;
            flat_car(&mut m, car, k * 13 + (s > 0.0) as u32 * 7 + 1, 0.6 + 0.4 * hash(k, 7), Mat4::from_rotation_translation(rot, p));
        }
        y += 2.0 * lh;
        k += 1;
    }
    // The spine: an I-beam up through the middle, a beacon and a flag.
    let steel = Paint::painted(0x5a3a2a, 0.9);
    m.bevel_box(Vec3::new(0.25, (top + 3.0) * 0.5, 0.25), 0.05, 1, steel, at(Vec3::Y * (top + 3.0) * 0.5));
    m.cylinder(0.35, 0.25, 0.08, 1, 12, Paint::steel(0x3a3632, 0.5), at(Vec3::Y * (top + 3.1)));
    m.cylinder(0.26, 0.3, 0.1, 2, 12, Paint::glow(0xff4a20, 3.0), at(Vec3::Y * (top + 3.55)));
    m.bevel_box(Vec3::new(1.1, 0.6, 0.03), 0.02, 1, Paint::matte(0xc8322a), Mat4::from_rotation_translation(Quat::from_rotation_y(0.4), Vec3::new(0.8, top + 2.2, 0.3)));
    // Guy cables down to the ground.
    for a in [0.4f32, 2.5, 4.6] {
        let foot = Vec3::new(a.cos(), 0.0, a.sin()) * 9.0;
        octo_pipe(&mut m, Vec3::Y * (top + 1.5), foot, 0.03, Paint::steel(0x2a2826, 0.6));
        m.bevel_box(Vec3::new(0.4, 0.25, 0.4), 0.06, 1, concrete(0x9a9284), at(foot + Vec3::Y * 0.15));
    }
    m
}

// ── The crusher ──────────────────────────────────────────────────────────

/// The car crusher (whole obstacle): a heavy yellow press bed with a
/// crushing chamber, the press head up on four guide columns with its
/// rams, a control cab and a car waiting in the jaws.
pub fn crusher(bed: (Vec3, Vec3), head: (Vec3, Vec3)) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let yellow = Paint::painted(0xd9a21c, 0.75);
    let dark = Paint::steel(0x3a3632, 0.55);
    let (bc, bh) = bed;
    let base_top = bc.y + bh.y;
    // The bed: a plinth, the chamber walls, the floor between them.
    m.bevel_box(Vec3::new(bh.x, bh.y * 0.45, bh.z), 0.15, 2, dark, at(Vec3::new(bc.x, bc.y - bh.y * 0.55, bc.z)));
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(bh.x - 0.2, bh.y * 0.55, 0.7), 0.18, 2, yellow, at(Vec3::new(bc.x, bc.y + bh.y * 0.45, bc.z + s * (bh.z - 0.7))));
        m.bevel_box(Vec3::new(bh.x - 0.3, 0.16, 0.02), 0.01, 1, hazard(), at(Vec3::new(bc.x, bc.y + bh.y * 0.2, bc.z + s * (bh.z + 0.01))));
        // Ribs down the outside of the walls.
        for k in 0..6 {
            let x = bc.x - bh.x + 1.2 + k as f32 * (2.0 * bh.x - 2.4) / 5.0;
            m.bevel_box(Vec3::new(0.14, bh.y * 0.5, 0.12), 0.04, 1, yellow, at(Vec3::new(x, bc.y + bh.y * 0.4, bc.z + s * (bh.z + 0.08))));
        }
    }
    m.bevel_box(Vec3::new(bh.x - 0.3, 0.1, bh.z - 1.4), 0.03, 1, Paint::steel(0x5a5652, 0.6), at(Vec3::new(bc.x, bc.y + 0.1, bc.z)));
    // The car in the jaws, half crushed.
    flat_car(&mut m, Vec3::new(2.2, 0.6, 1.05), 77, 0.8, at(Vec3::new(bc.x + 2.5, bc.y + 0.85, bc.z)));
    // End rams: big hydraulic cylinders lying along X.
    for s in [-1.0f32, 1.0] {
        let x = bc.x + s * (bh.x + 0.9);
        m.bevel_box(Vec3::new(0.9, bh.y * 0.8, bh.z * 0.8), 0.12, 2, yellow, at(Vec3::new(x, bc.y - bh.y * 0.1, bc.z)));
        m.cylinder(0.55, 0.6, 0.06, 1, 14, Paint::steel(0xb8b8b0, 0.3), along_x(Vec3::new(x - s * 1.2, bc.y + 0.2, bc.z)));
    }
    // The press head on its columns.
    let (hc, hh) = head;
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let p = Vec3::new(hc.x + sx * (hh.x - 0.35), 0.0, hc.z + sz * (hh.z + 0.35));
        let top = hc.y + hh.y + 1.2;
        m.cylinder(0.3, (top - base_top) * 0.5, 0.05, 1, 12, Paint::steel(0xa8a8a0, 0.35), at(p + Vec3::Y * (base_top + top) * 0.5));
    }
    m.bevel_box(Vec3::new(hh.x, hh.y * 0.55, hh.z), 0.2, 2, yellow, at(Vec3::new(hc.x, hc.y - hh.y * 0.45, hc.z)));
    m.bevel_box(Vec3::new(hh.x + 0.05, 0.18, hh.z + 0.05), 0.02, 1, hazard(), at(Vec3::new(hc.x, hc.y - hh.y + 0.25, hc.z)));
    // The crown: a cross beam and two rams standing up out of it.
    m.bevel_box(Vec3::new(hh.x + 0.6, 0.45, hh.z + 0.8), 0.12, 2, dark, at(Vec3::new(hc.x, hc.y + hh.y + 1.0, hc.z)));
    for s in [-1.0f32, 1.0] {
        let p = Vec3::new(hc.x + s * hh.x * 0.45, hc.y + hh.y * 0.4, hc.z);
        m.cylinder(0.5, hh.y * 0.5, 0.06, 1, 14, yellow, at(p));
        m.cylinder(0.3, hh.y * 0.35, 0.04, 1, 12, Paint::steel(0xc8c8c0, 0.25), at(p + Vec3::Y * (hh.y * 0.5 + 0.2)));
    }
    // The control cab on the side, its window, a hose loop, an exhaust.
    let cab = Vec3::new(bc.x + bh.x - 2.0, base_top + 1.3, bc.z + bh.z + 1.2);
    m.bevel_box(Vec3::new(1.2, 1.3, 1.0), 0.15, 2, yellow, at(cab));
    m.bevel_box(Vec3::new(0.9, 0.5, 0.03), 0.02, 1, glass(), at(cab + Vec3::new(0.0, 0.35, 1.0)));
    m.bevel_box(Vec3::new(1.35, 0.1, 1.15), 0.05, 1, dark, at(cab + Vec3::Y * 1.35));
    m.bevel_box(Vec3::new(0.25, 1.3, 0.25), 0.05, 1, dark, at(cab - Vec3::new(0.0, 2.3, 0.0)));
    octo_pipe(&mut m, Vec3::new(bc.x - bh.x + 1.0, base_top, bc.z - bh.z - 0.2), Vec3::new(hc.x - hh.x, hc.y, hc.z - hh.z - 0.2), 0.09, Paint::rubber(0x1a1a18));
    m.cylinder(0.18, 1.6, 0.03, 1, 10, dark, at(Vec3::new(bc.x - bh.x + 0.8, base_top + 1.6, bc.z + bh.z - 0.6)));
    m
}

// ── The magnet crane ─────────────────────────────────────────────────────

/// The electromagnet crane (whole obstacle, jib out along +X): a rust-red
/// lattice tower with a cab, the jib and its counterweight, a cable down to
/// the magnet disc and the car it is holding.
pub fn magnet_crane(mast: (Vec3, Vec3), jib: (Vec3, Vec3)) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let red = Paint::painted(0xb8461f, 0.8);
    let dark = Paint::steel(0x3a3632, 0.6);
    let (mc, mh) = mast;
    let top = mc.y + mh.y;
    // Legs, rungs and braces.
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let c = Vec3::new(mc.x + sx * (mh.x - 0.18), 0.0, mc.z + sz * (mh.z - 0.18));
        m.bevel_box(Vec3::new(0.18, top * 0.5, 0.18), 0.05, 1, red, at(c + Vec3::Y * top * 0.5));
    }
    let mut y = 1.4;
    let mut k = 0;
    while y < top - 1.2 {
        for side in [-1.0f32, 1.0] {
            let rot = Quat::from_rotation_z(if k % 2 == 0 { 0.8 } else { -0.8 });
            m.bevel_box(Vec3::new(mh.x * 1.36, 0.07, 0.07), 0.02, 1, red, Mat4::from_rotation_translation(rot, Vec3::new(mc.x, y + 1.0, mc.z + side * (mh.z - 0.18))));
            let rot = Quat::from_rotation_x(if k % 2 == 0 { 0.8 } else { -0.8 });
            m.bevel_box(Vec3::new(0.07, 0.07, mh.z * 1.36), 0.02, 1, red, Mat4::from_rotation_translation(rot, Vec3::new(mc.x + side * (mh.x - 0.18), y + 1.0, mc.z)));
        }
        m.bevel_box(Vec3::new(mh.x, 0.09, mh.z), 0.03, 1, red, at(Vec3::new(mc.x, y, mc.z)));
        y += 2.0;
        k += 1;
    }
    // Footings with hazard paint.
    m.bevel_box(Vec3::new(mh.x + 0.6, 0.35, mh.z + 0.6), 0.08, 1, concrete(0xa49c8e), at(Vec3::Y * 0.2));
    m.bevel_box(Vec3::new(mh.x + 0.62, 0.1, mh.z + 0.62), 0.02, 1, hazard(), at(Vec3::Y * 0.45));
    // The slewing cab under the jib.
    let (jc, jh) = jib;
    m.bevel_box(Vec3::new(1.7, 1.2, 1.5), 0.18, 2, red, at(Vec3::new(mc.x + 0.8, jc.y - jh.y - 1.2, mc.z)));
    m.bevel_box(Vec3::new(0.05, 0.6, 1.1), 0.02, 1, glass(), at(Vec3::new(mc.x + 2.5, jc.y - jh.y - 1.0, mc.z)));
    // The jib: a box truss.
    let tip = jc.x + jh.x;
    for (dy, dz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, 0.0)] {
        m.bevel_box(Vec3::new(jh.x, 0.12, 0.12), 0.04, 1, red, at(Vec3::new(jc.x, jc.y + dy * (jh.y - 0.12), jc.z + dz * (jh.z - 0.12))));
    }
    let mut x = jc.x - jh.x + 0.8;
    let mut k = 0;
    while x < tip - 0.5 {
        let rot = Quat::from_rotation_z(if k % 2 == 0 { 0.9 } else { -0.9 });
        for dz in [-1.0f32, 1.0] {
            m.bevel_box(Vec3::new(0.06, jh.y * 1.25, 0.06), 0.02, 1, red, Mat4::from_rotation_translation(rot, Vec3::new(x, jc.y, jc.z + dz * (jh.z - 0.12))));
        }
        x += 1.6;
        k += 1;
    }
    // Counterweight blocks on the tail, a ladder of hazard paint.
    let tail = jc.x - jh.x;
    m.bevel_box(Vec3::new(1.4, 1.1, 1.1), 0.12, 2, concrete(0x8e877a), at(Vec3::new(tail + 1.5, jc.y - 0.6, jc.z)));
    m.bevel_box(Vec3::new(1.42, 0.2, 1.12), 0.02, 1, hazard(), at(Vec3::new(tail + 1.5, jc.y - 1.2, jc.z)));
    // Pendant ties from a peak over the tower to the tip and the tail.
    let peak = Vec3::new(mc.x, jc.y + 5.0, mc.z);
    octo_pipe(&mut m, Vec3::new(mc.x, jc.y + jh.y, mc.z), peak, 0.2, red);
    octo_pipe(&mut m, peak, Vec3::new(tip - 0.5, jc.y + jh.y, jc.z), 0.05, dark);
    octo_pipe(&mut m, peak, Vec3::new(tail + 0.5, jc.y + jh.y, jc.z), 0.05, dark);
    // The trolley, the cable, the magnet and its catch.
    let tx = tip - 1.2;
    m.bevel_box(Vec3::new(0.7, 0.3, 0.8), 0.06, 1, dark, at(Vec3::new(tx, jc.y - jh.y - 0.3, jc.z)));
    let magnet_y = jc.y - 8.5;
    for dz in [-0.3f32, 0.3] {
        octo_pipe(&mut m, Vec3::new(tx, jc.y - jh.y - 0.5, jc.z + dz), Vec3::new(tx, magnet_y + 0.9, jc.z + dz * 2.5), 0.04, dark);
    }
    m.cylinder(1.9, 0.45, 0.12, 2, 24, Paint::steel(0x2e2c2a, 0.5), at(Vec3::new(tx, magnet_y, jc.z)));
    m.cylinder(1.95, 0.12, 0.03, 1, 24, hazard(), at(Vec3::new(tx, magnet_y + 0.25, jc.z)));
    m.cylinder(0.6, 0.3, 0.06, 1, 12, Paint::steel(0x5a5652, 0.5), at(Vec3::new(tx, magnet_y + 0.7, jc.z)));
    let car = Mat4::from_rotation_translation(Quat::from_rotation_y(0.6) * Quat::from_rotation_x(0.12), Vec3::new(tx, magnet_y - 1.25, jc.z));
    flat_car(&mut m, Vec3::new(2.2, 0.75, 1.0), 42, 0.1, car);
    m
}

// ── School buses ─────────────────────────────────────────────────────────

/// A rusted school bus, half extents `h` (long along X, front at +X),
/// centred: the long yellow box on its wheels, a stubby bonnet, a row of
/// windows (some gone), black rub rails, the roof sign, rust everywhere.
pub fn bus(h: Vec3, variant: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let colour = BUS_COLOURS[variant as usize % 4];
    let body = Paint::painted(colour, 0.9);
    let black = Paint::painted(0x1e1c1a, 0.8);
    let (hx, hy, hz) = (h.x, h.y, h.z);
    let floor = -hy + 0.75;
    let bonnet = 1.4;
    // The body (behind the bonnet), on its skirt.
    let body_hx = hx - bonnet * 0.5;
    let body_c = Vec3::new(-bonnet * 0.5, (floor + hy) * 0.5, 0.0);
    m.bevel_box(Vec3::new(body_hx, (hy - floor) * 0.5, hz), 0.35, 3, body, at(body_c));
    m.bevel_box(Vec3::new(hx - 0.2, 0.22, hz - 0.15), 0.06, 1, Paint::steel(0x2a2624, 0.7), at(Vec3::new(0.0, floor - 0.1, 0.0)));
    // The bonnet and grille.
    let bon_c = Vec3::new(hx - bonnet * 0.5, floor + 0.55, 0.0);
    m.bevel_box(Vec3::new(bonnet * 0.5, 0.6, hz * 0.78), 0.25, 3, body, at(bon_c));
    m.bevel_box(Vec3::new(0.05, 0.4, hz * 0.55), 0.03, 1, Paint::steel(0x8a8a84, 0.35), at(Vec3::new(hx + 0.02, floor + 0.5, 0.0)));
    m.bevel_box(Vec3::new(0.12, 0.16, hz * 0.95), 0.05, 1, black, at(Vec3::new(hx + 0.05, floor - 0.05, 0.0)));
    for s in [-1.0f32, 1.0] {
        m.cylinder(0.16, 0.05, 0.02, 1, 10, Paint::glow(0xe8d8a0, 0.4), along_x(Vec3::new(hx + 0.04, floor + 0.75, s * hz * 0.55)));
    }
    // Windscreen.
    m.bevel_box(Vec3::new(0.04, 0.55, hz * 0.85), 0.03, 1, glass(), at(Vec3::new(hx - bonnet + 0.03, hy - 0.9, 0.0)));
    // Side windows: a row, a few smashed out (dark holes) or boarded.
    let n = 7;
    let win_w = (2.0 * body_hx - 2.0) / n as f32;
    for side in [-1.0f32, 1.0] {
        for k in 0..n {
            let x = -hx + 1.0 + win_w * (k as f32 + 0.5);
            let r = hash(k + (side > 0.0) as u32 * 31, variant + 3);
            let paint = if r < 0.15 { Paint::with(Material::Wood, 0x7a5a3a, 0.8) } else if r < 0.4 { Paint::steel(0x0e0c0a, 0.9) } else { glass() };
            m.bevel_box(Vec3::new(win_w * 0.42, 0.42, 0.03), 0.03, 1, paint, at(Vec3::new(x, hy - 0.95, side * (hz + 0.01))));
        }
        // Rub rails and the lettering band.
        for y in [floor + 0.35, floor + 0.95] {
            m.bevel_box(Vec3::new(body_hx - 0.25, 0.05, 0.03), 0.02, 1, black, at(Vec3::new(-bonnet * 0.5, y, side * (hz + 0.02))));
        }
        m.bevel_box(Vec3::new(body_hx * 0.55, 0.12, 0.02), 0.01, 1, black, at(Vec3::new(-bonnet * 0.5, hy - 0.32, side * (hz + 0.015))));
        // Wheels (the odd one gone to the rim).
        for (k, x) in [hx - 1.5, -hx + 2.2].into_iter().enumerate() {
            let flat = hash(k as u32 + (side > 0.0) as u32 * 5, variant + 9) < 0.3;
            let r = if flat { 0.38 } else { 0.55 };
            m.cylinder(r, 0.2, 0.08, 2, 14, Paint::rubber(0x1c1a18), along_z(Vec3::new(x, -hy + r, side * (hz - 0.15))));
            m.cylinder(0.26, 0.22, 0.03, 1, 10, Paint::steel(0x6a5a4a, 0.6), along_z(Vec3::new(x, -hy + r, side * (hz - 0.13))));
        }
        // Rust patches.
        for k in 0..4 {
            let x = (hash(k, variant * 7 + side as u32) - 0.5) * 2.0 * (body_hx - 1.0) - bonnet * 0.5;
            let y = floor + 0.15 + 1.2 * hash(k, variant + 21);
            let w = 0.4 + 0.6 * hash(k, variant + 22);
            m.bevel_box(Vec3::new(w, 0.18 + 0.3 * hash(k, variant + 23), 0.015), 0.01, 1, rust(), at(Vec3::new(x, y, side * (hz + 0.02))));
        }
    }
    // Roof: a stop-sign bar at the front, a rust stain, the warning lamps.
    m.bevel_box(Vec3::new(0.25, 0.22, hz * 0.6), 0.05, 1, black, at(Vec3::new(hx - bonnet - 0.3, hy - 0.05, 0.0)));
    for s in [-1.0f32, 1.0] {
        m.cylinder(0.12, 0.06, 0.02, 1, 8, Paint::glow(0xd8401a, 0.6), along_x(Vec3::new(hx - bonnet - 0.02, hy - 0.05, s * hz * 0.45)));
    }
    m.bevel_box(Vec3::new(body_hx * 0.4, 0.02, hz * 0.5), 0.01, 1, rust(), at(Vec3::new(-hx * 0.3, hy + 0.0, 0.0)));
    // The rear: an emergency door.
    m.bevel_box(Vec3::new(0.03, (hy - floor) * 0.4, hz * 0.45), 0.02, 1, Paint::painted(colour, 1.0), at(Vec3::new(-hx - 0.01, (floor + hy) * 0.5, 0.0)));
    m
}

// ── The salvage market ────────────────────────────────────────────────────

/// A tin shack, half extents `h`, centred: walls of mismatched corrugated
/// sheets on timber posts, a lean-to roof, a plank door, a shuttered
/// window, a stove pipe and a hand-painted sign.
pub fn shack(h: Vec3, variant: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let post = Paint::with(Material::Wood, 0x6a4a30, 0.8);
    let wall_h = 2.0 * h.y - 0.6;
    let base = -h.y;
    // Corner posts.
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        m.bevel_box(Vec3::new(0.12, h.y, 0.12), 0.03, 1, post, at(Vec3::new(sx * (h.x - 0.12), 0.0, sz * (h.z - 0.12))));
    }
    // Walls: panels of tin, each its own colour.
    let sheet = |m: &mut MeshBuilder, width: f32, c: Vec3, yaw: f32, k: u32| {
        let tone = TIN[(hash(k, variant * 11 + 1) * 6.0) as usize % 6];
        let lean = (hash(k, variant + 5) - 0.5) * 0.04;
        let xf = Mat4::from_rotation_translation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(lean), c);
        m.corrugated(width, wall_h - 0.1 * hash(k, variant + 6), 0.28, 0.07, 0.02, Paint::painted(tone, 0.9), xf);
    };
    let panels_x = ((2.0 * h.x) / 1.6).round().max(2.0) as u32;
    let pw = 2.0 * h.x / panels_x as f32;
    for s in [-1.0f32, 1.0] {
        for k in 0..panels_x {
            let x = -h.x + pw * (k as f32 + 0.5);
            sheet(&mut m, pw + 0.08, Vec3::new(x, base + wall_h * 0.5, s * (h.z - 0.03)), 0.0, k + (s > 0.0) as u32 * 17);
        }
    }
    let panels_z = ((2.0 * h.z) / 1.6).round().max(2.0) as u32;
    let pz = 2.0 * h.z / panels_z as f32;
    for s in [-1.0f32, 1.0] {
        for k in 0..panels_z {
            let z = -h.z + pz * (k as f32 + 0.5);
            sheet(&mut m, pz + 0.08, Vec3::new(s * (h.x - 0.03), base + wall_h * 0.5, z), std::f32::consts::FRAC_PI_2, k + 40 + (s > 0.0) as u32 * 9);
        }
    }
    // The roof: one sloped sheet over the top, overhanging the front.
    let roof_tone = TIN[(hash(variant, 77) * 6.0) as usize % 6];
    let slope = 0.12;
    let roof = Mat4::from_rotation_translation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 + slope), Vec3::new(0.0, h.y - 0.3, 0.15));
    m.corrugated(2.0 * h.x + 0.6, 2.0 * h.z + 0.9, 0.3, 0.08, 0.03, Paint::painted(roof_tone, 0.95), roof);
    m.bevel_box(Vec3::new(h.x + 0.3, 0.08, 0.1), 0.02, 1, post, at(Vec3::new(0.0, h.y - 0.45, h.z + 0.55)));
    // Door, window with a shutter, a sign over the door.
    let front = h.z + 0.06;
    m.bevel_box(Vec3::new(0.55, 0.95, 0.05), 0.03, 1, Paint::with(Material::Wood, 0x7a5a3a, 0.8), at(Vec3::new(-h.x * 0.4, base + 0.95, front)));
    m.bevel_box(Vec3::new(0.6, 0.45, 0.04), 0.02, 1, Paint::steel(0x141210, 0.9), at(Vec3::new(h.x * 0.45, base + 1.55, front)));
    m.bevel_box(Vec3::new(0.62, 0.05, 0.4), 0.02, 1, Paint::painted(TIN[(variant as usize + 2) % 6], 0.8), Mat4::from_rotation_translation(Quat::from_rotation_x(0.5), Vec3::new(h.x * 0.45, base + 2.1, front + 0.25)));
    let sign = [0xd8b040, 0xc8322a, 0x2f6ad0, 0xe8e0c8][variant as usize % 4];
    m.bevel_box(Vec3::new(1.1, 0.28, 0.04), 0.02, 1, Paint::painted(sign, 0.6), at(Vec3::new(-h.x * 0.25, base + wall_h - 0.25, front + 0.04)));
    for k in 0..4 {
        m.bevel_box(Vec3::new(0.09, 0.12, 0.02), 0.01, 1, Paint::matte(0x1e1c1a), at(Vec3::new(-h.x * 0.25 - 0.6 + k as f32 * 0.4, base + wall_h - 0.25, front + 0.07)));
    }
    // A stove pipe through the roof, a tyre on the roof holding it down.
    m.cylinder(0.12, 0.9, 0.02, 1, 10, Paint::steel(0x3a3632, 0.6), at(Vec3::new(h.x * 0.6, h.y + 0.4, -h.z * 0.4)));
    m.cylinder(0.18, 0.06, 0.02, 1, 10, Paint::steel(0x3a3632, 0.6), at(Vec3::new(h.x * 0.6, h.y + 1.3, -h.z * 0.4)));
    m.torus(0.4, 0.14, 0.15, 14, Paint::rubber(0x1c1a18), at(Vec3::new(-h.x * 0.5, h.y - 0.15, -h.z * 0.2)));
    m
}

/// A market stall over its counter box (half extents `h`, centred): a
/// plank counter on trestles piled with salvage, four poles and a striped
/// canvas awning (visual only, high enough to drive under).
pub fn stall(h: Vec3, variant: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let wood = Paint::with(Material::Wood, 0x8a6a46, 0.8);
    let base = -h.y;
    // Trestles and the counter top.
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.08, h.y * 0.8, h.z * 0.85), 0.02, 1, wood, at(Vec3::new(s * (h.x - 0.4), base + h.y * 0.8, 0.0)));
    }
    m.bevel_box(Vec3::new(h.x, 0.07, h.z), 0.03, 1, wood, at(Vec3::new(0.0, h.y - 0.45, 0.0)));
    // Wares: batteries, a headlamp, jerrycans, gears, a car door.
    let top = h.y - 0.38;
    let wares = [0xc8322a, 0x2f6ad0, 0xd8b040, 0x5aa03a, 0x8a8a82];
    for k in 0..5u32 {
        let x = -h.x + 0.5 + k as f32 * (2.0 * h.x - 1.0) / 4.0;
        let c = wares[((hash(k, variant) * 5.0) as usize + k as usize) % 5];
        match (hash(k, variant + 1) * 3.0) as u32 {
            0 => m.bevel_box(Vec3::new(0.18, 0.14, 0.12), 0.03, 1, Paint::painted(c, 0.6), at(Vec3::new(x, top + 0.14, (hash(k, variant + 2) - 0.5) * h.z))),
            1 => m.cylinder(0.16, 0.1, 0.03, 1, 10, Paint::steel(0x8a8682, 0.4), at(Vec3::new(x, top + 0.1, 0.0))),
            _ => m.torus(0.22, 0.08, 0.1, 12, Paint::rubber(0x1c1a18), at(Vec3::new(x, top + 0.09, 0.0))),
        }
    }
    // Poles and the awning (3.2 m up the front, 3.6 at the back).
    let pole = Paint::steel(0x5a5652, 0.6);
    let aw = 3.2;
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let p = Vec3::new(sx * (h.x + 0.2), base, sz * (h.z + 0.6));
        let height = aw + if sz < 0.0 { 0.4 } else { 0.0 };
        octo_pipe(&mut m, p, p + Vec3::Y * height, 0.05, pole);
    }
    let stripes = 6;
    let (ca, cb) = [(0xc8322a, 0xe8e0c8), (0x2f6ad0, 0xe8e0c8), (0x5aa03a, 0xd8b040), (0xd07a2a, 0xe8e0c8)][variant as usize % 4];
    let sw = 2.0 * (h.x + 0.35) / stripes as f32;
    let tilt = Quat::from_rotation_x(0.4 / (2.0 * h.z + 1.4));
    for k in 0..stripes {
        let x = -(h.x + 0.35) + sw * (k as f32 + 0.5);
        let c = if k % 2 == 0 { ca } else { cb };
        m.bevel_box(Vec3::new(sw * 0.5, 0.03, h.z + 0.8), 0.01, 1, Paint::matte(c), Mat4::from_rotation_translation(tilt, Vec3::new(x, base + aw + 0.22, 0.0)));
        // A scalloped valance down the front edge.
        m.bevel_box(Vec3::new(sw * 0.5, 0.18, 0.02), 0.01, 1, Paint::matte(c), at(Vec3::new(x, base + aw - 0.05, h.z + 0.8)));
    }
    m
}

/// A run of tin fence, half extents `h` (along X), centred: corrugated
/// panels in patchwork colours on pipe posts, a rail along the top,
/// barbed wire, the odd panel leaning.
pub fn fence(h: Vec3, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let panels = ((2.0 * h.x) / 2.2).round().max(1.0) as u32;
    let w = 2.0 * h.x / panels as f32;
    let post = Paint::steel(0x4a4440, 0.6);
    for k in 0..panels {
        let x = -h.x + w * (k as f32 + 0.5);
        let r = hash(k, seed);
        let tone = TIN[(r * 6.0) as usize % 6];
        let lean = (hash(k, seed + 1) - 0.5) * 0.06;
        let hh = 2.0 * h.y - 0.25 - 0.35 * hash(k, seed + 2);
        let side = if k % 2 == 0 { 0.06 } else { -0.06 };
        let xf = Mat4::from_rotation_translation(Quat::from_rotation_x(lean), Vec3::new(x, -h.y + 0.1 + hh * 0.5, side));
        m.corrugated(w + 0.12, hh, 0.3, 0.08, 0.02, Paint::painted(tone, 0.9), xf);
        // Rust bloom along the bottom of the sheet.
        m.bevel_box(Vec3::new(w * 0.45, 0.18 + 0.2 * hash(k, seed + 3), 0.05), 0.01, 1, rust(), at(Vec3::new(x, -h.y + 0.3, side)));
    }
    for k in 0..=panels {
        let x = -h.x + w * k as f32;
        m.cylinder(0.08, h.y + 0.15, 0.02, 1, 8, post, at(Vec3::new(x, 0.15, 0.0)));
    }
    m.bevel_box(Vec3::new(h.x, 0.05, 0.05), 0.02, 1, post, at(Vec3::new(0.0, h.y - 0.2, 0.0)));
    // Barbed wire strands on short arms.
    for (dy, dz) in [(0.25f32, 0.0f32), (0.5, 0.12)] {
        m.bevel_box(Vec3::new(h.x, 0.012, 0.012), 0.0, 0, Paint::steel(0x3a3632, 0.5), at(Vec3::new(0.0, h.y + dy, dz)));
    }
    m
}

// ── The bridge ───────────────────────────────────────────────────────────

/// The broken concrete bridge (whole obstacle, frame on the river bed, the
/// deck along Z): a slab with an asphalt road and white dashes, parapets
/// with chunks knocked out and rebar showing, a dead lamp post, the pier
/// in the bed with its footing, abutment walls down the river banks.
pub fn bridge(deck: (Vec3, Vec3), pier: (Vec3, Vec3)) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let (dc, dh) = deck;
    let slab = concrete(0xa49c8e);
    let slab_dark = concrete(0x8a8276);
    let road_y = dc.y + dh.y;
    // The slab, in three spans with expansion joints.
    let spans = 3;
    let sl = 2.0 * dh.z / spans as f32;
    for k in 0..spans {
        let z = -dh.z + sl * (k as f32 + 0.5);
        let sag = if k == 1 { -0.04 } else { 0.0 };
        m.bevel_box(Vec3::new(dh.x, dh.y, sl * 0.5 - 0.04), 0.08, 1, slab, at(Vec3::new(dc.x, dc.y + sag, dc.z + z)));
        // Girders showing along the slab's sides.
        for s in [-1.0f32, 1.0] {
            m.bevel_box(Vec3::new(0.06, dh.y * 0.6, sl * 0.5 - 0.1), 0.02, 1, slab_dark, at(Vec3::new(dc.x + s * (dh.x + 0.02), dc.y - dh.y * 0.2, dc.z + z)));
        }
    }
    // The road: dark asphalt, a dashed centre line, kerbs.
    m.bevel_box(Vec3::new(dh.x - 1.0, 0.03, dh.z), 0.01, 1, Paint::matte(0x2c2a28), at(Vec3::new(dc.x, road_y + 0.01, dc.z)));
    let mut z = -dh.z + 2.0;
    while z < dh.z - 2.0 {
        m.bevel_box(Vec3::new(0.12, 0.02, 1.2), 0.0, 0, Paint::matte(0xe8e4d8), at(Vec3::new(dc.x, road_y + 0.04, dc.z + z)));
        z += 4.5;
    }
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.1, 0.02, dh.z), 0.0, 0, Paint::matte(0xd8b040), at(Vec3::new(dc.x + s * (dh.x - 1.3), road_y + 0.04, dc.z)));
    }
    // Parapets, knocked out in places (rebar sticking out of the breaks).
    let rebar = Paint::steel(0x5a3a28, 0.7);
    for s in [-1.0f32, 1.0] {
        let seg = 2.5;
        let n = (2.0 * dh.z / seg).round() as i32;
        for k in 0..n {
            let z = -dh.z + seg * (k as f32 + 0.5);
            let broken = (s > 0.0 && (k == 5 || k == 6)) || (s < 0.0 && k == 12);
            let x = dc.x + s * (dh.x - 0.35);
            if broken {
                for r in 0..4 {
                    let a = Vec3::new(x + (r as f32 - 1.5) * 0.12, road_y + 0.1, dc.z + z - seg * 0.5);
                    octo_pipe(&mut m, a, a + Vec3::new(s * 0.2 * hash(r, k as u32), 0.4 + 0.5 * hash(r, k as u32 + 3), 0.6 + 0.3 * hash(r, 9)), 0.025, rebar);
                }
                m.bevel_box(Vec3::new(0.3, 0.12, 0.6), 0.06, 1, slab, Mat4::from_rotation_translation(Quat::from_rotation_z(s * 0.4), Vec3::new(x, road_y + 0.15, dc.z + z)));
                continue;
            }
            let hgt = 0.45 - if hash(k as u32, 31 + (s > 0.0) as u32) < 0.2 { 0.15 } else { 0.0 };
            m.bevel_box(Vec3::new(0.3, hgt, seg * 0.5 - 0.05), 0.06, 1, slab, at(Vec3::new(x, road_y + hgt, dc.z + z)));
            m.bevel_box(Vec3::new(0.31, 0.08, seg * 0.5 - 0.06), 0.01, 1, hazard(), at(Vec3::new(x, road_y + 0.2, dc.z + z)));
        }
    }
    // A dead lamp post, bent over.
    let lp = Vec3::new(dc.x - (dh.x - 0.4), road_y, dc.z + dh.z * 0.3);
    octo_pipe(&mut m, lp, lp + Vec3::Y * 4.5, 0.08, Paint::steel(0x5a5652, 0.6));
    octo_pipe(&mut m, lp + Vec3::Y * 4.5, lp + Vec3::new(1.6, 4.0, 0.3), 0.07, Paint::steel(0x5a5652, 0.6));
    m.bevel_box(Vec3::new(0.35, 0.12, 0.2), 0.05, 1, Paint::steel(0x3a3632, 0.5), at(lp + Vec3::new(1.8, 3.9, 0.35)));
    // The pier: a wall in the bed, a footing, a cap under the slab.
    let (pc, ph) = pier;
    m.bevel_box(Vec3::new(ph.x + 0.8, 0.4, ph.z + 0.8), 0.1, 1, slab_dark, at(Vec3::new(pc.x, 0.3, pc.z)));
    m.bevel_box(Vec3::new(ph.x, ph.y, ph.z), 0.15, 2, slab, at(pc));
    m.bevel_box(Vec3::new(ph.x + 0.5, 0.35, ph.z + 0.5), 0.08, 1, slab_dark, at(Vec3::new(pc.x, pc.y + ph.y - 0.35, pc.z)));
    // Water marks and a spray-painted tag on the pier.
    m.bevel_box(Vec3::new(ph.x, 0.25, ph.z + 0.01), 0.0, 0, concrete(0x7a7268), at(Vec3::new(pc.x, 1.1, pc.z)));
    for (k, c) in [0xc8322a, 0x2f6ad0, 0xe8e0c8].into_iter().enumerate() {
        let x = pc.x - 1.5 + k as f32 * 1.1;
        m.bevel_box(Vec3::new(0.5, 0.35 + 0.15 * k as f32, 0.02), 0.01, 1, Paint::matte(c), Mat4::from_rotation_translation(Quat::from_rotation_z(0.2 * k as f32 - 0.2), Vec3::new(x, 2.4, pc.z + ph.z + 0.02)));
    }
    // Abutments: concrete walls facing the bed under each end of the deck.
    let bed_half = arena_core::rustbelt::BED_HALF;
    for s in [-1.0f32, 1.0] {
        let z = dc.z + s * (bed_half + 0.6);
        let hgt = dc.y - dh.y;
        m.bevel_box(Vec3::new(dh.x + 1.5, hgt * 0.5, 0.6), 0.1, 1, slab, at(Vec3::new(dc.x, hgt * 0.5, z)));
        for k in 0..4 {
            let x = dc.x - dh.x + 1.0 + k as f32 * (2.0 * dh.x - 2.0) / 3.0;
            m.bevel_box(Vec3::new(0.08, hgt * 0.5 - 0.2, 0.05), 0.02, 1, slab_dark, at(Vec3::new(x, hgt * 0.5, z - s * 0.62)));
        }
    }
    m
}
