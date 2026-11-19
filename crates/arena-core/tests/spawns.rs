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
                // A new player holds W for a good while: 120 m of open run.
                let run = 4.0 * SPAWN_CLEAR;
                if !map.heading_clear(p, s.yaw, run) {
                    bad.push(format!("{id:?} seed {seed} spawn {i} at {p}: blocked within {run} m ahead"));
                }
                if dir.dot((o - p).normalize()) < 0.25 {
                    bad.push(format!("{id:?} seed {seed} spawn {i} at {p}: faces away from the action"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Convoy crews (escorts behind the truck, attackers ahead of it) spawn
/// facing open ground too, on every map, in both rounds' setups.
#[test]
fn convoy_spawns_face_open_ground() {
    let mut bad = Vec::new();
    let mut checked = 0;
    for id in MapId::ALL {
        for seed in [3u64, 36, 77] {
            let (mut s, _me) = arena_core::Sim::practice_convoy_on(id, seed, 4, 0.5);
            // A few respawns along the way as well as the opening spawns.
            for check in 0..3 {
                let ids: Vec<_> = s.players.iter().filter(|p| p.tank.alive && !s.is_convoy_truck(p.id)).map(|p| p.id).collect();
                for pid in ids {
                    let (pos, _) = s.tank_pose(pid).unwrap();
                    let yaw = s.hull_yaw(pid).unwrap();
                    checked += (check == 0) as u32;
                    if check == 0 && !s.map.heading_clear(Vec2::new(pos.x, pos.z), yaw, SPAWN_CLEAR) {
                        bad.push(format!("{id:?} seed {seed}: crew {pid} spawned at {pos} facing blocked ground"));
                    }
                }
                if check < 2 {
                    for _ in 0..(60 * 40) {
                        s.step();
                        for e in s.drain_events() {
                            if let arena_core::Event::Respawned { player, .. } = e {
                                if s.is_convoy_truck(player) {
                                    continue;
                                }
                                let (pos, _) = s.tank_pose(player).unwrap();
                                let yaw = s.hull_yaw(player).unwrap();
                                checked += 1;
                                if !s.map.heading_clear(Vec2::new(pos.x, pos.z), yaw, SPAWN_CLEAR) {
                                    bad.push(format!("{id:?} seed {seed}: crew {player} respawned at {pos} facing blocked ground"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 60, "only {checked} convoy spawns checked");
    assert!(bad.is_empty(), "{} blocked convoy spawns:\n{}", bad.len(), bad.join("\n"));
}
