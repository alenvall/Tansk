//! Spring pads and fuel trailers: placed sensibly on every map, the springs
//! throw a tank to its landing spot, the trailers burn, blow big and come back.

use arena_core::hazards::{TRAILER_BLAST_RADIUS, TRAILER_RESPAWN};
use arena_core::map::{Cover, Map, MapId, ObstacleKind};
use arena_core::{Event, Sim};
use glam::{Vec2, Vec3};

fn flat(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

#[test]
fn springs_and_trailers_are_placed_sensibly() {
    let mut bad = Vec::new();
    for id in MapId::ALL {
        for seed in [0u64, 7, 21] {
            let map = Map::by_id(id, seed);
            let nav = arena_core::bots::NavGrid::build(&map, 4.0);
            let o = flat(map.outpost);
            let trailers: Vec<_> = map.obstacles.iter().filter(|o| o.kind == ObstacleKind::FuelTrailer).collect();
            if !(2..=4).contains(&map.springs.len()) || trailers.len() < 2 {
                bad.push(format!("{id:?}: {} springs, {} trailers", map.springs.len(), trailers.len()));
            }
            let near_spawn = |p: Vec2, r: f32| map.spawns.iter().any(|s| flat(s.pos).distance(p) < r);
            for (k, s) in map.springs.iter().enumerate() {
                let pad = &map.obstacles[s.obstacle];
                assert_eq!(pad.kind, ObstacleKind::SpringPad);
                let p = flat(pad.pos);
                let land = flat(s.land);
                let fwd = pad.rot * Vec3::X;
                let yaw = (-fwd.z).atan2(fwd.x);
                // A run-up behind the pad, the pad itself and the landing are open and flat.
                let back = p - Vec2::new(fwd.x, fwd.z) * 24.0;
                if !map.heading_clear(back, yaw, 26.0) {
                    bad.push(format!("{id:?} spring {k}: no clear run-up to the pad at {p}"));
                }
                for q in std::iter::once(land).chain((0..8).map(|i| land + Vec2::from_angle(i as f32 * 0.785) * 6.0)) {
                    if map.blocked_xz(q, 2.0) || map.normal_at(q.x, q.y).y < 0.93 {
                        bad.push(format!("{id:?} spring {k}: landing near {land} not clear and flat (at {q})"));
                        break;
                    }
                }
                if near_spawn(p, 40.0) || near_spawn(land, 40.0) {
                    bad.push(format!("{id:?} spring {k}: next to a spawn"));
                }
                if !nav.reachable(land, o) || !nav.reachable(back, o) {
                    bad.push(format!("{id:?} spring {k}: cut off"));
                }
            }
            for t in &trailers {
                let p = flat(t.pos);
                if near_spawn(p, 40.0) {
                    bad.push(format!("{id:?}: trailer at {p} next to a spawn"));
                }
                if p.distance(o) < 30.0 {
                    bad.push(format!("{id:?}: trailer at {p} in the outpost"));
                }
                // Nothing solid in a slot beside it: either touching or a lane.
                for other in map.obstacles.iter().filter(|q| q.cover != Cover::Flat && !std::ptr::eq(*q, *t)) {
                    let d = flat(other.pos).distance(p);
                    if d > other.radius + t.radius + 8.0 {
                        continue;
                    }
                    for (c, _, h) in other.world_boxes() {
                        if c.y - h.y > other.pos.y + 4.0 {
                            continue;
                        }
                        let (tc, tr, th) = t.world_boxes().next().unwrap();
                        let l = tr.inverse() * (c - tc);
                        let gap = (l.x.abs() - th.x - h.x.max(h.z)).max(l.z.abs() - th.z - h.x.max(h.z));
                        if gap > 0.5 && gap < 6.0 {
                            bad.push(format!("{id:?}: trailer at {p} leaves a {gap:.1} m slot by a {:?} at {}", other.kind, flat(c)));
                        }
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Driving fast over each pad throws the tank and it lands near the spot.
#[test]
fn springs_throw_tanks_to_their_landing() {
    for id in MapId::ALL {
        let (mut s, me) = Sim::practice_on(id, 7, 0, 0.5);
        for k in 0..s.map.springs.len() {
            let site = s.map.springs[k];
            let pad = s.map.obstacles[site.obstacle].clone();
            let fwd = pad.rot * Vec3::X;
            let start = pad.pos - fwd * 22.0;
            s.teleport(me, start.x, start.z, (-fwd.z).atan2(fwd.x));
            let mut launched = false;
            let mut best = f32::MAX;
            for _ in 0..(60 * 9) {
                if let Some((p, _)) = s.tank_pose(me) {
                    let to = site.land - p;
                    let yaw = s.hull_yaw(me).unwrap_or(0.0);
                    let mut d = (-to.z).atan2(to.x) - yaw;
                    while d > std::f32::consts::PI { d -= std::f32::consts::TAU; }
                    while d < -std::f32::consts::PI { d += std::f32::consts::TAU; }
                    let go = if launched { 0.0 } else { 1.0 };
                    s.set_input(me, arena_core::Input { throttle: go, steer: (-d * 2.0).clamp(-1.0, 1.0) * go, aim_point: site.land, ..Default::default() });
                    if launched {
                        best = best.min(flat(p).distance(flat(site.land)));
                    }
                }
                s.step();
                launched |= s.drain_events().iter().any(|e| matches!(e, Event::Launched { player, .. } if *player == me));
            }
            assert!(launched, "{id:?} spring {k} never fired");
            let (end, _) = s.tank_pose(me).unwrap();
            assert!(best < 8.0, "{id:?} spring {k}: came no closer than {best} m to {}", site.land);
            assert!(flat(end).distance(flat(site.land)) < 18.0, "{id:?} spring {k}: ended at {end}, landing {}", site.land);
            assert!(s.players[s.players.iter().position(|p| p.id == me).unwrap()].tank.alive);
        }
    }
}

/// A trailer shot up burns, then blows big (hurting and throwing a tank
/// nearby), and is back after its respawn time.
#[test]
fn trailers_burn_blow_and_come_back() {
    for id in MapId::ALL {
        let (mut s, me) = Sim::practice_on(id, 7, 0, 0.5);
        let ti = s.map.obstacles.iter().position(|o| o.kind == ObstacleKind::FuelTrailer).unwrap();
        let t = s.map.obstacles[ti].clone();
        // Park the tank 12 m off its side.
        let side = t.rot * Vec3::Z * 12.0;
        let at = t.pos + side;
        s.teleport(me, at.x, at.z, 0.0);
        for _ in 0..30 {
            s.step();
        }
        let j = s.players.iter().position(|p| p.id == me).unwrap();
        let hp0 = s.players[j].tank.health;
        // Shot up (a blast on its far side, out of the tank's reach).
        let hit = t.pos - side * 0.15 + Vec3::Y * 2.0;
        s.blast(hit, 120.0, 3.0, 0.0);
        let mut puffs = 0;
        let mut blew = None;
        for tick in 0..(60 * 8) {
            s.step();
            for e in s.drain_events() {
                match e {
                    Event::Explosion { weapon: arena_core::Weapon::Burn, .. } => puffs += 1,
                    Event::HazardExploded { radius, .. } if radius >= TRAILER_BLAST_RADIUS => blew = Some(tick),
                    _ => {}
                }
            }
            if blew.is_some() {
                break;
            }
        }
        assert!(blew.is_some(), "{id:?}: the trailer never blew");
        assert!(puffs >= 3 && blew.unwrap() > 60, "{id:?}: no burning warning ({puffs} puffs, blew at tick {blew:?})");
        assert!(s.removed.iter().any(|r| r.obstacle as usize == ti), "{id:?}: the trailer is still there");
        for _ in 0..20 {
            s.step();
        }
        let j = s.players.iter().position(|p| p.id == me).unwrap();
        let p = &s.players[j];
        assert!(!p.tank.alive || p.tank.health < hp0 - 100.0, "{id:?}: the blast barely touched a tank 12 m away");
        // Back after the respawn time (move the tank away first).
        s.teleport(me, s.map.outpost.x, s.map.outpost.z, 0.0);
        for _ in 0..((TRAILER_RESPAWN + 5.0) * 60.0) as u32 {
            s.step();
        }
        assert!(!s.removed.iter().any(|r| r.obstacle as usize == ti), "{id:?}: the trailer never came back");
    }
}
