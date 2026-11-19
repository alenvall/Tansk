//! The recorded-sound tables: every file they name exists, is a small Ogg
//! Vorbis file and has a licence entry; the load order covers everything
//! once with the urgent sounds first; the engine crossfade is sane.

use std::collections::HashSet;
use std::path::PathBuf;

use arena_audio::samples::{all_files, engine_mix, load_order, takes, Asset, Bed};
use arena_audio::{EngineInput, EngineKind, SoundId};

fn sounds_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../arena-client/assets/sounds")
}

#[test]
fn every_named_file_exists_and_is_ogg_vorbis() {
    let mut total = 0;
    for f in all_files() {
        let path = sounds_dir().join(format!("{f}.ogg"));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(&bytes[..4], b"OggS", "{f}: not an Ogg file");
        assert!(bytes.windows(6).take(200).any(|w| w == b"vorbis"), "{f}: not Vorbis");
        assert!(bytes.len() < 400_000, "{f}: {} bytes is too big for one sound", bytes.len());
        total += bytes.len();
    }
    assert!(total < 6_000_000, "all recordings together are {total} bytes");
}

#[test]
fn every_file_in_the_folder_is_used_and_licensed() {
    let used: HashSet<&str> = all_files().into_iter().collect();
    let licences = std::fs::read_to_string(sounds_dir().join("LICENSES.md")).expect("LICENSES.md");
    for e in std::fs::read_dir(sounds_dir()).unwrap() {
        let name = e.unwrap().file_name().into_string().unwrap();
        if let Some(stem) = name.strip_suffix(".ogg") {
            assert!(used.contains(stem), "{name} is shipped but never played");
            assert!(licences.contains(&format!("`{name}`")), "{name} has no entry in LICENSES.md");
        }
    }
}

#[test]
fn the_load_order_covers_every_file_once_urgent_first() {
    let order = load_order();
    let files: Vec<&str> = order.iter().map(|a| a.file()).collect();
    let unique: HashSet<&str> = files.iter().copied().collect();
    assert_eq!(unique.len(), files.len(), "a file is fetched twice");
    let expected = SoundId::ALL.iter().map(|id| takes(*id).len()).sum::<usize>() + Bed::ALL.len();
    assert_eq!(files.len(), expected);
    // The main gun, the hits and the engines come before the horns and UI.
    let at = |a: Asset| order.iter().position(|x| *x == a).unwrap();
    assert!(at(Asset::Shot(SoundId::Cannon, 0)) < at(Asset::Shot(SoundId::UiTick, 0)));
    assert!(at(Asset::Bed(Bed::EngineIdle)) < at(Asset::Shot(SoundId::Horn0, 0)));
    assert!(at(Asset::Shot(SoundId::HitHull, 0)) < at(Asset::Shot(SoundId::HitHull, 1)));
    for a in &order {
        assert!(a.url().starts_with("sounds/") && a.url().ends_with(".ogg"));
    }
}

#[test]
fn guns_each_have_their_own_recordings() {
    let guns = [
        SoundId::StubGun,
        SoundId::Cannon,
        SoundId::BigBore,
        SoundId::TwinBarrel,
        SoundId::LongGun,
        SoundId::AutocannonBurst,
        SoundId::MachineGun,
        SoundId::CoilgunZap,
        SoundId::TeslaCrackle,
        SoundId::RocketLaunch,
        SoundId::MortarShot,
        SoundId::Flamethrower,
    ];
    let mut seen = HashSet::new();
    for g in guns {
        assert!(!takes(g).is_empty(), "{g:?} has no recording");
        for t in takes(g) {
            assert!(seen.insert(t.file), "{} is shared between guns", t.file);
            assert!(t.trim > 0.2 && t.trim < 2.0, "{}: trim {}", t.file, t.trim);
        }
    }
}

#[test]
fn the_engine_crossfade_is_bounded_and_follows_the_revs() {
    for kind in EngineKind::ALL {
        let mut prev_idle = f32::MAX;
        for i in 0..=20 {
            let rpm = i as f32 / 20.0;
            let m = engine_mix(kind, EngineInput { rpm, load: 0.5, boost: 0.0, track_speed: rpm * 10.0 }, 0.5);
            for (g, r) in m.gain.iter().zip(&m.rate) {
                assert!(g.is_finite() && (0.0..=1.2).contains(g), "{kind:?} rpm {rpm}: gain {g}");
                assert!(r.is_finite() && (0.4..=2.0).contains(r), "{kind:?} rpm {rpm}: rate {r}");
            }
            assert!(m.gain[0] <= prev_idle + 1e-6, "idle grows with revs");
            prev_idle = m.gain[0];
        }
        let idle = engine_mix(kind, EngineInput::default(), 0.0);
        assert!(idle.gain[0] > 0.5 && idle.gain[2] < 0.01 && idle.gain[3] < 0.01);
        let flat = engine_mix(kind, EngineInput { rpm: 1.0, load: 1.0, boost: 1.0, track_speed: 12.0 }, 1.0);
        assert!(flat.gain[2] > 0.5 && flat.gain[0] < 0.01 && flat.gain[3] > 0.5);
        let nan = engine_mix(kind, EngineInput { rpm: f32::NAN, load: f32::INFINITY, boost: -1.0, track_speed: f32::NAN }, f32::NAN);
        assert!(nan.gain.iter().chain(&nan.rate).all(|v| v.is_finite()));
    }
    let scout = engine_mix(EngineKind::Scout, EngineInput::default(), 0.5);
    let hauler = engine_mix(EngineKind::Hauler, EngineInput::default(), 0.5);
    assert!(scout.rate[0] > hauler.rate[0], "a small engine idles faster");
}
