//! Combo parts, rare parts, the air strike, destruction and hazards.

use arena_core::events::HookTarget;
use arena_core::physics::pose;
use arena_core::sim::Sim;
use arena_core::snapshot::HazardKind;
use arena_core::*;
use glam::Vec2;

fn sim(seed: u64) -> Sim {
    Sim::new(seed, Map::the_sink(seed), Tuning::default())
}

fn idx(sim: &Sim, id: PlayerId) -> usize {
    sim.players.iter().position(|p| p.id == id).unwrap()
}

fn place(sim: &mut Sim, id: PlayerId, x: f32, z: f32, yaw: f32) {
    let y = sim.map.height_at(x, z) + 1.1;
    let i = idx(sim, id);
    let b = sim.players[i].tank.body;
    let body = sim.phys.body_mut(b);
    body.set_position(pose(Vec3::new(x, y, z), Quat::from_rotation_y(yaw)), true);
    body.set_linvel(Vec3::ZERO, true);
    body.set_angvel(Vec3::ZERO, true);
}

fn pos(sim: &Sim, id: PlayerId) -> Vec3 {
    sim.tank_pose(id).unwrap().0
}

fn run(sim: &mut Sim, seconds: f32) -> Vec<Event> {
    let mut ev = Vec::new();
    for _ in 0..(seconds * 60.0) as u32 {
        sim.step();
        ev.extend(sim.drain_events());
    }
    ev
}

/// A spot where two tanks `gap` apart along X both stand clear with an open line between them.
fn duel_spot(sim: &Sim, gap: f32) -> Vec2 {
    for k in 0..4000 {
        let c = Vec2::new(-240.0 + (k as f32 * 37.7) % 480.0, -240.0 + (k as f32 * 71.3) % 480.0);
        let steps = (gap / 4.0) as i32 + 6;
        let ok = (0..=steps).all(|i| {
            let p = c + Vec2::new(-gap / 2.0 - 12.0 + i as f32 * (gap + 24.0) / steps as f32, 0.0);
            !sim.map.blocked_xz(p, 9.0) && sim.map.normal_at(p.x, p.y).y > 0.97
        });
        if ok {
            return c;
        }
    }
    panic!("no duel spot");
}

/// Two tanks facing each other `gap` metres apart on open ground; both past spawn protection.
fn duel(seed: u64, gap: f32) -> (Sim, PlayerId, PlayerId) {
    let mut s = sim(seed);
    let a = s.add_player("a");
    let b = s.add_player("b");
    let c = duel_spot(&s, gap);
    place(&mut s, a, c.x - gap / 2.0, c.y, 0.0);
    place(&mut s, b, c.x + gap / 2.0, c.y, std::f32::consts::PI);
    run(&mut s, 2.2);
    (s, a, b)
}

fn give(sim: &mut Sim, id: PlayerId, kind: ScrapKind, n: usize) {
    let i = idx(sim, id);
    for _ in 0..n {
        sim.add_scrap(i, kind);
    }
}

fn has(sim: &Sim, id: PlayerId, kind: PartKind) -> bool {
    sim.tank(id).unwrap().parts.iter().any(|p| p.kind == kind)
}

#[test]
fn combos_bolt_on_when_two_tracks_reach_tier_two() {
    let mut s = sim(1);
    let a = s.add_player("a");
    give(&mut s, a, ScrapKind::Powder, 6);
    assert!(!has(&s, a, PartKind::Flamethrower));
    give(&mut s, a, ScrapKind::Fuel, 6);
    assert!(has(&s, a, PartKind::Flamethrower), "Powder + Fuel = flamethrower");
    assert_eq!(s.tank(a).unwrap().stats.alt, AltKind::Flamethrower);
    give(&mut s, a, ScrapKind::Volts, 6);
    assert!(has(&s, a, PartKind::RocketRack), "Powder + Volts = rocket rack");
    give(&mut s, a, ScrapKind::Steel, 6);
    assert!(has(&s, a, PartKind::ShieldGenerator), "Volts + Steel = shield");
    assert!(has(&s, a, PartKind::SpikedRam), "Steel + Fuel = spiked ram");
    let ev = s.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::PartAttached { part: PartKind::SpikedRam, .. })));
    // Combo parts are parts: they show in the snapshot with health.
    let snap = s.snapshot();
    let t = snap.tank(a).unwrap();
    assert!(t.parts.iter().any(|p| p.kind == PartKind::ShieldGenerator));
    assert!(t.shield > 0);
    // Shooting a combo off costs no tier.
    let tiers = s.tank(a).unwrap().tiers;
    run(&mut s, 2.2);
    let i = idx(&s, a);
    let p = pos(&s, a);
    s.players[i].tank.shield = 0.0;
    s.damage(i, None, HitLocation::Part(PartKind::SpikedRam), 300.0, p, Vec3::X, Weapon::World);
    assert!(!has(&s, a, PartKind::SpikedRam));
    assert_eq!(s.tank(a).unwrap().tiers, tiers);
}

#[test]
fn flamethrower_sets_tanks_alight() {
    let (mut s, a, b) = duel(2, 12.0);
    give(&mut s, a, ScrapKind::Powder, 6);
    give(&mut s, a, ScrapKind::Fuel, 6);
    let h0 = s.tank(b).unwrap().health;
    let target = pos(&s, b);
    let mut burnt = false;
    for _ in 0..120 {
        s.set_input(a, Input { aim_point: target, alt_fire: true, ..Default::default() });
        s.step();
        burnt |= s.tank(b).unwrap().burning > 0.0;
        assert!(s.snapshot().tank(a).unwrap().flaming || s.tick < 5);
    }
    assert!(burnt, "target never caught fire");
    let ev = run(&mut s, 2.0);
    assert!(ev.iter().any(|e| matches!(e, Event::Hit { weapon: Weapon::Burn, .. })), "burn damage ticks after the flame stops");
    assert!(s.tank(b).unwrap().health < h0 - 60.0, "flame + burn did {}", h0 - s.tank(b).unwrap().health);
}

#[test]
fn shield_soaks_then_recharges() {
    let (mut s, a, _) = duel(3, 40.0);
    give(&mut s, a, ScrapKind::Volts, 6);
    give(&mut s, a, ScrapKind::Steel, 6);
    let i = idx(&s, a);
    let max = s.players[i].tank.stats.shield_max;
    assert!(max > 0.0 && s.players[i].tank.shield == max);
    let h0 = s.players[i].tank.health;
    let p = pos(&s, a);
    s.damage(i, None, HitLocation::Hull, 100.0, p, Vec3::X, Weapon::World);
    assert_eq!(s.players[i].tank.health, h0, "the bubble took it all");
    s.damage(i, None, HitLocation::Hull, max, p, Vec3::X, Weapon::World);
    assert_eq!(s.players[i].tank.shield, 0.0);
    assert!(s.players[i].tank.health < h0);
    let ev = run(&mut s, 10.0);
    assert!(ev.iter().any(|e| matches!(e, Event::ShieldBroken { .. }) || matches!(e, Event::ShieldRestored { .. })));
    assert!(s.players[i].tank.shield > max * 0.9, "shield recharged to {}", s.players[i].tank.shield);
}

#[test]
fn rockets_lock_on_and_home() {
    let (mut s, a, b) = duel(4, 70.0);
    give(&mut s, a, ScrapKind::Powder, 6);
    give(&mut s, a, ScrapKind::Volts, 6);
    assert_eq!(s.tank(a).unwrap().stats.alt, AltKind::Rockets);
    let mut locked = false;
    let mut launched = 0;
    let mut hits = 0;
    for k in 0..(8 * 60) {
        let target = pos(&s, b);
        // b drives sideways so dumb rockets would miss.
        s.set_input(b, Input { throttle: 0.6, steer: 0.3, aim_point: target + Vec3::X * 50.0, ..Default::default() });
        let alt = locked && k % 30 == 0;
        s.set_input(a, Input { aim_point: target, alt_fire: alt, ..Default::default() });
        s.step();
        for e in s.drain_events() {
            match e {
                Event::LockedOn { target, .. } if target == b => locked = true,
                Event::Fired { weapon: Weapon::Alt(AltKind::Rockets), .. } => launched += 1,
                Event::Hit { target, weapon: Weapon::Alt(AltKind::Rockets), .. } if target == b => hits += 1,
                _ => {}
            }
        }
    }
    assert!(locked, "never locked on");
    assert!(launched >= 4, "launched {launched}");
    assert!(hits >= 2, "only {hits} rocket hits on a locked target");
}

#[test]
fn spiked_ram_hurts_more_the_faster_you_go() {
    let ram_damage = |spiked: bool| -> f32 {
        let (mut s, a, b) = duel(5, 40.0);
        if spiked {
            give(&mut s, a, ScrapKind::Steel, 6);
            give(&mut s, a, ScrapKind::Fuel, 6);
            assert!(has(&s, a, PartKind::SpikedRam));
        }
        let h0 = s.tank(b).unwrap().health + s.tank(b).unwrap().shield;
        for _ in 0..(12 * 60) {
            let tb = pos(&s, b);
            s.set_input(a, Input { throttle: 1.0, aim_point: tb, ..Default::default() });
            s.step();
            for e in s.drain_events() {
                if let Event::Ram { speed, .. } = e {
                    println!("ram at {speed:.1} m/s (spiked {spiked})");
                }
            }
        }
        println!("a at {:?}, b at {:?}", pos(&s, a), pos(&s, b));
        h0 - s.tank(b).unwrap().health
    };
    let plain = ram_damage(false);
    let spiked = ram_damage(true);
    println!("ram damage: plain {plain:.0}, spiked {spiked:.0}");
    assert!(spiked > plain * 1.5 && spiked > 20.0, "spiked {spiked} vs plain {plain}");
}

#[test]
fn rare_crate_gives_a_part_and_death_drops_it() {
    let mut s = sim(6);
    let a = s.add_player("a");
    let killer = s.add_player("k");
    let i = idx(&s, a);
    run(&mut s, 2.2);
    s.give_part(i, PartKind::Mortar);
    assert!(has(&s, a, PartKind::Mortar));
    assert_eq!(s.tank(a).unwrap().rare_ammo, Tuning::default().special.mortar_ammo);
    // A new rare part replaces the old one.
    s.give_part(i, PartKind::MineLayer);
    assert!(!has(&s, a, PartKind::Mortar) && has(&s, a, PartKind::MineLayer));
    let p = pos(&s, a);
    s.damage(i, Some(killer), HitLocation::Hull, 10_000.0, p, Vec3::X, Weapon::World);
    assert!(!has(&s, a, PartKind::MineLayer));
    assert!(s.pickups.iter().any(|q| q.kind == PickupKind::Rare(PartKind::MineLayer)), "death drops the golden crate");
}

#[test]
fn golden_crate_pickup_attaches_the_part() {
    let mut s = sim(7);
    let a = s.add_player("a");
    run(&mut s, 2.2);
    let p = pos(&s, a);
    s.spawn_pickup(PickupKind::Rare(PartKind::RepairRig), p);
    let ev = run(&mut s, 1.0);
    assert!(ev.iter().any(|e| matches!(e, Event::PickedUp { kind: PickupKind::Rare(PartKind::RepairRig), .. })));
    assert!(has(&s, a, PartKind::RepairRig));
    // The rig heals over time when left alone.
    let i = idx(&s, a);
    s.players[i].tank.health = 100.0;
    run(&mut s, 6.0);
    assert!(s.players[i].tank.health > 120.0, "rig healed to {}", s.players[i].tank.health);
}

#[test]
fn mortar_lobs_over_cover() {
    let (mut s, a, b) = duel(8, 120.0);
    let i = idx(&s, a);
    s.give_part(i, PartKind::Mortar);
    let h0 = s.tank(b).unwrap().health;
    let mut fired = 0;
    for k in 0..(16 * 60) {
        let t = pos(&s, b);
        s.set_input(a, Input { aim_point: t, special: k % 240 == 0, ..Default::default() });
        s.step();
        for e in s.drain_events() {
            if matches!(e, Event::Fired { weapon: Weapon::Special(PartKind::Mortar), .. }) {
                fired += 1;
            }
        }
    }
    assert!(fired >= 3, "fired {fired}");
    assert!(s.tank(b).unwrap().health < h0 || !s.tank(b).unwrap().alive, "the mortar never landed near the target");
}

#[test]
fn harpoon_reels_in_a_tank() {
    let (mut s, a, b) = duel(9, 50.0);
    let i = idx(&s, a);
    s.give_part(i, PartKind::Harpoon);
    let d0 = pos(&s, a).distance(pos(&s, b));
    let t = pos(&s, b);
    s.set_input(a, Input { aim_point: t, ..Default::default() });
    run(&mut s, 2.0); // turn the turret onto it
    s.set_input(a, Input { aim_point: t, special: true, ..Default::default() });
    let mut ev = run(&mut s, 0.5);
    s.set_input(a, Input { aim_point: t, ..Default::default() });
    ev.extend(run(&mut s, 2.5));
    assert!(ev.iter().any(|e| matches!(e, Event::HarpoonHooked { target: HookTarget::Tank(x), .. } if *x == b)), "harpoon missed");
    let d1 = pos(&s, a).distance(pos(&s, b));
    println!("harpoon: {d0:.1} m -> {d1:.1} m");
    assert!(d1 < d0 - 8.0, "reeled from {d0} to {d1}");
}

#[test]
fn smoke_blocks_sight() {
    let (mut s, a, b) = duel(10, 40.0);
    let i = idx(&s, a);
    s.give_part(i, PartKind::SmokeLauncher);
    let (pa, pb) = (pos(&s, a) + Vec3::Y * 1.5, pos(&s, b) + Vec3::Y * 1.5);
    assert!(s.sight_clear(pa, pb));
    s.set_input(a, Input { aim_point: pb, ..Default::default() });
    run(&mut s, 3.0); // turret onto the target first
    s.set_input(a, Input { aim_point: pb, special: true, ..Default::default() });
    let ev = run(&mut s, 0.2);
    assert!(ev.iter().any(|e| matches!(e, Event::SmokeDeployed { .. })));
    assert!(!s.sight_clear(pa, pb), "smoke between them should block sight");
    assert_eq!(s.snapshot().smokes.len(), 1);
    run(&mut s, 20.0);
    assert!(s.smokes.is_empty(), "smoke clears");
}

#[test]
fn mines_blow_up_enemies() {
    let (mut s, a, b) = duel(11, 30.0);
    let i = idx(&s, a);
    s.give_part(i, PartKind::MineLayer);
    s.set_input(a, Input { special: true, ..Default::default() });
    let ev = run(&mut s, 0.2);
    assert!(ev.iter().any(|e| matches!(e, Event::MineLaid { .. })));
    let mine = s.mines[0].pos;
    // The owner is immune; the enemy drives over it.
    run(&mut s, 2.0);
    assert_eq!(s.mines.len(), 1);
    place(&mut s, b, mine.x + 1.0, mine.z, 0.0);
    let h0 = s.tank(b).unwrap().health;
    let ev = run(&mut s, 0.5);
    assert!(ev.iter().any(|e| matches!(e, Event::MineTriggered { victim: Some(v), .. } if *v == b)));
    assert!(s.tank(b).unwrap().health < h0 - 50.0 || !s.tank(b).unwrap().alive);
}

#[test]
fn air_strike_is_announced_then_carpets_the_line() {
    let (mut s, a, b) = duel(12, 120.0);
    let i = idx(&s, a);
    s.players[i].tank.air_strike = true;
    let target = pos(&s, b);
    s.set_input(a, Input { aim_point: target, special: true, ..Default::default() });
    let ev = run(&mut s, 0.1);
    let called = ev.iter().find_map(|e| match e {
        Event::AirStrikeCalled { delay, .. } => Some(*delay),
        _ => None,
    });
    assert_eq!(called, Some(Tuning::default().special.strike_delay));
    assert!(!s.tank(a).unwrap().air_strike);
    assert_eq!(s.snapshot().strikes.len(), 1);
    // Nothing for ~4 s.
    let ev = run(&mut s, 3.5);
    assert!(!ev.iter().any(|e| matches!(e, Event::Explosion { weapon: Weapon::AirStrike, .. })));
    let ev = run(&mut s, 3.0);
    assert!(ev.iter().any(|e| matches!(e, Event::BombingRun { .. })));
    let bombs = ev.iter().filter(|e| matches!(e, Event::Explosion { weapon: Weapon::AirStrike, .. })).count();
    assert_eq!(bombs, Tuning::default().special.strike_bombs as usize);
    assert!(ev.iter().any(|e| matches!(e, Event::Destroyed { victim, weapon: Weapon::AirStrike, .. } if *victim == b)), "a tank sitting on the flare dies");
    assert!(s.strikes.is_empty());
}

#[test]
fn low_cover_is_destructible_and_leaves_no_collider() {
    let mut s = sim(13);
    let map = s.map.clone();
    let oi = map.obstacles.iter().position(|o| o.kind == ObstacleKind::Barrier).expect("a barrier");
    let o = &map.obstacles[oi];
    let (c, _, _) = o.world_boxes().next().unwrap();
    let from = c + Vec3::Y * 20.0;
    assert!(s.phys.ray(from, -Vec3::Y, 30.0, arena_core::physics::static_query(), None).map(|h| h.1).unwrap() < 19.9);
    let mut ev = Vec::new();
    for _ in 0..10 {
        s.blast(c + Vec3::Y * 0.5, 120.0, 5.0, 0.0);
        ev.extend(s.drain_events());
    }
    assert!(ev.iter().any(|e| matches!(e, Event::ObstacleDestroyed { obstacle, .. } if *obstacle as usize == oi)));
    assert!(s.snapshot().removed.iter().any(|r| r.obstacle as usize == oi && r.box_index == 255));
    let hit = s.phys.ray(from, -Vec3::Y, 30.0, arena_core::physics::static_query(), None).unwrap().1;
    assert!(hit > 19.5, "the barrier collider should be gone (ray stopped at {hit})");
    assert!(s.props.iter().any(|p| p.kind == PropKind::Rubble(ObstacleKind::Barrier)));
    // reset_match puts it back.
    s.reset_match();
    assert!(s.removed.is_empty());
    s.step();
    assert!(s.phys.ray(from, -Vec3::Y, 30.0, arena_core::physics::static_query(), None).unwrap().1 < 19.9);
}

#[test]
fn big_blasts_topple_containers() {
    let mut s = sim(14);
    let map = s.map.clone();
    let oi = map.obstacles.iter().position(|o| o.kind == ObstacleKind::ContainerStack).unwrap();
    let (c, r, h) = map.obstacles[oi].world_boxes().next().unwrap();
    let side = r * Vec3::Z * (h.z + 2.0);
    let ev = {
        s.blast(c + side, 100.0, 12.0, 400_000.0);
        s.drain_events()
    };
    assert!(ev.iter().any(|e| matches!(e, Event::ContainerToppled { obstacle, .. } if *obstacle as usize == oi)));
    let loose = s.props.iter().filter(|p| p.kind == PropKind::Container).count();
    assert!(loose >= 1);
    run(&mut s, 3.0);
    assert!(s.snapshot().removed.iter().any(|x| x.obstacle as usize == oi));
}

#[test]
fn fuel_hazards_explode_and_chain() {
    let mut s = sim(15);
    let map = s.map.clone();
    let hz: Vec<usize> = s.hazards.iter().filter(|h| matches!(h.kind, HazardKind::FuelSilo | HazardKind::FuelTank)).map(|h| h.obstacle).collect();
    assert!(hz.len() >= 3, "{} fuel hazards", hz.len());
    let oi = hz[0];
    let c = map.obstacles[oi].pos + Vec3::Y * 2.0;
    let mut ev = Vec::new();
    for _ in 0..6 {
        s.blast(c, 80.0, 8.0, 0.0);
        ev.extend(s.drain_events());
    }
    assert!(ev.iter().any(|e| matches!(e, Event::HazardExploded { .. })));
    assert!(ev.iter().any(|e| matches!(e, Event::ObstacleDestroyed { obstacle, .. } if *obstacle as usize == oi)));
    let snap = s.snapshot();
    assert!(snap.hazards.iter().any(|h| h.obstacle as usize == oi && !h.ready));
    // Oil drums chain too: shoot one and it goes up.
    let drum = s.props.iter().position(|p| p.kind == PropKind::OilDrum).unwrap();
    let dp = s.phys.body(s.props[drum].body).translation();
    s.blast(dp + Vec3::X * 1.0, 120.0, 4.0, 0.0);
    let ev = s.drain_events();
    assert!(ev.iter().filter(|e| matches!(e, Event::HazardExploded { .. })).count() >= 1);
}

#[test]
fn launch_plate_throws_a_fast_tank() {
    let mut s = sim(16);
    let a = s.add_player("a");
    let map = s.map.clone();
    let plate = map.obstacles.iter().find(|o| o.kind == ObstacleKind::LaunchPlate).expect("a launch plate");
    let fwd = plate.rot * Vec3::X;
    let start = plate.pos - fwd * 40.0;
    let yaw = (-fwd.z).atan2(fwd.x);
    place(&mut s, a, start.x, start.z, yaw);
    run(&mut s, 2.2);
    let mut launched = false;
    for _ in 0..(8 * 60) {
        let (p, r) = s.tank_pose(a).unwrap();
        let to = plate.pos - p;
        let hull_fwd = r * Vec3::X;
        let err = (-to.z).atan2(to.x) - (-hull_fwd.z).atan2(hull_fwd.x);
        let err = (err + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        s.set_input(a, Input { throttle: 1.0, steer: (-err * 2.0).clamp(-1.0, 1.0), ..Default::default() });
        s.step();
        if s.drain_events().iter().any(|e| matches!(e, Event::Launched { .. })) {
            launched = true;
            break;
        }
    }
    assert!(launched, "never launched");
    let y0 = pos(&s, a).y;
    run(&mut s, 0.5);
    assert!(pos(&s, a).y > y0 + 1.5, "airborne after the plate");
    assert!(s.snapshot().hazards.iter().any(|h| h.kind == HazardKind::LaunchPlate && !h.ready));
}

#[test]
fn wrecking_ball_swings() {
    let mut s = sim(17);
    assert!(!s.map.cranes.is_empty(), "the map has a crane");
    let ball = s.props.iter().find(|p| p.kind == PropKind::WreckingBall).unwrap().body;
    let mut max_speed: f32 = 0.0;
    for _ in 0..(20 * 60) {
        s.step();
        max_speed = max_speed.max(s.phys.body(ball).linvel().length());
    }
    let p = s.phys.body(ball).translation();
    let c = s.map.cranes[0];
    println!("wrecking ball: max speed {max_speed:.1} m/s, {:.1} m from anchor (rope {:.1})", p.distance(c.anchor), c.rope);
    assert!(max_speed > 4.0);
    assert!((p.distance(c.anchor) - c.rope).abs() < 0.5);
    assert!(p.y > s.map.height_at(p.x, p.z) + 1.0, "ball hangs above the ground");
}
