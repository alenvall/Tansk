//! Terrain meshes: the map heightfield in chunks (soft facets: each
//! triangle's normal is half its own face, half the smooth surface, so the
//! dunes break into broad planes without hard creases), and a coarse outer
//! ring of dunes and rim out to the horizon.

use arena_core::map::{Cover, GroundMarkKind, ObstacleKind};
use arena_core::Map;
use glam::{Mat4, Vec2, Vec3};

use crate::meshkit::{Material, MeshBuilder, Paint};
use crate::shapes::hash;

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

// ── Ground marks: tracks, packed ground, oil (visual only) ──────────────

/// How far each layer floats over the terrain (later layers on top).
const LIFT_FRINGE: f32 = 0.04;
const LIFT_PACKED: f32 = 0.06;
const LIFT_TRACK: f32 = 0.09;
const LIFT_RUT: f32 = 0.12;
const LIFT_OIL: f32 = 0.15;
/// The shader's "height above the ground" for the marks: high enough that
/// the matte paint gets no foot dust or contact darkening.
const MARK_HEIGHT: f32 = 1.5;

fn mark_paint(rgb: Vec3, roughness: f32) -> Paint {
    Paint { color: rgb, roughness, material: Material::GroundDecal, wear: 0.0 }
}

/// Is `p` on a flush deck (the highway, rails)? Marks stop at its edge.
fn on_deck(map: &Map, p: Vec2) -> bool {
    map.obstacles.iter().filter(|o| o.cover == Cover::Flat && o.kind != ObstacleKind::LaunchPlate).any(|o| {
        if Vec2::new(o.pos.x, o.pos.z).distance(p) > o.radius + 6.0 {
            return false;
        }
        o.world_boxes().any(|(c, r, h)| {
            let l = r.inverse() * (Vec3::new(p.x, c.y, p.y) - c);
            l.x.abs() < h.x + 0.6 && l.z.abs() < h.z + 0.6
        })
    })
}

/// A smooth path through `pts` (Catmull-Rom), sampled about every `step`
/// m: (point, unit direction).
fn spline(pts: &[Vec2], step: f32) -> Vec<(Vec2, Vec2)> {
    let mut out = Vec::new();
    if pts.len() < 2 {
        return out;
    }
    let n = pts.len();
    for i in 0..n - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(n - 1)];
        let k = (p1.distance(p2) / step).ceil().max(1.0) as usize;
        let last = if i == n - 2 { k + 1 } else { k };
        for j in 0..last {
            let t = j as f32 / k as f32;
            let (t2, t3) = (t * t, t * t * t);
            let a = 2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3;
            let b = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
            let p = 0.5 * ((2.0 * p1) + (p2 - p0) * t + a * t2 + b * t3);
            let d = 0.5 * ((p2 - p0) + a * 2.0 * t + b * 3.0 * t2);
            out.push((p, d.normalize_or(Vec2::X)));
        }
    }
    out
}

/// A band `width` wide along `path` (shifted `offset` to its left), draped
/// `lift` over the ground, its edges ragged by `rag` (a fraction of the half
/// width), `lanes` cells across. Quads over a deck are left out.
#[allow(clippy::too_many_arguments)]
fn band(m: &mut MeshBuilder, map: &Map, path: &[(Vec2, Vec2)], offset: f32, width: f32, lanes: usize, rag: f32, lift: f32, paint: Paint, seed: u32) {
    band_shaped(m, map, path, offset, width, lanes, rag, lift, paint, seed, false)
}

/// [`band`], optionally with rounded ends (a packed area, not a strip).
#[allow(clippy::too_many_arguments)]
fn band_shaped(m: &mut MeshBuilder, map: &Map, path: &[(Vec2, Vec2)], offset: f32, width: f32, lanes: usize, rag: f32, lift: f32, paint: Paint, seed: u32, round: bool) {
    let half = width / 2.0;
    let total = path.windows(2).map(|w| w[0].0.distance(w[1].0)).sum::<f32>().max(1e-3);
    let mut run = 0.0;
    let mut rows: Vec<Vec<(u32, bool)>> = Vec::new();
    for (i, (p, d)) in path.iter().enumerate() {
        if i > 0 {
            run += path[i - 1].0.distance(*p);
        }
        let side = Vec2::new(-d.y, d.x);
        // Rounded ends: the width eases off over the last half width.
        let ends = if round {
            let e = (run.min(total - run) / (0.35 * half)).min(1.0);
            (e * (2.0 - e)).sqrt().max(0.25)
        } else {
            1.0
        };
        // Each edge wanders on its own, smoothly, with a little grit.
        let wob = |s: u32| {
            let f = i as f32 * 0.37 + hash(s, seed) * 10.0;
            ((f.sin() * 0.45) + (f * 2.3 + 1.0).sin() * 0.3 + (f * 5.1 + 2.0).sin() * 0.15 + (hash(i as u32, seed + s) - 0.5) * 0.4) * rag
        };
        let (wl, wr) = (wob(1), wob(2));
        let mut row = Vec::with_capacity(lanes + 1);
        for c in 0..=lanes {
            let u = c as f32 / lanes as f32 * 2.0 - 1.0;
            let edge = (if u < 0.0 { 1.0 + wl * -u } else { 1.0 + wr * u }) * ends;
            let q = *p + side * (offset + u * half * edge);
            let y = map.height_at(q.x, q.y) + lift;
            let idx = m.push(Vec3::new(q.x, y, q.y), map.normal_at(q.x, q.y), paint);
            m.vertices[idx as usize].params[3] = MARK_HEIGHT - y;
            row.push((idx, on_deck(map, q)));
        }
        rows.push(row);
    }
    for i in 0..rows.len().saturating_sub(1) {
        for c in 0..lanes {
            let q = [rows[i][c], rows[i][c + 1], rows[i + 1][c + 1], rows[i + 1][c]];
            if q.iter().any(|v| v.1) {
                continue;
            }
            m.tri(q[0].0, q[1].0, q[2].0);
            m.tri(q[0].0, q[2].0, q[3].0);
        }
    }
}

/// One ragged disc of oil round `c`, draped over the ground.
fn oil_blob(m: &mut MeshBuilder, map: &Map, c: Vec2, r: f32, paint: Paint, seed: u32) {
    let n = 14u32;
    let put = |m: &mut MeshBuilder, q: Vec2| {
        let y = map.height_at(q.x, q.y) + LIFT_OIL;
        let i = m.push(Vec3::new(q.x, y, q.y), map.normal_at(q.x, q.y), paint);
        m.vertices[i as usize].params[3] = MARK_HEIGHT - y;
        i
    };
    let centre = put(m, c);
    let mut rings = [Vec::new(), Vec::new()];
    for (ri, f) in [0.5f32, 1.0].into_iter().enumerate() {
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let rr = r * (0.6 + 0.4 * hash(k, seed)) * f;
            rings[ri].push(put(m, c + Vec2::new(a.cos(), a.sin()) * rr));
        }
    }
    let (inner, outer) = (&rings[0], &rings[1]);
    for k in 0..n as usize {
        let j = (k + 1) % n as usize;
        m.tri(centre, inner[k], inner[j]);
        m.tri(inner[k], outer[k], outer[j]);
        m.tri(inner[k], outer[j], inner[j]);
    }
}

/// A splat of oil round `c`: a ragged disc and a few drips.
fn oil_splat(m: &mut MeshBuilder, map: &Map, c: Vec2, radius: f32, paint: Paint, seed: u32) {
    oil_blob(m, map, c, radius, paint, seed);
    for k in 0..3 {
        let a = hash(k, seed + 7) * std::f32::consts::TAU;
        let d = radius * (0.9 + 0.5 * hash(k, seed + 8));
        oil_blob(m, map, c + Vec2::new(a.cos(), a.sin()) * d, radius * (0.1 + 0.1 * hash(k, seed + 9)), paint, seed + 11 + k);
    }
}

/// The map's ground marks (`Map::marks`) as meshes, one per mark: dirt
/// tracks with two darker tyre ruts, packed ground, oil stains. Matte paint
/// draped just over the terrain, a shade darker than the ground round it.
pub fn marks(map: &Map) -> Vec<MeshBuilder> {
    let salt = map.id == arena_core::MapId::Refinery;
    // Linear albedos.
    // (A soft fringe in between the packed ground and the sand round it.)
    let (track, rut, packed, fringe) = if salt {
        (Vec3::new(0.42, 0.35, 0.28), Vec3::new(0.29, 0.23, 0.18), Vec3::new(0.45, 0.38, 0.31), Vec3::new(0.51, 0.44, 0.36))
    } else {
        (Vec3::new(0.40, 0.275, 0.165), Vec3::new(0.27, 0.18, 0.11), Vec3::new(0.48, 0.34, 0.215), Vec3::new(0.53, 0.375, 0.235))
    };
    let oil = mark_paint(Vec3::new(0.11, 0.085, 0.06), 0.45);
    let mut out = Vec::new();
    for (mi, mark) in map.marks.iter().enumerate() {
        let seed = mi as u32 * 977 + 5;
        let mut m = MeshBuilder::new();
        match mark.kind {
            GroundMarkKind::Track => {
                let path = spline(&mark.points, 2.0);
                band(&mut m, map, &path, 0.0, mark.width, 2, 0.35, LIFT_TRACK, mark_paint(track, 0.95), seed);
                for side in [-1.15f32, 1.15] {
                    band(&mut m, map, &path, side, 0.75, 1, 0.25, LIFT_RUT, mark_paint(rut, 0.9), seed + 3);
                }
            }
            GroundMarkKind::Packed => {
                // Run the band out past its ends, then round them off.
                let (a, b) = (mark.points[0], mark.points[mark.points.len() - 1]);
                let d = (b - a).normalize_or(Vec2::X) * mark.width * 0.1;
                let mut pts = mark.points.clone();
                pts[0] = a - d;
                let last = pts.len() - 1;
                pts[last] = b + d;
                let path = spline(&pts, 2.5);
                let lanes = (mark.width / 2.5).ceil().max(2.0) as usize;
                band_shaped(&mut m, map, &path, 0.0, mark.width + 7.0, lanes, 0.12, LIFT_FRINGE, mark_paint(fringe, 0.95), seed + 1, true);
                band_shaped(&mut m, map, &path, 0.0, mark.width, lanes, 0.1, LIFT_PACKED, mark_paint(packed, 0.95), seed, true);
            }
            GroundMarkKind::Oil => {
                for (k, p) in mark.points.iter().enumerate() {
                    oil_splat(&mut m, map, *p, mark.width * 0.5, oil, seed + k as u32 * 31);
                }
            }
        }
        if !m.indices.is_empty() {
            out.push(m);
        }
    }
    out
}

/// A flat sand floor for lineups and tests.
pub fn flat(half: f32) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.heightfield(half, 40, |_, _| 0.0, sand(0.0));
    m
}
