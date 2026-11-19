//! THE LAND-SHIP: Raider Waves' second boss.

use arena_core::parts::{SHIP_MODULES, SHIP_PLATES};
use arena_core::raid::{BossKind, TEAM_RAIDERS};
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

fn open(sim: &Sim, near: glam::Vec2, margin: f32) -> glam::Vec2 {
    for r in 0..80 {
        for k in 0..24 {
            let a = k as f32 * 0.2618;
            let p = near + glam::Vec2::new(a.cos(), a.sin()) * r as f32 * 4.0;
            if !sim.map.blocked_xz(p, margin) && sim.map.normal_at(p.x, p.y).y > 0.97 {
                return p;
            }
        }
    }
    panic!("no open spot")
}

fn knock_off(s: &mut Sim, boss: PlayerId, me: PlayerId, m: PartKind) {
    let b = idx(s, boss);
    let pos = s.tank_pose(boss).unwrap().0;
    let hp = s.players[b].tank.parts.iter().find(|p| p.kind == m).unwrap().max_health;
    s.damage(b, Some(me), HitLocation::Part(m), hp + 1.0, pos, Vec3::X, Weapon::World);
}

#[test]
fn boss_waves_alternate_rig_and_land_ship() {
    let mut s = raid(21);
    assert_eq!(s.boss_for_wave(5), BossKind::WarRig);
    assert_eq!(s.boss_for_wave(10), BossKind::LandShip);
    assert_eq!(s.boss_for_wave(15), BossKind::WarRig);
    assert_eq!(s.boss_for_wave(20), BossKind::LandShip);
    s.add_player("me");
    s.debug_jump_to_wave(10);
    let ev = run(&mut s, 12.0);
    assert!(ev.iter().any(|e| matches!(e, Event::WaveStarted { wave: 10, boss: true, .. })));
    assert!(ev.iter().any(|e| matches!(e, Event::BossSpawned { kind: BossKind::LandShip, .. })), "wave 10 brings the Land-Ship");
    assert!(!ev.iter().any(|e| matches!(e, Event::BossSpawned { kind: BossKind::WarRig, .. })));
    let r = s.raid.as_ref().unwrap();
    assert_eq!(r.boss_spawn_log, vec![BossKind::LandShip]);
    // Turning alternation off brings the Rig every time.
    let mut t = Tuning::default();
    t.raid.boss_alternate = false;
    let s2 = Sim::new_raid(21, Map::the_sink(21), t);
    assert_eq!(s2.boss_for_wave(10), BossKind::WarRig);
}

#[test]
fn land_ship_modules_shoot_off_and_change_it() {
    let mut s = raid(22);
    let me = s.add_player("me");
    let boss = s.debug_spawn_landship().unwrap();
    let ev = s.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::BossSpawned { kind: BossKind::LandShip, .. })));
    let b = idx(&s, boss);
    let tank = &s.players[b].tank;
    assert_eq!(tank.vehicle, VehicleKind::LandShip);
    assert_eq!(tank.parts.len(), SHIP_MODULES.len());
    assert!(tank.mass() > 300_000.0, "an enormous crawler: {} kg", tank.mass());
    assert_eq!(s.team_of(boss), Some(TEAM_RAIDERS));
    // About 30 x 9 x 8 m: its boxes span that.
    let boxes = tank.hit_boxes();
    let (mut lo, mut hi) = (Vec3::splat(1e9), Vec3::splat(-1e9));
    for (_, _, hb) in &boxes {
        lo = lo.min(hb.center - hb.half);
        hi = hi.max(hb.center + hb.half);
    }
    let size = hi - lo;
    assert!(size.x > 28.0 && size.x < 34.0 && size.z > 8.0 && size.z < 12.0 && size.y > 7.0, "ship size {size}");

    let snap = s.snapshot();
    let bs = snap.bosses.iter().find(|x| x.player == boss).unwrap();
    assert_eq!(bs.kind, BossKind::LandShip);
    assert_eq!(bs.modules.len(), 13);
    assert_eq!(bs.turrets.len(), 2, "two independent deck guns");
    assert!(bs.modules.iter().all(|m| m.alive) && !bs.core_exposed);
    assert_eq!(Snapshot::decode(&snap.encode()).unwrap(), snap);

    // The hull core shrugs off hits under the plates.
    let pos = s.tank_pose(boss).unwrap().0;
    let h0 = s.players[b].tank.health;
    s.damage(b, Some(me), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::World);
    let shielded = h0 - s.players[b].tank.health;
    assert!(shielded > 0.0 && shielded < 20.0, "shielded hit did {shielded}");
    // A stray track hit on the ship counts as the hull (its tracks are modules).
    let h1 = s.players[b].tank.health;
    s.damage(b, Some(me), HitLocation::Track(0), 100.0, pos, Vec3::X, Weapon::World);
    assert!(s.players[b].tank.tracks[0].blown <= 0.0);
    assert!(s.players[b].tank.health < h1);

    // Track units: one lost slows it; a whole side gone and it crawls.
    let v0 = s.players[b].tank.stats.speed_mult;
    knock_off(&mut s, boss, me, PartKind::ShipTrackFL);
    let b = idx(&s, boss);
    let v1 = s.players[b].tank.stats.speed_mult;
    assert!(v1 < v0 * 0.95, "one track unit lost: slower ({v0} -> {v1})");
    knock_off(&mut s, boss, me, PartKind::ShipTrackRL);
    let b = idx(&s, boss);
    let t = Tuning::default();
    assert!((s.players[b].tank.stats.speed_mult - v0 * t.raid.ship_side_lost_speed).abs() < 1e-4, "a side gone: crawl");
    // ... and slews toward the dead (left) side.
    s.players[b].brain = None;
    s.set_input(boss, Input { throttle: 1.0, ..Default::default() });
    s.step();
    let b = idx(&s, boss);
    assert!(s.players[b].input.steer < -0.2, "slews left: steer {}", s.players[b].input.steer);

    // Plates off: the core is exposed.
    let mut core = false;
    for m in SHIP_PLATES {
        knock_off(&mut s, boss, me, m);
        core |= s.drain_events().iter().any(|e| matches!(e, Event::BossCoreExposed { .. }));
    }
    assert!(core);
    let b = idx(&s, boss);
    s.players[b].tank.health = 1000.0;
    s.damage(b, Some(me), HitLocation::Hull, 100.0, pos, Vec3::X, Weapon::World);
    assert!(1000.0 - s.players[b].tank.health > 50.0, "the exposed core takes real damage");

    // The rest: guns, mortar, nests, bridge, hangar.
    for m in [PartKind::ShipGunFore, PartKind::ShipGunAft, PartKind::ShipMortar, PartKind::ShipMgLeft, PartKind::ShipBridge, PartKind::ShipHangar] {
        knock_off(&mut s, boss, me, m);
    }
    let ev = s.drain_events();
    assert!(ev.iter().filter(|e| matches!(e, Event::BossModuleLost { .. })).count() >= 6);
    assert!(ev.iter().filter(|e| matches!(e, Event::PartBrokeOff { .. })).count() >= 6, "modules clatter off as debris");
    let snap = s.snapshot();
    let bs = snap.bosses.iter().find(|x| x.player == boss).unwrap();
    assert!(bs.core_exposed);
    assert_eq!(bs.modules.iter().filter(|m| !m.alive).count(), 10);

    // Sink it: defeat event, big drops (two rare parts and a flare), it leaves the match.
    let b = idx(&s, boss);
    s.players[b].tank.health = 10.0;
    s.damage(b, Some(me), HitLocation::Hull, 500.0, pos, Vec3::X, Weapon::World);
    let ev = s.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::BossDefeated { killer: Some(k), .. } if *k == me)));
    assert!(s.pickups.iter().any(|p| p.kind == PickupKind::AirStrike));
    assert!(s.pickups.iter().filter(|p| matches!(p.kind, PickupKind::Rare(_))).count() >= 2);
    s.step();
    assert!(s.players.iter().all(|p| p.id != boss));
    let r = s.raid.as_ref().unwrap();
    assert_eq!(r.boss_defeat_log.len(), 1);
    assert_eq!(r.boss_defeat_log[0].0, BossKind::LandShip);
    assert!(r.ships.is_empty());
}

#[test]
fn land_ship_guns_mortar_nests_and_hangar_all_work() {
    let mut s = raid(23);
    let me = s.add_player("me");
    let boss = s.debug_spawn_landship().unwrap();
    // Out in the open, well away from the outpost, a defender 70 m off its bow.
    let p = open(&s, glam::Vec2::new(-150.0, 30.0), 25.0);
    s.teleport(boss, p.x, p.y, 0.0);
    let b = idx(&s, boss);
    s.players[b].brain = None;
    let q = open(&s, p + glam::Vec2::new(70.0, 0.0), 6.0);
    s.teleport(me, q.x, q.y, std::f32::consts::PI);
    let ev = run(&mut s, 3.0);
    let _ = ev;
    let ev = run(&mut s, 34.0);
    let fired = |w: Weapon| ev.iter().filter(|e| matches!(e, Event::Fired { player, weapon, .. } if *player == boss && *weapon == w)).count();
    assert!(fired(Weapon::Gun(GunKind::DeckGun)) >= 3, "deck guns fired {}", fired(Weapon::Gun(GunKind::DeckGun)));
    assert!(fired(Weapon::Special(PartKind::ShipMortar)) >= 3, "mortar salvos");
    assert!(ev.iter().any(|e| matches!(e, Event::ShipLaunchedBuggy { ship, .. } if *ship == boss)), "the hangar drops buggies");
    assert!(s.players.iter().any(|p| p.team == Some(TEAM_RAIDERS) && p.tank.vehicle == VehicleKind::Buggy));
    let snap = s.snapshot();
    let bs = snap.bosses.iter().find(|x| x.player == boss).unwrap();
    // The fore gun swung toward the target, the aft one can't (the bridge is in the way).
    assert!(bs.turrets[0].yaw.abs() < 1.0, "fore gun yaw {}", bs.turrets[0].yaw);

    // Broadside nests hose a crew alongside.
    let mut s = raid(24);
    let me = s.add_player("me");
    let boss = s.debug_spawn_landship().unwrap();
    let p = open(&s, glam::Vec2::new(-150.0, 30.0), 25.0);
    s.teleport(boss, p.x, p.y, 0.0);
    let b = idx(&s, boss);
    s.players[b].brain = None;
    let q = open(&s, p + glam::Vec2::new(0.0, 30.0), 5.0);
    s.teleport(me, q.x, q.y, 0.0);
    let ev = run(&mut s, 6.0);
    assert!(ev.iter().any(|e| matches!(e, Event::Tracer { player, .. } if *player == boss)), "an MG nest opened up");
}

#[test]
fn land_ship_wave_plays_out_with_bot_defenders() {
    // Four bot defenders against the wave-10 Land-Ship: it fights, loses modules,
    // and the run ends one way or another within a few minutes.
    let mut s = raid(25);
    for k in 0..4 {
        s.add_bot(arena_core::bots::BOT_NAMES[k], 0.6);
    }
    for p in &mut s.players {
        p.counts_as_human = true;
    }
    s.debug_jump_to_wave(10);
    let mut spawned = false;
    let mut modules = 0;
    let mut over = false;
    for _ in 0..(6 * 60 * 60) {
        s.step();
        for e in s.drain_events() {
            match e {
                Event::BossSpawned { kind: BossKind::LandShip, .. } => spawned = true,
                Event::BossModuleLost { .. } => modules += 1,
                Event::BossDefeated { .. } | Event::RaidLost { .. } => over = true,
                _ => {}
            }
        }
        if over {
            break;
        }
    }
    println!("land-ship wave: modules lost {modules}, over {over}, phase {:?}", s.phase);
    assert!(spawned);
    assert!(modules >= 1, "defenders shot something off");
    assert!(s.players.iter().all(|p| p.tank.rescues == 0 || p.tank.vehicle != VehicleKind::LandShip), "the ship never fell through the floor");
}
