use super::*;

fn game(settings: Settings, n: usize) -> (Game3d, Vec<u32>) {
    let mut g = Game3d::new(settings, 7);
    let ids: Vec<u32> = (0..n).map(|i| g.add_player(&format!("p{i}"), i as u8)).collect();
    g.start();
    for _ in 0..200 {
        g.step();
    }
    (g, ids)
}

fn tank_of(g: &Game3d, pid: u32) -> &Tank {
    g.tanks.iter().find(|t| t.player == pid && !t.dead).expect("tank")
}

fn tank_of_mut(g: &mut Game3d, pid: u32) -> &mut Tank {
    g.tanks.iter_mut().find(|t| t.player == pid && !t.dead).expect("tank")
}

#[test]
fn spawn_after_countdown() {
    let mut g = Game3d::new(Settings::default(), 1);
    let a = g.add_player("a", 0);
    let b = g.add_player("b", 1);
    assert_eq!(g.view().phase, Phase::Lobby);
    g.start();
    g.step();
    assert_eq!(g.view().phase, Phase::Countdown);
    for _ in 0..200 {
        g.step();
    }
    let v = g.view();
    assert_eq!(v.phase, Phase::Playing);
    assert_eq!(v.tanks.len(), 2);
    assert_eq!(v.round, 1);
    for t in &v.tanks {
        assert!((t.pos[1] - terrain::height(t.pos[0], t.pos[2])).abs() < 1.0);
        assert!(t.shield.is_some() || g.tick > 100);
    }
    assert!(g.player(a).unwrap().tank.is_some() && g.player(b).unwrap().tank.is_some());
}

#[test]
fn driving_stays_on_ground_and_in_arena() {
    let (mut g, ids) = game(Settings::default(), 2);
    let a = ids[0];
    let start = tank_of(&g, a).pos();
    g.set_input(a, Input3d { forward: true, ..Input3d::default() });
    for _ in 0..30 {
        g.step();
    }
    let p = tank_of(&g, a).pos();
    assert!((p[0] - start[0]).hypot(p[2] - start[2]) > 20.0, "tank should move: {start:?} -> {p:?}");
    for i in 0..(20_000 / TICK_MS) {
        // Turn now and then so it explores.
        let right = (i / 60) % 3 == 0;
        g.set_input(a, Input3d { forward: true, right, ..Input3d::default() });
        g.step();
        let v = g.view();
        for t in &v.tanks {
            assert!(t.pos[0] > 0.0 && t.pos[0] < ARENA_W && t.pos[2] > 0.0 && t.pos[2] < ARENA_D, "{:?}", t.pos);
            assert!((t.pos[1] - terrain::height(t.pos[0], t.pos[2])).abs() < 1.0);
            assert!(t.hull_pitch.abs() < 60.0 && t.hull_roll.abs() < 60.0);
        }
    }
}

#[test]
fn shell_arcs_and_lands() {
    let (mut g, ids) = game(Settings::default(), 2);
    let a = ids[0];
    let t = tank_of(&g, a);
    let yaw = (384.0 - t.z).atan2(512.0 - t.x).to_degrees();
    {
        let t = tank_of_mut(&mut g, a);
        t.turret_yaw = yaw;
        t.barrel_pitch = 15.0;
    }
    g.set_input(a, Input3d { fire: true, aim_yaw: yaw, aim_pitch: 15.0, ..Input3d::default() });
    g.step();
    g.set_input(a, Input3d { aim_yaw: yaw, aim_pitch: 15.0, ..Input3d::default() });
    let p0 = g.projectiles.iter().find(|p| p.kind == ProjKind::Default).expect("shell").pos;
    let mut max_y = p0[1];
    let mut impact = false;
    g.view();
    for _ in 0..200 {
        g.step();
        if let Some(p) = g.projectiles.first() {
            max_y = max_y.max(p.pos[1]);
        }
        if g.view().events.iter().any(|e| matches!(e, Event3d::Impact(_))) {
            impact = true;
            break;
        }
    }
    assert!(max_y > p0[1] + 20.0, "should arc up: {p0:?} max {max_y}");
    assert!(impact, "should land");
    assert!(g.projectiles.is_empty());
}

#[test]
fn kill_and_standard_scoring() {
    let s = Settings { lives: 1, ..Settings::default() };
    let (mut g, ids) = game(s, 2);
    let (a, b) = (ids[0], ids[1]);
    // b on the zone plateau, a 120 px west of it.
    let (bx, bz) = (512.0, 384.0);
    {
        let t = tank_of_mut(&mut g, b);
        t.x = bx;
        t.z = bz;
    }
    {
        let t = tank_of_mut(&mut g, a);
        t.x = bx - 120.0;
        t.z = bz;
        t.turret_yaw = 0.0;
    }
    let mut died = false;
    for _ in 0..3000 {
        if let Some(bt) = g.tanks.iter_mut().find(|t| t.player == b && !t.dead) {
            bt.shield = None;
        }
        let (Some(at), Some(bt)) =
            (g.tanks.iter().find(|t| t.player == a && !t.dead), g.tanks.iter().find(|t| t.player == b && !t.dead))
        else {
            died = true;
            break;
        };
        let m = at.pivot();
        let target = add(bt.pos(), [0.0, 20.0, 0.0]);
        let d = sub(target, m);
        let yaw = d[2].atan2(d[0]).to_degrees();
        let pitch = d[1].atan2(d[0].hypot(d[2])).to_degrees() + 1.0;
        g.set_input(a, Input3d { fire: true, aim_yaw: yaw, aim_pitch: pitch, ..Input3d::default() });
        g.step();
        if g.player(b).unwrap().eliminated {
            died = true;
            break;
        }
    }
    assert!(died, "b should die");
    g.step();
    let pa = g.player(a).unwrap();
    let pb = g.player(b).unwrap();
    assert!(pb.eliminated || pb.lives <= 0);
    assert_eq!(pa.score, 1, "survivor scores");
    assert_eq!(pb.score, 0);
}

#[test]
fn koth_scores_in_zone() {
    let s = Settings { koth: true, ..Settings::default() };
    let (mut g, ids) = game(s, 2);
    let a = ids[0];
    assert!(g.view().zone.is_some());
    {
        let t = tank_of_mut(&mut g, a);
        t.x = 512.0;
        t.z = 384.0 + 20.0;
    }
    for _ in 0..100 {
        g.step();
    }
    assert!(g.player(a).unwrap().score >= 2);
    assert_eq!(g.player(ids[1]).unwrap().score, 0);
}

#[test]
fn every_weapon_fires() {
    for w in [
        Weapon::Default,
        Weapon::Bounce,
        Weapon::Flamethrower,
        Weapon::Shockwave,
        Weapon::Shotgun,
        Weapon::Slowspeedy,
    ] {
        let (mut g, ids) = game(Settings::default(), 2);
        let a = ids[0];
        g.give_weapon(a, w);
        let t = tank_of(&g, a);
        let yaw = (384.0 - t.z).atan2(512.0 - t.x).to_degrees();
        g.set_input(a, Input3d { fire: true, aim_yaw: yaw, aim_pitch: 5.0, ..Input3d::default() });
        let mut seen = 0;
        let mut secondaries = 0;
        let mut exploded = false;
        let mut sound = false;
        for _ in 0..120 {
            g.step();
            seen = seen.max(g.projectiles.len());
            secondaries = secondaries.max(g.projectiles.iter().filter(|p| p.kind == ProjKind::ShockwaveSecondary).count());
            let ev = g.view().events;
            sound |= ev.iter().any(|e| matches!(e, Event3d::Sound(..)));
            exploded |= ev.iter().any(|e| matches!(e, Event3d::Explosion(..)));
        }
        assert!(seen > 0, "{w:?} fired nothing");
        assert!(sound || w == Weapon::Shockwave, "{w:?} made no sound");
        if w == Weapon::Shockwave {
            assert!(exploded && secondaries >= 10, "shockwave should detonate ({secondaries})");
        }
    }
}

#[test]
fn pickups_spawn_and_are_collected() {
    let (mut g, ids) = game(Settings::default(), 2);
    for _ in 0..(16_000 / TICK_MS) {
        g.step();
    }
    assert!(g.view().pickups.len() >= 2);
    let a = ids[0];
    let t = tank_of(&g, a);
    let (x, z) = (t.x, t.z);
    g.add_pickup(PickupKind::Weapon(Weapon::Shotgun), x, z);
    g.step();
    assert_eq!(tank_of(&g, a).weapon, Weapon::Shotgun);
}

#[test]
fn random_play_does_not_panic() {
    let mut rng = Rng::new(42);
    for koth in [false, true] {
        let s = Settings { koth, game_time_ms: 1_800_000, round_time_ms: 120_000, ..Settings::default() };
        let mut g = Game3d::new(s, 3);
        let ids: Vec<u32> = (0..4).map(|i| g.add_player("p", i)).collect();
        g.start();
        let mut inputs = vec![Input3d::default(); 4];
        for tick in 0..10_000 {
            for (i, &id) in ids.iter().enumerate() {
                if rng.below(15) == 0 {
                    inputs[i] = Input3d {
                        forward: rng.below(3) > 0,
                        back: rng.below(5) == 0,
                        left: rng.below(3) == 0,
                        right: rng.below(3) == 0,
                        fire: rng.below(2) == 0,
                        aim_yaw: rng.f64() as f32 * 720.0 - 360.0,
                        aim_pitch: rng.f64() as f32 * 60.0 - 15.0,
                    };
                }
                g.set_input(id, inputs[i]);
                if rng.below(300) == 0 {
                    let w = [Weapon::Bounce, Weapon::Flamethrower, Weapon::Shockwave, Weapon::Shotgun, Weapon::Slowspeedy]
                        [rng.below(5) as usize];
                    g.give_weapon(id, w);
                }
            }
            g.step();
            let v = g.view();
            for p in &v.projectiles {
                assert!(p.pos.iter().all(|c| c.is_finite()), "tick {tick}");
            }
        }
    }
}
