//! The aim point: a ray from the camera through the screen centre onto the
//! terrain, the obstacles and the other tanks.

use arena_core::map::Map;
use arena_core::{Cover, PlayerId};
use glam::Vec3;

use crate::world::TankView;

/// How far the reticle reaches when the ray hits nothing.
pub const FAR: f32 = 600.0;

/// Slab test: entry distance of the ray into an oriented box, if any.
fn ray_box(ro: Vec3, rd: Vec3, c: Vec3, rot: glam::Quat, half: Vec3, max: f32) -> Option<f32> {
    let inv = rot.inverse();
    let o = inv * (ro - c);
    let d = inv * rd;
    let (mut t0, mut t1) = (0.0f32, max);
    for k in 0..3 {
        if d[k].abs() < 1e-6 {
            if o[k].abs() > half[k] {
                return None;
            }
        } else {
            let a = (-half[k] - o[k]) / d[k];
            let b = (half[k] - o[k]) / d[k];
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(t0)
}

/// Casts from `from` along `dir` (unit), skipping the first `skip` metres
/// (between the camera and your own tank) and the tank `me`.
pub fn cast(map: &Map, tanks: &[TankView], me: Option<PlayerId>, from: Vec3, dir: Vec3, skip: f32) -> Vec3 {
    let mut best = FAR;
    // Terrain: march, then refine by bisection.
    let step = 2.0;
    let mut prev_t = skip;
    let mut t = skip;
    while t < best {
        let p = from + dir * t;
        if p.y < map.height_at(p.x, p.z) {
            let (mut lo, mut hi) = (prev_t, t);
            for _ in 0..12 {
                let mid = (lo + hi) * 0.5;
                let q = from + dir * mid;
                if q.y < map.height_at(q.x, q.z) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            best = hi;
            break;
        }
        prev_t = t;
        t += step;
        // Out of the map and climbing: nothing to hit.
        if p.y > 120.0 && dir.y > 0.0 {
            break;
        }
    }
    // Obstacles.
    for o in &map.obstacles {
        if o.cover == Cover::Flat {
            continue;
        }
        // Cheap reject: distance from the ray to the obstacle centre.
        let to = o.pos - from;
        let along = to.dot(dir);
        if along < -o.radius || along > best + o.radius {
            continue;
        }
        if (to - dir * along).length() > o.radius + 6.0 {
            continue;
        }
        for (c, rot, half) in o.world_boxes() {
            if let Some(h) = ray_box(from, dir, c, rot, half, best) {
                if h > skip && h < best {
                    best = h;
                }
            }
        }
    }
    // Tanks: a sphere round each hull.
    for tv in tanks {
        if Some(tv.player) == me || !tv.alive {
            continue;
        }
        let c = tv.pos + Vec3::Y * 0.6;
        let r = 2.2;
        let oc = from - c;
        let b = oc.dot(dir);
        let disc = b * b - (oc.length_squared() - r * r);
        if disc > 0.0 {
            let h = -b - disc.sqrt();
            if h > skip && h < best {
                best = h;
            }
        }
    }
    from + dir * best
}
