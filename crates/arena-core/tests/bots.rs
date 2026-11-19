//! Bot behaviour and the sim's fast paths (batched suspension rays, the
//! cached A* scratch) that must give the same answers as the slow ones.

use arena_core::bots::{BotStats, Difficulty, NavGrid};
use arena_core::physics::ground_query;
use arena_core::sim::Sim;
use arena_core::*;
use glam::Vec2;

#[test]
fn difficulty_tiers_round_trip() {
    for d in Difficulty::ALL {
        assert_eq!(Difficulty::of(d.value()), d);
    }
    assert_eq!(Difficulty::of(0.0), Difficulty::Easy);
    assert_eq!(Difficulty::of(0.5), Difficulty::Normal);
    assert_eq!(Difficulty::of(1.0), Difficulty::Hard);
}

/// `Physics::rays_near` (one broad-phase query for a tank's wheels) hits what
/// one `Physics::ray` per wheel hits, on the ground, on cover and on tanks.
#[test]
fn batched_rays_match_single_rays() {
    for id in [MapId::Sink, MapId::Refinery] {
        let mut sim = Sim::new(5, Map::by_id(id, 5), Tuning::default());
        for k in 0..6 {
            sim.add_bot(bots::BOT_NAMES[k], 0.5);
        }
        for _ in 0..300 {
            sim.step();
        }
        let mut checked = 0;
        // Rays down from a grid of points, some over obstacles and tanks.
        for k in 0..400 {
            let x = -150.0 + (k as f32 * 37.3) % 300.0;
            let z = -150.0 + (k as f32 * 53.9) % 300.0;
            let top = sim.map.height_at(x, z) + 6.0;
            let origins: Vec<Vec3> = (0..10).map(|w| Vec3::new(x + (w % 5) as f32 * 1.2, top, z + (w / 5) as f32 * 3.0)).collect();
            let mut out = vec![None; origins.len()];
            sim.phys.rays_near(&origins, -Vec3::Y, 9.0, ground_query(), None, &mut out);
            for (o, got) in origins.iter().zip(&out) {
                let want = sim.phys.ray(*o, -Vec3::Y, 9.0, ground_query(), None);
                match (want, got) {
                    (None, None) => {}
                    (Some(a), Some(b)) => {
                        assert!((a.1 - b.1).abs() < 1e-4, "toi {} vs {}", a.1, b.1);
                        assert!(a.2.distance(b.2) < 1e-3);
                        checked += 1;
                    }
                    _ => panic!("hit mismatch at {o:?}: {want:?} vs {got:?}"),
                }
            }
        }
        assert!(checked > 1000);
        // And under each tank, excluding its own body (what the suspension does).
        for p in &sim.players {
            let c = sim.phys.body(p.tank.body).translation();
            let origins = [c + Vec3::new(2.0, 0.5, 1.4), c + Vec3::new(-2.0, 0.5, -1.4)];
            let mut out = [None; 2];
            sim.phys.rays_near(&origins, -Vec3::Y, 2.0, ground_query(), Some(p.tank.body), &mut out);
            for (o, got) in origins.iter().zip(&out) {
                let want = sim.phys.ray(*o, -Vec3::Y, 2.0, ground_query(), Some(p.tank.body));
                assert_eq!(want.map(|h| h.0), got.map(|h| h.0));
            }
        }
    }
}

/// The A* and string pulling `NavGrid::path` had before its scratch buffers
/// were cached: the Convoy route depends on it, so it must not change.
fn reference_path(nav: &NavGrid, from: Vec2, to: Vec2) -> Vec<Vec2> {
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;
    #[derive(Copy, Clone, PartialEq)]
    struct Node {
        f: f32,
        i: usize,
    }
    impl Eq for Node {}
    impl Ord for Node {
        fn cmp(&self, o: &Self) -> Ordering {
            o.f.total_cmp(&self.f).then(self.i.cmp(&o.i))
        }
    }
    impl PartialOrd for Node {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }
    let n = nav.n;
    let cell_of = |p: Vec2| {
        let x = ((p.x - nav.origin) / nav.cell).floor().clamp(0.0, (n - 1) as f32) as usize;
        let z = ((p.y - nav.origin) / nav.cell).floor().clamp(0.0, (n - 1) as f32) as usize;
        (x, z)
    };
    let center = |x: usize, z: usize| Vec2::new(nav.origin + (x as f32 + 0.5) * nav.cell, nav.origin + (z as f32 + 0.5) * nav.cell);
    let nearest_free = |c: (usize, usize)| {
        if !nav.blocked[c.1 * n + c.0] {
            return c;
        }
        for r in 1..8i32 {
            for dz in -r..=r {
                for dx in -r..=r {
                    let x = c.0 as i32 + dx;
                    let z = c.1 as i32 + dz;
                    if x >= 0 && z >= 0 && (x as usize) < n && (z as usize) < n && !nav.blocked[z as usize * n + x as usize] {
                        return (x as usize, z as usize);
                    }
                }
            }
        }
        c
    };
    let s = nearest_free(cell_of(from));
    let g = nearest_free(cell_of(to));
    let si = s.1 * n + s.0;
    let gi = g.1 * n + g.0;
    if si == gi {
        return vec![to];
    }
    let mut cost = vec![f32::INFINITY; n * n];
    let mut came = vec![usize::MAX; n * n];
    let mut open = BinaryHeap::new();
    cost[si] = 0.0;
    let h = |i: usize| -> f32 {
        let dx = (i % n) as f32 - g.0 as f32;
        let dz = (i / n) as f32 - g.1 as f32;
        let (a, b) = (dx.abs(), dz.abs());
        a.max(b) + 0.414 * a.min(b)
    };
    open.push(Node { f: h(si), i: si });
    let mut found = false;
    while let Some(Node { i, .. }) = open.pop() {
        if i == gi {
            found = true;
            break;
        }
        let (x, z) = ((i % n) as i32, (i / n) as i32);
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let nx = x + dx;
            let nz = z + dz;
            if nx < 0 || nz < 0 || nx >= n as i32 || nz >= n as i32 {
                continue;
            }
            let j = nz as usize * n + nx as usize;
            if nav.blocked[j] {
                continue;
            }
            if dx != 0 && dz != 0 && (nav.blocked[z as usize * n + nx as usize] || nav.blocked[nz as usize * n + x as usize]) {
                continue;
            }
            let step = if dx != 0 && dz != 0 { 1.414 } else { 1.0 };
            let c = cost[i] + step;
            if c < cost[j] {
                cost[j] = c;
                came[j] = i;
                open.push(Node { f: c + h(j), i: j });
            }
        }
    }
    if !found {
        return vec![to];
    }
    let mut cells = vec![gi];
    let mut cur = gi;
    while came[cur] != usize::MAX && cur != si {
        cur = came[cur];
        cells.push(cur);
    }
    cells.reverse();
    let pts: Vec<Vec2> = cells.iter().map(|&i| center(i % n, i / n)).collect();
    let mut out = Vec::new();
    let mut anchor = from;
    let mut k = 0;
    while k < pts.len() {
        let mut far = k;
        for m in (k..pts.len()).rev() {
            if nav.line_clear(anchor, pts[m]) {
                far = m;
                break;
            }
        }
        out.push(pts[far]);
        anchor = pts[far];
        k = far + 1;
    }
    if !nav.is_blocked(to) {
        if let Some(last) = out.last_mut() {
            *last = to;
        }
    }
    out
}

/// `NavGrid::path` is what it was (the Convoy route is built from it), and
/// the bots' cheaper `path_into` gets to the same place.
#[test]
fn path_matches_the_reference() {
    for id in [MapId::Sink, MapId::Refinery] {
        let map = Map::by_id(id, 4);
        let nav = NavGrid::build(&map, 4.0);
        let mut pts: Vec<Vec2> = map.spawns.iter().map(|s| Vec2::new(s.pos.x, s.pos.z)).collect();
        pts.extend(map.convoy_route.iter().map(|p| Vec2::new(p.x, p.z)));
        pts.extend(map.scrap_sites.iter().take(12).map(|s| Vec2::new(s.pos.x, s.pos.z)));
        let mut mine = Vec::new();
        for a in &pts {
            for b in &pts {
                assert_eq!(nav.path(*a, *b), reference_path(&nav, *a, *b), "{a} -> {b}");
                nav.path_into(*a, *b, &mut mine);
                assert!(mine.last().unwrap().distance(*b) < 6.0 || nav.is_blocked(*b));
            }
        }
    }
}

/// Paths come back the same when the scratch buffers are reused, start near
/// `from` and end at `to`.
#[test]
fn paths_are_repeatable_and_reach_the_goal() {
    let map = Map::the_sink(3);
    let nav = NavGrid::build(&map, 4.0);
    let free: Vec<Vec2> = map.spawns.iter().map(|s| Vec2::new(s.pos.x, s.pos.z)).collect();
    for a in &free {
        for b in &free {
            let p1 = nav.path(*a, *b);
            let p2 = nav.path(*a, *b);
            assert_eq!(p1, p2);
            assert!(!p1.is_empty());
            assert!(p1.last().unwrap().distance(*b) < 6.0);
        }
    }
    assert!(!nav.covers.is_empty(), "the bots need cover to use");
}

fn play(seed: u64, team: bool, seconds: u32) -> (Sim, Vec<Event>) {
    let map = Map::the_sink(seed);
    let mut sim = if team { Sim::new_team_brawl(seed, map, Tuning::default()) } else { Sim::new(seed, map, Tuning::default()) };
    for k in 0..8 {
        sim.add_bot(bots::BOT_NAMES[k], Difficulty::ALL[k % 3].value());
    }
    let mut events = Vec::new();
    for _ in 0..seconds * 60 {
        sim.step();
        events.extend(sim.drain_events());
    }
    (sim, events)
}

fn total(sim: &Sim) -> BotStats {
    let mut s = BotStats::default();
    for b in sim.players.iter().filter_map(|p| p.brain.as_ref()) {
        let x = b.stats();
        s.flanks += x.flanks;
        s.covers += x.covers;
        s.peeks += x.peeks;
        s.ducks += x.ducks;
        s.evades += x.evades;
        s.honks += x.honks;
        s.cover_s += x.cover_s;
        s.alive_s += x.alive_s;
    }
    s
}

/// Over a few minutes of 8 bots, they fight like crews: they take cover,
/// flank, turn on whoever shoots them in the back, kill each other and honk
/// about it, and don't sit still.
#[test]
fn bots_use_cover_flank_and_celebrate() {
    let mut s = BotStats::default();
    let mut kills = 0;
    let mut horns = 0;
    for seed in [11, 12] {
        let (sim, events) = play(seed, false, 150);
        let t = total(&sim);
        s.flanks += t.flanks;
        s.covers += t.covers;
        s.evades += t.evades;
        s.cover_s += t.cover_s;
        s.alive_s += t.alive_s;
        kills += events.iter().filter(|e| matches!(e, Event::Destroyed { killer: Some(_), .. })).count();
        horns += events.iter().filter(|e| matches!(e, Event::Horn { .. })).count();
    }
    println!("{s:?}, kills {kills}, horns {horns}");
    assert!(kills >= 6, "only {kills} kills");
    assert!(s.flanks > 0, "nobody flanked");
    assert!(s.covers > 0, "nobody took cover");
    assert!(s.evades > 0, "nobody turned on a flanker");
    assert!(horns > 0, "nobody honked");
    assert!(s.cover_s < s.alive_s * 0.6, "bots camp behind cover too much");
}

/// Team Brawl: teammates gang up on the same enemy now and then.
#[test]
fn team_bots_focus_fire() {
    let (mut sim, _) = play(21, true, 90);
    let mut shared = 0;
    for _ in 0..60 * 60 {
        sim.step();
        sim.drain_events();
        if sim.tick % 30 != 0 {
            continue;
        }
        for team in 0..2u8 {
            let targets: Vec<u8> = sim
                .players
                .iter()
                .filter(|p| p.team == Some(team) && p.tank.alive)
                .filter_map(|p| p.brain.as_ref().and_then(|b| b.target()))
                .collect();
            if targets.iter().enumerate().any(|(i, t)| targets[i + 1..].contains(t)) {
                shared += 1;
            }
        }
    }
    assert!(shared > 10, "teammates never share a target ({shared})");
}
