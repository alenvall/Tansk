//! Headless balance / fun harness: N seeded 8-bot Scrap Brawl matches.
//!
//!     cargo run --release -p arena-core --example harness -- [matches] [threads]
//!
//! Prints fun metrics and flags "not fun" signals. Native only (uses threads).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use arena_core::sim::Sim;
use arena_core::*;

#[derive(Default, Clone)]
struct MatchStats {
    seconds: f32,
    kills: u32,
    any_tier2: bool,
    any_tier3: bool,
    branch_picks: BTreeMap<String, u32>,
    /// (loadout, won)
    loadouts: Vec<(String, bool)>,
    distance: f32,
    parts_off: u32,
    combos: u32,
    rares: u32,
    strikes: u32,
    strike_kills: u32,
    cover_destroyed: u32,
    hazards: u32,
    launches: u32,
    topples: u32,
    upgrades: u32,
    top_kills: u32,
    throttling: u64,
    stuck: u64,
}

fn loadout(t: &arena_core::tank::Tank) -> String {
    format!("{:?}+{:?}", t.stats.gun, t.stats.alt)
}

fn run_match(seed: u64) -> MatchStats {
    let mut sim = Sim::new(seed, Map::the_sink(seed), Tuning::default());
    for k in 0..8 {
        sim.add_bot(arena_core::bots::BOT_NAMES[k], 0.3 + 0.1 * (k % 6) as f32);
    }
    let mut st = MatchStats::default();
    let mut last: Vec<Vec3> = sim.players.iter().map(|p| sim.phys.body(p.tank.body).translation()).collect();
    let max_ticks = (sim.tuning.mode.time_limit * 60.0) as u32 + 120;
    let mut winner = None;
    for tick in 0..max_ticks {
        sim.step();
        for e in sim.drain_events() {
            match e {
                Event::Destroyed { killer: Some(_), weapon, .. } => {
                    st.kills += 1;
                    if weapon == Weapon::AirStrike {
                        st.strike_kills += 1;
                    }
                }
                Event::UpgradeApplied { tier, .. } => {
                    st.upgrades += 1;
                    st.any_tier2 |= tier >= 2;
                    st.any_tier3 |= tier >= 3;
                }
                Event::BranchChosen { part, .. } => *st.branch_picks.entry(format!("{part:?}")).or_default() += 1,
                Event::PartBrokeOff { .. } => st.parts_off += 1,
                Event::PartAttached { part, .. } => {
                    if arena_core::parts::RARE_PARTS.contains(&part) {
                        st.rares += 1;
                    } else {
                        st.combos += 1;
                    }
                }
                Event::AirStrikeCalled { .. } => st.strikes += 1,
                Event::ObstacleDestroyed { .. } => st.cover_destroyed += 1,
                Event::HazardExploded { .. } => st.hazards += 1,
                Event::Launched { .. } => st.launches += 1,
                Event::ContainerToppled { .. } => st.topples += 1,
                Event::MatchEnded { winner: w } => winner = Some(w),
                _ => {}
            }
        }
        if tick % 6 == 0 {
            stuck_sample(&sim, &mut st.throttling, &mut st.stuck);
        }
        if tick % 30 == 0 {
            for (k, p) in sim.players.iter().enumerate() {
                let c = sim.phys.body(p.tank.body).translation();
                if p.tank.alive {
                    let d = c.distance(last[k]);
                    if d < 30.0 {
                        st.distance += d;
                    }
                }
                last[k] = c;
            }
        }
        if let Some(w) = winner {
            st.seconds = tick as f32 / 60.0;
            for p in &sim.players {
                st.loadouts.push((loadout(&p.tank), w == Some(p.id)));
            }
            break;
        }
    }
    st.top_kills = sim.players.iter().map(|p| p.kills).max().unwrap_or(0);
    st
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("raid") {
        let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);
        raid_main(n);
        return;
    }
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(200);
    let threads: usize = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map(|x| x.get()).unwrap_or(4));
    let next = Arc::new(AtomicUsize::new(0));
    let results = Arc::new(Mutex::new(Vec::new()));
    let start = std::time::Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let next = next.clone();
            let results = results.clone();
            std::thread::spawn(move || loop {
                let k = next.fetch_add(1, Ordering::SeqCst);
                if k >= n {
                    break;
                }
                let st = run_match(1000 + k as u64);
                results.lock().unwrap().push(st);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let r = results.lock().unwrap().clone();
    let m = r.len() as f32;
    let mean = |f: &dyn Fn(&MatchStats) -> f32| r.iter().map(f).sum::<f32>() / m;
    println!("== {} matches, 8 bots each, {:.0} s wall on {threads} threads ==", r.len(), start.elapsed().as_secs_f32());
    let len = mean(&|s| s.seconds);
    println!("match length: mean {:.0} s (min {:.0}, max {:.0}), {:.0}% ended on the kill limit",
        len,
        r.iter().map(|s| s.seconds).fold(f32::MAX, f32::min),
        r.iter().map(|s| s.seconds).fold(0.0, f32::max),
        100.0 * r.iter().filter(|s| s.top_kills >= Tuning::default().mode.kill_limit).count() as f32 / m);
    println!("top scorer kills: mean {:.1}", mean(&|s| s.top_kills as f32));
    println!("kills per minute: {:.1}  (kills per match {:.1})", mean(&|s| s.kills as f32 / (s.seconds / 60.0).max(0.1)), mean(&|s| s.kills as f32));
    println!("matches where anyone hit tier 2: {:.0}%, tier 3: {:.0}%", 100.0 * mean(&|s| s.any_tier2 as u8 as f32), 100.0 * mean(&|s| s.any_tier3 as u8 as f32));
    println!("upgrades per match {:.1}, parts shot off per minute {:.2}", mean(&|s| s.upgrades as f32), mean(&|s| s.parts_off as f32 / (s.seconds / 60.0).max(0.1)));
    let (th, stk): (u64, u64) = (r.iter().map(|s| s.throttling).sum(), r.iter().map(|s| s.stuck).sum());
    println!("stuck: {:.1}% of throttling time under 0.5 m/s", 100.0 * stk as f64 / th.max(1) as f64);
    println!("distance driven per tank per minute: {:.0} m", mean(&|s| s.distance / 8.0 / (s.seconds / 60.0).max(0.1)));
    println!("per match: combos {:.1}, rare parts {:.1}, air strikes {:.2} ({:.2} kills), cover destroyed {:.1}, hazard blasts {:.1}, launches {:.1}, containers toppled {:.1}",
        mean(&|s| s.combos as f32), mean(&|s| s.rares as f32), mean(&|s| s.strikes as f32), mean(&|s| s.strike_kills as f32),
        mean(&|s| s.cover_destroyed as f32), mean(&|s| s.hazards as f32), mean(&|s| s.launches as f32), mean(&|s| s.topples as f32));
    let mut picks: BTreeMap<String, u32> = BTreeMap::new();
    for s in &r {
        for (k, v) in &s.branch_picks {
            *picks.entry(k.clone()).or_default() += v;
        }
    }
    println!("branch picks: {picks:?}");
    let mut lo: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for s in &r {
        for (l, w) in &s.loadouts {
            let e = lo.entry(l.clone()).or_default();
            e.0 += 1;
            e.1 += *w as u32;
        }
    }
    let total_wins: u32 = lo.values().map(|v| v.1).sum();
    println!("final loadouts (appearances, wins, win rate, share of all wins):");
    let mut rows: Vec<_> = lo.iter().collect();
    rows.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
    for (l, (n, w)) in &rows {
        println!("  {l:28} {n:5} {w:4}  {:5.1}%  {:5.1}%", 100.0 * *w as f32 / *n as f32, 100.0 * *w as f32 / total_wins.max(1) as f32);
    }
    // Not-fun flags.
    let mut flags = Vec::new();
    for (l, (n, w)) in &rows {
        // A fair FFA win rate is 1 in 8 (12.5%). Over 40% with a real sample = dominant.
        if *n >= 15 && *w as f32 / *n as f32 > 0.4 {
            flags.push(format!("loadout {l} wins {:.0}% of the matches it ends in ({n} samples)", 100.0 * *w as f32 / *n as f32));
        }
    }
    let low = r.iter().filter(|s| s.kills < 5).count();
    if low > 0 {
        flags.push(format!("{low} matches with under 5 kills"));
    }
    let no_t2 = r.iter().filter(|s| !s.any_tier2).count();
    if no_t2 > 0 {
        flags.push(format!("{no_t2} matches where nobody reached tier 2"));
    }
    if flags.is_empty() {
        println!("NOT-FUN FLAGS: none");
    } else {
        for f in flags {
            println!("NOT-FUN FLAG: {f}");
        }
    }
}

// ---------------------------------------------------------------- Raider Waves

#[derive(Default, Clone)]
struct RaidRun {
    waves: u32,
    reason: String,
    bosses_spawned: u32,
    bosses_killed: u32,
    boss_lives: Vec<f32>,
    minutes: f32,
    kills: u32,
    defender_deaths: u32,
    modules_lost: u32,
    peak_tier: u8,
    throttling: u64,
    stuck: u64,
}

const RAID_WAVE_CAP: u32 = 15;

fn run_raid(seed: u64) -> RaidRun {
    let mut sim = Sim::new_raid(seed, Map::the_sink(seed), Tuning::default());
    for k in 0..4 {
        sim.add_bot(arena_core::bots::BOT_NAMES[k], 0.6);
    }
    // Bot stand-ins for the 4 humans.
    for p in &mut sim.players {
        p.counts_as_human = true;
    }
    let mut r = RaidRun::default();
    let cap = 45 * 60 * 60;
    for tick in 0..cap {
        sim.step();
        for e in sim.drain_events() {
            match e {
                Event::Destroyed { victim, .. } => {
                    if sim.team_of(victim) == Some(0) {
                        r.defender_deaths += 1;
                    } else {
                        r.kills += 1;
                    }
                }
                Event::BossModuleLost { .. } => r.modules_lost += 1,
                Event::RaidLost { reason, .. } => r.reason = format!("{reason:?}"),
                _ => {}
            }
        }
        if tick % 6 == 0 {
            stuck_sample(&sim, &mut r.throttling, &mut r.stuck);
        }
        for p in &sim.players {
            if p.team == Some(0) {
                r.peak_tier = r.peak_tier.max(*p.tank.tiers.iter().max().unwrap());
            }
        }
        let raid = sim.raid.as_ref().unwrap();
        if raid.lost.is_some() || raid.waves_survived >= RAID_WAVE_CAP || tick == cap - 1 {
            r.waves = raid.waves_survived;
            r.bosses_spawned = raid.bosses_spawned;
            r.bosses_killed = raid.bosses_killed;
            r.boss_lives = raid.boss_lifetimes.clone();
            r.minutes = tick as f32 / 3600.0;
            if raid.lost.is_none() {
                r.reason = if raid.waves_survived >= RAID_WAVE_CAP { "WaveCap".into() } else { "TimeCap".into() };
            }
            break;
        }
    }
    r
}

fn raid_main(n: usize) {
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(4);
    let next = Arc::new(AtomicUsize::new(0));
    let results = Arc::new(Mutex::new(Vec::new()));
    let start = std::time::Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let next = next.clone();
            let results = results.clone();
            std::thread::spawn(move || loop {
                let k = next.fetch_add(1, Ordering::SeqCst);
                if k >= n {
                    break;
                }
                let r = run_raid(5000 + k as u64);
                results.lock().unwrap().push(r);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let mut r = results.lock().unwrap().clone();
    r.sort_by_key(|x| x.waves);
    let m = r.len() as f32;
    println!("== Raider Waves: {} runs, 4 bot defenders (difficulty 0.6), cap {RAID_WAVE_CAP} waves, {:.0} s wall ==", r.len(), start.elapsed().as_secs_f32());
    let waves: Vec<u32> = r.iter().map(|x| x.waves).collect();
    println!(
        "waves survived: mean {:.1}, median {}, min {}, max {}",
        waves.iter().sum::<u32>() as f32 / m,
        waves[waves.len() / 2],
        waves[0],
        waves[waves.len() - 1]
    );
    let mut hist = BTreeMap::new();
    for w in &waves {
        *hist.entry(*w).or_insert(0) += 1;
    }
    println!("histogram (waves: runs): {hist:?}");
    let mut reasons = BTreeMap::new();
    for x in &r {
        *reasons.entry(x.reason.clone()).or_insert(0) += 1;
    }
    println!("how runs ended: {reasons:?}");
    let spawned: u32 = r.iter().map(|x| x.bosses_spawned).sum();
    let killed: u32 = r.iter().map(|x| x.bosses_killed).sum();
    let lives: Vec<f32> = r.iter().flat_map(|x| x.boss_lives.clone()).collect();
    let first_boss_runs = r.iter().filter(|x| x.bosses_spawned >= 1).count();
    let first_boss_kills = r.iter().filter(|x| x.bosses_killed >= 1).count();
    println!(
        "bosses: {killed}/{spawned} killed ({:.0}%); first boss beaten in {first_boss_kills}/{first_boss_runs} runs that met it",
        100.0 * killed as f32 / spawned.max(1) as f32
    );
    if !lives.is_empty() {
        let mean = lives.iter().sum::<f32>() / lives.len() as f32;
        let max = lives.iter().cloned().fold(0.0, f32::max);
        let min = lives.iter().cloned().fold(f32::MAX, f32::min);
        println!("boss lifetime: mean {mean:.0} s (min {min:.0}, max {max:.0}); modules shot off per boss {:.1}", r.iter().map(|x| x.modules_lost).sum::<u32>() as f32 / spawned.max(1) as f32);
    }
    let (th, stk): (u64, u64) = (r.iter().map(|s| s.throttling).sum(), r.iter().map(|s| s.stuck).sum());
    println!("stuck: {:.1}% of throttling time under 0.5 m/s (defenders and raiders)", 100.0 * stk as f64 / th.max(1) as f64);
    println!(
        "per run: {:.1} min, {:.0} raider kills, {:.1} defender deaths, peak defender tier {:.1}",
        r.iter().map(|x| x.minutes).sum::<f32>() / m,
        r.iter().map(|x| x.kills).sum::<u32>() as f32 / m,
        r.iter().map(|x| x.defender_deaths).sum::<u32>() as f32 / m,
        r.iter().map(|x| x.peak_tier as f32).sum::<f32>() / m
    );
}

/// Count bot samples that are throttling, and those that are stuck (under 0.5 m/s).
fn stuck_sample(sim: &Sim, throttling: &mut u64, stuck: &mut u64) {
    for p in &sim.players {
        if !p.tank.alive || p.brain.is_none() || p.input.throttle.abs() < 0.4 {
            continue;
        }
        *throttling += 1;
        if sim.phys.body(p.tank.body).linvel().length() < 0.5 {
            *stuck += 1;
        }
    }
}
