//! What broken cover looks like: the dents and holes of a beaten-up shack
//! or fence (stage 1), the heap or the flattened mess it leaves (stage 2).
//! Built in the obstacle's frame (standing on y = 0) from its boxes, in the
//! same chunky painted-metal language as everything else.

use glam::{Mat4, Quat, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};
use crate::shapes::hash;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}

const SOOT: u32 = 0x1c1714;
const TIN: [u32; 6] = [0x8a8a82, 0x9a4a2a, 0x4a6a8a, 0xb8a060, 0x6a7a5a, 0xa8382a];

/// Stage 1: holes punched through the walls (dark, ragged, a scorched
/// ring round each), soot streaks up the sides, bent-out flaps and a few
/// shards fallen at the foot. `boxes` are the obstacle's ground boxes
/// (centre, half extents, yaw) in its frame.
pub fn dent_overlay(boxes: &[(Vec3, Vec3, f32)], seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let soot = Paint::matte(SOOT);
    let char = Paint::painted(0x3a2a20, 1.0);
    for (bi, (c, h, yaw)) in boxes.iter().enumerate() {
        let rot = Quat::from_rotation_y(*yaw);
        let s0 = seed * 13 + bi as u32 * 7;
        let holes = 2 + (hash(s0, 1) * 3.0) as u32;
        for k in 0..holes {
            let s = s0 + k * 5;
            // A face: one of the long sides (or an end, now and then).
            let long = h.x >= h.z;
            let end = hash(s, 2) < 0.25;
            let sign = if hash(s, 3) < 0.5 { -1.0 } else { 1.0 };
            let (normal, along, ext) = if long != end { (Vec3::Z * sign, Vec3::X, h.x) } else { (Vec3::X * sign, Vec3::Z, h.z) };
            let depth = if long != end { h.z } else { h.x };
            let y = 0.4 + (2.0 * h.y - 0.9) * hash(s, 4);
            let t = (hash(s, 5) - 0.5) * 1.6 * ext;
            let p = *c + rot * (along * t + normal * (depth + 0.03));
            let p = Vec3::new(p.x, c.y - h.y + y, p.z);
            let size = 0.25 + 0.35 * hash(s, 6);
            let face = Mat4::from_rotation_translation(rot * Quat::from_rotation_arc(Vec3::Z, normal), p);
            // The scorched ring, the hole itself, jagged petals bent outwards.
            m.bevel_box(Vec3::new(size * 1.6, size * 1.4, 0.015), 0.01, 1, char, face * at(Vec3::ZERO));
            m.bevel_box(Vec3::new(size, size * 0.8, 0.02), 0.01, 1, soot, face * at(Vec3::Z * 0.01));
            for q in 0..4u32 {
                let a = q as f32 * std::f32::consts::FRAC_PI_2 + hash(q, s) * 0.6;
                let petal = face
                    * Mat4::from_rotation_translation(
                        Quat::from_rotation_z(a) * Quat::from_rotation_x(-0.7),
                        Vec3::new(a.cos() * size, a.sin() * size * 0.8, 0.05),
                    );
                m.bevel_box(Vec3::new(size * 0.35, 0.04, size * 0.3), 0.01, 1, Paint::painted(TIN[(hash(q, s + 1) * 6.0) as usize % 6], 0.95), petal);
            }
        }
        // Soot licking up from the holes.
        for k in 0..3u32 {
            let s = s0 + 40 + k;
            let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
            let x = (hash(s, 1) - 0.5) * 1.6 * h.x;
            let len = 0.4 + 0.6 * hash(s, 2);
            let p = *c + rot * Vec3::new(x, 0.0, sign * (h.z + 0.02));
            m.bevel_box(Vec3::new(0.25, len, 0.01), 0.005, 1, soot, Mat4::from_rotation_translation(rot, Vec3::new(p.x, c.y + h.y - len - 0.1, p.z)));
        }
        // Shards fallen at the foot.
        for k in 0..3u32 {
            let s = s0 + 70 + k;
            let a = hash(s, 1) * std::f32::consts::TAU;
            let r = h.x.max(h.z) * 0.6 + 0.8 + hash(s, 2) * 1.2;
            let p = *c + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
            let tilt = Quat::from_rotation_y(hash(s, 3) * 6.28) * Quat::from_rotation_x(0.15 + 0.3 * hash(s, 4));
            m.bevel_box(Vec3::new(0.5, 0.02, 0.35), 0.01, 1, Paint::painted(TIN[(hash(s, 5) * 6.0) as usize % 6], 0.95), Mat4::from_rotation_translation(tilt, Vec3::new(p.x, 0.06, p.z)));
        }
    }
    m
}

/// What a rubble heap is made of (by the obstacle that fell).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Debris {
    /// Tin sheets, timber, a door: shacks and fences.
    Tin,
    /// Canvas and poles: tents and market stalls.
    Canvas,
}

/// Stage 2: a heap (`flat` false: up to about 0.9 m, still low cover) or a
/// flattened spread (`flat`: under 0.3 m, drive over it) over each ground
/// box's footprint: buckled sheets, snapped timbers, the odd crate or tyre.
pub fn rubble(boxes: &[(Vec3, Vec3, f32)], flat: bool, debris: Debris, seed: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let wood = Paint::with(Material::Wood, 0x6a4a30, 0.85);
    let canvas = [0x8a8458, 0xb8322a, 0xe8e0c8, 0x2f6ad0];
    for (bi, (c, h, yaw)) in boxes.iter().enumerate() {
        let rot = Quat::from_rotation_y(*yaw);
        let area = (h.x * h.z).max(1.0);
        let n = ((area * if flat { 0.9 } else { 1.6 }) as u32).clamp(6, 40);
        let top = if flat { 0.25 } else { 0.85 };
        for k in 0..n {
            let s = seed * 31 + bi as u32 * 101 + k * 3;
            let lx = (hash(s, 1) - 0.5) * 2.0 * h.x * 0.95;
            let lz = (hash(s, 2) - 0.5) * 2.0 * h.z * 0.95;
            // Heaped: higher in the middle.
            let mid = 1.0 - ((lx / h.x.max(0.1)).powi(2) + (lz / h.z.max(0.1)).powi(2)).min(1.0);
            let y = 0.04 + (top - 0.15) * mid * (0.5 + 0.5 * hash(s, 3));
            let p = *c + rot * Vec3::new(lx, 0.0, lz);
            let p = Vec3::new(p.x, y, p.z);
            let tilt = rot * Quat::from_rotation_y(hash(s, 4) * 6.28) * Quat::from_rotation_x((hash(s, 5) - 0.5) * if flat { 0.25 } else { 0.9 }) * Quat::from_rotation_z((hash(s, 6) - 0.5) * if flat { 0.2 } else { 0.7 });
            let xf = Mat4::from_rotation_translation(tilt, p);
            let pick = hash(s, 7);
            match debris {
                Debris::Tin => {
                    if pick < 0.55 {
                        let tone = TIN[(hash(s, 8) * 6.0) as usize % 6];
                        m.bevel_box(Vec3::new(0.7 + 0.5 * hash(s, 9), 0.03, 0.45 + 0.3 * hash(s, 10)), 0.01, 1, Paint::painted(tone, 0.95), xf);
                    } else if pick < 0.85 {
                        m.bevel_box(Vec3::new(0.9 + 0.6 * hash(s, 9), 0.07, 0.07), 0.02, 1, wood, xf);
                    } else if pick < 0.93 {
                        m.bevel_box(Vec3::new(0.3, 0.25, 0.3), 0.03, 1, Paint::with(Material::Wood, 0x8a6a46, 0.8), xf);
                    } else {
                        m.torus(0.35, 0.12, 0.14, 12, Paint::rubber(0x1c1a18), xf);
                    }
                }
                Debris::Canvas => {
                    if pick < 0.6 {
                        let tone = canvas[(hash(s, 8) * 4.0) as usize % 4];
                        m.bevel_box(Vec3::new(0.9 + 0.6 * hash(s, 9), 0.02, 0.6 + 0.4 * hash(s, 10)), 0.01, 1, Paint::matte(tone), xf);
                    } else if pick < 0.85 {
                        m.bevel_box(Vec3::new(0.8 + 0.5 * hash(s, 9), 0.04, 0.04), 0.01, 1, Paint::steel(0x5a5652, 0.6), xf);
                    } else {
                        m.bevel_box(Vec3::new(0.3, 0.2, 0.25), 0.03, 1, Paint::with(Material::Wood, 0x8a6a46, 0.8), xf);
                    }
                }
            }
        }
        // Soot over the lot.
        let soot = Paint { color: crate::meshkit::srgb_hex(SOOT), roughness: 0.95, material: Material::GroundDecal, wear: 0.0 };
        m.bevel_box(Vec3::new(h.x * 1.05, 0.01, h.z * 1.05), 0.0, 0, soot, Mat4::from_rotation_translation(rot, Vec3::new(c.x, 0.02, c.z)));
    }
    m
}
