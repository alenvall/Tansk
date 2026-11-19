//! What can be checked without ears: every buffer is finite, peaks at or
//! under 0.99, has no DC, is a sane length and is not silent; takes differ;
//! the engine is bounded, continuous across blocks and responds to its
//! inputs; the spatial maths pans and attenuates the right way.

use arena_audio::analysis::analyse;
use arena_audio::dsp::SR;
use arena_audio::spatial::{pan_gains, spatialize, Listener, MAX_DISTANCE};
use arena_audio::{render, schedule, take_seed, Bank, EngineInput, EngineKind, EngineMixer, SoundId, TankEngine};

#[test]
fn every_sound_is_finite_bounded_centred_and_sane() {
    for id in SoundId::ALL {
        let info = id.info();
        for k in 0..info.variants as usize {
            let take = render(*id, take_seed(k));
            assert_eq!(take.channels.len(), if info.stereo { 2 } else { 1 }, "{id:?} channel count");
            for ch in &take.channels {
                assert_eq!(ch.len(), take.frames(), "{id:?} channels differ in length");
                assert!(ch.iter().all(|s| s.is_finite()), "{id:?} take {k}: non-finite sample");
                let r = analyse(ch);
                assert!(r.peak <= 0.99, "{id:?} take {k}: peak {}", r.peak);
                assert!(r.peak >= 0.1, "{id:?} take {k}: nearly silent, peak {}", r.peak);
                assert!(r.rms > 0.005, "{id:?} take {k}: rms {}", r.rms);
                assert!(r.dc.abs() < 1e-3, "{id:?} take {k}: dc {}", r.dc);
                assert!(
                    r.seconds >= 0.03 && r.seconds <= info.seconds + 1e-3,
                    "{id:?} take {k}: {} s (max {})",
                    r.seconds,
                    info.seconds
                );
                // Starts and ends quietly: no click at either edge.
                assert!(ch[0].abs() < 0.05, "{id:?} take {k}: starts at {}", ch[0]);
                assert!(ch[ch.len() - 1].abs() < 1e-3, "{id:?} take {k}: ends at {}", ch[ch.len() - 1]);
            }
        }
    }
}

#[test]
fn ids_are_unique_and_round_trip_by_name() {
    for (i, id) in SoundId::ALL.iter().enumerate() {
        assert_eq!(id.index(), i);
        assert_eq!(SoundId::from_name(id.name()), Some(*id));
        assert!(id.info().voices >= 1 && id.info().variants >= 1);
    }
    let mut names: Vec<_> = SoundId::ALL.iter().map(|s| s.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), SoundId::ALL.len());
}

#[test]
fn the_same_seed_is_the_same_take_and_another_seed_is_not() {
    for id in SoundId::ALL {
        let a = render(*id, 11);
        let b = render(*id, 11);
        assert_eq!(a.channels, b.channels, "{id:?} is not deterministic");
        let c = render(*id, 12);
        assert_ne!(a.channels, c.channels, "{id:?} does not vary with the seed");
    }
}

#[test]
fn stereo_sounds_are_actually_wide() {
    for id in SoundId::ALL.iter().filter(|s| s.info().stereo) {
        let t = render(*id, 3);
        let (l, r) = (&t.channels[0], &t.channels[1]);
        let diff: f32 = l.iter().zip(r).map(|(a, b)| (a - b).abs()).sum::<f32>() / l.len() as f32;
        assert!(diff > 1e-4, "{id:?} left and right are the same");
    }
}

#[test]
fn the_bank_fills_first_takes_then_the_rest() {
    let mut bank = Bank::new();
    bank.render_first_takes();
    for id in SoundId::ALL {
        assert_eq!(bank.takes(*id).len(), 1, "{id:?}");
    }
    assert!(!bank.is_complete());
    bank.render_all();
    assert!(bank.is_complete());
    for id in SoundId::ALL {
        assert_eq!(bank.takes(*id).len(), id.info().variants as usize, "{id:?}");
    }
    assert_eq!(schedule().len(), SoundId::ALL.iter().map(|s| s.info().variants as usize).sum::<usize>());
}

fn run(e: &mut TankEngine, input: EngineInput, seconds: f32) -> Vec<f32> {
    e.set_input(input);
    let mut out = vec![0.0; (seconds * SR) as usize];
    for chunk in out.chunks_mut(128) {
        e.render(chunk);
    }
    out
}

fn idle() -> EngineInput {
    EngineInput::default()
}

fn flat_out() -> EngineInput {
    EngineInput { rpm: 1.0, load: 1.0, boost: 0.0, track_speed: 12.0 }
}

#[test]
fn the_engine_is_finite_and_bounded_whatever_it_is_told() {
    let inputs = [
        idle(),
        flat_out(),
        EngineInput { rpm: 1.0, load: 1.0, boost: 1.0, track_speed: 40.0 },
        EngineInput { rpm: 0.3, load: 0.0, boost: 0.0, track_speed: 5.0 },
        EngineInput { rpm: f32::NAN, load: f32::INFINITY, boost: -3.0, track_speed: -1e9 },
        EngineInput { rpm: 7.0, load: 7.0, boost: 7.0, track_speed: 7e6 },
    ];
    for kind in EngineKind::ALL {
        let mut e = TankEngine::new(kind, 1, SR);
        for input in inputs {
            let out = run(&mut e, input, 0.6);
            assert!(out.iter().all(|s| s.is_finite() && s.abs() < 0.99), "{kind:?} {input:?}");
        }
    }
}

#[test]
fn the_engine_has_no_dc_once_settled() {
    for kind in EngineKind::ALL {
        let mut e = TankEngine::new(kind, 2, SR);
        for input in [idle(), flat_out()] {
            let _ = run(&mut e, input, 1.0);
            let r = analyse(&run(&mut e, input, 2.0));
            assert!(r.dc.abs() < 2e-3, "{kind:?} {input:?}: dc {}", r.dc);
        }
    }
}

#[test]
fn load_rpm_and_speed_are_heard() {
    for kind in EngineKind::ALL {
        let mut e = TankEngine::new(kind, 3, SR);
        let _ = run(&mut e, idle(), 0.5);
        let at_idle = analyse(&run(&mut e, idle(), 1.0));
        let idle_hz = e.firing_hz();
        let _ = run(&mut e, flat_out(), 0.8);
        let full = analyse(&run(&mut e, flat_out(), 1.0));
        let full_hz = e.firing_hz();
        assert!(full.rms > at_idle.rms * 1.5, "{kind:?}: full {} vs idle {}", full.rms, at_idle.rms);
        assert!(full_hz > idle_hz * 3.0, "{kind:?}: firing {idle_hz} -> {full_hz}");
        assert!((25.0..80.0).contains(&idle_hz), "{kind:?}: idle firing {idle_hz} Hz");
        assert!(at_idle.rms > 0.02, "{kind:?}: idle is inaudible, rms {}", at_idle.rms);

        // Tracks: the same engine rolling is louder above 400 Hz than parked.
        let parked = EngineInput { rpm: 0.4, load: 0.3, boost: 0.0, track_speed: 0.0 };
        let rolling = EngineInput { track_speed: 10.0, ..parked };
        let mut a = TankEngine::new(kind, 4, SR);
        let mut b = TankEngine::new(kind, 4, SR);
        let _ = run(&mut a, parked, 0.8);
        let _ = run(&mut b, rolling, 0.8);
        let pa = analyse(&run(&mut a, parked, 1.0));
        let pb = analyse(&run(&mut b, rolling, 1.0));
        let mids = |r: &arena_audio::analysis::Report| r.rms * (r.bands[1] + r.bands[2]).sqrt();
        assert!(mids(&pb) > mids(&pa) * 1.2, "{kind:?}: track clatter not heard");
    }
}

#[test]
fn an_engine_block_boundary_is_as_smooth_as_its_inside() {
    let mut whole = TankEngine::new(EngineKind::Brawler, 5, SR);
    let mut split = TankEngine::new(EngineKind::Brawler, 5, SR);
    whole.set_input(flat_out());
    split.set_input(flat_out());
    let mut a = vec![0.0; 4800];
    whole.render(&mut a);
    let mut b = vec![0.0; 4800];
    for chunk in b.chunks_mut(37) {
        split.render(chunk);
    }
    assert_eq!(a, b, "block size changes the output");
}

#[test]
fn the_mixer_pans_fades_and_frees_engines() {
    let mut mix = EngineMixer::new(SR);
    let listener = Listener { pos: [0.0; 3], forward: [0.0, 0.0, -1.0] };
    let h = mix.add(EngineKind::Brawler, 9).expect("slot");
    let (mut l, mut r) = (vec![0.0; 1024], vec![0.0; 1024]);
    for _ in 0..40 {
        mix.set(h, flat_out(), spatialize(&listener, [20.0, 0.0, -1.0]), 1.0);
        mix.render(&mut l, &mut r);
    }
    let el: f32 = l.iter().map(|s| s * s).sum();
    let er: f32 = r.iter().map(|s| s * s).sum();
    assert!(er > el * 3.0, "a tank to the right is not on the right: L {el} R {er}");
    assert!(l.iter().chain(&r).all(|s| s.is_finite() && s.abs() <= 1.0));
    mix.remove(h);
    for _ in 0..20 {
        mix.render(&mut l, &mut r);
    }
    assert_eq!(mix.active(), 0);
    assert!(l.iter().chain(&r).all(|s| *s == 0.0));
    // The handle is dead; a new engine gets a fresh one.
    mix.set(h, flat_out(), spatialize(&listener, [0.0; 3]), 1.0);
    let h2 = mix.add(EngineKind::Scout, 1).expect("slot");
    assert_ne!(h, h2);
    for i in 0..arena_audio::MAX_ENGINES - 1 {
        assert!(mix.add(EngineKind::Hauler, i as u64).is_some());
    }
    assert!(mix.add(EngineKind::Hauler, 99).is_none());
}

#[test]
fn spatial_pans_attenuates_and_dulls() {
    let me = Listener { pos: [0.0; 3], forward: [0.0, 0.0, -1.0] };
    let right = spatialize(&me, [30.0, 0.0, 0.0]);
    let left = spatialize(&me, [-30.0, 0.0, 0.0]);
    let ahead = spatialize(&me, [0.0, 0.0, -30.0]);
    let behind = spatialize(&me, [0.0, 0.0, 30.0]);
    assert!(right.pan > 0.5 && left.pan < -0.5);
    assert!(ahead.pan.abs() < 1e-3);
    assert!(behind.cutoff_hz < ahead.cutoff_hz);
    let near = spatialize(&me, [0.0, 0.0, -5.0]);
    let far = spatialize(&me, [0.0, 0.0, -150.0]);
    assert!(near.gain > 0.99);
    assert!(far.gain < near.gain * 0.2 && far.gain > 0.0);
    assert!(far.cutoff_hz < near.cutoff_hz);
    assert_eq!(spatialize(&me, [0.0, 0.0, -MAX_DISTANCE * 1.1]).gain, 0.0);
    // Turning to face the source centres it.
    let turned = Listener { forward: [1.0, 0.0, 0.0], ..me };
    assert!(spatialize(&turned, [30.0, 0.0, 0.0]).pan.abs() < 1e-3);
    let (l, r) = pan_gains(0.0);
    assert!((l * l + r * r - 1.0).abs() < 1e-5);
    assert!(spatialize(&me, [f32::NAN, 0.0, 0.0]).gain == 0.0);
}

#[test]
fn wav_encoding_has_the_right_header_and_size() {
    let data = vec![vec![0.0f32, 0.5, -0.5, 1.0], vec![0.0, 0.25, -0.25, -1.0]];
    let bytes = arena_audio::wav::encode(&data, 48_000);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(bytes.len(), 44 + 4 * 2 * 2);
    assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2);
    assert_eq!(u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]), 48_000);
}

#[test]
fn horns_are_six_distinct_sounds_and_any_index_plays_one() {
    let horns: Vec<_> = (0..6).map(SoundId::horn).collect();
    for (i, h) in horns.iter().enumerate() {
        assert_eq!(h.name(), format!("horn_{i}"));
        let s = h.info().seconds;
        assert!((0.6..=2.0).contains(&s), "{h:?} max length {s}");
        let take = render(*h, 0);
        assert!((0.5..=2.0).contains(&take.seconds()), "{h:?} is {} s", take.seconds());
    }
    assert_eq!(SoundId::horn(6), SoundId::horn(0));
    assert_eq!(SoundId::horn(usize::MAX), SoundId::HORNS[usize::MAX % 6]);
}

fn ambience(a: &mut arena_audio::Ambience, wind: f32, storm: f32, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    a.set(wind, storm);
    let n = (seconds * SR) as usize;
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        a.render(cl, cr);
    }
    (l, r)
}

#[test]
fn ambience_is_bounded_centred_and_follows_the_weather() {
    let mut a = arena_audio::Ambience::new(7, SR);
    let (l, r) = ambience(&mut a, 0.0, 0.0, 0.5);
    assert!(l.iter().chain(&r).all(|s| *s == 0.0), "still air is not silent");
    let mut level = Vec::new();
    for (wind, storm) in [(0.3, 0.0), (1.0, 0.0), (1.0, 1.0)] {
        let _ = ambience(&mut a, wind, storm, 2.0);
        let (l, r) = ambience(&mut a, wind, storm, 4.0);
        for ch in [&l, &r] {
            let rep = analyse(ch);
            assert!(ch.iter().all(|s| s.is_finite() && s.abs() < 0.99), "{wind} {storm}");
            assert!(rep.dc.abs() < 2e-3, "{wind} {storm}: dc {}", rep.dc);
            assert!(rep.rms > 0.003, "{wind} {storm}: inaudible");
        }
        assert_ne!(l, r, "ambience is not wide");
        level.push(analyse(&l).rms);
    }
    assert!(level[1] > level[0] * 1.3, "more wind is not louder: {level:?}");
    assert!(level[2] > level[1] * 1.3, "the storm is not louder: {level:?}");
    for bad in [f32::NAN, f32::INFINITY, -5.0, 9.0] {
        let (l, _) = ambience(&mut a, bad, bad, 0.2);
        assert!(l.iter().all(|s| s.is_finite() && s.abs() < 0.99));
    }
}
