//! The Gorge: the level rules, its three heights, its places, and that they
//! all connect.

use arena_core::canyon::{gorge_half, gorge_z, MESA, RIM};
use arena_core::map::{Cover, Map, MapId, ObstacleKind};
use arena_core::Sim;
use glam::{Vec2, Vec3};

#[test]
fn the_gorge_follows_the_level_rules() {
    let map = Map::gorge(1);
    assert_eq!(map.id, MapId::Canyon);
    assert_eq!(map, Map::by_id(MapId::Canyon, 1));
    assert_eq!(MapId::from_key("gorge"), Some(MapId::Canyon));
    assert_eq!(map.spawns.len(), 8);
    assert!(map.scrap_sites.len() >= 40);
    for o in &map.obstacles {
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
    // Three heights: the floor, the rims, the mesa tops (and the butte at rim height).
    let floor = map.height_at(-200.0, gorge_z(-200.0));
    let rim = map.height_at(0.0, -150.0);
    let mesa = map.height_at(-150.0, -205.0);
    let butte = map.height_at(0.0, 0.0);
    assert!((rim - floor - RIM).abs() < 2.0, "rim {rim}, floor {floor}");
    assert!((mesa - floor - MESA).abs() < 2.0, "mesa {mesa}, floor {floor}");
    assert!((butte - rim).abs() < 2.0, "butte {butte}, rim {rim}");
    // The girder bridges: clear underneath, flush at both ends.
    let bridges: Vec<_> = map.obstacles.iter().filter(|o| o.kind == ObstacleKind::GirderBridge).collect();
    assert_eq!(bridges.len(), 2);
    for br in bridges {
        let (c, _, h) = br.world_boxes().next().unwrap();
        let clear = c.y - h.y - map.height_at(c.x, c.z);
        assert!(clear > 6.0, "bridge only {clear} m clear");
        for end in [-1.0f32, 1.0] {
            let p = c + Vec3::Z * end * (h.z - 1.0);
            let step = c.y + h.y - map.height_at(p.x, p.z);
            assert!(step.abs() < 0.6, "a bridge deck is {step} m off the ground at {p}");
        }
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
fn the_gorge_has_no_slots() {
    for seed in [1u64, 7, 21] {
        let map = Map::gorge(seed);
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
fn the_gorge_has_its_places_and_they_connect() {
    let map = Map::gorge(21);
    let names: Vec<&str> = map.regions.iter().map(|r| r.name.as_str()).collect();
    for n in ["The Butte", "The West Gorge", "The East Gorge", "The North Rim", "The South Rim", "Red Mesa", "Eagle Rock", "Table Top", "The Anvil"] {
        assert!(names.contains(&n), "missing region {n}");
    }
    for k in [ObstacleKind::RockArch, ObstacleKind::GirderBridge, ObstacleKind::SpringPad, ObstacleKind::FuelTrailer] {
        assert!(map.obstacles.iter().any(|o| o.kind == k), "no {k:?}");
    }
    println!("The Gorge: {} obstacles, {} boxes", map.obstacles.len(), map.obstacles.iter().map(|o| o.boxes.len()).sum::<usize>());
    for r in &map.regions {
        let n = map.scrap_sites.iter().filter(|s| s.pos.distance(r.center) < r.radius).count();
        assert!(n >= 3, "{} has only {n} scrap sites", r.name);
    }
    assert_eq!(map.wells.len(), 3);
    assert!(map.convoy_route.len() >= 10);
    let nav = arena_core::bots::NavGrid::build(&map, 4.0);
    let o = Vec2::new(map.outpost.x, map.outpost.z);
    assert!(!map.blocked_xz(o, 16.0), "the outpost courtyard must be open");
    for (i, s) in map.spawns.iter().enumerate() {
        let p = Vec2::new(s.pos.x, s.pos.z);
        assert!(!map.blocked_xz(p, 5.0), "spawn {i} is blocked");
        assert!(nav.reachable(p, o), "spawn {i} at {p} can't drive to the outpost");
        for t in map.spawns.iter().skip(i + 1) {
            assert!(t.pos.distance(s.pos) > 90.0, "spawns too close");
        }
        assert!(map.obstacles.iter().any(|ob| ob.cover != Cover::Flat && Vec2::new(ob.pos.x, ob.pos.z).distance(p) < 30.0), "spawn {i} has no cover near");
    }
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let p = Vec2::new(a.cos(), a.sin()) * 255.0;
        if !map.blocked_xz(p, 4.0) && !nav.is_blocked(p) {
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
    // Every height drives to every other: floor, both rims, all four mesa tops.
    let floor_w = Vec2::new(-220.0, gorge_z(-220.0));
    for (name, p) in [("north rim", Vec2::new(0.0, -150.0)), ("south rim", Vec2::new(0.0, 150.0)), ("butte", o)] {
        assert!(nav.reachable(floor_w, p), "the floor doesn't reach the {name}");
    }
    for r in map.regions.iter().filter(|r| r.radius == 50.0) {
        assert!(nav.reachable(floor_w, Vec2::new(r.center.x, r.center.z)), "{} unreachable", r.name);
    }
    let _ = gorge_half;
}

/// The modes run on the map.
#[test]
fn the_gorge_runs_its_modes() {
    let (mut s, _me) = Sim::practice_convoy_on(MapId::Canyon, 36, 4, 0.5);
    assert!(s.convoy_path().len() > 10);
    for _ in 0..(60 * 30) {
        s.step();
    }
    let (mut w, _) = Sim::practice_wellhead_on(MapId::Canyon, 3, 4, 0.5);
    for _ in 0..(60 * 10) {
        w.step();
    }
    let (mut l, _) = Sim::practice_last_crew_on(MapId::Canyon, 9, 7, 0.6);
    for _ in 0..(60 * 10) {
        l.step();
    }
    let (mut r, _) = Sim::practice_raid_on(MapId::Canyon, 4, 3, 0.5);
    for _ in 0..(60 * 20) {
        r.step();
    }
}
