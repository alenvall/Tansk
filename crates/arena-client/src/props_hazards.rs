//! The spectacle hazards' kit: the hand-welded spring pad and the fuel
//! trailer. Same language as the rest: chunky bevelled plates, rivets and
//! weld beads, paint worn to steel, hazard stripes. Obstacle-shaped builders
//! take the collider's half extents and are centred on the box (like `props`).

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

/// A spring pad over its (flush) box, standing on y = 0, arrow along +X: a
/// dark steel pit frame, a scrap leaf spring showing down each side, the
/// riveted top plate on it with welded brackets, a hazard border and big
/// painted chevrons pointing the way you fly.
pub fn spring_pad(half: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let dark = Paint::steel(0x2e2b28, 0.6);
    let plate = Paint::painted(0x7a7268, 0.8);
    let spring = Paint::steel(0x5a4a3c, 0.55);
    let (hx, hz) = (half.x, half.z);
    // The pit frame, flush with the ground.
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(hx + 0.25, 0.08, 0.22), 0.03, 1, dark, at(Vec3::new(0.0, 0.04, s * (hz + 0.1))));
        m.bevel_box(Vec3::new(0.22, 0.08, hz + 0.25), 0.03, 1, dark, at(Vec3::new(s * (hx + 0.1), 0.04, 0.0)));
    }
    // A leaf spring down each side: stacked leaves, each shorter, bowed
    // (built from two tilted halves), with eye shackles at the ends.
    for s in [-1.0f32, 1.0] {
        let z = s * (hz - 0.35);
        for k in 0..4u32 {
            let len = hx * (0.95 - 0.18 * k as f32);
            let y = 0.05 + 0.03 * k as f32;
            for side in [-1.0f32, 1.0] {
                let rot = Quat::from_rotation_z(side * -0.07);
                m.bevel_box(Vec3::new(len * 0.5, 0.025, 0.16), 0.01, 1, spring, Mat4::from_rotation_translation(rot, Vec3::new(side * len * 0.5, y, z)));
            }
        }
        for end in [-1.0f32, 1.0] {
            m.cylinder(0.09, 0.2, 0.02, 1, 10, dark, along_z(Vec3::new(end * hx * 0.95, 0.12, z)));
        }
        // The centre clamp, welded up to the plate.
        m.bevel_box(Vec3::new(0.18, 0.06, 0.22), 0.03, 1, dark, at(Vec3::new(0.0, 0.12, z)));
    }
    // The top plate, a touch proud of the frame.
    let top = 0.16;
    m.bevel_box(Vec3::new(hx - 0.1, 0.06, hz - 0.1), 0.04, 1, plate, at(Vec3::Y * top));
    // Hazard border and the chevrons.
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(hx - 0.1, 0.02, 0.18), 0.01, 1, hazard(), at(Vec3::new(0.0, top + 0.06, s * (hz - 0.3))));
    }
    let yellow = Paint::painted(0xe0b020, 0.5);
    for k in 0..3 {
        let x = -1.1 + k as f32 * 1.0;
        for s in [-1.0f32, 1.0] {
            let rot = Quat::from_rotation_y(s * 0.75);
            m.bevel_box(Vec3::new(0.16, 0.02, 0.75), 0.01, 1, yellow, Mat4::from_rotation_translation(rot, Vec3::new(x, top + 0.065, s * 0.48)));
        }
    }
    // Weld beads and rivets round the plate.
    let mut x = -hx + 0.35;
    while x < hx - 0.3 {
        for s in [-1.0f32, 1.0] {
            m.rivet(Vec3::new(x, top + 0.06, s * (hz - 0.55)), Vec3::Y, 0.06, Paint::steel(0x9a958c, 0.4));
        }
        x += 0.55;
    }
    // A stop lamp on a stub at the front corners.
    for s in [-1.0f32, 1.0] {
        let p = Vec3::new(hx + 0.1, 0.0, s * (hz + 0.1));
        octo_pipe(&mut m, p, p + Vec3::Y * 0.7, 0.05, dark);
        m.cylinder(0.11, 0.08, 0.03, 1, 10, Paint::glow(0xff8a20, 1.5), at(p + Vec3::Y * 0.78));
    }
    m
}

/// Paint for the trailers' tanks, by variant.
const TANKER: [u32; 3] = [0xd9d6cc, 0xd8a52a, 0xb8b6ae];

/// A fuel trailer, half extents `h` (long along X, hitch at +X), centred:
/// a fat round tank on a chassis, dual wheels at the back, landing legs at
/// the front, a catwalk with hatches on top, a red FLAMMABLE band and
/// placard, valves and hoses at the back, rust streaks down the sides.
pub fn fuel_trailer(h: Vec3, variant: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let (hx, hy, hz) = (h.x, h.y, h.z);
    let base = -hy;
    let dark = Paint::steel(0x2c2a28, 0.6);
    let tank = Paint::painted(TANKER[variant as usize % 3], 0.6);
    let red = Paint::painted(0xb8281e, 0.5);
    // Chassis rails and the rear bumper.
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(hx - 0.2, 0.12, 0.1), 0.03, 1, dark, at(Vec3::new(0.0, base + 1.0, s * 0.55)));
    }
    m.bevel_box(Vec3::new(0.1, 0.12, hz * 0.9), 0.03, 1, hazard(), at(Vec3::new(-hx + 0.1, base + 0.75, 0.0)));
    // Wheels: two axles of duals at the back.
    for ax in [-hx + 1.4, -hx + 2.6] {
        for s in [-1.0f32, 1.0] {
            for d in [0.0f32, 0.32] {
                let z = s * (hz - 0.18 - d);
                m.cylinder(0.5, 0.14, 0.07, 2, 16, Paint::rubber(0x1c1a18), along_z(Vec3::new(ax, base + 0.5, z)));
            }
            m.cylinder(0.24, 0.16, 0.03, 1, 10, Paint::steel(0x8a8682, 0.45), along_z(Vec3::new(ax, base + 0.5, s * (hz - 0.08))));
        }
        m.cylinder(0.08, hz - 0.2, 0.02, 1, 8, dark, along_z(Vec3::new(ax, base + 0.5, 0.0)));
    }
    // Mudguards over them.
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(1.2, 0.05, 0.36), 0.03, 1, dark, at(Vec3::new(-hx + 2.0, base + 1.1, s * (hz - 0.33))));
    }
    // Landing legs and the kingpin plate at the front.
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.08, 0.5, 0.08), 0.02, 1, dark, at(Vec3::new(hx - 1.6, base + 0.5, s * 0.6)));
        m.bevel_box(Vec3::new(0.2, 0.04, 0.2), 0.02, 1, dark, at(Vec3::new(hx - 1.6, base + 0.03, s * 0.6)));
    }
    m.bevel_box(Vec3::new(0.6, 0.06, 0.7), 0.02, 1, dark, at(Vec3::new(hx - 0.7, base + 1.05, 0.0)));
    // The tank: a round barrel with domed ends.
    let r = hz.min((2.0 * hy - 1.1) * 0.5) - 0.02;
    let cy = base + 1.1 + r;
    let len = hx - 0.35;
    m.cylinder(r, len, r * 0.35, 3, 24, tank, along_x(Vec3::new(0.0, cy, 0.0)));
    // Reinforcing rings, the red band and the placard diamond.
    for k in 0..5 {
        let x = -len + 0.6 + k as f32 * (2.0 * len - 1.2) / 4.0;
        m.cylinder(r + 0.03, 0.05, 0.02, 1, 24, Paint::painted(TANKER[variant as usize % 3], 0.8), along_x(Vec3::new(x, cy, 0.0)));
    }
    m.cylinder(r + 0.02, 0.35, 0.02, 1, 24, red, along_x(Vec3::new(len * 0.25, cy, 0.0)));
    for s in [-1.0f32, 1.0] {
        let diamond = Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4), Vec3::new(-len * 0.45, cy, s * (r + 0.02)));
        m.bevel_box(Vec3::new(0.32, 0.32, 0.02), 0.01, 1, red, diamond);
        m.bevel_box(Vec3::new(0.22, 0.22, 0.025), 0.01, 1, Paint::painted(0xf0e8d0, 0.4), diamond);
        // A flame mark in the diamond.
        m.bevel_box(Vec3::new(0.05, 0.12, 0.03), 0.01, 1, red, at(Vec3::new(-len * 0.45, cy, s * (r + 0.05))));
        // Rust streaks down the side.
        for k in 0..4 {
            let x = (hash(k, variant + 3) - 0.5) * 2.0 * len * 0.8;
            let l = 0.3 + 0.5 * hash(k, variant + 5);
            m.bevel_box(Vec3::new(0.08 + 0.08 * hash(k, variant + 7), l, 0.012), 0.005, 1, Paint::painted(0x7a3a1e, 1.0), at(Vec3::new(x, cy - r * 0.35 - l, s * (r * 0.93 + 0.02))));
        }
    }
    // The catwalk and hatches on top, a hand rail.
    let top = cy + r;
    m.bevel_box(Vec3::new(len * 0.75, 0.03, 0.28), 0.01, 1, dark, at(Vec3::new(0.0, top + 0.05, 0.0)));
    for k in 0..3 {
        let x = -len * 0.5 + k as f32 * len * 0.5;
        m.cylinder(0.32, 0.1, 0.05, 1, 14, Paint::steel(0x8a8682, 0.4), at(Vec3::new(x, top + 0.12, 0.0)));
    }
    for k in 0..6 {
        let x = -len * 0.7 + k as f32 * len * 0.28;
        octo_pipe(&mut m, Vec3::new(x, top + 0.05, 0.3), Vec3::new(x, top + 0.7, 0.3), 0.025, Paint::painted(0xd6a21e, 0.7));
    }
    m.bevel_box(Vec3::new(len * 0.7, 0.025, 0.025), 0.01, 1, Paint::painted(0xd6a21e, 0.7), at(Vec3::new(0.0, top + 0.7, 0.3)));
    // Valves and a hose loop at the back, the ladder.
    let back = Vec3::new(-len - r * 0.3, cy - r * 0.6, 0.0);
    m.cylinder(0.12, 0.25, 0.03, 1, 10, dark, along_x(back));
    m.torus(0.16, 0.04, 0.05, 12, red, Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), back - Vec3::X * 0.25));
    octo_pipe(&mut m, back + Vec3::new(-0.2, 0.0, 0.3), Vec3::new(-hx + 0.2, base + 0.7, hz - 0.2), 0.06, Paint::rubber(0x1a1a18));
    for k in 0..5 {
        let y = base + 1.2 + k as f32 * 0.45;
        m.bevel_box(Vec3::new(0.03, 0.03, 0.25), 0.01, 1, dark, at(Vec3::new(-len - 0.05, y, 0.55)));
    }
    m
}
