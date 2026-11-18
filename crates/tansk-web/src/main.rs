//! Tansk in the browser. The menus are HTML over the original's art; the game
//! draws to a 1024x768 canvas. Online games run on the server; the
//! playground runs the same simulation right here.

mod assets;
mod net;
mod render;
mod render3d;

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use assets::{Audio, Images, Music};
use net::{Net, NetEvent};
use render::{ConsoleMsg, Explosion, Hud};
use tansk_sim::proto::{ClientMsg, Event, GameView, MsgLevel, Phase, PickupKind, ServerMsg, Settings, Weapon};
use tansk_sim::{Game, Input, V2, TICK_MS};
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, Document, Element, HtmlCanvasElement, HtmlElement, HtmlInputElement, KeyboardEvent,
    MouseEvent,
};

const TICK: f64 = TICK_MS as f64;
/// How far behind the newest server view the online game is drawn, in ticks.
const BUFFER_TICKS: f64 = 2.5;

fn window() -> web_sys::Window {
    web_sys::window().unwrap()
}

fn now() -> f64 {
    window().performance().map(|p| p.now()).unwrap_or(0.0)
}

fn storage_get(key: &str) -> Option<String> {
    window().local_storage().ok().flatten().and_then(|s| s.get_item(key).ok().flatten())
}

fn storage_set(key: &str, value: &str) {
    if let Some(s) = window().local_storage().ok().flatten() {
        let _ = s.set_item(key, value);
    }
}

/// What the DOM listeners hand to the frame loop.
#[derive(Default)]
struct Shared {
    keys: HashSet<String>,
    presses: Vec<(String, bool)>,
    mouse: V2,
    mouse_down: bool,
    clicks: Vec<V2>,
    actions: Vec<String>,
    gesture: bool,
}

/// The test arena (states/Play.java): you and three dummies.
struct Local {
    game: Game,
    me: u32,
    others: Vec<u32>,
    acc: f64,
    prev: GameView,
    cur: GameView,
}

impl Local {
    fn new() -> Self {
        let mut game = Game::new(Settings::playground(), (js_sys::Math::random() * 1e15) as u64);
        let me = game.add_player("Euler", 5);
        let others = vec![
            game.add_player("Newton", 1),
            game.add_player("Gauss", 2),
            game.add_player("Leibniz", 3),
        ];
        game.start_with_delay(3000);
        let cur = game.view();
        Local { game, me, others, acc: 0.0, prev: cur.clone(), cur }
    }
}

/// A game on the server.
struct Online {
    net: Net,
    first: Option<ClientMsg>,
    connected: bool,
    code: Option<String>,
    me: Option<u32>,
    host: bool,
    settings: Settings,
    views: VecDeque<GameView>,
    render_tick: f64,
    last_sent: Option<Input>,
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
    ctx: CanvasRenderingContext2d,
    chat: HtmlInputElement,
    shared: Rc<RefCell<Shared>>,
    images: Images,
    audio: Audio,
    mode: Mode,
    screen: &'static str,
    koth: bool,
    console: Vec<ConsoleMsg>,
    explosions: Vec<Explosion>,
    debug: bool,
    last_frame: f64,
    fps: (u32, f64, u32),
    rooms_fetched_at: f64,
    rooms: Rc<RefCell<Option<Vec<serde_json::Value>>>>,
    three: Option<render3d::Renderer3d>,
    view3d: bool,
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
        self.sync_view();
        if screen == "join" {
            self.rooms_fetched_at = -1e9;
            let _ = self.input("join-code").focus();
        }
        if !in_game {
            self.mode = Mode::Menu;
            self.audio.play_music(Music::Menu);
            let _ = window().history().map(|h| h.replace_state_with_url(&JsValue::NULL, "", Some("/")));
        }
    }

    fn uses_3d(&self) -> bool {
        self.view3d && self.three.is_some()
    }

    fn sync_view(&self) {
        let show = self.uses_3d() && self.screen.is_empty();
        if let Some(r) = &self.three {
            let _ = r.canvas.class_list().toggle_with_force("hidden", !show);
        }
        let _ = self.el("radio-3d").class_list().toggle_with_force("selected", self.view3d);
        let _ = self.el("radio-2d").class_list().toggle_with_force("selected", !self.view3d);
    }

    fn set_view(&mut self, three: bool) {
        self.view3d = three;
        storage_set("tansk.view", if three { "3d" } else { "2d" });
        self.sync_view();
    }

    /// The point on the map the mouse is over: in 3D, where its ray meets the ground.
    fn aim(&self, mouse: V2) -> V2 {
        match &self.three {
            Some(r) if self.view3d => r.unproject(mouse),
            _ => mouse,
        }
    }

    /// Draws the 3D world if that view is on; returns where the zone's bar goes.
    fn draw3d(&mut self, v: Option<&GameView>) -> Option<V2> {
        if !self.view3d {
            return None;
        }
        let r = self.three.as_mut()?;
        r.draw(&self.images, v, &self.explosions, now());
        v.and_then(|v| v.zone).map(|z| r.project([z.x, 75.0, z.y]))
    }

    fn back_to_menu(&mut self) {
        self.console.clear();
        self.explosions.clear();
        self.show("menu");
    }

    fn start_playground(&mut self) {
        self.console.clear();
        self.explosions.clear();
        self.mode = Mode::Playground(Box::new(Local::new()));
        self.show("");
        self.audio.play_music(Music::Battle);
        self.log("Playground: 1-6 swap weapons, Q drops a pickup.", MsgLevel::Info);
    }

    fn start_online(&mut self, first: ClientMsg) {
        self.console.clear();
        self.explosions.clear();
        match Net::connect() {
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
        self.console.push(ConsoleMsg {
            stamp: format!("{:02}:{:02}", d.get_hours(), d.get_minutes()),
            text: text.to_string(),
            color,
            at: now(),
        });
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
            "view-3d" => self.set_view(true),
            "view-2d" => self.set_view(false),
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
                let msg = ClientMsg::Create { name: self.name(), settings: self.host_settings() };
                self.start_online(msg);
            }
            "join-go" => {
                let code = self.input("join-code").value().trim().to_uppercase();
                if code.len() != 4 {
                    self.el("join-error").set_text_content(Some("Enter the game's four letter code."));
                    return;
                }
                self.start_online(ClientMsg::Join { code, name: self.name() });
            }
            _ => {
                if let Some(code) = a.strip_prefix("join-room:") {
                    self.input("join-code").set_value(code);
                    self.start_online(ClientMsg::Join { code: code.to_string(), name: self.name() });
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

    fn game_input(&self, sh: &Shared, aim: V2) -> Input {
        if self.typing() {
            return Input { aim, ..Input::default() };
        }
        let k = |c: &str| sh.keys.contains(c);
        Input {
            up: k("KeyW"),
            left: k("KeyA"),
            down: k("KeyS"),
            right: k("KeyD"),
            fire: sh.mouse_down,
            aim,
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
                            o.net.send(&ClientMsg::Chat(text));
                        }
                    }
                    self.chat.set_value("");
                    let _ = self.chat.blur();
                } else {
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
            "ArrowUp" => self.audio.set_volume(self.audio.volume + 0.05),
            "ArrowDown" => self.audio.set_volume(self.audio.volume - 0.05),
            "KeyS" if ctrl => self.audio.toggle_mute(),
            "KeyV" => self.set_view(!self.view3d),
            _ => {}
        }
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
                l.game.add_pickup(PickupKind::Weapon(Weapon::Slowspeedy), V2::new(800.0, 300.0));
            }
        }
    }

    fn click(&mut self, p: V2) {
        if !self.screen.is_empty() {
            return;
        }
        if render::hit(render::SOUND_ICON, p) {
            self.audio.toggle_mute();
            return;
        }
        let phase = self.current_phase();
        if phase == Some(Phase::GameOver) {
            if render::hit(render::MENU_BUTTON, p) {
                self.back_to_menu();
                return;
            }
            if render::hit(render::RESTART_BUTTON, p) {
                match &self.mode {
                    Mode::Playground(_) => self.start_playground(),
                    Mode::Online(o) if o.host => o.net.send(&ClientMsg::Restart),
                    _ => {}
                }
                return;
            }
        }
        if let Mode::Online(o) = &mut self.mode {
            if phase == Some(Phase::Lobby) {
                if o.host && render::hit(render::START_BUTTON, p) {
                    o.net.send(&ClientMsg::Start);
                }
                if render::hit(render::LINK_BOX, p) {
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

    fn events(&mut self, events: &[Event]) {
        for e in events {
            match e {
                Event::Sound(s) => self.audio.play(*s),
                Event::Explosion(pos) => self.explosions.push(Explosion { pos: *pos, started: now() }),
            }
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
        self.fit_stage();

        let (presses, clicks, actions, gesture) = {
            let mut sh = self.shared.borrow_mut();
            let g = std::mem::take(&mut sh.gesture);
            (std::mem::take(&mut sh.presses), std::mem::take(&mut sh.clicks), std::mem::take(&mut sh.actions), g)
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
        self.explosions.retain(|e| t - e.started < 36.0 * 50.0);

        let mode = std::mem::replace(&mut self.mode, Mode::Menu);
        self.mode = match mode {
            Mode::Menu => Mode::Menu,
            Mode::Playground(l) => self.frame_local(l, dt),
            Mode::Online(o) => self.frame_online(o, dt),
        };
    }

    fn fit_stage(&self) {
        let w = window();
        let iw = w.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1024.0);
        let ih = w.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(768.0);
        let s = (iw / 1024.0).min(ih / 768.0);
        let style = self.stage.style();
        let _ = style.set_property("left", &format!("{}px", (iw - 1024.0 * s) / 2.0));
        let _ = style.set_property("top", &format!("{}px", (ih - 768.0 * s) / 2.0));
        let _ = style.set_property("transform", &format!("scale({s})"));
    }

    /// Slider labels, the volume, and the open games list.
    fn sync_menus(&mut self) {
        if self.screen == "settings" {
            let v = self.input("s-volume").value().parse::<f32>().unwrap_or(35.0) / 100.0;
            if (v - self.audio.volume).abs() > 0.001 {
                self.audio.set_volume(v);
                storage_set("tansk.volume", &v.to_string());
            }
        } else {
            let v = (self.audio.volume * 100.0).round() as i32;
            self.input("s-volume").set_value(&v.to_string());
        }
        if self.screen == "join" {
            let t = now();
            if t - self.rooms_fetched_at > 3000.0 {
                self.rooms_fetched_at = t;
                fetch_rooms(self.rooms.clone());
            }
            if let Some(rooms) = self.rooms.borrow_mut().take() {
                render_rooms(&self.doc, &rooms);
            }
        }
    }

    fn frame_local(&mut self, mut l: Box<Local>, dt: f64) -> Mode {
        let (input, mouse, mouse_down, aim) = {
            let sh = self.shared.borrow();
            let aim = self.aim(sh.mouse);
            (self.game_input(&sh, aim), sh.mouse, sh.mouse_down, aim)
        };
        l.acc += dt;
        let mut events = Vec::new();
        while l.acc >= TICK {
            l.acc -= TICK;
            if l.cur.phase != Phase::GameOver {
                l.game.set_input(l.me, input);
                // As in the original's test arena, every turret follows the
                // mouse and Newton fires when you do.
                for (i, &o) in l.others.iter().enumerate() {
                    let fire = i == 0 && mouse_down;
                    l.game.set_input(o, Input { aim, fire, ..Input::default() });
                }
                l.game.step();
            }
            let next = l.game.view();
            events.extend(next.events.iter().copied());
            l.prev = std::mem::replace(&mut l.cur, next);
        }
        self.events(&events);
        let view = lerp_view(&l.prev, &l.cur, (l.acc / TICK) as f32);
        let zone_bar = self.draw3d(Some(&view));
        let mut hud = self.hud(Some(l.me), Settings::playground(), None, false, false, true, false, mouse, mouse_down);
        hud.world = !self.uses_3d();
        hud.zone_bar = zone_bar;
        render::draw(&self.ctx, &self.images, Some(&view), &hud);
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
                    ServerMsg::Welcome { code, you, host, settings } => {
                        let first_time = o.me.is_none();
                        o.code = Some(code.clone());
                        o.me = Some(you);
                        o.host = host;
                        o.settings = settings;
                        o.views.clear();
                        if first_time {
                            self.log("Joined game!", MsgLevel::Info);
                            self.audio.play_music(Music::Battle);
                            let _ = window()
                                .history()
                                .map(|h| h.replace_state_with_url(&JsValue::NULL, "", Some(&format!("/?room={code}"))));
                        }
                    }
                    ServerMsg::Rejected(reason) => {
                        o.closed = true;
                        self.show("join");
                        self.el("join-error").set_text_content(Some(&format!("Connection refused: {reason}")));
                        return Mode::Menu;
                    }
                    ServerMsg::Kicked => {
                        o.closed = true;
                        self.show("join");
                        self.el("join-error").set_text_content(Some("You have been kicked from the server."));
                        return Mode::Menu;
                    }
                    ServerMsg::Host(h) => o.host = h,
                    ServerMsg::Message { text, level } => self.log(&text, level),
                    ServerMsg::View(mut v) => {
                        events.append(&mut v.events);
                        o.views.push_back(v);
                        while o.views.len() > 32 {
                            o.views.pop_front();
                        }
                    }
                },
            }
        }
        self.events(&events);

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

        let (input, mouse, mouse_down, aim) = {
            let sh = self.shared.borrow();
            let aim = self.aim(sh.mouse);
            (self.game_input(&sh, aim), sh.mouse, sh.mouse_down, aim)
        };
        let t = now();
        if o.me.is_some() && !o.closed && (o.last_sent != Some(input) && t - o.last_send_at >= TICK || t - o.last_send_at > 500.0) {
            o.net.send(&ClientMsg::Input(input));
            o.last_sent = Some(input);
            o.last_send_at = t;
        }

        // Your own turret follows the mouse right away rather than a round trip later.
        let own_turret = view.as_ref().and_then(|v| {
            let t = v.tanks.iter().find(|t| Some(t.player) == o.me)?;
            let tp = t.turret_pos;
            Some((((tp.x - aim.x) as f64).atan2((tp.y - aim.y) as f64).to_degrees() * -1.0 + 180.0) as f32)
        });
        let show_info = view.as_ref().is_some_and(|v| {
            v.phase == Phase::Lobby || (v.phase == Phase::Countdown && v.round == 0 && v.delay_ms >= 3000)
        });
        let link = share_link(o.code.as_deref());
        let copied = t - o.copied_at < 2000.0;
        let zone_bar = self.draw3d(view.as_ref());
        let mut hud = self.hud(o.me, o.settings, o.code.as_deref(), o.host, true, o.connected, show_info, mouse, mouse_down);
        hud.link = link.as_deref();
        hud.own_turret = own_turret;
        hud.copied = copied;
        hud.world = !self.uses_3d();
        hud.zone_bar = zone_bar;
        render::draw(&self.ctx, &self.images, view.as_ref(), &hud);
        Mode::Online(o)
    }

    #[allow(clippy::too_many_arguments)]
    fn hud<'a>(
        &'a self,
        me: Option<u32>,
        settings: Settings,
        code: Option<&'a str>,
        host: bool,
        online: bool,
        connected: bool,
        show_info: bool,
        mouse: V2,
        mouse_down: bool,
    ) -> Hud<'a> {
        let chat_open = self.doc.active_element().is_some_and(|e| e.id() == "chat");
        Hud {
            now: now(),
            me,
            settings,
            code,
            link: None,
            host,
            online,
            show_info,
            connected,
            console: &self.console,
            chat_open,
            explosions: &self.explosions,
            mouse,
            mouse_down,
            sound_icon: self.audio.icon(),
            own_turret: None,
            debug: self.debug.then(|| {
                vec![
                    format!("FPS: {}", self.fps.2),
                    format!("Explosions: {}", self.explosions.len()),
                    format!("Volume: {:.2}", self.audio.volume),
                ]
            }),
            copied: false,
            world: true,
            zone_bar: None,
        }
    }
}

fn share_link(code: Option<&str>) -> Option<String> {
    let origin = window().location().origin().ok()?;
    Some(format!("{origin}/?room={}", code?))
}

/// The view at a fractional tick, between the two views around it.
fn sample(views: &VecDeque<GameView>, tick: f64) -> GameView {
    let first = views.front().unwrap();
    if tick <= first.tick as f64 {
        return first.clone();
    }
    for w in views.iter().zip(views.iter().skip(1)) {
        let (a, b) = w;
        if (a.tick as f64) <= tick && tick < b.tick as f64 {
            let span = (b.tick - a.tick).max(1) as f64;
            return lerp_view(a, b, ((tick - a.tick as f64) / span) as f32);
        }
    }
    views.back().unwrap().clone()
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let d = ((b - a + 540.0) % 360.0) - 180.0;
    a + d * t
}

fn lerp_v(a: V2, b: V2, t: f32) -> V2 {
    if a.sub(b).len2() > 120.0 * 120.0 {
        return b;
    }
    V2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// `b`, with tanks and projectiles moved back toward where they were in `a`.
fn lerp_view(a: &GameView, b: &GameView, t: f32) -> GameView {
    let t = t.clamp(0.0, 1.0);
    let mut v = b.clone();
    v.events.clear();
    for tk in &mut v.tanks {
        if let Some(o) = a.tanks.iter().find(|o| o.id == tk.id) {
            tk.pos = lerp_v(o.pos, tk.pos, t);
            tk.turret_pos = lerp_v(o.turret_pos, tk.turret_pos, t);
            tk.theta = lerp_angle(o.theta, tk.theta, t);
            tk.turret_angle = lerp_angle(o.turret_angle, tk.turret_angle, t);
        }
    }
    for p in &mut v.projectiles {
        if let Some(o) = a.projectiles.iter().find(|o| o.id == p.id) {
            p.pos = lerp_v(o.pos, p.pos, t);
            p.theta = lerp_angle(o.theta, p.theta, t);
        }
    }
    v
}

fn fetch_rooms(into: Rc<RefCell<Option<Vec<serde_json::Value>>>>) {
    wasm_bindgen_futures::spawn_local(async move {
        let Ok(resp) = wasm_bindgen_futures::JsFuture::from(window().fetch_with_str("/api/rooms")).await else { return };
        let Ok(resp) = resp.dyn_into::<web_sys::Response>() else { return };
        let Ok(p) = resp.text() else { return };
        let Ok(text) = wasm_bindgen_futures::JsFuture::from(p).await else { return };
        let rooms: Vec<serde_json::Value> = serde_json::from_str(&text.as_string().unwrap_or_default()).unwrap_or_default();
        *into.borrow_mut() = Some(rooms);
    });
}

fn render_rooms(doc: &Document, rooms: &[serde_json::Value]) {
    let Some(list) = doc.get_element_by_id("room-list") else { return };
    list.set_inner_html("");
    let open: Vec<&serde_json::Value> = rooms.iter().filter(|r| r["started"] == false).collect();
    if open.is_empty() {
        let e = doc.create_element("div").unwrap();
        e.set_class_name("empty");
        e.set_text_content(Some("No open games right now. Host one!"));
        let _ = list.append_child(&e);
        return;
    }
    for r in open {
        let code = r["code"].as_str().unwrap_or("");
        let e = doc.create_element("div").unwrap();
        e.set_class_name("room");
        let _ = e.set_attribute("data-action", &format!("join-room:{code}"));
        let left = doc.create_element("span").unwrap();
        left.set_text_content(Some(&format!(
            "{code}  {}'s game",
            r["host"].as_str().unwrap_or("?")
        )));
        let right = doc.create_element("span").unwrap();
        right.set_text_content(Some(&format!(
            "{}  {}/{}",
            if r["koth"] == true { "King of the hill" } else { "Standard" },
            r["players"],
            r["max_players"]
        )));
        let _ = e.append_child(&left);
        let _ = e.append_child(&right);
        let _ = list.append_child(&e);
    }
}

// ------------------------------------------------------------------ setup

fn listen<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    f: impl FnMut(E) + 'static,
) {
    let c = Closure::<dyn FnMut(E)>::new(f);
    let _ = target.add_event_listener_with_callback(name, c.as_ref().unchecked_ref());
    c.forget();
}

fn canvas_point(canvas: &HtmlCanvasElement, x: i32, y: i32) -> V2 {
    let r = canvas.get_bounding_client_rect();
    let sx = 1024.0 / r.width().max(1.0);
    let sy = 768.0 / r.height().max(1.0);
    V2::new(((x as f64 - r.left()) * sx) as f32, ((y as f64 - r.top()) * sy) as f32)
}

fn main() {
    console_error_panic_hook::set_once();
    let doc = window().document().unwrap();
    let canvas: HtmlCanvasElement = doc.get_element_by_id("game").unwrap().dyn_into().unwrap();
    let ctx: CanvasRenderingContext2d = canvas.get_context("2d").unwrap().unwrap().dyn_into().unwrap();
    let stage: HtmlElement = doc.get_element_by_id("stage").unwrap().dyn_into().unwrap();
    let chat: HtmlInputElement = doc.get_element_by_id("chat").unwrap().dyn_into().unwrap();
    let shared: Rc<RefCell<Shared>> = Rc::default();

    let volume = storage_get("tansk.volume").and_then(|v| v.parse().ok()).unwrap_or(0.35);
    let images = Images::default();
    images.preload(&["map", "background", "tank_spawnpoint", "horizontal_wall", "vertical_wall", "explosion_animation", "shield"]);

    let mut app = App {
        doc: doc.clone(),
        stage,
        ctx,
        chat,
        shared: shared.clone(),
        images,
        audio: Audio::new(volume),
        mode: Mode::Menu,
        screen: "menu",
        koth: false,
        console: Vec::new(),
        explosions: Vec::new(),
        debug: false,
        last_frame: now(),
        fps: (0, now(), 0),
        rooms_fetched_at: -1e9,
        rooms: Rc::default(),
        three: doc
            .get_element_by_id("game3d")
            .and_then(|c| c.dyn_into::<HtmlCanvasElement>().ok())
            .and_then(render3d::Renderer3d::new),
        view3d: storage_get("tansk.view").as_deref() != Some("2d"),
    };
    app.input("player-name").set_value(&storage_get("tansk.name").unwrap_or_else(|| "Unnamed".into()));
    app.show("menu");

    // ?room=CODE opens the join screen with the code filled in.
    let params = window().location().search().ok().and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok());
    if let Some(code) = params.and_then(|p| p.get("room")) {
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
                "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Space" | "F1" | "KeyW" | "KeyA" | "KeyS" | "KeyD"
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
    });

    let canvas_t: web_sys::EventTarget = canvas.clone().into();
    let (sh, cv) = (shared.clone(), canvas.clone());
    listen(&win, "mousemove", move |e: MouseEvent| {
        sh.borrow_mut().mouse = canvas_point(&cv, e.client_x(), e.client_y());
    });
    let (sh, cv) = (shared.clone(), canvas.clone());
    let chat_el = app.chat.clone();
    listen(&canvas_t, "mousedown", move |e: MouseEvent| {
        let mut s = sh.borrow_mut();
        s.gesture = true;
        s.mouse = canvas_point(&cv, e.client_x(), e.client_y());
        if e.button() == 0 {
            s.mouse_down = true;
            let _ = chat_el.blur();
            e.prevent_default();
        }
    });
    let (sh, cv) = (shared.clone(), canvas.clone());
    listen(&win, "mouseup", move |e: MouseEvent| {
        let mut s = sh.borrow_mut();
        if e.button() == 0 {
            if s.mouse_down {
                let p = canvas_point(&cv, e.client_x(), e.client_y());
                s.clicks.push(p);
            }
            s.mouse_down = false;
        }
    });
    listen(&canvas_t, "contextmenu", |e: MouseEvent| e.prevent_default());

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
