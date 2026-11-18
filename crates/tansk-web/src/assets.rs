//! Sprites (ImageHandler) and sound (SoundHandler): Web Audio for effects,
//! streamed `<audio>` elements for the two music tracks.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use js_sys::{ArrayBuffer, Promise};
use tansk_sim::proto::Sound;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{AudioBuffer, AudioContext, GainNode, HtmlAudioElement, HtmlImageElement, Response};

#[derive(Default)]
pub struct Images {
    map: RefCell<HashMap<String, HtmlImageElement>>,
}

impl Images {
    /// The sprite `images/<name>.png`, once it has loaded.
    pub fn get(&self, name: &str) -> Option<HtmlImageElement> {
        let mut map = self.map.borrow_mut();
        let img = map.entry(name.to_string()).or_insert_with(|| {
            let img = HtmlImageElement::new().expect("img");
            img.set_src(&format!("images/{name}.png"));
            img
        });
        (img.complete() && img.natural_width() > 0).then(|| img.clone())
    }

    pub fn preload(&self, names: &[&str]) {
        for n in names {
            let _ = self.get(n);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Music {
    Menu,
    Battle,
}

pub struct Audio {
    ctx: Option<AudioContext>,
    gain: Option<GainNode>,
    buffers: Rc<RefCell<HashMap<&'static str, AudioBuffer>>>,
    menu: HtmlAudioElement,
    battle: HtmlAudioElement,
    playing: Option<Music>,
    /// 0..1, the original's default 0.35.
    pub volume: f32,
    stored_volume: f32,
    unlocked: bool,
}

impl Audio {
    pub fn new(volume: f32) -> Self {
        let track = |src: &str| {
            let a = HtmlAudioElement::new_with_src(src).expect("audio");
            a.set_loop(true);
            a.set_preload("none");
            a
        };
        Audio {
            ctx: None,
            gain: None,
            buffers: Rc::new(RefCell::new(HashMap::new())),
            menu: track("music/Tensions.mp3"),
            battle: track("music/Battle_Music.mp3"),
            playing: None,
            volume,
            stored_volume: if volume > 0.0 { volume } else { 0.35 },
            unlocked: false,
        }
    }

    /// Browsers allow sound only after the page is clicked or a key pressed.
    pub fn unlock(&mut self) {
        if self.unlocked {
            return;
        }
        let Ok(ctx) = AudioContext::new() else { return };
        let Ok(gain) = ctx.create_gain() else { return };
        let _ = gain.connect_with_audio_node(&ctx.destination());
        gain.gain().set_value(self.volume);
        let _ = ctx.resume();
        for s in Sound::ALL {
            load_sound(ctx.clone(), s.file(), self.buffers.clone());
        }
        self.ctx = Some(ctx);
        self.gain = Some(gain);
        self.unlocked = true;
        if let Some(m) = self.playing.take() {
            self.play_music(m);
        }
    }

    pub fn play(&self, sound: Sound) {
        let (Some(ctx), Some(gain)) = (&self.ctx, &self.gain) else { return };
        if self.volume <= 0.0 {
            return;
        }
        let Some(buf) = self.buffers.borrow().get(sound.file()).cloned() else { return };
        let Ok(src) = ctx.create_buffer_source() else { return };
        src.set_buffer(Some(&buf));
        let _ = src.connect_with_audio_node(gain);
        let _ = src.start();
    }

    pub fn play_music(&mut self, m: Music) {
        if self.playing == Some(m) {
            return;
        }
        self.stop_music();
        self.playing = Some(m);
        if !self.unlocked {
            return;
        }
        let el = self.track(m);
        el.set_volume(self.volume as f64);
        let _ = el.play();
    }

    pub fn stop_music(&mut self) {
        let _ = self.menu.pause();
        let _ = self.battle.pause();
        self.menu.set_current_time(0.0);
        self.battle.set_current_time(0.0);
        self.playing = None;
    }

    fn track(&self, m: Music) -> &HtmlAudioElement {
        match m {
            Music::Menu => &self.menu,
            Music::Battle => &self.battle,
        }
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
        if self.volume > 0.0 {
            self.stored_volume = self.volume;
        }
        if let Some(g) = &self.gain {
            g.gain().set_value(self.volume);
        }
        self.menu.set_volume(self.volume as f64);
        self.battle.set_volume(self.volume as f64);
    }

    /// SoundSwitch: mute, or back to the volume before muting.
    pub fn toggle_mute(&mut self) {
        if self.volume > 0.0 {
            let keep = self.volume;
            self.set_volume(0.0);
            self.stored_volume = keep;
        } else {
            self.set_volume(self.stored_volume);
        }
    }

    /// SoundSwitch's icon for the current volume.
    pub fn icon(&self) -> &'static str {
        if self.volume <= 0.0 {
            "speaker_off"
        } else if self.volume > 0.75 {
            "speaker_on_3"
        } else if self.volume > 0.4 {
            "speaker_on_2"
        } else {
            "speaker_on_1"
        }
    }
}

fn load_sound(ctx: AudioContext, name: &'static str, into: Rc<RefCell<HashMap<&'static str, AudioBuffer>>>) {
    spawn_local(async move {
        let result: Result<(), JsValue> = async {
            let window = web_sys::window().unwrap();
            let resp: Response = JsFuture::from(window.fetch_with_str(&format!("sounds/{name}.wav"))).await?.dyn_into()?;
            let data: ArrayBuffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;
            let promise: Promise = ctx.decode_audio_data(&data)?;
            let buf: AudioBuffer = JsFuture::from(promise).await?.dyn_into()?;
            into.borrow_mut().insert(name, buf);
            Ok(())
        }
        .await;
        if let Err(e) = result {
            web_sys::console::warn_2(&JsValue::from_str(&format!("sound {name} failed")), &e);
        }
    });
}
