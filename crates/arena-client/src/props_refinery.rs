//! The Refinery's kit, in the same hard-surface language as everything
//! else: bevelled plates, rivet rows, weld seams, painted metal worn to
//! steel at the edges, hazard stripes. Obstacle-shaped builders take the
//! collider's half extents and are centred on the box (like `props`);
//! whole-obstacle builders work in the obstacle frame (standing on y = 0).

use glam::{Mat4, Quat, Vec2, Vec3};

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
fn concrete() -> Paint {
    Paint::matte(0xa49c8e)
}
fn dark_steel() -> Paint {
    Paint::steel(0x3e3a36, 0.55)
}

/// Company colours for the stencils and bands, by variant.
pub const COMPANY: [u32; 3] = [0xb8341f, 0x2a5c9a, 0x2f7a52];

/// A ring of rivets round a vertical cylinder of radius `r` at height `y`.
fn rivet_ring(m: &mut MeshBuilder, r: f32, y: f32, spacing: f32, paint: Paint) {
    let n = ((std::f32::consts::TAU * r) / spacing).round().max(6.0) as u32;
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        m.rivet(d * r + Vec3::Y * y, d, 0.06, paint);
    }
}

/// A railing ring round a vertical cylinder: posts and a top rail.
fn railing_ring(m: &mut MeshBuilder, r: f32, y: f32, paint: Paint) {
    let n = ((std::f32::consts::TAU * r) / 1.4).round().max(6.0) as u32;
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        octo_pipe(m, d * r + Vec3::Y * y, d * r + Vec3::Y * (y + 1.0), 0.035, paint);
    }
    m.torus(r, 0.04, 0.08, (n * 2).max(16), paint, at(Vec3::Y * (y + 1.0)));
    m.torus(r, 0.03, 0.06, (n * 2).max(16), paint, at(Vec3::Y * (y + 0.5)));
}

/// A big vertical storage tank, radius `r`, half height `hy`, centred:
/// white and silver plate courses with weld seams and rivet rows, a
/// shallow roof with a hand rail, a stair up the side, a company band and
/// stencil, rust drips from the roof rim, hazard stripes at the foot.
pub fn storage_tank(r: f32, hy: f32, variant: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let white = Paint::painted([0xe9e5dc, 0xd9dcdc, 0xe4dfd2][variant as usize % 3], 0.55);
    let seam = Paint::painted(0xc9c4b8, 0.7);
    let rivets = Paint::steel(0x9a958c, 0.45);
    let company = Paint::painted(COMPANY[variant as usize % 3], 0.5);
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let roof = r * 0.12;
    let profile = [
        (0.0, hy + roof, Vec2::Y),
        (r * 0.3, hy + roof * 0.85, n(0.15, 1.0)),
        (r - 0.15, hy + 0.05, n(0.15, 1.0)),
        (r, hy - 0.1, n(1.0, 0.2)),
        (r, -hy, Vec2::X),
        (r, -hy, Vec2::NEG_Y),
        (0.0, -hy, Vec2::NEG_Y),
    ];
    m.lathe(&profile, 40, false, white, Mat4::IDENTITY);
    // Plate courses: weld seams with rivet rows.
    let courses = ((2.0 * hy) / 2.4).round().max(2.0) as u32;
    for c in 1..courses {
        let y = -hy + c as f32 * 2.0 * hy / courses as f32;
        m.cylinder(r + 0.03, 0.05, 0.02, 1, 40, seam, at(Vec3::Y * y));
        rivet_ring(&mut m, r + 0.04, y + 0.18, 1.1, rivets);
    }
    // The company band near the top, and a stencil disc on it.
    let band_y = hy - 1.6;
    m.cylinder(r + 0.05, 0.55, 0.03, 1, 40, company, at(Vec3::Y * band_y));
    let side = Vec3::new(0.0, 0.0, 1.0);
    let disc = Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), side * (r + 0.05) + Vec3::Y * (band_y - 2.4));
    m.cylinder(1.5, 0.04, 0.02, 1, 20, company, disc);
    m.cylinder(1.15, 0.06, 0.02, 1, 20, Paint::painted(0xf2eee6, 0.4), disc);
    // The stencil mark: a flame chevron in the disc.
    for (dx, dy, rot) in [(-0.25f32, 0.0f32, 0.5f32), (0.25, 0.0, -0.5), (0.0, 0.45, 0.0)] {
        let p = side * (r + 0.13) + Vec3::new(dx, band_y - 2.4 + dy - 0.15, 0.0);
        m.bevel_box(Vec3::new(0.14, 0.5, 0.03), 0.02, 1, company, Mat4::from_rotation_translation(Quat::from_rotation_z(rot), p));
    }
    // Rust drips from the roof rim (broad painted streaks).
    for k in 0..9 {
        let a = hash(k, 17 + variant) * std::f32::consts::TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let len = 0.8 + 2.2 * hash(k, 23 + variant);
        let w = 0.12 + 0.2 * hash(k, 29);
        let rot = Quat::from_rotation_arc(Vec3::Z, d);
        let rust = Paint::painted([0x7a3a1e, 0x8e4a24, 0x6a3018][k as usize % 3], 1.0);
        m.bevel_box(Vec3::new(w, len, 0.015), 0.01, 1, rust, Mat4::from_rotation_translation(rot, d * (r + 0.02) + Vec3::Y * (hy - 0.3 - len)));
        m.bevel_box(Vec3::new(w * 0.45, len * 0.5, 0.016), 0.01, 1, rust, Mat4::from_rotation_translation(rot, d * (r + 0.03) + Vec3::Y * (hy - 0.3 - 2.0 * len)));
    }
    // A hand rail round the roof.
    railing_ring(&mut m, r - 0.3, hy + 0.02, Paint::painted(0xd6a21e, 0.7));
    // The stair: a stringer climbing round the side, with steps.
    let steps = 26;
    let stair = Paint::steel(0x6a6660, 0.5);
    for k in 0..steps {
        let t = k as f32 / steps as f32;
        let a = -0.6 + t * 1.5;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let y = -hy + 0.3 + t * (2.0 * hy - 0.3);
        let rot = Quat::from_rotation_arc(Vec3::X, d);
        m.bevel_box(Vec3::new(0.45, 0.04, 0.3), 0.02, 1, stair, Mat4::from_rotation_translation(rot, d * (r + 0.5) + Vec3::Y * y));
        if k % 3 == 0 {
            octo_pipe(&mut m, d * (r + 0.95) + Vec3::Y * y, d * (r + 0.95) + Vec3::Y * (y + 1.0), 0.03, Paint::painted(0xd6a21e, 0.7));
        }
    }
    // The foot: a concrete ring, a hazard band, a pipe stub with a valve.
    m.cylinder(r + 0.5, 0.25, 0.08, 1, 40, concrete(), at(Vec3::Y * (-hy + 0.05)));
    m.cylinder(r + 0.04, 0.22, 0.02, 1, 40, hazard(), at(Vec3::Y * (-hy + 0.55)));
    let stub = Vec3::new(-(r + 1.2), -hy + 0.9, 0.0);
    m.cylinder(0.35, 1.0, 0.04, 1, 14, Paint::painted(0x5a5a54, 0.7), along_x(stub));
    m.torus(0.42, 0.06, 0.1, 14, Paint::painted(0xb02a20, 0.5), Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), stub + Vec3::new(-0.6, 0.0, 0.0)));
    m
}

/// A distillation column, radius `r`, half height `hy`, centred: a tall
/// silver tower with insulation bands, ringed platforms, a caged ladder,
/// a dome cap with a red light, a riser pipe down its side.
pub fn column(r: f32, hy: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let silver = Paint::painted(0xc4c2ba, 0.6);
    let band = Paint::painted(0x6c6a64, 0.7);
    let yellow = Paint::painted(0xd6a21e, 0.7);
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let profile = [
        (0.0, hy + r * 0.55, Vec2::Y),
        (r * 0.6, hy + r * 0.42, n(0.5, 1.0)),
        (r, hy, n(1.0, 0.3)),
        (r, -hy + 1.4, Vec2::X),
        (r + 0.3, -hy + 1.0, n(1.0, -0.3)),
        (r + 0.3, -hy, Vec2::X),
        (0.0, -hy, Vec2::NEG_Y),
    ];
    m.lathe(&profile, 28, false, silver, Mat4::IDENTITY);
    let mut y = -hy + 2.5;
    while y < hy - 0.5 {
        m.cylinder(r + 0.04, 0.12, 0.03, 1, 28, band, at(Vec3::Y * y));
        y += 2.8;
    }
    // Platforms with rails every 7 m.
    let mut py = -hy + 6.0;
    while py < hy - 1.0 {
        let deck = [(r, 0.06, Vec2::Y), (r + 1.2, 0.06, Vec2::Y), (r + 1.2, -0.06, Vec2::NEG_Y), (r, -0.06, Vec2::NEG_Y)];
        m.lathe(&deck, 28, true, Paint::steel(0x5a5650, 0.5), at(Vec3::Y * py));
        railing_ring(&mut m, r + 1.15, py, yellow);
        py += 7.0;
    }
    // Caged ladder on +X.
    for z in [-0.25f32, 0.25] {
        m.bevel_box(Vec3::new(0.04, hy - 1.0, 0.04), 0.01, 1, yellow, at(Vec3::new(r + 0.35, 0.6, z)));
    }
    let mut ry = -hy + 2.0;
    while ry < hy {
        m.torus(0.42, 0.025, 0.05, 10, yellow, Mat4::from_rotation_translation(Quat::IDENTITY, Vec3::new(r + 0.5, ry, 0.0)));
        ry += 1.2;
    }
    // Riser pipe down the side, with flanges.
    let px = Vec3::new(-(r + 0.6), 0.0, 0.0);
    m.cylinder(0.32, hy - 0.5, 0.04, 1, 12, Paint::painted(0x8a8a80, 0.6), at(px - Vec3::Y * 0.5));
    let mut fy = -hy + 1.0;
    while fy < hy - 1.0 {
        m.cylinder(0.42, 0.06, 0.02, 1, 12, dark_steel(), at(px + Vec3::Y * fy));
        fy += 3.5;
    }
    // A red aviation light on the cap.
    m.cylinder(0.12, 0.25, 0.04, 1, 8, dark_steel(), at(Vec3::Y * (hy + r * 0.55 + 0.2)));
    m.cylinder(0.16, 0.12, 0.08, 2, 10, Paint::glow(0xff2a10, 2.5), at(Vec3::Y * (hy + r * 0.55 + 0.5)));
    m
}

/// The overhead pipe bundle of a rack, half extents `h` (long along X),
/// centred: steel I-beams along the sides, cross beams at each bent, and
/// a bed of painted pipes of different sizes.
pub fn rack_bundle(h: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let steel = Paint::painted(0x5e6266, 0.75);
    let len = 2.0 * h.x;
    // Side beams (I-section: web and flanges).
    for z in [-(h.z - 0.25), h.z - 0.25] {
        m.bevel_box(Vec3::new(h.x, 0.32, 0.06), 0.02, 1, steel, at(Vec3::new(0.0, -h.y + 0.32, z)));
        for fy in [-h.y + 0.04, -h.y + 0.6] {
            m.bevel_box(Vec3::new(h.x, 0.04, 0.2), 0.015, 1, steel, at(Vec3::new(0.0, fy, z)));
        }
    }
    // Cross beams where the legs stand (same spacing as the colliders).
    let bents = (len / 14.0).round().max(1.0) as i32;
    for k in 0..=bents {
        let x = -len / 2.0 + 0.4 + k as f32 * (len - 0.8) / bents as f32;
        m.bevel_box(Vec3::new(0.2, 0.3, h.z), 0.03, 1, steel, at(Vec3::new(x, -h.y + 0.3, 0.0)));
        // A hazard-striped strip under each cross beam (headroom marker).
        m.bevel_box(Vec3::new(0.21, 0.08, h.z - 0.3), 0.02, 1, hazard(), at(Vec3::new(x, -h.y + 0.0, 0.0)));
    }
    // Intermediate cross beams.
    let mut x = -h.x + 3.5;
    while x < h.x - 1.0 {
        m.bevel_box(Vec3::new(0.1, 0.12, h.z - 0.2), 0.02, 1, steel, at(Vec3::new(x, -h.y + 0.62, 0.0)));
        x += 3.5;
    }
    // The pipes: (z, radius, colour), resting on the beams.
    let pipes = [(-4.2f32, 0.42f32, 0x8a8a82u32), (-3.1, 0.28, 0xd6a21e), (-2.2, 0.36, 0x7a3a26), (-1.1, 0.5, 0xc8c6be), (0.2, 0.3, 0x3f6e4a), (1.2, 0.45, 0x8a8a82), (2.4, 0.26, 0xb02a20), (3.3, 0.36, 0x5a5e62), (4.3, 0.3, 0xc8c6be)];
    for (z, r, c) in pipes {
        let z = z * (h.z / 5.4);
        let y = -h.y + 0.72 + r;
        m.cylinder(r, h.x, 0.03, 1, 12, Paint::painted(c, 0.6), along_x(Vec3::new(0.0, y, z)));
        // Flanges along the run.
        let mut fx = -h.x + 6.0;
        while fx < h.x - 2.0 {
            m.cylinder(r + 0.07, 0.06, 0.02, 1, 12, dark_steel(), along_x(Vec3::new(fx, y, z)));
            fx += 12.0;
        }
    }
    m
}

/// One leg of a pipe rack, half extents `h`, centred: a steel column on a
/// base plate with bolts and a knee brace up to the bundle.
pub fn rack_leg(h: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let steel = Paint::painted(0x5e6266, 0.75);
    m.bevel_box(Vec3::new(h.x * 0.9, h.y, 0.06), 0.02, 1, steel, Mat4::IDENTITY);
    for z in [-h.z + 0.04, h.z - 0.04] {
        m.bevel_box(Vec3::new(h.x, h.y, 0.035), 0.012, 1, steel, at(Vec3::new(0.0, 0.0, z)));
    }
    m.bevel_box(Vec3::new(0.5, 0.05, 0.5), 0.02, 1, dark_steel(), at(Vec3::Y * (-h.y + 0.05)));
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        m.rivet(Vec3::new(sx * 0.38, -h.y + 0.1, sz * 0.38), Vec3::Y, 0.07, Paint::steel(0x9a958c, 0.4));
    }
    // A concrete pedestal with a hazard band (bumpers find it).
    m.bevel_box(Vec3::new(0.45, 0.35, 0.45), 0.05, 1, concrete(), at(Vec3::Y * (-h.y + 0.35)));
    m.bevel_box(Vec3::new(h.x + 0.02, 0.35, h.z + 0.02), 0.02, 1, hazard(), at(Vec3::Y * (-h.y + 1.1)));
    m
}

pub const PUMPJACK_PIVOT: Vec3 = Vec3::new(0.3, 4.3, 0.0);
pub const PUMPJACK_CRANK: Vec3 = Vec3::new(-2.5, 1.25, 0.0);

/// The fixed part of a pumpjack in the obstacle frame (standing on y = 0):
/// the skid, the gearbox and motor, the samson post, the wellhead.
pub fn pumpjack_base() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let frame = Paint::painted(0x4a4e52, 0.7);
    let red = Paint::painted(0xb8432a, 0.6);
    m.bevel_box(Vec3::new(3.6, 0.15, 0.9), 0.04, 1, frame, at(Vec3::new(-0.6, 0.15, 0.0)));
    for z in [-0.75f32, 0.75] {
        m.bevel_box(Vec3::new(3.6, 0.12, 0.1), 0.03, 1, hazard(), at(Vec3::new(-0.6, 0.34, z)));
    }
    // Gearbox and motor.
    m.bevel_box(Vec3::new(0.65, 0.55, 0.5), 0.08, 2, red, at(Vec3::new(-2.5, 0.85, 0.0)));
    m.bevel_box(Vec3::new(0.45, 0.35, 0.35), 0.06, 2, Paint::painted(0x2f5f9a, 0.6), at(Vec3::new(-3.6, 0.65, 0.0)));
    m.cylinder(0.1, 0.5, 0.02, 1, 8, dark_steel(), along_z(PUMPJACK_CRANK));
    // Samson post: an A-frame of four legs to the pivot.
    let top = PUMPJACK_PIVOT - Vec3::Y * 0.2;
    for (x, z) in [(-0.6f32, -0.85f32), (-0.6, 0.85), (1.2, -0.85), (1.2, 0.85)] {
        octo_pipe(&mut m, Vec3::new(x, 0.3, z), top + Vec3::new(0.0, 0.0, z * 0.15), 0.09, frame);
    }
    m.bevel_box(Vec3::new(0.3, 0.2, 0.35), 0.04, 1, frame, at(top));
    for y in [1.4f32, 2.6] {
        let t = (y - 0.3) / (top.y - 0.3);
        let s = 0.85 * (1.0 - t) + 0.13 * t;
        m.bevel_box(Vec3::new(0.05, 0.05, s), 0.01, 1, frame, at(Vec3::new(0.3 + 0.0 * t, y, 0.0)));
    }
    // The wellhead under the horsehead: a little valve tree.
    let w = Vec3::new(3.75, 0.0, 0.0);
    m.cylinder(0.35, 0.15, 0.04, 1, 12, concrete(), at(w + Vec3::Y * 0.15));
    m.cylinder(0.12, 0.6, 0.02, 1, 10, Paint::steel(0x7a766e, 0.4), at(w + Vec3::Y * 0.85));
    for y in [0.7f32, 1.15] {
        m.torus(0.16, 0.03, 0.06, 10, Paint::painted(0xb02a20, 0.5), Mat4::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), w + Vec3::new(0.0, y, 0.22)));
        m.cylinder(0.05, 0.1, 0.01, 1, 6, dark_steel(), along_z(w + Vec3::new(0.0, y, 0.12)));
    }
    m
}

/// The walking beam and horsehead, in the beam's frame (origin at the
/// samson-post pivot): it rocks about Z.
pub fn pumpjack_beam() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let red = Paint::painted(0xb8432a, 0.6);
    let frame = Paint::painted(0x4a4e52, 0.7);
    m.bevel_box(Vec3::new(3.0, 0.24, 0.18), 0.05, 2, red, at(Vec3::new(0.1, 0.1, 0.0)));
    // Horsehead: a curved plate built from facets.
    let hx = 3.15;
    for k in 0..6 {
        let a0 = -0.9 + k as f32 * 0.3;
        let p = Vec3::new(hx + a0.cos() * 0.55 - 0.2, a0.sin() * 1.0, 0.0);
        m.bevel_box(Vec3::new(0.12, 0.2, 0.28), 0.04, 1, red, Mat4::from_rotation_translation(Quat::from_rotation_z(a0), p));
    }
    m.bevel_box(Vec3::new(0.35, 0.8, 0.22), 0.06, 2, red, at(Vec3::new(hx - 0.35, -0.05, 0.0)));
    // The bridle down to the polished rod.
    for z in [-0.08f32, 0.08] {
        m.bevel_box(Vec3::new(0.02, 1.4, 0.02), 0.005, 1, dark_steel(), at(Vec3::new(hx + 0.32, -2.2, z)));
    }
    m.bevel_box(Vec3::new(0.12, 0.05, 0.16), 0.02, 1, dark_steel(), at(Vec3::new(hx + 0.32, -3.55, 0.0)));
    // Rear: the equaliser and the pitman arms hanging to the crank.
    m.bevel_box(Vec3::new(0.25, 0.2, 0.7), 0.04, 1, frame, at(Vec3::new(-2.8, -0.1, 0.0)));
    for z in [-0.62f32, 0.62] {
        m.bevel_box(Vec3::new(0.06, 1.4, 0.05), 0.02, 1, frame, at(Vec3::new(-2.8, -1.5, z)));
    }
    // Hazard stripes on the beam's sides.
    for z in [-0.19f32, 0.19] {
        m.bevel_box(Vec3::new(0.6, 0.12, 0.01), 0.01, 1, hazard(), at(Vec3::new(-1.6, 0.1, z)));
    }
    m
}

/// The crank and its counterweights, in the crank's frame (it turns about Z).
pub fn pumpjack_crank() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let weight = Paint::painted(0x3a3a3a, 0.7);
    for z in [-0.62f32, 0.62] {
        m.bevel_box(Vec3::new(0.12, 0.85, 0.07), 0.03, 1, Paint::painted(0x4a4e52, 0.7), at(Vec3::new(0.0, 0.3, z)));
        m.bevel_box(Vec3::new(0.55, 0.32, 0.12), 0.06, 2, weight, at(Vec3::new(0.0, -0.45, z)));
        m.bevel_box(Vec3::new(0.56, 0.08, 0.125), 0.02, 1, hazard(), at(Vec3::new(0.0, -0.45, z)));
    }
    m
}

/// A drilling derrick in the obstacle frame: four tapering legs of lattice,
/// a drill floor, the crown block and a hanging travelling block.
pub fn derrick(base_half: f32, top_y: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let paint = Paint::painted(0xa8562c, 0.8);
    let top_half = 1.0;
    let corner = |sx: f32, sz: f32, y: f32| {
        let t = y / top_y;
        let s = base_half * (1.0 - t) + top_half * t;
        Vec3::new(sx * s, y, sz * s)
    };
    let corners = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for (sx, sz) in corners {
        octo_pipe(&mut m, corner(sx, sz, 0.0), corner(sx, sz, top_y), 0.14, paint);
        m.bevel_box(Vec3::new(0.4, 0.2, 0.4), 0.04, 1, concrete(), at(corner(sx, sz, 0.1)));
    }
    // Girts and X-braces on every face, every 3 m.
    let mut y = 2.6;
    let mut k = 0;
    while y < top_y - 1.0 {
        let y1 = (y + 3.0).min(top_y);
        for i in 0..4 {
            let (ax, az) = corners[i];
            let (bx, bz) = corners[(i + 1) % 4];
            octo_pipe(&mut m, corner(ax, az, y), corner(bx, bz, y), 0.06, paint);
            if k % 2 == 0 || y > 9.0 {
                octo_pipe(&mut m, corner(ax, az, y), corner(bx, bz, y1), 0.045, paint);
                octo_pipe(&mut m, corner(bx, bz, y), corner(ax, az, y1), 0.045, paint);
            }
        }
        y = y1;
        k += 1;
    }
    // Drill floor (high: tanks pass under) and a monkey board.
    m.bevel_box(Vec3::new(2.9, 0.1, 2.9), 0.03, 1, Paint::steel(0x5a5650, 0.5), at(Vec3::Y * 9.4));
    m.bevel_box(Vec3::new(3.0, 0.08, 0.1), 0.02, 1, hazard(), at(Vec3::new(0.0, 9.55, 2.9)));
    m.bevel_box(Vec3::new(1.6, 0.06, 1.6), 0.02, 1, Paint::steel(0x5a5650, 0.5), at(Vec3::Y * 17.0));
    // Crown block and the travelling block on its lines.
    m.bevel_box(Vec3::new(1.2, 0.35, 1.2), 0.06, 2, Paint::painted(0xd6a21e, 0.7), at(Vec3::Y * (top_y + 0.35)));
    for z in [-0.2f32, 0.0, 0.2] {
        m.bevel_box(Vec3::new(0.015, 3.5, 0.015), 0.005, 1, dark_steel(), at(Vec3::new(0.0, top_y - 3.5, z)));
    }
    m.bevel_box(Vec3::new(0.35, 0.6, 0.3), 0.08, 2, Paint::painted(0xd6a21e, 0.7), at(Vec3::Y * (top_y - 7.3)));
    m.cylinder(0.08, 0.5, 0.02, 1, 8, dark_steel(), at(Vec3::Y * (top_y - 8.3)));
    // A red light on top.
    m.cylinder(0.14, 0.12, 0.06, 2, 10, Paint::glow(0xff2a10, 2.5), at(Vec3::Y * (top_y + 0.85)));
    m
}

/// A rail tank car, half extents `h` (long along X), centred: a domed
/// barrel on a riveted underframe and two bogies, a top hatch and walkway,
/// a company band and a hazard placard.
pub fn tank_car(h: Vec3, variant: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let body = Paint::painted([0x2c2a28, 0xc8c4bc, 0x7a3a26][variant as usize % 3], 0.6);
    let r = h.z - 0.05;
    let cy = h.y - r;
    let len = h.x - 0.3;
    let n = |x: f32, y: f32| Vec2::new(x, y).normalize();
    let profile = [
        (0.0, len + 0.45, Vec2::Y),
        (r * 0.6, len + 0.33, n(0.4, 1.0)),
        (r, len, n(1.0, 0.35)),
        (r, -len, n(1.0, -0.35)),
        (r * 0.6, -len - 0.33, n(0.4, -1.0)),
        (0.0, -len - 0.45, Vec2::NEG_Y),
    ];
    m.lathe(&profile, 22, false, body, along_x(Vec3::Y * cy));
    // Seams with rivets.
    for x in [-len * 0.5, 0.0, len * 0.5] {
        m.cylinder(r + 0.025, 0.04, 0.01, 1, 22, Paint::painted(0x4a4642, 0.7), along_x(Vec3::new(x, cy, 0.0)));
        for k in 0..12 {
            let a = k as f32 / 12.0 * std::f32::consts::TAU;
            let d = Vec3::new(0.0, a.sin(), a.cos());
            m.rivet(Vec3::new(x + 0.12, cy, 0.0) + d * (r + 0.02), d, 0.05, Paint::steel(0x8a857c, 0.5));
        }
    }
    // Company band round the middle and a hazard placard on each side.
    m.cylinder(r + 0.03, 1.1, 0.02, 1, 22, Paint::painted(COMPANY[(variant as usize + 1) % 3], 0.5), along_x(Vec3::new(-len * 0.25, cy, 0.0)));
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.45, 0.45, 0.02), 0.02, 1, hazard(), Mat4::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4), Vec3::new(len * 0.45, cy, s * (r + 0.03))));
    }
    // Dome hatch and walkway.
    m.cylinder(0.5, 0.25, 0.06, 2, 14, body, at(Vec3::new(0.0, cy + r + 0.15, 0.0)));
    m.bevel_box(Vec3::new(1.2, 0.04, 0.6), 0.02, 1, Paint::steel(0x5a5650, 0.5), at(Vec3::new(0.0, cy + r + 0.05, 0.0)));
    // Underframe, couplers and two bogies.
    let floor = -h.y + 1.0;
    m.bevel_box(Vec3::new(h.x, 0.18, 0.5), 0.04, 1, dark_steel(), at(Vec3::Y * floor));
    for s in [-1.0f32, 1.0] {
        m.bevel_box(Vec3::new(0.25, 0.15, 0.15), 0.03, 1, dark_steel(), at(Vec3::new(s * (h.x + 0.1), floor, 0.0)));
        let bx = s * (h.x - 2.3);
        m.bevel_box(Vec3::new(1.4, 0.22, h.z - 0.25), 0.05, 2, Paint::painted(0x3a3632, 0.8), at(Vec3::new(bx, -h.y + 0.65, 0.0)));
        for ax in [-0.85f32, 0.85] {
            for z in [-0.72f32, 0.72] {
                m.cylinder(0.42, 0.07, 0.03, 1, 14, Paint::steel(0x5a5650, 0.5), along_z(Vec3::new(bx + ax, -h.y + 0.42, z)));
            }
            m.cylinder(0.07, 0.85, 0.01, 1, 6, dark_steel(), along_z(Vec3::new(bx + ax, -h.y + 0.42, 0.0)));
        }
    }
    // End ladders.
    for s in [-1.0f32, 1.0] {
        for z in [-0.3f32, 0.3] {
            m.bevel_box(Vec3::new(0.03, 1.2, 0.03), 0.01, 1, Paint::painted(0xd6a21e, 0.7), at(Vec3::new(s * (len + 0.2), cy - 0.2, z)));
        }
    }
    m
}

/// A railway piece, half extents `h` (long along X), centred: a gravel
/// bed, timber sleepers and two steel rails.
pub fn rails(h: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(Vec3::new(h.x, 0.14, h.z), 0.1, 1, Paint::matte(0x8a8378), at(Vec3::Y * (h.y - 0.2)));
    let mut x = -h.x + 0.3;
    let mut i = 0;
    while x < h.x {
        let j = (hash(i, 77) - 0.5) * 0.1;
        m.bevel_box(Vec3::new(0.13, 0.08, 1.3), 0.02, 1, Paint::with(Material::Wood, 0x5a4434, 0.8), Mat4::from_rotation_translation(Quat::from_rotation_y(j), Vec3::new(x, h.y - 0.02, 0.0)));
        x += 0.65;
        i += 1;
    }
    for z in [-0.72f32, 0.72] {
        m.bevel_box(Vec3::new(h.x, 0.07, 0.04), 0.015, 1, Paint::steel(0x8a8680, 0.3), at(Vec3::new(0.0, h.y + 0.12, z)));
        m.bevel_box(Vec3::new(h.x, 0.025, 0.08), 0.01, 1, Paint::steel(0x5a5650, 0.5), at(Vec3::new(0.0, h.y + 0.05, z)));
    }
    m
}

/// The flare stack in the obstacle frame (standing on y = 0): a concrete
/// plinth, a chimney in red and white aviation bands with platforms and a
/// ladder, the flare tip with its wind shield, guy wires to three anchors.
pub fn flare_stack(plinth: (Vec3, Vec3), stack: (Vec3, Vec3)) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let (pc, ph) = plinth;
    m.bevel_box(ph, 0.12, 2, concrete(), at(pc));
    m.bevel_box(Vec3::new(ph.x + 0.02, 0.2, ph.z + 0.02), 0.03, 1, hazard(), at(Vec3::Y * (pc.y + ph.y - 0.5)));
    let (sc, sh) = stack;
    let bottom = sc.y - sh.y;
    let top = sc.y + sh.y;
    let r = sh.x * 0.85;
    // Bands, bottom to top: red and white, seven of them.
    let bands = 9;
    let bh = (top - bottom) / bands as f32;
    for k in 0..bands {
        let c = if k % 2 == 0 { 0xb8341f } else { 0xe8e4dc };
        let rr = r * (1.0 - 0.18 * k as f32 / bands as f32);
        m.cylinder(rr, bh * 0.5, 0.03, 1, 18, Paint::painted(c, 0.55), at(Vec3::Y * (bottom + bh * (k as f32 + 0.5))));
        m.cylinder(rr + 0.04, 0.05, 0.02, 1, 18, Paint::painted(0x4a4642, 0.7), at(Vec3::Y * (bottom + bh * k as f32)));
    }
    // Platforms with rails.
    for py in [bottom + bh * 4.0, top - 2.0] {
        let rr = r * 0.9;
        let deck = [(rr, 0.06, Vec2::Y), (rr + 1.1, 0.06, Vec2::Y), (rr + 1.1, -0.06, Vec2::NEG_Y), (rr, -0.06, Vec2::NEG_Y)];
        m.lathe(&deck, 18, true, Paint::steel(0x5a5650, 0.5), at(Vec3::Y * py));
        railing_ring(&mut m, rr + 1.05, py, Paint::painted(0xd6a21e, 0.7));
    }
    // Ladder.
    for z in [-0.25f32, 0.25] {
        octo_pipe(&mut m, Vec3::new(r + 0.3, bottom, z), Vec3::new(r * 0.85 + 0.3, top - 1.0, z), 0.04, Paint::painted(0xd6a21e, 0.7));
    }
    // The tip: a wider steel head and a wind shield, soot-black.
    m.cylinder(r * 0.95, 0.6, 0.05, 1, 16, Paint::steel(0x2a2624, 0.7), at(Vec3::Y * (top + 0.4)));
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        m.bevel_box(Vec3::new(0.05, 0.5, 0.32), 0.02, 1, Paint::steel(0x1e1a18, 0.8), Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::X, d), d * r * 1.05 + Vec3::Y * (top + 1.3)));
    }
    // Guy wires to three anchors.
    for k in 0..3 {
        let a = k as f32 / 3.0 * std::f32::consts::TAU + 0.5;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let anchor = d * 17.0;
        m.bevel_box(Vec3::new(0.5, 0.3, 0.5), 0.05, 1, concrete(), at(anchor + Vec3::Y * 0.2));
        m.tube(&[anchor + Vec3::Y * 0.5, d * r + Vec3::Y * (bottom + bh * 6.0)], 0.03, 4, Paint::steel(0x2a2826, 0.6));
    }
    // Pilot light (always lit).
    m.cylinder(0.12, 0.1, 0.05, 1, 8, Paint::glow(0xffb040, 3.0), at(Vec3::Y * (top + 0.95)));
    m
}

/// The control house, half extents `h` (long along X), centred: a low
/// blast-proof concrete block with a parapet, slit windows, a steel door
/// in a hazard frame, roof units and a radio mast.
pub fn control_house(h: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(h, 0.12, 2, Paint::matte(0xcfc6b4), Mat4::IDENTITY);
    m.bevel_box(Vec3::new(h.x + 0.1, 0.22, h.z + 0.1), 0.05, 1, Paint::matte(0xb8ae9c), at(Vec3::Y * (h.y - 0.1)));
    m.bevel_box(Vec3::new(h.x + 0.05, 0.35, h.z + 0.05), 0.04, 1, Paint::painted(COMPANY[0], 0.5), at(Vec3::Y * (h.y - 0.75)));
    // Slit windows on the long sides.
    for s in [-1.0f32, 1.0] {
        let mut x = -h.x + 1.6;
        while x < h.x - 1.0 {
            m.bevel_box(Vec3::new(0.7, 0.18, 0.04), 0.02, 1, Paint::steel(0x1a2228, 0.25), at(Vec3::new(x, 0.4, s * (h.z + 0.01))));
            x += 2.2;
        }
    }
    // The door, at +X.
    m.bevel_box(Vec3::new(0.04, 1.1, 0.7), 0.02, 1, Paint::steel(0x3a4248, 0.6), at(Vec3::new(h.x + 0.02, -h.y + 1.1, 0.0)));
    m.bevel_box(Vec3::new(0.03, 1.25, 0.85), 0.02, 1, hazard(), at(Vec3::new(h.x + 0.005, -h.y + 1.25, 0.0)));
    // Roof units, ducting and a radio mast.
    for (x, z) in [(-h.x * 0.5, -h.z * 0.3), (h.x * 0.15, h.z * 0.25)] {
        m.bevel_box(Vec3::new(0.9, 0.45, 0.7), 0.06, 2, Paint::painted(0x8a8e8e, 0.6), at(Vec3::new(x, h.y + 0.45, z)));
        m.cylinder(0.32, 0.03, 0.01, 1, 12, dark_steel(), at(Vec3::new(x, h.y + 0.92, z)));
    }
    let mast = Vec3::new(h.x * 0.7, h.y, -h.z * 0.5);
    octo_pipe(&mut m, mast, mast + Vec3::Y * 6.0, 0.07, Paint::steel(0x8a8680, 0.4));
    m.bevel_box(Vec3::new(0.5, 0.03, 0.03), 0.01, 1, Paint::steel(0x8a8680, 0.4), at(mast + Vec3::Y * 5.0));
    m.cylinder(0.08, 0.08, 0.04, 1, 8, Paint::glow(0xff2a10, 2.5), at(mast + Vec3::Y * 6.1));
    m
}

/// A pipe on two concrete sleepers for the open ground (visual dressing).
pub fn ground_pipe(len: f32, colour: u32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.cylinder(0.25, len * 0.5, 0.03, 1, 10, Paint::painted(colour, 0.7), along_x(Vec3::Y * 0.55));
    for x in [-len * 0.35, len * 0.35] {
        m.bevel_box(Vec3::new(0.2, 0.15, 0.4), 0.03, 1, concrete(), at(Vec3::new(x, 0.15, 0.0)));
    }
    m
}
