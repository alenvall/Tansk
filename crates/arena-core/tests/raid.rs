//! Raider Waves, raider chassis, THE WAR RIG, bot styles.

use arena_core::raid::{LossReason, RaidPhase, TEAM_DEFENDERS, TEAM_RAIDERS};
use arena_core::sim::Sim;
use arena_core::*;

fn raid(seed: u64) -> Sim {
    Sim::new_raid(seed, Map::the_sink(seed), Tuning::default())
}

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

fn open(sim: &Sim, near: glam::Vec2) -> glam::Vec2 {
    for r in 0..80 {
        for k in 0..24 {
            let a = k as f32 * 0.2618;
            let p = near + glam::Vec2::new(a.cos(), a.sin()) * r as f32 * 4.0;
            if !sim.map.blocked_xz(p, 30.0) && sim.map.normal_at(p.x, p.y).y > 0.97 {
                return p;
            }
        }
    }
    panic!("no open spot")
}

#[test]
fn waves_start_and_raiders_come_from_the_edges() {
    let mut s = raid(1);
    let me = s.add_player("me");
    assert_eq!(s.team_of(me), Some(TEAM_DEFENDERS));
    let o = s.raid.as_ref().unwrap().outpost;
    let (p, _) = s.tank_pose(me).unwrap();
    assert!(p.distance(o) < 45.0, "defenders spawn at the outpost");
    let ev = run(&mut s, Tuning::default().raid.first_wave_delay + 6.0);
    assert!(ev.iter().any(|e| matches!(e, Event::WaveStarted { wave: 1, .. })));
    let raiders: Vec<_> = s.players.iter().filter(|p| p.team == Some(TEAM_RAIDERS)).collect();
    assert!(!raiders.is_empty());
    for r in &raiders {
        let c = s.phys.body(r.tank.body).translation();
        assert!(glam::Vec2::new(c.x, c.z).length() > 150.0, "raider spawned at {c}");
    }
    let snap = s.snapshot();
    let rs = snap.raid.as_ref().unwrap();
    assert_eq!(rs.wave, 1);
    assert_eq!(rs.phase, RaidPhase::Fighting);
    assert!(snap.tanks.iter().any(|t| t.team == TEAM_RAIDERS && t.vehicle == VehicleKind::Buggy));
    assert_eq!(Snapshot::decode(&snap.encode()).unwrap(), snap);
}

#[test]
fn buggies_are_fast_and_light() {
    let mut s = raid(2);
    let id = s.spawn_raider_now(VehicleKind::Buggy).unwrap();
    let c = open(&s, glam::Vec2::new(-60.0, 40.0));
    // Face away from the outpost (a buggy that reaches it blows itself up).
    let o = s.raid.as_ref().unwrap().outpost;
    let away = (c - glam::Vec2::new(o.x, o.z)).normalize();
    let yaw = (-away.y).atan2(away.x);
    s.teleport(id, c.x, c.y, yaw);
    let i = idx(&s, id);
    s.players[i].brain = None; // drive it by hand
    run(&mut s, 1.0);
    let mut top: f32 = 0.0;
    for _ in 0..(4 * 60) {
        s.set_input(id, Input { throttle: 1.0, ..Default::default() });
        s.step();
        let i = idx(&s, id);
        top = top.max(s.phys.body(s.players[i].tank.body).linvel().length());
    }
    let i = idx(&s, id);
    println!("buggy top speed {:.0} km/h, mass {:.0} kg", top * 3.6, s.players[i].tank.mass());
    assert!(top > 15.0, "buggy only reached {top} m/s");
    assert!(s.players[i].tank.mass() < 8_000.0);
    assert_eq!(s.players[i].tank.wheels.len(), 4);
}

#[test]
fn war_rig_modules_shoot_off_and_change_it() {
    let mut s = raid(3);
    let me = s.add_player("me");
    let boss = s.spawn_raider_now(VehicleKind::WarRig).unwrap();
    let ev = s.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::BossSpawned { .. })));
    let b = idx(&s, boss);
    assert_eq!(s.players[b].tank.parts.len(), 9);
    assert_eq!(s.players[b].tank.stats.gun, GunKind::RigCannons);
    assert!(s.players[b].tank.mass() > 100_000.0);
    let snap = s.snapshot();
    let bs = &snap.bosses[0];
    assert_eq!(bs.modules.len(), 9);
    assert!(bs.modules.iter().all(|m| m.alive) && !bs.core_exposed);

    // The core shrugs off hull hits while plates stand.
    let pos = s.tank_pose(boss).unwrap().0;
    let h0 = s.players[b].tank.health;
    s.damage(b, Some(me), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::World);
    let shielded = h0 - s.players[b].tank.health;
    assert!(shielded > 0.0 && shielded < 20.0, "shielded hit did {shielded}");

    // Knock off the plates: the core is exposed.
    let mut core_ev = false;
    for m in [PartKind::RigPlateLeft, PartKind::RigPlateRight, PartKind::RigPlateFront] {
        let b = idx(&s, boss);
        let hp = s.players[b].tank.parts.iter().find(|p| p.kind == m).unwrap().max_health;
        s.damage(b, Some(me), HitLocation::Part(m), hp + 1.0, pos, Vec3::X, Weapon::World);
        let ev = s.drain_events();
        core_ev |= ev.iter().any(|e| matches!(e, Event::BossCoreExposed { .. }));
    }
    assert!(core_ev);
    let b = idx(&s, boss);
    assert!(s.players[b].tank.alive, "plates gone but the boss lives");
    s.players[b].tank.health = 1000.0;
    let h1 = s.players[b].tank.health;
    s.damage(b, Some(me), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::World);
    assert!(h1 - s.players[b].tank.health > 50.0, "the exposed core takes real damage");

    // Engines: lose both and it crawls. Cannons: no main gun. Mast: blind.
    let v0 = s.players[b].tank.stats.speed_mult;
    for m in [PartKind::RigEngineLeft, PartKind::RigEngineRight, PartKind::RigCannons, PartKind::RigMast] {
        let b = idx(&s, boss);
        let hp = s.players[b].tank.parts.iter().find(|p| p.kind == m).unwrap().max_health;
        s.damage(b, Some(me), HitLocation::Part(m), hp + 1.0, pos, Vec3::X, Weapon::World);
    }
    let b = idx(&s, boss);
    assert!((s.players[b].tank.stats.speed_mult - v0 * 0.25).abs() < 1e-3, "two engines lost: crawl");
    assert_ne!(s.players[b].tank.stats.gun, GunKind::RigCannons);
    let snap = s.snapshot();
    assert_eq!(snap.bosses[0].modules.iter().filter(|m| !m.alive).count(), 7);
    assert!(snap.bosses[0].core_exposed);
    let ev = s.drain_events();
    assert!(ev.iter().filter(|e| matches!(e, Event::BossModuleLost { .. })).count() >= 4);

    // Kill it: defeat event, big drops, it leaves the match.
    s.players[b].tank.health = 10.0;
    s.damage(b, Some(me), HitLocation::Hull, 500.0, pos, Vec3::X, Weapon::World);
    let ev = s.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::BossDefeated { killer: Some(k), .. } if *k == me)));
    assert!(s.pickups.iter().any(|p| p.kind == PickupKind::AirStrike));
    s.step();
    assert!(s.players.iter().all(|p| p.id != boss), "dead raiders leave");
    assert_eq!(s.raid.as_ref().unwrap().bosses_killed, 1);
}

#[test]
fn no_friendly_fire_between_defenders() {
    let mut s = raid(4);
    let a = s.add_player("a");
    let b = s.add_player("b");
    run(&mut s, 2.5);
    let bi = idx(&s, b);
    let h0 = s.players[bi].tank.health;
    let pos = s.tank_pose(b).unwrap().0;
    s.damage(bi, Some(a), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::World);
    assert_eq!(s.players[bi].tank.health, h0);
}

#[test]
fn wave_clears_into_a_breather_with_repairs() {
    let mut s = raid(5);
    let me = s.add_player("me");
    s.raid.as_mut().unwrap().timer = 0.0;
    s.raid.as_mut().unwrap().health = 1000.0;
    run(&mut s, 0.1);
    assert_eq!(s.raid.as_ref().unwrap().phase, RaidPhase::Fighting);
    let mut ev = Vec::new();
    for _ in 0..(40 * 60) {
        s.step();
        // The defender is a god: everything that spawns dies.
        let raiders: Vec<usize> = (0..s.players.len()).filter(|&j| s.players[j].team == Some(1) && s.players[j].tank.alive).collect();
        for j in raiders {
            let pos = s.phys.body(s.players[j].tank.body).translation();
            s.damage(j, Some(me), HitLocation::Hull, 1.0e6, pos, Vec3::X, Weapon::World);
        }
        ev.extend(s.drain_events());
        if ev.iter().any(|e| matches!(e, Event::WaveCleared { .. })) {
            break;
        }
    }
    assert!(ev.iter().any(|e| matches!(e, Event::WaveCleared { wave: 1 })));
    let r = s.raid.as_ref().unwrap();
    assert_eq!(r.phase, RaidPhase::Breather);
    assert_eq!(r.waves_survived, 1);
    assert!(r.health > 1000.0, "repaired in the breather");
    assert!(s.pickups.iter().any(|p| p.site.is_none()), "raiders drop scrap");
}

#[test]
fn the_outpost_can_fall() {
    let mut s = raid(6);
    s.add_player("me");
    s.raid.as_mut().unwrap().health = 50.0;
    let id = s.spawn_raider_now(VehicleKind::Buggy).unwrap();
    let o = s.raid.as_ref().unwrap().outpost;
    s.teleport(id, o.x, o.z, 0.0);
    let ev = run(&mut s, 0.5);
    assert!(ev.iter().any(|e| matches!(e, Event::RaidLost { reason: LossReason::OutpostFell, .. })));
    assert!(matches!(s.phase, Phase::Defeated { .. }));
}

#[test]
fn everyone_down_at_once_loses() {
    let mut s = raid(7);
    let a = s.add_player("a");
    let b = s.add_player("b");
    s.raid.as_mut().unwrap().timer = 0.0;
    run(&mut s, 2.5);
    for id in [a, b] {
        let i = idx(&s, id);
        let pos = s.tank_pose(id).unwrap().0;
        s.damage(i, None, HitLocation::Hull, 1.0e6, pos, Vec3::X, Weapon::World);
    }
    let ev = run(&mut s, 0.2);
    assert!(ev.iter().any(|e| matches!(e, Event::RaidLost { reason: LossReason::AllDefendersDown, .. })));
}

#[test]
fn bots_have_styles_and_pick_branches_variously() {
    let mut s = Sim::new(8, Map::the_sink(8), Tuning::default());
    for k in 0..8 {
        s.add_bot(arena_core::bots::BOT_NAMES[k], 0.5);
    }
    let styles: std::collections::BTreeSet<String> =
        s.players.iter().map(|p| format!("{:?}", p.brain.as_ref().unwrap().style())).collect();
    assert!(styles.len() >= 3, "styles {styles:?}");
    for j in 0..8 {
        for _ in 0..6 {
            s.add_scrap(j, ScrapKind::Powder);
        }
    }
    let mut picks = std::collections::BTreeSet::new();
    for _ in 0..(4 * 60) {
        s.step();
        for e in s.drain_events() {
            if let Event::BranchChosen { part, .. } = e {
                picks.insert(format!("{part:?}"));
            }
        }
    }
    assert!(picks.len() >= 2, "picks {picks:?}");
    assert_eq!(Tuning::default().mode.kill_limit, 10);
}

#[test]
fn reset_match_restarts_a_lost_raid() {
    let mut s = raid(9);
    s.add_player("me");
    s.raid.as_mut().unwrap().health = 50.0;
    let id = s.spawn_raider_now(VehicleKind::Buggy).unwrap();
    let o = s.raid.as_ref().unwrap().outpost;
    s.teleport(id, o.x, o.z, 0.0);
    run(&mut s, 0.5);
    assert!(matches!(s.phase, Phase::Defeated { .. }));
    s.reset_match();
    assert_eq!(s.phase, Phase::Playing);
    let r = s.raid.as_ref().unwrap();
    assert_eq!((r.wave, r.waves_survived, r.lost), (0, 0, None));
    assert_eq!(r.health, r.max_health);
    assert!(s.players.iter().all(|p| p.team == Some(TEAM_DEFENDERS)));
    let ev = run(&mut s, Tuning::default().raid.first_wave_delay + 2.0);
    assert!(ev.iter().any(|e| matches!(e, Event::WaveStarted { wave: 1, .. })), "the raid runs again");
}

#[test]
fn team_brawl_splits_teams_and_counts_team_kills() {
    let mut s = Sim::new_team_brawl(10, Map::the_sink(10), Tuning::default());
    let me = s.add_player("me");
    for k in 0..7 {
        s.add_bot(arena_core::bots::BOT_NAMES[k], 0.5);
    }
    let t0 = s.players.iter().filter(|p| p.team == Some(0)).count();
    let t1 = s.players.iter().filter(|p| p.team == Some(1)).count();
    assert_eq!((t0, t1), (4, 4));
    // Rustbacks spawn west, Dust Saints east.
    for p in &s.players {
        let x = s.phys.body(p.tank.body).translation().x;
        assert_eq!(x < 0.0, p.team == Some(0), "{} spawned on the wrong side", p.name);
    }
    // No friendly fire.
    run(&mut s, 2.5);
    let mate = s.players.iter().find(|p| p.team == s.team_of(me) && p.id != me).unwrap().id;
    let mi = idx(&s, mate);
    let h0 = s.players[mi].tank.health;
    let pos = s.tank_pose(mate).unwrap().0;
    s.damage(mi, Some(me), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::World);
    assert_eq!(s.players[mi].tank.health, h0);
    let snap = s.snapshot();
    assert_eq!(snap.teams.len(), 2);
    // Play it out: someone reaches 20 team kills or the clock runs out.
    let mut ended = None;
    for _ in 0..(8 * 3600) {
        s.step();
        for e in s.drain_events() {
            if let Event::TeamMatchEnded { team } = e {
                ended = Some(team);
            }
        }
        if ended.is_some() {
            break;
        }
    }
    let scores = s.team_scores();
    println!("team brawl: {:?} -> {:?}", scores, ended);
    assert!(ended.is_some(), "the match ends");
    assert!(matches!(s.phase, Phase::TeamWon { .. }));
    let total: u32 = scores.iter().map(|t| t.kills as u32).sum();
    assert!(total > 0);
}

#[test]
fn an_idle_outpost_gets_raided_and_waves_move_on() {
    // Nobody shoots back: raiders must still reach (or shell) the outpost.
    let (mut s, _me) = Sim::practice_raid(12, 0, 0.5);
    // The idle "human" would die and lose the raid (AllDefendersDown); keep only the outpost at stake.
    for p in &mut s.players {
        p.counts_as_human = false;
    }
    let max = s.raid.as_ref().unwrap().max_health;
    let ev = run(&mut s, 240.0);
    let mut how = std::collections::BTreeMap::new();
    for e in &ev {
        if let Event::Destroyed { weapon, killer, .. } = e {
            *how.entry(format!("{weapon:?} by {}", if killer.is_some() { "someone" } else { "nobody" })).or_insert(0) += 1;
        }
    }
    println!("idle raid deaths: {how:?}");
    let r = s.raid.as_ref().unwrap();
    println!("idle raid after 4 min: wave {}, outpost {:.0}/{max:.0}, phase {:?}", r.wave, r.health, s.phase);
    assert!(r.health < max || r.lost.is_some(), "no raider ever hurt the outpost");
    // Shields stay rare: no raider spawns with a bubble.
    assert!(s.snapshot().tanks.iter().filter(|t| t.team == TEAM_RAIDERS).all(|t| t.shield == 0));
}

#[test]
fn debug_helpers_jump_and_spawn_the_boss() {
    let mut s = raid(13);
    s.add_player("me");
    s.debug_jump_to_wave(5);
    let ev = run(&mut s, 12.0);
    assert!(ev.iter().any(|e| matches!(e, Event::WaveStarted { wave: 5, boss: true, .. })));
    assert!(ev.iter().any(|e| matches!(e, Event::BossSpawned { .. })));
    let b = s.debug_spawn_boss().unwrap();
    assert_eq!(s.tank(b).unwrap().vehicle, VehicleKind::WarRig);
}

#[test]
fn allies_holding_the_line_keep_the_raid_alive() {
    let (mut s, me) = Sim::practice_raid(14, 3, 0.5);
    s.raid.as_mut().unwrap().timer = 0.0;
    run(&mut s, 3.0);
    let i = idx(&s, me);
    let pos = s.tank_pose(me).unwrap().0;
    s.damage(i, None, HitLocation::Hull, 1.0e6, pos, Vec3::X, Weapon::World);
    let ev = run(&mut s, 1.0);
    assert!(!ev.iter().any(|e| matches!(e, Event::RaidLost { .. })), "the human alone going down must not lose the raid");
    assert_eq!(s.phase, Phase::Playing);
}
