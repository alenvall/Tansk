//! Mode objectives in the world, in the kit language: Wellhead's derricks
//! (the Refinery's pumpjack on a pad, a mast flying the holder's flag, a
//! ring of painted ground plates that fill in the capturing crew's colour)
//! and the points of Last Crew's dust curtain (the wall itself is drawn as
//! dust by `effects`).

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};

/// The team-paint marker colour (the instance tint replaces it).
pub const TINT: u32 = 0xff00ff;
/// Where the flag hangs off the mast (pad frame).
pub const FLAG_AT: Vec3 = Vec3::new(-1.6, 9.2, 2.2);
/// Plates round the capture ring.
pub const RING_PLATES: usize = 40;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}

/// The derrick pad (frame on the ground): a concrete apron the pumpjack
/// stands on, a lattice mast with a crow's nest for the flag, sandbags.
pub fn well_pad() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let concrete = Paint::matte(0x9c9486);
    let steel = Paint::painted(0x4a4e52, 0.7);
    m.bevel_box(Vec3::new(5.4, 0.12, 2.0), 0.05, 1, concrete, at(Vec3::new(0.2, 0.06, 0.0)));
    // Hazard edge on the apron.
    for z in [-2.0f32, 2.0] {
        m.bevel_box(Vec3::new(5.4, 0.05, 0.08), 0.01, 1, Paint::with(Material::Hazard, 0xffffff, 0.6), at(Vec3::new(0.2, 0.14, z)));
    }
    // The mast: three legs braced, a little platform, the pole.
    let foot = Vec3::new(-1.6, 0.0, 2.2);
    let top = foot + Vec3::Y * 7.0;
    for k in 0..3 {
        let a = k as f32 / 3.0 * std::f32::consts::TAU;
        let f = foot + Vec3::new(a.cos(), 0.0, a.sin()) * 0.9;
        crate::tank::octo_pipe(&mut m, f, top + Vec3::new(a.cos(), 0.0, a.sin()) * 0.25, 0.07, steel);
    }
    for y in [2.0f32, 4.0, 6.0] {
        let s = 0.9 - (y / 7.0) * 0.65;
        m.bevel_box(Vec3::new(s, 0.04, s), 0.01, 1, steel, at(foot + Vec3::Y * y));
    }
    m.bevel_box(Vec3::new(0.55, 0.06, 0.55), 0.02, 1, Paint::painted(0x6a5a44, 0.6), at(top));
    m.cylinder(0.06, 1.6, 0.01, 1, 8, Paint::steel(0x8a867e, 0.35), at(top + Vec3::Y * 1.6));
    m.cylinder(0.11, 0.08, 0.02, 1, 8, Paint::painted(0xd8b030, 0.4), at(top + Vec3::Y * 3.25));
    // Sandbags round the wellhead end.
    for (x, z, yaw) in [(4.6f32, -2.6f32, 0.2f32), (5.8, 1.4, 1.4), (2.4, 2.8, -0.1)] {
        for k in 0..3 {
            let off = Quat::from_rotation_y(yaw) * Vec3::new((k as f32 - 1.0) * 0.75, 0.0, 0.0);
            m.bevel_box(Vec3::new(0.36, 0.18, 0.24), 0.12, 2, Paint::matte(0xb8a47a), Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), Vec3::new(x, 0.18, z) + off));
        }
    }
    m
}

/// The flag, hanging from its pole on +X (frame at the pole, top edge).
/// Team paint: the instance tint is the holder's colour.
pub fn well_flag() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let cloth = Paint::team(TINT, 0.35);
    // A cloth in three panels, each a little further off the plane: it hangs and ripples.
    for k in 0..3 {
        let x = 0.35 + k as f32 * 0.7;
        let z = (k as f32 * 1.7).sin() * 0.08;
        m.bevel_box(Vec3::new(0.36, 0.62, 0.025), 0.01, 1, cloth, Mat4::from_rotation_translation(Quat::from_rotation_y(0.08 * k as f32), Vec3::new(x, -0.62, z)));
    }
    // A stencilled band.
    m.bevel_box(Vec3::new(1.05, 0.08, 0.03), 0.01, 1, Paint::painted(0x1c1812, 0.5), at(Vec3::new(1.05, -0.62, 0.0)));
    m
}

/// One plate of the capture ring, long along X, flat on the ground.
pub fn ring_plate(len: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(Vec3::new(len * 0.5, 0.06, 0.32), 0.03, 1, Paint::team(TINT, 0.5), at(Vec3::Y * 0.06));
    m
}

/// A stake every few plates, so the ring reads from a distance.
pub fn ring_stake() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.cylinder(0.05, 0.55, 0.01, 1, 6, Paint::steel(0x4a4440, 0.5), at(Vec3::Y * 0.55));
    m.bevel_box(Vec3::new(0.06, 0.18, 0.06), 0.02, 1, Paint::with(Material::Hazard, 0xffffff, 0.6), at(Vec3::Y * 1.0));
    m
}

/// Points along Last Crew's wall for the dust curtain: (base point, puff
/// size, opacity, seed). Dense near `eye`, sparse round the far side.
pub fn curtain_points(center: Vec2, radius: f32, eye: Vec3, time: f32, ground: &dyn Fn(f32, f32) -> f32) -> Vec<(Vec3, f32, f32, f32)> {
    let mut out = Vec::new();
    if radius < 1.0 {
        return out;
    }
    let tau = std::f32::consts::TAU;
    let e = Vec2::new(eye.x, eye.z);
    let a0 = (e.y - center.y).atan2(e.x - center.x);
    // Layers: (height, size, opacity).
    const NEAR: [(f32, f32, f32); 5] = [(2.5, 8.0, 0.92), (9.0, 10.0, 0.8), (17.0, 12.0, 0.6), (27.0, 14.0, 0.45), (39.0, 16.0, 0.3)];
    const FAR: [(f32, f32, f32); 3] = [(6.0, 26.0, 0.8), (24.0, 30.0, 0.6), (44.0, 32.0, 0.35)];
    let near_len = 360.0f32.min(radius * std::f32::consts::PI);
    let step = 7.0;
    let n = (2.0 * near_len / step) as i32;
    // Snap the samples to the circle (not to the eye) so they don't swim as you move.
    let da = step / radius;
    let k0 = ((a0 - near_len / radius) / da).floor() as i32;
    for i in 0..=n {
        let k = k0 + i;
        let a = k as f32 * da;
        let seed = ((k.rem_euclid(9973)) as f32 * 0.618).fract();
        let drift = (time * 0.35 + seed * tau).sin() * 2.5;
        let a = a + drift / radius;
        let p = center + Vec2::new(a.cos(), a.sin()) * radius;
        let d = p.distance(e);
        if d > 420.0 {
            continue;
        }
        let g = ground(p.x, p.y);
        for (li, (h, size, alpha)) in NEAR.iter().enumerate() {
            let bob = (time * 0.5 + seed * 7.0 + li as f32).sin() * 1.2;
            out.push((Vec3::new(p.x, g + h + bob, p.y), *size, *alpha, seed + li as f32 * 0.13));
        }
    }
    // The rest of the ring: big, sparse, far.
    let far_step = 38.0 / radius;
    let m = (tau / far_step) as i32;
    for k in 0..m {
        let a = k as f32 * far_step;
        let p = center + Vec2::new(a.cos(), a.sin()) * radius;
        let d = p.distance(e);
        if d <= 400.0 || d > 1400.0 {
            continue;
        }
        let g = ground(p.x, p.y);
        let seed = (k as f32 * 0.377).fract();
        for (h, size, alpha) in FAR {
            out.push((Vec3::new(p.x, g + h, p.y), size, alpha, seed));
        }
    }
    out
}
