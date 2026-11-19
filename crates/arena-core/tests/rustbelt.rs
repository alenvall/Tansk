//! The Rust Belt: the level rules, its places, and that they all connect.

use arena_core::map::{Cover, Map, MapId, ObstacleKind};
use arena_core::Sim;
use glam::{Vec2, Vec3};

#[test]
fn the_rust_belt_follows_the_level_rules() {
    let map = Map::rust_belt(1);
    assert_eq!(map.id, MapId::RustBelt);
    assert_eq!(map, Map::by_id(MapId::RustBelt, 1));
    assert_eq!(MapId::from_key("rustbelt"), Some(MapId::RustBelt));
    assert_eq!(MapId::from_key("the_rust_belt"), Some(MapId::RustBelt));
    assert_eq!(map.spawns.len(), 8);
    assert!(map.scrap_sites.len() >= 40);
    for o in &map.obstacles {
        // (The canyon cliffs out of bounds are scenery.)
        if o.pos.x.abs() > map.play_half || o.pos.z.abs() > map.play_half {
            continue;
        }
        let top = map.obstacle_top_above_ground(o);
        match o.cover {
            Cover::Low => assert!(top < 1.2, "{:?} low cover is {top} m tall at {}", o.kind, o.pos),
            Cover::Tall => {
                for (c, r, h) in o.world_boxes() {
                    let top_c = c.y + (r * Vec3::Y).y.abs() * h.y - map.height_at(c.x, c.z);
                    assert!(top_c > 2.5, "{:?} tall cover box is only {top_c} m at {c}", o.kind);
                }
            }
            Cover::Flat => assert!(top < 0.6, "{:?} deck sticks up {top} m at {}", o.kind, o.pos),
        }
    }
    // The bridge: its deck is flush with both banks and leaves room for a
    // tank (and its mast) on the river bed under it.
    let br = map.obstacles.iter().find(|o| o.kind == ObstacleKind::Bridge).expect("a bridge");
    let (c, _, h) = br.world_boxes().next().unwrap();
    let clear = c.y - h.y - map.height_at(c.x, c.z);
    assert!(clear > 5.0, "bridge only {clear} m clear");
    for end in [-1.0f32, 1.0] {
        let p = c + Vec3::Z * end * (h.z - 1.0);
        let step = c.y + h.y - map.height_at(p.x, p.z);
        assert!(step.abs() < 0.45, "the bridge deck is {step} m off the bank at {p}");
    }
}

/// The gap between two oriented rectangles (centre, x axis, half extents) on
/// the ground, 0 when they touch or overlap.
fn rect_gap(a: (Vec2, Vec2, Vec2), b: (Vec2, Vec2, Vec2)) -> f32 {
    let corners = |r: (Vec2, Vec2, Vec2)| {
        let (c, ax, h) = r;
        let az = Vec2::new(-ax.y, ax.x);
        [c + ax * h.x + az * h.y, c - ax * h.x + az * h.y, c - ax * h.x - az * h.y, c + ax * h.x - az * h.y]
    };
    let inside = |r: (Vec2, Vec2, Vec2), p: Vec2| {
        let (c, ax, h) = r;
        let az = Vec2::new(-ax.y, ax.x);
        let d = p - c;
        d.dot(ax).abs() <= h.x + 1e-3 && d.dot(az).abs() <= h.y + 1e-3
    };
    let ca = corners(a);
    let cb = corners(b);
    if ca.iter().any(|p| inside(b, *p)) || cb.iter().any(|p| inside(a, *p)) {
        return 0.0;
    }
    let seg = |p: Vec2, s0: Vec2, s1: Vec2| {
        let ab = s1 - s0;
        let t = ((p - s0).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        (s0 + ab * t).distance(p)
    };
    let mut best = f32::MAX;
    for (pa, pb) in [(ca, cb), (cb, ca)] {
        for p in pa {
            for k in 0..4 {
                best = best.min(seg(p, pb[k], pb[(k + 1) % 4]));
            }
        }
    }
    best
}

/// No slots: any two solid things on the ground either touch or leave a lane
/// a tank fits through (7.5 m; 6 m next to a thin pole or a crate pile).
#[test]
fn the_rust_belt_has_no_slots() {
    for seed in [1u64, 7, 21] {
        let map = Map::rust_belt(seed);
        let mut rects = Vec::new();
        for (oi, o) in map.obstacles.iter().enumerate() {
            if o.cover == Cover::Flat || o.pos.z.abs() > map.play_half {
                continue;
            }
            for (c, r, h) in o.world_boxes() {
                if c.y - h.y > o.pos.y + 4.0 {
                    continue;
                }
                let ax = r * Vec3::X;
                let ax = Vec2::new(ax.x, ax.z).normalize();
                rects.push((oi, o.kind, (Vec2::new(c.x, c.z), ax, Vec2::new(h.x, h.z))));
            }
        }
        // Per pair of obstacles: the closest their boxes come.
        let mut gaps: std::collections::BTreeMap<(usize, usize), (f32, ObstacleKind, ObstacleKind, Vec2, Vec2)> = Default::default();
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                if a.0 == b.0 {
                    continue;
                }
                let gap = rect_gap(a.2, b.2);
                let e = gaps.entry((a.0.min(b.0), a.0.max(b.0))).or_insert((f32::MAX, a.1, b.1, a.2 .0, b.2 .0));
                if gap < e.0 {
                    *e = (gap, a.1, b.1, a.2 .0, b.2 .0);
                }
            }
        }
        let mut bad = Vec::new();
        let small = |k: ObstacleKind| matches!(k, ObstacleKind::PowerPole | ObstacleKind::Crates | ObstacleKind::Hedgehog);
        for (gap, ka, kb, pa, pb) in gaps.values() {
            let need = if small(*ka) || small(*kb) { 6.0 } else { 7.5 };
            if *gap > 0.6 && *gap < need {
                bad.push(format!("{ka:?} at {pa} and {kb:?} at {pb}: {gap:.1} m"));
            }
        }
        assert!(bad.is_empty(), "seed {seed}: slots:\n{}", bad.join("\n"));
    }
}

#[test]
fn the_rust_belt_has_its_places_and_they_connect() {
    let map = Map::rust_belt(21);
    let names: Vec<&str> = map.regions.iter().map(|r| r.name.as_str()).collect();
    for n in ["The Salvage Market", "The Dry River", "The Crusher", "The Bus Graveyard", "The Magnet Crane", "The Tower"] {
        assert!(names.contains(&n), "missing region {n}");
    }
    for k in [
        ObstacleKind::CarCube,
        ObstacleKind::Crusher,
        ObstacleKind::MagnetCrane,
        ObstacleKind::Bus,
        ObstacleKind::Shack,
        ObstacleKind::Stall,
        ObstacleKind::Fence,
        ObstacleKind::Bridge,
        ObstacleKind::CarTower,
        ObstacleKind::Highway,
        ObstacleKind::LaunchPlate,
    ] {
        assert!(map.obstacles.iter().any(|o| o.kind == k), "no {k:?}");
    }
    println!("The Rust Belt: {} obstacles, {} boxes", map.obstacles.len(), map.obstacles.iter().map(|o| o.boxes.len()).sum::<usize>());
    for r in &map.regions {
        let n = map.scrap_sites.iter().filter(|s| s.pos.distance(r.center) < r.radius).count();
        assert!(n >= 3, "{} has only {n} scrap sites", r.name);
        let sites = map.repair_sites.iter().chain(&map.rare_sites).chain(&map.strike_sites).filter(|p| p.distance(r.center) < r.radius + 10.0).count();
        assert!(sites >= 1, "{} has no repair, rare or strike site", r.name);
    }
    assert_eq!(map.wells.len(), 3);
    assert!(map.convoy_route.len() >= 8);
    assert!(map.marks.len() >= 10);
    let nav = arena_core::bots::NavGrid::build(&map, 4.0);
    let o = Vec2::new(map.outpost.x, map.outpost.z);
    assert!(!map.blocked_xz(o, 16.0), "the outpost courtyard must be open");
    for (i, s) in map.spawns.iter().enumerate() {
        let p = Vec2::new(s.pos.x, s.pos.z);
        assert!(!map.blocked_xz(p, 5.0), "spawn {i} is blocked");
        assert!(!nav.is_blocked(p), "spawn {i} is on steep ground");
        assert!(nav.reachable(p, o), "spawn {i} can't drive to the outpost");
        for t in map.spawns.iter().skip(i + 1) {
            assert!(t.pos.distance(s.pos) > 90.0, "spawns too close");
        }
        assert!(map.obstacles.iter().any(|ob| ob.cover != Cover::Flat && Vec2::new(ob.pos.x, ob.pos.z).distance(p) < 30.0));
    }
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let p = Vec2::new(a.cos(), a.sin()) * 255.0;
        if !map.blocked_xz(p, 4.0) {
            assert!(nav.reachable(p, o), "raid edge at {p} is cut off");
        }
    }
    for s in &map.scrap_sites {
        let p = Vec2::new(s.pos.x, s.pos.z);
        assert!(!map.blocked_xz(p, 1.0), "scrap site at {} is inside cover", s.pos);
        assert!(nav.reachable(p, o), "scrap site at {} unreachable", s.pos);
    }
    for w in &map.wells {
        assert!(nav.reachable(Vec2::new(w.pos.x, w.pos.z), o), "well {} unreachable", w.name);
    }
    for p in map.repair_sites.iter().chain(&map.rare_sites).chain(&map.strike_sites) {
        assert!(nav.reachable(Vec2::new(p.x, p.z), o), "site at {p} unreachable");
    }
    // Both sides of the river connect over the fords, and you can drive
    // along the bed under the bridge.
    let bz = arena_core::rustbelt::river_z(0.0);
    assert!(nav.reachable(Vec2::new(-40.0, bz), Vec2::new(40.0, bz)), "the bed is cut at the bridge");
    assert!(nav.reachable(Vec2::new(0.0, -120.0), Vec2::new(0.0, 160.0)), "north and south don't connect");
    // The river bed really is sunken.
    let deep = map.height_at(-40.0, arena_core::rustbelt::river_z(-40.0));
    let bank = map.height_at(-40.0, arena_core::rustbelt::river_z(-40.0) - 40.0);
    assert!(bank - deep > 5.0, "the river bed is only {} m down", bank - deep);
}

/// The modes run on the map: a short Convoy keeps its truck rolling, the
/// Wellhead and the Last Crew start.
#[test]
fn the_rust_belt_runs_its_modes() {
    let (mut s, _me) = Sim::practice_convoy_on(MapId::RustBelt, 36, 4, 0.5);
    let len = s.convoy_path().len();
    assert!(len > 10, "convoy path has {len} points");
    for _ in 0..(60 * 40) {
        s.step();
    }
    let (mut w, _) = Sim::practice_wellhead_on(MapId::RustBelt, 3, 4, 0.5);
    for _ in 0..(60 * 10) {
        w.step();
    }
    let (mut l, _) = Sim::practice_last_crew_on(MapId::RustBelt, 9, 7, 0.6);
    for _ in 0..(60 * 10) {
        l.step();
    }
}

/// Both levels drive: over the bridge deck from bank to bank (never dropping
/// into the bed), and along the bed under it.
#[test]
fn the_bridge_carries_tanks_over_and_under() {
    let bz = arena_core::rustbelt::river_z(0.0);
    let (mut s, me) = Sim::practice_on(MapId::RustBelt, 4, 0, 0.5);
    let bed = s.map.height_at(0.0, bz);
    let drive = |s: &mut Sim, secs: f32, goal: Vec3| {
        let mut lowest = f32::MAX;
        for _ in 0..(secs * 60.0) as u32 {
            if let Some((p, _)) = s.tank_pose(me) {
                let to = goal - p;
                let yaw = s.hull_yaw(me).unwrap_or(0.0);
                let mut d = (-to.z).atan2(to.x) - yaw;
                while d > std::f32::consts::PI { d -= std::f32::consts::TAU; }
                while d < -std::f32::consts::PI { d += std::f32::consts::TAU; }
                s.set_input(me, arena_core::Input { throttle: 0.8, steer: (-d * 2.0).clamp(-1.0, 1.0), aim_point: goal, ..Default::default() });
                if (p.z - bz).abs() < 12.0 && p.x.abs() < 6.0 {
                    lowest = lowest.min(p.y);
                }
            }
            s.step();
        }
        (s.tank_pose(me).unwrap().0, lowest)
    };
    // Over: north bank to south bank along the road.
    s.teleport(me, 0.0, bz - 38.0, -std::f32::consts::FRAC_PI_2);
    let (end, lowest) = drive(&mut s, 12.0, Vec3::new(0.0, 0.0, bz + 45.0));
    assert!(end.z > bz + 28.0, "didn't cross the bridge: {end}");
    assert!(lowest > bed + 4.0, "fell off the deck: lowest {lowest}, bed {bed}");
    // Under: along the bed's north lane, west to east.
    s.teleport(me, -45.0, bz - 8.0, 0.0);
    let (end, _) = drive(&mut s, 12.0, Vec3::new(50.0, 0.0, bz - 8.0));
    assert!(end.x > 30.0 && end.y < bed + 2.0, "didn't pass under the bridge: {end}");
}
