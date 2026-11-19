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
/// The girder bridge's side trusses rise this high over the deck, m.
const TRUSS_H: f32 = 3.4;

pub struct SceneCollider<'a>(pub &'a Map);

/// How far the drawn model reaches past its colliders (sides, top), m.
fn dressing(kind: ObstacleKind) -> (f32, f32) {
    use ObstacleKind as K;
    match kind {
        K::Shack | K::Tent => (0.6, 0.9),
        K::CarCube | K::CarTower | K::CarWreck => (0.5, 0.6),
        K::Pipe => (0.6, 0.8),
        K::ContainerStack | K::FuelTank => (0.3, 0.5),
        K::GirderBridge => (0.0, 0.1),
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
            let mut boxes: Vec<(Vec3, glam::Quat, Vec3)> = o.world_boxes().collect();
            if o.kind == ObstacleKind::GirderBridge {
                // The drawn trusses stand on the deck's edges (the sim only
                // has the deck): walls for the camera, not for driving.
                if let Some(&(c, rot, h)) = boxes.first() {
                    for s in [-1.0, 1.0] {
                        boxes.push((c + rot * Vec3::new(s * (h.x - 0.35), h.y + TRUSS_H * 0.5, 0.0), rot, Vec3::new(0.35, TRUSS_H * 0.5, h.z)));
                    }
                }
            }
            for (c, rot, half) in boxes {
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

    /// On the Gorge's bridge deck, a boom swung sideways meets the truss;
    /// along the deck it stays free.
    #[test]
    fn the_bridge_trusses_stop_the_camera() {
        let map = Map::by_id(arena_core::MapId::Canyon, 3);
        let Some(b) = map.obstacles.iter().find(|o| o.kind == ObstacleKind::GirderBridge) else { return };
        let (c, rot, h) = b.world_boxes().next().unwrap();
        let on_deck = c + rot * Vec3::new(0.0, h.y + 2.4, 0.0);
        let col = SceneCollider(&map);
        let side = col.free_fraction(on_deck, on_deck + rot * Vec3::new(9.0, 1.5, 0.0));
        let along = col.free_fraction(on_deck, on_deck + rot * Vec3::new(0.0, 1.5, 7.0));
        assert!(side < 0.8, "sideways {side}");
        assert!(along > 0.9, "along the deck {along}");
    }

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
