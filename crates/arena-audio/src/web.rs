//! Web Audio playback (wasm32 only).
//!
//! Two sources for every sound:
//!
//! - **Recordings** ([`crate::samples`]): Ogg Vorbis files fetched in the
//!   background from `sounds/` as soon as [`Audio`] is created (the most
//!   frequent combat sounds first, at most four requests at a time), then
//!   decoded with `decodeAudioData` once the context exists (a few files per
//!   frame; the decoding runs off the main thread). A sound plays its file
//!   once one has loaded; before that it is silent (normally well under a
//!   second after unlock).
//! - **Synthesis**: the procedural bank, now only a fallback: a take is
//!   synthesized (one per frame, lazily) only for a sound whose file failed
//!   to load after a retry, or is still missing 10 s after unlock. The
//!   sounds with no recording ship as offline renders of the synthesizer, so
//!   normally nothing is synthesized at run time at all.
//!
//! Graph: one-shots are `AudioBufferSource → Gain → [BiquadFilter (low-pass)
//! → StereoPanner] → sfx bus`, with a send from the gain into a shared
//! convolution reverb (a synthesized desert "slap and tail"), more of it the
//! further away the sound is. Sampled engines are four looping sources per
//! tank (idle, load, flat-out, track clatter) crossfaded by [`engine_mix`]
//! through their own filter and panner into the engine bus; the synthesized
//! engines run on a ScriptProcessor as before and are faded out for any tank
//! that has a sampled voice. The weather is two looping beds (wind, storm)
//! on the ambience bus, with the synthesized wind as the fallback. A held
//! flamethrower is an ignition burst, a seamless roar loop per tank, and a
//! cut-off tail. Music is
//! synthesized live, unchanged.
//!
//! Variation on every play: a different take than last time, playback rate
//! jitter, and ±1.5 dB of level.
//!
//! For tests and diagnostics, `window.__tanskAudio` holds `{total, loaded,
//! failed, plays, flames}` (files to load, decoded, failed; one-shots
//! started; flamethrowers lit).

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, AudioProcessingEvent, BiquadFilterNode,
    BiquadFilterType, ConvolverNode, GainNode, ScriptProcessorNode, StereoPannerNode,
};

use crate::ambience::Ambience;
use crate::dsp::{Rng, SR};
use crate::engine::{EngineInput, EngineKind};
use crate::mixer::{EngineHandle, EngineMixer};
use crate::music::{Music, MusicTrack, Stinger};
use crate::samples::{engine_mix, load_order, takes, Asset, Bed};
use crate::sounds::{render, SoundId};
use crate::spatial::{spatialize, Listener, Spatial, CENTRE, MAX_DISTANCE};
use crate::take_seed;

/// Most one-shots playing at once, all sounds together.
const MAX_ONE_SHOTS: usize = 64;
/// Frames per engine callback.
const ENGINE_BLOCK: u32 = 2048;
/// Files handed to `decodeAudioData` per frame (it copies the bytes on the
/// main thread before decoding off it).
const DECODES_PER_TICK: usize = 4;
/// A file whose fetch failed is tried once more after this long, ms.
const RETRY_MS: f64 = 4000.0;
/// A sound still without a file this long after unlock (a slow connection)
/// gets its synthesized fallback when it is next played, ms.
const SLOW_LOAD_MS: f64 = 10_000.0;
/// Files fetched at once.
const FETCHERS: usize = 4;
/// Level of a sampled engine at full gain (the loops are peak-normalised).
const ENGINE_SAMPLE_LEVEL: f32 = 0.55;
/// Level of the weather beds at full wind / storm.
const WIND_LEVEL: f32 = 0.5;
const STORM_LEVEL: f32 = 0.75;
/// Level of a held flamethrower's roar at full gain.
const FLAME_LEVEL: f32 = 0.6;

/// The mixer's bus levels (linear, `0..2`): the Settings sliders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volumes {
    pub master: f32,
    pub sfx: f32,
    pub engine: f32,
    pub ambience: f32,
    pub music: f32,
}

/// A ScriptProcessor and the closure it calls, kept alive together.
type Stream = (ScriptProcessorNode, Closure<dyn FnMut(AudioProcessingEvent)>);

struct Voice {
    src: AudioBufferSourceNode,
    gain: GainNode,
    ends: f64,
}

/// The recordings: fetched bytes waiting for a context, and decoded buffers.
#[derive(Default)]
struct Store {
    queue: VecDeque<Asset>,
    fetched: Vec<(Asset, js_sys::ArrayBuffer)>,
    /// Per sound, per take: the decoded recording.
    shots: Vec<Vec<Option<AudioBuffer>>>,
    beds: Vec<Option<AudioBuffer>>,
    total: usize,
    loaded: usize,
    failed: usize,
    plays: u32,
    flames: u32,
    reported: bool,
    /// Fetches that failed once, and when to try them again.
    retry: Vec<(Asset, f64)>,
    retried: Vec<Asset>,
    /// Sounds with a file that failed for good: they need the synthesizer.
    failed_ids: Vec<SoundId>,
}

impl Store {
    fn new() -> Self {
        let order = load_order();
        Store {
            total: order.len(),
            queue: order.into_iter().collect(),
            shots: SoundId::ALL.iter().map(|id| vec![None; takes(*id).len()]).collect(),
            beds: vec![None; Bed::ALL.len()],
            ..Store::default()
        }
    }

    fn put(&mut self, asset: Asset, buf: AudioBuffer) {
        match asset {
            Asset::Shot(id, k) => self.shots[id.index()][k] = Some(buf),
            Asset::Bed(b) => self.beds[b.index()] = Some(buf),
        }
        self.loaded += 1;
        self.publish();
    }

    /// A fetch failed: try once more later.
    fn fetch_failed(&mut self, asset: Asset, why: &JsValue) {
        if self.retried.contains(&asset) {
            self.fail(asset, why);
        } else {
            self.retried.push(asset);
            self.retry.push((asset, now_ms() + RETRY_MS));
        }
    }

    fn fail(&mut self, asset: Asset, why: &JsValue) {
        self.failed += 1;
        if let Asset::Shot(id, _) = asset {
            self.failed_ids.push(id);
        }
        web_sys::console::warn_3(&"arena-audio: could not load".into(), &asset.url().into(), why);
        self.publish();
    }

    fn bed(&self, b: Bed) -> Option<&AudioBuffer> {
        self.beds[b.index()].as_ref()
    }

    fn engine_ready(&self) -> bool {
        Bed::ENGINE.iter().all(|b| self.bed(*b).is_some())
    }

    /// Mirrors the counts to `window.__tanskAudio` and logs once when done.
    fn publish(&mut self) {
        if let Some(w) = web_sys::window() {
            let o = js_sys::Object::new();
            let set = |k: &str, v: f64| {
                let _ = js_sys::Reflect::set(&o, &k.into(), &v.into());
            };
            set("total", self.total as f64);
            set("loaded", self.loaded as f64);
            set("failed", self.failed as f64);
            set("plays", f64::from(self.plays));
            set("flames", f64::from(self.flames));
            let _ = js_sys::Reflect::set(&w, &"__tanskAudio".into(), &o);
        }
        if !self.reported && self.loaded + self.failed >= self.total {
            self.reported = true;
            log(&format!(
                "arena-audio: {} of {} sound files loaded ({} failed; those sounds fall back to the synthesizer)",
                self.loaded, self.total, self.failed
            ));
        }
    }
}

/// One tank's sampled engine: four looping layers into a filter and panner.
struct EngineVoice {
    srcs: Vec<AudioBufferSourceNode>,
    gains: Vec<GainNode>,
    filter: BiquadFilterNode,
    panner: StereoPannerNode,
    out: GainNode,
    kind: EngineKind,
    variant: f32,
}

/// One held flamethrower: the roar loop through its own filter and panner,
/// or `None` when it is covered by the one-shot fallback (loop not loaded).
struct FlameVoice {
    roar: Option<(AudioBufferSourceNode, GainNode, BiquadFilterNode, StereoPannerNode)>,
    pos: Option<[f32; 3]>,
}

/// The two weather beds.
struct Weather {
    wind: Option<(AudioBufferSourceNode, GainNode)>,
    storm: Option<(AudioBufferSourceNode, GainNode)>,
}

struct Live {
    ctx: AudioContext,
    master: GainNode,
    sfx: GainNode,
    engine_bus: GainNode,
    ambience_bus: GainNode,
    music_bus: GainNode,
    verb_in: Option<GainNode>,
    buffers: Vec<Vec<AudioBuffer>>,
    voices: Vec<Vec<Voice>>,
    last_take: Vec<usize>,
    /// Sounds waiting for a synthesized fallback take, one per frame.
    synth_queue: VecDeque<SoundId>,
    unlocked_at: f64,
    engines: HashMap<EngineHandle, EngineVoice>,
    flames: HashMap<u64, FlameVoice>,
    weather: Weather,
    _streams: Vec<Stream>,
    // Kept alive with the graph.
    _verb: Option<(ConvolverNode, GainNode)>,
}

/// The game's audio. Create it at startup (the recordings start
/// downloading then); call [`Audio::unlock`] from the first click or key
/// press (browsers keep audio off until then), and [`Audio::tick`] once per
/// frame. Every call before unlock is a quiet no-op, except that engines and
/// settings are remembered.
pub struct Audio {
    live: Option<Live>,
    volumes: Volumes,
    listener: Listener,
    engines: Rc<RefCell<EngineMixer>>,
    engine_meta: HashMap<EngineHandle, (EngineKind, u64)>,
    ambience: Rc<RefCell<Ambience>>,
    ambience_wanted: (f32, f32),
    music: Rc<RefCell<Option<Music>>>,
    music_wanted: (MusicTrack, f32),
    store: Rc<RefCell<Store>>,
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

/// Fetches `url` as bytes; `Err` on a network error or a non-2xx status.
async fn fetch_bytes(url: String) -> Result<js_sys::ArrayBuffer, JsValue> {
    let w = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let resp: web_sys::Response = JsFuture::from(w.fetch_with_str(&url)).await?.dyn_into()?;
    if !resp.ok() {
        return Err(JsValue::from_str(&format!("HTTP {}", resp.status())));
    }
    JsFuture::from(resp.array_buffer()?).await?.dyn_into()
}

/// One of the [`FETCHERS`]: takes files off the queue until it is empty.
async fn fetcher(store: Rc<RefCell<Store>>) {
    loop {
        let Some(asset) = store.borrow_mut().queue.pop_front() else { return };
        match fetch_bytes(asset.url()).await {
            Ok(bytes) => store.borrow_mut().fetched.push((asset, bytes)),
            Err(e) => store.borrow_mut().fetch_failed(asset, &e),
        }
    }
}

impl Audio {
    #[must_use]
    pub fn new() -> Self {
        let store = Rc::new(RefCell::new(Store::new()));
        store.borrow_mut().publish();
        for _ in 0..FETCHERS {
            spawn_local(fetcher(store.clone()));
        }
        Self {
            live: None,
            volumes: Volumes { master: 0.8, sfx: 1.0, engine: 0.7, ambience: 0.6, music: 0.55 },
            listener: Listener::default(),
            engines: Rc::new(RefCell::new(EngineMixer::new(SR))),
            engine_meta: HashMap::new(),
            ambience: Rc::new(RefCell::new(Ambience::new(1, SR))),
            ambience_wanted: (0.0, 0.0),
            music: Rc::new(RefCell::new(None)),
            music_wanted: (MusicTrack::Silence, 0.0),
            store,
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
            Ok(live) => {
                self.live = Some(live);
                self.decode_fetched();
            }
            Err(e) => web_sys::console::warn_2(&"arena-audio: could not start Web Audio".into(), &e),
        }
    }

    fn start(&mut self) -> Result<Live, wasm_bindgen::JsValue> {
        let ctx = AudioContext::new()?;
        let master = ctx.create_gain()?;
        let sfx = ctx.create_gain()?;
        let engine_bus = ctx.create_gain()?;
        let ambience_bus = ctx.create_gain()?;
        let music_bus = ctx.create_gain()?;
        music_bus.gain().set_value(self.volumes.music);
        music_bus.connect_with_audio_node(&master)?;
        master.gain().set_value(self.volumes.master);
        sfx.gain().set_value(self.volumes.sfx);
        engine_bus.gain().set_value(self.volumes.engine);
        ambience_bus.gain().set_value(self.volumes.ambience);
        sfx.connect_with_audio_node(&master)?;
        engine_bus.connect_with_audio_node(&master)?;
        ambience_bus.connect_with_audio_node(&master)?;
        master.connect_with_audio_node(&ctx.destination())?;
        let _ = ctx.resume();

        // The shared reverb: a send bus into a convolver with a synthesized
        // open-air response (a slap off the dunes and a short dark tail).
        let verb = match reverb(&ctx, &sfx, &mut self.rng) {
            Ok(v) => Some(v),
            Err(e) => {
                web_sys::console::warn_2(&"arena-audio: no reverb".into(), &e);
                None
            }
        };
        let verb_in = verb.as_ref().map(|(_, g)| g.clone());

        // Engines and the ambience, each on its own ScriptProcessor and bus.
        let rate = ctx.sample_rate();
        self.engines.borrow_mut().set_sample_rate(rate);
        {
            let mut amb = self.ambience.borrow_mut();
            let (wind, storm) = amb.settings();
            *amb = Ambience::new(self.rng.next_u64(), rate);
            amb.set(wind, storm);
        }
        let mut streams = Vec::new();
        let engines = self.engines.clone();
        match stream(&ctx, &engine_bus, move |l, r| match engines.try_borrow_mut() {
            Ok(mut m) => m.render(l, r),
            Err(_) => {
                l.fill(0.0);
                r.fill(0.0);
            }
        }) {
            Ok(s) => streams.push(s),
            Err(e) => web_sys::console::warn_2(&"arena-audio: no ScriptProcessor; engines are silent".into(), &e),
        }
        let ambience = self.ambience.clone();
        match stream(&ctx, &ambience_bus, move |l, r| match ambience.try_borrow_mut() {
            Ok(mut a) => a.render(l, r),
            Err(_) => {
                l.fill(0.0);
                r.fill(0.0);
            }
        }) {
            Ok(s) => streams.push(s),
            Err(e) => web_sys::console::warn_2(&"arena-audio: no ScriptProcessor; ambience is silent".into(), &e),
        }
        // The band: built here (its drum kit takes a few ms), then streamed.
        {
            let mut band = Music::new(self.rng.next_u64(), rate);
            band.set(self.music_wanted.0, self.music_wanted.1);
            *self.music.borrow_mut() = Some(band);
        }
        let music = self.music.clone();
        match stream(&ctx, &music_bus, move |l, r| match music.try_borrow_mut() {
            Ok(mut m) => match m.as_mut() {
                Some(m) => m.render(l, r),
                None => {
                    l.fill(0.0);
                    r.fill(0.0);
                }
            },
            Err(_) => {
                l.fill(0.0);
                r.fill(0.0);
            }
        }) {
            Ok(s) => streams.push(s),
            Err(e) => web_sys::console::warn_2(&"arena-audio: no ScriptProcessor; music is silent".into(), &e),
        }

        let live = Live {
            ctx,
            master,
            sfx,
            engine_bus,
            ambience_bus,
            music_bus,
            verb_in,
            buffers: vec![Vec::new(); SoundId::ALL.len()],
            voices: (0..SoundId::ALL.len()).map(|_| Vec::new()).collect(),
            last_take: vec![usize::MAX; SoundId::ALL.len()],
            synth_queue: VecDeque::new(),
            unlocked_at: now_ms(),
            engines: HashMap::new(),
            flames: HashMap::new(),
            weather: Weather { wind: None, storm: None },
            _streams: streams,
            _verb: verb,
        };
        // Nothing is synthesized up front: every sound is a file (fetched
        // since startup, decoded off the main thread from now on), and a
        // take is synthesized only for a sound whose file failed.
        log(&format!("arena-audio: started ({rate} Hz output)"));
        Ok(live)
    }

    /// Hands a few fetched files to the decoder (needs the context); the
    /// decoding itself runs off the main thread and lands asynchronously.
    fn decode_fetched(&mut self) {
        let Some(live) = &self.live else { return };
        let fetched: Vec<_> = {
            let mut st = self.store.borrow_mut();
            let n = st.fetched.len().min(DECODES_PER_TICK);
            st.fetched.drain(..n).collect()
        };
        for (asset, bytes) in fetched {
            let store = self.store.clone();
            match live.ctx.decode_audio_data(&bytes) {
                Ok(promise) => spawn_local(async move {
                    match JsFuture::from(promise).await.and_then(|b| b.dyn_into::<AudioBuffer>()) {
                        Ok(buf) => store.borrow_mut().put(asset, buf),
                        Err(e) => store.borrow_mut().fail(asset, &e),
                    }
                }),
                Err(e) => store.borrow_mut().fail(asset, &e),
            }
        }
    }

    /// Call once per frame: retries failed fetches, decodes a few newly
    /// fetched files, starts the weather beds once they are in, and renders
    /// at most one synthesized fallback take (only ever for a sound whose
    /// file failed or is very late, so normally never).
    pub fn tick(&mut self) {
        let now = now_ms();
        let due = {
            let mut st = self.store.borrow_mut();
            let due: Vec<Asset> = st.retry.iter().filter(|(_, t)| *t <= now).map(|(a, _)| *a).collect();
            st.retry.retain(|(_, t)| *t > now);
            st.queue.extend(due.iter().copied());
            !due.is_empty()
        };
        if due {
            spawn_local(fetcher(self.store.clone()));
        }
        if self.live.is_none() {
            return;
        }
        self.decode_fetched();
        self.update_weather();
        let Some(live) = &mut self.live else { return };
        let failed = std::mem::take(&mut self.store.borrow_mut().failed_ids);
        for id in failed {
            live.want_synth(id);
        }
        if let Some(id) = live.synth_queue.pop_front() {
            if !self.store.borrow().shots[id.index()].iter().any(Option::is_some) {
                let _ = live.render_synth(id);
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
        let mut store = self.store.borrow_mut();
        if !store.shots[id.index()].iter().any(Option::is_some) && now_ms() - live.unlocked_at > SLOW_LOAD_MS {
            live.want_synth(id);
        }
        if live.play(id, level, &heard, pos.is_some(), rate, &store, &mut self.rng).unwrap_or(false) {
            store.plays += 1;
            if store.plays % 16 == 1 {
                store.publish();
            }
        }
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

    pub fn set_ambience_volume(&mut self, v: f32) {
        self.volumes.ambience = clean(v);
        if let Some(l) = &self.live {
            ramp(&l.ctx, &l.ambience_bus, self.volumes.ambience);
        }
    }

    /// The music bus: the Settings "Music" slider.
    pub fn set_music_volume(&mut self, v: f32) {
        self.volumes.music = clean(v);
        if let Some(l) = &self.live {
            ramp(&l.ctx, &l.music_bus, self.volumes.music);
        }
    }

    /// Every bus level at once (e.g. restored from saved Settings).
    pub fn set_volumes(&mut self, v: Volumes) {
        self.set_master_volume(v.master);
        self.set_sfx_volume(v.sfx);
        self.set_engine_volume(v.engine);
        self.set_ambience_volume(v.ambience);
        self.set_music_volume(v.music);
    }

    /// The current bus levels, for a Settings screen.
    #[must_use]
    pub fn volumes(&self) -> Volumes {
        self.volumes
    }

    /// How many recorded files are loaded, failed, and wanted in all.
    #[must_use]
    pub fn recordings(&self) -> (usize, usize, usize) {
        let s = self.store.borrow();
        (s.loaded, s.failed, s.total)
    }

    /// Chooses the music: `MusicTrack::Title`, `Battle` or `Silence`, and
    /// the battle's `intensity` (`0` calm .. `1` full band; glides). Cheap
    /// to call every frame. Before unlock it is remembered.
    pub fn music_set(&mut self, track: MusicTrack, intensity: f32) {
        self.music_wanted = (track, intensity);
        if let Ok(mut m) = self.music.try_borrow_mut() {
            if let Some(m) = m.as_mut() {
                m.set(track, intensity);
            }
        }
    }

    /// Plays a stinger over the music (which ducks under it).
    pub fn music_stinger(&mut self, s: Stinger) {
        if let Ok(mut m) = self.music.try_borrow_mut() {
            if let Some(m) = m.as_mut() {
                m.stinger(s);
            }
        }
    }

    /// The weather: `wind` is the everyday desert wind (`0` still .. `1`
    /// blustery), `storm` the sandstorm layer (`0..1`). Glides over about
    /// half a second, so it can be set every frame. Both `0` is silence.
    pub fn ambience_set(&mut self, wind: f32, storm: f32) {
        let clean01 = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
        self.ambience_wanted = (clean01(wind), clean01(storm));
        self.update_weather();
    }

    /// Starts any weather bed that has loaded and sets both levels; the
    /// synthesized wind covers whichever bed is missing.
    fn update_weather(&mut self) {
        let (wind, storm) = self.ambience_wanted;
        let mut synth = (wind, storm);
        if let Some(live) = &mut self.live {
            let store = self.store.borrow();
            let now = live.ctx.current_time();
            for (bed, slot, level) in [
                (Bed::Wind, &mut live.weather.wind, wind.powf(0.8) * WIND_LEVEL * (1.0 - 0.5 * storm)),
                (Bed::Storm, &mut live.weather.storm, storm * STORM_LEVEL),
            ] {
                if slot.is_none() {
                    if let Some(buf) = store.bed(bed) {
                        *slot = start_loop(&live.ctx, buf, &live.ambience_bus, &mut self.rng).ok();
                    }
                }
                if let Some((_, g)) = slot {
                    let _ = g.gain().set_target_at_time(level, now, 0.25);
                    if bed == Bed::Wind {
                        synth.0 = 0.0;
                    } else {
                        synth.1 = 0.0;
                    }
                }
            }
        }
        if let Ok(mut a) = self.ambience.try_borrow_mut() {
            a.set(synth.0, synth.1);
        }
    }

    /// Every frame: the flamethrowers that are lit, by a stable key (e.g. the
    /// player id) and where they are (`None` for your own). A new one plays
    /// its ignition and starts a seamless roar loop; one that is gone stops
    /// its loop and plays the cut-off tail. Until the loop has loaded, a lit
    /// flamethrower plays the one-shot burst instead.
    pub fn flames(&mut self, lit: &[(u64, Option<[f32; 3]>)]) {
        let Some(live) = &self.live else { return };
        let gone: Vec<u64> = live.flames.keys().copied().filter(|k| !lit.iter().any(|(l, _)| l == k)).collect();
        for k in gone {
            let Some(v) = self.live.as_mut().and_then(|l| l.flames.remove(&k)) else { continue };
            if let (Some(live), Some((src, g, _, _))) = (&self.live, &v.roar) {
                let now = live.ctx.current_time();
                let _ = g.gain().set_target_at_time(0.0, now, 0.06);
                let _ = web_sys::AudioScheduledSourceNode::stop_with_when(src, now + 0.4);
            }
            if v.roar.is_some() {
                self.play(SoundId::FlameTail, v.pos, 1.0);
            }
        }
        for &(k, pos) in lit {
            let heard = pos.map_or(CENTRE, |p| spatialize(&self.listener, p));
            let fresh = !self.live.as_ref().is_some_and(|l| l.flames.contains_key(&k));
            if fresh {
                let roar = {
                    let store = self.store.borrow();
                    match (store.bed(Bed::FlameLoop), &self.live) {
                        (Some(buf), Some(live)) => flame_roar(live, buf, &mut self.rng).ok(),
                        _ => None,
                    }
                };
                if roar.is_some() {
                    self.play(SoundId::FlameStart, pos, 1.0);
                } else {
                    self.play(SoundId::Flamethrower, pos, 0.9);
                }
                if let Some(live) = &mut self.live {
                    live.flames.insert(k, FlameVoice { roar, pos });
                }
                let mut store = self.store.borrow_mut();
                store.flames += 1;
                store.publish();
            }
            let Some(live) = &mut self.live else { return };
            let Some(v) = live.flames.get_mut(&k) else { continue };
            v.pos = pos;
            if let Some((_, g, filter, panner)) = &v.roar {
                let now = live.ctx.current_time();
                let level = (heard.gain * FLAME_LEVEL).clamp(0.0, 2.0);
                let level = if level.is_finite() { level } else { 0.0 };
                // Fades in under the ignition burst.
                let _ = g.gain().set_target_at_time(level, now, if fresh { 0.15 } else { 0.05 });
                let _ = filter.frequency().set_target_at_time(heard.cutoff_hz.clamp(200.0, 20_000.0), now, 0.05);
                let _ = panner.pan().set_target_at_time(heard.pan.clamp(-1.0, 1.0), now, 0.05);
            }
        }
    }

    /// Plays Garage horn `index` (`0..6`, wrapping) from a tank at `pos`
    /// (`None` for your own).
    pub fn horn(&mut self, index: usize, pos: Option<[f32; 3]>, gain: f32) {
        self.play(SoundId::horn(index), pos, gain);
    }

    /// Starts an engine voice for one visible tank. `seed` (e.g. the tank's
    /// id) gives it its own character. `None` when all engine slots are busy.
    pub fn engine_add(&mut self, kind: EngineKind, seed: u64) -> Option<EngineHandle> {
        let h = self.engines.borrow_mut().add(kind, seed)?;
        self.engine_meta.insert(h, (kind, seed));
        Some(h)
    }

    /// Every frame for every engine: what it is doing and where it is.
    /// `gain` is its own level (1 normal).
    pub fn engine_set(&mut self, h: EngineHandle, pos: [f32; 3], input: EngineInput, gain: f32) {
        let heard = spatialize(&self.listener, pos);
        self.engine_update(h, input, heard, gain);
    }

    /// [`Audio::engine_set`] for the player's own tank: centred, full level.
    pub fn engine_set_own(&mut self, h: EngineHandle, input: EngineInput, gain: f32) {
        self.engine_update(h, input, CENTRE, gain);
    }

    fn engine_update(&mut self, h: EngineHandle, input: EngineInput, heard: Spatial, gain: f32) {
        let sampled = match (&mut self.live, self.engine_meta.get(&h)) {
            (Some(live), Some(&(kind, seed))) if self.store.borrow().engine_ready() => {
                if !live.engines.contains_key(&h) {
                    if let Ok(v) = live.engine_voice(kind, seed, &self.store.borrow(), &mut self.rng) {
                        live.engines.insert(h, v);
                    }
                }
                match live.engines.get(&h) {
                    Some(v) => {
                        v.set(&live.ctx, input, &heard, gain);
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        };
        if let Ok(mut m) = self.engines.try_borrow_mut() {
            // A sampled engine silences its synthesized twin (which then
            // stops being rendered).
            let g = if sampled { 0.0 } else { gain };
            m.set(h, input, heard, g);
        }
    }

    /// Fades an engine out (tank destroyed or out of sight) and frees it.
    pub fn engine_remove(&mut self, h: EngineHandle) {
        if let Ok(mut m) = self.engines.try_borrow_mut() {
            m.remove(h);
        }
        self.engine_meta.remove(&h);
        if let Some(live) = &mut self.live {
            if let Some(v) = live.engines.remove(&h) {
                v.stop(&live.ctx);
            }
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
    /// Queues a synthesized take of `id`, once.
    fn want_synth(&mut self, id: SoundId) {
        if self.buffers[id.index()].is_empty() && !self.synth_queue.contains(&id) {
            self.synth_queue.push_back(id);
        }
    }

    /// Synthesizes one take of `id`: the fallback for a failed file.
    fn render_synth(&mut self, id: SoundId) -> Result<(), wasm_bindgen::JsValue> {
        let k = self.buffers[id.index()].len();
        let take = render(id, take_seed(k));
        let frames = take.frames().max(1) as u32;
        let buf = self.ctx.create_buffer(take.channels.len() as u32, frames, SR)?;
        for (c, data) in take.channels.iter().enumerate() {
            buf.copy_to_channel(data, c as i32)?;
        }
        self.buffers[id.index()].push(buf);
        Ok(())
    }

    /// Picks a take of `id`: a loaded recording if there is one (with its
    /// level trim), else a synthesized take. Never the same take twice
    /// running when there is a choice.
    fn pick(&mut self, id: SoundId, store: &Store, rng: &mut Rng) -> Option<(AudioBuffer, f32, bool)> {
        let recorded: Vec<(usize, &AudioBuffer)> =
            store.shots[id.index()].iter().enumerate().filter_map(|(k, b)| b.as_ref().map(|b| (k, b))).collect();
        let last = self.last_take[id.index()];
        if !recorded.is_empty() {
            let mut i = (rng.unit() * recorded.len() as f32) as usize % recorded.len();
            if recorded.len() > 1 && recorded[i].0 == last {
                i = (i + 1) % recorded.len();
            }
            let (k, buf) = recorded[i];
            self.last_take[id.index()] = k;
            return Some((buf.clone(), takes(id)[k].trim, true));
        }
        let synth = &self.buffers[id.index()];
        if synth.is_empty() {
            return None;
        }
        let mut k = (rng.unit() * synth.len() as f32) as usize % synth.len();
        if synth.len() > 1 && k == last {
            k = (k + 1) % synth.len();
        }
        self.last_take[id.index()] = k;
        Some((synth[k].clone(), 1.0, false))
    }

    /// Starts one copy of `id`; `Ok(true)` when something started.
    #[allow(clippy::too_many_arguments)]
    fn play(
        &mut self,
        id: SoundId,
        level: f32,
        heard: &Spatial,
        positional: bool,
        rate: f32,
        store: &Store,
        rng: &mut Rng,
    ) -> Result<bool, wasm_bindgen::JsValue> {
        let Some((buffer, trim, recorded)) = self.pick(id, store, rng) else { return Ok(false) };
        let info = id.info();
        let now = self.ctx.current_time();
        // Voice limits: drop finished voices, then steal the oldest.
        for vs in &mut self.voices {
            vs.retain(|v| v.ends > now);
        }
        let total: usize = self.voices.iter().map(Vec::len).sum();
        let voices = &mut self.voices[id.index()];
        if voices.len() >= usize::from(info.voices.max(1)) || total >= MAX_ONE_SHOTS {
            if voices.is_empty() {
                return Ok(false);
            }
            let v = voices.remove(0);
            fade_out(&v, now);
        }

        let src = self.ctx.create_buffer_source()?;
        src.set_buffer(Some(&buffer));
        // Recordings get a little more pitch spread than the synthesized
        // takes (which already vary by seed), and a random ±1.5 dB.
        let jitter = if recorded && info.pitch_jitter > 0.0 { info.pitch_jitter.max(0.035) } else { info.pitch_jitter };
        let rate = (rate * rng.vary(jitter)).clamp(0.25, 4.0);
        src.playback_rate().set_value(rate);
        let level = if recorded { level * trim * info.peak * 10f32.powf(rng.range(-1.5, 1.5) / 20.0) } else { level };
        let gain = self.ctx.create_gain()?;
        gain.gain().set_value(level.min(4.0));
        src.connect_with_audio_node(&gain)?;
        if positional {
            let filter = self.ctx.create_biquad_filter()?;
            filter.set_type(BiquadFilterType::Lowpass);
            filter.frequency().set_value(heard.cutoff_hz.clamp(200.0, 20_000.0));
            filter.q().set_value(0.5);
            let panner = self.ctx.create_stereo_panner()?;
            panner.pan().set_value(heard.pan.clamp(-1.0, 1.0));
            gain.connect_with_audio_node(&filter)?;
            filter.connect_with_audio_node(&panner)?;
            panner.connect_with_audio_node(&self.sfx)?;
        } else {
            gain.connect_with_audio_node(&self.sfx)?;
        }
        // The reverb send: more of the room the further off it is; a dry
        // interface.
        if let Some(verb) = &self.verb_in {
            let far = (heard.distance / MAX_DISTANCE).clamp(0.0, 1.0);
            let send = if positional {
                0.12 + 0.55 * far.sqrt()
            } else if info.positional {
                0.08
            } else {
                0.0
            };
            if send > 0.0 {
                let s = self.ctx.create_gain()?;
                s.gain().set_value(send);
                gain.connect_with_audio_node(&s)?;
                s.connect_with_audio_node(verb)?;
            }
        }
        src.start()?;
        let ends = now + buffer.duration() / f64::from(rate) + 0.05;
        self.voices[id.index()].push(Voice { src, gain, ends });
        Ok(true)
    }

    /// Builds a sampled engine voice: the four loops, each from a random
    /// point so two tanks never run in phase.
    fn engine_voice(&self, kind: EngineKind, seed: u64, store: &Store, rng: &mut Rng) -> Result<EngineVoice, JsValue> {
        let out = self.ctx.create_gain()?;
        out.gain().set_value(0.0);
        let filter = self.ctx.create_biquad_filter()?;
        filter.set_type(BiquadFilterType::Lowpass);
        filter.q().set_value(0.5);
        let panner = self.ctx.create_stereo_panner()?;
        filter.connect_with_audio_node(&panner)?;
        panner.connect_with_audio_node(&out)?;
        out.connect_with_audio_node(&self.engine_bus)?;
        let mut srcs = Vec::new();
        let mut gains = Vec::new();
        for bed in Bed::ENGINE {
            let buf = store.bed(bed).ok_or_else(|| JsValue::from_str("engine loop missing"))?;
            let src = self.ctx.create_buffer_source()?;
            src.set_buffer(Some(buf));
            src.set_loop(true);
            let g = self.ctx.create_gain()?;
            g.gain().set_value(0.0);
            src.connect_with_audio_node(&g)?;
            g.connect_with_audio_node(&filter)?;
            src.start_with_when_and_grain_offset(0.0, f64::from(rng.unit()) * buf.duration())?;
            srcs.push(src);
            gains.push(g);
        }
        let variant = (seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f32 / (1u64 << 24) as f32;
        Ok(EngineVoice { srcs, gains, filter, panner, out, kind, variant })
    }
}

impl EngineVoice {
    fn set(&self, ctx: &AudioContext, input: EngineInput, heard: &Spatial, gain: f32) {
        let now = ctx.current_time();
        let mix = engine_mix(self.kind, input, self.variant);
        for (i, (src, g)) in self.srcs.iter().zip(&self.gains).enumerate() {
            let _ = g.gain().set_target_at_time(mix.gain[i], now, 0.06);
            let _ = src.playback_rate().set_target_at_time(mix.rate[i].clamp(0.3, 3.0), now, 0.08);
        }
        let level = (heard.gain * gain * ENGINE_SAMPLE_LEVEL).clamp(0.0, 4.0);
        let level = if level.is_finite() { level } else { 0.0 };
        let _ = self.out.gain().set_target_at_time(level, now, 0.05);
        let _ = self.filter.frequency().set_target_at_time(heard.cutoff_hz.clamp(200.0, 20_000.0), now, 0.05);
        let _ = self.panner.pan().set_target_at_time(heard.pan.clamp(-1.0, 1.0), now, 0.05);
    }

    fn stop(&self, ctx: &AudioContext) {
        let now = ctx.current_time();
        let _ = self.out.gain().set_target_at_time(0.0, now, 0.08);
        for s in &self.srcs {
            let _ = web_sys::AudioScheduledSourceNode::stop_with_when(s, now + 0.5);
        }
    }
}

/// A flamethrower's roar: the loop from a random point, silent, through a
/// low-pass and a panner into the sfx bus.
fn flame_roar(
    live: &Live,
    buf: &AudioBuffer,
    rng: &mut Rng,
) -> Result<(AudioBufferSourceNode, GainNode, BiquadFilterNode, StereoPannerNode), JsValue> {
    let ctx = &live.ctx;
    let src = ctx.create_buffer_source()?;
    src.set_buffer(Some(buf));
    src.set_loop(true);
    src.playback_rate().set_value(rng.vary(0.04));
    let g = ctx.create_gain()?;
    g.gain().set_value(0.0);
    let filter = ctx.create_biquad_filter()?;
    filter.set_type(BiquadFilterType::Lowpass);
    filter.q().set_value(0.5);
    let panner = ctx.create_stereo_panner()?;
    src.connect_with_audio_node(&g)?;
    g.connect_with_audio_node(&filter)?;
    filter.connect_with_audio_node(&panner)?;
    panner.connect_with_audio_node(&live.sfx)?;
    src.start_with_when_and_grain_offset(0.0, f64::from(rng.unit()) * buf.duration())?;
    Ok((src, g, filter, panner))
}

/// A looping bed from a random point, through its own gain (starting
/// silent) into `dest`.
fn start_loop(
    ctx: &AudioContext,
    buf: &AudioBuffer,
    dest: &GainNode,
    rng: &mut Rng,
) -> Result<(AudioBufferSourceNode, GainNode), JsValue> {
    let src = ctx.create_buffer_source()?;
    src.set_buffer(Some(buf));
    src.set_loop(true);
    let g = ctx.create_gain()?;
    g.gain().set_value(0.0);
    src.connect_with_audio_node(&g)?;
    g.connect_with_audio_node(dest)?;
    src.start_with_when_and_grain_offset(0.0, f64::from(rng.unit()) * buf.duration())?;
    Ok((src, g))
}

/// The open-air reverb: an impulse response of decaying, darkening noise
/// with two early slaps (the nearest dunes), returned as (convolver, send
/// input). The wet signal goes into `dest` (the sfx bus).
fn reverb(ctx: &AudioContext, dest: &GainNode, rng: &mut Rng) -> Result<(ConvolverNode, GainNode), JsValue> {
    let rate = ctx.sample_rate();
    let len = (1.6 * rate) as usize;
    let ir = ctx.create_buffer(2, len as u32, rate)?;
    for c in 0..2 {
        let mut data = vec![0.0f32; len];
        let mut lp = 0.0f32;
        for (i, s) in data.iter_mut().enumerate() {
            let t = i as f32 / rate;
            if t < 0.012 {
                continue;
            }
            // Darker as it decays: the low-pass closes over the tail.
            let k = 0.55 - 0.4 * (t / 1.6).min(1.0);
            lp += k * (rng.bi() - lp);
            *s = lp * (-t / 0.38).exp();
        }
        for (at, g) in [(0.085 + 0.01 * c as f32, 0.6), (0.21 + 0.015 * c as f32, 0.35)] {
            let i0 = (at * rate) as usize;
            for j in 0..(0.012 * rate) as usize {
                if let Some(s) = data.get_mut(i0 + j) {
                    *s += g * rng.bi() * (1.0 - j as f32 / (0.012 * rate));
                }
            }
        }
        ir.copy_to_channel(&data, c)?;
    }
    let conv = ctx.create_convolver()?;
    conv.set_buffer(Some(&ir));
    let send = ctx.create_gain()?;
    send.gain().set_value(1.0);
    let wet = ctx.create_gain()?;
    wet.gain().set_value(0.35);
    send.connect_with_audio_node(&conv)?;
    conv.connect_with_audio_node(&wet)?;
    wet.connect_with_audio_node(dest)?;
    Ok((conv, send))
}

/// Starts a stereo ScriptProcessor feeding `dest`, filled by `fill(left,
/// right)` on every callback.
fn stream(
    ctx: &AudioContext,
    dest: &GainNode,
    mut fill: impl FnMut(&mut [f32], &mut [f32]) + 'static,
) -> Result<Stream, wasm_bindgen::JsValue> {
    let node = ctx
        .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
            ENGINE_BLOCK,
            1,
            2,
        )?;
    let mut left = vec![0.0f32; ENGINE_BLOCK as usize];
    let mut right = vec![0.0f32; ENGINE_BLOCK as usize];
    let cb = Closure::<dyn FnMut(AudioProcessingEvent)>::new(move |e: AudioProcessingEvent| {
        let Ok(out) = e.output_buffer() else { return };
        let n = out.length() as usize;
        left.resize(n, 0.0);
        right.resize(n, 0.0);
        fill(&mut left, &mut right);
        let _ = out.copy_to_channel(&left, 0);
        if out.number_of_channels() > 1 {
            let _ = out.copy_to_channel(&right, 1);
        }
    });
    node.set_onaudioprocess(Some(cb.as_ref().unchecked_ref()));
    node.connect_with_audio_node(dest)?;
    Ok((node, cb))
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
