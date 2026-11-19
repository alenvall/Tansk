//! The game loop, shared by the web and native shells: title backdrop,
//! menus, joining, input, camera, events into effects / sound / HUD /
//! radio, and the end screen.
//!
//! The platform feeds [`Controls`], calls [`Game::frame`] once per frame
//! and draws the returned world; it also honours a few requests
//! ([`Game::wants_pointer`], [`Game::want_rooms`], [`Game::settings_dirty`]).

// Some hooks (pointer lock, rooms list, saving settings) are web-only.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

pub mod aim;
pub mod controls;
pub mod juice;
pub mod names;
pub mod radio;
pub mod sound;

use std::collections::HashMap;

use arena_core::events::{HitLocation, RepairKind};
use arena_core::map::Map;
use arena_core::net::Join;
use arena_core::snapshot::{Snapshot, TankSnap};
use arena_core::raid::{ModeKind, RaidPhase, TEAM_RAIDERS};
use arena_core::{Event, Input, Phase, PlayerId};
use glam::Vec3;

use crate::camera::{Camera, ChaseCam};
use crate::demo::{Demo, MapCollider};
use crate::net::{self, ChatLine, NetClient, Status};
use crate::ui::{BranchOption, EndRow, HitKind, OpenGame, ScoreRow, Settings, Team, TrackState, Ui, UiAction};
use crate::world::{paint_for, RenderWorld, TankView, WorldRenderer};

pub use controls::Controls;
use juice::Juice;
use radio::Radio;
use sound::Sound;

/// The backdrop and Practice share this map seed, so starting Practice
/// doesn't rebuild the scenery.
const HOME_SEED: u64 = 7;
const PRACTICE_BOTS: usize = 7;
const PRACTICE_DIFFICULTY: f32 = 0.55;
const RAID_ALLIES: usize = 3;
const BRANCH_SECS: f32 = 4.0;
/// Seconds a blown track waits before the crew starts patching (tuning:
/// track_repair_time - track_patch_duration).
const TRACK_WAIT: f32 = 2.5;
const TRACK_PATCH: f32 = 1.5;

/// Test hooks from URL flags; they only touch the offline Practice sim.
#[derive(Clone, Debug, Default)]
pub struct DebugFlags {
    /// `?matchtime=45`: Scrap Brawl length, seconds.
    pub match_time: Option<f32>,
    /// `?raidfast=1`: waves at once, a boss every wave, a weak outpost.
    pub raid_fast: bool,
    /// `?scrap=1`: a Powder scrap every second until tier 2 (branch offer).
    pub scrap: bool,
    /// `?flare=1`: start holding an air-strike flare.
    pub flare: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Loading,
    InGame,
    End,
}

#[derive(Clone, Debug, PartialEq)]
enum Session {
    Practice { raid: bool },
    Online(Join),
}

/// Per-match bookkeeping for the HUD and the radio.
#[derive(Default)]
struct Match {
    aligned: bool,
    alive: bool,
    killer: Option<PlayerId>,
    killed_by: Option<(String, String)>,
    first_blood: bool,
    streaks: HashMap<PlayerId, u32>,
    bounty: HashMap<PlayerId, u8>,
    bounty_check: f32,
    /// Seconds left on an open branch offer.
    branch: Option<f32>,
    /// A pick held on the input until the sim takes it.
    choice: Option<(u8, f32)>,
    track_blown_at: [Option<f32>; 2],
    board_refresh: f32,
    chat_seen: usize,
    chat_last: Option<ChatLine>,
    ended: bool,
    debug_scrap_t: f32,
    names: HashMap<PlayerId, String>,
    /// Build the podium on the next frame (the end screen just opened).
    podium_pending: bool,
    /// Raid: the mode label last written, the outpost's max health, and the
    /// last outpost quarter the radio called out.
    raid_label: String,
    outpost_max: f32,
    outpost_quarter: u8,
}

pub struct Game {
    pub ui: Ui,
    pub controls: Controls,
    pub settings: Settings,
    /// `?mouse=free`: no pointer lock, mouse always steers the camera.
    pub free_mouse: bool,
    /// The pause menu is open (pointer released mid-match).
    pub paused: bool,
    pub debug: DebugFlags,
    juice: Juice,
    heart: f32,
    sound: Sound,
    screen: Screen,
    backdrop: Demo,
    orbit_centre: Vec3,
    net: Option<NetClient>,
    session: Option<Session>,
    map: Map,
    map_seed: u64,
    map_built: bool,
    chase: ChaseCam,
    camera: Option<Camera>,
    time: f32,
    input: Input,
    want_rooms: bool,
    settings_dirty: bool,
    m: Match,
    radio: Radio,
    status_note: String,
    /// The end-screen podium world (its statics replace the map's).
    podium: Option<RenderWorld>,
}

impl Game {
    pub fn new() -> Self {
        let backdrop = Demo::new(HOME_SEED, PRACTICE_BOTS);
        let map = Map::the_sink(HOME_SEED);
        let mut ui = Ui::mount();
        ui.set_ticker("Scrap Brawl on The Sink  //  three of a kind bolts on  //  shoot it off, take it home");
        Game {
            ui,
            controls: Controls::default(),
            settings: Settings::default(),
            free_mouse: false,
            paused: false,
            sound: Sound::new(),
            screen: Screen::Title,
            backdrop,
            orbit_centre: Vec3::ZERO,
            net: None,
            session: None,
            map,
            map_seed: HOME_SEED,
            map_built: false,
            chase: ChaseCam::default(),
            camera: None,
            time: 0.0,
            input: Input::default(),
            want_rooms: false,
            settings_dirty: false,
            m: Match::default(),
            radio: Radio::default(),
            status_note: String::new(),
            podium: None,
            debug: DebugFlags::default(),
            juice: Juice::default(),
            heart: 0.0,
        }
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The platform should hold the pointer lock now.
    pub fn wants_pointer(&self) -> bool {
        self.screen == Screen::InGame && !self.paused
    }

    /// A user gesture happened (click / key): browsers only start audio then.
    pub fn gesture(&mut self) {
        self.sound.unlock();
    }

    /// The pointer lock went away by itself (Esc): open the pause menu.
    pub fn pointer_lost(&mut self) {
        self.controls.captured = self.free_mouse;
        self.controls.release_all();
        if self.screen == Screen::InGame {
            self.paused = true;
        }
    }

    pub fn pointer_locked(&mut self) {
        self.controls.captured = true;
        self.paused = false;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// The platform should fetch `/api/arena/rooms` and call [`Game::set_rooms`].
    pub fn want_rooms(&mut self) -> bool {
        std::mem::take(&mut self.want_rooms)
    }

    /// Settings changed in the menu: the platform may persist them.
    pub fn settings_dirty(&mut self) -> bool {
        std::mem::take(&mut self.settings_dirty)
    }

    pub fn apply_settings(&mut self, s: Settings) {
        self.sound.set_volumes(s.vol_master, s.vol_sfx);
        self.ui.apply_settings(&s);
        self.settings = s;
    }

    /// `(code, humans, max_humans)` of the open public rooms.
    pub fn set_rooms(&mut self, rooms: &[(String, u32, u32)]) {
        let games: Vec<OpenGame> = rooms
            .iter()
            .map(|(code, n, max)| OpenGame {
                host: format!("Room {code}"),
                mode: "Scrap Brawl".into(),
                map: "The Sink".into(),
                players: *n,
                max_players: *max,
                ping_ms: 0,
                code: code.clone(),
            })
            .collect();
        self.ui.set_open_games(&games);
    }

    pub fn start_practice(&mut self) {
        self.start(Session::Practice { raid: false });
    }

    /// Offline Raider Waves with bot allies.
    pub fn start_practice_raid(&mut self) {
        self.start(Session::Practice { raid: true });
    }

    pub fn start_online(&mut self, join: Join) {
        self.start(Session::Online(join));
    }

    /// Puts the map's statics back after the podium replaced them.
    fn close_podium(&mut self) {
        if self.podium.take().is_some() {
            self.map_built = false;
        }
    }

    fn start(&mut self, session: Session) {
        if let Some(n) = &mut self.net {
            n.close();
        }
        self.close_podium();
        self.m = Match::default();
        self.paused = false;
        self.ui.hide_endscreen();
        self.ui.hide_branch();
        self.ui.show_respawn(None, None);
        self.ui.show_scoreboard(false, &[]);
        let net = match &session {
            Session::Practice { raid: false } => {
                self.ui.show_loading("Rolling out", "Seven bots, no mercy. Warming the engines.", Some(0.6));
                NetClient::practice(rand_seed(self.time), PRACTICE_BOTS, PRACTICE_DIFFICULTY)
            }
            Session::Practice { raid: true } => {
                self.ui.show_loading("Manning the outpost", "Three allies, a lot of raiders. Dig in.", Some(0.6));
                NetClient::practice_raid(rand_seed(self.time), RAID_ALLIES, PRACTICE_DIFFICULTY)
            }
            Session::Online(join) => {
                let sub = match join {
                    Join::Code(c) => format!("Tuning the radio to {c}..."),
                    Join::Host { mode: ModeKind::RaiderWaves, .. } => "Fortifying the outpost...".to_string(),
                    Join::Host { .. } => "Opening the gates...".to_string(),
                    _ => "Looking for a crew to fight...".to_string(),
                };
                self.ui.show_loading("Connecting", &sub, Some(0.2));
                NetClient::connect(&net::transport::default_url(), &self.settings.name, join.clone())
            }
        };
        self.ui.set_loading_tip(tip(self.time));
        self.net = Some(net);
        self.apply_debug();
        self.session = Some(session);
        self.screen = Screen::Loading;
    }

    fn apply_debug(&mut self) {
        let d = self.debug.clone();
        let Some(sim) = self.net.as_mut().and_then(|n| n.practice_sim()) else { return };
        if let Some(t) = d.match_time {
            sim.tuning.mode.time_limit = t;
            sim.time_left = t;
        }
        if d.raid_fast {
            sim.tuning.raid.boss_every = 1;
            sim.tuning.raid.breather = 4.0;
            sim.tuning.raid.first_wave_delay = 3.0;
            if let Some(r) = &mut sim.raid {
                r.timer = r.timer.min(3.0);
                r.health = r.max_health * 0.3;
            }
        }
        if d.flare {
            for p in &mut sim.players {
                if p.brain.is_none() {
                    p.tank.air_strike = true;
                }
            }
        }
    }

    /// `?scrap=1`: feeds Powder until tier 2 so a branch offer opens.
    fn debug_scrap(&mut self, dt: f32) {
        if !self.debug.scrap {
            return;
        }
        self.m.debug_scrap_t += dt;
        if self.m.debug_scrap_t < 1.0 {
            return;
        }
        self.m.debug_scrap_t = 0.0;
        let you = self.you();
        let Some(sim) = self.net.as_mut().and_then(|n| n.practice_sim()) else { return };
        if let Some(j) = sim.players.iter().position(|p| Some(p.id) == you) {
            if sim.players[j].tank.tiers[1] < 2 && sim.players[j].tank.alive {
                sim.add_scrap(j, arena_core::ScrapKind::Powder);
            }
        }
    }

    /// Back to the title screen.
    pub fn leave(&mut self) {
        if let Some(n) = &mut self.net {
            n.close();
        }
        self.net = None;
        self.session = None;
        self.paused = false;
        self.close_podium();
        self.screen = Screen::Title;
        self.sound.clear_engines();
        self.ui.hide_endscreen();
        self.ui.hide_branch();
        self.ui.show_respawn(None, None);
        self.ui.show_scoreboard(false, &[]);
        self.ui.show_title();
    }

    fn you(&self) -> Option<PlayerId> {
        self.net.as_ref().and_then(|n| n.state().you)
    }

    fn name_of(&self, id: PlayerId) -> String {
        if Some(id) == self.you() {
            return self.settings.name.clone();
        }
        self.net
            .as_ref()
            .and_then(|n| n.state().name_of(id))
            .map(str::to_string)
            .or_else(|| self.m.names.get(&id).cloned())
            .unwrap_or_else(|| "a raider".to_string())
    }

    fn handle_actions(&mut self) {
        for a in self.ui.poll_actions() {
            match a {
                UiAction::Play => self.start_online(Join::Play),
                UiAction::Practice => self.start_practice(),
                UiAction::Host(cfg) => {
                    let mode = if cfg.mode == "raider_waves" { ModeKind::RaiderWaves } else { ModeKind::ScrapBrawl };
                    self.start_online(Join::Host { public: !cfg.private, mode });
                }
                UiAction::Join { code } => self.start_online(Join::Code(code)),
                UiAction::RefreshGames => self.want_rooms = true,
                UiAction::Cancel => self.leave(),
                UiAction::SettingsChanged(s) => {
                    self.apply_settings(s);
                    self.settings_dirty = true;
                }
                UiAction::GarageChanged(_) => {}
                UiAction::BranchPicked(i) => self.pick_branch(i),
                UiAction::Chat(text) => {
                    if let Some(n) = &mut self.net {
                        n.chat(&text);
                    }
                }
                UiAction::Again => match self.session.clone() {
                    Some(Session::Practice { raid }) => self.start(Session::Practice { raid }),
                    None => self.start_practice(),
                    Some(Session::Online(_)) => {
                        // The server rolls the next round by itself.
                        self.close_podium();
                        self.ui.hide_endscreen();
                        self.m.ended = false;
                        self.screen = Screen::InGame;
                    }
                },
                UiAction::Leave => self.leave(),
            }
        }
    }

    fn pick_branch(&mut self, i: u8) {
        if self.m.branch.take().is_some() {
            self.m.choice = Some((i, 0.5));
        }
    }

    /// One frame: `now` a monotonic clock in seconds, `dt` the frame time.
    pub fn frame(&mut self, now: f64, dt: f32, wr: &mut WorldRenderer) -> RenderWorld {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        if !self.map_built && self.podium.is_none() {
            wr.set_map(&self.map);
            self.map_built = true;
        }
        self.handle_actions();
        self.ui.tick(dt);
        self.radio.tick(dt);

        let world = match self.screen {
            Screen::Title => self.title_frame(dt, wr),
            Screen::Loading => self.loading_frame(now, dt, wr),
            Screen::InGame | Screen::End => self.game_frame(now, dt, wr),
        };
        if let Some(line) = self.radio.take() {
            self.ui.radio(&line);
        }
        self.controls.end_frame();
        world
    }

    fn use_map(&mut self, seed: u64, wr: &mut WorldRenderer) {
        if seed != self.map_seed {
            self.map = Map::the_sink(seed);
            self.map_seed = seed;
            wr.set_map(&self.map);
        }
    }

    /// The live backdrop: a demo match under a slow orbit.
    fn title_frame(&mut self, dt: f32, wr: &mut WorldRenderer) -> RenderWorld {
        self.use_map(HOME_SEED, wr);
        let mut world = self.backdrop.advance(dt, wr).clone();
        if let Some(t) = self.backdrop.follow.and_then(|f| world.tanks.iter().find(|t| t.player == f)) {
            let k = 1.0 - (-dt * 0.6).exp();
            if self.orbit_centre == Vec3::ZERO {
                self.orbit_centre = t.pos;
            }
            self.orbit_centre += (t.pos - self.orbit_centre) * k;
            let mut cam = Camera::orbit(self.orbit_centre, 19.0, 5.5, self.time * 0.6, 0.0);
            cam.fov_y = 44f32.to_radians();
            world.camera = Some(cam);
        }
        self.sound.frame(world.camera.map_or(Vec3::ZERO, |c| c.eye), Vec3::X, &[], None, 0.0);
        world
    }

    fn loading_frame(&mut self, now: f64, dt: f32, wr: &mut WorldRenderer) -> RenderWorld {
        let Some(net) = &mut self.net else {
            self.leave();
            return self.title_frame(dt, wr);
        };
        net.update(now);
        let st = net.state();
        let (status, seed, has_world, code) = (st.status.clone(), st.seed, st.world.is_some(), st.code.clone());
        match &status {
            Status::Closed(why) => {
                let why = why.clone();
                if self.status_note != why {
                    self.ui.show_loading("No signal", &why, Some(0.0));
                    self.status_note = why;
                }
            }
            Status::Connecting | Status::Reconnecting => {}
            Status::Practice | Status::Playing => {
                if let (Some(seed), true) = (seed, has_world) {
                    self.status_note.clear();
                    self.use_map(seed, wr);
                    self.screen = Screen::InGame;
                    self.ui.show_hud();
                    self.ui.set_ffa(true);
                    let mode = match (&self.session, &code) {
                        (Some(Session::Practice { .. }), _) | (_, None) => "Practice \u{b7} The Sink".to_string(),
                        (_, Some(code)) => format!("Scrap Brawl \u{b7} The Sink \u{b7} {code}"),
                    };
                    self.ui.set_mode_label(&mode);
                    self.ui.set_scoreboard_title("Scoreboard", &mode);
                    if let Some(code) = &code {
                        self.ui.chat_system(&format!("On the band in room {code}. Enter to talk."));
                    }
                    return self.game_frame(now, dt, wr);
                }
            }
        }
        // Keep the backdrop running behind the card.
        self.title_frame(dt, wr)
    }

    fn game_frame(&mut self, now: f64, dt: f32, wr: &mut WorldRenderer) -> RenderWorld {
        let you = self.you();
        // ---- Input, from last frame's camera.
        let typing = self.ui.chat_open();
        let live = self.screen == Screen::InGame && !self.paused && !typing && self.controls.captured;
        let sens = self.settings.mouse_sens.clamp(0.1, 3.0);
        let fov_k = self.camera.map_or(1.0, |c| c.fov_y / 62f32.to_radians());
        if self.controls.captured && !self.paused {
            let (mx, my) = self.controls.mouse;
            self.chase.yaw -= mx * 0.0024 * sens * fov_k;
            self.chase.pitch = (self.chase.pitch - my * 0.0024 * sens * fov_k).clamp(-0.75, 0.5);
        }
        if !typing && !self.paused {
            self.chase.yaw += self.controls.axis(&["ArrowRight"], &["ArrowLeft"]) * 1.8 * dt;
            self.chase.pitch =
                (self.chase.pitch + self.controls.axis(&["ArrowDown"], &["ArrowUp"]) * 1.0 * dt).clamp(-0.75, 0.5);
        }
        let c = &self.controls;
        let mut input = Input { aim_point: self.input.aim_point, ..Input::default() };
        if live {
            input.throttle = c.axis(&["KeyS"], &["KeyW"]);
            input.steer = c.axis(&["KeyA"], &["KeyD"]);
            input.fire = c.buttons[0];
            input.alt_fire = c.buttons[2];
            input.boost = c.any(&["ShiftLeft", "ShiftRight"]);
            input.special = c.any(&["KeyF", "KeyE"]);
        }
        let zoom = live && c.any(&["ControlLeft", "ControlRight", "KeyZ"]);
        if let Some((i, left)) = &mut self.m.choice {
            input.choice = Some(*i);
            *left -= dt;
            if *left <= 0.0 {
                self.m.choice = None;
            }
        }
        // A camera aim even before the first frame's ray.
        if input.aim_point == Vec3::ZERO {
            input.aim_point = Vec3::new(1000.0, 0.0, 0.0);
        }

        let Some(net) = &mut self.net else {
            self.leave();
            return self.title_frame(dt, wr);
        };
        net.send_input(input);
        net.update(now);
        let events = net.drain_events();
        let st = net.state();
        if let Status::Closed(why) = &st.status {
            let why = why.clone();
            self.leave();
            self.ui.show_loading("Signal lost", &why, Some(0.0));
            self.screen = Screen::Loading;
            self.status_note = why;
            return self.title_frame(dt, wr);
        }
        let Some(snap) = st.world.clone() else { return RenderWorld::default() };
        let rtt = st.rtt_ms;
        for p in &st.players {
            if !self.m.names.contains_key(&p.id) {
                self.m.names.insert(p.id, p.name.clone());
            }
        }
        self.sync_chat();

        self.debug_scrap(dt);
        let mut world = RenderWorld::from_snapshot(&snap, &paint_for);
        world.time = self.time;
        world.dt = dt * self.juice.time_scale(dt);
        let me = you.and_then(|y| snap.tank(y)).cloned();
        let my_view = you.and_then(|y| world.tanks.iter().find(|t| t.player == y)).cloned();

        // ---- Camera.
        let alive = me.as_ref().is_some_and(|t| t.alive);
        if let Some(v) = &my_view {
            if (alive && !self.m.alive) || !self.m.aligned {
                // Spawned: look along the hull.
                let fwd = v.rot * Vec3::X;
                self.chase.yaw = (-fwd.z).atan2(fwd.x);
                self.chase.pitch = -0.12;
                self.m.aligned = true;
            }
        }
        if alive && !self.m.alive {
            self.ui.show_respawn(None, None);
            self.m.killer = None;
        }
        self.m.alive = alive;
        let killer_view = self.m.killer.and_then(|k| world.tanks.iter().find(|t| t.player == k && t.alive)).cloned();
        let pivot = match (&my_view, alive, &killer_view) {
            (_, false, Some(k)) => {
                // Spectate the killer: drift round to look where it looks.
                let fwd = k.turret_transform().transform_vector3(Vec3::X);
                let want = (-fwd.z).atan2(fwd.x);
                self.chase.yaw += wrap(want - self.chase.yaw) * (1.0 - (-dt * 1.5).exp());
                k.pos
            }
            (Some(v), _, _) => v.pos,
            _ => self.camera.map_or(Vec3::ZERO, |c| c.target),
        };
        self.chase.zoom_target = if zoom && alive { 1.0 } else { 0.0 };
        let mut cam = self.chase.update(dt, self.time, pivot, &MapCollider(&self.map));
        let long_gun = me.as_ref().is_some_and(|t| t.gun == arena_core::GunKind::LongGun);
        if long_gun {
            cam.fov_y *= 1.0 - 0.5 * self.chase.zoom;
        }
        self.camera = Some(cam);
        // Feel, on the drawn camera only (the aim keeps the rig's line).
        let (boosting, lag) = match (&my_view, alive) {
            (Some(v), true) => {
                let tf = v.turret_transform().transform_vector3(Vec3::X);
                (v.boosting, wrap((-tf.z).atan2(tf.x) - self.chase.yaw))
            }
            _ => (false, 0.0),
        };
        if let Some(t) = &me {
            self.juice.track_landing(t.linvel[1]);
        }
        world.camera = Some(self.juice.apply(cam, dt, boosting, lag, self.chase.zoom));
        // ---- Aim: the screen centre onto the world.
        let dir = (cam.target - cam.eye).normalize_or(Vec3::X);
        let skip = (pivot + Vec3::Y * self.chase.pivot_height - cam.eye).dot(dir).max(0.0);
        self.input.aim_point = aim::cast(&self.map, &world.tanks, you, cam.eye, dir, skip);

        // ---- Events: effects, shake, sound, HUD, radio.
        let listener = my_view.as_ref().map_or(cam.eye, |v| v.pos);
        for e in &events {
            let trauma = wr.event(e, cam.eye);
            self.chase.add_trauma(trauma);
            if let Event::UpgradeApplied { player, .. } | Event::PartAttached { player, .. } = e {
                if let Some(t) = world.tanks.iter().find(|t| t.player == *player) {
                    wr.fx.weld(t.pos + Vec3::Y * 1.5);
                }
            }
            if let Event::Fired { player, weapon, .. } = e {
                if Some(*player) == you {
                    match weapon {
                        arena_core::Weapon::Gun(g) => {
                            let shake = self.juice.fire(*g);
                            self.chase.add_trauma(shake);
                        }
                        _ => self.juice.small_fire(),
                    }
                }
            }
            self.sound.event(e, you);
            self.on_event(e, you, &world, cam);
        }
        self.sound.frame(listener, dir, &world.tanks, you, input.throttle);

        // ---- HUD.
        if let Some(t) = &me {
            self.hud(t, &snap, dt);
            let hp = t.health as f32 / f32::from(t.max_health.max(1));
            if t.alive && hp < 0.3 {
                self.heart -= dt;
                if self.heart <= 0.0 {
                    self.sound.heartbeat(hp);
                    self.heart = 0.55 + hp * 1.5;
                }
            } else {
                self.heart = 0.0;
            }
        }
        self.raid_hud(&snap);
        if let Some(left) = &mut self.m.branch {
            *left -= dt;
            let closed = me.as_ref().is_some_and(|t| t.choice.is_none());
            if *left <= 0.0 || (closed && *left < BRANCH_SECS - 0.3) {
                self.m.branch = None;
                self.ui.hide_branch();
            }
        }
        if let Some(t) = &me {
            if !t.alive {
                let by = self.m.killed_by.clone();
                self.ui.show_respawn(
                    Some(t.respawn_in as f32 / 60.0),
                    by.as_ref().map(|(n, w)| (n.as_str(), Team::Ffa, w.as_str())),
                );
            }
        }
        let board = self.screen == Screen::InGame && !typing && self.controls.held("Tab");
        self.m.board_refresh -= dt;
        if board && self.m.board_refresh <= 0.0 {
            let rows = self.score_rows(&snap, rtt);
            self.ui.show_scoreboard(true, &rows);
            self.m.board_refresh = 0.5;
        } else if !board && self.m.board_refresh > -1000.0 {
            self.ui.show_scoreboard(false, &[]);
            self.m.board_refresh = -2000.0;
        }
        if self.snap_is_raid(&snap) {
            // Raiders grow too, but the bounty call is for crews.
        } else {
            self.bounty_watch(&world, dt);
        }
        if std::mem::take(&mut self.m.podium_pending) {
            let mut ids: Vec<(PlayerId, u16)> = snap
                .scores
                .iter()
                .filter(|s| !self.is_raider(s.player))
                .map(|s| (s.player, s.kills))
                .collect();
            ids.sort_by(|a, b| b.1.cmp(&a.1));
            let top: Vec<TankView> =
                ids.iter().filter_map(|(id, _)| world.tanks.iter().find(|t| t.player == *id).cloned()).take(3).collect();
            if !top.is_empty() {
                self.podium = Some(crate::world::podium(wr, &top));
            }
        }
        if self.screen == Screen::End {
            if let Some(p) = &self.podium {
                let mut pw = p.clone();
                pw.time = self.time;
                pw.dt = dt;
                return pw;
            }
        }
        // A round restarted online while we sat on the end screen.
        if self.screen == Screen::End && self.m.ended && matches!(snap.mode.phase, Phase::Playing) && snap.mode.ticks_left > 60 * 60
        {
            if let Some(Session::Online(_)) = self.session {
                self.close_podium();
                self.ui.hide_endscreen();
                self.m.ended = false;
                self.screen = Screen::InGame;
            }
        }
        world
    }

    fn sync_chat(&mut self) {
        let Some(net) = &self.net else { return };
        let chat = &net.state().chat;
        // Find where we left off (the log is capped, so search by content).
        let start = match &self.m.chat_last {
            Some(last) => chat.iter().rposition(|l| l == last).map_or(chat.len().min(self.m.chat_seen), |i| i + 1),
            None => 0,
        };
        let new: Vec<ChatLine> = chat[start.min(chat.len())..].to_vec();
        self.m.chat_seen = chat.len();
        self.m.chat_last = chat.last().cloned();
        for l in new {
            match &l.from {
                Some(from) => self.ui.chat_line(from, Team::Ffa, &l.text),
                None => self.ui.chat_system(&l.text),
            }
        }
    }

    fn hud(&mut self, t: &TankSnap, snap: &Snapshot, _dt: f32) {
        let ui = &mut self.ui;
        ui.set_health(t.health as f32 / f32::from(t.max_health.max(1)));
        let plates: Vec<f32> =
            t.parts.iter().filter(|p| names::track_of(p.kind) == Some(0)).map(|p| p.health as f32 / 255.0).collect();
        let armour = if t.shield > 0 {
            t.shield as f32 / 255.0
        } else if plates.is_empty() {
            0.0
        } else {
            plates.iter().sum::<f32>() / plates.len() as f32
        };
        ui.set_armour(armour);
        ui.set_boost(t.fuel as f32 / 255.0);
        let mut tracks = [TrackState::Ok; 2];
        for (side, ts) in tracks.iter_mut().enumerate() {
            if !t.tracks[side].blown {
                self.m.track_blown_at[side] = None;
                continue;
            }
            let since = self.time - *self.m.track_blown_at[side].get_or_insert(self.time);
            *ts = match t.repair {
                Some(r) if r.kind == RepairKind::Track(side as u8) => {
                    TrackState::Repairing { done_in: (1.0 - r.progress as f32 / 255.0) * TRACK_PATCH }
                }
                _ => TrackState::Blown { repair_in: (TRACK_WAIT - since).max(0.0) },
            };
        }
        ui.set_tracks(tracks);
        ui.set_scrap(t.scrap, t.tiers);
        let mut parts = ["-"; 4];
        for p in &t.parts {
            if let Some(i) = names::track_of(p.kind) {
                parts[i] = names::part_name(p.kind);
            }
        }
        parts[1] = names::part_name(names::gun_part(t.gun));
        ui.set_part_names(parts);
        ui.set_reticle(t.bloom as f32 / 255.0, 1.0 - t.reload as f32 / 255.0);
        ui.set_timer(snap.mode.ticks_left as f32 / 60.0);
        // Free-for-all: your kills against the leader's.
        let you = t.player;
        let mine = snap.scores.iter().find(|s| s.player == you).map_or(0, |s| s.kills);
        let best = snap.scores.iter().filter(|s| s.player != you).map(|s| s.kills).max().unwrap_or(0);
        ui.set_scores(u32::from(mine), u32::from(best));
    }

    fn score_rows(&self, snap: &Snapshot, rtt: f32) -> Vec<ScoreRow> {
        let you = self.you();
        let players = self.net.as_ref().map(|n| n.state().players.clone()).unwrap_or_default();
        let mut rows: Vec<ScoreRow> = players
            .iter()
            .filter(|p| p.team != Some(TEAM_RAIDERS))
            .map(|p| {
                let s = snap.scores.iter().find(|s| s.player == p.id);
                let t = snap.tank(p.id);
                let is_you = Some(p.id) == you;
                ScoreRow {
                    name: self.name_of(p.id),
                    team: Team::Ffa,
                    kills: s.map_or(0, |s| u32::from(s.kills)),
                    deaths: s.map_or(0, |s| u32::from(s.deaths)),
                    tiers: t.map_or([0; 4], |t| t.tiers),
                    ping_ms: if p.is_bot {
                        None
                    } else if is_you {
                        Some(rtt.round() as u32)
                    } else {
                        Some(0)
                    },
                    is_you,
                    dead: t.is_some_and(|t| !t.alive),
                }
            })
            .collect();
        rows.sort_by(|a, b| b.kills.cmp(&a.kills).then(a.deaths.cmp(&b.deaths)));
        rows
    }

    /// The radio calls out anyone who has grown into a monster.
    fn bounty_watch(&mut self, world: &RenderWorld, dt: f32) {
        self.m.bounty_check -= dt;
        if self.m.bounty_check > 0.0 {
            return;
        }
        self.m.bounty_check = 1.0;
        let you = self.you();
        let Some(net) = &self.net else { return };
        let Some(snap) = &net.state().world else { return };
        let me = you.and_then(|y| world.tanks.iter().find(|t| t.player == y)).map(|t| t.pos);
        let mut call = None;
        for t in &snap.tanks {
            if Some(t.player) == you || !t.alive {
                continue;
            }
            let total: u8 = t.tiers.iter().sum();
            let level = total.saturating_sub(7) / 2; // 9 tiers: level 1, 11: 2, ...
            let seen = self.m.bounty.entry(t.player).or_insert(0);
            if level > *seen {
                *seen = level;
                call = Some((t.player, t.parts.len(), t.position()));
            }
        }
        if let Some((p, parts, pos)) = call {
            let from = me.unwrap_or(Vec3::ZERO);
            let d = pos - from;
            let lines = radio::bounty(&self.name_of(p), parts, names::compass(d.x, d.z), d.length());
            self.radio.say(&lines, false);
        }
    }

    fn on_event(&mut self, e: &Event, you: Option<PlayerId>, world: &RenderWorld, cam: Camera) {
        let is_you = |p: PlayerId| Some(p) == you;
        let my_pos = you.and_then(|y| world.tanks.iter().find(|t| t.player == y)).map_or(cam.eye, |t| t.pos);
        match e {
            Event::Hit { target, attacker, location, .. } => {
                if attacker.is_some_and(is_you) && !is_you(*target) {
                    let kind = match location {
                        HitLocation::Part(_) | HitLocation::Track(_) => HitKind::Part,
                        _ => HitKind::Armour,
                    };
                    self.ui.hit_marker(kind);
                    self.sound.hit_tick(kind == HitKind::Part);
                }
                if is_you(*target) {
                    if let Some(src) = attacker.filter(|a| !is_you(*a)).and_then(|a| world.tanks.iter().find(|t| t.player == a)) {
                        let fwd = cam.target - cam.eye;
                        let d = src.pos - my_pos;
                        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
                        let ang = d.dot(right).atan2(d.x * fwd.x + d.z * fwd.z);
                        self.ui.damage_from(ang);
                    }
                }
            }
            Event::Destroyed { victim, killer, weapon, .. } => {
                let victim_name = self.name_of(*victim);
                let killer_name = killer.filter(|k| k != victim).map(|k| self.name_of(k));
                let wname = names::weapon_name(*weapon);
                self.ui.kill_feed(killer_name.as_deref().unwrap_or("The Sink"), Team::Ffa, &victim_name, Team::Ffa, wname);
                if let Some(k) = killer.filter(|k| k != victim) {
                    if is_you(k) {
                        self.ui.hit_marker(HitKind::Kill);
                        self.sound.kill_tick();
                        self.juice.kill();
                    }
                    let n = {
                        let s = self.m.streaks.entry(k).or_insert(0);
                        *s += 1;
                        *s
                    };
                    let kn = killer_name.clone().unwrap_or_default();
                    if !self.m.first_blood {
                        self.m.first_blood = true;
                        self.radio.say(&radio::first_blood(&kn, &victim_name, is_you(k)), true);
                    } else if n == 3 || n == 5 || n == 8 || n == 12 {
                        self.radio.say(&radio::streak(&kn, n, is_you(k)), is_you(k));
                    }
                }
                self.m.streaks.insert(*victim, 0);
                if is_you(*victim) {
                    self.m.killer = killer.filter(|k| k != victim);
                    self.m.killed_by = killer_name.clone().map(|n| (n, wname.to_string()));
                    self.radio.say(&radio::you_died(killer_name.as_deref()), true);
                    self.ui.hide_branch();
                    self.m.branch = None;
                }
            }
            Event::UpgradeApplied { player, track, tier, part } if is_you(*player) && !(*tier == 2 && part.is_some_and(names::is_branch)) => {
                let name = part.map_or(names::scrap_name(*track), names::part_name);
                self.radio.say(&radio::upgraded(name, *tier), *tier >= 3);
            }
            Event::PartAttached { player, part } if is_you(*player) => {
                self.radio.say(&radio::upgraded(names::part_name(*part), 2), true);
            }
            Event::BranchOffered { player, track, options, default } if is_you(*player) => {
                let opts: Vec<BranchOption> = options
                    .iter()
                    .map(|p| BranchOption {
                        kind: names::ui_scrap(*track),
                        name: names::part_name(*p).to_string(),
                        blurb: names::part_blurb(*p).to_string(),
                    })
                    .collect();
                let def = options.iter().position(|p| p == default).unwrap_or(0);
                let title = format!("{} tier 2 \u{b7} pick one", names::scrap_name(*track));
                self.ui.offer_branch(&title, &opts, def, BRANCH_SECS);
                self.m.branch = Some(BRANCH_SECS);
            }
            Event::BranchChosen { player, part, .. } if is_you(*player) => {
                self.ui.hide_branch();
                self.m.branch = None;
                self.radio.say(&radio::branch_taken(names::part_name(*part)), false);
            }
            Event::PartBrokeOff { player, part, .. } if is_you(*player) => {
                self.radio.say(&radio::part_lost(names::part_name(*part)), false);
            }
            Event::TrackBlown { player, side } if is_you(*player) => {
                self.m.track_blown_at[(*side as usize).min(1)] = Some(self.time);
                self.radio.say(&radio::track_blown(), false);
            }
            Event::WaveStarted { wave, raiders, boss } => {
                self.radio.say(&radio::wave(*wave, *raiders, *boss), true);
            }
            Event::WaveCleared { wave } => self.radio.say(&radio::wave_cleared(*wave), false),
            Event::OutpostHit { health, .. } => {
                let frac = (*health / self.m.outpost_max.max(1.0)).clamp(0.0, 1.0);
                let quarter = (frac * 4.0).ceil() as u8;
                if self.m.outpost_quarter == 0 {
                    self.m.outpost_quarter = 4;
                }
                if quarter < self.m.outpost_quarter {
                    self.m.outpost_quarter = quarter;
                    self.radio.say(&radio::outpost(frac), quarter <= 2);
                }
            }
            Event::OutpostRepaired { .. } => {
                if let Some(r) = self.net.as_ref().and_then(|n| n.state().world.as_ref()).and_then(|w| w.raid.as_ref()) {
                    let frac = f32::from(r.health) / f32::from(r.max_health.max(1));
                    self.m.outpost_quarter = ((frac * 4.0).ceil() as u8).max(1);
                }
            }
            Event::RaidLost { waves_survived, reason } => {
                self.end_raid(*waves_survived, *reason == arena_core::raid::LossReason::OutpostFell);
            }
            Event::BossSpawned { pos, .. } => {
                let d = *pos - my_pos;
                self.radio.say(&radio::boss_spawned(names::compass(d.x, d.z)), true);
            }
            Event::BossModuleLost { part, .. } => self.radio.say(&radio::boss_module(names::part_name(*part)), false),
            Event::BossCoreExposed { .. } => self.radio.say(&radio::boss_core(), true),
            Event::BossDefeated { killer, .. } => {
                let k = killer.map(|k| self.name_of(k));
                let mine = killer.is_some_and(is_you);
                if mine {
                    self.ui.hit_marker(HitKind::Kill);
                }
                self.radio.say(&radio::boss_down(k.as_deref(), mine), true);
            }
            Event::AirStrikeCalled { player, target, .. } => {
                let d = *target - my_pos;
                self.radio.say(&radio::air_strike(is_you(*player), names::compass(d.x, d.z)), true);
            }
            Event::HazardExploded { pos, .. } => {
                let d = *pos - my_pos;
                self.radio.say(&radio::hazard(names::compass(d.x, d.z), d.length() < 25.0), false);
            }
            Event::Respawned { player, .. } if is_you(*player) => {
                self.ui.show_respawn(None, None);
                self.m.killer = None;
                self.m.killed_by = None;
            }
            Event::MatchEnded { winner } => self.end_match(*winner),
            _ => {}
        }
    }

    fn end_match(&mut self, winner: Option<PlayerId>) {
        if self.m.ended {
            return;
        }
        let you = self.you();
        let you_won = winner.is_some() && winner == you;
        let wname = winner.map(|w| self.name_of(w));
        let title = match (&wname, you_won) {
            (_, true) => "You take the Sink".to_string(),
            (Some(n), _) => format!("{n} takes the Sink"),
            (None, _) => "The Sink takes everyone".to_string(),
        };
        let note = match self.session {
            Some(Session::Online(_)) => "Next round rolls out in a few seconds",
            _ => "Again for another round",
        };
        self.show_end(&title, note);
        self.radio.say(&radio::match_over(wname.as_deref(), you_won), true);
    }

    fn end_raid(&mut self, waves: u16, outpost_fell: bool) {
        if self.m.ended {
            return;
        }
        let title = if outpost_fell { "The outpost fell" } else { "The crew is scrap" };
        let note = format!("Held {waves} wave{}", if waves == 1 { "" } else { "s" });
        self.show_end(title, &note);
        self.radio.say(&radio::raid_lost(waves, outpost_fell), true);
    }

    fn show_end(&mut self, title: &str, note: &str) {
        self.m.ended = true;
        self.m.podium_pending = true;
        self.screen = Screen::End;
        self.ui.hide_branch();
        self.ui.show_scoreboard(false, &[]);
        self.ui.show_respawn(None, None);
        let snap = self.net.as_ref().and_then(|n| n.state().world.clone());
        let mut rows = snap.as_ref().map(|s| self.score_rows(s, 0.0)).unwrap_or_default();
        rows.truncate(8);
        let size = |r: &ScoreRow| r.tiers.iter().map(|t| u32::from(*t)).sum::<u32>();
        let mut biggest = None;
        let mut most = 0;
        for (i, r) in rows.iter().enumerate() {
            if size(r) > most {
                most = size(r);
                biggest = Some(i);
            }
        }
        let end: Vec<EndRow> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| EndRow {
                name: r.name.clone(),
                team: Team::Ffa,
                kills: r.kills,
                deaths: r.deaths,
                tag: if i == 0 {
                    Some("Top gun".into())
                } else if Some(i) == biggest {
                    Some("Biggest build".into())
                } else {
                    None
                },
            })
            .collect();
        self.ui.show_endscreen(title, None, &end, note);
    }

    fn is_raider(&self, id: PlayerId) -> bool {
        self.net.as_ref().is_some_and(|n| n.state().players.iter().any(|p| p.id == id && p.team == Some(TEAM_RAIDERS)))
    }

    fn snap_is_raid(&self, snap: &Snapshot) -> bool {
        snap.raid.is_some()
    }

    /// Raid: wave, raiders left, outpost health and the boss bar, in the
    /// clock plate (the timer counts the breather down).
    fn raid_hud(&mut self, snap: &Snapshot) {
        let Some(r) = &snap.raid else { return };
        self.m.outpost_max = f32::from(r.max_health.max(1));
        let outpost = f32::from(r.health) / self.m.outpost_max * 100.0;
        let mut label = match r.phase {
            RaidPhase::Breather => {
                self.ui.set_timer(r.ticks_left as f32 / 60.0);
                format!("Wave {} next \u{b7} Outpost {:.0}%", r.wave + 1, outpost)
            }
            RaidPhase::Fighting => {
                // Fighting: the clock shows how long the outpost has held.
                self.ui.set_timer(snap.tick as f32 / 60.0);
                format!("Wave {} \u{b7} {} raiders \u{b7} Outpost {:.0}%", r.wave, r.raiders_alive + r.raiders_queued, outpost)
            }
        };
        if let Some(b) = snap.bosses.first() {
            let hp = f32::from(b.health) / f32::from(b.max_health.max(1)) * 100.0;
            let left = b.modules.iter().filter(|m| m.alive).count();
            let core = if b.core_exposed { " \u{b7} CORE OPEN" } else { "" };
            label.push_str(&format!(" \u{b7} War Rig {hp:.0}% \u{b7} {left}/{} modules{core}", b.modules.len()));
        }
        if label != self.m.raid_label {
            self.ui.set_mode_label(&label);
            self.m.raid_label = label;
        }
    }

    /// One line of state for headless checks.
    pub fn debug_line(&self) -> String {
        let st = self.net.as_ref().map(|n| n.state());
        let me = st.and_then(|s| s.my_tank());
        format!(
            "screen={:?} status={:?} you={:?} code={:?} tanks={} alive={} hp={} pos={:?} paused={} captured={} rtt={:.0}",
            self.screen,
            st.map(|s| s.status.clone()),
            st.and_then(|s| s.you),
            st.and_then(|s| s.code.clone()),
            st.and_then(|s| s.world.as_ref()).map_or(0, |w| w.tanks.len()),
            me.is_some_and(|t| t.alive),
            me.map_or(0, |t| t.health),
            me.map(|t| [t.pos[0].round(), t.pos[2].round()]),
            self.paused,
            self.controls.captured,
            st.map_or(0.0, |s| s.rtt_ms),
        )
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

fn wrap(mut a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    while a > PI {
        a -= TAU;
    }
    while a < -PI {
        a += TAU;
    }
    a
}

/// Practice shares the backdrop's map; the seed only varies the match.
fn rand_seed(_t: f32) -> u64 {
    HOME_SEED
}

fn tip(t: f32) -> &'static str {
    const TIPS: [&str; 6] = [
        "Tip: side and rear plates are thinner. Flanking pays.",
        "Tip: three of a kind bolts on. Shoot theirs off, take it home.",
        "Tip: hold Ctrl or Z for the gunsight. Long Gun sees further.",
        "Tip: accuracy blooms on the move. Stop to land the long ones.",
        "Tip: Shift burns fuel for a burst. F calls in what you're carrying.",
        "Tip: tracks blow off. Your crew patches them, if you give them a second.",
    ];
    TIPS[(t * 7.0) as usize % TIPS.len()]
}
