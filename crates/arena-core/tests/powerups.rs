//! Timed power-ups: spawning, pickup, the buffs, running out, the snapshot.

use arena_core::powerups::{Buff, PowerUpKind};
use arena_core::sim::Sim;
use arena_core::*;

fn sim(seed: u64) -> Sim {
    Sim::new(seed, Map::the_sink(seed), Tuning::default())
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

fn give(sim: &mut Sim, player: PlayerId, kind: PowerUpKind) {
    let t = sim.tuning.powerups.clone();
    let hp = if kind == PowerUpKind::Bubble { t.bubble_hp } else { 0.0 };
    sim.powerups.buffs.push(Buff { player, kind, left: kind.duration(&t), hp });
}

#[test]
fn power_ups_spawn_on_the_radio_after_a_while() {
    let mut s = sim(3);
    s.add_player("a");
    let first = s.tuning.powerups.first_spawn;
    let ev = run(&mut s, first - 1.0);
    assert!(!ev.iter().any(|e| matches!(e, Event::PowerUpSpawned { .. })), "none before {first} s");
    let ev = run(&mut s, 17.0);
    let spawned = ev.iter().filter(|e| matches!(e, Event::PowerUpSpawned { .. })).count();
    assert!(spawned >= 2, "spawned {spawned}");
    let live = s.pickups.iter().filter(|p| matches!(p.kind, PickupKind::PowerUp(_))).count();
    assert_eq!(live, spawned);
    assert!(s.snapshot().pickups.iter().any(|p| matches!(p.kind, PickupKind::PowerUp(_))));
}

#[test]
fn driving_over_one_gives_a_timed_buff_in_the_snapshot() {
    let mut s = sim(4);
    let a = s.add_player("a");
    let wait = s.tuning.powerups.first_spawn + 16.0;
    run(&mut s, wait);
    let pu = s.pickups.iter().find(|p| matches!(p.kind, PickupKind::PowerUp(_))).expect("a power-up").clone();
    let PickupKind::PowerUp(kind) = pu.kind else { unreachable!() };
    // Drop it on the tank.
    let i = idx(&s, a);
    let c = s.phys.body(s.players[i].tank.body).translation();
    s.pickups.iter_mut().find(|p| p.id == pu.id).unwrap().pos = c;
    let ev = run(&mut s, 0.5);
    assert!(ev.iter().any(|e| matches!(e, Event::PickedUp { player, kind: PickupKind::PowerUp(_), .. } if *player == a)));
    assert!(ev.iter().any(|e| matches!(e, Event::PowerUpStarted { player, kind: k, .. } if *player == a && *k == kind)));
    let snap = s.tank_snap(a).unwrap();
    assert_eq!(snap.powerups.len(), 1);
    assert_eq!(snap.powerups[0].kind, kind);
    assert!(snap.powerups[0].seconds_left > 5.0);
    // It goes over the wire.
    let msg = net::ServerMsg::Tick { ack: 0, snapshot: s.snapshot(), events: vec![] };
    assert_eq!(net::decode::<net::ServerMsg>(&net::encode(&msg)), Some(msg));
    // And runs out.
    let ev = run(&mut s, 16.0);
    assert!(ev.iter().any(|e| matches!(e, Event::PowerUpEnded { player, kind: k } if *player == a && *k == kind)));
    assert!(s.tank_snap(a).unwrap().powerups.is_empty());
}

#[test]
fn overcharge_doubles_damage_and_the_bubble_soaks_it() {
    let mut s = sim(5);
    let a = s.add_player("a");
    let b = s.add_player("b");
    run(&mut s, 3.0);
    let j = idx(&s, b);
    let pos = s.phys.body(s.players[j].tank.body).translation();
    let hit = |s: &mut Sim| {
        let before = s.players[j].tank.health;
        s.damage(j, Some(a), HitLocation::Hull, 10.0, pos, Vec3::X, Weapon::Gun(GunKind::Stub));
        before - s.players[j].tank.health
    };
    let plain = hit(&mut s);
    assert!(plain > 0.0);
    give(&mut s, a, PowerUpKind::Overcharge);
    let over = hit(&mut s);
    assert!((over / plain - 2.0).abs() < 0.05, "plain {plain} overcharged {over}");
    s.powerups.buffs.clear();
    give(&mut s, b, PowerUpKind::Bubble);
    assert_eq!(hit(&mut s), 0.0, "the bubble soaks it");
    let hp = s.tuning.powerups.bubble_hp;
    s.damage(j, Some(a), HitLocation::Hull, hp, pos, Vec3::X, Weapon::Gun(GunKind::Stub));
    let ev = run(&mut s, 0.1);
    assert!(ev.iter().any(|e| matches!(e, Event::PowerUpEnded { player, kind: PowerUpKind::Bubble } if *player == b)));
    assert!(hit(&mut s) > 0.0, "popped");
}

#[test]
fn gatling_fires_faster_and_nitro_leaves_the_stats_as_they_were() {
    let mut s = sim(6);
    let a = s.add_player("a");
    run(&mut s, 3.0);
    let i = idx(&s, a);
    let full = s.players[i].tank.stats.speed_mult;
    give(&mut s, a, PowerUpKind::Gatling);
    give(&mut s, a, PowerUpKind::Nitro);
    let c = s.phys.body(s.players[i].tank.body).translation();
    let mut reloads = Vec::new();
    for _ in 0..30 {
        s.set_input(a, Input { aim_point: c + Vec3::new(40.0, 0.0, 0.0), fire: true, throttle: 1.0, boost: true, ..Default::default() });
        s.step();
        s.drain_events();
        reloads.push(s.players[i].tank.reload);
    }
    let top = reloads.iter().cloned().fold(0.0, f32::max);
    let g = arena_core::tank::gun_stats(&s.tuning, &s.players[i].tank.stats);
    assert!(top > 0.0 && top < g.reload * 0.5, "reload {top} of {}", g.reload);
    assert_eq!(s.players[i].tank.stats.speed_mult, full);
    assert!(s.players[i].tank.fuel > s.players[i].tank.stats.fuel_max * 0.99, "boost never runs dry");
}

#[test]
fn the_magnet_pulls_loose_scrap_in() {
    let mut s = sim(7);
    let a = s.add_player("a");
    run(&mut s, 3.0);
    let i = idx(&s, a);
    let c = s.phys.body(s.players[i].tank.body).translation();
    let at = c + Vec3::new(20.0, 0.0, 0.0);
    let y = s.map.height_at(at.x, at.z) + 0.3;
    let pickup = |id| arena_core::sim::Pickup {
        id,
        kind: PickupKind::Scrap(ScrapKind::Steel),
        pos: Vec3::new(at.x, y, at.z),
        vel: Vec3::ZERO,
        age: 5.0,
        ttl: None,
        site: None,
        settled: true,
    };
    s.pickups.push(pickup(60000));
    run(&mut s, 3.0);
    assert!(s.pickups.iter().any(|p| p.id == 60000), "without a magnet it stays put");
    give(&mut s, a, PowerUpKind::Magnet);
    run(&mut s, 3.0);
    assert!(!s.pickups.iter().any(|p| p.id == 60000), "the magnet pulled it in");
}
