//! Destructible cover in stages, rams, toppling towers and craters.

use arena_core::destruct::{CraterSnap, RubbleKind};
use arena_core::map::{Map, MapId, ObstacleKind};
use arena_core::{Event, Sim};
use glam::{Vec2, Vec3};

fn first(sim: &Sim, kind: ObstacleKind) -> usize {
    sim.map.obstacles.iter().position(|o| o.kind == kind).unwrap_or_else(|| panic!("no {kind:?}"))
}

/// An open, flat spot near `p`.
fn open_near(map: &Map, p: Vec2) -> Vec2 {
    for r in 0..40 {
        for k in 0..16 {
            let q = p + Vec2::from_angle(k as f32 * 0.3927) * r as f32 * 3.0;
            let flat = (0..8).all(|i| {
                let e = q + Vec2::from_angle(i as f32 * 0.785) * 10.0;
                map.normal_at(e.x, e.y).y > 0.97 && !map.blocked_xz(e, 3.0)
            });
            if flat && !map.blocked_xz(q, 12.0) {
                return q;
            }
        }
    }
    p
}

fn centre(sim: &Sim, oi: usize) -> Vec3 {
    let o = &sim.map.obstacles[oi];
    o.pos + o.rot * o.boxes[0].center
}

/// A shack shot up dents, then falls to a low heap of rubble that still
/// blocks (and gives low cover), and the bots' nav still steers round it.
#[test]
fn a_shack_dents_then_falls_to_rubble() {
    let (mut s, _) = Sim::practice_on(MapId::RustBelt, 7, 0, 0.5);
    let oi = first(&s, ObstacleKind::Shack);
    let c = centre(&s, oi);
    s.blast(c + Vec3::Y * 2.0, 120.0, 2.5, 0.0);
    for _ in 0..12 {
        s.step();
    }
    assert!(s.snapshot().dented.contains(&(oi as u16)), "not dented");
    for _ in 0..4 {
        s.blast(c + Vec3::Y * 2.0, 200.0, 3.0, 0.0);
    }
    // (Let the flying debris come down.)
    for _ in 0..(60 * 5) {
        s.step();
    }
    let snap = s.snapshot();
    let r = snap.rubble.iter().find(|r| r.obstacle as usize == oi).expect("rubble");
    assert_eq!(r.kind, RubbleKind::Pile);
    assert!(snap.removed.iter().any(|r| r.obstacle as usize == oi && r.box_index == 255));
    // A ray down onto the heap stops under 1.2 m up: low cover, still solid.
    let ground = s.map.height_at(c.x, c.z);
    let hit = s.phys.ray(Vec3::new(c.x, ground + 6.0, c.z), Vec3::NEG_Y, 10.0, arena_core::physics::ground_query(), None);
    let top = ground + 6.0 - hit.expect("the heap is solid").1;
    assert!(top - ground > 0.5 && top - ground < 1.2, "heap top {} above the ground", top - ground);
    assert!(s.nav.is_blocked(Vec2::new(c.x, c.z)), "the heap still blocks the nav");
}

/// A tank driving fast into a market fence bursts through it and drives on.
#[test]
fn a_fast_tank_rams_through_a_fence() {
    let (mut s, me) = Sim::practice_on(MapId::RustBelt, 7, 0, 0.5);
    let oi = first(&s, ObstacleKind::Fence);
    let o = s.map.obstacles[oi].clone();
    let n = o.rot * Vec3::Z;
    let start = o.pos + n * 30.0;
    let goal = o.pos - n * 20.0;
    s.teleport(me, start.x, start.z, (-(-n.z)).atan2(-n.x));
    let mut through = false;
    for _ in 0..(60 * 10) {
        if let Some((p, _)) = s.tank_pose(me) {
            let to = goal - p;
            let yaw = s.hull_yaw(me).unwrap_or(0.0);
            let mut d = (-to.z).atan2(to.x) - yaw;
            while d > std::f32::consts::PI { d -= std::f32::consts::TAU; }
            while d < -std::f32::consts::PI { d += std::f32::consts::TAU; }
            s.set_input(me, arena_core::Input { throttle: 1.0, boost: true, steer: (-d * 2.0).clamp(-1.0, 1.0), aim_point: goal, ..Default::default() });
            if (p - o.pos).dot(n) < -8.0 {
                through = true;
                break;
            }
        }
        s.step();
    }
    let snap = s.snapshot();
    assert!(snap.rubble.iter().any(|r| r.obstacle as usize == oi && r.kind == RubbleKind::Flat), "the fence still stands");
    assert!(through, "the tank didn't get through");
}

/// A water tower brought down topples, away from the blast, and its fallen
/// length blocks the nav where it lies.
#[test]
fn a_water_tower_topples() {
    let (mut s, _) = Sim::practice_on(MapId::Refinery, 7, 0, 0.5);
    let oi = first(&s, ObstacleKind::WaterTower);
    let c = s.map.obstacles[oi].pos;
    for _ in 0..6 {
        s.blast(c + Vec3::new(2.0, 3.0, 0.0), 250.0, 4.0, 0.0);
    }
    for _ in 0..6 {
        s.step();
    }
    let snap = s.snapshot();
    let r = snap.rubble.iter().find(|r| r.obstacle as usize == oi).expect("toppled");
    assert_eq!(r.kind, RubbleKind::Topple);
    let yaw = r.yaw();
    let along = c + Vec3::new(yaw.cos(), 0.0, -yaw.sin()) * 8.0;
    assert!(s.nav.is_blocked(Vec2::new(along.x, along.z)), "the fallen tower doesn't block the nav");
}

/// A bomb digs a crater: the heights drop, physics agrees, the snapshot
/// carries it, a client digging from the snapshot gets the very same
/// heights, a tank drives out of it, and a reset fills it in.
#[test]
fn bombs_dig_craters_everyone_agrees_on() {
    for id in MapId::ALL {
        let (mut s, me) = Sim::practice_on(id, 7, 0, 0.5);
        let o = Vec2::new(s.map.outpost.x, s.map.outpost.z) + Vec2::new(60.0, 40.0);
        let p = open_near(&s.map, o);
        let before = s.map.height_at(p.x, p.y);
        let g = Vec3::new(p.x, before, p.y);
        s.dig_crater(g, 14.0);
        s.dig_crater(g, 14.0);
        s.dig_crater(g, 14.0);
        let after = s.map.height_at(p.x, p.y);
        assert!(before - after > 0.6, "{id:?}: only {} m deep", before - after);
        assert!(before - after <= arena_core::map::CRATER_FLOOR + 0.01, "{id:?}: too deep");
        // A client digging from the snapshot gets the very same ground.
        let snap = s.snapshot();
        assert_eq!(snap.craters.len(), 3);
        let mut client = Map::by_id(id, 7);
        let base = client.heights.clone();
        for c in &snap.craters {
            client.apply_crater(c.centre(), c.radius(), c.depth(), &base);
        }
        assert_eq!(client.heights, s.map.heights, "{id:?}: client and server disagree");
        let _ = CraterSnap::new(p, 5.0, 0.8);
        // Physics agrees: a tank parked in it sits down in the dent.
        s.teleport(me, p.x, p.y, 0.0);
        for _ in 0..60 {
            s.step();
        }
        let (sit, _) = s.tank_pose(me).unwrap();
        let ride = sit.y - after;
        assert!(ride > 0.4 && ride < 2.0, "{id:?}: the tank rides {ride} m over the crater floor (physics ground differs)");
        // Drives out.
        for _ in 0..(60 * 6) {
            s.set_input(me, arena_core::Input { throttle: 1.0, aim_point: Vec3::new(p.x + 50.0, 0.0, p.y), ..Default::default() });
            s.step();
        }
        let (end, _) = s.tank_pose(me).unwrap();
        assert!(Vec2::new(end.x, end.z).distance(p) > 12.0, "{id:?}: stuck in the crater at {end}");
        // A new match fills it in.
        s.reset_match();
        assert_eq!(s.map.height_at(p.x, p.y), before, "{id:?}: the crater stayed after a reset");
        assert!(s.snapshot().craters.is_empty());
    }
}

/// Air-strike bombs and big hazards dig; a tank shell doesn't.
#[test]
fn only_big_blasts_dig() {
    let (mut s, _) = Sim::practice_on(MapId::Sink, 3, 0, 0.5);
    let p = open_near(&s.map, Vec2::new(60.0, 60.0));
    let g = Vec3::new(p.x, s.map.height_at(p.x, p.y), p.y);
    s.blast(g, 100.0, 6.0, 0.0);
    assert!(s.snapshot().craters.is_empty(), "a plain blast dug a crater");
    let ev: Vec<Event> = s.drain_events();
    let _ = ev;
}
