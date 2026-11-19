//! Convoy: escort the fuel tanker, or stop it.

use arena_core::convoy::{convoy_winner, ConvoyPhase, ConvoyRound};
use arena_core::sim::Sim;
use arena_core::*;

fn idx(sim: &Sim, id: PlayerId) -> usize {
    sim.players.iter().position(|p| p.id == id).unwrap()
}

fn run(sim: &mut Sim, seconds: f32) -> Vec<Event> {
    let mut ev = Vec::new();
    for _ in 0..(seconds * 60.0) as u32 {
        sim.step();
        ev.extend(sim.drain_events());
    }
    ev
}

/// A convoy with two idle human crews: `esc` on team 0 (escorting), `att` on team 1.
fn duo(seed: u64) -> (Sim, PlayerId, PlayerId, PlayerId) {
    let mut s = Sim::new_convoy(seed, Map::the_sink(seed), Tuning::default());
    let esc = s.add_player("esc");
    let att = s.add_player("att");
    assert_eq!(s.team_of(esc), Some(0));
    assert_eq!(s.team_of(att), Some(1));
    let truck = s.convoy.as_ref().unwrap().truck;
    (s, esc, att, truck)
}

/// Park a crew far from the route's start (out of everyone's way).
fn park_far(s: &mut Sim, id: PlayerId, x: f32, z: f32) {
    s.teleport(id, x, z, 0.0);
}

fn truck_pos(s: &Sim) -> Vec3 {
    let t = s.convoy.as_ref().unwrap().truck;
    s.tank_pose(t).unwrap().0
}

#[test]
fn routes_follow_the_roads_on_both_maps() {
    for id in MapId::ALL {
        let map = Map::by_id(id, 3);
        assert!(map.convoy_route.len() >= 2, "{id:?} has a route");
        let a = map.convoy_route[0];
        let b = *map.convoy_route.last().unwrap();
        assert!(a.distance(b) > 400.0, "{id:?} route runs across the map");
        let s = Sim::new_convoy(3, map, Tuning::default());
        let c = s.convoy.as_ref().unwrap();
        assert!(c.length > 450.0 && c.length < 900.0, "{id:?} route length {}", c.length);
        // The joined path stays drivable: no point inside an obstacle.
        for p in s.convoy_path() {
            assert!(!s.map.blocked_xz(glam::Vec2::new(p.x, p.z), 0.5), "{id:?} route point {p} is blocked");
        }
    }
    // The Sink's route is Highway 9: its control points sit on the road line.
    let sink = Map::the_sink(3);
    let on_road = sink
        .convoy_route
        .iter()
        .filter(|p| sink.obstacles.iter().any(|o| o.kind == ObstacleKind::Highway && glam::Vec2::new(o.pos.x - p.x, o.pos.z - p.z).length() < 12.0))
        .count();
    assert!(on_road * 10 >= sink.convoy_route.len() * 8, "{on_road}/{} sink route points on the highway", sink.convoy_route.len());
}

#[test]
fn practice_sets_up_two_teams_and_a_truck() {
    let (mut s, me) = Sim::practice_convoy_on(MapId::Sink, 31, 4, 0.5);
    let truck = s.convoy.as_ref().unwrap().truck;
    assert!(s.is_convoy_truck(truck) && !s.is_convoy_truck(me));
    let crews: Vec<_> = s.players.iter().filter(|p| p.id != truck).collect();
    assert_eq!(crews.len(), 8);
    assert_eq!(crews.iter().filter(|p| p.team == Some(0)).count(), 4);
    assert_eq!(crews.iter().filter(|p| p.team == Some(1)).count(), 4);
    assert_eq!(s.team_of(me), Some(0), "you escort first");
    let t = s.tank(truck).unwrap();
    assert_eq!(t.vehicle, VehicleKind::FuelTanker);
    assert_eq!(s.team_of(truck), Some(0));
    assert!(s.players().iter().any(|p| p.id == truck && p.is_bot), "the truck lists as a bot");
    // Escorts start by the truck, attackers well up the road.
    let tp = truck_pos(&s);
    for p in &s.players {
        if p.id == truck {
            continue;
        }
        let d = s.phys.body(p.tank.body).translation().distance(tp);
        if p.team == Some(0) {
            assert!(d < 60.0, "escort {} spawned {d:.0} m from the truck", p.name);
        } else {
            assert!(d > 80.0, "attacker {} spawned {d:.0} m from the truck", p.name);
        }
    }
    s.step();
    let snap = s.snapshot();
    let cs = snap.convoy.as_ref().unwrap();
    assert_eq!((cs.round, cs.side, cs.phase), (1, 0, ConvoyPhase::Setup));
    assert_eq!(cs.truck, truck);
    assert!(cs.health > 0 && cs.health == cs.max_health);
    assert!(cs.ticks_left > 0);
    assert!(snap.tank(truck).is_some_and(|t| t.vehicle == VehicleKind::FuelTanker));
    assert_eq!(Snapshot::decode(&snap.encode()).unwrap(), snap);
    let ev = s.drain_events();
    let _ = ev;
}

#[test]
fn the_truck_rolls_stops_when_held_and_rolls_again_with_escorts() {
    let (mut s, esc, att, truck) = duo(32);
    park_far(&mut s, esc, 0.0, 150.0);
    park_far(&mut s, att, 40.0, 160.0);
    let setup = Tuning::default().convoy.setup_time;
    let ev = run(&mut s, setup + 0.5);
    assert!(ev.iter().any(|e| matches!(e, Event::ConvoyMoving { truck: t, .. } if *t == truck)));
    let ev = run(&mut s, 10.0);
    let _ = ev;
    let d0 = s.convoy.as_ref().unwrap().progress;
    assert!(d0 > 15.0, "the truck drives itself: {d0:.1} m");
    assert!(s.snapshot().convoy.unwrap().moving);

    // An attacker alongside, no escort: it stops.
    let p = truck_pos(&s);
    s.teleport(att, p.x + 6.0, p.z + 14.0, 0.0);
    let ev = run(&mut s, 2.0);
    assert!(ev.iter().any(|e| matches!(e, Event::ConvoyStopped { .. })));
    let ev = run(&mut s, 4.0);
    let _ = ev;
    let snap = s.snapshot();
    let cs = snap.convoy.as_ref().unwrap();
    assert!(!cs.moving && cs.contested);
    let speed = s.phys.body(s.tank(truck).unwrap().body).linvel().length();
    assert!(speed < 1.0, "held truck still doing {speed:.1} m/s");

    // An escort arrives: it rolls again.
    let p = truck_pos(&s);
    s.teleport(esc, p.x - 6.0, p.z - 12.0, 0.0);
    let ev = run(&mut s, 2.0);
    assert!(ev.iter().any(|e| matches!(e, Event::ConvoyMoving { .. })));
    let d1 = s.convoy.as_ref().unwrap().progress;
    let ev = run(&mut s, 6.0);
    let _ = ev;
    assert!(s.convoy.as_ref().unwrap().progress > d1 + 5.0, "rolling again");
}

#[test]
fn escorts_patch_the_truck_and_its_armour_shrugs_off_bullets() {
    let (mut s, esc, att, truck) = duo(33);
    park_far(&mut s, att, 40.0, 160.0);
    park_far(&mut s, esc, 0.0, 150.0);
    run(&mut s, Tuning::default().convoy.setup_time + 1.0);
    let ti = idx(&s, truck);
    let pos = truck_pos(&s);
    let h0 = s.players[ti].tank.health;
    s.damage(ti, Some(att), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::Gun(GunKind::HeavyCannon));
    let shell = h0 - s.players[ti].tank.health;
    let h1 = s.players[ti].tank.health;
    s.damage(ti, Some(att), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::Alt(AltKind::Mg));
    let bullet = h1 - s.players[ti].tank.health;
    assert!(shell > 0.0 && shell < 60.0, "armoured: a 100 shell did {shell}");
    assert!(bullet < shell * 0.5, "bullets plink: {bullet} vs {shell}");
    // Tyres can't be shot off (a stalled run is no fun).
    s.damage(ti, Some(att), HitLocation::Track(0), 500.0, pos, Vec3::X, Weapon::Gun(GunKind::HeavyCannon));
    assert!(s.players[ti].tank.tracks[0].blown <= 0.0);
    // No friendly fire from the escort.
    let h2 = s.players[ti].tank.health;
    s.damage(ti, Some(esc), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::World);
    assert_eq!(s.players[ti].tank.health, h2);
    // An escort alongside patches it up.
    let p = truck_pos(&s);
    s.teleport(esc, p.x, p.z + 8.0, 0.0);
    run(&mut s, 4.0);
    let ti = idx(&s, truck);
    assert!(s.players[ti].tank.health > h2 + 20.0, "repaired: {h2} -> {}", s.players[ti].tank.health);
}

#[test]
fn a_destroyed_truck_ends_the_round_and_the_sides_swap() {
    let (mut s, esc, att, truck) = duo(34);
    park_far(&mut s, esc, 0.0, 150.0);
    park_far(&mut s, att, 40.0, 160.0);
    let ct = Tuning::default().convoy;
    run(&mut s, ct.setup_time + 8.0);
    let ti = idx(&s, truck);
    let pos = truck_pos(&s);
    s.damage(ti, Some(att), HitLocation::Hull, 1.0e7, pos, Vec3::X, Weapon::Gun(GunKind::HeavyCannon));
    let ev = run(&mut s, 0.2);
    assert!(ev.iter().any(|e| matches!(e, Event::ConvoyDestroyed { killer: Some(k), .. } if *k == att)));
    assert!(ev.iter().any(|e| matches!(e, Event::ConvoyRoundEnded { round: 1, escort_team: 0, delivered: false, .. })));
    assert_eq!(s.snapshot().convoy.unwrap().phase, ConvoyPhase::RoundOver);
    assert!(!s.tank(truck).unwrap().alive);
    let ev = run(&mut s, ct.intermission + 0.5);
    assert!(ev.iter().any(|e| matches!(e, Event::ConvoyRoundStarted { round: 2, escort_team: 1, .. })));
    // Round 2: the truck is back at the start for team 1; the old escort now attacks.
    assert!(s.tank(truck).unwrap().alive);
    assert_eq!(s.team_of(truck), Some(1));
    let snap = s.snapshot();
    let cs = snap.convoy.as_ref().unwrap();
    assert_eq!((cs.round, cs.side, cs.phase), (2, 1, ConvoyPhase::Setup));
    assert_eq!(cs.rounds.len(), 1);
    assert!(cs.distance < 1.0);
    let tp = truck_pos(&s);
    let d_att_now = s.tank_pose(att).unwrap().0.distance(tp);
    assert!(d_att_now < 60.0, "team 1 escorts now: spawned by the truck ({d_att_now:.0} m)");
    // Team 1 can hurt the truck no longer; team 0 can.
    let ti = idx(&s, truck);
    run(&mut s, 2.5);
    let h = s.players[ti].tank.health;
    s.damage(ti, Some(att), HitLocation::Hull, 100.0, tp, Vec3::X, Weapon::World);
    assert_eq!(s.players[ti].tank.health, h);
    s.damage(ti, Some(esc), HitLocation::Hull, 100.0, tp, Vec3::X, Weapon::World);
    assert!(s.players[ti].tank.health < h);
}

#[test]
fn delivering_wins_and_ties_go_by_distance() {
    let (mut s, esc, att, truck) = duo(35);
    park_far(&mut s, esc, 0.0, 150.0);
    park_far(&mut s, att, 40.0, 160.0);
    let ct = Tuning::default().convoy;
    run(&mut s, ct.setup_time + 1.0);
    // Round 1: put the truck at the end of the road.
    let end = *s.convoy_path().last().unwrap();
    s.teleport(truck, end.x, end.z, 0.0);
    let ev = run(&mut s, 0.5);
    assert!(ev.iter().any(|e| matches!(e, Event::ConvoyDelivered { escort_team: 0, .. })));
    // (and past the respawned truck's spawn protection)
    run(&mut s, ct.intermission + 2.5);
    // Round 2: team 1's truck is blown up early: team 0 wins the match.

    let ti = idx(&s, truck);
    let pos = truck_pos(&s);
    s.damage(ti, Some(esc), HitLocation::Hull, 1.0e7, pos, Vec3::X, Weapon::World);
    let ev = run(&mut s, 0.2);
    assert!(ev.iter().any(|e| matches!(e, Event::TeamMatchEnded { team: Some(0) })));
    assert_eq!(s.phase, Phase::TeamWon { team: Some(0) });
    assert_eq!(s.snapshot().convoy.unwrap().phase, ConvoyPhase::Done);

    // The tie-breaks.
    let r = |escort_team, delivered, distance, seconds| ConvoyRound { escort_team, delivered, distance, seconds };
    assert_eq!(convoy_winner(&[r(0, false, 300.0, 100.0), r(1, false, 420.0, 120.0)]), Some(1), "further wins");
    assert_eq!(convoy_winner(&[r(0, true, 580.0, 190.0), r(1, true, 580.0, 170.0)]), Some(1), "faster wins");
    assert_eq!(convoy_winner(&[r(0, true, 580.0, 190.0), r(1, false, 579.0, 300.0)]), Some(0));
    assert_eq!(convoy_winner(&[r(0, false, 300.0, 100.0), r(1, false, 300.4, 120.0)]), None, "a draw");

    // reset_match starts over from round 1.
    s.reset_match();
    let cs = s.snapshot().convoy.unwrap();
    assert_eq!((cs.round, cs.side, cs.phase), (1, 0, ConvoyPhase::Setup));
    assert!(cs.rounds.is_empty() && s.tank(truck).unwrap().alive);
    assert_eq!(s.team_of(truck), Some(0));
    assert_eq!(s.phase, Phase::Playing);
}

#[test]
fn a_bot_convoy_match_plays_out() {
    let (mut s, _me) = Sim::practice_convoy_on(MapId::Refinery, 36, 4, 0.5);
    // The "human" is a bot here too.
    let me = s.players.iter().position(|p| p.brain.is_none() && !s.is_convoy_truck(p.id)).unwrap();
    s.players[me].brain = Some(arena_core::bots::Brain::new(99, 0.5));
    let mut ended = None;
    let mut stops = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for _ in 0..(14 * 60 * 60) {
        s.step();
        for e in s.drain_events() {
            match e {
                Event::TeamMatchEnded { team } => ended = Some(team),
                Event::ConvoyStopped { .. } => stops += 1,
                Event::ConvoyMoving { .. } => {
                    kinds.insert("moving");
                }
                Event::ConvoyHalfway { .. } => {
                    kinds.insert("halfway");
                }
                Event::ConvoyRoundEnded { .. } => {
                    kinds.insert("round");
                }
                _ => {}
            }
        }
        if ended.is_some() {
            break;
        }
    }
    let rounds = s.convoy.as_ref().unwrap().results.clone();
    println!("bot convoy: {rounds:?}, stops {stops}, ended {ended:?}");
    assert!(ended.is_some(), "both rounds finish");
    assert_eq!(rounds.len(), 2);
    assert_eq!((rounds[0].escort_team, rounds[1].escort_team), (0, 1));
    assert!(kinds.contains("moving") && kinds.contains("round"));
    assert!(rounds.iter().any(|r| r.distance > 100.0), "the truck got somewhere");
    assert!(s.players.iter().all(|p| p.tank.rescues == 0), "nobody fell through the floor");
}
