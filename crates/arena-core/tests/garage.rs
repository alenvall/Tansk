//! Garage chassis classes, the sandstorm, the horn.

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

fn lo(chassis: ChassisClass) -> Loadout {
    Loadout { chassis, paint: 3, emblem: 5, horn: 2 }
}

#[test]
fn chassis_classes_differ() {
    let mut s = sim(1);
    let scout = s.add_player_with("scout", lo(ChassisClass::Scout));
    let brawler = s.add_player("brawler");
    let hauler = s.add_player_with("hauler", lo(ChassisClass::Hauler));
    let st = |s: &Sim, id| s.tank(id).unwrap().stats.clone();
    assert_eq!(st(&s, scout).max_health, 330.0);
    assert_eq!(st(&s, brawler).max_health, 400.0);
    assert_eq!(st(&s, hauler).max_health, 430.0);
    assert!(st(&s, scout).mass < st(&s, brawler).mass && st(&s, brawler).mass < st(&s, hauler).mass);
    assert_eq!(s.tank(scout).unwrap().health, 330.0);

    // The Hauler's crew bolts on with two scrap; the others need three.
    let h = idx(&s, hauler);
    s.add_scrap(h, ScrapKind::Powder);
    s.add_scrap(h, ScrapKind::Powder);
    assert_eq!(s.players[h].tank.tiers[ScrapKind::Powder.index()], 1);
    let b = idx(&s, brawler);
    s.add_scrap(b, ScrapKind::Powder);
    s.add_scrap(b, ScrapKind::Powder);
    assert_eq!(s.players[b].tank.tiers[ScrapKind::Powder.index()], 0);

    // The Scout has no Front or Skirts slots: Steel tier 2 brings no branch part.
    let sc = idx(&s, scout);
    s.drain_events();
    for _ in 0..6 {
        s.add_scrap(sc, ScrapKind::Steel);
    }
    let ev = s.drain_events();
    assert!(!ev.iter().any(|e| matches!(e, Event::BranchOffered { track: ScrapKind::Steel, .. })));
    let kinds: Vec<PartKind> = s.players[sc].tank.parts.iter().map(|p| p.kind).collect();
    assert!(kinds.contains(&PartKind::FlankPlates));
    assert!(!kinds.contains(&PartKind::Bulwark) && !kinds.contains(&PartKind::SpacedArmour));

    // The Scout is the fastest on the flat.
    let speed = |class: ChassisClass| -> f32 {
        let mut s = sim(2);
        let id = s.add_player_with("x", lo(class));
        let map = s.map.clone();
        // A long clear run.
        let mut start = glam::Vec2::ZERO;
        'find: for k in 0..4000 {
            let p = glam::Vec2::new(-240.0 + (k as f32 * 37.7) % 480.0, -240.0 + (k as f32 * 71.3) % 480.0);
            if (0..25).all(|i| !map.blocked_xz(p + glam::Vec2::X * (i as f32 * 4.0), 8.0) && map.normal_at(p.x + i as f32 * 4.0, p.y).y > 0.97) {
                start = p;
                break 'find;
            }
        }
        s.teleport(id, start.x, start.y, 0.0);
        run(&mut s, 1.0);
        let mut top: f32 = 0.0;
        for _ in 0..(7 * 60) {
            s.set_input(id, Input { throttle: 1.0, ..Default::default() });
            s.step();
            top = top.max(s.phys.body(s.tank(id).unwrap().body).linvel().length());
        }
        top
    };
    let (vs, vb, vh) = (speed(ChassisClass::Scout), speed(ChassisClass::Brawler), speed(ChassisClass::Hauler));
    println!("top speed km/h: scout {:.0}, brawler {:.0}, hauler {:.0}", vs * 3.6, vb * 3.6, vh * 3.6);
    assert!(vs > vb && vb > vh);
}

#[test]
fn loadout_shows_in_snapshot_and_applies_on_respawn() {
    let mut s = sim(3);
    let me = s.add_player_with("me", lo(ChassisClass::Scout));
    let snap = s.snapshot();
    let t = snap.tank(me).unwrap();
    assert_eq!((t.chassis, t.paint, t.emblem), (ChassisClass::Scout, 3, 5));
    assert_eq!(s.players()[0].loadout.horn, 2);
    // Switch in the Garage: applies when the tank next respawns.
    s.set_loadout(me, lo(ChassisClass::Hauler));
    assert_eq!(s.tank(me).unwrap().class, ChassisClass::Scout);
    run(&mut s, 2.5);
    let i = idx(&s, me);
    let pos = s.tank_pose(me).unwrap().0;
    s.damage(i, None, HitLocation::Hull, 1.0e6, pos, Vec3::X, Weapon::World);
    run(&mut s, 3.5);
    assert_eq!(s.tank(me).unwrap().class, ChassisClass::Hauler);
    assert_eq!(s.tank(me).unwrap().stats.max_health, 430.0);
    // A loadout survives the wire (bincode varint, as the net protocol uses).
    let l = lo(ChassisClass::Hauler);
    assert_eq!(arena_core::net::decode::<Loadout>(&arena_core::net::encode(&l)), Some(l));
}

#[test]
fn bots_pick_chassis_by_style() {
    let mut s = sim(4);
    for k in 0..8 {
        s.add_bot(arena_core::bots::BOT_NAMES[k], 0.5);
    }
    let classes: std::collections::BTreeSet<String> = s.players.iter().map(|p| format!("{:?}", p.tank.class)).collect();
    assert!(classes.len() >= 2, "{classes:?}");
    for p in &s.players {
        assert_eq!(p.tank.class, p.brain.as_ref().unwrap().style().chassis());
    }
}

#[test]
fn a_sandstorm_rolls_in_and_out() {
    let mut s = sim(5);
    s.add_bot("a", 0.5);
    s.debug_storm();
    let ev = run(&mut s, 1.0);
    assert!(ev.iter().any(|e| matches!(e, Event::StormIncoming { .. })));
    assert_eq!(s.snapshot().storm, 0.0);
    let ev = run(&mut s, 20.0);
    assert!(ev.iter().any(|e| matches!(e, Event::StormStarted { .. })));
    assert!(s.snapshot().storm > 0.99, "full storm, got {}", s.snapshot().storm);
    let ev = run(&mut s, 60.0);
    assert!(ev.iter().any(|e| matches!(e, Event::StormEnded)));
    assert_eq!(s.snapshot().storm, 0.0);
}

#[test]
fn a_brawl_gets_about_one_storm() {
    let mut s = sim(6);
    for k in 0..2 {
        s.add_bot(arena_core::bots::BOT_NAMES[k], 0.5);
    }
    let ev = run(&mut s, 7.0 * 60.0);
    let n = ev.iter().filter(|e| matches!(e, Event::StormStarted { .. })).count();
    assert_eq!(n, 1);
}

#[test]
fn the_horn_honks() {
    let mut s = sim(7);
    let me = s.add_player_with("me", lo(ChassisClass::Brawler));
    s.set_input(me, Input { horn: true, ..Default::default() });
    let ev = run(&mut s, 0.5);
    let honks: Vec<_> = ev.iter().filter(|e| matches!(e, Event::Horn { .. })).collect();
    assert_eq!(honks.len(), 1, "one honk per press");
    assert!(matches!(honks[0], Event::Horn { player, horn: 2 } if *player == me));
}
