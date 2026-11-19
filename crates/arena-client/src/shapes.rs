//! Higher-level shapes on top of the mesh kit: faceted rocks and cliffs
//! (flat-shaded, big irregular planes), tubes along paths (wires, razor
//! coils, bent pipes), tori (tyres) and flat triangles.

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{bevelled_cylinder_profile, MeshBuilder, Paint};

/// A small deterministic hash in [0, 1].
pub fn hash(i: u32, salt: u32) -> f32 {
    let mut h = i.wrapping_mul(0x9e37_79b9) ^ salt.wrapping_mul(0x85eb_ca6b) ^ 0x27d4_eb2f;
    h ^= h >> 15;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 13;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 65535.0
}

impl MeshBuilder {
    /// One flat-shaded triangle, wound to face away from `inside`.
    pub fn flat_tri(&mut self, a: Vec3, b: Vec3, c: Vec3, inside: Vec3, paint: Paint) {
        let mut n = (b - a).cross(c - a);
        if n.length_squared() < 1e-12 {
            return;
        }
        if n.dot((a + b + c) / 3.0 - inside) < 0.0 {
            n = -n;
        }
        let ia = self.push(a, n, paint);
        let ib = self.push(b, n, paint);
        let ic = self.push(c, n, paint);
        self.tri(ia, ib, ic);
    }

    /// A faceted boulder: a jittered, squashed octahedron subdivided once,
    /// every triangle its own flat plane. `radius` is roughly its size.
    pub fn rock_blob(&mut self, seed: u32, radius: Vec3, paint: Paint, transform: Mat4) {
        let first = self.vertices.len();
        let base = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];
        let faces = [
            (0usize, 2usize, 4usize),
            (4, 2, 1),
            (1, 2, 5),
            (5, 2, 0),
            (4, 3, 0),
            (1, 3, 4),
            (5, 3, 1),
            (0, 3, 5),
        ];
        // Jitter a point by a hash of its quantised direction, so shared
        // vertices of neighbouring triangles move together (no cracks).
        let jit = |p: Vec3| -> Vec3 {
            let d = p.normalize();
            let key = ((d.x * 97.0).round() as i32 as u32)
                .wrapping_mul(73856093)
                ^ ((d.y * 97.0).round() as i32 as u32).wrapping_mul(19349663)
                ^ ((d.z * 97.0).round() as i32 as u32).wrapping_mul(83492791);
            let r = 0.78 + 0.4 * hash(key, seed);
            let mut q = d * r;
            // Flatten the bottom so it sits on the ground.
            q.y = q.y.max(-0.25);
            q * radius
        };
        for (a, b, c) in faces {
            let (a, b, c) = (base[a], base[b], base[c]);
            let (ab, bc, ca) = ((a + b).normalize(), (b + c).normalize(), (c + a).normalize());
            for (x, y, z) in [(a, ab, ca), (ab, b, bc), (ca, bc, c), (ab, bc, ca)] {
                self.flat_tri(jit(x), jit(y), jit(z), Vec3::ZERO, paint);
            }
        }
        self.transform_from(first, transform);
    }

    /// A faceted cliff block: an irregular polygon footprint extruded in
    /// `levels` bands, each ring jittered in and out so the walls break into
    /// big uneven planes, with a jagged flat-ish top. Footprint radius
    /// `radius` (x, z), height `height`.
    pub fn cliff(&mut self, seed: u32, radius: Vec2, height: f32, sides: u32, levels: u32, paint: Paint, transform: Mat4) {
        let first = self.vertices.len();
        let ring = |level: u32| -> Vec<Vec3> {
            let t = level as f32 / levels as f32;
            let y = height * t + if level > 0 && level < levels { (hash(level, seed + 7) - 0.5) * height / levels as f32 * 0.5 } else { 0.0 };
            // Taper a little towards the top; the top ring is inset most.
            let taper = 1.0 - 0.12 * t - if level == levels { 0.06 } else { 0.0 };
            (0..sides)
                .map(|i| {
                    let a = (i as f32 + 0.35 * (hash(i, seed) - 0.5)) / sides as f32 * std::f32::consts::TAU;
                    let r = taper * (0.75 + 0.4 * hash(i * 31 + level * 7, seed + 1));
                    let yj = if level == levels { height * 0.06 * (hash(i, seed + 3) - 0.5) } else { 0.0 };
                    Vec3::new(a.cos() * radius.x * r, y + yj, a.sin() * radius.y * r)
                })
                .collect()
        };
        let rings: Vec<Vec<Vec3>> = (0..=levels).map(ring).collect();
        let centre = Vec3::new(0.0, height * 0.5, 0.0);
        for l in 0..levels as usize {
            for i in 0..sides as usize {
                let j = (i + 1) % sides as usize;
                let (a, b, c, d) = (rings[l][i], rings[l][j], rings[l + 1][j], rings[l + 1][i]);
                // Split along alternating diagonals: more irregular facets.
                if (i + l) % 2 == 0 {
                    self.flat_tri(a, b, c, Vec3::new(0.0, a.y, 0.0), paint);
                    self.flat_tri(a, c, d, Vec3::new(0.0, a.y, 0.0), paint);
                } else {
                    self.flat_tri(a, b, d, Vec3::new(0.0, a.y, 0.0), paint);
                    self.flat_tri(b, c, d, Vec3::new(0.0, a.y, 0.0), paint);
                }
            }
        }
        // Top: a fan round a slightly raised, off-centre peak.
        let top = &rings[levels as usize];
        let peak = top.iter().copied().sum::<Vec3>() / sides as f32
            + Vec3::new(0.0, height * 0.04, 0.0);
        for i in 0..sides as usize {
            let j = (i + 1) % sides as usize;
            self.flat_tri(top[i], top[j], peak, peak - Vec3::Y * height, paint);
        }
        let _ = centre;
        self.transform_from(first, transform);
    }

    /// A tube of `radius` along a polyline, `sides` around (8 = octagonal,
    /// faceted). Each segment is a bevelled-cylinder piece, so joints are
    /// rounded over.
    pub fn tube(&mut self, points: &[Vec3], radius: f32, sides: u32, paint: Paint) {
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = b - a;
            let len = d.length();
            if len < 1e-5 {
                continue;
            }
            let profile = bevelled_cylinder_profile(radius, 0.5 * len + radius * 0.3, radius * 0.3, 1);
            let rot = Quat::from_rotation_arc(Vec3::Y, d / len);
            self.lathe(&profile, sides, sides <= 8, paint, Mat4::from_rotation_translation(rot, 0.5 * (a + b)));
        }
    }

    /// A torus around +Y (a tyre): ring radius `major`, tube radius `minor`,
    /// squashed to `width` across.
    pub fn torus(&mut self, major: f32, minor: f32, width: f32, segments: u32, paint: Paint, transform: Mat4) {
        let steps = 8;
        let profile: Vec<(f32, f32, Vec2)> = (0..=steps)
            .map(|k| {
                let a = k as f32 / steps as f32 * std::f32::consts::TAU + std::f32::consts::PI;
                let n = Vec2::new(-a.cos(), a.sin());
                (major - minor * a.cos(), width * 0.5 * a.sin(), n)
            })
            .collect();
        self.lathe(&profile, segments, true, paint, transform);
    }
}
