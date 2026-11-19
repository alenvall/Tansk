//! Native release benchmark of `Sim::step`: N bots play for T ticks on each of
//! several seeds, and the time of each phase (bots, tank driving, rapier,
//! projectiles, world and mode rules) plus the snapshot build and encode is
//! printed in µs per tick.
//!
//!     cargo run --release -p arena-core --example bench -- [brawl|team|raid|convoy|wellhead|lastcrew] [bots] [ticks] [--map sink|refinery] [--seeds N]
//!
//! Defaults: brawl, 8 bots, 7200 ticks (two minutes of play) after a
//! 600-tick warm-up, on 4 seeds. One thread, so the numbers are one room's
//! cost; matches that end restart, so it keeps measuring play.

use std::time::Instant;

use arena_core::sim::{Sim, StepPhase};
use arena_core::*;

const PHASES: [StepPhase; 5] = [StepPhase::Bots, StepPhase::Tanks, StepPhase::Physics, StepPhase::Projectiles, StepPhase::World];

#[derive(Default)]
struct Totals {
    ticks: u64,
    phase: [f64; 5],
    wall: f64,
    worst: f64,
    snap_build: f64,
    snap_encode: f64,
    snaps: u64,
    bytes: u64,
    bodies: u64,
    pairs: u64,
    vehicles: usize,
    runs: u32,
    sim_new: f64,
    nav_build: f64,
}

fn run(mode: &str, map: MapId, seed: u64, bots: usize, ticks: u32, tot: &mut Totals) {
    let m = Map::by_id(map, seed);
    let t = Tuning::default();
    let a = Instant::now();
    let nav = arena_core::bots::NavGrid::build(&m, t.bots.nav_cell);
    tot.nav_build += a.elapsed().as_secs_f64();
    drop(nav);
    let a = Instant::now();
    let mut sim = match mode {
        "brawl" => Sim::new(seed, m, t),
        "team" => Sim::new_team_brawl(seed, m, t),
        "raid" => Sim::new_raid(seed, m, t),
        "convoy" => Sim::new_convoy(seed, m, t),
        "wellhead" => Sim::new_wellhead(seed, m, t),
        "lastcrew" => Sim::new_last_crew(seed, m, t),
        other => panic!("unknown mode {other}"),
    };
    tot.sim_new += a.elapsed().as_secs_f64();
    tot.runs += 1;
    for k in 0..bots {
        sim.add_bot(bots::BOT_NAMES[k % bots::BOT_NAMES.len()], 0.3 + 0.1 * (k % 6) as f32);
    }
    if sim.raid.is_some() {
        for p in &mut sim.players {
            p.counts_as_human = true;
        }
    }
    for _ in 0..600 {
        sim.step();
        sim.drain_events();
    }
    let start = Instant::now();
    for tick in 0..ticks {
        let t0 = Instant::now();
        let mut last = t0;
        sim.step_timed(|ph| {
            let now = Instant::now();
            let k = PHASES.iter().position(|p| *p == ph).unwrap();
            tot.phase[k] += (now - last).as_secs_f64();
            last = now;
        });
        tot.worst = tot.worst.max(t0.elapsed().as_secs_f64());
        if tick % 2 == 0 {
            // The server sends a snapshot every other tick.
            let a = Instant::now();
            let s = sim.snapshot();
            let b = Instant::now();
            let enc = s.encode();
            let c = Instant::now();
            tot.snap_build += (b - a).as_secs_f64();
            tot.snap_encode += (c - b).as_secs_f64();
            tot.bytes += enc.len() as u64;
            tot.snaps += 1;
        }
        sim.drain_events();
        tot.vehicles = tot.vehicles.max(sim.players.len());
        if tick % 30 == 0 {
            let w = &sim.phys.world;
            tot.bodies += 30 * w.bodies.iter().filter(|(_, b)| b.is_dynamic() && b.is_enabled() && !b.is_sleeping()).count() as u64;
            tot.pairs += 30 * w.narrow_phase.contact_pairs().filter(|c| c.has_any_active_contact()).count() as u64;
        }
        if !matches!(sim.phase, Phase::Playing) {
            sim.reset_match();
        }
    }
    tot.wall += start.elapsed().as_secs_f64();
    tot.ticks += ticks as u64;
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut map = MapId::Sink;
    let mut seeds = 4u64;
    if let Some(i) = args.iter().position(|a| a == "--map") {
        map = args.get(i + 1).and_then(|s| MapId::from_key(s)).expect("--map sink|refinery");
        args.drain(i..i + 2);
    }
    if let Some(i) = args.iter().position(|a| a == "--seeds") {
        seeds = args.get(i + 1).and_then(|s| s.parse().ok()).expect("--seeds N");
        args.drain(i..i + 2);
    }
    let mode = if args.first().is_some_and(|a| a.parse::<u32>().is_err()) { args.remove(0) } else { "brawl".to_string() };
    let bots: usize = args.first().and_then(|s| s.parse().ok()).unwrap_or(8);
    let ticks: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(7200);

    let mut tot = Totals::default();
    for k in 0..seeds {
        run(&mode, map, 1234 + 7 * k, bots, ticks, &mut tot);
    }
    let n = tot.ticks as f64;
    let us = |s: f64| s * 1e6 / n;
    let total: f64 = tot.phase.iter().sum();
    println!("== bench: {mode} on {}, {bots} bots ({} vehicles), {ticks} ticks x {seeds} seeds ==", map.name(), tot.vehicles);
    for (k, p) in PHASES.iter().enumerate() {
        println!("  {:12} {:8.1} µs/tick  ({:4.1}%)", format!("{p:?}"), us(tot.phase[k]), 100.0 * tot.phase[k] / total);
    }
    println!("  {:12} {:8.1} µs/tick  (worst tick {:.0} µs)", "step", us(total), tot.worst * 1e6);
    println!("  awake dynamic bodies {:.1}, touching contact pairs {:.1} (mean)", tot.bodies as f64 / n, tot.pairs as f64 / n);
    println!(
        "  setup: Sim::new {:.1} ms, of which NavGrid::build {:.1} ms",
        tot.sim_new * 1e3 / tot.runs.max(1) as f64,
        tot.nav_build * 1e3 / tot.runs.max(1) as f64
    );
    println!(
        "  snapshot     {:8.1} µs build + {:.1} µs encode per snapshot, {} bytes",
        tot.snap_build * 1e6 / tot.snaps.max(1) as f64,
        tot.snap_encode * 1e6 / tot.snaps.max(1) as f64,
        tot.bytes / tot.snaps.max(1)
    );
}
