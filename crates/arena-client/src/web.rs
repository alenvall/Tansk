//! Web entry: WebGPU into `#tansk-canvas`, driven by requestAnimationFrame,
//! with DOM input (pointer lock on the canvas), the HTML UI and Web Audio.
//!
//! Debug URL flags: `?autoplay=practice|raid|play|host-raid` starts at once, `&mouse=free`
//! skips the pointer lock (the mouse always turns the camera; Esc pauses),
//! `&name=X` sets the crew name. Practice test hooks: `&matchtime=45`,
//! `&raidfast=1`, `&scrap=1` (Powder until a branch offer), `&flare=1`,
//! `&wave=N`, `&boss=1|weak`, `&landship=1`; `autoplay=team|host-team|convoy|host-convoy` for Team Brawl / Convoy,
//! `autoplay=host-brawl` hosts a private Scrap Brawl,
//! `autoplay=wellhead|host-wellhead` and `autoplay=lastcrew|host-lastcrew` for Wellhead / Last Crew Rolling
//! (`&wallfast=1` makes Last Crew practice's wall close at once and fast).
//! `&quality=low|medium|high|ultra` forces a preset (profiling); `&hints=reset`
//! shows the onboarding tips again. The canvas
//! carries `data-cpu`: mean/max CPU ms per frame (game, draw) over 60 frames.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::{anyhow, Result};
use arena_core::net::Join;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::game::{Game, Screen};
use crate::renderer::{request_device, Renderer};
use crate::ui::{Quality, Settings};
use crate::world::WorldRenderer;

struct ConsoleLog;

impl log::Log for ConsoleLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info || !m.target().starts_with("wgpu")
    }
    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!("[{}] {}: {}", r.level(), r.target(), r.args());
        match r.level() {
            log::Level::Error => web_sys::console::error_1(&line.into()),
            log::Level::Warn => web_sys::console::warn_1(&line.into()),
            _ => web_sys::console::log_1(&line.into()),
        }
    }
    fn flush(&self) {}
}

static LOGGER: ConsoleLog = ConsoleLog;

pub fn main() {
    console_error_panic_hook::set_once();
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info));
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = run().await {
            log::error!("{e:#}");
            show_message(&format!("{e:#}"));
        }
    });
}

/// Replaces the canvas with a visible explanation.
fn show_message(text: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    if let Some(el) = document.get_element_by_id("tansk-message") {
        let escaped = text.replace('&', "&amp;").replace('<', "&lt;");
        el.set_inner_html(&format!("<div><strong>TANSK could not start</strong>{escaped}</div>"));
        if let Ok(html) = el.dyn_into::<web_sys::HtmlElement>() {
            let _ = html.style().set_property("display", "flex");
        }
    }
}

fn canvas_size(canvas: &web_sys::HtmlCanvasElement) -> (u32, u32) {
    let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0);
    let w = (f64::from(canvas.client_width()) * dpr).round().max(1.0) as u32;
    let h = (f64::from(canvas.client_height()) * dpr).round().max(1.0) as u32;
    // WebGPU's guaranteed texture limit.
    (w.min(8192), h.min(8192))
}

struct State {
    canvas: web_sys::HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    wr: WorldRenderer,
    game: Game,
    pause: Option<web_sys::Element>,
    rooms: Rc<RefCell<Option<Vec<RoomRow>>>>,
    start: f64,
    last: f64,
    frames: u32,
    /// CPU time per frame, ms: (sum, max, game sum, draw sum) since the last report.
    cpu: (f64, f64, f64, f64),
    /// Sums of `WorldRenderer::prof` since the last report.
    prof: [f64; 8],
    /// The last few frames over 8 ms CPU, with their breakdown (`data-spike`).
    spikes: std::collections::VecDeque<String>,
}

type Shared = Rc<RefCell<State>>;

fn query_flag(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == name).then(|| js_sys::decode_uri_component(v).ok().and_then(|s| s.as_string()).unwrap_or_default())
    })
}

async fn run() -> Result<()> {
    let window = web_sys::window().ok_or_else(|| anyhow!("no window"))?;
    let document = window.document().ok_or_else(|| anyhow!("no document"))?;
    let canvas = document
        .get_element_by_id("tansk-canvas")
        .ok_or_else(|| anyhow!("no #tansk-canvas element"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| anyhow!("#tansk-canvas is not a canvas"))?;

    let gpu = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("gpu")).unwrap_or(JsValue::UNDEFINED);
    if gpu.is_undefined() || gpu.is_null() {
        return Err(anyhow!(
            "This browser does not expose WebGPU (navigator.gpu is missing). Try a current Chrome, Edge, Safari or Firefox."
        ));
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let (width, height) = canvas_size(&canvas);
    canvas.set_width(width);
    canvas.set_height(height);
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| anyhow!("creating the WebGPU canvas surface: {e}"))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow!("WebGPU is present but no GPU adapter is available ({e}). Hardware acceleration may be off."))?;
    let (device, queue) = request_device(&adapter).await?;
    device.on_uncaptured_error(std::sync::Arc::new(|e: wgpu::Error| {
        log::error!("wgpu: {e}");
    }));
    let mut config = surface
        .get_default_config(&adapter, width, height)
        .ok_or_else(|| anyhow!("the adapter cannot present to the canvas"))?;
    config.present_mode = wgpu::PresentMode::Fifo;
    surface.configure(&device, &config);
    log::info!("web: canvas {}x{} {:?}", width, height, config.format);

    let wr = WorldRenderer::new(Renderer::new(device, queue, config.format, width, height));
    let mut game = Game::new();
    let saved = load_settings();
    // First visit: Medium, and Low if the first seconds turn out slow.
    game.quality_auto = saved.is_none();
    let mut settings = saved.unwrap_or_else(|| Settings { quality: Quality::Medium, ..Settings::default() });
    if let Some(name) = query_flag("name").filter(|n| !n.is_empty()) {
        settings.name = name;
    }
    if let Some(q) = query_flag("quality").filter(|q| !q.is_empty()) {
        settings.quality = Quality::parse(&q);
        game.quality_auto = false;
    }
    game.apply_settings(settings);
    if query_flag("hints").as_deref() == Some("reset") {
        if let Some(st) = storage() {
            let _ = st.remove_item(HINTS_KEY);
        }
    } else if let Some(seen) = storage().and_then(|st| st.get_item(HINTS_KEY).ok().flatten()) {
        game.hints.load(&seen);
    }
    if query_flag("mouse").as_deref() == Some("free") {
        game.free_mouse = true;
        game.controls.captured = true;
    }
    game.debug.match_time = query_flag("matchtime").and_then(|v| v.parse().ok());
    game.debug.raid_fast = query_flag("raidfast").is_some();
    game.debug.scrap = query_flag("scrap").is_some();
    game.debug.flare = query_flag("flare").is_some();
    game.debug.wave = query_flag("wave").and_then(|v| v.parse().ok());
    game.debug.fragile = query_flag("fragile").is_some();
    game.debug.boss = query_flag("boss").map(|v| v == "weak");
    game.debug.landship = query_flag("landship").is_some();
    game.debug.wall_fast = query_flag("wallfast").is_some();
    if let Some(m) = query_flag("map").and_then(|m| arena_core::MapId::from_key(&m)) {
        game.practice_map = m;
        // Show it on the title's map switch too.
        if let Some(el) = document.query_selector(&format!("input[name=pmap][value={}]", m.key())).ok().flatten() {
            let _ = js_sys::Reflect::set(&el, &JsValue::from_str("checked"), &JsValue::TRUE);
        }
    }
    match query_flag("autoplay").as_deref() {
        Some("practice") => game.start_practice(),
        Some("play") => game.start_online(Join::Play),
        Some("raid") => game.start_practice_raid(),
        Some("team") => game.start_practice_team(),
        Some("convoy") => game.start_practice_convoy(),
        Some("host-convoy") => game.start_online(Join::Host { public: false, mode: arena_core::raid::ModeKind::Convoy, map: game.practice_map }),
        Some("host-brawl") => game.start_online(Join::Host { public: false, mode: arena_core::raid::ModeKind::ScrapBrawl, map: game.practice_map }),
        Some("host-team") => game.start_online(Join::Host { public: false, mode: arena_core::raid::ModeKind::TeamBrawl, map: game.practice_map }),
        Some("host-raid") => game.start_online(Join::Host { public: false, mode: arena_core::raid::ModeKind::RaiderWaves, map: game.practice_map }),
        Some("wellhead") => game.start_practice_wellhead(),
        Some("host-wellhead") => game.start_online(Join::Host { public: false, mode: arena_core::raid::ModeKind::Wellhead, map: game.practice_map }),
        Some("lastcrew") => game.start_practice_last_crew(),
        Some("host-lastcrew") => game.start_online(Join::Host { public: false, mode: arena_core::raid::ModeKind::LastCrew, map: game.practice_map }),
        _ => {}
    }
    let pause = mount_pause(&document);
    let now = now_ms();
    let rooms = Rc::new(RefCell::new(None));
    let state: Shared = Rc::new(RefCell::new(State {
        canvas,
        surface,
        config,
        wr,
        game,
        pause,
        rooms: rooms.clone(),
        start: now,
        last: now,
        frames: 0,
        cpu: (0.0, 0.0, 0.0, 0.0),
        prof: [0.0; 8],
        spikes: std::collections::VecDeque::new(),
    }));
    install_input(&state)?;
    fetch_rooms(rooms);

    // The requestAnimationFrame loop: the closure re-registers itself.
    let tick: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let again = Rc::clone(&tick);
    *tick.borrow_mut() = Some(Closure::new(move || {
        frame(&mut state.borrow_mut());
        request_frame(again.borrow().as_ref().expect("tick closure is set"));
    }));
    request_frame(tick.borrow().as_ref().expect("tick closure is set"));
    Ok(())
}

// ------------------------------------------------------------------ input

fn prop(v: &JsValue, name: &str) -> JsValue {
    js_sys::Reflect::get(v, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}

fn pointer_locked() -> bool {
    web_sys::window().and_then(|w| w.document()).and_then(|d| d.pointer_lock_element()).is_some()
}

fn listen(target: &web_sys::EventTarget, kind: &str, f: impl FnMut(web_sys::Event) + 'static) -> Result<()> {
    let cl: Closure<dyn FnMut(web_sys::Event)> = Closure::new(f);
    target
        .add_event_listener_with_callback(kind, cl.as_ref().unchecked_ref())
        .map_err(|_| anyhow!("adding a {kind} listener"))?;
    cl.forget();
    Ok(())
}

fn install_input(state: &Shared) -> Result<()> {
    let window = web_sys::window().ok_or_else(|| anyhow!("no window"))?;
    let document = window.document().ok_or_else(|| anyhow!("no document"))?;
    let win: web_sys::EventTarget = window.clone().into();
    let doc: web_sys::EventTarget = document.clone().into();
    // Poll the pads only once one has shown up (see `read_pad`).
    listen(&win, "gamepadconnected", |_| PAD_SEEN.with(|p| p.set(true)))?;

    let s = state.clone();
    listen(&win, "keydown", move |ev| {
        let Ok(mut st) = s.try_borrow_mut() else { return };
        let code = prop(&ev, "code").as_string().unwrap_or_default();
        let game = &mut st.game;
        game.gesture();
        let in_game = matches!(game.screen(), Screen::InGame | Screen::End);
        if !in_game || game.ui.chat_open() {
            return;
        }
        if matches!(code.as_str(), "Tab" | "Space" | "ArrowUp" | "ArrowDown") || prop(&ev, "ctrlKey").is_truthy() {
            ev.prevent_default();
        }
        if code == "Escape" && game.free_mouse {
            if game.paused {
                game.resume();
            } else {
                game.pointer_lost();
            }
            return;
        }
        game.controls.key_down(&code);
    })?;
    let s = state.clone();
    listen(&win, "keyup", move |ev| {
        let Ok(mut st) = s.try_borrow_mut() else { return };
        let code = prop(&ev, "code").as_string().unwrap_or_default();
        st.game.controls.key_up(&code);
    })?;
    let s = state.clone();
    listen(&win, "mousedown", move |ev| {
        let Ok(mut st) = s.try_borrow_mut() else { return };
        st.game.gesture();
        let on_canvas = prop(&ev, "target").dyn_into::<web_sys::Element>().is_ok_and(|t| t.id() == "tansk-canvas");
        let button = prop(&ev, "button").as_f64().unwrap_or(0.0) as usize;
        if st.game.wants_pointer() && !st.game.free_mouse && !pointer_locked() {
            if on_canvas {
                st.canvas.request_pointer_lock();
            }
            return;
        }
        if st.game.controls.captured && (on_canvas || pointer_locked()) {
            let b = match button {
                0 => 0,
                2 => 2,
                1 => 1,
                _ => return,
            };
            st.game.controls.button(b, true);
        }
    })?;
    let s = state.clone();
    listen(&win, "mouseup", move |ev| {
        let Ok(mut st) = s.try_borrow_mut() else { return };
        let button = prop(&ev, "button").as_f64().unwrap_or(0.0) as usize;
        st.game.controls.button(button.min(3), false);
    })?;
    let s = state.clone();
    listen(&win, "mousemove", move |ev| {
        let Ok(mut st) = s.try_borrow_mut() else { return };
        let dx = prop(&ev, "movementX").as_f64().unwrap_or(0.0) as f32;
        let dy = prop(&ev, "movementY").as_f64().unwrap_or(0.0) as f32;
        st.game.controls.mouse_move(dx, dy);
    })?;
    let s = state.clone();
    listen(&win, "blur", move |_| {
        if let Ok(mut st) = s.try_borrow_mut() {
            st.game.controls.release_all();
        }
    })?;
    let s = state.clone();
    listen(&win, "contextmenu", move |ev| {
        if let Ok(st) = s.try_borrow() {
            if matches!(st.game.screen(), Screen::InGame) {
                ev.prevent_default();
            }
        }
    })?;
    let s = state.clone();
    listen(&doc, "pointerlockchange", move |_| {
        let Ok(mut st) = s.try_borrow_mut() else { return };
        if pointer_locked() {
            st.game.pointer_locked();
        } else {
            st.game.pointer_lost();
        }
    })?;
    // Title buttons the UI layer doesn't know (`data-game`): Raid practice.
    if let Some(root) = document.get_element_by_id("tansk-ui") {
        let s = state.clone();
        listen(&root.into(), "click", move |ev| {
            let Some(t) = prop(&ev, "target").dyn_into::<web_sys::Element>().ok() else { return };
            let Ok(Some(b)) = t.closest("[data-game]") else { return };
            let Ok(mut st) = s.try_borrow_mut() else { return };
            match b.get_attribute("data-game").as_deref() {
                Some("raid") => st.game.start_practice_raid(),
                Some("team") => st.game.start_practice_team(),
                Some("convoy") => st.game.start_practice_convoy(),
                Some("wellhead") => st.game.start_practice_wellhead(),
                Some("lastcrew") => st.game.start_practice_last_crew(),
                _ => {}
            }
        })?;
    }
    // The pause menu's buttons (clicks are gestures, so the lock can be taken here).
    let pause = state.borrow().pause.clone();
    if let Some(p) = pause {
        let s = state.clone();
        listen(&p.into(), "click", move |ev| {
            let Ok(mut st) = s.try_borrow_mut() else { return };
            let Some(t) = prop(&ev, "target").dyn_into::<web_sys::Element>().ok() else { return };
            let Ok(Some(b)) = t.closest("[data-pause]") else { return };
            match b.get_attribute("data-pause").as_deref() {
                Some("resume") => {
                    st.game.ui_click();
                    st.game.resume();
                    if !st.game.free_mouse {
                        st.canvas.request_pointer_lock();
                    }
                }
                Some("leave") => {
                    st.game.ui_click();
                    st.game.leave();
                }
                _ => {}
            }
        })?;
    }
    Ok(())
}

/// A small pause card inside the UI root, so it gets the salvage styles.
fn mount_pause(document: &web_sys::Document) -> Option<web_sys::Element> {
    let root = document.get_element_by_id("tansk-ui")?;
    let el = document.create_element("section").ok()?;
    el.set_id("tansk-pause");
    let _ = el.set_attribute("class", "t-screen");
    let _ = el.set_attribute("hidden", "");
    let _ = el.set_attribute(
        "style",
        "display:grid;place-items:center;pointer-events:auto;background:rgba(24,14,8,.5);z-index:50",
    );
    el.set_inner_html(&format!(
        "<div class=\"t-pause-wrap\"><div class=\"plate\" style=\"padding:1.4rem 1.8rem;display:flex;flex-direction:column;gap:.8rem;min-width:18rem\">\
         <h2 style=\"margin:0 0 .3rem;font-family:'Black Ops One',sans-serif;letter-spacing:.08em;color:var(--cream)\">Engine idling</h2>\
         <button class=\"t-btn\" data-pause=\"resume\"><span class=\"t-btn-label\">Back in</span><span class=\"t-btn-hint\">Grabs the mouse</span></button>\
         <button class=\"t-btn\" data-pause=\"leave\"><span class=\"t-btn-label\">Leave</span><span class=\"t-btn-hint\">Back to the title</span></button>\
         </div><div class=\"plate t-pause-ctl\">{}</div></div>",
        crate::ui::CONTROLS_HTML
    ));
    root.append_child(&el).ok()?;
    Some(el)
}

// --------------------------------------------------------------- settings

const SETTINGS_KEY: &str = "tansk.settings.v1";
/// Onboarding tips already shown (comma separated keys).
const HINTS_KEY: &str = "tansk.hints.v1";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn load_settings() -> Option<Settings> {
    let raw = storage()?.get_item(SETTINGS_KEY).ok().flatten()?;
    let f: Vec<&str> = raw.split('\t').collect();
    if f.len() < 7 {
        return None;
    }
    let num = |i: usize, d: f32| f[i].parse::<f32>().unwrap_or(d);
    let d = Settings::default();
    Some(Settings {
        name: if f[0].trim().is_empty() { d.name } else { f[0].to_string() },
        vol_master: num(1, d.vol_master),
        vol_sfx: num(2, d.vol_sfx),
        vol_music: num(3, d.vol_music),
        vol_radio: num(4, d.vol_radio),
        mouse_sens: num(5, d.mouse_sens),
        vol_ambience: f.get(7).and_then(|v| v.parse().ok()).unwrap_or(d.vol_ambience),
        quality: match f[6] {
            "low" => Quality::Low,
            "medium" => Quality::Medium,
            "ultra" => Quality::Ultra,
            _ => Quality::High,
        },
    })
}

fn save_settings(s: &Settings) {
    let q = match s.quality {
        Quality::Low => "low",
        Quality::Medium => "medium",
        Quality::High => "high",
        Quality::Ultra => "ultra",
    };
    let raw = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        s.name.replace('\t', " "),
        s.vol_master,
        s.vol_sfx,
        s.vol_music,
        s.vol_radio,
        s.mouse_sens,
        q,
        s.vol_ambience
    );
    if let Some(st) = storage() {
        let _ = st.set_item(SETTINGS_KEY, &raw);
    }
}

// ------------------------------------------------------------------ rooms

/// Fetches `/api/arena/rooms` into `out` (picked up by the next frame).
/// (code, humans, max humans, mode, map) from `/api/arena/rooms`.
type RoomRow = (String, u32, u32, String, String);

fn fetch_rooms(out: Rc<RefCell<Option<Vec<RoomRow>>>>) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(window) = web_sys::window() else { return };
        let Ok(resp) = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str("/api/arena/rooms")).await else {
            return;
        };
        let Ok(json_fn) = prop(&resp, "json").dyn_into::<js_sys::Function>() else { return };
        let Ok(promise) = json_fn.call0(&resp) else { return };
        let Ok(promise) = promise.dyn_into::<js_sys::Promise>() else { return };
        let Ok(list) = wasm_bindgen_futures::JsFuture::from(promise).await else { return };
        let mut rooms = Vec::new();
        for r in js_sys::Array::from(&list).iter() {
            if prop(&r, "ended").is_truthy() {
                continue;
            }
            let code = prop(&r, "code").as_string().unwrap_or_default();
            let n = prop(&r, "humans").as_f64().unwrap_or(0.0) as u32;
            let max = prop(&r, "max_humans").as_f64().unwrap_or(8.0) as u32;
            let mode = prop(&r, "mode").as_string().unwrap_or_default();
            let map = prop(&r, "map").as_string().unwrap_or_default();
            rooms.push((code, n, max, mode, map));
        }
        *out.borrow_mut() = Some(rooms);
    });
}

// ---------------------------------------------------------------- gamepad

thread_local! {
    /// A `gamepadconnected` event has fired. Until then `read_pad` skips
    /// `navigator.getGamepads()`, which on Windows Chrome can stall a frame
    /// for 6-10 ms (measured with `data-spike`) even with no pad plugged in.
    static PAD_SEEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The first connected standard-mapping gamepad, via `navigator.getGamepads()`.
fn read_pad() -> Option<crate::game::Pad> {
    if !PAD_SEEN.with(std::cell::Cell::get) {
        return None;
    }
    let nav = web_sys::window()?.navigator();
    let get = prop(&nav, "getGamepads").dyn_into::<js_sys::Function>().ok()?;
    let list = get.call0(&nav).ok()?;
    let list = js_sys::Array::from(&list);
    for g in list.iter() {
        if g.is_null() || g.is_undefined() || !prop(&g, "connected").is_truthy() {
            continue;
        }
        let mut pad = crate::game::Pad::default();
        let axes = js_sys::Array::from(&prop(&g, "axes"));
        for (i, a) in axes.iter().take(4).enumerate() {
            pad.axes[i] = a.as_f64().unwrap_or(0.0) as f32;
        }
        let buttons = js_sys::Array::from(&prop(&g, "buttons"));
        for (i, b) in buttons.iter().take(16).enumerate() {
            pad.buttons[i] = prop(&b, "value").as_f64().unwrap_or(0.0) as f32;
        }
        return Some(pad);
    }
    None
}

// ------------------------------------------------------------------ frame

fn now_ms() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map(|p| p.now()).unwrap_or(0.0)
}

fn request_frame(f: &Closure<dyn FnMut()>) {
    if let Some(w) = web_sys::window() {
        let _ = w.request_animation_frame(f.as_ref().unchecked_ref());
    }
}

fn frame(s: &mut State) {
    let now = now_ms();
    let dt = ((now - s.last) / 1000.0) as f32;
    s.last = now;

    let (w, h) = canvas_size(&s.canvas);
    if (w, h) != (s.config.width, s.config.height) {
        s.canvas.set_width(w);
        s.canvas.set_height(h);
        s.config.width = w;
        s.config.height = h;
        s.surface.configure(&s.wr.renderer.device, &s.config);
        s.wr.renderer.resize(w, h);
    }

    s.game.pad = read_pad();
    // Platform requests from the game.
    if let Some(rooms) = s.rooms.borrow_mut().take() {
        s.game.set_rooms(&rooms);
    }
    if s.game.want_rooms() {
        fetch_rooms(s.rooms.clone());
    }
    if s.game.settings_dirty() {
        save_settings(&s.game.settings);
    }
    if std::mem::take(&mut s.game.hints.dirty) {
        if let Some(st) = storage() {
            let _ = st.set_item(HINTS_KEY, &s.game.hints.saved());
        }
    }

    let t_pre = now_ms();
    let world = s.game.frame(now / 1000.0, dt, &mut s.wr);
    let t_game = now_ms();

    if !s.game.wants_pointer() && pointer_locked() {
        if let Some(d) = web_sys::window().and_then(|w| w.document()) {
            d.exit_pointer_lock();
        }
    }
    if let Some(p) = &s.pause {
        let show = s.game.paused && s.game.screen() == Screen::InGame;
        let hidden = p.has_attribute("hidden");
        if show == hidden {
            let _ = if show { p.remove_attribute("hidden") } else { p.set_attribute("hidden", "") };
        }
    }

    let frame = match s.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            s.surface.configure(&s.wr.renderer.device, &s.config);
            return;
        }
        _ => return,
    };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    s.wr.draw(&view, &world);
    let t_draw = now_ms();
    s.wr.renderer.queue.present(frame);
    let end = now_ms();
    let total = end - now;
    s.cpu.0 += total;
    s.cpu.1 = s.cpu.1.max(total);
    s.cpu.2 += t_game - now;
    s.cpu.3 += t_draw - t_game;
    let cull = s.wr.renderer.cull_ms;
    let phases = s.wr.renderer.phase_ms;
    for (sum, v) in s.prof.iter_mut().zip(s.wr.prof.into_iter().chain([cull]).chain(phases)) {
        *sum += v;
    }
    // Hitches: which phase a slow frame spent its time in.
    if total > 8.0 {
        let p = s.wr.prof;
        let line = format!(
            "t={:.1} total={total:.1} game={:.1} web={:.1} net={:.1} g[pre={:.1} frame={:.1} post={:.1} cam={:.1} ev={:.1} snd={:.1} hud={:.1}] draw={:.1} present={:.1} fx={:.1} inst={:.1} sprites={:.1} render={:.1} cull={cull:.1} upload={:.1} encode={:.1} submit={:.1}",
            (now - s.start) / 1000.0,
            t_game - now,
            t_pre - now,
            s.game.prof_net_ms,
            s.game.prof_game[0],
            s.game.prof_game[5],
            s.game.prof_game[6],
            s.game.prof_game[1],
            s.game.prof_game[2],
            s.game.prof_game[3],
            s.game.prof_game[4],
            t_draw - t_game,
            end - t_draw,
            p[0],
            p[1],
            p[2],
            p[3],
            phases[0],
            phases[1],
            phases[2]
        );
        if s.spikes.len() == 5 {
            s.spikes.pop_front();
        }
        s.spikes.push_back(line);
        let all = s.spikes.iter().cloned().collect::<Vec<_>>().join(" | ");
        let _ = s.canvas.set_attribute("data-spike", &all);
    }
    // For headless checks: frames drawn and what is on screen.
    s.frames += 1;
    if s.frames % 30 == 0 {
        let _ = s.canvas.set_attribute("data-frames", &s.frames.to_string());
        let (calls, inst) = s.wr.renderer.stats;
        let elapsed = ((now - s.start) / 1000.0) as f32;
        let _ = s.canvas.set_attribute(
            "data-stats",
            &format!(
                "tanks={} draws={calls} instances={inst} particles={} sprites={} q={:?} scale={:.2} t={elapsed:.1}",
                world.tanks.len(),
                s.wr.fx.particle_count(),
                s.wr.renderer.sprite_count,
                s.wr.renderer.quality(),
                s.wr.renderer.render_scale()
            ),
        );
        let _ = s.canvas.set_attribute("data-game", &s.game.debug_line());
    }
    if s.frames % 60 == 0 {
        let (sum, max, game, draw) = s.cpu;
        let p = s.prof.map(|v| v / 60.0);
        let _ = s.canvas.set_attribute(
            "data-cpu",
            &format!(
                "cpu={:.2} max={max:.2} game={:.2} draw={:.2} fx={:.2} inst={:.2} sprites={:.2} render={:.2} cull={:.2} upload={:.2} encode={:.2} submit={:.2}",
                sum / 60.0,
                game / 60.0,
                draw / 60.0,
                p[0],
                p[1],
                p[2],
                p[3],
                p[4],
                p[5],
                p[6],
                p[7]
            ),
        );
        s.cpu = (0.0, 0.0, 0.0, 0.0);
        s.prof = [0.0; 8];
    }
}
