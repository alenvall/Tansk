//! The big machines of wave 7, in the kit language: the Convoy's FUEL
//! TANKER and THE LAND-SHIP (a beached ship hull on four giant track
//! units). Hull meshes are in the vehicle's hull frame; the Land-Ship's
//! 13 modules sit on their `part_def` hit boxes so they break off as the
//! pieces you shot. The deck guns and the stern ramp also come as loose
//! pieces (turret house, barrel, ramp) for the renderer to animate.

use arena_core::parts::PartKind;
use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};
use crate::shapes::hash;
use crate::tank::{panel, pipe, ring_x};

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
fn dark() -> Paint {
    Paint::steel(0x2e2a27, 0.6)
}
fn glass() -> Paint {
    Paint::steel(0x1a2228, 0.25)
}

// ── The fuel tanker ──────────────────────────────────────────────────────

/// The Convoy's fuel tanker in its hull frame (hull half 4.6 x 0.9 x 1.4,
/// wheels drawn separately): a big-rig cab in the team colour, a long
/// yellow and white fuel trailer with hazard stripes, rust and a
/// FLAMMABLE placard.
pub fn fuel_tanker(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let cab = Paint::painted(colour, 0.7);
    let trim = dark();
    // Chassis rails, bumper, fuel steps.
    m.bevel_box(Vec3::new(4.5, 0.16, 0.55), 0.12, 2, trim, at(Vec3::new(0.0, -0.45, 0.0)));
    m.bevel_box(Vec3::new(0.2, 0.25, 1.35), 0.18, 3, Paint::steel(0x8a8680, 0.35), at(Vec3::new(4.55, -0.35, 0.0)));
    m.bevel_box(Vec3::new(0.2, 0.08, 1.3), 0.06, 2, hazard(), at(Vec3::new(4.64, -0.35, 0.0)));
    // The cab: a tall boxy sleeper with a raked windscreen and a sun visor.
    m.bevel_box(Vec3::new(0.85, 0.75, 1.3), 0.4, 3, cab, at(Vec3::new(3.45, 0.55, 0.0)));
    m.wedge(Vec3::new(0.55, 0.55, 1.25), 0.3, 3, 0.6, cab, at(Vec3::new(4.3, 0.2, 0.0)));
    m.bevel_box(Vec3::new(0.05, 0.36, 1.0), 0.04, 2, glass(), Mat4::from_rotation_translation(Quat::from_rotation_z(0.18), Vec3::new(4.27, 0.9, 0.0)));
    m.bevel_box(Vec3::new(0.25, 0.06, 1.32), 0.05, 2, trim, at(Vec3::new(4.35, 1.3, 0.0)));
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.36, 0.28, 0.03), 0.03, 2, glass(), at(Vec3::new(3.75, 0.85, s * 1.31)));
        // Mirrors, an exhaust stack each side, a fuel tank on the rail.
        m.bevel_box(Vec3::new(0.06, 0.22, 0.1), 0.05, 2, trim, at(Vec3::new(4.2, 0.85, s * 1.5)));
        pipe(&mut m, Vec3::new(2.75, -0.3, s * 1.2), Vec3::new(2.75, 2.1, s * 1.2), 0.12, Paint::steel(0x8a8680, 0.3));
        m.cylinder(0.15, 0.06, 0.04, 2, 12, dark(), at(Vec3::new(2.75, 2.08, s * 1.2)));
        m.cylinder(0.32, 0.5, 0.14, 3, 18, Paint::steel(0xa8a49c, 0.3), along_x(Vec3::new(3.0, -0.3, s * 1.0)));
    }
    // Grille and headlamps.
    m.bevel_box(Vec3::new(0.05, 0.3, 0.75), 0.04, 2, Paint::steel(0x9a968e, 0.3), at(Vec3::new(4.84, 0.15, 0.0)));
    for z in [-0.95f32, 0.95] {
        ring_x(&mut m, Vec3::new(4.84, 0.05, z), 0.17, 0.06, dark());
        m.cylinder(0.12, 0.01, 0.0, 1, 12, Paint::glow(0xfff0c0, 0.8), along_x(Vec3::new(4.905, 0.05, z)));
    }
    // Fifth wheel, then the trailer barrel.
    m.cylinder(0.45, 0.08, 0.05, 2, 16, trim, at(Vec3::new(2.2, -0.2, 0.0)));
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let (r, h) = (1.12f32, 2.55f32);
    let profile = [
        (0.0, h + 0.35, Vec2::Y),
        (r * 0.62, h + 0.27, n(0.4, 1.0)),
        (r, h, n(1.0, 0.35)),
        (r, -h, n(1.0, -0.35)),
        (r * 0.62, -h - 0.27, n(0.4, -1.0)),
        (0.0, -h - 0.35, Vec2::NEG_Y),
    ];
    let cx = -1.75;
    let cy = 0.75;
    m.lathe(&profile, 28, false, Paint::painted(0xe6dfd0, 0.5), along_x(Vec3::new(cx, cy, 0.0)));
    // Yellow belly band and end rings with rivets.
    m.cylinder(r + 0.02, 1.0, 0.03, 1, 28, Paint::painted(0xe0b018, 0.45), along_x(Vec3::new(cx + 0.6, cy, 0.0)));
    for x in [cx - 2.0, cx + 2.0] {
        m.cylinder(r + 0.05, 0.1, 0.07, 2, 28, Paint::painted(0x8a8478, 0.6), along_x(Vec3::new(x, cy, 0.0)));
        for k in 0..8 {
            let a = (k as f32 + 0.5) / 8.0 * std::f32::consts::TAU;
            let d = Vec3::new(0.0, a.sin(), a.cos());
            m.rivet(Vec3::new(x, cy, 0.0) + d * (r + 0.05), d, 0.08, Paint::steel(0x8a857c, 0.5));
        }
    }
    // Rust bleeding from the seams.
    for k in 0..7 {
        let x = cx - 2.3 + 4.6 * hash(k, 51);
        let a = (hash(k, 52) - 0.5) * 2.4;
        let d = Vec3::new(0.0, a.cos(), a.sin());
        let rot = Quat::from_rotation_arc(Vec3::Y, d);
        m.bevel_box(Vec3::new(0.12 + 0.15 * hash(k, 53), 0.012, 0.5 + 0.4 * hash(k, 54)), 0.005, 1, Paint::painted(0x7a3a1e, 1.0), Mat4::from_rotation_translation(rot, Vec3::new(x, cy, 0.0) + d * (r + 0.01)));
    }
    // Hazard stripes along the bottom and the rear bumper.
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(2.5, 0.12, 0.02), 0.01, 1, hazard(), at(Vec3::new(cx, cy - 0.62, s * (r - 0.05))));
        // The FLAMMABLE panel: a red placard with a white flame, and a stencil plate.
        let side = s * (r + 0.02);
        m.bevel_box(Vec3::new(0.42, 0.42, 0.02), 0.02, 1, Paint::painted(0xc0281c, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4), Vec3::new(cx - 1.2, cy + 0.1, side)));
        for (dx, dy, rot) in [(-0.07f32, -0.05f32, 0.4f32), (0.07, -0.05, -0.4), (0.0, 0.12, 0.0)] {
            m.bevel_box(Vec3::new(0.04, 0.14, 0.01), 0.01, 1, Paint::painted(0xf4efe4, 0.3), Mat4::from_rotation_translation(Quat::from_rotation_z(rot), Vec3::new(cx - 1.2 + dx, cy + 0.1 + dy, side + s * 0.02)));
        }
        m.bevel_box(Vec3::new(1.25, 0.2, 0.015), 0.01, 1, Paint::painted(0x1c1a18, 0.5), at(Vec3::new(cx + 0.6, cy + 0.15, side + s * 0.005)));
        // Stencilled letters: block strokes on the dark plate.
        for k in 0..9 {
            let x = cx + 0.6 - 1.05 + k as f32 * 0.26;
            m.bevel_box(Vec3::new(0.07, 0.12, 0.01), 0.005, 1, Paint::painted(0xe0b018, 0.4), at(Vec3::new(x, cy + 0.15, side + s * 0.015)));
        }
    }
    m.bevel_box(Vec3::new(0.14, 0.15, 1.3), 0.1, 2, hazard(), at(Vec3::new(-4.6, -0.25, 0.0)));
    // Top walkway, a hatch and a hand rail.
    m.bevel_box(Vec3::new(2.2, 0.03, 0.3), 0.01, 1, Paint::steel(0x6a6660, 0.5), at(Vec3::new(cx, cy + r + 0.03, 0.0)));
    m.cylinder(0.38, 0.12, 0.08, 2, 16, Paint::painted(0xe0b018, 0.45), at(Vec3::new(cx + 0.8, cy + r + 0.1, 0.0)));
    for s in [-1.0f32, 1.0] {
        pipe(&mut m, Vec3::new(cx - 2.0, cy + r + 0.4, s * 0.35), Vec3::new(cx + 2.0, cy + r + 0.4, s * 0.35), 0.04, Paint::painted(0xe0b018, 0.45));
    }
    // Rear ladder and mud flaps.
    for z in [-0.25f32, 0.25] {
        m.bevel_box(Vec3::new(0.03, 0.9, 0.03), 0.01, 1, trim, at(Vec3::new(-4.45, 0.75, z)));
    }
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.03, 0.3, 0.3), 0.01, 1, Paint::rubber(0x24211f), at(Vec3::new(-4.3, -0.55, s * 1.15)));
    }
    m
}

// ── THE LAND-SHIP ────────────────────────────────────────────────────────

const HULL_GREY: u32 = 0x56636a;
const RUST_RED: u32 = 0x8a2e1e;
const DECK: u32 = 0x5a524a;

/// The Land-Ship's hull outline at the deck and at the keel (x, z), stern
/// to bow, one side.
const HULL_TOP: [(f32, f32); 5] = [(-15.0, 4.5), (-2.0, 4.6), (8.0, 4.5), (12.5, 2.8), (15.6, 0.0)];
const HULL_BOT: [(f32, f32); 5] = [(-14.4, 3.4), (-2.0, 3.7), (7.5, 3.6), (10.5, 2.0), (12.4, 0.0)];
const HULL_DECK_Y: f32 = 2.0;
const HULL_KEEL_Y: f32 = -2.0;

/// Half-beam of the Land-Ship's hull side at `x`, height `y` (hull frame).
fn hull_half_beam(x: f32, y: f32) -> f32 {
    let outline = |pts: &[(f32, f32); 5]| -> f32 {
        if x <= pts[0].0 {
            return pts[0].1;
        }
        for w in pts.windows(2) {
            if x <= w[1].0 {
                let t = (x - w[0].0) / (w[1].0 - w[0].0);
                return w[0].1 + (w[1].1 - w[0].1) * t;
            }
        }
        0.0
    };
    let f = ((y - HULL_KEEL_Y) / (HULL_DECK_Y - HULL_KEEL_Y)).clamp(0.0, 1.0);
    let (zt, zb) = (outline(&HULL_TOP), outline(&HULL_BOT));
    zb + (zt - zb) * f
}

/// The ship's hull core (hull frame; hull half 15 x 2 x 4.5, deck at y = 2):
/// a raked bow and a square stern, naval grey gone to rust over a red
/// waterline, plate seams, rivet rows, portholes, a deck rail and an anchor.
/// `colour` paints the raider stripes.
pub fn land_ship(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let grey = Paint::painted(HULL_GREY, 0.95);
    let red = Paint::painted(RUST_RED, 0.9);
    let top = HULL_TOP;
    let bot = HULL_BOT;
    let y_top = HULL_DECK_Y;
    let y_mid = -0.6;
    let y_bot = HULL_KEEL_Y;
    let inside = Vec3::new(0.0, 0.0, 0.0);
    let ring = |pts: &[(f32, f32)], y: f32, side: f32| -> Vec<Vec3> { pts.iter().map(|(x, z)| Vec3::new(*x, y, z * side)).collect() };
    for side in [-1.0f32, 1.0] {
        let t = ring(&top, y_top, side);
        let b = ring(&bot, y_bot, side);
        // The waterline height: an interpolated ring between.
        let k = (y_mid - y_bot) / (y_top - y_bot);
        let mid: Vec<Vec3> = t.iter().zip(&b).map(|(p, q)| q.lerp(*p, k)).collect();
        for i in 0..t.len() - 1 {
            m.flat_tri(t[i], t[i + 1], mid[i + 1], inside, grey);
            m.flat_tri(t[i], mid[i + 1], mid[i], inside, grey);
            m.flat_tri(mid[i], mid[i + 1], b[i + 1], inside, red);
            m.flat_tri(mid[i], b[i + 1], b[i], inside, red);
        }
    }
    // Stern transom, deck and keel faces.
    let (tl, tr) = (Vec3::new(-15.0, y_top, -4.5), Vec3::new(-15.0, y_top, 4.5));
    let (bl, br) = (Vec3::new(-14.4, y_bot, -3.4), Vec3::new(-14.4, y_bot, 3.4));
    m.flat_tri(tl, tr, br, inside, grey);
    m.flat_tri(tl, br, bl, inside, grey);
    let deck = Paint::painted(DECK, 0.8);
    let keel = Paint::painted(RUST_RED, 1.0);
    let c_top = Vec3::new(0.0, y_top, 0.0);
    let c_bot = Vec3::new(0.0, y_bot, 0.0);
    for side in [-1.0f32, 1.0] {
        let t = ring(&top, y_top, side);
        let b = ring(&bot, y_bot, side);
        for i in 0..t.len() - 1 {
            m.flat_tri(c_top, t[i], t[i + 1], Vec3::new(0.0, -5.0, 0.0), deck);
            m.flat_tri(c_bot, b[i], b[i + 1], Vec3::new(0.0, 5.0, 0.0), keel);
        }
        m.flat_tri(c_top, t[0], Vec3::new(-15.0, y_top, 0.0), Vec3::new(0.0, -5.0, 0.0), deck);
        m.flat_tri(c_bot, b[0], Vec3::new(-14.4, y_bot, 0.0), Vec3::new(0.0, 5.0, 0.0), keel);
    }
    // Plate seams, rivet rows and portholes along both sides (the straight run).
    let side_z = |x: f32, y: f32| -> f32 {
        let f = (y - y_bot) / (y_top - y_bot);
        let zt = if x < 8.0 { 4.5 } else { 4.5 - (x - 8.0) / 4.5 * 1.7 };
        let zb = if x < 7.5 { 3.5 } else { 3.6 - (x - 7.5) / 3.0 * 1.6 };
        zb + (zt - zb) * f
    };
    let seam = Paint::painted(0x3e474c, 0.9);
    let rivet = Paint::steel(0x7a7068, 0.5);
    for side in [-1.0f32, 1.0] {
        let mut x = -13.5;
        while x < 8.0 {
            let z = side_z(x, 0.6);
            let lean = (side_z(x, y_top) - side_z(x, y_bot)) / (y_top - y_bot);
            let rot = Quat::from_rotation_x(side * -lean.atan());
            m.bevel_box(Vec3::new(0.08, 1.9, 0.06), 0.05, 2, seam, Mat4::from_rotation_translation(rot, Vec3::new(x, 0.0, side * (z + 0.02))));
            x += 3.0;
        }
        let mut x = -14.0;
        while x < 7.8 {
            for y in [1.7f32, -0.4] {
                let z = side_z(x, y);
                m.rivet(Vec3::new(x, y, side * (z + 0.01)), Vec3::new(0.0, 0.0, side), 0.13, rivet);
            }
            x += 1.5;
        }
        for k in 0..7 {
            let x = -11.0 + k as f32 * 2.6;
            let z = side_z(x, 0.8);
            m.cylinder(0.42, 0.08, 0.07, 2, 18, Paint::steel(0x6a625a, 0.5), along_z(Vec3::new(x, 0.8, side * (z + 0.02))));
            m.cylinder(0.28, 0.1, 0.03, 1, 16, glass(), along_z(Vec3::new(x, 0.8, side * (z + 0.03))));
        }
        // Raider stripes: two bold slashes in the crew colour near the bow.
        for k in 0..2 {
            let x = 4.5 + k as f32 * 1.4;
            let z = side_z(x, 0.6);
            m.bevel_box(Vec3::new(0.35, 1.2, 0.02), 0.01, 1, Paint::painted(colour, 0.8), Mat4::from_rotation_translation(Quat::from_rotation_z(0.5), Vec3::new(x, 0.5, side * (z + 0.03))));
        }
        // Anchor and hawse pipe at the bow.
        let ax = 11.2;
        let az = side_z(ax, 1.2) * side;
        m.cylinder(0.45, 0.1, 0.08, 2, 16, dark(), along_z(Vec3::new(ax, 1.2, az + side * 0.04)));
        m.bevel_box(Vec3::new(0.14, 0.7, 0.1), 0.08, 2, Paint::steel(0x3a3532, 0.6), at(Vec3::new(ax, 0.4, az + side * 0.1)));
        m.bevel_box(Vec3::new(0.6, 0.14, 0.1), 0.08, 2, Paint::steel(0x3a3532, 0.6), Mat4::from_rotation_translation(Quat::from_rotation_z(0.25), Vec3::new(ax, -0.25, az + side * 0.1)));
        // Deck rail along the straight run.
        let mut x = -14.6;
        while x < 8.0 {
            pipe(&mut m, Vec3::new(x, y_top, side * 4.4), Vec3::new(x, y_top + 0.9, side * 4.4), 0.07, Paint::steel(0x8a8478, 0.5));
            x += 2.4;
        }
        pipe(&mut m, Vec3::new(-14.6, y_top + 0.9, side * 4.4), Vec3::new(8.0, y_top + 0.9, side * 4.4), 0.08, Paint::steel(0x8a8478, 0.5));
    }
    // A hazard band across the bow stem and the core vent amidships.
    m.bevel_box(Vec3::new(0.25, 1.6, 0.25), 0.05, 1, hazard(), Mat4::from_rotation_translation(Quat::from_rotation_z(-0.55), Vec3::new(14.0, 0.1, 0.0)));
    m.bevel_box(Vec3::new(1.2, 0.3, 1.0), 0.25, 3, Paint::steel(0x2a2422, 0.6), at(Vec3::new(0.8, 2.25, 0.0)));
    m.cylinder(0.6, 0.05, 0.02, 1, 14, Paint::glow(0xff6020, 1.2), at(Vec3::new(0.8, 2.58, 0.0)));
    // Funnel with a soot-black cap behind the bridge.
    m.cylinder(1.0, 1.6, 0.3, 3, 24, Paint::painted(colour, 0.6), at(Vec3::new(-7.0, 3.6, 0.0)));
    m.cylinder(1.1, 0.28, 0.18, 3, 24, Paint::steel(0x1e1a18, 0.8), at(Vec3::new(-7.0, 5.3, 0.0)));
    m.cylinder(0.75, 0.02, 0.0, 1, 20, Paint::with(Material::BareSteel, 0x0c0a09, 0.9), at(Vec3::new(-7.0, 5.59, 0.0)));
    // Struts down to where the track units hang.
    for (x, z) in [(8.0f32, -1.0f32), (8.0, 1.0), (-8.0, -1.0), (-8.0, 1.0)] {
        m.bevel_box(Vec3::new(1.2, 0.4, 1.4), 0.3, 3, dark(), at(Vec3::new(x, -2.1, z * 3.0)));
    }
    m
}

/// A deck gun's turret house in its own frame (ring centre at the origin,
/// facing +X): a faceted armoured house on a bolted ring.
pub fn ship_gun_house(colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(HULL_GREY, 0.85);
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let profile = [
        (0.0, 1.0, Vec2::Y),
        (1.25, 1.0, n(0.35, 1.0)),
        (1.85, 0.4, n(1.0, 0.3)),
        (1.95, -0.6, Vec2::X),
        (1.95, -0.9, Vec2::NEG_Y),
        (0.0, -0.9, Vec2::NEG_Y),
    ];
    m.lathe(&profile, 28, false, paint, Mat4::IDENTITY);
    m.cylinder(2.1, 0.14, 0.1, 2, 28, dark(), at(Vec3::Y * -0.95));
    for k in 0..8 {
        let a = (k as f32 + 0.5) / 8.0 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        m.rivet(d * 2.02 + Vec3::Y * -0.8, Vec3::Y, 0.14, Paint::steel(0x8a857c, 0.4));
    }
    // A mantlet, a crew hatch, a crew-colour band and a number stripe.
    m.bevel_box(Vec3::new(0.42, 0.58, 0.9), 0.3, 3, dark(), at(Vec3::new(1.75, 0.25, 0.0)));
    m.cylinder(0.5, 0.12, 0.09, 3, 18, paint, at(Vec3::new(-0.6, 1.05, 0.5)));
    m.cylinder(0.56, 0.06, 0.04, 2, 18, dark(), at(Vec3::new(-0.6, 0.98, 0.5)));
    m.cylinder(1.955, 0.16, 0.04, 1, 28, Paint::painted(colour, 0.6), at(Vec3::Y * -0.3));
    m.bevel_box(Vec3::new(0.5, 0.08, 0.02), 0.01, 1, hazard(), at(Vec3::new(-1.2, 0.5, 1.6)));
    m
}

/// Where a deck gun's barrel pivots, in the turret frame.
pub const SHIP_TRUNNION: Vec3 = Vec3::new(1.75, 0.25, 0.0);

/// A deck gun's barrel in its own frame (trunnion at the origin, along +X).
pub fn ship_gun_barrel() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let steel = Paint::steel(0x3e3a36, 0.5);
    ring_x(&mut m, Vec3::X * 0.4, 0.42, 0.5, steel);
    m.cylinder(0.3, 1.7, 0.1, 2, 20, Paint::painted(HULL_GREY, 0.6), along_x(Vec3::X * 2.2));
    ring_x(&mut m, Vec3::X * 3.72, 0.5, 0.34, steel);
    m.cylinder(0.3, 0.01, 0.0, 1, 16, Paint::steel(0x0c0a09, 0.9), along_x(Vec3::X * 4.065));
    m
}

/// Where the stern ramp hinges, hull frame.
pub const SHIP_RAMP_HINGE: Vec3 = Vec3::new(-16.0, -1.7, 0.0);

/// The stern ramp in its own frame (hinge along Z at the origin, standing up
/// along +Y; it falls open toward -X).
pub fn ship_ramp() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(HULL_GREY, 0.9);
    m.bevel_box(Vec3::new(0.14, 1.75, 2.9), 0.12, 3, paint, at(Vec3::Y * 1.75));
    for k in 0..6 {
        m.bevel_box(Vec3::new(0.08, 0.08, 2.6), 0.06, 2, dark(), at(Vec3::new(-0.15, 0.4 + k as f32 * 0.55, 0.0)));
    }
    for z in [-2.7f32, 2.7] {
        m.bevel_box(Vec3::new(0.14, 1.75, 0.16), 0.03, 1, hazard(), at(Vec3::new(-0.02, 1.75, z)));
    }
    m.cylinder(0.14, 2.9, 0.02, 1, 10, dark(), along_z(Vec3::ZERO));
    m
}

/// The hangar's frame round the ramp (hull frame).
fn ship_hangar_house(m: &mut MeshBuilder) {
    let paint = Paint::painted(HULL_GREY, 0.9);
    for z in [-3.1f32, 3.1] {
        m.bevel_box(Vec3::new(0.5, 1.9, 0.22), 0.2, 3, paint, at(Vec3::new(-15.5, 0.0, z)));
    }
    m.bevel_box(Vec3::new(0.55, 0.26, 3.3), 0.22, 3, paint, at(Vec3::new(-15.5, 1.85, 0.0)));
    m.bevel_box(Vec3::new(0.05, 1.6, 2.8), 0.02, 1, Paint::steel(0x141210, 0.9), at(Vec3::new(-15.1, 0.0, 0.0)));
    m.cylinder(0.12, 0.08, 0.04, 1, 8, Paint::glow(0xffa020, 2.0), at(Vec3::new(-15.9, 2.15, -2.6)));
    m.cylinder(0.12, 0.08, 0.04, 1, 8, Paint::glow(0xffa020, 2.0), at(Vec3::new(-15.9, 2.15, 2.6)));
}

/// The hangar without its ramp (the renderer swings the ramp).
pub fn ship_hangar_frame() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    ship_hangar_house(&mut m);
    m
}

/// One track unit centred on `c` with half extents `h` (hull frame).
fn track_unit(m: &mut MeshBuilder, c: Vec3, h: Vec3, colour: u32) {
    let rubber = Paint::rubber(0x24211f);
    // The track loop: a rounded slab with grousers on top and bottom.
    m.bevel_box(Vec3::new(h.x, h.y * 0.95, h.z * 0.8), 1.1, 3, rubber, at(c));
    let mut x = -h.x + 1.2;
    while x < h.x - 1.1 {
        for y in [h.y * 0.95, -h.y * 0.95] {
            m.bevel_box(Vec3::new(0.18, 0.1, h.z * 0.82), 0.08, 2, Paint::steel(0x3a3532, 0.6), at(c + Vec3::new(x, y, 0.0)));
        }
        x += 0.9;
    }
    // Road wheels, sprocket and idler on the outside face.
    let side = c.z.signum();
    for k in 0..5 {
        let x = -h.x + 1.8 + k as f32 * (2.0 * h.x - 3.6) / 4.0;
        m.cylinder(0.72, 0.16, 0.12, 3, 22, Paint::steel(0x5c5752, 0.5), along_z(c + Vec3::new(x, -0.25, side * h.z * 0.82)));
        m.cylinder(0.48, 0.2, 0.08, 2, 18, Paint::steel(0x3e3a37, 0.55), along_z(c + Vec3::new(x, -0.25, side * h.z * 0.84)));
        m.cylinder(0.24, 0.24, 0.1, 3, 14, Paint::steel(0xa8a398, 0.35), along_z(c + Vec3::new(x, -0.25, side * h.z * 0.86)));
    }
    for x in [-h.x + 0.95, h.x - 0.95] {
        m.cylinder(0.85, 0.2, 0.12, 3, 20, Paint::steel(0x3e3a36, 0.5), along_z(c + Vec3::new(x, 0.15, side * h.z * 0.82)));
        m.cylinder(0.3, 0.24, 0.1, 3, 14, Paint::steel(0xa8a398, 0.35), along_z(c + Vec3::new(x, 0.15, side * h.z * 0.84)));
    }
    // A riveted fender in the crew colour, with a hazard lip.
    let fender = c + Vec3::new(0.0, h.y + 0.08, 0.0);
    panel(m, fender, Vec3::new(h.x, 0.14, h.z), 1, 1.0, Paint::painted(colour, 0.6), Paint::steel(0x8a857c, 0.4));
    m.bevel_box(Vec3::new(h.x, 0.1, 0.06), 0.05, 2, hazard(), at(fender + Vec3::new(0.0, -0.12, side * h.z)));
}

/// A Land-Ship module in the hull frame, on its hit boxes. The deck guns
/// come with their barrels laid level (fore along +X, aft along -X); the
/// hangar with its ramp shut.
pub fn ship_part(kind: PartKind, colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let boxes = arena_core::part_def(kind).hitboxes;
    let crew = Paint::painted(colour, 0.8);
    let steel = Paint::painted(0x5a4a3e, 0.9);
    let rivet = Paint::steel(0x8a857c, 0.4);
    match kind {
        PartKind::ShipGunFore | PartKind::ShipGunAft => {
            let ring = arena_core::parts::SHIP_GUNS.iter().find(|g| g.0 == kind).map_or(Vec3::ZERO, |g| g.1);
            let yaw = if kind == PartKind::ShipGunAft { std::f32::consts::PI } else { 0.0 };
            let frame = Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), ring);
            let mut house = ship_gun_house(colour);
            house.transform_from(0, frame);
            append(&mut m, house);
            let mut barrel = ship_gun_barrel();
            barrel.transform_from(0, frame * at(SHIP_TRUNNION));
            append(&mut m, barrel);
        }
        PartKind::ShipMortar => {
            let b = boxes[0];
            let c = b.center;
            panel(&mut m, c - Vec3::Y * 0.2, b.half - Vec3::new(0.05, 0.25, 0.05), 1, 1.0, steel, rivet);
            for (i, (dx, dz)) in [(-0.6f32, -0.9f32), (-0.6, 0.0), (-0.6, 0.9), (0.6, -0.45), (0.6, 0.45)].iter().enumerate() {
                let base = c + Vec3::new(*dx, 0.3, *dz);
                let tip = base + Vec3::new(0.45, 1.0, 0.0) * 0.9;
                pipe(&mut m, base, tip, 0.24, Paint::steel(0x3a3532, 0.55));
                m.cylinder(0.32, 0.08, 0.06, 2, 14, Paint::steel(0x8a8478, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Y, (tip - base).normalize()), tip));
                if i % 2 == 0 {
                    m.bevel_box(Vec3::new(0.25, 0.12, 0.2), 0.08, 2, crew, at(base + Vec3::new(-0.25, -0.15, 0.0)));
                }
            }
            m.bevel_box(Vec3::new(b.half.x, 0.08, 0.04), 0.02, 1, hazard(), at(c + Vec3::new(0.0, -0.3, b.half.z - 0.02)));
            // Shell racks.
            for k in 0..6 {
                m.cylinder(0.1, 0.3, 0.07, 2, 10, Paint::painted(0xd6a21e, 0.45), at(c + Vec3::new(-1.2 + k as f32 * 0.18, 0.0, -b.half.z + 0.25)));
            }
        }
        PartKind::ShipMgLeft | PartKind::ShipMgRight => {
            let b = boxes[0];
            let side = b.center.z.signum();
            panel(&mut m, b.center, b.half - Vec3::splat(0.03), 2, side, steel, rivet);
            m.bevel_box(Vec3::new(b.half.x * 0.6, 0.08, 0.03), 0.01, 1, Paint::steel(0x0c0a09, 0.9), at(b.center + Vec3::new(0.0, 0.1, side * b.half.z)));
            // A twin MG poking out sideways and sandbags on the lip.
            for x in [-0.15f32, 0.15] {
                m.cylinder(0.08, 0.6, 0.03, 1, 10, Paint::steel(0x2a2725, 0.4), along_z(b.center + Vec3::new(x, 0.1, side * (b.half.z + 0.5))));
                m.cylinder(0.12, 0.08, 0.05, 2, 10, dark(), along_z(b.center + Vec3::new(x, 0.1, side * (b.half.z + 1.05))));
            }
            let mut bag = crate::props::sandbags(Vec3::new(b.half.x * 0.9, 0.18, 0.25), 7);
            bag.transform_from(0, at(b.center + Vec3::new(0.0, b.half.y + 0.15, 0.0)));
            append(&mut m, bag);
            m.bevel_box(Vec3::new(b.half.x, 0.08, 0.02), 0.01, 1, crew, at(b.center + Vec3::new(0.0, -b.half.y + 0.12, side * (b.half.z + 0.01))));
        }
        PartKind::ShipBridge => {
            let house = boxes[0];
            let c = house.center;
            let h = house.half;
            m.bevel_box(h - Vec3::new(0.05, 0.1, 0.05), 0.4, 3, Paint::painted(0xd8d0bc, 0.6), at(c - Vec3::Y * 0.1));
            // Wraparound windows, wings and a roof.
            m.bevel_box(Vec3::new(h.x + 0.02, 0.3, h.z - 0.3), 0.03, 1, glass(), at(c + Vec3::new(0.0, 0.45, 0.0)));
            m.bevel_box(Vec3::new(h.x - 0.4, 0.3, h.z + 0.02), 0.03, 1, glass(), at(c + Vec3::new(0.0, 0.45, 0.0)));
            m.bevel_box(Vec3::new(h.x + 0.25, 0.14, h.z + 0.25), 0.13, 3, Paint::painted(HULL_GREY, 0.6), at(c + Vec3::Y * (h.y - 0.05)));
            for s in [-1.0f32, 1.0] {
                m.bevel_box(Vec3::new(0.7, 0.08, 0.8), 0.07, 2, Paint::steel(0x5a5650, 0.5), at(c + Vec3::new(0.6, 0.0, s * (h.z + 0.6))));
            }
            m.bevel_box(Vec3::new(h.x + 0.03, 0.14, h.z + 0.03), 0.1, 2, crew, at(c - Vec3::Y * 0.5));
            // The mast with yard arms and the crow's-nest radar.
            let mast = boxes.get(1).map_or(c, |b| b.center);
            let mh = boxes.get(1).map_or(1.5, |b| b.half.y);
            pipe(&mut m, mast - Vec3::Y * mh, mast + Vec3::Y * mh, 0.2, Paint::steel(0x6a6660, 0.5));
            m.bevel_box(Vec3::new(0.06, 0.06, 1.4), 0.02, 1, Paint::steel(0x6a6660, 0.5), at(mast + Vec3::Y * 0.6));
            let top = boxes.get(2).map_or(mast + Vec3::Y * mh, |b| b.center);
            m.bevel_box(Vec3::new(0.5, 0.25, 0.5), 0.2, 3, Paint::painted(0xd8d0bc, 0.6), at(top));
            m.bevel_box(Vec3::new(0.08, 0.2, 1.0), 0.03, 1, dark(), at(top + Vec3::Y * 0.4));
            m.cylinder(0.12, 0.1, 0.05, 2, 8, Paint::glow(0xff3020, 3.0), at(top + Vec3::Y * 0.75));
            m.cylinder(0.18, 0.04, 0.02, 1, 10, Paint::glow(0x5fc8ff, 2.0), along_x(top + Vec3::X * 0.52));
        }
        PartKind::ShipTrackFL | PartKind::ShipTrackFR | PartKind::ShipTrackRL | PartKind::ShipTrackRR => {
            let b = boxes[0];
            track_unit(&mut m, b.center, b.half, colour);
        }
        PartKind::ShipPlatesFore | PartKind::ShipPlatesAft => {
            // Deck plate (riveted armour with weld seams) and belt plates on the sides.
            let d = boxes[0];
            let n = 4;
            for k in 0..n {
                let w = d.half.x / n as f32;
                let x = d.center.x - d.half.x + w * (2 * k + 1) as f32;
                panel(&mut m, Vec3::new(x, d.center.y + 0.02 * (k % 2) as f32, d.center.z), Vec3::new(w - 0.05, d.half.y, d.half.z - 0.3), 1, 1.0, steel, rivet);
            }
            // Belt plates: thin slabs laid on the hull side, below the deck
            // line and above the waterline, following the bow's rake (and
            // stopping short of the stem) so the hull's lines still read.
            // Visual only: the hit boxes stay the flat slabs they were.
            for b in &boxes[1..] {
                let side = b.center.z.signum();
                let (y0, y1) = (-0.4, HULL_DECK_Y - 0.45);
                let (x0, x1) = (b.center.x - b.half.x, (b.center.x + b.half.x).min(11.2));
                let n = 4;
                let w = (x1 - x0) / n as f32;
                for k in 0..n {
                    let xc = x0 + w * (k as f32 + 0.5);
                    let yc = 0.5 * (y0 + y1);
                    let paint = Paint::painted(if (k + (side > 0.0) as usize) % 2 == 0 { 0x5a4a3e } else { 0x4e4038 }, 0.95);
                    let thick = 0.05 + 0.015 * (k % 2) as f32;
                    // The hull side's frame here: along the hull, up its
                    // flare, and out.
                    let e = 0.05;
                    let dzdx = (hull_half_beam(xc + e, yc) - hull_half_beam(xc - e, yc)) / (2.0 * e);
                    let dzdy = (hull_half_beam(xc, yc + e) - hull_half_beam(xc, yc - e)) / (2.0 * e);
                    let along = Vec3::new(1.0, 0.0, side * dzdx).normalize();
                    let up = Vec3::new(0.0, 1.0, side * dzdy);
                    let up = (up - along * up.dot(along)).normalize();
                    let mut x_axis = along;
                    let mut out = x_axis.cross(up);
                    if out.z * side < 0.0 {
                        x_axis = -x_axis;
                        out = -out;
                    }
                    let centre = Vec3::new(xc, yc, side * hull_half_beam(xc, yc)) + out * (thick + 0.005);
                    let frame = Mat4::from_cols(x_axis.extend(0.0), up.extend(0.0), out.extend(0.0), centre.extend(1.0));
                    let h = 0.5 * (y1 - y0);
                    let mut pm = MeshBuilder::new();
                    panel(&mut pm, Vec3::ZERO, Vec3::new(0.5 * w - 0.04, h, thick), 2, 1.0, paint, rivet);
                    pm.bevel_box(Vec3::new(0.5 * w - 0.04, 0.12, 0.02), 0.01, 1, crew, at(Vec3::new(0.0, h - 0.35, thick + 0.01)));
                    pm.bevel_box(Vec3::new(0.5 * w - 0.04, 0.08, 0.02), 0.01, 1, hazard(), at(Vec3::new(0.0, -h + 0.14, thick + 0.01)));
                    pm.transform_from(0, frame);
                    append(&mut m, pm);
                }
            }
        }
        PartKind::ShipHangar => {
            ship_hangar_house(&mut m);
            let mut ramp = ship_ramp();
            ramp.transform_from(0, at(SHIP_RAMP_HINGE));
            append(&mut m, ramp);
        }
        _ => {
            for hb in boxes {
                m.bevel_box(hb.half, hb.half.min_element() * 0.4, 3, steel, at(hb.center));
            }
        }
    }
    m
}

/// A faint chevron painted on the road (points along +X), standing on y = 0.
pub fn chevron() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::glow(0xffc84a, 0.55);
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(1.1, 0.02, 0.18), 0.01, 1, paint, Mat4::from_rotation_translation(Quat::from_rotation_y(s * 0.65), Vec3::new(-0.45, 0.02, s * 0.6)));
    }
    m
}

fn append(m: &mut MeshBuilder, mut other: MeshBuilder) {
    let base = m.vertices.len() as u32;
    m.vertices.append(&mut other.vertices);
    m.indices.extend(other.indices.iter().map(|i| i + base));
}
