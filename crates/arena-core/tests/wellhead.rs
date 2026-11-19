//! Wellhead: hold the derricks, pump fuel, fill the tank first.

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

/// Two idle human crews, one per team, parked far from every well.
fn duo(seed: u64) -> (Sim, PlayerId, PlayerId) {
    let mut s = Sim::new_wellhead(seed, Map::the_sink(seed), Tuning::default());
    let a = s.add_player("a");
    let b = s.add_player("b");
    assert_eq!(s.team_of(a), Some(0));
    assert_eq!(s.team_of(b), Some(1));
    s.teleport(a, -230.0, 200.0, 0.0);
    s.teleport(b, -200.0, 220.0, 0.0);
    (s, a, b)
}

fn well(s: &Sim, k: usize) -> Vec3 {
    s.wellhead.as_ref().unwrap().wells[k].pos
}

#[test]
fn both_maps_have_three_open_wells_and_split_spawns() {
    for id in MapId::ALL {
        let map = Map::by_id(id, 5);
        assert_eq!(map.wells.len(), 3, "{id:?}");
        for w in &map.wells {
            assert!(!map.blocked_xz(Vec2::new(w.pos.x, w.pos.z), 3.0), "{id:?}: {} is blocked", w.name);
            assert!(w.pos.x.abs() < map.play_half - 20.0 && w.pos.z.abs() < map.play_half - 20.0);
        }
        // Wells far enough apart that holding two means being in two places.
        for i in 0..3 {
            for j in i + 1..3 {
                assert!(map.wells[i].pos.distance(map.wells[j].pos) > 120.0, "{id:?} wells {i} {j} too close");
            }
        }
        // Each team gets half the spawns, on the side of its own well.
        let s = Sim::new_wellhead(5, map.clone(), Tuning::default());
        let (o, axis) = s.wellhead_split().unwrap();
        let west = map.spawns.iter().filter(|sp| (Vec2::new(sp.pos.x, sp.pos.z) - o).dot(axis) < 0.0).count();
        assert!(west >= 3 && map.spawns.len() - west >= 3, "{id:?}: {west} of {} spawns on team 0's side", map.spawns.len());
        let own = |k: usize| (Vec2::new(map.wells[k].pos.x, map.wells[k].pos.z) - o).dot(axis);
        assert!(own(0) < 0.0 && own(2) > 0.0, "{id:?}: each side's well on its side");
    }
}

#[test]
fn a_crew_takes_a_well_and_the_other_side_flips_it() {
    let (mut s, a, b) = duo(3);
    let w = well(&s, 1);
    let ct = s.tuning.wellhead.capture_time;
    s.teleport(a, w.x + 3.0, w.z, 0.0);
    let ev = run(&mut s, ct * 0.5);
    let wh = s.wellhead.as_ref().unwrap();
    assert!(wh.wells[1].control < -0.3 && wh.wells[1].owner.is_none(), "half way: {:?}", wh.wells[1].control);
    let ev2 = run(&mut s, ct * 0.6 + 0.5);
    assert!(ev.iter().chain(&ev2).any(|e| matches!(e, Event::WellCaptured { well: 1, team: 0 })));
    assert_eq!(s.wellhead.as_ref().unwrap().wells[1].owner, Some(0));
    // Both sides in the ring: contested, the ring doesn't move.
    s.teleport(b, w.x - 3.0, w.z, 3.1);
    let ev = run(&mut s, 2.0);
    assert!(ev.iter().any(|e| matches!(e, Event::WellContested { well: 1 })));
    let snap = s.snapshot();
    let ws = &snap.wellhead.as_ref().unwrap().wells[1];
    assert!(ws.contested && ws.owner == Some(0) && ws.progress <= -0.99, "{ws:?}");
    // The Rustback leaves: the Dust Saint neutralises it, then takes it.
    s.teleport(a, -230.0, 200.0, 0.0);
    let ev = run(&mut s, ct * 2.0 + 1.0);
    let lost = ev.iter().position(|e| matches!(e, Event::WellLost { well: 1, team: 0 })).expect("lost");
    let taken = ev.iter().position(|e| matches!(e, Event::WellCaptured { well: 1, team: 1 })).expect("taken");
    assert!(lost < taken);
    assert_eq!(s.wellhead.as_ref().unwrap().wells[1].owner, Some(1));
}

#[test]
fn held_wells_pump_and_all_three_pump_double() {
    let (mut s, a, _b) = duo(4);
    // Hand team 0 one well: +1/s.
    s.wellhead.as_mut().unwrap().wells[0].owner = Some(0);
    s.wellhead.as_mut().unwrap().wells[0].control = -1.0;
    run(&mut s, 10.0);
    let f = s.wellhead.as_ref().unwrap().fuel;
    assert!((f[0] - 10.0).abs() < 0.3 && f[1] == 0.0, "{f:?}");
    // All three: an AllWellsHeld call and 2 x 3 per second.
    for w in &mut s.wellhead.as_mut().unwrap().wells {
        w.owner = Some(0);
        w.control = -1.0;
    }
    let ev = run(&mut s, 5.0);
    assert!(ev.iter().any(|e| matches!(e, Event::AllWellsHeld { team: 0 })));
    let g = s.wellhead.as_ref().unwrap().fuel;
    assert!((g[0] - f[0] - 30.0).abs() < 0.5, "{g:?} vs {f:?}");
    assert_eq!(s.snapshot().wellhead.unwrap().all_held, Some(0));
    // Fill the tank: the Rustbacks win.
    let target = s.tuning.wellhead.target;
    let ev = run(&mut s, (target - g[0]) / 6.0 + 1.0);
    assert!(ev.iter().any(|e| matches!(e, Event::TeamMatchEnded { team: Some(0) })));
    assert_eq!(s.phase, Phase::TeamWon { team: Some(0) });
    let snap = s.snapshot();
    assert_eq!(snap.wellhead.as_ref().unwrap().fuel[0] as f32, target);
    let _ = a;
    // A new match: neutral wells, empty tanks.
    s.reset_match();
    let wh = s.wellhead.as_ref().unwrap();
    assert!(wh.wells.iter().all(|w| w.owner.is_none()) && wh.fuel == [0.0, 0.0]);
}

#[test]
fn the_clock_decides_on_fuel() {
    let (mut s, _, _) = duo(6);
    s.wellhead.as_mut().unwrap().fuel = [120.0, 140.0];
    s.tuning.wellhead.time_limit = 2.0;
    let ev = run(&mut s, 2.5);
    assert!(ev.iter().any(|e| matches!(e, Event::TeamMatchEnded { team: Some(1) })));
}

#[test]
fn snapshot_carries_the_wells() {
    let (mut s, _, _) = duo(7);
    run(&mut s, 1.0);
    let snap = s.snapshot();
    let back = arena_core::snapshot::Snapshot::decode(&snap.encode()).unwrap();
    assert_eq!(back, snap);
    let w = back.wellhead.unwrap();
    assert_eq!(w.wells.len(), 3);
    assert_eq!(w.target as f32, s.tuning.wellhead.target);
    assert!(back.storm_circle.is_none());
    assert!(!back.teams.is_empty(), "team scores so the HUD knows it's a team game");
}

#[test]
fn bots_take_wells_on_both_maps() {
    for id in MapId::ALL {
        let (mut s, _me) = Sim::practice_wellhead_on(id, 11, 4, 0.6);
        assert_eq!(s.players.iter().filter(|p| p.team == Some(0)).count(), 4);
        assert_eq!(s.players.iter().filter(|p| p.team == Some(1)).count(), 4);
        let ev = run(&mut s, 120.0);
        let caps = ev.iter().filter(|e| matches!(e, Event::WellCaptured { .. })).count();
        assert!(caps >= 2, "{id:?}: {caps} captures in two minutes");
        let teams: std::collections::BTreeSet<u8> =
            ev.iter().filter_map(|e| if let Event::WellCaptured { team, .. } = e { Some(*team) } else { None }).collect();
        assert!(teams.len() == 2 || caps >= 3, "{id:?}: only {teams:?} took wells");
    }
}
