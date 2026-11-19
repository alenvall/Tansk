//! Terrain meshes: the map heightfield in chunks (soft facets: each
//! triangle's normal is half its own face, half the smooth surface, so the
//! dunes break into broad planes without hard creases), and a coarse outer
//! ring of dunes and rim out to the horizon.

use arena_core::Map;
use glam::{Mat4, Vec2, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};

const CHUNK_CELLS: usize = 15;
/// How much of the flat face normal goes into the shading normal.
const FACET: f32 = 0.55;

/// The ground paint. `salt` (the paint's wear channel) turns the sand into
/// a pale cracked salt flat in the shader (0 on the Sink).
fn sand(salt: f32) -> Paint {
    Paint { wear: salt, ..Paint::with(Material::Sand, 0xffffff, 0.95) }
}

thread_local! {
    static SALT: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
}

fn smooth_normal(h: &dyn Fn(f32, f32) -> f32, x: f32, z: f32, e: f32) -> Vec3 {
    Vec3::new(h(x - e, z) - h(x + e, z), 2.0 * e, h(x, z - e) - h(x, z + e)).normalize()
}

/// Emits one triangle with soft-faceted normals.
fn tri(m: &mut MeshBuilder, h: &dyn Fn(f32, f32) -> f32, e: f32, p: [Vec3; 3]) {
    let face = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or(Vec3::Y);
    let face = if face.y < 0.0 { -face } else { face };
    let paint = sand(SALT.with(|s| s.get()));
    let idx: Vec<u32> = p
        .iter()
        .map(|q| {
            let n = smooth_normal(h, q.x, q.z, e).lerp(face, FACET);
            m.push(*q, n, paint)
        })
        .collect();
    m.tri(idx[0], idx[1], idx[2]);
}

/// The map's terrain and its outer ring, as (mesh, transform) pieces.
pub fn build(map: &Map) -> Vec<(MeshBuilder, Mat4)> {
    SALT.with(|s| s.set(if map.id == arena_core::MapId::Refinery { 0.85 } else { 0.0 }));
    let res = map.resolution;
    let sp = map.spacing();
    let half = map.size / 2.0;
    let h = |x: f32, z: f32| map.height_at(x, z);
    let mut out = Vec::new();
    let cells = res - 1;
    let mut cz = 0;
    while cz < cells {
        let mut cx = 0;
        while cx < cells {
            let mut m = MeshBuilder::new();
            for iz in cz..(cz + CHUNK_CELLS).min(cells) {
                for ix in cx..(cx + CHUNK_CELLS).min(cells) {
                    let p = |i: usize, j: usize| Vec3::new(-half + i as f32 * sp, map.sample(i, j), -half + j as f32 * sp);
                    let (a, b, c, d) = (p(ix, iz), p(ix + 1, iz), p(ix + 1, iz + 1), p(ix, iz + 1));
                    // Split along (ix, iz+1)-(ix+1, iz), as the collider does.
                    tri(&mut m, &h, sp * 0.5, [a, b, d]);
                    tri(&mut m, &h, sp * 0.5, [b, c, d]);
                }
            }
            out.push((m, Mat4::IDENTITY));
            cx += CHUNK_CELLS;
        }
        cz += CHUNK_CELLS;
    }

    // Outer ring: coarse dunes rising past the rim, out to ~1.6 km.
    let border = |x: f32, z: f32| map.height_at(x.clamp(-half, half), z.clamp(-half, half));
    let far = move |x: f32, z: f32| -> f32 {
        let outside = (x.abs().max(z.abs()) - half).max(0.0);
        let dunes = (0.5 + 0.5 * (x * 0.011 + (z * 0.007).sin() * 2.0).sin()) * (0.5 + 0.5 * (z * 0.009 + 1.3).sin());
        border(x, z) + (outside / 300.0).min(1.0) * (6.0 + 22.0 * dunes) - if outside <= 0.0 { 2.0 } else { 0.0 }
    };
    let step = 40.0;
    let n = 80i32;
    let ext = step * n as f32 / 2.0;
    let mut m = MeshBuilder::new();
    for j in 0..n {
        for i in 0..n {
            let x0 = -ext + i as f32 * step;
            let z0 = -ext + j as f32 * step;
            // Skip cells wholly inside the map (the chunks cover them).
            if x0 > -half && x0 + step < half && z0 > -half && z0 + step < half {
                continue;
            }
            let p = |x: f32, z: f32| Vec3::new(x, far(x, z), z);
            let (a, b, c, d) = (p(x0, z0), p(x0 + step, z0), p(x0 + step, z0 + step), p(x0, z0 + step));
            tri(&mut m, &far, step * 0.5, [a, b, d]);
            tri(&mut m, &far, step * 0.5, [b, c, d]);
        }
    }
    out.push((m, Mat4::IDENTITY));
    let _ = Vec2::ZERO;
    out
}

/// A flat sand floor for lineups and tests.
pub fn flat(half: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.heightfield(half, 40, |_, _| 0.0, sand(0.0));
    m
}
