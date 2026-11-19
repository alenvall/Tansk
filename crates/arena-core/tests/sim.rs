use arena_core::physics::pose;
use arena_core::sim::Sim;
use arena_core::*;

fn solo(seed: u64) -> (Sim, PlayerId) {
    let mut sim = Sim::new(seed, Map::the_sink(seed), Tuning::default());
    let me = sim.add_player("test");
    (sim, me)
}

fn idx(sim: &Sim, id: PlayerId) -> usize {
    sim.players.iter().position(|p| p.id == id).unwrap()
}

/// Put a tank at (x, z) on the ground, facing `yaw`, at rest.
fn place(sim: &mut Sim, id: PlayerId, x: f32, z: f32, yaw: f32) {
    let y = sim.map.height_at(x, z) + 1.1;
    let i = idx(sim, id);
    let b = sim.players[i].tank.body;
    let body = sim.phys.body_mut(b);
    body.set_position(pose(Vec3::new(x, y, z), Quat::from_rotation_y(yaw)), true);
    body.set_linvel(Vec3::ZERO, true);
    body.set_angvel(Vec3::ZERO, true);
}

fn run(sim: &mut Sim, seconds: f32) {
    for _ in 0..(seconds * 60.0) as u32 {
        sim.step();
    }
}

/// An open patch of ground near the middle, clear of obstacles.
fn clear_run(sim: &Sim, yaw: f32, len: f32) -> (f32, f32) {
    let d = glam::Vec2::new(yaw.cos(), -yaw.sin());
    for k in 0..4000 {
        let x = -200.0 + (k as f32 * 37.7) % 400.0;
        let z = -200.0 + (k as f32 * 71.3) % 400.0;
        let p = glam::Vec2::new(x, z);
        if (0..=(len as i32 / 4)).all(|s| !sim.map.blocked_xz(p + d * (s as f32 * 4.0), 8.0)) {
            return (x, z);
        }
    }
    panic!("no clear run");
}

fn open_spot(sim: &Sim, near: Vec3) -> (f32, f32) {
    for r in 0..60 {
        for k in 0..16 {
            let a = k as f32 * 0.39;
            let x = near.x + a.cos() * r as f32 * 4.0;
            let z = near.z + a.sin() * r as f32 * 4.0;
            if !sim.map.blocked_xz(glam::Vec2::new(x, z), 22.0) {
                return (x, z);
            }
        }
    }
    panic!("no open spot");
}

#[test]
fn map_follows_the_level_rules() {
    let map = Map::the_sink(1);
    assert_eq!(map.spawns.len(), 8);
    assert!(map.scrap_sites.len() >= 30, "{} scrap sites", map.scrap_sites.len());
    assert!(map.obstacles.iter().any(|o| o.kind == ObstacleKind::ShipHull));
    assert!(map.obstacles.iter().any(|o| o.kind == ObstacleKind::ContainerStack));
    assert!(map.obstacles.iter().any(|o| o.kind == ObstacleKind::Highway));
    for o in &map.obstacles {
        let top = map.obstacle_top_above_ground(o);
        match o.cover {
            Cover::Low => assert!(top < 1.2, "{:?} low cover is {top} m tall", o.kind),
            Cover::Tall => {
                // Every box of tall cover rises at least 2.5 m above the ground at its centre.
                for (c, r, h) in o.world_boxes() {
                    let top_c = c.y + (r * Vec3::Y).y.abs() * h.y - map.height_at(c.x, c.z);
                    assert!(top_c > 2.5, "{:?} tall cover box is only {top_c} m", o.kind);
                }
            }
            Cover::Flat => assert!(top < 0.6, "highway deck sticks up {top} m"),
        }
    }
    // Same seed, same map.
    assert_eq!(map, Map::the_sink(1));
}

#[test]
fn collider_matches_height_at() {
    let (sim, _) = solo(3);
    let mut worst: f32 = 0.0;
    for k in 0..400 {
        let x = -250.37 + (k as f32 * 37.3) % 500.0;
        let z = -250.21 + (k as f32 * 53.9) % 500.0;
        let from = Vec3::new(x, 200.0, z);
        let hit = sim.phys.ray(from, -Vec3::Y, 400.0, arena_core::physics::static_query(), None);
        let (_, d, _) = hit.expect("ray hits the ground");
        let y = 200.0 - d;
        // Obstacles sit on top of the ground; only compare open ground.
        if !sim.map.blocked_xz(glam::Vec2::new(x, z), 1.0) {
            let near_road = sim.map.obstacles.iter().any(|o| o.cover == Cover::Flat && glam::Vec2::new(o.pos.x - x, o.pos.z - z).length() < 14.0);
            if !near_road {
                worst = worst.max((y - sim.map.height_at(x, z)).abs());
            }
        }
    }
    assert!(worst < 0.01, "collider and height_at differ by {worst} m");
}

#[test]
fn driving_moves_and_turns_a_tank() {
    let (mut sim, me) = solo(5);
    let (x, z) = clear_run(&sim, 0.3, 80.0);
    place(&mut sim, me, x, z, 0.3);
    run(&mut sim, 1.5);
    let (p0, _) = sim.tank_pose(me).unwrap();
    let y0 = sim.hull_yaw(me).unwrap();

    sim.set_input(me, Input { throttle: 1.0, aim_point: p0 + Vec3::X * 100.0, ..Default::default() });
    run(&mut sim, 5.0);
    let (p1, _) = sim.tank_pose(me).unwrap();
    let moved = (p1 - p0).length();
    let speed = sim.phys.body(sim.players[0].tank.body).linvel().length();
    println!("5 s at full throttle: {moved:.1} m, {:.1} km/h", speed * 3.6);
    assert!(moved > 30.0, "only moved {moved} m");
    assert!(speed * 3.6 > 35.0 && speed * 3.6 < 60.0, "top speed {} km/h", speed * 3.6);

    // Stop, then turn in place.
    sim.set_input(me, Input::default());
    run(&mut sim, 4.0);
    let (p2, _) = sim.tank_pose(me).unwrap();
    let ya = sim.hull_yaw(me).unwrap();
    sim.set_input(me, Input { steer: 1.0, ..Default::default() });
    run(&mut sim, 2.0);
    let (p3, _) = sim.tank_pose(me).unwrap();
    let yb = sim.hull_yaw(me).unwrap();
    let mut dy = yb - ya;
    while dy > std::f32::consts::PI {
        dy -= std::f32::consts::TAU;
    }
    while dy < -std::f32::consts::PI {
        dy += std::f32::consts::TAU;
    }
    println!("turn in place 2 s: {:.0} deg, drift {:.2} m", dy.to_degrees(), (p3 - p2).length());
    assert!(dy < -0.8, "steer right should turn clockwise (negative yaw), got {dy}");
    assert!((p3 - p2).length() < 3.0, "turning in place drifted {} m", (p3 - p2).length());
    let _ = y0;
}

#[test]
fn tank_never_falls_through_the_ground() {
    let (mut sim, me) = solo(9);
    sim.add_scrap(0, ScrapKind::Fuel);
    sim.add_scrap(0, ScrapKind::Fuel);
    sim.add_scrap(0, ScrapKind::Fuel);
    let mut lowest = f32::MAX;
    for tick in 0..3600u32 {
        let t = tick as f32 / 60.0;
        let steer = (t * 0.7).sin() * if (t as u32 / 9) % 2 == 0 { 1.0 } else { 0.4 };
        let throttle = if (t as u32 / 13) % 4 == 3 { -1.0 } else { 1.0 };
        sim.set_input(me, Input { throttle, steer, boost: (t as u32 / 5) % 3 == 0, ..Default::default() });
        sim.step();
        let (p, _) = sim.tank_pose(me).unwrap();
        let clearance = p.y - sim.map.height_at(p.x, p.z);
        lowest = lowest.min(clearance);
        assert!(clearance > 0.2, "tank sank to {clearance} m above ground at t={t}");
    }
    assert_eq!(sim.players[0].tank.rescues, 0, "the safety net had to catch the tank");
    println!("lowest hull centre above ground over 60 s: {lowest:.2} m");
}

#[test]
fn three_scrap_of_a_kind_upgrades_and_bolts_on() {
    let (mut sim, me) = solo(2);
    let i = idx(&sim, me);
    let m0 = sim.phys.body(sim.players[i].tank.body).mass();
    sim.add_scrap(i, ScrapKind::Powder);
    sim.add_scrap(i, ScrapKind::Powder);
    assert_eq!(sim.players[i].tank.tiers[ScrapKind::Powder.index()], 0);
    sim.add_scrap(i, ScrapKind::Powder);
    let tank = &sim.players[i].tank;
    assert_eq!(tank.tiers[ScrapKind::Powder.index()], 1);
    assert!(tank.parts.iter().any(|p| p.kind == PartKind::HeavyCannon));
    assert_eq!(tank.stats.gun, GunKind::HeavyCannon);
    let m1 = sim.phys.body(tank.body).mass();
    assert!(m1 > m0 + 500.0, "mass {m0} -> {m1}");
    let ev = sim.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::UpgradeApplied { tier: 1, track: ScrapKind::Powder, .. })));

    // Tier 2 branches: default applied at once, then the pick switches it.
    for _ in 0..3 {
        sim.add_scrap(i, ScrapKind::Powder);
    }
    assert_eq!(sim.players[i].tank.stats.gun, GunKind::BigBore);
    assert!(sim.players[i].tank.choice.is_some());
    sim.set_input(me, Input { choice: Some(2), ..Default::default() });
    sim.step();
    assert_eq!(sim.players[i].tank.stats.gun, GunKind::LongGun);
    assert!(sim.players[i].tank.choice.is_none());

    // Stacking tracks: Steel tier 2 keeps the flank plates and adds the branch.
    for _ in 0..6 {
        sim.add_scrap(i, ScrapKind::Steel);
    }
    let kinds: Vec<PartKind> = sim.players[i].tank.parts.iter().map(|p| p.kind).collect();
    assert!(kinds.contains(&PartKind::FlankPlates) && kinds.contains(&PartKind::Bulwark), "{kinds:?}");
    // Mass makes it slower: heavier than before.
    assert!(sim.phys.body(sim.players[i].tank.body).mass() > m1 + 8000.0);
}

#[test]
fn shooting_a_part_breaks_it_off_and_drops_scrap() {
    let mut sim = Sim::new(4, Map::the_sink(4), Tuning::default());
    let shooter = sim.add_player("shooter");
    let target = sim.add_player("target");
    let (x, z) = open_spot(&sim, Vec3::new(20.0, 0.0, -30.0));
    // Target broadside to the shooter, its heavy cannon pointing across the line of fire.
    place(&mut sim, shooter, x - 22.0, z, 0.0);
    place(&mut sim, target, x, z, std::f32::consts::FRAC_PI_2);
    let ti = idx(&sim, target);
    for _ in 0..3 {
        sim.add_scrap(ti, ScrapKind::Powder);
    }
    run(&mut sim, 2.5);
    sim.drain_events();
    let pickups_before = sim.pickups.iter().filter(|p| p.site.is_none()).count();
    let mut broke = false;
    for _ in 0..(30 * 60) {
        // Aim at the middle of the target's cannon barrel.
        let t = &sim.players[ti].tank;
        let hp = arena_core::tank::HullPose::of(&sim.phys, t.body);
        let (o, r) = t.frame(&hp, &sim.tuning, Frame::Gun);
        let aim = o + r * Vec3::new(1.9, 0.0, 0.0);
        sim.set_input(shooter, Input { aim_point: aim, fire: true, ..Default::default() });
        sim.set_input(target, Input { aim_point: hp.pos + hp.rot * Vec3::X * 50.0, ..Default::default() });
        sim.step();
        for e in sim.drain_events() {
            if let Event::PartBrokeOff { player, part, .. } = e {
                assert_eq!(player, target);
                assert_eq!(part, PartKind::HeavyCannon);
                broke = true;
            }
        }
        if broke {
            break;
        }
    }
    assert!(broke, "the cannon never broke off");
    let t = &sim.players[ti].tank;
    assert_eq!(t.tiers[ScrapKind::Powder.index()], 0);
    assert_eq!(t.stats.gun, GunKind::Stub);
    assert!(t.alive, "target should survive losing a part");
    let dropped: Vec<_> = sim.pickups.iter().filter(|p| p.site.is_none()).collect();
    assert!(dropped.len() >= pickups_before + 2, "dropped {} scrap", dropped.len() - pickups_before);
    assert!(dropped.iter().any(|p| p.kind == PickupKind::Scrap(ScrapKind::Powder)));
    assert!(sim.props.iter().any(|p| p.kind == PropKind::Debris(PartKind::HeavyCannon)));
}

#[test]
fn death_spills_scrap_and_respawn_keeps_tier_one() {
    let (mut sim, me) = solo(6);
    let killer = sim.add_player("killer");
    let i = idx(&sim, me);
    for _ in 0..6 {
        sim.add_scrap(i, ScrapKind::Powder);
    }
    for _ in 0..3 {
        sim.add_scrap(i, ScrapKind::Steel);
    }
    sim.add_scrap(i, ScrapKind::Volts);
    assert_eq!(sim.players[i].tank.tiers, [1, 2, 0, 0]);
    run(&mut sim, 2.5); // spawn protection off
    let before = sim.pickups.iter().filter(|p| p.site.is_none()).count();
    let pos = sim.tank_pose(me).unwrap().0;
    sim.drain_events();
    sim.damage(i, Some(killer), HitLocation::Hull, 10_000.0, pos, Vec3::X, Weapon::World);
    assert!(!sim.players[i].tank.alive);
    let ev = sim.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::Destroyed { victim, killer: Some(k), .. } if *victim == me && *k == killer)));
    let spilled = sim.pickups.iter().filter(|p| p.site.is_none()).count() - before;
    // 1 lost Powder tier * 2 + 1 held Volts + 1 bonus.
    assert!(spilled >= 4, "spilled {spilled}");
    let ki = idx(&sim, killer);
    assert_eq!(sim.players[ki].kills, 1);
    assert_eq!(sim.players[i].deaths, 1);

    run(&mut sim, 3.2);
    let t = &sim.players[i].tank;
    assert!(t.alive, "respawned after 3 s");
    assert_eq!(t.tiers, [1, 1, 0, 0]);
    assert_eq!(t.scrap, [0; 4]);
    assert_eq!(t.stats.gun, GunKind::HeavyCannon);
    assert_eq!(t.health, t.stats.max_health);
}

#[test]
fn blown_track_cripples_then_auto_repairs() {
    let (mut sim, me) = solo(8);
    run(&mut sim, 2.5);
    let pos = sim.tank_pose(me).unwrap().0;
    sim.damage(0, None, HitLocation::Track(0), 500.0, pos, Vec3::Z, Weapon::World);
    assert!(sim.players[0].tank.tracks[0].blown > 0.0);
    let mut saw_repair = false;
    for _ in 0..(5 * 60) {
        sim.step();
        if let Some(r) = sim.players[0].tank.repair {
            saw_repair |= r.kind == RepairKind::Track(0);
        }
    }
    assert!(saw_repair, "crew repair state exposed");
    assert_eq!(sim.players[0].tank.tracks[0].blown, 0.0);
}

#[test]
fn snapshot_round_trips_and_is_compact() {
    let mut sim = Sim::new(11, Map::the_sink(11), Tuning::default());
    for k in 0..8 {
        sim.add_bot(arena_core::bots::BOT_NAMES[k], 0.6);
    }
    // Some loadouts so parts are in the snapshot.
    for i in 0..8 {
        for _ in 0..(3 * (i % 3)) {
            sim.add_scrap(i, ScrapKind::from_index(i));
        }
    }
    run(&mut sim, 20.0);
    let snap = sim.snapshot();
    let bytes = snap.encode();
    let back = Snapshot::decode(&bytes).unwrap();
    assert_eq!(back, snap);
    let tank_only = Snapshot { projectiles: vec![], pickups: vec![], props: vec![], ..snap.clone() }.encode().len();
    println!(
        "snapshot: {} bytes total for 8 tanks ({} tanks+scores+mode, {} pickups, {} projectiles, {} props)",
        bytes.len(),
        tank_only,
        snap.pickups.len(),
        snap.projectiles.len(),
        snap.props.len()
    );
    assert!(tank_only < 1600, "tank part of snapshot is {tank_only} bytes");
    assert!(bytes.len() < 4000, "snapshot is {} bytes", bytes.len());
}

#[test]
fn cloned_sim_steps_identically() {
    let mut a = Sim::new(12, Map::the_sink(12), Tuning::default());
    for k in 0..4 {
        a.add_bot(arena_core::bots::BOT_NAMES[k], 0.5);
    }
    run(&mut a, 3.0);
    let mut b = a.clone();
    run(&mut a, 3.0);
    run(&mut b, 3.0);
    let sa = a.snapshot();
    let sb = b.snapshot();
    for (ta, tb) in sa.tanks.iter().zip(&sb.tanks) {
        let d = (Vec3::from(ta.pos) - Vec3::from(tb.pos)).length();
        assert!(d < 0.05, "clone diverged by {d} m");
    }
}

#[test]
fn tuning_loads_partial_ron() {
    let t = Tuning::from_ron("(tank: (max_speed: 15.0), mode: (kill_limit: 20))").unwrap();
    assert_eq!(t.tank.max_speed, 15.0);
    assert_eq!(t.mode.kill_limit, 20);
    assert_eq!(t.tank.base_mass, Tuning::default().tank.base_mass);
    let round = Tuning::from_ron(&Tuning::default().to_ron()).unwrap();
    assert_eq!(round, Tuning::default());
}

#[test]
fn bot_match_ten_minutes() {
    let minutes: f32 = 10.0;
    let mut tuning = Tuning::default();
    tuning.mode.time_limit = minutes * 60.0;
    tuning.mode.kill_limit = 0;
    let seed = 2026;
    let mut sim = Sim::new(seed, Map::the_sink(seed), tuning);
    for k in 0..8 {
        sim.add_bot(arena_core::bots::BOT_NAMES[k], 0.3 + 0.1 * k as f32);
    }
    let ticks = (minutes * 60.0 * 60.0) as u32 + 5;
    let mut worst = std::time::Duration::ZERO;
    let start = std::time::Instant::now();
    let mut per_track = [0u32; 4];
    let (mut shots, mut hits, mut breaks, mut upgrades, mut pickups) = (0, 0, 0, 0, 0);
    for _ in 0..ticks {
        let t0 = std::time::Instant::now();
        sim.step();
        worst = worst.max(t0.elapsed());
        for e in sim.drain_events() {
            match e {
                Event::Fired { .. } => shots += 1,
                Event::Hit { .. } => hits += 1,
                Event::PartBrokeOff { .. } => breaks += 1,
                Event::UpgradeApplied { track, .. } => {
                    upgrades += 1;
                    per_track[track.index()] += 1;
                }
                Event::PickedUp { .. } => pickups += 1,
                _ => {}
            }
        }
    }
    let total = start.elapsed();
    let mean_us = total.as_micros() as f64 / ticks as f64;
    println!("== bot match: {minutes} min, seed {seed}, 8 bots ==");
    println!("step time: mean {mean_us:.0} us, worst {} us ({} build)", worst.as_micros(), if cfg!(debug_assertions) { "debug" } else { "release" });
    println!("shots {shots}, hits {hits}, parts shot off {breaks}, upgrades {upgrades} (steel/powder/volts/fuel {per_track:?}), pickups {pickups}");
    let mut kills = 0;
    let mut deaths = 0;
    for p in &sim.players {
        println!(
            "  {:10} kills {:2} deaths {:2} tiers {:?} rescues {}",
            p.name, p.kills, p.deaths, p.tank.tiers, p.tank.rescues
        );
        kills += p.kills;
        deaths += p.deaths;
        let pos = sim.phys.body(p.tank.body).translation();
        assert!(pos.is_finite());
        assert!(pos.x.abs() < 300.0 && pos.z.abs() < 300.0, "{} left the arena: {pos}", p.name);
    }
    println!("phase {:?}", sim.phase);
    assert!(matches!(sim.phase, Phase::Ended { .. }));
    assert!(kills >= (minutes * 2.0) as u32, "only {kills} kills in {minutes} min");
    assert!(kills <= deaths, "more kills than deaths");
    assert!(upgrades > 0 && pickups > 0);
    if !cfg!(debug_assertions) {
        assert!(mean_us < 4000.0, "mean step {mean_us} us");
    }
}

#[test]
fn the_sink_has_its_places_and_they_connect() {
    let map = Map::the_sink(21);
    let names: Vec<&str> = map.regions.iter().map(|r| r.name.as_str()).collect();
    for n in ["The Outpost", "Highway 9", "The Gas Station", "The Yard", "The Beach", "The Plateau"] {
        assert!(names.contains(&n), "missing region {n}");
    }
    for k in [
        ObstacleKind::Wall,
        ObstacleKind::Tent,
        ObstacleKind::Tower,
        ObstacleKind::GasStation,
        ObstacleKind::Hedgehog,
        ObstacleKind::PowerPole,
        ObstacleKind::Crane,
        ObstacleKind::Mesa,
        ObstacleKind::LaunchPlate,
    ] {
        assert!(map.obstacles.iter().any(|o| o.kind == k), "no {k:?}");
    }
    println!("The Sink: {} obstacles, {} boxes", map.obstacles.len(), map.obstacles.iter().map(|o| o.boxes.len()).sum::<usize>());
    // Every region has pickups to visit.
    for r in &map.regions {
        let n = map.scrap_sites.iter().filter(|s| s.pos.distance(r.center) < r.radius).count();
        assert!(n >= 3, "{} has only {n} scrap sites", r.name);
    }
    // Spawns are clear, spread out, and all drive to the outpost (and the outpost zone is open).
    let nav = arena_core::bots::NavGrid::build(&map, 4.0);
    let o = glam::Vec2::new(map.outpost.x, map.outpost.z);
    assert!(!map.blocked_xz(o, 16.0), "the outpost courtyard must be open");
    for (i, s) in map.spawns.iter().enumerate() {
        let p = glam::Vec2::new(s.pos.x, s.pos.z);
        assert!(!map.blocked_xz(p, 5.0), "spawn {i} is blocked");
        assert!(nav.reachable(p, o), "spawn {i} can't drive to the outpost");
        for t in map.spawns.iter().skip(i + 1) {
            assert!(t.pos.distance(s.pos) > 90.0, "spawns too close");
        }
        // Cover within 30 m.
        assert!(map.obstacles.iter().any(|ob| ob.cover != Cover::Flat && glam::Vec2::new(ob.pos.x, ob.pos.z).distance(p) < 30.0));
    }
    // The raid's edges connect too, and the pickup sites are reachable.
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let p = glam::Vec2::new(a.cos(), a.sin()) * 255.0;
        if !map.blocked_xz(p, 4.0) {
            assert!(nav.reachable(p, o), "raid edge at {p} is cut off");
        }
    }
    for s in &map.scrap_sites {
        assert!(nav.reachable(glam::Vec2::new(s.pos.x, s.pos.z), o), "scrap site at {} unreachable", s.pos);
    }
}
