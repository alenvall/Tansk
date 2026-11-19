//! Upgrade part meshes, one distinct silhouette per `PartKind`, in the frame
//! the simulation mounts it in (gun parts in the gun frame with the barrel
//! along +X; hull parts in the hull frame, origin at the hull centre about
//! 0.95 m above the ground; turret parts in the turret frame). Shapes follow
//! the part hit boxes in arena-core, so what you see is what you can hit.

use arena_core::PartKind;
use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};
use crate::tank::{crate_box, octo_pipe, plate};

const TRIM: u32 = 0x2a2725;
const BARE: u32 = 0x9a958c;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}
fn along_x(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p)
}

fn bolt_ring(m: &mut MeshBuilder, centre: Vec3, r: f32, n: u32, size: f32) {
    for k in 0..n {
        let a = (k as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
        let d = Vec3::new(0.0, a.sin(), a.cos());
        m.rivet(centre + d * r, d, size, Paint::steel(BARE, 0.35));
    }
}

/// A barrel along +X from `x0` to `x1` at height/side `yz`, with a
/// reinforcing band and a muzzle ring.
fn barrel(m: &mut MeshBuilder, team: u32, yz: Vec3, x0: f32, x1: f32, r: f32, muzzle_r: f32, muzzle_len: f32) {
    let paint = Paint::team(team, 0.35);
    let trim = Paint::steel(TRIM, 0.5);
    let len = x1 - x0;
    m.cylinder(r, len * 0.5, 0.02, 1, 16, paint, along_x(yz + Vec3::X * (x0 + len * 0.5)));
    m.cylinder(r * 1.22, 0.07, 0.025, 2, 16, trim, along_x(yz + Vec3::X * (x0 + 0.35)));
    let mz = yz + Vec3::X * (x1 - muzzle_len * 0.5);
    m.cylinder(muzzle_r, muzzle_len * 0.5, 0.05, 2, 16, trim, along_x(mz));
    bolt_ring(m, mz, muzzle_r, 8, muzzle_r * 0.13);
    m.cylinder(muzzle_r * 0.58, 0.01, 0.0, 1, 12, Paint::with(Material::BareSteel, 0x0c0a09, 0.9), along_x(mz + Vec3::X * (muzzle_len * 0.5 + 0.005)));
}

/// The mantlet frame at the gun origin, `h` half size.
fn mantlet(m: &mut MeshBuilder, h: f32, depth: f32) {
    let trim = Paint::steel(TRIM, 0.5);
    let w = h * 0.22;
    m.bevel_box(Vec3::new(depth, w, h), 0.045, 2, trim, at(Vec3::new(0.12, h - w, 0.0)));
    m.bevel_box(Vec3::new(depth, w, h), 0.045, 2, trim, at(Vec3::new(0.12, -h + w, 0.0)));
    m.bevel_box(Vec3::new(depth, h, w), 0.045, 2, trim, at(Vec3::new(0.12, 0.0, h - w)));
    m.bevel_box(Vec3::new(depth, h, w), 0.045, 2, trim, at(Vec3::new(0.12, 0.0, -h + w)));
    for (y, z) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        m.rivet(Vec3::new(0.12 + depth, y * (h - w), z * (h - w)), Vec3::X, 0.06, Paint::steel(BARE, 0.35));
    }
}

/// The gun for a gun part (gun frame). `None` kinds fall back to the stub.
pub fn gun_mesh(kind: PartKind, team: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::team(team, 0.35);
    match kind {
        PartKind::HeavyCannon => {
            mantlet(&mut m, 0.45, 0.15);
            m.cylinder(0.3, 0.16, 0.05, 2, 16, paint, along_x(Vec3::X * 0.2));
            barrel(&mut m, team, Vec3::ZERO, 0.3, 3.2, 0.19, 0.31, 0.45);
        }
        PartKind::BigBore => {
            mantlet(&mut m, 0.52, 0.2);
            m.cylinder(0.42, 0.22, 0.06, 2, 16, paint, along_x(Vec3::X * 0.25));
            // A short fat tube with a huge reinforced bell.
            barrel(&mut m, team, Vec3::ZERO, 0.4, 2.8, 0.31, 0.46, 0.6);
            for x in [1.0f32, 1.7] {
                m.cylinder(0.36, 0.06, 0.02, 1, 16, Paint::steel(TRIM, 0.5), along_x(Vec3::X * x));
            }
        }
        PartKind::TwinBarrel => {
            mantlet(&mut m, 0.48, 0.15);
            for z in [-0.28f32, 0.28] {
                barrel(&mut m, team, Vec3::Z * z, 0.25, 3.0, 0.13, 0.19, 0.32);
            }
            // Clamp bands tying them together.
            for x in [1.0f32, 2.0] {
                m.bevel_box(Vec3::new(0.08, 0.17, 0.48), 0.04, 2, Paint::steel(TRIM, 0.5), at(Vec3::X * x));
            }
        }
        PartKind::LongGun => {
            mantlet(&mut m, 0.4, 0.15);
            barrel(&mut m, team, Vec3::ZERO, 0.25, 5.2, 0.12, 0.2, 0.55);
            // Slotted muzzle brake.
            m.bevel_box(Vec3::new(0.2, 0.1, 0.26), 0.03, 1, Paint::steel(TRIM, 0.5), at(Vec3::X * 5.25));
            // Scope on the left, with a glinting lens.
            m.cylinder(0.11, 0.45, 0.02, 1, 12, Paint::steel(TRIM, 0.4), along_x(Vec3::new(0.4, 0.38, -0.35)));
            m.cylinder(0.14, 0.06, 0.02, 1, 12, Paint::steel(TRIM, 0.4), along_x(Vec3::new(0.88, 0.38, -0.35)));
            m.cylinder(0.1, 0.005, 0.0, 1, 12, Paint::glow(0x9fd8ff, 1.5), along_x(Vec3::new(0.95, 0.38, -0.35)));
            m.bevel_box(Vec3::new(0.08, 0.12, 0.05), 0.02, 1, Paint::steel(TRIM, 0.4), at(Vec3::new(0.4, 0.25, -0.3)));
        }
        _ => {
            // Stub gun.
            mantlet(&mut m, 0.36, 0.12);
            barrel(&mut m, team, Vec3::ZERO, 0.2, 2.0, 0.12, 0.18, 0.3);
        }
    }
    // The mantlet sits proud of the turret face.
    m.transform_from(0, at(Vec3::X * GUN_FACE));
    m
}

/// How far ahead of the gun pivot the mantlet face sits.
pub const GUN_FACE: f32 = 0.32;

/// Muzzle distance along the gun frame's +X, per gun.
#[allow(dead_code)]
pub fn muzzle_x(kind: PartKind) -> f32 {
    GUN_FACE
        + match kind {
            PartKind::HeavyCannon => 3.2,
            PartKind::BigBore => 2.8,
            PartKind::TwinBarrel => 3.0,
            PartKind::LongGun => 5.45,
            _ => 2.0,
        }
}

/// A non-gun part, in its own frame (see the module docs).
pub fn part_mesh(kind: PartKind, team: u32) -> MeshBuilder {
    if arena_core::parts::SHIP_MODULES.contains(&kind) {
        return crate::ships::ship_part(kind, team);
    }
    let mut m = MeshBuilder::new();
    let paint = Paint::team(team, 0.4);
    let rivet = Paint::team(team, 0.2);
    let trim = Paint::steel(TRIM, 0.5);
    let bare = Paint::steel(BARE, 0.35);
    match kind {
        PartKind::SensorMast => {
            // Hull frame (-2.4, 2.0, -1.1): a lattice mast and a sensor head.
            let b = Vec3::new(-2.4, 0.55, -1.1);
            m.bevel_box(Vec3::new(0.25, 0.08, 0.25), 0.03, 1, trim, at(b));
            octo_pipe(&mut m, b, b + Vec3::Y * 3.1, 0.08, trim);
            for k in 0..5 {
                m.cylinder(0.12, 0.03, 0.01, 1, 8, bare, at(b + Vec3::Y * (0.5 + k as f32 * 0.55)));
            }
            let head = Vec3::new(-2.4, 3.7, -1.1);
            m.bevel_box(Vec3::new(0.3, 0.18, 0.3), 0.06, 2, paint, at(head));
            m.cylinder(0.12, 0.04, 0.01, 1, 10, Paint::glow(0x5fc8ff, 2.0), along_x(head + Vec3::X * 0.31));
            let dish = [(0.0f32, 0.0f32, Vec2::NEG_Y), (0.32, 0.12, Vec2::new(-0.35, -0.94)), (0.34, 0.14, Vec2::Y), (0.0, 0.05, Vec2::Y)];
            m.lathe(&dish, 12, false, Paint::painted(0xb8b4a8, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_z(-1.2), head + Vec3::new(-0.1, 0.35, 0.0)));
        }
        PartKind::Coilgun => {
            // Turret frame (0.5, 0.55, 1.15): a rail with glowing coils.
            let c = Vec3::new(0.5, 0.55, 1.15);
            m.bevel_box(Vec3::new(1.2, 0.08, 0.2), 0.03, 2, trim, at(c - Vec3::Y * 0.1));
            m.bevel_box(Vec3::new(1.2, 0.06, 0.06), 0.02, 1, bare, at(c + Vec3::Y * 0.05));
            for k in 0..6 {
                let x = c.x - 0.95 + k as f32 * 0.38;
                m.cylinder(0.17, 0.06, 0.02, 1, 12, Paint::steel(0x8a5a2a, 0.3), along_x(Vec3::new(x, c.y, c.z)));
                m.cylinder(0.175, 0.025, 0.0, 1, 12, Paint::glow(0x40a8ff, 3.0), along_x(Vec3::new(x + 0.07, c.y, c.z)));
            }
            m.bevel_box(Vec3::new(0.25, 0.15, 0.12), 0.03, 1, paint, at(Vec3::new(c.x - 1.1, c.y, c.z - 0.05)));
        }
        PartKind::TeslaSpire => {
            // Hull frame (-2.4, 1.2, 1.1): stacked insulators and a ball.
            let b = Vec3::new(-2.4, 0.5, 1.1);
            m.bevel_box(Vec3::new(0.3, 0.1, 0.3), 0.04, 2, trim, at(b));
            for k in 0..5 {
                m.cylinder(0.24 - k as f32 * 0.02, 0.06, 0.03, 2, 10, Paint::matte(0xd8d0c0), at(b + Vec3::Y * (0.25 + k as f32 * 0.26)));
                m.cylinder(0.12, 0.1, 0.0, 1, 8, trim, at(b + Vec3::Y * (0.37 + k as f32 * 0.26)));
            }
            let ball = Vec3::new(-2.4, 2.25, 1.1);
            m.rock_blob(7, Vec3::splat(0.36), Paint::steel(0xb0a890, 0.25), at(ball));
            m.rock_blob(9, Vec3::splat(0.22), Paint::glow(0x80c8ff, 4.0), at(ball + Vec3::Y * 0.12));
        }
        PartKind::FlankPlates => {
            for side in [-1.0f32, 1.0] {
                for k in 0..3 {
                    let x = -1.7 + k as f32 * 1.7;
                    plate(&mut m, Vec3::new(x, 0.1, side * 1.92), Vec3::new(0.82, 0.48, 0.06), 2, side, paint, rivet);
                }
                for x in [-2.4f32, 0.0, 2.4] {
                    m.bevel_box(Vec3::new(0.06, 0.06, 0.12), 0.02, 1, trim, at(Vec3::new(x, 0.45, side * 1.82)));
                }
            }
        }
        PartKind::Bulwark => {
            // A thick front slab and a plow below it.
            plate(&mut m, Vec3::new(3.2, 0.1, 0.0), Vec3::new(0.16, 0.72, 1.82), 0, 1.0, paint, rivet);
            for z in [-1.2f32, 0.0, 1.2] {
                m.bevel_box(Vec3::new(0.3, 0.08, 0.08), 0.03, 1, trim, at(Vec3::new(3.0, 0.4, z)));
            }
            m.wedge(Vec3::new(0.25, 0.25, 1.9), 0.05, 2, 0.15, Paint::steel(0x6a6058, 0.5), at(Vec3::new(3.5, -0.55, 0.0)));
            for k in 0..7 {
                let z = -1.65 + k as f32 * 0.55;
                m.wedge(Vec3::new(0.16, 0.1, 0.12), 0.02, 1, 0.1, bare, at(Vec3::new(3.85, -0.72, z)));
            }
        }
        PartKind::SpacedArmour => {
            // Overlapping riveted plates on brackets, all round.
            for side in [-1.0f32, 1.0] {
                for k in 0..4 {
                    let x = -2.25 + k as f32 * 1.5;
                    let tilt = Quat::from_rotation_y((k as f32 - 1.5) * 0.03);
                    let c = Vec3::new(x, -0.15 + 0.03 * (k % 2) as f32, side * (2.05 + 0.03 * (k % 2) as f32));
                    m.bevel_box(Vec3::new(0.8, 0.48, 0.05), 0.03, 2, paint, Mat4::from_rotation_translation(tilt, c));
                    for dx in [-0.6f32, 0.0, 0.6] {
                        for dy in [-0.32f32, 0.32] {
                            m.rivet(c + Vec3::new(dx, dy, side * 0.05), Vec3::Z * side, 0.05, rivet);
                        }
                    }
                    m.bevel_box(Vec3::new(0.05, 0.05, 0.25), 0.02, 1, trim, at(Vec3::new(x, 0.3, side * 1.85)));
                }
            }
            for sx in [-1.0f32, 1.0] {
                plate(&mut m, Vec3::new(sx * 3.15, 0.1, 0.0), Vec3::new(0.06, 0.38, 1.55), 0, sx, paint, rivet);
            }
        }
        PartKind::Nitro => {
            // Twin blue bottles on the rear plate with red valves.
            for z in [-0.45f32, 0.45] {
                m.cylinder(0.2, 0.42, 0.12, 3, 14, Paint::painted(0x2f6aa8, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), Vec3::new(-3.2, 0.2, z)));
                m.cylinder(0.07, 0.06, 0.02, 1, 8, Paint::painted(0xb02a20, 0.3), Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), Vec3::new(-3.2, 0.2, z + z.signum() * 0.48)));
            }
            for z in [-0.8f32, 0.0, 0.8] {
                m.bevel_box(Vec3::new(0.16, 0.04, 0.05), 0.015, 1, trim, at(Vec3::new(-3.12, 0.2, z)));
            }
        }
        PartKind::Afterburner => {
            let c = Vec3::new(-2.5, 0.8, 0.0);
            m.bevel_box(Vec3::new(0.5, 0.25, 0.42), 0.06, 2, paint, at(c));
            let n = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            m.cylinder(0.3, 0.28, 0.04, 2, 12, Paint::steel(0x4a4440, 0.5), Mat4::from_rotation_translation(n, c - Vec3::X * 0.65));
            m.cylinder(0.24, 0.01, 0.0, 1, 12, Paint::glow(0xff8a30, 2.5), Mat4::from_rotation_translation(n, c - Vec3::X * 0.94));
            bolt_ring(&mut m, c - Vec3::X * 0.5, 0.31, 8, 0.04);
        }
        PartKind::TwinEngines => {
            for z in [-0.45f32, 0.45] {
                let c = Vec3::new(-2.4, 0.8, z);
                m.bevel_box(Vec3::new(0.45, 0.3, 0.3), 0.06, 2, Paint::steel(0x5a5650, 0.4), at(c));
                for k in 0..4 {
                    m.bevel_box(Vec3::new(0.03, 0.31, 0.31), 0.01, 1, trim, at(c + Vec3::X * (-0.3 + k as f32 * 0.2)));
                }
                octo_pipe(&mut m, c + Vec3::new(-0.2, 0.3, 0.0), c + Vec3::new(-0.35, 1.1, z * 0.3), 0.08, Paint::steel(0x2b2725, 0.75));
            }
            crate_box(&mut m, Vec3::new(-2.0, 1.25, 0.0), Vec3::new(0.25, 0.15, 0.2));
        }
        PartKind::Flamethrower => {
            // Gun frame (1.1, -0.3, 0.32): a nozzle under the barrel, its
            // fuel bottle and feed hose, a pilot light at the tip.
            let c = Vec3::new(1.1, -0.3, 0.32);
            m.cylinder(0.09, 0.7, 0.02, 1, 8, trim, along_x(c));
            m.cylinder(0.14, 0.12, 0.03, 2, 8, Paint::steel(0x5a4a3a, 0.5), along_x(c + Vec3::X * 0.75));
            m.cylinder(0.05, 0.02, 0.0, 1, 8, Paint::glow(0x60b0ff, 3.0), along_x(c + Vec3::X * 0.9));
            m.cylinder(0.16, 0.32, 0.12, 3, 10, Paint::painted(0xb02a20, 0.4), along_x(c + Vec3::new(-0.35, -0.05, 0.12)));
            m.tube(&[c + Vec3::new(-0.1, 0.0, 0.0), c + Vec3::new(0.0, -0.08, 0.1), c + Vec3::new(0.2, -0.05, 0.05)], 0.035, 6, Paint::rubber(0x24211f));
            for x in [0.2f32, 0.7] {
                m.bevel_box(Vec3::new(0.04, 0.16, 0.12), 0.015, 1, trim, at(c + Vec3::new(x, 0.12, -0.08)));
            }
        }
        PartKind::ShieldGenerator => {
            // Turret frame (-0.5, 1.0, 0): an emitter dome with glowing rings.
            let c = Vec3::new(-0.5, 0.8, 0.0);
            m.cylinder(0.38, 0.08, 0.03, 2, 12, trim, at(c));
            for k in 0..3 {
                let y = 0.12 + k as f32 * 0.12;
                m.cylinder(0.3 - k as f32 * 0.06, 0.035, 0.015, 1, 12, Paint::steel(0x8a8070, 0.3), at(c + Vec3::Y * y));
                m.cylinder(0.31 - k as f32 * 0.06, 0.012, 0.0, 1, 12, Paint::glow(0x60c8ff, 3.0), at(c + Vec3::Y * (y + 0.05)));
            }
            m.rock_blob(11, Vec3::splat(0.14), Paint::glow(0x9fe0ff, 4.0), at(c + Vec3::Y * 0.55));
        }
        PartKind::RocketRack => {
            // Turret frame (0.2, 0.6, -1.3): a 2 x 3 pod with red-tipped rockets.
            let c = Vec3::new(0.2, 0.6, -1.3);
            m.bevel_box(Vec3::new(0.8, 0.3, 0.3), 0.05, 2, paint, at(c));
            for row in 0..2 {
                for col in 0..3 {
                    let p = c + Vec3::new(0.8, -0.13 + row as f32 * 0.26, -0.18 + col as f32 * 0.18);
                    m.cylinder(0.075, 0.02, 0.0, 1, 8, Paint::steel(0x141210, 0.9), along_x(p));
                    m.cylinder(0.05, 0.03, 0.0, 1, 8, Paint::painted(0xb02a20, 0.3), along_x(p + Vec3::X * 0.02));
                }
            }
            m.bevel_box(Vec3::new(0.12, 0.2, 0.1), 0.03, 1, trim, at(c + Vec3::new(-0.3, -0.35, 0.25)));
            for x in [-0.5f32, 0.5] {
                m.bevel_box(Vec3::new(0.05, 0.32, 0.32), 0.02, 1, trim, at(c + Vec3::X * x));
            }
        }
        PartKind::SpikedRam => {
            // Hull frame (3.3, -0.35, 0): a beam bristling with steel spikes.
            let c = Vec3::new(3.2, -0.35, 0.0);
            m.bevel_box(Vec3::new(0.18, 0.18, 1.7), 0.05, 2, trim, at(c));
            for k in 0..7 {
                let z = -1.5 + k as f32 * 0.5;
                let cone = [(0.12f32, 0.0f32, Vec2::new(0.9, 0.3)), (0.0, 0.45, Vec2::Y)];
                let xf = Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), c + Vec3::new(0.15, (k % 2) as f32 * 0.12 - 0.06, z));
                m.lathe(&cone, 6, true, Paint::steel(0xb8b4aa, 0.25), xf);
            }
            for z in [-1.2f32, 1.2] {
                m.bevel_box(Vec3::new(0.3, 0.08, 0.08), 0.02, 1, trim, at(c + Vec3::new(-0.3, 0.2, z)));
            }
        }
        PartKind::Mortar => {
            // Hull frame (-1.7, 0.85, -1.35): a stubby tube cocked skywards.
            let c = Vec3::new(-1.7, 0.45, -1.35);
            m.bevel_box(Vec3::new(0.32, 0.06, 0.32), 0.03, 1, trim, at(c));
            let tube = Mat4::from_rotation_translation(Quat::from_rotation_z(0.45), c + Vec3::new(0.1, 0.45, 0.0));
            m.cylinder(0.2, 0.45, 0.04, 2, 10, Paint::steel(0x4a4440, 0.5), tube);
            m.cylinder(0.24, 0.06, 0.02, 1, 10, trim, tube * at(Vec3::Y * 0.4));
            m.cylinder(0.13, 0.01, 0.0, 1, 10, Paint::steel(0x0c0a09, 0.9), tube * at(Vec3::Y * 0.46));
            m.bevel_box(Vec3::new(0.06, 0.25, 0.06), 0.02, 1, trim, at(c + Vec3::new(0.2, 0.25, 0.15)));
        }
        PartKind::Harpoon => {
            // Turret frame (0.3, 0.95, 0.5): a launcher rail, a barbed
            // harpoon and a cable drum.
            let c = Vec3::new(0.3, 0.95, 0.5);
            m.bevel_box(Vec3::new(0.9, 0.06, 0.12), 0.03, 1, trim, at(c - Vec3::Y * 0.08));
            m.cylinder(0.04, 0.75, 0.0, 1, 6, Paint::steel(0x6a6460, 0.4), along_x(c + Vec3::Y * 0.02));
            let tip = [(0.1f32, 0.0f32, Vec2::new(0.6, 0.6)), (0.0, 0.3, Vec2::Y)];
            m.lathe(&tip, 6, true, Paint::steel(0xb8b4aa, 0.25), along_x(c + Vec3::new(0.75, 0.02, 0.0)));
            let drum = Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), c + Vec3::new(-0.6, 0.05, 0.0));
            m.cylinder(0.2, 0.12, 0.03, 1, 10, paint, drum);
            m.torus(0.16, 0.05, 0.2, 10, Paint::rubber(0x24211f), drum);
        }
        PartKind::SmokeLauncher => {
            // Turret frame, both cheeks: three tubes each, angled up and out.
            for side in [-1.0f32, 1.0] {
                let c = Vec3::new(0.9, 0.65, side * 0.95);
                m.bevel_box(Vec3::new(0.16, 0.06, 0.18), 0.02, 1, trim, at(c - Vec3::Y * 0.1));
                for k in 0..3 {
                    let rot = Quat::from_rotation_x(-side * 0.5) * Quat::from_rotation_z(-0.9);
                    m.cylinder(0.055, 0.14, 0.02, 1, 8, Paint::steel(0x4a4a44, 0.5), Mat4::from_rotation_translation(rot, c + Vec3::new(0.0, 0.0, -0.12 + k as f32 * 0.12)));
                }
            }
        }
        PartKind::MineLayer => {
            // Hull frame (-3.2, -0.35, 0): a hazard-striped chute with a
            // stack of mines showing.
            let c = Vec3::new(-3.15, -0.3, 0.0);
            m.bevel_box(Vec3::new(0.2, 0.16, 1.2), 0.04, 2, Paint::with(crate::meshkit::Material::Hazard, 0xffffff, 0.6), at(c));
            for z in [-0.6f32, 0.0, 0.6] {
                m.cylinder(0.2, 0.05, 0.02, 1, 10, Paint::painted(0x4a5038, 0.6), at(c + Vec3::new(-0.05, 0.2, z)));
            }
            m.bevel_box(Vec3::new(0.05, 0.1, 1.1), 0.02, 1, trim, at(c + Vec3::new(-0.22, -0.1, 0.0)));
        }
        PartKind::RepairRig => {
            // Hull frame (1.9, 0.75, -0.8): a jointed arm with a wrench claw
            // and a green work light, on a toolbox.
            let c = Vec3::new(1.9, 0.5, -0.8);
            crate_box(&mut m, c, Vec3::new(0.35, 0.18, 0.28));
            let shoulder = c + Vec3::new(-0.1, 0.25, 0.0);
            let elbow = shoulder + Vec3::new(0.35, 0.45, 0.0);
            let hand = elbow + Vec3::new(0.4, -0.15, 0.0);
            octo_pipe(&mut m, shoulder, elbow, 0.06, paint);
            octo_pipe(&mut m, elbow, hand, 0.05, paint);
            m.rivet(elbow, Vec3::Z, 0.09, bare);
            m.bevel_box(Vec3::new(0.12, 0.04, 0.08), 0.02, 1, Paint::steel(0xb8b4aa, 0.3), at(hand + Vec3::X * 0.1));
            m.bevel_box(Vec3::new(0.06, 0.1, 0.06), 0.02, 1, Paint::steel(0xb8b4aa, 0.3), at(hand + Vec3::new(0.2, 0.05, 0.0)));
            m.cylinder(0.06, 0.03, 0.01, 1, 8, Paint::glow(0x50ff80, 2.5), at(c + Vec3::new(0.2, 0.22, 0.15)));
        }
        // ── The War Rig's modules (hull frame unless noted), 3x scale ──
        PartKind::RigCannons => {
            // Turret frame: a squat armoured house with two long cannons.
            let base = Vec3::new(0.0, 1.2, 0.0);
            m.wedge(Vec3::new(3.2, 1.2, 2.2), 0.18, 2, 0.75, paint, at(base));
            for side in [-1.0f32, 1.0] {
                plate(&mut m, base + Vec3::new(-0.4, 0.0, side * 2.25), Vec3::new(2.2, 0.8, 0.06), 2, side, paint, rivet);
            }
            m.cylinder(1.4, 0.14, 0.05, 2, 16, trim, at(base + Vec3::new(-0.8, 1.25, 0.0)));
            m.bevel_box(Vec3::new(0.35, 0.9, 1.5), 0.1, 2, trim, at(Vec3::new(3.3, 1.3, 0.0)));
            for z in [-0.8f32, 0.8] {
                let c = Vec3::new(5.0, 1.3, z);
                m.cylinder(0.3, 2.4, 0.04, 1, 14, paint, along_x(c));
                m.cylinder(0.4, 0.18, 0.05, 2, 14, trim, along_x(c - Vec3::X * 1.6));
                m.cylinder(0.46, 0.4, 0.08, 2, 14, trim, along_x(c + Vec3::X * 2.4));
                bolt_ring(&mut m, c + Vec3::X * 2.4, 0.46, 10, 0.06);
                m.cylinder(0.26, 0.01, 0.0, 1, 12, Paint::steel(0x0c0a09, 0.9), along_x(c + Vec3::X * 2.81));
            }
        }
        PartKind::RigRockets => {
            // Two big launcher pods on the rear deck, 3 x 4 tubes each.
            for side in [-1.0f32, 1.0] {
                let c = Vec3::new(-4.6, 2.2, side * 2.4);
                m.bevel_box(Vec3::new(1.2, 0.8, 0.7), 0.1, 2, paint, at(c));
                for row in 0..3 {
                    for col in 0..4 {
                        let p = c + Vec3::new(1.21, -0.5 + row as f32 * 0.5, -0.5 + col as f32 * 0.33);
                        m.cylinder(0.13, 0.03, 0.0, 1, 8, Paint::steel(0x141210, 0.9), along_x(p));
                        m.cylinder(0.09, 0.05, 0.0, 1, 8, Paint::painted(0xb02a20, 0.3), along_x(p + Vec3::X * 0.02));
                    }
                }
                m.bevel_box(Vec3::new(0.3, 0.5, 0.3), 0.05, 1, trim, at(c - Vec3::Y * 1.0));
                plate(&mut m, c + Vec3::Y * 0.82, Vec3::new(1.1, 0.04, 0.65), 1, 1.0, paint, rivet);
            }
        }
        PartKind::RigFlamer => {
            // A fan of three nozzles in the nose, fed from twin tanks.
            let c = Vec3::new(8.1, 0.3, 0.0);
            m.bevel_box(Vec3::new(0.45, 0.5, 1.0), 0.08, 2, trim, at(c));
            for z in [-0.6f32, 0.0, 0.6] {
                m.cylinder(0.16, 0.5, 0.04, 1, 10, Paint::steel(0x5a4a3a, 0.5), along_x(c + Vec3::new(0.6, 0.1, z)));
                m.cylinder(0.22, 0.12, 0.04, 2, 10, trim, along_x(c + Vec3::new(1.05, 0.1, z)));
                m.cylinder(0.08, 0.02, 0.0, 1, 8, Paint::glow(0x60b0ff, 3.0), along_x(c + Vec3::new(1.18, 0.1, z)));
            }
            for z in [-0.75f32, 0.75] {
                m.cylinder(0.35, 0.6, 0.15, 3, 12, Paint::painted(0xb02a20, 0.4), along_x(c + Vec3::new(-0.6, 0.75, z)));
            }
        }
        PartKind::RigPlateLeft | PartKind::RigPlateRight => {
            // Big overlapping riveted side plates with a dark trim band.
            let side = if kind == PartKind::RigPlateLeft { -1.0f32 } else { 1.0 };
            for k in 0..4 {
                let x = -4.95 + k as f32 * 3.3;
                let c = Vec3::new(x, 0.2 + 0.05 * (k % 2) as f32, side * (3.75 + 0.04 * (k % 2) as f32));
                plate(&mut m, c, Vec3::new(1.7, 1.25, 0.12), 2, side, paint, rivet);
            }
            m.bevel_box(Vec3::new(6.6, 0.12, 0.14), 0.05, 2, trim, at(Vec3::new(0.0, 1.45, side * 3.8)));
            m.bevel_box(Vec3::new(6.6, 0.12, 0.14), 0.05, 2, trim, at(Vec3::new(0.0, -1.05, side * 3.8)));
            m.bevel_box(Vec3::new(1.2, 0.18, 0.01), 0.01, 1, Paint::painted(0xd8ccb0, 0.7), at(Vec3::new(3.0, 0.6, side * 3.92)));
        }
        PartKind::RigPlateFront => {
            // A sloped ram plate over the nose, with a hazard-striped lip.
            let c = Vec3::new(7.7, 0.5, 0.0);
            plate(&mut m, c, Vec3::new(0.18, 1.2, 3.3), 0, 1.0, paint, rivet);
            m.bevel_box(Vec3::new(0.22, 0.18, 3.32), 0.05, 2, Paint::with(Material::Hazard, 0xffffff, 0.6), at(c - Vec3::Y * 1.05));
            for z in [-2.2f32, 0.0, 2.2] {
                m.bevel_box(Vec3::new(0.5, 0.15, 0.15), 0.04, 1, trim, at(c + Vec3::new(-0.5, 0.6, z)));
            }
        }
        PartKind::RigEngineLeft | PartKind::RigEngineRight => {
            // An exposed engine block: finned cylinders, a blower, two stacks.
            let side = if kind == PartKind::RigEngineLeft { -1.0f32 } else { 1.0 };
            let c = Vec3::new(-6.5, 1.9, side * 1.6);
            m.bevel_box(Vec3::new(1.0, 0.55, 1.1), 0.1, 2, Paint::steel(0x5a5650, 0.4), at(c - Vec3::Y * 0.2));
            for k in 0..6 {
                m.bevel_box(Vec3::new(0.05, 0.6, 1.15), 0.02, 1, trim, at(c + Vec3::new(-0.8 + k as f32 * 0.32, -0.15, 0.0)));
            }
            m.cylinder(0.4, 0.3, 0.08, 2, 12, Paint::steel(0x8a8070, 0.3), at(c + Vec3::Y * 0.6));
            for z in [-0.6f32, 0.6] {
                crate::tank::octo_pipe(&mut m, c + Vec3::new(-0.6, 0.3, z), c + Vec3::new(-0.8, 2.6, z), 0.17, Paint::steel(0x2b2725, 0.75));
                crate::tank::octo_pipe(&mut m, c + Vec3::new(-0.8, 2.5, z), c + Vec3::new(-1.3, 3.2, z), 0.17, Paint::steel(0x2b2725, 0.75));
            }
        }
        PartKind::RigMast => {
            // A tall lattice mast with a sensor head and a red lamp.
            let b = Vec3::new(-2.5, 2.0, -2.6);
            m.bevel_box(Vec3::new(0.45, 0.12, 0.45), 0.04, 1, trim, at(b));
            for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                crate::tank::octo_pipe(&mut m, b + Vec3::new(sx * 0.3, 0.0, sz * 0.3), b + Vec3::new(sx * 0.12, 4.6, sz * 0.12), 0.05, trim);
            }
            for k in 0..6 {
                let y = 0.6 + k as f32 * 0.7;
                let s = 0.3 - 0.18 * (y / 4.6);
                m.bevel_box(Vec3::new(s, 0.03, s), 0.01, 1, trim, at(b + Vec3::Y * y));
            }
            let head = Vec3::new(-2.5, 7.0, -2.6);
            m.bevel_box(Vec3::new(0.5, 0.4, 0.5), 0.08, 2, paint, at(head));
            m.cylinder(0.25, 0.05, 0.02, 1, 12, Paint::glow(0x5fc8ff, 2.0), along_x(head + Vec3::X * 0.51));
            m.cylinder(0.1, 0.12, 0.05, 2, 8, Paint::glow(0xff3020, 3.0), at(head + Vec3::Y * 0.55));
        }
        // Guns are drawn with `gun_mesh`.
        k if is_gun(k) => {}
        // Combo and rare parts without a dedicated model yet: riveted
        // plated blocks on their hit boxes, so they read and can be hit.
        k => {
            for hb in arena_core::part_def(k).hitboxes {
                let h = hb.half;
                let axis = if h.x <= h.y && h.x <= h.z { 0 } else if h.y <= h.z { 1 } else { 2 };
                plate(&mut m, hb.center, h, axis, 1.0, paint, rivet);
                m.bevel_box(h * 0.6, 0.03, 1, trim, at(hb.center - Vec3::Y * h.y * 0.3));
            }
            let _ = bare;
        }
    }
    m
}

/// Every part kind, for the lineup.
pub const ALL_PARTS: [PartKind; 24] = [
    PartKind::StubGun,
    PartKind::HeavyCannon,
    PartKind::BigBore,
    PartKind::TwinBarrel,
    PartKind::LongGun,
    PartKind::SensorMast,
    PartKind::Coilgun,
    PartKind::TeslaSpire,
    PartKind::FlankPlates,
    PartKind::Bulwark,
    PartKind::SpacedArmour,
    PartKind::Nitro,
    PartKind::Afterburner,
    PartKind::TwinEngines,
    PartKind::Flamethrower,
    PartKind::ShieldGenerator,
    PartKind::RocketRack,
    PartKind::SpikedRam,
    PartKind::Mortar,
    PartKind::Harpoon,
    PartKind::SmokeLauncher,
    PartKind::MineLayer,
    PartKind::RepairRig,
    PartKind::RigCannons,
];

pub fn is_gun(kind: PartKind) -> bool {
    matches!(kind, PartKind::StubGun | PartKind::HeavyCannon | PartKind::BigBore | PartKind::TwinBarrel | PartKind::LongGun)
}
