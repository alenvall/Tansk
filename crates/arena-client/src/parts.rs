//! Upgrade part meshes, one distinct silhouette per `PartKind`, in the frame
//! the simulation mounts it in (gun parts in the gun frame with the barrel
//! along +X; hull parts in the hull frame, origin at the hull centre about
//! 0.95 m above the ground; turret parts in the turret frame). Shapes follow
//! the part hit boxes in arena-core, so what you see is what you can hit.

use arena_core::PartKind;
use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};
use crate::tank::{ball, bare_paint, panel, pipe, ring_x, team_paint, toon_crate, trim_paint};

const BARE: u32 = 0xa8a398;
/// The dark inside of a bore or a tube.
const BORE: u32 = 0x0c0a09;
const ROCKET_RED: u32 = 0xd03a26;

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}
fn along_x(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p)
}
fn along_z(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), p)
}

fn bolt_ring(m: &mut MeshBuilder, centre: Vec3, r: f32, n: u32, size: f32) {
    for k in 0..n {
        let a = (k as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
        let d = Vec3::new(0.0, a.sin(), a.cos());
        m.rivet(centre + d * r, d, size, Paint::steel(BARE, 0.35));
    }
}

/// A dark bore disc facing +X at `c`.
fn bore(m: &mut MeshBuilder, c: Vec3, r: f32) {
    m.cylinder(r, 0.012, 0.0, 1, 16, Paint::with(Material::BareSteel, BORE, 0.9), along_x(c));
}

/// A fat stubby barrel along +X from `x0` to `x1` at height/side `yz`: a
/// dark collar at the breech and a big rounded muzzle ring with a dark bore.
fn barrel(m: &mut MeshBuilder, team: u32, yz: Vec3, x0: f32, x1: f32, r: f32, muzzle_r: f32, muzzle_len: f32) {
    let paint = team_paint(team);
    let trim = trim_paint();
    let x_end = x1 - muzzle_len * 0.8;
    let len = x_end - x0;
    m.cylinder(r, len * 0.5, r * 0.3, 2, 20, paint, along_x(yz + Vec3::X * (x0 + len * 0.5)));
    ring_x(m, yz + Vec3::X * (x0 + 0.14), r * 1.38, 0.1, trim);
    let mz = yz + Vec3::X * (x1 - muzzle_len * 0.5);
    ring_x(m, mz, muzzle_r, muzzle_len * 0.5, trim);
    bore(m, yz + Vec3::X * (x1 + 0.004), muzzle_r * 0.6);
}

/// The mantlet at the gun origin: a fat rounded dark block, `h` half
/// height, with two big bolts on its face.
fn mantlet(m: &mut MeshBuilder, h: f32) {
    let trim = trim_paint();
    m.bevel_box(Vec3::new(0.2, h, h * 1.15), h * 0.45, 3, trim, at(Vec3::new(-0.02, 0.0, 0.0)));
    for z in [-1.0f32, 1.0] {
        m.rivet(Vec3::new(0.18, h * 0.45, z * h * 0.78), Vec3::X, 0.07, bare_paint());
    }
}

/// The gun for a gun part (gun frame). `None` kinds fall back to the stub.
pub fn gun_mesh(kind: PartKind, team: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let trim = trim_paint();
    match kind {
        PartKind::HeavyCannon => {
            mantlet(&mut m, 0.42);
            barrel(&mut m, team, Vec3::ZERO, 0.25, 3.2, 0.19, 0.33, 0.55);
            ring_x(&mut m, Vec3::X * 1.75, 0.235, 0.08, trim);
        }
        PartKind::BigBore => {
            mantlet(&mut m, 0.5);
            // A short fat tube with a huge reinforced bell.
            barrel(&mut m, team, Vec3::ZERO, 0.3, 2.8, 0.33, 0.52, 0.72);
            for x in [1.0f32, 1.6] {
                ring_x(&mut m, Vec3::X * x, 0.385, 0.08, trim);
            }
            bore(&mut m, Vec3::X * 2.806, 0.38);
        }
        PartKind::TwinBarrel => {
            mantlet(&mut m, 0.46);
            for z in [-0.28f32, 0.28] {
                barrel(&mut m, team, Vec3::Z * z, 0.2, 3.0, 0.13, 0.2, 0.38);
            }
            // Clamp blocks tying them together.
            for x in [1.15f32, 2.05] {
                m.bevel_box(Vec3::new(0.1, 0.17, 0.48), 0.08, 3, trim, at(Vec3::X * x));
                m.rivet(Vec3::new(x, 0.17, 0.0), Vec3::Y, 0.06, bare_paint());
            }
        }
        PartKind::LongGun => {
            mantlet(&mut m, 0.38);
            // A fat breech sleeve, the long thin tube, a slotted brake.
            m.cylinder(0.17, 0.65, 0.06, 2, 20, team_paint(team), along_x(Vec3::X * 0.9));
            barrel(&mut m, team, Vec3::ZERO, 0.25, 5.2, 0.12, 0.2, 0.3);
            m.bevel_box(Vec3::new(0.26, 0.15, 0.24), 0.1, 3, trim, at(Vec3::X * 5.12));
            for z in [-1.0f32, 1.0] {
                m.bevel_box(Vec3::new(0.05, 0.08, 0.01), 0.01, 1, Paint::with(Material::BareSteel, BORE, 0.9), at(Vec3::new(5.12, 0.0, z * 0.245)));
            }
            bore(&mut m, Vec3::X * 5.385, 0.09);
            // The big scope on the left with a glinting lens and a round sight.
            let sc = Vec3::new(0.4, 0.4, -0.36);
            m.cylinder(0.14, 0.45, 0.06, 2, 18, Paint::steel(0x4a4642, 0.4), along_x(sc));
            ring_x(&mut m, sc + Vec3::X * 0.47, 0.19, 0.08, trim);
            m.cylinder(0.15, 0.006, 0.0, 1, 16, Paint::glow(0x9fd8ff, 1.6), along_x(sc + Vec3::X * 0.555));
            ring_x(&mut m, sc - Vec3::X * 0.44, 0.17, 0.06, trim);
            m.bevel_box(Vec3::new(0.12, 0.13, 0.07), 0.05, 2, trim, at(Vec3::new(0.4, 0.24, -0.3)));
            let sight = Vec3::new(0.05, 0.62, -0.36);
            m.cylinder(0.04, 0.08, 0.02, 1, 8, trim, at(sight - Vec3::Y * 0.12));
            m.cylinder(0.17, 0.035, 0.03, 2, 18, trim, along_x(sight + Vec3::Y * 0.06));
            m.cylinder(0.11, 0.04, 0.02, 1, 16, Paint::steel(BARE, 0.3), along_x(sight + Vec3::Y * 0.06));
        }
        _ => {
            // Stub gun.
            mantlet(&mut m, 0.34);
            barrel(&mut m, team, Vec3::ZERO, 0.2, 2.0, 0.15, 0.26, 0.42);
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
    let paint = team_paint(team);
    let rivet = Paint::team(team, 0.15);
    let trim = trim_paint();
    let bare = bare_paint();
    match kind {
        PartKind::SensorMast => {
            // Hull frame (-2.4, 2.0, -1.1): a fat pole and a sensor head
            // with a big lens, a dish and a red lamp.
            let b = Vec3::new(-2.4, 0.55, -1.1);
            m.cylinder(0.24, 0.07, 0.05, 2, 14, trim, at(b));
            pipe(&mut m, b, b + Vec3::Y * 3.0, 0.1, trim);
            for k in 0..3 {
                m.cylinder(0.15, 0.05, 0.035, 2, 12, bare, at(b + Vec3::Y * (0.7 + k as f32 * 0.8)));
            }
            let head = Vec3::new(-2.4, 3.7, -1.1);
            m.bevel_box(Vec3::new(0.32, 0.2, 0.32), 0.13, 3, paint, at(head));
            ring_x(&mut m, head + Vec3::X * 0.33, 0.15, 0.05, trim);
            m.cylinder(0.11, 0.006, 0.0, 1, 14, Paint::glow(0x5fc8ff, 2.0), along_x(head + Vec3::X * 0.385));
            ball(&mut m, head + Vec3::Y * 0.26, 0.08, Paint::glow(0xff3020, 2.5));
            let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
            let dish = [(0.0f32, 0.0f32, Vec2::NEG_Y), (0.2, 0.03, n(-0.2, -1.0)), (0.36, 0.14, n(-0.4, -0.9)), (0.38, 0.17, Vec2::Y), (0.2, 0.08, n(0.3, 1.0)), (0.0, 0.06, Vec2::Y)];
            m.lathe(&dish, 18, false, Paint::painted(0xd8d2c4, 0.3), Mat4::from_rotation_translation(Quat::from_rotation_z(-1.2), head + Vec3::new(-0.12, 0.38, 0.0)));
        }
        PartKind::Coilgun => {
            // Turret frame (0.5, 0.55, 1.15): a rail with fat glowing coils,
            // hung on the turret's flank.
            let c = Vec3::new(0.5, 0.55, 1.38);
            m.bevel_box(Vec3::new(1.2, 0.1, 0.16), 0.08, 3, trim, at(c - Vec3::Y * 0.14));
            m.cylinder(0.06, 1.2, 0.03, 2, 12, bare, along_x(c));
            for k in 0..5 {
                let x = c.x - 0.88 + k as f32 * 0.44;
                m.cylinder(0.2, 0.08, 0.06, 3, 16, Paint::steel(0xb06a30, 0.3), along_x(Vec3::new(x, c.y, c.z)));
                m.cylinder(0.205, 0.022, 0.0, 1, 16, Paint::glow(0x40a8ff, 3.0), along_x(Vec3::new(x + 0.11, c.y, c.z)));
            }
            m.bevel_box(Vec3::new(0.22, 0.18, 0.16), 0.1, 3, paint, at(Vec3::new(c.x - 1.22, c.y, c.z)));
            ball(&mut m, Vec3::new(c.x + 1.25, c.y, c.z), 0.12, Paint::glow(0x80d0ff, 3.0));
        }
        PartKind::TeslaSpire => {
            // Hull frame (-2.4, 1.2, 1.1): fat stacked insulators and a ball
            // with a glowing ring.
            let b = Vec3::new(-2.4, 0.55, 1.1);
            m.cylinder(0.3, 0.07, 0.05, 2, 14, trim, at(b));
            pipe(&mut m, b, b + Vec3::Y * 1.4, 0.1, trim);
            for k in 0..4 {
                m.cylinder(0.26 - k as f32 * 0.025, 0.07, 0.065, 3, 16, Paint::matte(0xe8e0d0), at(b + Vec3::Y * (0.3 + k as f32 * 0.3)));
            }
            let c = Vec3::new(-2.4, 2.25, 1.1);
            ball(&mut m, c, 0.34, Paint::steel(0xc0b8a0, 0.25));
            m.torus(0.36, 0.035, 0.05, 20, Paint::glow(0x80c8ff, 3.5), at(c));
            ball(&mut m, c + Vec3::Y * 0.36, 0.1, Paint::glow(0x9fe0ff, 4.0));
        }
        PartKind::FlankPlates => {
            for side in [-1.0f32, 1.0] {
                for k in 0..3 {
                    let x = -1.72 + k as f32 * 1.72;
                    let lift = 0.04 * (k % 2) as f32;
                    panel(&mut m, Vec3::new(x, 0.1 + lift, side * (1.94 + lift)), Vec3::new(0.86, 0.5, 0.08), 2, side, paint, rivet);
                }
                for x in [-2.0f32, 0.0, 2.0] {
                    m.bevel_box(Vec3::new(0.1, 0.08, 0.1), 0.06, 2, trim, at(Vec3::new(x, 0.52, side * 1.86)));
                }
            }
        }
        PartKind::Bulwark => {
            // A thick front slab and a plow with big teeth below it.
            panel(&mut m, Vec3::new(3.2, 0.1, 0.0), Vec3::new(0.17, 0.74, 1.86), 0, 1.0, paint, rivet);
            for z in [-1.2f32, 0.0, 1.2] {
                m.bevel_box(Vec3::new(0.3, 0.1, 0.1), 0.07, 2, trim, at(Vec3::new(2.95, 0.45, z)));
            }
            m.wedge(Vec3::new(0.26, 0.26, 1.9), 0.18, 3, 0.2, Paint::steel(0x6a6058, 0.45), at(Vec3::new(3.5, -0.55, 0.0)));
            for k in 0..5 {
                let z = -1.5 + k as f32 * 0.75;
                m.wedge(Vec3::new(0.2, 0.12, 0.17), 0.08, 2, 0.15, bare, at(Vec3::new(3.88, -0.7, z)));
            }
        }
        PartKind::SpacedArmour => {
            // Overlapping fat plates on brackets, all round.
            for side in [-1.0f32, 1.0] {
                for k in 0..3 {
                    let x = -2.0 + k as f32 * 2.0;
                    let tilt = Quat::from_rotation_y((k as f32 - 1.0) * 0.04);
                    let lift = 0.04 * (k % 2) as f32;
                    let c = Vec3::new(x, -0.15 + lift, side * (2.06 + lift));
                    let mut pm = MeshBuilder::new();
                    panel(&mut pm, Vec3::ZERO, Vec3::new(1.06, 0.5, 0.07), 2, side, paint, rivet);
                    pm.transform_from(0, Mat4::from_rotation_translation(tilt, c));
                    append(&mut m, pm);
                    m.bevel_box(Vec3::new(0.08, 0.07, 0.2), 0.05, 2, trim, at(Vec3::new(x, 0.32, side * 1.9)));
                }
            }
            for sx in [-1.0f32, 1.0] {
                panel(&mut m, Vec3::new(sx * 3.15, 0.1, 0.0), Vec3::new(0.08, 0.4, 1.58), 0, sx, paint, rivet);
            }
        }
        PartKind::Nitro => {
            // Twin fat blue bottles on the rear plate, white bands, red valves.
            for z in [-0.45f32, 0.45] {
                let c = Vec3::new(-3.2, 0.2, z);
                m.cylinder(0.23, 0.42, 0.2, 3, 18, Paint::painted(0x2f6ad0, 0.25), along_z(c));
                m.cylinder(0.235, 0.06, 0.02, 1, 18, Paint::painted(0xf2ece0, 0.25), along_z(c + Vec3::Z * z.signum() * -0.12));
                m.cylinder(0.08, 0.07, 0.04, 2, 10, Paint::painted(0xd02a20, 0.25), along_z(c + Vec3::Z * z.signum() * 0.47));
            }
            for z in [-0.75f32, 0.0, 0.75] {
                m.bevel_box(Vec3::new(0.2, 0.06, 0.06), 0.04, 2, trim, at(Vec3::new(-3.1, 0.2, z)));
            }
        }
        PartKind::Afterburner => {
            let c = Vec3::new(-2.5, 0.8, 0.0);
            m.bevel_box(Vec3::new(0.5, 0.27, 0.42), 0.2, 3, paint, at(c));
            let nz = c - Vec3::X * 0.68;
            m.cylinder(0.32, 0.26, 0.1, 3, 20, Paint::steel(0x4a4440, 0.45), along_x(nz));
            ring_x(&mut m, nz - Vec3::X * 0.24, 0.36, 0.06, trim);
            m.cylinder(0.24, 0.01, 0.0, 1, 16, Paint::glow(0xff8a30, 2.5), along_x(nz - Vec3::X * 0.31));
            bolt_ring(&mut m, c - Vec3::X * 0.46, 0.33, 4, 0.06);
        }
        PartKind::TwinEngines => {
            for z in [-0.45f32, 0.45] {
                let c = Vec3::new(-2.4, 0.8, z);
                m.bevel_box(Vec3::new(0.46, 0.3, 0.3), 0.16, 3, Paint::steel(0x6a6560, 0.4), at(c));
                for k in 0..3 {
                    m.bevel_box(Vec3::new(0.05, 0.32, 0.32), 0.04, 2, trim, at(c + Vec3::X * (-0.25 + k as f32 * 0.25)));
                }
                let foot = c + Vec3::new(-0.2, 0.25, 0.0);
                let top = c + Vec3::new(-0.35, 1.05, z * 0.3);
                pipe(&mut m, foot, top, 0.1, Paint::steel(0x2b2725, 0.75));
                m.cylinder(0.13, 0.05, 0.03, 2, 12, trim, Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Y, (top - foot).normalize()), top));
            }
            toon_crate(&mut m, Vec3::new(-1.95, 1.27, 0.0), Vec3::new(0.24, 0.16, 0.22));
        }
        PartKind::Flamethrower => {
            // Gun frame (1.1, -0.3, 0.32): a fat nozzle under the barrel,
            // its red fuel bottle and feed hose, a pilot light at the tip.
            let c = Vec3::new(1.1, -0.3, 0.32);
            m.cylinder(0.1, 0.7, 0.04, 2, 12, trim, along_x(c));
            ring_x(&mut m, c + Vec3::X * 0.75, 0.16, 0.12, Paint::steel(0x5a4a3a, 0.45));
            m.cylinder(0.06, 0.01, 0.0, 1, 10, Paint::glow(0x60b0ff, 3.0), along_x(c + Vec3::X * 0.875));
            m.cylinder(0.17, 0.3, 0.15, 3, 14, Paint::painted(0xd02a20, 0.3), along_x(c + Vec3::new(-0.35, -0.05, 0.13)));
            m.tube(&[c + Vec3::new(-0.1, 0.0, 0.0), c + Vec3::new(0.0, -0.08, 0.1), c + Vec3::new(0.2, -0.05, 0.05)], 0.045, 8, Paint::rubber(0x24211f));
            for x in [0.2f32, 0.7] {
                m.bevel_box(Vec3::new(0.05, 0.16, 0.12), 0.04, 2, trim, at(c + Vec3::new(x, 0.12, -0.08)));
            }
        }
        PartKind::ShieldGenerator => {
            // Turret frame (-0.5, 1.0, 0): an emitter on the turret roof, fat
            // stacked rings and a glowing orb.
            let c = Vec3::new(-0.5, 1.1, 0.0);
            m.cylinder(0.38, 0.08, 0.06, 2, 20, trim, at(c));
            for k in 0..2 {
                let y = 0.16 + k as f32 * 0.14;
                m.cylinder(0.3 - k as f32 * 0.07, 0.05, 0.04, 3, 18, Paint::steel(0x8a8070, 0.3), at(c + Vec3::Y * y));
                m.torus(0.31 - k as f32 * 0.07, 0.02, 0.03, 18, Paint::glow(0x60c8ff, 3.0), at(c + Vec3::Y * (y + 0.06)));
            }
            ball(&mut m, c + Vec3::Y * 0.52, 0.17, Paint::glow(0x9fe0ff, 4.0));
        }
        PartKind::RocketRack => {
            // Turret frame (0.2, 0.6, -1.3): a rounded 2 x 3 pod of
            // red-tipped rockets.
            let c = Vec3::new(0.2, 0.6, -1.42);
            m.bevel_box(Vec3::new(0.8, 0.3, 0.3), 0.2, 3, paint, at(c));
            for row in 0..2 {
                for col in 0..3 {
                    let p = c + Vec3::new(0.78, -0.13 + row as f32 * 0.26, -0.18 + col as f32 * 0.18);
                    m.cylinder(0.08, 0.02, 0.0, 1, 10, Paint::with(Material::BareSteel, BORE, 0.9), along_x(p));
                    m.cylinder(0.06, 0.06, 0.055, 3, 10, Paint::painted(ROCKET_RED, 0.25), along_x(p + Vec3::X * 0.03));
                }
            }
            m.bevel_box(Vec3::new(0.12, 0.2, 0.1), 0.06, 2, trim, at(c + Vec3::new(-0.3, -0.35, 0.25)));
            for x in [-0.45f32, 0.45] {
                m.bevel_box(Vec3::new(0.06, 0.32, 0.32), 0.05, 2, trim, at(c + Vec3::X * x));
            }
        }
        PartKind::SpikedRam => {
            // Hull frame (3.3, -0.35, 0): a beam bristling with big spikes.
            let c = Vec3::new(3.2, -0.35, 0.0);
            m.bevel_box(Vec3::new(0.2, 0.2, 1.7), 0.15, 3, trim, at(c));
            let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
            let cone = [(0.17f32, 0.0f32, n(0.9, 0.3)), (0.08, 0.32, n(0.9, 0.35)), (0.0, 0.5, Vec2::Y)];
            for k in 0..5 {
                let z = -1.4 + k as f32 * 0.7;
                let xf = Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), c + Vec3::new(0.16, (k % 2) as f32 * 0.1 - 0.05, z));
                m.lathe(&cone, 10, true, Paint::steel(0xc0bcb2, 0.25), xf);
            }
            for z in [-1.2f32, 1.2] {
                m.bevel_box(Vec3::new(0.3, 0.09, 0.09), 0.06, 2, trim, at(c + Vec3::new(-0.3, 0.2, z)));
            }
        }
        PartKind::Mortar => {
            // Hull frame (-1.7, 0.85, -1.35): a fat stubby tube cocked skywards.
            let c = Vec3::new(-1.7, 0.55, -1.35);
            m.cylinder(0.3, 0.06, 0.05, 2, 14, trim, at(c));
            let tube = Mat4::from_rotation_translation(Quat::from_rotation_z(0.45), c + Vec3::new(0.1, 0.42, 0.0));
            m.cylinder(0.21, 0.4, 0.08, 2, 16, Paint::steel(0x4a4440, 0.45), tube);
            m.cylinder(0.27, 0.08, 0.06, 3, 16, trim, tube * at(Vec3::Y * 0.36));
            m.cylinder(0.15, 0.01, 0.0, 1, 14, Paint::with(Material::BareSteel, BORE, 0.9), tube * at(Vec3::Y * 0.445));
            m.bevel_box(Vec3::new(0.07, 0.24, 0.07), 0.05, 2, trim, at(c + Vec3::new(0.2, 0.22, 0.15)));
        }
        PartKind::Harpoon => {
            // Turret frame (0.3, 0.95, 0.5): a launcher rail on the roof, a
            // barbed harpoon and a cable drum.
            let c = Vec3::new(0.3, 1.2, 0.5);
            m.bevel_box(Vec3::new(0.9, 0.07, 0.13), 0.05, 2, trim, at(c - Vec3::Y * 0.09));
            m.cylinder(0.05, 0.75, 0.02, 1, 10, Paint::steel(0x6a6460, 0.4), along_x(c + Vec3::Y * 0.03));
            let tip = [(0.13f32, 0.0f32, Vec2::new(0.6, 0.6)), (0.0, 0.34, Vec2::Y)];
            m.lathe(&tip, 8, true, Paint::steel(0xc0bcb2, 0.25), along_x(c + Vec3::new(0.75, 0.03, 0.0)));
            let drum = along_z(c + Vec3::new(-0.6, 0.06, 0.0));
            m.cylinder(0.22, 0.13, 0.06, 2, 16, paint, drum);
            m.torus(0.17, 0.06, 0.2, 14, Paint::rubber(0x24211f), drum);
        }
        PartKind::SmokeLauncher => {
            // Turret frame, both cheeks: two fat tubes each, angled up and out.
            for side in [-1.0f32, 1.0] {
                let c = Vec3::new(0.9, 0.95, side * 0.95);
                m.bevel_box(Vec3::new(0.17, 0.06, 0.18), 0.05, 2, trim, at(c - Vec3::Y * 0.1));
                let rot = Quat::from_rotation_x(-side * 0.5) * Quat::from_rotation_z(-0.9);
                for k in 0..2 {
                    let p = c + Vec3::new(0.0, 0.0, -0.09 + k as f32 * 0.18);
                    m.cylinder(0.075, 0.14, 0.04, 2, 12, Paint::steel(0x4a4a44, 0.45), Mat4::from_rotation_translation(rot, p));
                    m.cylinder(0.05, 0.01, 0.0, 1, 10, Paint::with(Material::BareSteel, BORE, 0.9), Mat4::from_rotation_translation(rot, p + rot * Vec3::Y * 0.142));
                }
            }
        }
        PartKind::MineLayer => {
            // Hull frame (-3.2, -0.35, 0): a hazard-striped chute with a
            // stack of fat mines showing.
            let c = Vec3::new(-3.15, -0.3, 0.0);
            m.bevel_box(Vec3::new(0.2, 0.16, 1.2), 0.1, 3, Paint::with(Material::Hazard, 0xffffff, 0.6), at(c));
            for z in [-0.6f32, 0.0, 0.6] {
                m.cylinder(0.22, 0.06, 0.05, 2, 14, Paint::painted(0x4a5038, 0.5), at(c + Vec3::new(-0.05, 0.2, z)));
                m.cylinder(0.06, 0.03, 0.02, 1, 8, Paint::painted(0xd02a20, 0.3), at(c + Vec3::new(-0.05, 0.28, z)));
            }
            m.bevel_box(Vec3::new(0.06, 0.1, 1.1), 0.05, 2, trim, at(c + Vec3::new(-0.22, -0.1, 0.0)));
        }
        PartKind::RepairRig => {
            // Hull frame (1.9, 0.75, -0.8): a jointed arm with a big wrench
            // and a green work light, on a red toolbox.
            let c = Vec3::new(1.9, 0.62, -0.8);
            m.bevel_box(Vec3::new(0.36, 0.17, 0.28), 0.1, 3, Paint::painted(0xd02a20, 0.35), at(c));
            m.bevel_box(Vec3::new(0.14, 0.035, 0.03), 0.02, 1, trim, at(c + Vec3::new(0.0, 0.2, 0.0)));
            let shoulder = c + Vec3::new(-0.15, 0.2, 0.1);
            let elbow = shoulder + Vec3::new(0.3, 0.45, 0.0);
            let hand = elbow + Vec3::new(0.38, -0.12, 0.0);
            pipe(&mut m, shoulder, elbow, 0.07, paint);
            pipe(&mut m, elbow, hand, 0.06, paint);
            ball(&mut m, elbow, 0.1, bare);
            // The wrench: a handle and an open jaw.
            let w = Paint::steel(0xb8b4aa, 0.3);
            m.bevel_box(Vec3::new(0.16, 0.045, 0.045), 0.03, 2, w, at(hand + Vec3::X * 0.12));
            for y in [-0.07f32, 0.07] {
                m.bevel_box(Vec3::new(0.06, 0.035, 0.05), 0.025, 2, w, at(hand + Vec3::new(0.32, y, 0.0)));
            }
            m.bevel_box(Vec3::new(0.04, 0.1, 0.05), 0.025, 2, w, at(hand + Vec3::new(0.26, 0.0, 0.0)));
            ball(&mut m, c + Vec3::new(0.22, 0.24, -0.12), 0.07, Paint::glow(0x50ff80, 2.5));
        }
        // ── The War Rig's modules (hull frame unless noted), 3x scale ──
        PartKind::RigCannons => {
            // Turret frame: a squat armoured house with two long cannons.
            let base = Vec3::new(0.0, 1.2, 0.0);
            m.wedge(Vec3::new(3.2, 1.2, 2.2), 0.6, 4, 0.75, paint, at(base));
            for side in [-1.0f32, 1.0] {
                panel(&mut m, base + Vec3::new(-0.4, 0.0, side * 2.25), Vec3::new(2.2, 0.8, 0.14), 2, side, paint, rivet);
            }
            m.cylinder(1.4, 0.16, 0.12, 3, 24, trim, at(base + Vec3::new(-0.8, 1.2, 0.0)));
            m.bevel_box(Vec3::new(0.4, 0.9, 1.5), 0.35, 3, trim, at(Vec3::new(3.3, 1.3, 0.0)));
            for z in [-0.8f32, 0.8] {
                let c = Vec3::new(5.0, 1.3, z);
                m.cylinder(0.36, 2.4, 0.1, 2, 20, paint, along_x(c));
                ring_x(&mut m, c - Vec3::X * 1.6, 0.48, 0.2, trim);
                ring_x(&mut m, c + Vec3::X * 2.35, 0.62, 0.45, trim);
                bore(&mut m, c + Vec3::X * 2.805, 0.38);
            }
        }
        PartKind::RigRockets => {
            // Two big launcher pods on the rear deck, 3 x 4 tubes each.
            for side in [-1.0f32, 1.0] {
                let c = Vec3::new(-4.6, 2.2, side * 2.4);
                m.bevel_box(Vec3::new(1.2, 0.8, 0.7), 0.4, 3, paint, at(c));
                for row in 0..2 {
                    for col in 0..3 {
                        let p = c + Vec3::new(1.2, -0.3 + row as f32 * 0.6, -0.4 + col as f32 * 0.4);
                        m.cylinder(0.17, 0.03, 0.0, 1, 12, Paint::with(Material::BareSteel, BORE, 0.9), along_x(p));
                        m.cylinder(0.13, 0.1, 0.1, 3, 12, Paint::painted(ROCKET_RED, 0.25), along_x(p + Vec3::X * 0.06));
                    }
                }
                m.bevel_box(Vec3::new(0.3, 0.5, 0.3), 0.2, 3, trim, at(c - Vec3::Y * 1.0));
            }
        }
        PartKind::RigFlamer => {
            // A fan of three nozzles in the nose, fed from twin tanks.
            let c = Vec3::new(8.1, 0.3, 0.0);
            m.bevel_box(Vec3::new(0.45, 0.5, 1.0), 0.3, 3, trim, at(c));
            for z in [-0.6f32, 0.0, 0.6] {
                m.cylinder(0.18, 0.5, 0.06, 2, 14, Paint::steel(0x5a4a3a, 0.45), along_x(c + Vec3::new(0.6, 0.1, z)));
                ring_x(&mut m, c + Vec3::new(1.05, 0.1, z), 0.26, 0.14, trim);
                m.cylinder(0.1, 0.02, 0.0, 1, 12, Paint::glow(0x60b0ff, 3.0), along_x(c + Vec3::new(1.2, 0.1, z)));
            }
            for z in [-0.75f32, 0.75] {
                m.cylinder(0.38, 0.6, 0.3, 3, 18, Paint::painted(0xd02a20, 0.3), along_x(c + Vec3::new(-0.6, 0.75, z)));
            }
        }
        PartKind::RigPlateLeft | PartKind::RigPlateRight => {
            // Big overlapping riveted side plates with a dark trim band.
            let side = if kind == PartKind::RigPlateLeft { -1.0f32 } else { 1.0 };
            for k in 0..4 {
                let x = -4.95 + k as f32 * 3.3;
                let c = Vec3::new(x, 0.2 + 0.05 * (k % 2) as f32, side * (3.75 + 0.04 * (k % 2) as f32));
                panel(&mut m, c, Vec3::new(1.7, 1.25, 0.16), 2, side, paint, rivet);
            }
            m.bevel_box(Vec3::new(6.6, 0.14, 0.16), 0.13, 3, trim, at(Vec3::new(0.0, 1.45, side * 3.8)));
            m.bevel_box(Vec3::new(6.6, 0.14, 0.16), 0.13, 3, trim, at(Vec3::new(0.0, -1.05, side * 3.8)));
            m.bevel_box(Vec3::new(1.2, 0.2, 0.01), 0.01, 1, Paint::painted(0xd8ccb0, 0.5), at(Vec3::new(3.0, 0.6, side * 3.97)));
        }
        PartKind::RigPlateFront => {
            // A sloped ram plate over the nose, with a hazard-striped lip.
            let c = Vec3::new(7.7, 0.5, 0.0);
            panel(&mut m, c, Vec3::new(0.18, 1.2, 3.3), 0, 1.0, paint, rivet);
            m.bevel_box(Vec3::new(0.24, 0.2, 3.32), 0.15, 3, Paint::with(Material::Hazard, 0xffffff, 0.6), at(c - Vec3::Y * 1.05));
            for z in [-2.2f32, 0.0, 2.2] {
                m.bevel_box(Vec3::new(0.5, 0.16, 0.16), 0.12, 3, trim, at(c + Vec3::new(-0.5, 0.6, z)));
            }
        }
        PartKind::RigEngineLeft | PartKind::RigEngineRight => {
            // An exposed engine block: finned cylinders, a blower, two stacks.
            let side = if kind == PartKind::RigEngineLeft { -1.0f32 } else { 1.0 };
            let c = Vec3::new(-6.5, 1.9, side * 1.6);
            m.bevel_box(Vec3::new(1.0, 0.55, 1.1), 0.35, 3, Paint::steel(0x6a6560, 0.4), at(c - Vec3::Y * 0.2));
            for k in 0..4 {
                m.bevel_box(Vec3::new(0.08, 0.6, 1.15), 0.07, 2, trim, at(c + Vec3::new(-0.7 + k as f32 * 0.45, -0.15, 0.0)));
            }
            m.cylinder(0.45, 0.3, 0.2, 3, 18, Paint::steel(0x8a8070, 0.3), at(c + Vec3::Y * 0.6));
            for z in [-0.6f32, 0.6] {
                pipe(&mut m, c + Vec3::new(-0.6, 0.3, z), c + Vec3::new(-0.8, 2.6, z), 0.22, Paint::steel(0x2b2725, 0.75));
                pipe(&mut m, c + Vec3::new(-0.8, 2.5, z), c + Vec3::new(-1.3, 3.2, z), 0.22, Paint::steel(0x2b2725, 0.75));
                m.cylinder(0.28, 0.08, 0.06, 2, 14, trim, Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Y, Vec3::new(-0.5, 0.7, 0.0).normalize()), c + Vec3::new(-1.3, 3.2, z)));
            }
        }
        PartKind::RigMast => {
            // A tall lattice mast with a sensor head and a red lamp.
            let b = Vec3::new(-2.5, 2.0, -2.6);
            m.bevel_box(Vec3::new(0.45, 0.12, 0.45), 0.1, 2, trim, at(b));
            for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                pipe(&mut m, b + Vec3::new(sx * 0.3, 0.0, sz * 0.3), b + Vec3::new(sx * 0.12, 4.6, sz * 0.12), 0.07, trim);
            }
            for k in 0..6 {
                let y = 0.6 + k as f32 * 0.7;
                let s = 0.3 - 0.18 * (y / 4.6);
                m.bevel_box(Vec3::new(s, 0.04, s), 0.03, 2, trim, at(b + Vec3::Y * y));
            }
            let head = Vec3::new(-2.5, 7.0, -2.6);
            m.bevel_box(Vec3::new(0.5, 0.4, 0.5), 0.25, 3, paint, at(head));
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
                panel(&mut m, hb.center, h, axis, 1.0, paint, rivet);
                m.bevel_box(h * 0.6, (h * 0.6).min_element() * 0.5, 2, trim, at(hb.center - Vec3::Y * h.y * 0.3));
            }
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

fn append(m: &mut MeshBuilder, mut other: MeshBuilder) {
    let base = m.vertices.len() as u32;
    m.vertices.append(&mut other.vertices);
    m.indices.extend(other.indices.iter().map(|i| i + base));
}

pub fn is_gun(kind: PartKind) -> bool {
    matches!(kind, PartKind::StubGun | PartKind::HeavyCannon | PartKind::BigBore | PartKind::TwinBarrel | PartKind::LongGun)
}
