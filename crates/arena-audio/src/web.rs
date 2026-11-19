//! Web Audio playback (wasm32 only).
//!
//! Graph: one-shots are `AudioBufferSource → Gain → [BiquadFilter (low-pass)
//! → StereoPanner] → sfx bus`; engines are one `ScriptProcessorNode` running
//! the [`EngineMixer`] on the audio callback → engine bus; both buses go
//! through the master gain to the destination.
//!
//! Why a ScriptProcessor and not an AudioWorklet: a worklet runs in its own
//! global scope, so the Rust engine would have to be compiled and
//! instantiated a second time inside it (a separate WASM build and a module
//! URL to ship). The ScriptProcessor runs the same code on the main thread
//! with a 2048-frame buffer (43 ms), and is supported by every browser. One
//! engine costs about 1% of a core in WASM (10 ms per second of sound,
//! measured in Node); engines out of earshot are not rendered. It is
//! deprecated but not removed; if it is ever missing, engines are silent and
//! everything else still plays.
//!
//! Pre-rendering: `unlock()` renders one take of every sound (about 120 ms
//! in WASM, measured in Node; it logs the real time), and `tick()` renders
//! the remaining variations (about 170 ms more) 3 ms per frame.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, AudioProcessingEvent,
    BiquadFilterType, GainNode, ScriptProcessorNode,
};

use crate::dsp::{Rng, SR};
use crate::engine::{EngineInput, EngineKind};
use crate::mixer::{EngineHandle, EngineMixer};
use crate::sounds::{render, SoundId};
use crate::spatial::{spatialize, Listener, CENTRE};
use crate::{schedule, take_seed};

/// Most one-shots playing at once, all sounds together.
const MAX_ONE_SHOTS: usize = 64;
/// Frames per engine callback.
const ENGINE_BLOCK: u32 = 2048;
/// Milliseconds of background rendering per `tick()`.
const TICK_BUDGET_MS: f64 = 3.0;

#[derive(Clone, Copy, Debug)]
struct Volumes {
    master: f32,
    sfx: f32,
    engine: f32,
}

struct Voice {
    src: AudioBufferSourceNode,
    gain: GainNode,
    ends: f64,
}

struct Live {
    ctx: AudioContext,
    master: GainNode,
    sfx: GainNode,
    engine_bus: GainNode,
    buffers: Vec<Vec<AudioBuffer>>,
    voices: Vec<Vec<Voice>>,
    last_take: Vec<usize>,
    schedule: Vec<(SoundId, usize)>,
    next_render: usize,
    _processor: Option<ScriptProcessorNode>,
    _callback: Option<Closure<dyn FnMut(AudioProcessingEvent)>>,
}

/// The game's audio. Create it at startup; call [`Audio::unlock`] from the
/// first click or key press (browsers keep audio off until then), and
/// [`Audio::tick`] once per frame. Every call before unlock is a quiet
/// no-op, except that engines and settings are remembered.
pub struct Audio {
    live: Option<Live>,
    volumes: Volumes,
    listener: Listener,
    engines: Rc<RefCell<EngineMixer>>,
    rng: Rng,
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

fn log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}

fn now_ms() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now())
}

impl Audio {
    #[must_use]
    pub fn new() -> Self {
        Self {
            live: None,
            volumes: Volumes { master: 0.8, sfx: 1.0, engine: 0.7 },
            listener: Listener::default(),
            engines: Rc::new(RefCell::new(EngineMixer::new(SR))),
            rng: Rng::new(now_ms().to_bits()),
        }
    }

    #[must_use]
    pub fn is_unlocked(&self) -> bool {
        self.live.is_some()
    }

    /// Starts audio. Call from inside a user-gesture handler (click, key,
    /// touch). Safe to call on every gesture: after the first it only
    /// resumes a suspended context.
    pub fn unlock(&mut self) {
        if let Some(live) = &self.live {
            if live.ctx.state() != AudioContextState::Running {
                let _ = live.ctx.resume();
            }
            return;
        }
        match self.start() {
            Ok(live) => self.live = Some(live),
            Err(e) => web_sys::console::warn_2(&"arena-audio: could not start Web Audio".into(), &e),
        }
    }

    fn start(&mut self) -> Result<Live, wasm_bindgen::JsValue> {
        let ctx = AudioContext::new()?;
        let master = ctx.create_gain()?;
        let sfx = ctx.create_gain()?;
        let engine_bus = ctx.create_gain()?;
        master.gain().set_value(self.volumes.master);
        sfx.gain().set_value(self.volumes.sfx);
        engine_bus.gain().set_value(self.volumes.engine);
        sfx.connect_with_audio_node(&master)?;
        engine_bus.connect_with_audio_node(&master)?;
        master.connect_with_audio_node(&ctx.destination())?;
        let _ = ctx.resume();

        // Engines on a ScriptProcessor.
        let rate = ctx.sample_rate();
        self.engines.borrow_mut().set_sample_rate(rate);
        let (processor, callback) = match ctx
            .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
                ENGINE_BLOCK,
                1,
                2,
            ) {
            Ok(node) => {
                let mixer = self.engines.clone();
                let mut left = vec![0.0f32; ENGINE_BLOCK as usize];
                let mut right = vec![0.0f32; ENGINE_BLOCK as usize];
                let cb = Closure::<dyn FnMut(AudioProcessingEvent)>::new(move |e: AudioProcessingEvent| {
                    let Ok(out) = e.output_buffer() else { return };
                    let n = out.length() as usize;
                    left.resize(n, 0.0);
                    right.resize(n, 0.0);
                    if let Ok(mut m) = mixer.try_borrow_mut() {
                        m.render(&mut left, &mut right);
                    } else {
                        left.fill(0.0);
                        right.fill(0.0);
                    }
                    let _ = out.copy_to_channel(&left, 0);
                    if out.number_of_channels() > 1 {
                        let _ = out.copy_to_channel(&right, 1);
                    }
                });
                node.set_onaudioprocess(Some(cb.as_ref().unchecked_ref()));
                node.connect_with_audio_node(&engine_bus)?;
                (Some(node), Some(cb))
            }
            Err(e) => {
                web_sys::console::warn_2(&"arena-audio: no ScriptProcessor; engines are silent".into(), &e);
                (None, None)
            }
        };

        let mut live = Live {
            ctx,
            master,
            sfx,
            engine_bus,
            buffers: vec![Vec::new(); SoundId::ALL.len()],
            voices: (0..SoundId::ALL.len()).map(|_| Vec::new()).collect(),
            last_take: vec![usize::MAX; SoundId::ALL.len()],
            schedule: schedule(),
            next_render: 0,
            _processor: processor,
            _callback: callback,
        };
        // One take of every sound now, so anything can play from the start.
        let t0 = now_ms();
        while live.next_render < SoundId::ALL.len() {
            live.render_one()?;
        }
        log(&format!(
            "arena-audio: {} sounds ready in {:.0} ms ({} Hz output); {} more takes render in the background",
            SoundId::ALL.len(),
            now_ms() - t0,
            rate,
            live.schedule.len() - live.next_render
        ));
        Ok(live)
    }

    /// Call once per frame: renders more take variations, a few ms at a
    /// time, until all are done.
    pub fn tick(&mut self) {
        let Some(live) = &mut self.live else { return };
        if live.next_render >= live.schedule.len() {
            return;
        }
        let t0 = now_ms();
        while live.next_render < live.schedule.len() && now_ms() - t0 < TICK_BUDGET_MS {
            if live.render_one().is_err() {
                live.next_render = live.schedule.len();
            }
        }
    }

    /// Where the ears are: the player's tank, facing where the camera looks.
    pub fn set_listener(&mut self, pos: [f32; 3], forward: [f32; 3]) {
        self.listener = Listener { pos, forward };
    }

    /// Plays `id` at a world position (panned and attenuated from the
    /// listener) or, with `None`, centred at full level (UI, radio, your own
    /// tank's gun). `gain` is a linear level, 1 is the bank's own.
    pub fn play(&mut self, id: SoundId, pos: Option<[f32; 3]>, gain: f32) {
        self.play_pitched(id, pos, gain, 1.0);
    }

    /// [`Audio::play`] with a playback rate on top of the usual jitter
    /// (e.g. a bigger tank's hit plays at 0.9).
    pub fn play_pitched(&mut self, id: SoundId, pos: Option<[f32; 3]>, gain: f32, rate: f32) {
        let heard = pos.map_or(CENTRE, |p| spatialize(&self.listener, p));
        let level = heard.gain * gain.max(0.0);
        if !level.is_finite() || level < 0.002 {
            return;
        }
        let Some(live) = &mut self.live else { return };
        let _ = live.play(id, level, heard.pan, heard.cutoff_hz, pos.is_some(), rate, &mut self.rng);
    }

    /// Stops every playing copy of `id`, with a short fade (e.g. a coilgun
    /// charge that was cancelled).
    pub fn stop(&mut self, id: SoundId) {
        let Some(live) = &mut self.live else { return };
        let now = live.ctx.current_time();
        for v in live.voices[id.index()].drain(..) {
            fade_out(&v, now);
        }
    }

    pub fn set_master_volume(&mut self, v: f32) {
        self.volumes.master = clean(v);
        if let Some(l) = &self.live {
            ramp(&l.ctx, &l.master, self.volumes.master);
        }
    }

    pub fn set_sfx_volume(&mut self, v: f32) {
        self.volumes.sfx = clean(v);
        if let Some(l) = &self.live {
            ramp(&l.ctx, &l.sfx, self.volumes.sfx);
        }
    }

    pub fn set_engine_volume(&mut self, v: f32) {
        self.volumes.engine = clean(v);
        if let Some(l) = &self.live {
            ramp(&l.ctx, &l.engine_bus, self.volumes.engine);
        }
    }

    /// `(master, sfx, engine)`.
    #[must_use]
    pub fn volumes(&self) -> (f32, f32, f32) {
        (self.volumes.master, self.volumes.sfx, self.volumes.engine)
    }

    /// Starts an engine voice for one visible tank. `seed` (e.g. the tank's
    /// id) gives it its own character. `None` when all engine slots are busy.
    pub fn engine_add(&mut self, kind: EngineKind, seed: u64) -> Option<EngineHandle> {
        self.engines.borrow_mut().add(kind, seed)
    }

    /// Every frame for every engine: what it is doing and where it is.
    /// `gain` is its own level (1 normal).
    pub fn engine_set(&mut self, h: EngineHandle, pos: [f32; 3], input: EngineInput, gain: f32) {
        let heard = spatialize(&self.listener, pos);
        if let Ok(mut m) = self.engines.try_borrow_mut() {
            m.set(h, input, heard, gain);
        }
    }

    /// [`Audio::engine_set`] for the player's own tank: centred, full level.
    pub fn engine_set_own(&mut self, h: EngineHandle, input: EngineInput, gain: f32) {
        if let Ok(mut m) = self.engines.try_borrow_mut() {
            m.set(h, input, CENTRE, gain);
        }
    }

    /// Fades an engine out (tank destroyed or out of sight) and frees it.
    pub fn engine_remove(&mut self, h: EngineHandle) {
        if let Ok(mut m) = self.engines.try_borrow_mut() {
            m.remove(h);
        }
    }

    /// Suspends the context (tab hidden, game paused); `unlock` resumes.
    pub fn suspend(&mut self) {
        if let Some(l) = &self.live {
            let _ = l.ctx.suspend();
        }
    }
}

impl Live {
    fn render_one(&mut self) -> Result<(), wasm_bindgen::JsValue> {
        let Some(&(id, k)) = self.schedule.get(self.next_render) else { return Ok(()) };
        self.next_render += 1;
        let take = render(id, take_seed(k));
        let frames = take.frames().max(1) as u32;
        let buf = self.ctx.create_buffer(take.channels.len() as u32, frames, SR)?;
        for (c, data) in take.channels.iter().enumerate() {
            buf.copy_to_channel(data, c as i32)?;
        }
        self.buffers[id.index()].push(buf);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn play(
        &mut self,
        id: SoundId,
        level: f32,
        pan: f32,
        cutoff: f32,
        positional: bool,
        rate: f32,
        rng: &mut Rng,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let takes = &self.buffers[id.index()];
        if takes.is_empty() {
            return Ok(());
        }
        // A random take, never the same one twice running when there is a
        // choice.
        let mut k = (rng.unit() * takes.len() as f32) as usize % takes.len();
        if takes.len() > 1 && k == self.last_take[id.index()] {
            k = (k + 1) % takes.len();
        }
        self.last_take[id.index()] = k;
        let buffer = takes[k].clone();

        let info = id.info();
        let now = self.ctx.current_time();
        // Voice limits: drop finished voices, then steal the oldest.
        for vs in &mut self.voices {
            vs.retain(|v| v.ends > now);
        }
        let total: usize = self.voices.iter().map(Vec::len).sum();
        let voices = &mut self.voices[id.index()];
        if voices.len() >= usize::from(info.voices.max(1)) || total >= MAX_ONE_SHOTS {
            if !voices.is_empty() {
                let v = voices.remove(0);
                fade_out(&v, now);
            } else {
                return Ok(());
            }
        }

        let src = self.ctx.create_buffer_source()?;
        src.set_buffer(Some(&buffer));
        let rate = (rate * rng.vary(info.pitch_jitter)).clamp(0.25, 4.0);
        src.playback_rate().set_value(rate);
        let gain = self.ctx.create_gain()?;
        gain.gain().set_value(level.min(4.0));
        src.connect_with_audio_node(&gain)?;
        if positional {
            let filter = self.ctx.create_biquad_filter()?;
            filter.set_type(BiquadFilterType::Lowpass);
            filter.frequency().set_value(cutoff.clamp(200.0, 20_000.0));
            filter.q().set_value(0.5);
            let panner = self.ctx.create_stereo_panner()?;
            panner.pan().set_value(pan.clamp(-1.0, 1.0));
            gain.connect_with_audio_node(&filter)?;
            filter.connect_with_audio_node(&panner)?;
            panner.connect_with_audio_node(&self.sfx)?;
        } else {
            gain.connect_with_audio_node(&self.sfx)?;
        }
        src.start()?;
        let ends = now + buffer.duration() / f64::from(rate) + 0.05;
        self.voices[id.index()].push(Voice { src, gain, ends });
        Ok(())
    }
}

fn fade_out(v: &Voice, now: f64) {
    let _ = v.gain.gain().set_target_at_time(0.0, now, 0.012);
    let _ = web_sys::AudioScheduledSourceNode::stop_with_when(&v.src, now + 0.08);
}

fn ramp(ctx: &AudioContext, node: &GainNode, v: f32) {
    let _ = node.gain().set_target_at_time(v, ctx.current_time(), 0.03);
}

fn clean(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 2.0)
    } else {
        1.0
    }
}
