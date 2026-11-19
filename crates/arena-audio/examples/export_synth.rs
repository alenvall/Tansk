//! Renders the sounds that stay synthesized (no fitting recording) to WAV
//! files, so the browser can fetch them like the recordings instead of
//! synthesizing them at run time; also prints what every sound's take costs
//! to render, the fallback's worst case.
//!
//! `cargo run --release -p arena-audio --example export_synth -- <out-dir>`
//! then encode each WAV to Ogg Vorbis into `crates/arena-client/assets/sounds`.

use std::time::Instant;

use arena_audio::samples::SYNTH_ONLY;
use arena_audio::{render, take_seed, wav, SoundId, SAMPLE_RATE};

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "synth-out".into()));
    std::fs::create_dir_all(&dir).expect("out dir");
    for id in SYNTH_ONLY {
        for k in 0..id.info().variants as usize {
            let take = render(id, take_seed(k));
            // Peak-normalised to -1 dBFS like the recordings (the player
            // scales every file by its sound's peak and trim).
            let peak = take.channels.iter().flatten().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
            let ch: Vec<Vec<f32>> = take.channels.iter().map(|c| c.iter().map(|s| s * 0.89 / peak).collect()).collect();
            let path = dir.join(format!("synth_{}_{}.wav", id.name(), k + 1));
            wav::write(&path, &ch, SAMPLE_RATE).expect("write wav");
        }
    }
    let mut costs: Vec<(f64, SoundId)> = SoundId::ALL
        .iter()
        .map(|id| {
            let t = Instant::now();
            let _ = render(*id, take_seed(0));
            (t.elapsed().as_secs_f64() * 1e3, *id)
        })
        .collect();
    costs.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (ms, id) in costs.iter().take(12) {
        println!("{:>7.2} ms  {}", ms, id.name());
    }
}
