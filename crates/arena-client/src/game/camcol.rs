//! The chase camera's collision in a match: the terrain and every obstacle
//! box, grown by how far the drawn dressing reaches past its collider
//! (shack roofs, car stacks, pipe racks), so the boom pulls in before the
//! lens goes through a prop.

use arena_core::map::{Map, ObstacleKind};
use arena_core::Cover;
use glam::Vec3;

use crate::camera::CameraCollider;

/// Clearance kept from the ground, m.
const GROUND: f32 = 0.7;
/// Every box is grown by this much (the near plane and a little air).
const PAD: f32 = 0.55;

pub struct SceneCollider<'a>(pub &'a Map);

/// How far the drawn model reaches past its colliders (sides, top), m.
fn dressing(kind: ObstacleKind) -> (f32, f32) {
    use ObstacleKind as K;
    match kind {
        K::Shack | K::Tent => (0.6, 0.9),
        K::CarCube | K::CarTower | K::CarWreck => (0.5, 0.6),
        K::Pipe => (0.6, 0.8),
        K::ContainerStack | K::FuelTank => (0.3, 0.5),
        _ => (0.2, 0.3),
    }
}

impl CameraCollider for SceneCollider<'_> {
    fn free_fraction(&self, from: Vec3, to: Vec3) -> f32 {
        let map = self.0;
        let d = to - from;
        let steps = 40;
        let mut best: f32 = 1.0;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let p = from + d * t;
            if p.y < map.height_at(p.x, p.z) + GROUND {
                best = best.min((t - 1.0 / steps as f32).max(0.0));
                break;
            }
        }
        let len = d.length().max(1e-4);
        let dir = d / len;
        for o in &map.obstacles {
            if o.cover == Cover::Flat || o.pos.distance(from) > o.radius + len + 4.0 {
                continue;
            }
            let (side, top) = dressing(o.kind);
            for (c, rot, half) in o.world_boxes() {
                let inv = rot.inverse();
                let ro = inv * (from - c);
                let rd = inv * dir;
                let half = half + Vec3::new(side + PAD, top + PAD, side + PAD);
                // Starting inside (under a pipe rack, beside a wall): that box
                // can't be looked out of, so it doesn't count.
                if (0..3).all(|k| ro[k].abs() < half[k]) {
                    continue;
                }
                if let Some(t) = slab(ro, rd, half, len) {
                    best = best.min(t / len);
                }
            }
        }
        best
    }
}

/// Where a ray from `ro` along `rd` first enters the box of `half` extents
/// (both in the box's frame), within `len`.
fn slab(ro: Vec3, rd: Vec3, half: Vec3, len: f32) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, len);
    for k in 0..3 {
        if rd[k].abs() < 1e-6 {
            if ro[k].abs() > half[k] {
                return None;
            }
        } else {
            let a = (-half[k] - ro[k]) / rd[k];
            let b = (half[k] - ro[k]) / rd[k];
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(t0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_behind_pulls_the_boom_in() {
        assert_eq!(slab(Vec3::new(-5.0, 0.0, 0.0), Vec3::X, Vec3::ONE, 10.0), Some(4.0));
        assert_eq!(slab(Vec3::new(-5.0, 3.0, 0.0), Vec3::X, Vec3::ONE, 10.0), None);
        assert_eq!(slab(Vec3::new(-5.0, 0.0, 0.0), Vec3::X, Vec3::ONE, 3.0), None);
        // Every map: from above the middle of the field the boom is free somewhere.
        for id in arena_core::MapId::ALL {
            let map = Map::by_id(id, 7);
            let c = SceneCollider(&map);
            let p = Vec3::new(0.0, map.height_at(0.0, 0.0) + 40.0, 0.0);
            assert!(c.free_fraction(p, p + Vec3::new(6.0, 1.0, 0.0)) > 0.9, "{id:?}");
        }
    }
}
