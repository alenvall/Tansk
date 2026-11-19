//! Last Crew Rolling: no respawns, a sandstorm wall closing in.

use arena_core::sim::{Phase, Sim};
use arena_core::*;
use glam::Vec2;

fn run(sim: &mut Sim, seconds: f32) -> Vec<Event> {
    let mut ev = Vec::new();
    for _ in 0..(seconds * 60.0) as u32 {
        sim.step();
        ev.extend(sim.drain_events());
    }
    ev
}

fn idx(sim: &Sim, id: PlayerId) -> usize {
    sim.players.iter().position(|p| p.id == id).unwrap()
}

/// `n` idle crews (no bots).
fn crews(seed: u64, n: usize) -> (Sim, Vec<PlayerId>) {
    let mut s = Sim::new_last_crew(seed, Map::the_sink(seed), Tuning::default());
    let ids = (0..n).map(|k| s.add_player(&format!("c{k}"))).collect();
    (s, ids)
}

#[test]
fn the_wall_holds_then_closes_inside_the_old_circle() {
    let (mut s, _) = crews(3, 2);
    let t = s.tuning.last_crew.clone();
    let lc = s.last_crew.as_ref().unwrap();
    let (c0, r0) = (lc.center, lc.radius);
    assert!(r0 >= s.map.play_half * 1.4, "the first circle covers the map");
    let (to_c, to_r) = lc.to;
    assert!((to_r - r0 * t.factor[0]).abs() < 0.01);
    assert!(to_c.distance(c0) + to_r <= r0 + 0.01, "the next circle sits inside the current one");
    let ev = run(&mut s, t.hold[0] + 0.5);
    assert!(ev.iter().any(|e| matches!(e, Event::CircleShrinking { phase: 1, .. })));
    let snap = s.snapshot().storm_circle.unwrap();
    assert!(snap.shrinking && snap.radius < r0 && snap.phase == 1);
    let ev = run(&mut s, t.shrink[0]);
    assert!(ev.iter().any(|e| matches!(e, Event::CircleClosed { phase: 1 })));
    let lc = s.last_crew.as_ref().unwrap();
    assert!((lc.radius - to_r).abs() < 0.01 && lc.center.distance(to_c) < 0.01);
    // Phase 2's circle is inside phase 1's.
    assert!(lc.to.0.distance(lc.center) + lc.to.1 <= lc.radius + 0.01);
    assert!(lc.to.0.x.abs() < s.map.play_half && lc.to.0.y.abs() < s.map.play_half);
}

#[test]
fn every_phase_runs_to_a_closed_wall_in_about_six_minutes() {
    let (mut s, _) = crews(4, 2);
    let t = s.tuning.last_crew.clone();
    let total: f32 = t.hold.iter().sum::<f32>() + t.shrink.iter().sum::<f32>();
    assert!((240.0..=420.0).contains(&total), "the wall takes {total} s");
    // Keep both crews alive and inside (the test is about the wall).
    let mut closes = 0;
    for _ in 0..(total as u32 + 5) {
        for p in &mut s.players {
            p.tank.health = p.tank.stats.max_health;
        }
        for e in run(&mut s, 1.0) {
            if matches!(e, Event::CircleClosed { .. }) {
                closes += 1;
            }
        }
    }
    assert_eq!(closes, t.factor.len());
    let snap = s.snapshot().storm_circle.unwrap();
    assert!(snap.radius < 0.5 && !snap.shrinking, "{snap:?}");
}

#[test]
fn outside_the_wall_hurts_and_nobody_respawns() {
    let (mut s, ids) = crews(5, 3);
    // Pull the wall in round the middle; crew 0 stands outside it.
    {
        let lc = s.last_crew.as_mut().unwrap();
        lc.center = Vec2::ZERO;
        lc.radius = 60.0;
        lc.from = (Vec2::ZERO, 60.0);
    }
    s.teleport(ids[0], 150.0, 0.0, 0.0);
    s.teleport(ids[1], 10.0, 0.0, 0.0);
    s.teleport(ids[2], -10.0, 5.0, 0.0);
    let hp0 = s.tank(ids[0]).unwrap().health;
    run(&mut s, 2.0);
    assert!(s.tank(ids[0]).unwrap().health < hp0 - 5.0, "the storm bites");
    assert_eq!(s.tank(ids[1]).unwrap().health, s.tank(ids[1]).unwrap().stats.max_health, "inside is safe");
    let ev = run(&mut s, 120.0);
    let j = idx(&s, ids[0]);
    assert!(!s.players[j].tank.alive);
    assert!(ev.iter().any(|e| matches!(e, Event::Destroyed { victim, weapon: Weapon::Storm, killer: None, .. } if *victim == ids[0])));
    assert!(ev.iter().any(|e| matches!(e, Event::Eliminated { player, place: 3 } if *player == ids[0])));
    assert!(s.players[j].tank.respawn_in.is_infinite(), "out for good");
    run(&mut s, 10.0);
    assert!(!s.players[idx(&s, ids[0])].tank.alive);
    assert_eq!(s.crews_alive(), 2);
    assert_eq!(s.snapshot().storm_circle.unwrap().alive, 2);
}

#[test]
fn the_last_tank_rolling_wins_and_a_new_match_starts_everyone() {
    let (mut s, ids) = crews(6, 3);
    run(&mut s, 3.0);
    for &v in &ids[..2] {
        let j = idx(&s, v);
        s.players[j].tank.protect = 0.0;
        s.damage(j, Some(ids[2]), HitLocation::Hull, 1.0e6, Vec3::ZERO, Vec3::X, Weapon::Ram);
    }
    let ev = run(&mut s, 0.1);
    assert!(ev.iter().any(|e| matches!(e, Event::LastCrew { player } if *player == ids[2])));
    assert!(ev.iter().any(|e| matches!(e, Event::MatchEnded { winner: Some(w) } if *w == ids[2])));
    assert_eq!(s.phase, Phase::Ended { winner: Some(ids[2]) });
    let lc = s.last_crew.as_ref().unwrap();
    assert_eq!(lc.kills.len(), 2, "two gun kills logged with their distance to the wall");
    s.reset_match();
    run(&mut s, 0.1);
    assert!(s.players.iter().all(|p| p.tank.alive));
    assert_eq!(s.phase, Phase::Playing);
    assert!(s.last_crew.as_ref().unwrap().out.is_empty());
}

#[test]
fn late_joiners_wait_for_the_next_match() {
    let (mut s, _) = crews(7, 2);
    let wait = s.tuning.last_crew.late_join + 1.0;
    run(&mut s, wait);
    let late = s.add_player("late");
    run(&mut s, 1.0);
    let j = idx(&s, late);
    assert!(!s.players[j].tank.alive && s.players[j].tank.respawn_in.is_infinite());
    assert_eq!(s.crews_alive(), 2);
    s.reset_match();
    run(&mut s, 0.1);
    assert!(s.players[idx(&s, late)].tank.alive);
}

#[test]
fn scrap_comes_back_fast_early() {
    let (mut s, _) = crews(8, 1);
    // Every site live at the start; take them all, then count the comeback.
    s.pickups.clear();
    let respawn = s.tuning.pickups.site_respawn;
    for site in &mut s.sites {
        site.live = None;
        site.timer = respawn;
    }
    let wait = respawn / s.tuning.last_crew.scrap_boost + 0.5;
    run(&mut s, wait);
    let scrap = |x: &&arena_core::sim::Site| matches!(x.kind, PickupKind::Scrap(_));
    let n = s.sites.iter().filter(scrap).count();
    let back = s.sites.iter().filter(scrap).filter(|x| x.live.is_some()).count();
    assert!(n > 20 && back == n, "{back}/{n} scrap sites back");
}

#[test]
fn snapshot_carries_the_wall_and_bots_stay_inside() {
    let (mut s, _me) = Sim::practice_last_crew_on(MapId::Refinery, 9, 7, 0.6);
    assert_eq!(s.players.len(), 8);
    assert!(s.players.iter().all(|p| p.team.is_none()));
    // A tight circle off to one side, closing now: bots head in.
    {
        let lc = s.last_crew.as_mut().unwrap();
        lc.center = Vec2::new(60.0, 40.0);
        lc.radius = 150.0;
        lc.from = (lc.center, 150.0);
        lc.to = (Vec2::new(80.0, 60.0), 70.0);
        lc.timer = 1.0;
    }
    run(&mut s, 45.0);
    let lc = s.last_crew.as_ref().unwrap();
    let bots: Vec<_> = s.players.iter().filter(|p| p.brain.is_some() && p.tank.alive).collect();
    let inside = bots.iter().filter(|p| lc.inside(s.phys.body(p.tank.body).translation()) > 0.0).count();
    assert!(inside * 4 >= bots.len() * 3, "{inside}/{} living bots inside the wall", bots.len());
    let snap = s.snapshot();
    let back = arena_core::snapshot::Snapshot::decode(&snap.encode()).unwrap();
    assert_eq!(back, snap);
    let c = back.storm_circle.unwrap();
    assert!(c.radius > 60.0 && c.alive as usize == bots.len() + s.players.iter().filter(|p| p.brain.is_none() && p.tank.alive).count());
    assert!(back.wellhead.is_none() && back.teams.is_empty());
}
