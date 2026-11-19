//! Every spawn on every map faces open, drivable ground toward the action:
//! a new player who just holds W drives somewhere, not into a wall.

use arena_core::map::{Map, MapId, SPAWN_CLEAR};
use glam::Vec2;

#[test]
fn spawns_face_open_ground_toward_the_action() {
    let mut bad = Vec::new();
    for id in MapId::ALL {
        for seed in [0u64, 1, 5, 7, 21, 36] {
            let map = Map::by_id(id, seed);
            let o = Vec2::new(map.outpost.x, map.outpost.z);
            for (i, s) in map.spawns.iter().enumerate() {
                let p = Vec2::new(s.pos.x, s.pos.z);
                let dir = Vec2::new(s.yaw.cos(), -s.yaw.sin());
                if !map.heading_clear(p, s.yaw, SPAWN_CLEAR) {
                    bad.push(format!("{id:?} seed {seed} spawn {i} at {p}: blocked within {SPAWN_CLEAR} m ahead"));
                }
                if dir.dot((o - p).normalize()) < 0.25 {
                    bad.push(format!("{id:?} seed {seed} spawn {i} at {p}: faces away from the action"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
