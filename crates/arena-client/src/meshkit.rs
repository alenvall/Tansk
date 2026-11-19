//! The procedural mesh kit: every model in the game is built from these.
//!
//! Primitives are bevelled (rounded edges with smooth normals across the
//! bevel, flat faces between). The bevel band is also where the shader finds
//! an *edge*: the smooth normal there leans away from the geometric face
//! normal, and that lean drives edge wear (chipped paint, bare steel), so a
//! welded plate's edges read as handled and weathered without any textures.
//!
//! All primitives are appended to a [`MeshBuilder`] with a [`Paint`] (colour,
//! roughness, material, wear) and an affine transform, so a whole tank is one
//! mesh.

use glam::{Mat3, Mat4, Vec2, Vec3};

/// Shader material ids. Must match `scene.wgsl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// Painted steel: sun-bleached paint over metal, chipping at edges to
    /// bare steel, rust patches and streaks, dust on upward faces.
    PaintedSteel = 0,
    /// The desert floor, procedural in world space.
    Sand = 1,
    /// Bare steel (worn, slightly oily): metallic.
    BareSteel = 2,
    /// Rubber (track runs, tyres): matte, dusty.
    Rubber = 3,
    /// Stylized rock with painted horizontal strata.
    Rock = 4,
    /// Team paint: painted steel with a soft two-tone camo.
    TeamPaint = 5,
    /// Painted wooden planks (crates).
    Wood = 6,
    /// Plain matte paint with soft variation (plants, concrete).
    Matte = 7,
    /// Yellow and black hazard stripes.
    Hazard = 8,
    /// Cracked asphalt with lane markings (in the deck's local frame).
    Asphalt = 9,
    /// Self-lit: the colour is emitted (coils, lamps, glowing tips).
    Glow = 10,
}

/// What a surface is made of and how worn it is.
#[derive(Clone, Copy, Debug)]
pub struct Paint {
    /// Linear RGB albedo (the paint colour for painted steel).
    pub color: Vec3,
    pub roughness: f32,
    pub material: Material,
    /// 0 fresh .. 1 rusted through.
    pub wear: f32,
}

impl Paint {
    pub fn painted(srgb: u32, wear: f32) -> Self {
        Self { color: srgb_hex(srgb), roughness: 0.55, material: Material::PaintedSteel, wear }
    }
    pub fn steel(srgb: u32, roughness: f32) -> Self {
        Self { color: srgb_hex(srgb), roughness, material: Material::BareSteel, wear: 0.3 }
    }
    pub fn rubber(srgb: u32) -> Self {
        Self { color: srgb_hex(srgb), roughness: 0.85, material: Material::Rubber, wear: 0.5 }
    }
    pub fn team(srgb: u32, wear: f32) -> Self {
        Self { color: srgb_hex(srgb), roughness: 0.5, material: Material::TeamPaint, wear }
    }
    pub fn matte(srgb: u32) -> Self {
        Self { color: srgb_hex(srgb), roughness: 0.9, material: Material::Matte, wear: 0.0 }
    }
    pub fn glow(srgb: u32, strength: f32) -> Self {
        Self { color: srgb_hex(srgb) * strength, roughness: 0.5, material: Material::Glow, wear: 0.0 }
    }
    pub fn with(material: Material, srgb: u32, roughness: f32) -> Self {
        Self { color: srgb_hex(srgb), roughness, material, wear: 0.0 }
    }
}

/// `0xRRGGBB` sRGB to linear.
pub fn srgb_hex(hex: u32) -> Vec3 {
    let c = |v: u32| {
        let s = ((hex >> v) & 0xff) as f32 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    Vec3::new(c(16), c(8), c(0))
}

/// `params.x` base of a painted corrugated sheet's front face (see
/// [`MeshBuilder::corrugated`]); painted steel has no use for roughness.
pub const CORRUGATED_FRONT: f32 = 100.0;
/// ... and of its back face.
pub const CORRUGATED_BACK: f32 = 300.0;

/// One vertex as the GPU sees it. Matches `VertexIn` in `scene.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    /// x: roughness, y: material id, z: wear, w: height of the mesh's
    /// ground above this vertex's frame (0, except in merged chunks, where
    /// each piece keeps its own "height above the ground" for the shader).
    pub params: [f32; 4],
}

/// CPU-side mesh under construction.
#[derive(Default, Clone)]
pub struct MeshBuilder {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push(&mut self, p: Vec3, n: Vec3, paint: Paint) -> u32 {
        let i = self.vertices.len() as u32;
        self.vertices.push(Vertex {
            position: p.to_array(),
            normal: n.normalize_or_zero().to_array(),
            color: paint.color.to_array(),
            params: [paint.roughness, paint.material as u32 as f32, paint.wear, 0.0],
        });
        i
    }

    /// A triangle, wound counter-clockwise as seen from outside. The winding
    /// is fixed up from the vertex normals, so generators need not care.
    pub(crate) fn tri(&mut self, a: u32, b: u32, c: u32) {
        let va = &self.vertices;
        let p = |i: u32| Vec3::from(va[i as usize].position);
        let n = |i: u32| Vec3::from(va[i as usize].normal);
        let face = (p(b) - p(a)).cross(p(c) - p(a));
        if face.length_squared() < 1e-14 {
            return; // collapsed by a full-radius bevel
        }
        if face.dot(n(a) + n(b) + n(c)) >= 0.0 {
            self.indices.extend_from_slice(&[a, b, c]);
        } else {
            self.indices.extend_from_slice(&[a, c, b]);
        }
    }

    fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }

    /// Applies a transform to everything appended so far.
    pub(crate) fn transform_from(&mut self, first_vertex: usize, transform: Mat4) {
        let normal_m = Mat3::from_mat4(transform).inverse().transpose();
        for v in &mut self.vertices[first_vertex..] {
            v.position = transform.transform_point3(Vec3::from(v.position)).to_array();
            v.normal = (normal_m * Vec3::from(v.normal)).normalize_or_zero().to_array();
        }
    }

    // ── Primitives ──────────────────────────────────────────────────────

    /// A box of half extents `half`, every edge rounded with radius `bevel`
    /// in `segments` steps (0 = sharp box, 1 = chamfer, 2–3 = round).
    pub fn bevel_box(&mut self, half: Vec3, bevel: f32, segments: u32, paint: Paint, transform: Mat4) {
        let first = self.vertices.len();
        self.rounded_box_local(half, bevel, segments, paint, |p, n| (p, n));
        self.transform_from(first, transform);
    }

    /// A wedge: a bevelled box whose top slopes down towards +X, the front
    /// ending at `front_height` (fraction of the full height, 0..1). The
    /// bottom stays flat. Good for glacis plates, ramps and roofs.
    pub fn wedge(
        &mut self,
        half: Vec3,
        bevel: f32,
        segments: u32,
        front_height: f32,
        paint: Paint,
        transform: Mat4,
    ) {
        let first = self.vertices.len();
        let k = front_height.clamp(0.05, 1.0);
        // Height scale s(x) runs from 1 at the back (-x) to k at the front.
        let ds = (k - 1.0) / (2.0 * half.x);
        self.rounded_box_local(half, bevel, segments, paint, |p, n| {
            let s = 1.0 + (p.x + half.x) * ds;
            let up = p.y + half.y; // height above the bottom
            let q = Vec3::new(p.x, -half.y + up * s, p.z);
            // Inverse-transpose of the deformation's Jacobian on the normal.
            let m = Vec3::new(n.x - up * ds / s * n.y, n.y / s, n.z);
            (q, m)
        });
        self.transform_from(first, transform);
    }

    fn rounded_box_local(
        &mut self,
        half: Vec3,
        bevel: f32,
        segments: u32,
        paint: Paint,
        deform: impl Fn(Vec3, Vec3) -> (Vec3, Vec3),
    ) {
        let r = bevel.min(half.min_element()).max(0.0);
        let segs = if r <= 0.0 { 0 } else { segments.max(1) };
        let inner = half - Vec3::splat(r);
        // Grid coordinates along one axis of a face: the bevel band is
        // sampled at equal angles (tan spacing), so normals turn evenly.
        let coords = |h: f32| -> Vec<f32> {
            let mut c = Vec::new();
            if segs == 0 {
                return vec![-h, h];
            }
            for k in (0..=segs).rev() {
                let t = (k as f32 / segs as f32 * std::f32::consts::FRAC_PI_4).tan();
                c.push(-(h - r) - r * t);
            }
            for k in 0..=segs {
                let t = (k as f32 / segs as f32 * std::f32::consts::FRAC_PI_4).tan();
                c.push((h - r) + r * t);
            }
            c
        };
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let cu = coords(half[u]);
                let cv = coords(half[v]);
                let base = self.vertices.len() as u32;
                for &b in &cv {
                    for &a in &cu {
                        let mut p = Vec3::ZERO;
                        p[axis] = sign * half[axis];
                        p[u] = a;
                        p[v] = b;
                        let (pos, nrm) = if r > 0.0 {
                            let c = p.clamp(-inner, inner);
                            let d = (p - c).normalize();
                            (c + d * r, d)
                        } else {
                            let mut n = Vec3::ZERO;
                            n[axis] = sign;
                            (p, n)
                        };
                        let (pos, nrm) = deform(pos, nrm);
                        self.push(pos, nrm, paint);
                    }
                }
                let w = cu.len() as u32;
                for j in 0..cv.len() as u32 - 1 {
                    for i in 0..w - 1 {
                        let a = base + j * w + i;
                        self.quad(a, a + 1, a + 1 + w, a + w);
                    }
                }
            }
        }
    }

    /// A lathe: `profile` is a list of (radius, height, normal-in-rz-plane)
    /// points swept around +Y in `segments` steps. With `faceted`, each
    /// side panel gets its own flat normal around the axis (an octagonal
    /// turret); otherwise normals are smooth around.
    pub fn lathe(&mut self, profile: &[(f32, f32, Vec2)], segments: u32, faceted: bool, paint: Paint, transform: Mat4) {
        let first = self.vertices.len();
        let tau = std::f32::consts::TAU;
        let ring = |r: f32, y: f32, a: f32| Vec3::new(r * a.cos(), y, r * a.sin());
        for w in profile.windows(2) {
            let (r0, y0, n0) = w[0];
            let (r1, y1, n1) = w[1];
            for s in 0..segments {
                let a0 = s as f32 / segments as f32 * tau;
                let a1 = (s + 1) as f32 / segments as f32 * tau;
                let am = 0.5 * (a0 + a1);
                let nrm = |n: Vec2, a: f32| Vec3::new(n.x * a.cos(), n.y, n.x * a.sin());
                let (na0, na1) = if faceted { (am, am) } else { (a0, a1) };
                let i0 = self.push(ring(r0, y0, a0), nrm(n0, na0), paint);
                let i1 = self.push(ring(r0, y0, a1), nrm(n0, na1), paint);
                let i2 = self.push(ring(r1, y1, a1), nrm(n1, na1), paint);
                let i3 = self.push(ring(r1, y1, a0), nrm(n1, na0), paint);
                self.quad(i0, i1, i2, i3);
            }
        }
        self.transform_from(first, transform);
    }

    /// A cylinder along +Y (centred), radius `radius`, half height
    /// `half_height`, both cap edges rounded by `bevel` in `bevel_segments`.
    pub fn cylinder(
        &mut self,
        radius: f32,
        half_height: f32,
        bevel: f32,
        bevel_segments: u32,
        segments: u32,
        paint: Paint,
        transform: Mat4,
    ) {
        let profile = bevelled_cylinder_profile(radius, half_height, bevel, bevel_segments);
        self.lathe(&profile, segments, false, paint, transform);
    }
}

impl MeshBuilder {
    /// A corrugated metal sheet in the XY plane (corrugations run along Y),
    /// `width` x `height`, waves of `pitch` and `depth`, `thickness` apart
    /// front and back. Edges are left open; nobody looks at a sheet's rim.
    ///
    /// Painted sheets carry the sheet's flat normal and their wave phase
    /// (`params.x` = [`CORRUGATED_FRONT`] or [`CORRUGATED_BACK`] + cycles
    /// across), so the shader rebuilds the wavy normal up close and fades it
    /// out once a wave gets thinner than a pixel: far stacks read as clean
    /// painted metal, not moire.
    pub fn corrugated(&mut self, width: f32, height: f32, pitch: f32, depth: f32, thickness: f32, paint: Paint, transform: Mat4) {
        let first = self.vertices.len();
        let steps = ((width / pitch) * 8.0).ceil().max(2.0) as u32;
        let k = std::f32::consts::TAU / pitch;
        let painted = matches!(paint.material, Material::PaintedSteel | Material::TeamPaint);
        for side in [1.0f32, -1.0] {
            let base = self.vertices.len() as u32;
            for i in 0..=steps {
                let x = -0.5 * width + width * i as f32 / steps as f32;
                let z = 0.5 * depth * (k * x).sin() + side * 0.5 * thickness;
                let dz = 0.5 * depth * k * (k * x).cos();
                let n = if painted { Vec3::Z * side } else { Vec3::new(-dz, 0.0, 1.0).normalize() * side };
                let a = self.push(Vec3::new(x, -0.5 * height, z), n, paint);
                let b = self.push(Vec3::new(x, 0.5 * height, z), n, paint);
                if painted {
                    let phase = if side > 0.0 { CORRUGATED_FRONT } else { CORRUGATED_BACK } + x / pitch;
                    self.vertices[a as usize].params[0] = phase;
                    self.vertices[b as usize].params[0] = phase;
                }
            }
            for i in 0..steps {
                let a = base + 2 * i;
                self.quad(a, a + 2, a + 3, a + 1);
            }
        }
        self.transform_from(first, transform);
    }

    /// A rivet or bolt head: a low dome of `radius` sitting on a surface
    /// at `at` whose outward normal is `normal`.
    pub fn rivet(&mut self, at: Vec3, normal: Vec3, radius: f32, paint: Paint) {
        let h = radius * 0.55;
        let profile = [
            (radius, -0.002, Vec2::new(0.6, 0.8).normalize()),
            (radius * 0.75, h * 0.7, Vec2::new(0.55, 0.85).normalize()),
            (radius * 0.4, h, Vec2::new(0.2, 1.0).normalize()),
            (0.0, h * 1.05, Vec2::Y),
        ];
        let rot = glam::Quat::from_rotation_arc(Vec3::Y, normal.normalize());
        self.lathe(&profile, 7, false, paint, Mat4::from_rotation_translation(rot, at));
    }

    /// A heightfield grid over `[-half, half]^2` on XZ, `cells` per side.
    pub fn heightfield(&mut self, half: f32, cells: u32, height: impl Fn(f32, f32) -> f32, paint: Paint) {
        let base = self.vertices.len() as u32;
        let step = 2.0 * half / cells as f32;
        let e = step * 0.5;
        for j in 0..=cells {
            for i in 0..=cells {
                let x = -half + i as f32 * step;
                let z = -half + j as f32 * step;
                let n = Vec3::new(
                    height(x - e, z) - height(x + e, z),
                    2.0 * e,
                    height(x, z - e) - height(x, z + e),
                );
                self.push(Vec3::new(x, height(x, z), z), n, paint);
            }
        }
        let w = cells + 1;
        for j in 0..cells {
            for i in 0..cells {
                let a = base + j * w + i;
                self.quad(a, a + 1, a + 1 + w, a + w);
            }
        }
    }
}

/// The (radius, height, normal) profile of a cylinder with rounded cap edges,
/// from the top centre down to the bottom centre.
pub fn bevelled_cylinder_profile(radius: f32, half_height: f32, bevel: f32, segs: u32) -> Vec<(f32, f32, Vec2)> {
    let b = bevel.min(radius).min(half_height).max(0.0);
    let segs = segs.max(1);
    let mut p = vec![(0.0, half_height, Vec2::Y)];
    if b > 0.0 {
        // Top edge: from facing up to facing out.
        for k in 0..=segs {
            let a = k as f32 / segs as f32 * std::f32::consts::FRAC_PI_2;
            let n = Vec2::new(a.sin(), a.cos());
            p.push((radius - b + b * n.x, half_height - b + b * n.y, n));
        }
        for k in 0..=segs {
            let a = k as f32 / segs as f32 * std::f32::consts::FRAC_PI_2;
            let n = Vec2::new(a.cos(), -a.sin());
            p.push((radius - b + b * n.x, -half_height + b + b * n.y, n));
        }
    } else {
        p.push((radius, half_height, Vec2::Y));
        p.push((radius, half_height, Vec2::X));
        p.push((radius, -half_height, Vec2::X));
        p.push((radius, -half_height, -Vec2::Y));
    }
    p.push((0.0, -half_height, -Vec2::Y));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bevel_box_is_closed_and_outward() {
        let mut m = MeshBuilder::new();
        m.bevel_box(Vec3::new(1.0, 0.5, 0.7), 0.1, 2, Paint::painted(0xff0000, 0.3), Mat4::IDENTITY);
        assert!(!m.indices.is_empty());
        // Every triangle faces away from the centre.
        for t in m.indices.chunks(3) {
            let p = |i: u32| Vec3::from(m.vertices[i as usize].position);
            let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            let c = (p(t[0]) + p(t[1]) + p(t[2])) / 3.0;
            assert!(n.dot(c) > 0.0);
        }
    }
}
