//! Renders every sound to `target/audio-preview/*.wav` and prints what each
//! measures, so the bank can be checked (and listened to) without a browser.
//!
//! cargo run -p arena-audio --example preview --release
//!
//! Each one-shot's file holds all of its pre-rendered takes back to back,
//! 0.4 s apart, so the variation can be heard. The engine files run a
//! programme: idle, rev up and drive off, flat out, boost, lift, coast, idle.
//! `ambience.wav` runs calm wind, a blustery spell, a storm and back.
//! `music_title.wav` is the title theme once round and into its loop;
//! `music_battle_sweep.wav` takes the battle from calm to full and back;
//! `stinger_*.wav` are the stingers.

use std::path::PathBuf;
use std::time::Instant;

use arena_audio::analysis::{analyse, db, Report};
use arena_audio::dsp::SR;
use arena_audio::spatial::{spatialize, Listener};
use arena_audio::wav;
use arena_audio::music;
use arena_audio::{render, Ambience, Music, MusicTrack, Stinger, schedule, take_seed, EngineInput, EngineKind, EngineMixer, SoundId, TankEngine, SAMPLE_RATE};

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/audio-preview");
    std::fs::create_dir_all(&dir).expect("create target/audio-preview");
    dir
}

fn row(name: &str, r: &Report) {
    println!(
        "{name:<20} {:>5.2}s  peak {:>6.1} dB  rms {:>6.1} dB  dc {:>+8.5}  centroid {:>6.0} Hz  dom {:>5.0} Hz  low/mid/high {:>3.0}/{:>3.0}/{:>3.0}%  >8k {:>4.1}%  ring {:.2}s",
        r.seconds,
        db(r.peak),
        db(r.rms),
        r.dc,
        r.centroid_hz,
        r.dominant_hz,
        r.bands[0] * 100.0,
        r.bands[1] * 100.0,
        r.bands[2] * 100.0,
        r.air * 100.0,
        r.ring_out,
    );
}

/// Pearson correlation of two takes over their common length.
fn likeness(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        ab += f64::from(a[i]) * f64::from(b[i]);
        aa += f64::from(a[i]) * f64::from(a[i]);
        bb += f64::from(b[i]) * f64::from(b[i]);
    }
    (ab / (aa.sqrt() * bb.sqrt()).max(1e-12)) as f32
}

fn main() {
    let dir = out_dir();

    // ── Timing: what unlock() and the background pass cost ────────────
    let t = Instant::now();
    for id in SoundId::ALL {
        let _ = render(*id, take_seed(0));
    }
    let first = t.elapsed();
    let t = Instant::now();
    let sched = schedule();
    for (id, k) in &sched {
        let _ = render(*id, take_seed(*k));
    }
    let all = t.elapsed();
    println!(
        "render: first take of {} sounds {:.0} ms; all {} takes {:.0} ms (native, this build)\n",
        SoundId::ALL.len(),
        first.as_secs_f64() * 1e3,
        sched.len(),
        all.as_secs_f64() * 1e3
    );

    // ── One-shots ─────────────────────────────────────────────────────
    let gap = vec![0.0f32; (0.4 * SR) as usize];
    for id in SoundId::ALL {
        let info = id.info();
        let t = Instant::now();
        let takes: Vec<_> = (0..info.variants as usize).map(|k| render(*id, take_seed(k))).collect();
        let ms = t.elapsed().as_secs_f64() * 1e3 / takes.len() as f64;
        let mut channels: Vec<Vec<f32>> = vec![Vec::new(); takes[0].channels.len()];
        for t in &takes {
            for (c, data) in t.channels.iter().enumerate() {
                channels[c].extend_from_slice(data);
                channels[c].extend_from_slice(&gap);
            }
        }
        wav::write(&dir.join(format!("{}.wav", id.name())), &channels, SAMPLE_RATE).expect("write wav");
        let r = analyse(&takes[0].channels[0]);
        row(id.name(), &r);
        println!("{:<20} {ms:.1} ms per take", "");
        if takes.len() > 1 {
            let l = likeness(&takes[0].channels[0], &takes[1].channels[0]);
            let peaks: Vec<String> = takes.iter().map(|t| format!("{:.2}s", t.seconds())).collect();
            println!("{:<20} takes {}  take0~take1 correlation {:+.2}", "", peaks.join(" "), l);
        }
    }

    // ── Engines ───────────────────────────────────────────────────────
    println!();
    // (seconds, rpm, load, boost, track m/s) keyframes, linearly joined.
    let programme: &[(f32, f32, f32, f32, f32)] = &[
        (0.0, 0.0, 0.0, 0.0, 0.0),
        (2.5, 0.0, 0.0, 0.0, 0.0),
        (3.0, 0.5, 1.0, 0.0, 1.0),
        (6.0, 0.95, 1.0, 0.0, 11.0),
        (8.0, 1.0, 0.9, 0.0, 12.0),
        (8.1, 1.0, 1.0, 1.0, 12.5),
        (10.0, 1.0, 1.0, 1.0, 16.0),
        (10.1, 0.7, 0.0, 0.0, 15.0),
        (12.5, 0.35, 0.0, 0.0, 8.0),
        (14.0, 0.0, 0.0, 0.0, 0.0),
        (16.0, 0.0, 0.0, 0.0, 0.0),
    ];
    let at = |t: f32| {
        let i = programme.iter().rposition(|k| k.0 <= t).unwrap_or(0);
        let a = programme[i];
        let b = programme[(i + 1).min(programme.len() - 1)];
        let x = if b.0 > a.0 { ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 0.0 };
        let l = |p: f32, q: f32| p + (q - p) * x;
        EngineInput { rpm: l(a.1, b.1), load: l(a.2, b.2), boost: l(a.3, b.3), track_speed: l(a.4, b.4) }
    };
    for kind in EngineKind::ALL {
        let mut e = TankEngine::new(kind, 7, SR);
        let total = (16.0 * SR) as usize;
        let block = 256;
        let mut out = vec![0.0f32; total];
        let t0 = Instant::now();
        for (b, chunk) in out.chunks_mut(block).enumerate() {
            e.set_input(at(b as f32 * block as f32 / SR));
            e.render(chunk);
        }
        let cost = t0.elapsed().as_secs_f64() / 16.0 * 100.0;
        wav::write(&dir.join(format!("engine_{}.wav", kind.name())), &[out.clone()], SAMPLE_RATE).expect("write");
        let seg = |a: f32, b: f32| analyse(&out[(a * SR) as usize..(b * SR) as usize]);
        println!("engine {:<8} (one voice costs {cost:.2}% of a core)", kind.name());
        row("  idle", &seg(0.5, 2.5));
        row("  driving", &seg(5.0, 6.0));
        row("  flat out", &seg(7.0, 8.0));
        row("  boost", &seg(8.5, 10.0));
        row("  coasting", &seg(10.5, 12.0));
    }

    // ── The engine bus: three tanks driving past the listener ─────────
    let mut mix = EngineMixer::new(SR);
    let kinds = [EngineKind::Brawler, EngineKind::Scout, EngineKind::Hauler];
    let hs: Vec<_> = kinds.iter().enumerate().map(|(i, k)| mix.add(*k, i as u64).expect("slot")).collect();
    let listener = Listener { pos: [0.0, 0.0, 0.0], forward: [0.0, 0.0, -1.0] };
    let secs = 10.0;
    let block = 512;
    let n = (secs * SR) as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut i = 0;
    while i < n {
        let t = i as f32 / SR;
        for (j, h) in hs.iter().enumerate() {
            // Each crosses left to right 15 m ahead, at its own time.
            let x = (t - 1.5 - 2.5 * j as f32) * 12.0;
            let pos = [x, 0.0, -15.0 - 10.0 * j as f32];
            let input = EngineInput { rpm: 0.8, load: 0.8, boost: 0.0, track_speed: 12.0 };
            mix.set(*h, input, spatialize(&listener, pos), 1.0);
        }
        let end = (i + block).min(n);
        mix.render(&mut l[i..end], &mut r[i..end]);
        i = end;
    }
    wav::write(&dir.join("engines_passing.wav"), &[l.clone(), r.clone()], SAMPLE_RATE).expect("write");
    println!();
    row("engines_passing L", &analyse(&l));
    row("engines_passing R", &analyse(&r));

    // ── Ambience: calm, a blustery spell, a storm rolling in and out ────
    let mut amb = Ambience::new(3, SR);
    let weather: &[(f32, f32, f32)] = &[(0.0, 0.3, 0.0), (6.0, 1.0, 0.0), (12.0, 1.0, 1.0), (18.0, 1.0, 1.0), (24.0, 0.4, 0.0)];
    let total = (24.0 * SR) as usize;
    let (mut l, mut r) = (vec![0.0f32; total], vec![0.0f32; total]);
    let mut i = 0;
    while i < total {
        let t = i as f32 / SR;
        let k = weather.iter().rposition(|w| w.0 <= t).unwrap_or(0);
        let (a, b) = (weather[k], weather[(k + 1).min(weather.len() - 1)]);
        let x = if b.0 > a.0 { ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 0.0 };
        amb.set(a.1 + (b.1 - a.1) * x, a.2 + (b.2 - a.2) * x);
        let end = (i + 512).min(total);
        amb.render(&mut l[i..end], &mut r[i..end]);
        i = end;
    }
    wav::write(&dir.join("ambience.wav"), &[l.clone(), r.clone()], SAMPLE_RATE).expect("write");
    println!();
    let seg = |a: f32, b: f32| analyse(&l[(a * SR) as usize..(b * SR) as usize]);
    row("ambience calm", &seg(1.0, 3.0));
    row("ambience windy", &seg(6.0, 9.0));
    row("ambience storm", &seg(13.0, 18.0));

    // ── Music ─────────────────────────────────────────────────────────
    println!();
    let t = Instant::now();
    let title_len = Music::track_seconds(MusicTrack::Title);
    let [l, r] = music::render_track(MusicTrack::Title, 0.0, title_len + 6.0);
    println!(
        "music: title loop {title_len:.1} s, battle loop {:.1} s; the band costs {:.1}% of a core (native)",
        Music::track_seconds(MusicTrack::Battle),
        t.elapsed().as_secs_f32() / (title_len + 6.0) * 100.0
    );
    wav::write(&dir.join("music_title.wav"), &[l.clone(), r.clone()], SAMPLE_RATE).expect("write");
    let seg = |x: &[f32], a: f32, b: f32| analyse(&x[(a * SR) as usize..((b * SR) as usize).min(x.len())]);
    for (name, a, b) in [("title A (riff)", 2.5, 20.0), ("title B (tune)", 20.0, 40.0), ("title C (bridge)", 40.0, 60.0), ("title A' (all)", 60.0, 80.0)] {
        row(name, &seg(&l, a, b));
    }
    println!("{:<20} loop seam: last 0.5 s rms {:.1} dB, first 0.5 s after the loop {:.1} dB", "",
        db(seg(&l, title_len - 0.5, title_len).rms), db(seg(&l, title_len, title_len + 0.5).rms));
    // Battle: calm for a while, then mid, then full, then back down.
    let sweep: &[(f32, f32)] = &[(0.0, 0.0), (12.0, 0.0), (16.0, 0.5), (30.0, 0.5), (34.0, 1.0), (50.0, 1.0), (54.0, 0.2), (64.0, 0.2)];
    let [l, r] = music::render_with(64.0, |m, t| {
        let k = sweep.iter().rposition(|w| w.0 <= t).unwrap_or(0);
        let (a, b) = (sweep[k], sweep[(k + 1).min(sweep.len() - 1)]);
        let x = if b.0 > a.0 { ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 0.0 };
        m.set(MusicTrack::Battle, a.1 + (b.1 - a.1) * x);
    });
    wav::write(&dir.join("music_battle_sweep.wav"), &[l.clone(), r.clone()], SAMPLE_RATE).expect("write");
    for (name, a, b) in [("battle calm", 2.0, 12.0), ("battle mid", 20.0, 30.0), ("battle full", 38.0, 50.0), ("battle back to 0.2", 58.0, 64.0)] {
        row(name, &seg(&l, a, b));
    }
    for s in Stinger::ALL {
        let [l, r] = music::render_stinger(s);
        wav::write(&dir.join(format!("stinger_{}.wav", s.name())), &[l.clone(), r.clone()], SAMPLE_RATE).expect("write");
        let rep = analyse(&l);
        row(&format!("stinger {}", s.name()), &rep);
    }
    println!("\nwrote {} files to {}", SoundId::ALL.len() + EngineKind::ALL.len() + 4 + Stinger::ALL.len(), dir.display());
}
