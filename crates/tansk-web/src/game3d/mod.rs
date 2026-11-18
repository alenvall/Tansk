//! Tansk 3D in the browser (the `/3d` route): the same menus, a first-person
//! view from your turret over hilly terrain, online games over `/ws3d` and
//! an offline playground running `Game3d` right here.

mod gl;
mod hud;
mod render;

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use tansk3d_sim::game::{BARREL_LENGTH, GRAVITY, TURRET_PIVOT_UP};
use tansk3d_sim::proto::{
    ClientMsg3d, Event3d, Input3d, MsgLevel, Phase, PickupKind, Power, ServerMsg3d, Settings, Sound, View3d, Weapon,
    MAX_PITCH, MIN_PITCH,
};
use tansk3d_sim::terrain::{self, ARENA_D, ARENA_W};
use tansk3d_sim::{Game3d, TICK_MS};
use tansk_sim::V2;
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, Document, Element, HtmlCanvasElement, HtmlElement, HtmlInputElement, KeyboardEvent,
    MouseEvent,
};

use self::gl::{add, dir, len, madd, sub, V3};
use self::render::{Camera, Frame, Fx, FxKind, Heights, OwnGun, Scene};
use crate::assets::{Audio, Images, Music};
use crate::net::NetEvent;
use crate::render::{hit, ConsoleMsg, LINK_BOX, MENU_BUTTON, RESTART_BUTTON, SOUND_ICON, START_BUTTON};
use crate::{canvas_point, fetch_rooms, lerp_angle, listen, now, render_rooms, storage_get, storage_set, window};

type Net = crate::net::Net<ClientMsg3d, ServerMsg3d>;

const TICK: f64 = TICK_MS as f64;
/// How far behind the newest server view the online game is drawn, in ticks.
const BUFFER_TICKS: f64 = 2.5;
/// The camera may look a little past where the barrel can point.
const CAM_MIN_PITCH: f32 = MIN_PITCH - 14.0;
const CAM_MAX_PITCH: f32 = MAX_PITCH + 15.0;
const EYE_UP: f32 = TURRET_PIVOT_UP + 11.0;

/// What the DOM listeners hand to the frame loop.
#[derive(Default)]
struct Shared {
    keys: HashSet<String>,
    presses: Vec<(String, bool)>,
    mouse: V2,
    mouse_down: bool,
    /// The left button while the pointer is locked: firing.
    fire: bool,
    clicks: Vec<V2>,
    actions: Vec<String>,
    gesture: bool,
    look: (f64, f64),
    locked: bool,
    /// A click on the canvas should grab the pointer (set by the frame loop).
    lock_wanted: bool,
}

/// The offline playground: you and three dummies that sit still.
struct Local {
    game: Game3d,
    me: u32,
    acc: f64,
    prev: View3d,
    cur: View3d,
    drops: usize,
}

impl Local {
    fn new() -> Self {
        let mut game = Game3d::new(Settings::playground(), (js_sys::Math::random() * 1e15) as u64);
        let me = game.add_player("Euler", 5);
        for (name, color) in [("Newton", 1), ("Gauss", 2), ("Leibniz", 3)] {
            game.add_player(name, color);
        }
        game.start_with_delay(3000);
        let cur = game.view();
        Local { game, me, acc: 0.0, prev: cur.clone(), cur, drops: 0 }
    }
}

/// A game on the server.
struct Online {
    net: Net,
    first: Option<ClientMsg3d>,
    connected: bool,
    code: Option<String>,
    me: Option<u32>,
    host: bool,
    settings: Settings,
    views: VecDeque<View3d>,
    render_tick: f64,
    last_sent: Option<Input3d>,
    last_send_at: f64,
    closed: bool,
    copied_at: f64,
}

enum Mode {
    Menu,
    Playground(Box<Local>),
    Online(Box<Online>),
}

struct App {
    doc: Document,
    stage: HtmlElement,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    chat: HtmlInputElement,
    shared: Rc<RefCell<Shared>>,
    images: Images,
    audio: Audio,
    mode: Mode,
    screen: &'static str,
    koth: bool,
    console: Vec<ConsoleMsg>,
    fx: Vec<Fx>,
    debug: bool,
    last_frame: f64,
    fps: (u32, f64, u32),
    rooms_fetched_at: f64,
    rooms: Rc<RefCell<Option<Vec<serde_json::Value>>>>,
    scene: Option<Scene>,
    minimap: Option<HtmlCanvasElement>,
    /// Where you look (and aim): degrees.
    cam_yaw: f32,
    cam_pitch: f32,
    cam_eye: V3,
    recoil: f32,
    sens: f32,
    /// Your tank was on the field last frame (to face its turret on respawn).
    had_tank: bool,
}

impl App {
    fn el(&self, id: &str) -> Element {
        self.doc.get_element_by_id(id).unwrap_or_else(|| panic!("#{id}"))
    }

    fn input(&self, id: &str) -> HtmlInputElement {
        self.el(id).dyn_into().unwrap()
    }

    fn name(&self) -> String {
        let n = self.input("player-name").value();
        if n.trim().is_empty() {
            "Unnamed".into()
        } else {
            n
        }
    }

    // ------------------------------------------------------------- screens

    fn show(&mut self, screen: &'static str) {
        for s in ["menu", "host", "join", "settings"] {
            let _ = self.el(s).class_list().toggle_with_force("hidden", s != screen);
        }
        self.screen = screen;
        let in_game = screen.is_empty();
        let chat_visible = in_game && matches!(self.mode, Mode::Online(_));
        let _ = self.chat.class_list().toggle_with_force("hidden", !chat_visible);
        if screen == "join" {
            self.rooms_fetched_at = -1e9;
            let _ = self.input("join-code").focus();
        }
        if !in_game {
            self.unlock();
            self.mode = Mode::Menu;
            self.audio.play_music(Music::Menu);
            let _ = window().history().map(|h| h.replace_state_with_url(&JsValue::NULL, "", Some("/3d")));
        }
    }

    fn unlock(&self) {
        if self.doc.pointer_lock_element().is_some() {
            self.doc.exit_pointer_lock();
        }
    }

    fn back_to_menu(&mut self) {
        self.console.clear();
        self.fx.clear();
        self.show("menu");
    }

    fn reset_game_state(&mut self) {
        self.console.clear();
        self.fx.clear();
        self.had_tank = false;
        self.cam_pitch = 0.0;
    }

    fn start_playground(&mut self) {
        self.reset_game_state();
        self.mode = Mode::Playground(Box::new(Local::new()));
        self.show("");
        self.audio.play_music(Music::Battle);
        self.log("Playground: 1-6 swap weapons, Q drops a pickup in front of you.", MsgLevel::Info);
    }

    fn start_online(&mut self, first: ClientMsg3d) {
        self.reset_game_state();
        match Net::connect("/ws3d") {
            Ok(net) => {
                self.mode = Mode::Online(Box::new(Online {
                    net,
                    first: Some(first),
                    connected: false,
                    code: None,
                    me: None,
                    host: false,
                    settings: Settings::default(),
                    views: VecDeque::new(),
                    render_tick: 0.0,
                    last_sent: None,
                    last_send_at: 0.0,
                    closed: false,
                    copied_at: -1e9,
                }));
                self.show("");
            }
            Err(_) => {
                self.show("join");
                self.el("join-error").set_text_content(Some("Couldn't reach the server."));
            }
        }
    }

    fn host_settings(&self) -> Settings {
        let v = |id: &str| self.input(id).value().parse::<i32>().unwrap_or(0);
        Settings {
            koth: self.koth,
            lives: v("s-lives"),
            score_limit: v("s-score"),
            rounds: v("s-rounds"),
            spawn_time_ms: v("s-spawn") * 1000,
            round_time_ms: v("s-round") * 1000,
            game_time_ms: v("s-game") * 1000,
        }
        .clamped()
    }

    fn log(&mut self, text: &str, level: MsgLevel) {
        let d = js_sys::Date::new_0();
        let color = match level {
            MsgLevel::Chat => "#000000",
            MsgLevel::Info => "#ffff00",
            MsgLevel::Error => "#ff0000",
        };
        self.console.push(ConsoleMsg { stamp: format!("{:02}:{:02}", d.get_hours(), d.get_minutes()), text: text.to_string(), color, at: now() });
        if self.console.len() > 50 {
            self.console.remove(0);
        }
    }

    fn action(&mut self, a: &str) {
        match a {
            "playground" => self.start_playground(),
            "host-menu" => self.show("host"),
            "join-menu" => {
                self.el("join-error").set_text_content(None);
                self.show("join");
            }
            "settings-menu" => self.show("settings"),
            "back" => {
                storage_set("tansk.name", &self.input("player-name").value());
                self.show("menu");
            }
            "radio-standard" | "radio-koth" => {
                self.koth = a == "radio-koth";
                let _ = self.el("radio-standard").class_list().toggle_with_force("selected", !self.koth);
                let _ = self.el("radio-koth").class_list().toggle_with_force("selected", self.koth);
                let _ = self.el("row-lives").class_list().toggle_with_force("hidden", self.koth);
            }
            "host-start" => {
                let msg = ClientMsg3d::Create { name: self.name(), settings: self.host_settings() };
                self.start_online(msg);
            }
            "join-go" => {
                let code = self.input("join-code").value().trim().to_uppercase();
                if code.len() != 4 {
                    self.el("join-error").set_text_content(Some("Enter the game's four letter code."));
                    return;
                }
                self.start_online(ClientMsg3d::Join { code, name: self.name() });
            }
            _ => {
                if let Some(code) = a.strip_prefix("join-room:") {
                    self.input("join-code").set_value(code);
                    self.start_online(ClientMsg3d::Join { code: code.to_string(), name: self.name() });
                }
            }
        }
    }

    // ---------------------------------------------------------------- input

    fn typing(&self) -> bool {
        self.doc
            .active_element()
            .map(|e| e.tag_name().eq_ignore_ascii_case("input") && e.get_attribute("type").as_deref() != Some("range"))
            .unwrap_or(false)
    }

    fn game_input(&self, sh: &Shared) -> Input3d {
        let aim_yaw = self.cam_yaw.rem_euclid(360.0);
        let aim_pitch = self.cam_pitch.clamp(MIN_PITCH, MAX_PITCH);
        if self.typing() {
            return Input3d { aim_yaw, aim_pitch, ..Input3d::default() };
        }
        let k = |c: &str| sh.keys.contains(c);
        Input3d {
            forward: k("KeyW") || k("ArrowUp"),
            back: k("KeyS") || k("ArrowDown"),
            left: k("KeyA") || k("ArrowLeft"),
            right: k("KeyD") || k("ArrowRight"),
            fire: sh.fire && sh.locked,
            aim_yaw,
            aim_pitch,
        }
    }

    fn key(&mut self, code: &str, ctrl: bool) {
        let in_game = self.screen.is_empty();
        if code == "Enter" || code == "NumpadEnter" {
            if in_game && matches!(self.mode, Mode::Online(_)) {
                let focused = self.doc.active_element().is_some_and(|e| e.id() == "chat");
                if focused {
                    let text = self.chat.value();
                    if !text.trim().is_empty() {
                        if let Mode::Online(o) = &self.mode {
                            o.net.send(&ClientMsg3d::Chat(text));
                        }
                    }
                    self.chat.set_value("");
                    let _ = self.chat.blur();
                    if self.shared.borrow().lock_wanted {
                        self.canvas.request_pointer_lock();
                    }
                } else {
                    self.unlock();
                    let _ = self.chat.focus();
                }
            } else if self.screen == "join" {
                self.action("join-go");
            }
            return;
        }
        if code == "Escape" {
            if in_game {
                self.back_to_menu();
            } else if self.screen != "menu" {
                self.action("back");
            }
            return;
        }
        if !in_game || self.typing() {
            return;
        }
        match code {
            "F1" => self.debug = !self.debug,
            "Equal" | "NumpadAdd" => self.audio.set_volume(self.audio.volume + 0.05),
            "Minus" | "NumpadSubtract" => self.audio.set_volume(self.audio.volume - 0.05),
            "KeyM" => self.audio.toggle_mute(),
            "KeyS" if ctrl => self.audio.toggle_mute(),
            _ => {}
        }
        let yaw = self.cam_yaw;
        if let Mode::Playground(l) = &mut self.mode {
            let w = match code {
                "Digit1" => Some(Weapon::Default),
                "Digit2" => Some(Weapon::Flamethrower),
                "Digit3" => Some(Weapon::Shotgun),
                "Digit4" => Some(Weapon::Slowspeedy),
                "Digit5" => Some(Weapon::Shockwave),
                "Digit6" => Some(Weapon::Bounce),
                _ => None,
            };
            if let Some(w) = w {
                l.game.give_weapon(l.me, w);
            }
            if code == "KeyQ" {
                if let Some(t) = l.cur.tanks.iter().find(|t| t.player == l.me) {
                    const KINDS: [PickupKind; 9] = [
                        PickupKind::Weapon(Weapon::Slowspeedy),
                        PickupKind::Power(Power::Shield),
                        PickupKind::Weapon(Weapon::Shotgun),
                        PickupKind::Power(Power::Speed),
                        PickupKind::Weapon(Weapon::Bounce),
                        PickupKind::Power(Power::Damage),
                        PickupKind::Weapon(Weapon::Shockwave),
                        PickupKind::Power(Power::FireRate),
                        PickupKind::Weapon(Weapon::Flamethrower),
                    ];
                    let d = dir(yaw, 0.0);
                    let x = (t.pos[0] + d[0] * 90.0).clamp(20.0, ARENA_W - 20.0);
                    let z = (t.pos[2] + d[2] * 90.0).clamp(20.0, ARENA_D - 20.0);
                    l.game.add_pickup(KINDS[l.drops % KINDS.len()], x, z);
                    l.drops += 1;
                }
            }
        }
    }

    fn click(&mut self, p: V2) {
        if !self.screen.is_empty() {
            return;
        }
        if hit(SOUND_ICON, p) {
            self.audio.toggle_mute();
            return;
        }
        let phase = self.current_phase();
        if phase == Some(Phase::GameOver) {
            if hit(MENU_BUTTON, p) {
                self.back_to_menu();
                return;
            }
            if hit(RESTART_BUTTON, p) {
                match &self.mode {
                    Mode::Playground(_) => self.start_playground(),
                    Mode::Online(o) if o.host => o.net.send(&ClientMsg3d::Restart),
                    _ => {}
                }
                return;
            }
        }
        if let Mode::Online(o) = &mut self.mode {
            if phase == Some(Phase::Lobby) {
                if o.host && hit(START_BUTTON, p) {
                    o.net.send(&ClientMsg3d::Start);
                }
                if hit(LINK_BOX, p) {
                    if let Some(link) = share_link(o.code.as_deref()) {
                        let _ = window().navigator().clipboard().write_text(&link);
                        o.copied_at = now();
                    }
                }
            }
        }
    }

    fn current_phase(&self) -> Option<Phase> {
        match &self.mode {
            Mode::Playground(l) => Some(l.cur.phase),
            Mode::Online(o) => o.views.back().map(|v| v.phase),
            Mode::Menu => None,
        }
    }

    /// Sounds (quieter far away) and effects.
    fn events(&mut self, events: &[Event3d], my_muzzle: Option<V3>) {
        let t = now();
        for e in events {
            match *e {
                Event3d::Sound(s, pos) => {
                    let d = len(sub(pos, self.cam_eye));
                    self.audio.play_at(s, 1.0 / (1.0 + d / 320.0));
                    if matches!(s, Sound::DefaultFire | Sound::Shotgun | Sound::SlowspeedyFire) {
                        self.fx.push(Fx { kind: FxKind::Muzzle, pos, started: t });
                        if my_muzzle.is_some_and(|m| len(sub(m, pos)) < 40.0) {
                            self.recoil = 1.0;
                        }
                    }
                }
                Event3d::Explosion(pos) => self.fx.push(Fx { kind: FxKind::Explosion, pos, started: t }),
                Event3d::Impact(pos) => self.fx.push(Fx { kind: FxKind::Impact, pos, started: t }),
            }
        }
        if self.fx.len() > 240 {
            let n = self.fx.len() - 240;
            self.fx.drain(..n);
        }
    }

    // ---------------------------------------------------------------- frame

    fn frame(&mut self) {
        let t = now();
        let dt = (t - self.last_frame).clamp(0.0, 250.0);
        self.last_frame = t;
        self.fps.0 += 1;
        if t - self.fps.1 >= 1000.0 {
            self.fps = (0, t, self.fps.0);
        }
        fit_stage(&self.stage);

        let (presses, clicks, actions, gesture, look) = {
            let mut sh = self.shared.borrow_mut();
            let g = std::mem::take(&mut sh.gesture);
            let look = std::mem::take(&mut sh.look);
            (std::mem::take(&mut sh.presses), std::mem::take(&mut sh.clicks), std::mem::take(&mut sh.actions), g, look)
        };
        if gesture {
            self.audio.unlock();
        }
        for a in actions {
            self.action(&a);
        }
        for (code, ctrl) in presses {
            self.key(&code, ctrl);
        }
        for c in clicks {
            self.click(c);
        }
        self.sync_menus();
        self.fx.retain(|e| t - e.started < e.duration());

        // Mouse look.
        let k = 0.022 * self.sens;
        self.cam_yaw = (self.cam_yaw + look.0 as f32 * k).rem_euclid(360.0);
        self.cam_pitch = (self.cam_pitch - look.1 as f32 * k).clamp(CAM_MIN_PITCH, CAM_MAX_PITCH);
        self.recoil = (self.recoil - dt as f32 / 180.0).max(0.0);

        let mode = std::mem::replace(&mut self.mode, Mode::Menu);
        self.mode = match mode {
            Mode::Menu => Mode::Menu,
            Mode::Playground(l) => self.frame_local(l, dt),
            Mode::Online(o) => self.frame_online(o, dt),
        };
        if matches!(self.mode, Mode::Menu) {
            self.shared.borrow_mut().lock_wanted = false;
        }
    }

    /// Slider labels, the volume, the sensitivity, and the open games list.
    fn sync_menus(&mut self) {
        if self.screen == "settings" {
            let v = self.input("s-volume").value().parse::<f32>().unwrap_or(35.0) / 100.0;
            if (v - self.audio.volume).abs() > 0.001 {
                self.audio.set_volume(v);
                storage_set("tansk.volume", &v.to_string());
            }
            let s = self.input("s-sens").value().parse::<f32>().unwrap_or(10.0) / 10.0;
            if (s - self.sens).abs() > 0.001 {
                self.sens = s;
                storage_set("tansk.sens", &s.to_string());
            }
        } else {
            let v = (self.audio.volume * 100.0).round() as i32;
            self.input("s-volume").set_value(&v.to_string());
        }
        if self.screen == "join" {
            let t = now();
            if t - self.rooms_fetched_at > 3000.0 {
                self.rooms_fetched_at = t;
                fetch_rooms("/api/rooms3d", self.rooms.clone());
            }
            if let Some(rooms) = self.rooms.borrow_mut().take() {
                render_rooms(&self.doc, &rooms);
            }
        }
    }

    fn frame_local(&mut self, mut l: Box<Local>, dt: f64) -> Mode {
        let input = self.game_input(&self.shared.borrow());
        l.acc += dt;
        let mut events = Vec::new();
        while l.acc >= TICK {
            l.acc -= TICK;
            if l.cur.phase != Phase::GameOver {
                l.game.set_input(l.me, input);
                l.game.step();
            }
            let next = l.game.view();
            events.extend(next.events.iter().copied());
            l.prev = std::mem::replace(&mut l.cur, next);
        }
        let view = lerp_view(&l.prev, &l.cur, (l.acc / TICK) as f32);
        let muzzle = view.tanks.iter().find(|t| t.player == l.me).map(render::muzzle);
        self.events(&events, muzzle);
        self.draw(Some(&view), Some(l.me), Settings::playground(), None, false, None, false, None, false);
        Mode::Playground(l)
    }

    fn frame_online(&mut self, mut o: Box<Online>, dt: f64) -> Mode {
        let mut events = Vec::new();
        for ev in o.net.take() {
            match ev {
                NetEvent::Open => {
                    o.connected = true;
                    if let Some(first) = o.first.take() {
                        o.net.send(&first);
                    }
                }
                NetEvent::Closed => {
                    if !o.closed {
                        o.closed = true;
                        o.connected = false;
                        if o.me.is_none() {
                            self.show("join");
                            self.el("join-error").set_text_content(Some("Couldn't connect to the server."));
                            return Mode::Menu;
                        }
                        self.log("Disconnected from server.", MsgLevel::Error);
                    }
                }
                NetEvent::Msg(msg) => match msg {
                    ServerMsg3d::Welcome { code, you, host, settings } => {
                        let first_time = o.me.is_none();
                        o.code = Some(code.clone());
                        o.me = Some(you);
                        o.host = host;
                        o.settings = settings;
                        o.views.clear();
                        if first_time {
                            self.log("Joined game!", MsgLevel::Info);
                            self.audio.play_music(Music::Battle);
                            let url = format!("/3d?room={code}");
                            let _ = window().history().map(|h| h.replace_state_with_url(&JsValue::NULL, "", Some(&url)));
                        }
                    }
                    ServerMsg3d::Rejected(reason) => {
                        o.closed = true;
                        self.show("join");
                        self.el("join-error").set_text_content(Some(&format!("Connection refused: {reason}")));
                        return Mode::Menu;
                    }
                    ServerMsg3d::Kicked => {
                        o.closed = true;
                        self.show("join");
                        self.el("join-error").set_text_content(Some("You have been kicked from the server."));
                        return Mode::Menu;
                    }
                    ServerMsg3d::Host(h) => o.host = h,
                    ServerMsg3d::Message { text, level } => self.log(&text, level),
                    ServerMsg3d::View(mut v) => {
                        events.append(&mut v.events);
                        o.views.push_back(v);
                        while o.views.len() > 32 {
                            o.views.pop_front();
                        }
                    }
                },
            }
        }

        // Play the views back a little behind the newest, smoothing jitter.
        let view = if let Some(newest) = o.views.back() {
            let target = newest.tick as f64 - BUFFER_TICKS;
            o.render_tick += dt / TICK;
            let drift = target - o.render_tick;
            if drift.abs() > 8.0 {
                o.render_tick = target;
            } else {
                o.render_tick += drift * 0.08;
            }
            Some(sample(&o.views, o.render_tick))
        } else {
            None
        };
        let muzzle = view.as_ref().and_then(|v| v.tanks.iter().find(|t| Some(t.player) == o.me)).map(render::muzzle);
        self.events(&events, muzzle);

        let input = self.game_input(&self.shared.borrow());
        let t = now();
        if o.me.is_some() && !o.closed && (o.last_sent != Some(input) && t - o.last_send_at >= TICK || t - o.last_send_at > 500.0) {
            o.net.send(&ClientMsg3d::Input(input));
            o.last_sent = Some(input);
            o.last_send_at = t;
        }

        let show_info = view.as_ref().is_some_and(|v| v.phase == Phase::Lobby || (v.phase == Phase::Countdown && v.round == 0 && v.delay_ms >= 3000));
        let link = share_link(o.code.as_deref());
        let copied = t - o.copied_at < 2000.0;
        self.draw(view.as_ref(), o.me, o.settings, o.code.as_deref(), o.host, Some(o.connected), show_info, link.as_deref(), copied);
        Mode::Online(o)
    }

    /// Picks the camera, draws the world and the HUD, and decides whether the pointer is wanted.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        v: Option<&View3d>,
        me: Option<u32>,
        settings: Settings,
        code: Option<&str>,
        host: bool,
        connected: Option<bool>,
        show_info: bool,
        link: Option<&str>,
        copied: bool,
    ) {
        // `connected` is None offline.
        let online = connected.is_some();
        let connected = connected.unwrap_or(true);
        let t = now();
        let phase = v.map(|v| v.phase);
        let my_tank = v.and_then(|v| v.tanks.iter().find(|t| Some(t.player) == me));
        let first_person = my_tank.is_some() && !show_info && matches!(phase, Some(Phase::Countdown | Phase::Playing));

        // Face where your turret faces when you (re)spawn.
        if let Some(tk) = my_tank {
            if !self.had_tank {
                self.cam_yaw = tk.turret_yaw;
                self.cam_pitch = 0.0;
            }
        }
        self.had_tank = my_tank.is_some();

        let cam = match my_tank {
            Some(tk) if first_person => {
                let look = dir(self.cam_yaw, self.cam_pitch + self.recoil * 2.5);
                let eye = add(tk.pos, [0.0, EYE_UP, 0.0]);
                Camera { eye, target: madd(eye, look, 100.0), fov: 70.0, fog: 1.0 / 1900.0 }
            }
            _ => {
                let a = (t / 1000.0 * 0.07) as f32;
                let c = terrain::zone_center();
                let eye = [512.0 + a.cos() * 560.0, 470.0, 384.0 + a.sin() * 420.0];
                Camera { eye, target: [c[0], c[1] - 60.0, c[2]], fov: 60.0, fog: 1.0 / 3200.0 }
            }
        };
        self.cam_eye = cam.eye;

        // Where a shell would land, for the weapons that arc.
        let impact = match my_tank {
            Some(tk) if first_person => ballistic(tk.weapon, self.cam_yaw, self.cam_pitch.clamp(MIN_PITCH, MAX_PITCH), tk.pos),
            _ => None,
        };
        let range = impact.zip(my_tank).map(|(p, tk)| ((p[0] - tk.pos[0]).powi(2) + (p[2] - tk.pos[2]).powi(2)).sqrt());

        if let Some(scene) = &mut self.scene {
            let own = if first_person { my_tank.map(|tk| OwnGun { tank: tk.id }) } else { None };
            scene.draw(&self.images, &cam, &Frame { view: v, own, fx: &self.fx, now: t, impact });
        }

        // The pointer: wanted while you play, released for menus and game over.
        let typing = self.typing();
        let lock_wanted = !show_info && matches!(phase, Some(Phase::Countdown | Phase::Playing)) && !typing;
        let locked = {
            let mut sh = self.shared.borrow_mut();
            sh.lock_wanted = lock_wanted;
            sh.locked
        };
        if locked && !lock_wanted && !typing {
            self.unlock();
        }

        let (mouse, mouse_down, scores) = {
            let sh = self.shared.borrow();
            (sh.mouse, sh.mouse_down, sh.keys.contains("Tab") && !typing)
        };
        let chat_open = self.doc.active_element().is_some_and(|e| e.id() == "chat");
        let hud = hud::Hud3d {
            now: t,
            me,
            settings,
            code,
            link,
            host,
            online,
            connected,
            show_info,
            console: &self.console,
            chat_open,
            mouse,
            mouse_down,
            sound_icon: self.audio.icon(),
            copied,
            first_person,
            click_to_play: lock_wanted && !locked,
            aim_pitch: self.cam_pitch.clamp(MIN_PITCH, MAX_PITCH),
            cam_yaw: self.cam_yaw,
            range,
            scores,
            debug: self.debug.then(|| {
                vec![
                    format!("FPS: {}", self.fps.2),
                    format!("Effects: {}", self.fx.len()),
                    format!("Volume: {:.2}", self.audio.volume),
                    format!("Look: {:.0} / {:.1}", self.cam_yaw, self.cam_pitch),
                ]
            }),
        };
        match &self.scene {
            Some(scene) => hud::draw(&self.ctx, &self.images, scene, self.minimap.as_ref(), v, &hud),
            None => {
                self.ctx.clear_rect(0.0, 0.0, 1024.0, 768.0);
                self.ctx.set_fill_style_str("#000");
                let _ = self.ctx.fill_text("Tansk 3D needs WebGL2, which this browser doesn't offer.", 300.0, 380.0);
            }
        }
    }
}

/// Simulates a shell from your muzzle; where it meets the ground.
fn ballistic(weapon: Weapon, yaw: f32, pitch: f32, pos: V3) -> Option<V3> {
    // Muzzle speeds (px/ms) of the weapons whose shells arc.
    let speed = match weapon {
        Weapon::Default => 0.7,
        Weapon::Bounce => 0.5,
        Weapon::Shockwave => 0.4,
        _ => return None,
    };
    let d = dir(yaw, pitch);
    let mut p = madd(add(pos, [0.0, TURRET_PIVOT_UP, 0.0]), d, BARREL_LENGTH);
    let mut vel = gl::scale(d, speed);
    let step = 20.0;
    for _ in 0..600 {
        let next = madd(p, vel, step);
        vel[1] -= GRAVITY * step;
        let g = render::terrain_y(next[0], next[2]);
        if next[1] <= g {
            // Back up to where it crossed.
            let above = p[1] - render::terrain_y(p[0], p[2]);
            let k = (above / (above + (g - next[1])).max(1e-3)).clamp(0.0, 1.0);
            let hit = add(p, gl::scale(sub(next, p), k));
            return Some([hit[0], render::terrain_y(hit[0], hit[2]), hit[2]]);
        }
        if !(-200.0..ARENA_W + 200.0).contains(&next[0]) || !(-200.0..ARENA_D + 200.0).contains(&next[2]) {
            return None;
        }
        p = next;
    }
    None
}

fn share_link(code: Option<&str>) -> Option<String> {
    let origin = window().location().origin().ok()?;
    Some(format!("{origin}/3d?room={}", code?))
}

fn fit_stage(stage: &HtmlElement) {
    let w = window();
    let iw = w.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1024.0);
    let ih = w.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(768.0);
    let s = (iw / 1024.0).min(ih / 768.0);
    let style = stage.style();
    let _ = style.set_property("left", &format!("{}px", (iw - 1024.0 * s) / 2.0));
    let _ = style.set_property("top", &format!("{}px", (ih - 768.0 * s) / 2.0));
    let _ = style.set_property("transform", &format!("scale({s})"));
}

// -------------------------------------------------------- interpolation

/// The view at a fractional tick, between the two views around it.
fn sample(views: &VecDeque<View3d>, tick: f64) -> View3d {
    let first = views.front().unwrap();
    if tick <= first.tick as f64 {
        return first.clone();
    }
    for (a, b) in views.iter().zip(views.iter().skip(1)) {
        if (a.tick as f64) <= tick && tick < b.tick as f64 {
            let span = (b.tick - a.tick).max(1) as f64;
            return lerp_view(a, b, ((tick - a.tick as f64) / span) as f32);
        }
    }
    views.back().unwrap().clone()
}

fn lerp3(a: V3, b: V3, t: f32) -> V3 {
    if len(sub(a, b)) > 120.0 {
        return b;
    }
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// `b`, with tanks and projectiles moved back toward where they were in `a`.
fn lerp_view(a: &View3d, b: &View3d, t: f32) -> View3d {
    let t = t.clamp(0.0, 1.0);
    let mut v = b.clone();
    v.events.clear();
    for tk in &mut v.tanks {
        if let Some(o) = a.tanks.iter().find(|o| o.id == tk.id) {
            tk.pos = lerp3(o.pos, tk.pos, t);
            tk.yaw = lerp_angle(o.yaw, tk.yaw, t);
            tk.turret_yaw = lerp_angle(o.turret_yaw, tk.turret_yaw, t);
            tk.hull_pitch = o.hull_pitch + (tk.hull_pitch - o.hull_pitch) * t;
            tk.hull_roll = o.hull_roll + (tk.hull_roll - o.hull_roll) * t;
            tk.barrel_pitch = o.barrel_pitch + (tk.barrel_pitch - o.barrel_pitch) * t;
        }
    }
    for p in &mut v.projectiles {
        if let Some(o) = a.projectiles.iter().find(|o| o.id == p.id) {
            p.pos = lerp3(o.pos, p.pos, t);
        }
    }
    v
}

// ------------------------------------------------------------------ setup

pub fn run() {
    let doc = window().document().unwrap();
    if let Some(body) = doc.body() {
        let _ = body.class_list().add_1("is3d");
    }
    doc.set_title("Tansk 3D!");
    let canvas: HtmlCanvasElement = doc.get_element_by_id("game").unwrap().dyn_into().unwrap();
    let ctx: CanvasRenderingContext2d = canvas.get_context("2d").unwrap().unwrap().dyn_into().unwrap();
    let stage: HtmlElement = doc.get_element_by_id("stage").unwrap().dyn_into().unwrap();
    let chat: HtmlInputElement = doc.get_element_by_id("chat").unwrap().dyn_into().unwrap();
    let shared: Rc<RefCell<Shared>> = Rc::default();

    let heights = Rc::new(Heights::new());
    render::HEIGHTS.with(|h| *h.borrow_mut() = Some(heights.clone()));
    let scene = doc
        .get_element_by_id("game3d")
        .and_then(|c| c.dyn_into::<HtmlCanvasElement>().ok())
        .and_then(|c| Scene::new(c, &heights));
    let minimap = hud::minimap_canvas(&heights);

    let volume = storage_get("tansk.volume").and_then(|v| v.parse().ok()).unwrap_or(0.35);
    let sens: f32 = storage_get("tansk.sens").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let images = Images::default();
    images.preload(&["map", "background", "tank_spawnpoint", "horizontal_wall", "king_of_the_hill_zone", "crosshair"]);

    let mut app = App {
        doc: doc.clone(),
        stage,
        canvas: canvas.clone(),
        ctx,
        chat,
        shared: shared.clone(),
        images,
        audio: Audio::new(volume),
        mode: Mode::Menu,
        screen: "menu",
        koth: false,
        console: Vec::new(),
        fx: Vec::new(),
        debug: false,
        last_frame: now(),
        fps: (0, now(), 0),
        rooms_fetched_at: -1e9,
        rooms: Rc::default(),
        scene,
        minimap,
        cam_yaw: 0.0,
        cam_pitch: 0.0,
        cam_eye: [512.0, 300.0, 384.0],
        recoil: 0.0,
        sens,
        had_tank: false,
    };
    app.input("player-name").set_value(&storage_get("tansk.name").unwrap_or_else(|| "Unnamed".into()));
    let s = ((sens * 10.0).round() as i32).to_string();
    app.input("s-sens").set_value(&s);
    if let Some(b) = app.input("s-sens").parent_element().and_then(|p| p.query_selector("b").ok().flatten()) {
        b.set_text_content(Some(&s));
    }
    // Read before show() tidies the address bar.
    let params = window().location().search().ok().and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok());
    let room = params.and_then(|p| p.get("room"));
    app.show("menu");

    // ?room=CODE opens the join screen with the code filled in.
    if let Some(code) = room {
        app.show("join");
        app.input("join-code").set_value(&code.to_uppercase());
    }

    let win: web_sys::EventTarget = window().into();
    let docu: web_sys::EventTarget = doc.clone().into();

    let sh = shared.clone();
    let d2 = doc.clone();
    listen(&win, "keydown", move |e: KeyboardEvent| {
        let mut s = sh.borrow_mut();
        s.gesture = true;
        let code = e.code();
        let typing = d2
            .active_element()
            .is_some_and(|a| a.tag_name().eq_ignore_ascii_case("input") && a.get_attribute("type").as_deref() != Some("range"));
        if !typing {
            s.keys.insert(code.clone());
            let game_key = matches!(
                code.as_str(),
                "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Space" | "F1" | "Tab" | "KeyW" | "KeyA" | "KeyS" | "KeyD"
            );
            if game_key || (e.ctrl_key() && code == "KeyS") {
                e.prevent_default();
            }
        }
        if !e.repeat() {
            s.presses.push((code, e.ctrl_key()));
        }
    });
    let sh = shared.clone();
    listen(&win, "keyup", move |e: KeyboardEvent| {
        sh.borrow_mut().keys.remove(&e.code());
    });
    let sh = shared.clone();
    listen(&win, "blur", move |_: web_sys::FocusEvent| {
        let mut s = sh.borrow_mut();
        s.keys.clear();
        s.mouse_down = false;
        s.fire = false;
    });

    let canvas_t: web_sys::EventTarget = canvas.clone().into();
    let (sh, cv) = (shared.clone(), canvas.clone());
    listen(&win, "mousemove", move |e: MouseEvent| {
        let mut s = sh.borrow_mut();
        if s.locked {
            s.look.0 += e.movement_x() as f64;
            s.look.1 += e.movement_y() as f64;
        } else {
            s.mouse = canvas_point(&cv, e.client_x(), e.client_y());
        }
    });
    let (sh, cv) = (shared.clone(), canvas.clone());
    let chat_el = app.chat.clone();
    listen(&canvas_t, "mousedown", move |e: MouseEvent| {
        let mut s = sh.borrow_mut();
        s.gesture = true;
        if e.button() != 0 {
            return;
        }
        e.prevent_default();
        let _ = chat_el.blur();
        if s.locked {
            s.fire = true;
            return;
        }
        s.mouse = canvas_point(&cv, e.client_x(), e.client_y());
        // The sound icon still works; anywhere else grabs the pointer to play.
        if s.lock_wanted && !hit(SOUND_ICON, s.mouse) {
            cv.request_pointer_lock();
        } else {
            s.mouse_down = true;
        }
    });
    let (sh, cv) = (shared.clone(), canvas.clone());
    listen(&win, "mouseup", move |e: MouseEvent| {
        let mut s = sh.borrow_mut();
        if e.button() == 0 {
            if s.mouse_down && !s.locked {
                let p = canvas_point(&cv, e.client_x(), e.client_y());
                s.clicks.push(p);
            }
            s.mouse_down = false;
            s.fire = false;
        }
    });
    listen(&canvas_t, "contextmenu", |e: MouseEvent| e.prevent_default());
    let (sh, d2) = (shared.clone(), doc.clone());
    listen(&docu, "pointerlockchange", move |_: web_sys::Event| {
        let mut s = sh.borrow_mut();
        s.locked = d2.pointer_lock_element().is_some();
        s.fire = false;
        s.mouse_down = false;
    });

    // Menu buttons: any element with data-action.
    let sh = shared.clone();
    listen(&docu, "click", move |e: MouseEvent| {
        let mut s = sh.borrow_mut();
        s.gesture = true;
        let target = e.target().and_then(|t| t.dyn_into::<Element>().ok());
        if let Some(el) = target.and_then(|t| t.closest("[data-action]").ok().flatten()) {
            if let Some(a) = el.get_attribute("data-action") {
                s.actions.push(a);
            }
        }
    });
    // Slider labels show their value.
    listen(&docu, "input", move |e: web_sys::Event| {
        let Some(input) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) else { return };
        if input.type_() != "range" {
            return;
        }
        if let Some(b) = input.parent_element().and_then(|p| p.query_selector("b").ok().flatten()) {
            b.set_text_content(Some(&input.value()));
        }
    });

    // The frame loop.
    let app = Rc::new(RefCell::new(app));
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        app.borrow_mut().frame();
        let _ = window().request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }));
    let _ = window().request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref());
}
