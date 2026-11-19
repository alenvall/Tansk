//! The Gorge's kit: the steel girder bridges from the rims to the butte.
//! (The rock arches are built from the shared rock blocks in `props`.)
//! Same language as the rest: chunky riveted steel in worn paint.

use glam::{Mat4, Quat, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};
use crate::shapes::hash;
use crate::tank::octo_pipe;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}

/// A girder bridge (whole obstacle, frame on the low ground under its
/// middle, the deck along Z): two riveted Warren trusses in rust-red
/// paint, cross beams under a plank deck with steel running strips, a
/// rail of hazard paint at each end, a lantern on a post.
pub fn girder_bridge(deck: (Vec3, Vec3)) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let (dc, dh) = deck;
    let red = Paint::painted(0x9a3a22, 0.85);
    let dark = Paint::steel(0x3a3632, 0.6);
    let plank = Paint::with(Material::Wood, 0x7a5a3a, 0.85);
    let road = dc.y + dh.y;
    let base = dc.y - dh.y;
    let truss_h = 3.2;
    let bays = ((2.0 * dh.z) / 4.5).round().max(2.0) as i32;
    let bay = 2.0 * dh.z / bays as f32;
    for s in [-1.0f32, 1.0] {
        let x = dc.x + s * (dh.x + 0.25);
        // Bottom and top chords.
        m.bevel_box(Vec3::new(0.22, 0.28, dh.z), 0.05, 1, red, at(Vec3::new(x, base + 0.3, dc.z)));
        m.bevel_box(Vec3::new(0.22, 0.22, dh.z - bay * 0.5), 0.05, 1, red, at(Vec3::new(x, road + truss_h, dc.z)));
        // Verticals and diagonals (a Warren truss).
        for k in 0..=bays {
            let z = dc.z - dh.z + k as f32 * bay;
            let top_ok = k > 0 && k < bays;
            if top_ok {
                m.bevel_box(Vec3::new(0.16, (truss_h + road - base) * 0.5, 0.16), 0.03, 1, red, at(Vec3::new(x, (base + road + truss_h) * 0.5, z)));
            }
            if k < bays {
                let (a, b) = if k % 2 == 0 {
                    (Vec3::new(x, base + 0.4, z), Vec3::new(x, road + truss_h, z + bay))
                } else {
                    (Vec3::new(x, road + truss_h, z), Vec3::new(x, base + 0.4, z + bay))
                };
                let (a, b) = if k == 0 { (Vec3::new(x, base + 0.4, z), Vec3::new(x, road + truss_h, z + bay)) } else { (a, b) };
                octo_pipe(&mut m, a, b, 0.13, red);
            }
            // Rivet plates at the joints.
            for y in [base + 0.3, road + truss_h] {
                m.bevel_box(Vec3::new(0.24, 0.3, 0.3), 0.02, 1, dark, at(Vec3::new(x + s * 0.03, y, z)));
            }
        }
        // Rust streaks down the side.
        for k in 0..6 {
            let z = dc.z + (hash(k, 3 + (s > 0.0) as u32) - 0.5) * 2.0 * dh.z;
            m.bevel_box(Vec3::new(0.02, 0.4 + 0.4 * hash(k, 9), 0.1), 0.005, 1, Paint::painted(0x6a2a14, 1.0), at(Vec3::new(x + s * 0.24, base + 0.1, z)));
        }
    }
    // Cross beams and the plank deck with two steel running strips.
    let mut z = dc.z - dh.z + 0.6;
    while z < dc.z + dh.z - 0.4 {
        m.bevel_box(Vec3::new(dh.x + 0.3, 0.18, 0.18), 0.03, 1, dark, at(Vec3::new(dc.x, base + 0.25, z)));
        z += 2.2;
    }
    let n = ((2.0 * dh.z) / 0.62).floor() as i32;
    for k in 0..n {
        let z = dc.z - dh.z + 0.31 + k as f32 * 0.62;
        let tone = 0.85 + 0.25 * hash(k as u32, 7);
        let p = Paint { color: crate::meshkit::srgb_hex(0x7a5a3a) * tone, ..plank };
        let sag = (hash(k as u32, 8) - 0.5) * 0.04;
        m.bevel_box(Vec3::new(dh.x - 0.1, 0.09, 0.28), 0.02, 1, p, at(Vec3::new(dc.x, road - 0.12 + sag, z)));
    }
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.5, 0.03, dh.z), 0.01, 1, Paint::steel(0x8a8682, 0.4), at(Vec3::new(dc.x + s * 2.0, road - 0.01, dc.z)));
    }
    // Hazard rails at the ends, a lantern on a post.
    for e in [-1.0f32, 1.0] {
        for s in [-1.0f32, 1.0] {
            m.bevel_box(Vec3::new(0.3, 0.45, 0.3), 0.05, 1, Paint::with(Material::Hazard, 0xffffff, 0.6), at(Vec3::new(dc.x + s * (dh.x - 0.2), road + 0.45, dc.z + e * (dh.z - 0.4))));
        }
    }
    let post = Vec3::new(dc.x + dh.x - 0.3, road, dc.z + dh.z * 0.4);
    octo_pipe(&mut m, post, post + Vec3::Y * 3.4, 0.07, dark);
    m.bevel_box(Vec3::new(0.18, 0.25, 0.18), 0.04, 1, Paint::glow(0xffb040, 1.8), Mat4::from_rotation_translation(Quat::IDENTITY, post + Vec3::Y * 3.6));
    m
}
