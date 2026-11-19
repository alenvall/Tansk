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
use arena_core::map::{Map, MapId};
use arena_core::net::Join;
use arena_core::snapshot::{Snapshot, TankSnap};
use arena_core::raid::{ModeKind, RaidPhase, TEAM_RAIDERS};
use arena_core::{Event, Input, Phase, PlayerId};
use glam::Vec3;

use crate::camera::{Camera, ChaseCam};
use crate::renderer::{Look, Quality, TimeOfDay};
use crate::camera::CameraCollider;
use crate::demo::{Demo, MapCollider};
use crate::net::{self, ChatLine, NetClient, Status};
use crate::ui::{BranchOption, EndRow, HitKind, OpenGame, ScoreRow, Settings, Team, TrackState, Ui, UiAction};
use crate::world::{paint_for, RenderWorld, TankView, WorldRenderer};

pub use controls::{Controls, Pad};
use controls::pad;
use juice::Juice;
use radio::Radio;
use sound::Sound;

/// The backdrop and Practice share this map seed, so starting Practice
/// doesn't rebuild the scenery.
const HOME_SEED: u64 = 7;
const PRACTICE_BOTS: usize = 7;
const PRACTICE_DIFFICULTY: f32 = 0.55;
const RAID_ALLIES: usize = 3;
const KILLCAM_SECS: f32 = 2.0;
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
    /// `?fragile=1`: you start on 10% hull (to see the kill-cam).
    pub fragile: bool,
    /// `?wave=N`: raid practice starts at wave N.
    pub wave: Option<u32>,
    /// `?boss=1`: the War Rig spawns at once; `?boss=weak` with its
    /// modules and hull nearly shot through (to see them come off).
    pub boss: Option<bool>,
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
    Practice { mode: ModeKind },
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
    crowd_check: f32,
    crowd_called: HashMap<usize, f32>,
    /// Seconds since you died (kill-cam).
    dead_for: f32,
    killcam_angle: f32,
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
    /// The map Practice (and its Team / Raid variants) plays: the title's
    /// map switch, or `?map=`.
    pub practice_map: MapId,
    /// The Garage's pick (offline Practice uses it; online still default).
    garage: Option<crate::ui::Garage>,
    /// The newest gamepad state (web: Gamepad API), `None` without one.
    pub pad: Option<Pad>,
    prev_pad: Pad,
    /// Seconds since the pad was last touched (it takes over from the mouse).
    pad_idle: f32,
    /// No saved quality: start Medium, drop to Low if the first 3 s are slow.
    pub quality_auto: bool,
    quality_probe: (f32, u32),
    want_quality: Option<Quality>,
    want_look: Option<TimeOfDay>,
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
    camera_title: Option<Vec3>,
    title_follow: (Option<PlayerId>, f32),
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
            camera_title: None,
            title_follow: (None, 0.0),
            debug: DebugFlags::default(),
            practice_map: MapId::Sink,
            garage: None,
            pad: None,
            prev_pad: Pad::default(),
            pad_idle: 1e9,
            quality_auto: false,
            quality_probe: (0.0, 0),
            want_quality: None,
            want_look: Some(TimeOfDay::GoldenHour),
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
        self.sound.set_volumes(s.vol_master, s.vol_sfx, s.vol_ambience);
        self.want_quality = Some(render_quality(s.quality));
        self.ui.apply_settings(&s);
        self.settings = s;
    }

    /// `(code, humans, max_humans, mode, map)` of the open public rooms
    /// (mode and map as the server's JSON names them).
    pub fn set_rooms(&mut self, rooms: &[(String, u32, u32, String, String)]) {
        let games: Vec<OpenGame> = rooms
            .iter()
            .map(|(code, n, max, mode, map)| OpenGame {
                host: format!("Room {code}"),
                mode: match mode.as_str() {
                    "TeamBrawl" => "Team Brawl",
                    "RaiderWaves" => "Raid",
                    _ => "Scrap Brawl",
                }
                .into(),
                map: MapId::from_key(map).unwrap_or_default().name().into(),
                players: *n,
                max_players: *max,
                ping_ms: 0,
                code: code.clone(),
            })
            .collect();
        self.ui.set_open_games(&games);
    }

    pub fn start_practice(&mut self) {
        self.start(Session::Practice { mode: ModeKind::ScrapBrawl });
    }

    /// Offline Raider Waves with bot allies.
    pub fn start_practice_raid(&mut self) {
        self.start(Session::Practice { mode: ModeKind::RaiderWaves });
    }

    /// Offline Team Brawl: you and 3 bot allies against 4 bots.
    pub fn start_practice_team(&mut self) {
        self.start(Session::Practice { mode: ModeKind::TeamBrawl });
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
            Session::Practice { mode: ModeKind::TeamBrawl } => {
                self.ui.show_loading("Picking sides", "You and three Rustbacks against four Dust Saints.", Some(0.6));
                NetClient::practice_team(rand_seed(self.time), 4, PRACTICE_DIFFICULTY, self.practice_map)
            }
            Session::Practice { mode: ModeKind::ScrapBrawl } => {
                self.ui.show_loading("Rolling out", "Seven bots, no mercy. Warming the engines.", Some(0.6));
                NetClient::practice(rand_seed(self.time), PRACTICE_BOTS, PRACTICE_DIFFICULTY, self.practice_map)
            }
            Session::Practice { mode: ModeKind::RaiderWaves } => {
                self.ui.show_loading("Manning the outpost", "Three allies, a lot of raiders. Dig in.", Some(0.6));
                NetClient::practice_raid(rand_seed(self.time), RAID_ALLIES, PRACTICE_DIFFICULTY, self.practice_map)
            }
            Session::Online(join) => {
                let sub = match join {
                    Join::Code(c) => format!("Tuning the radio to {c}..."),
                    Join::Host { mode: ModeKind::RaiderWaves, .. } => "Fortifying the outpost...".to_string(),
                    Join::Host { mode: ModeKind::TeamBrawl, .. } => "Painting the hulls...".to_string(),
                    Join::Host { .. } => "Opening the gates...".to_string(),
                    _ => "Looking for a crew to fight...".to_string(),
                };
                self.ui.show_loading("Connecting", &sub, Some(0.2));
                NetClient::connect(&net::transport::default_url(), &self.settings.name, join.clone())
            }
        };
        self.ui.set_loading_tip(tip(self.time));
        let roll = (self.time * 977.0).fract();
        self.want_look = Some(if roll < 0.6 {
            TimeOfDay::GoldenHour
        } else if roll < 0.85 {
            TimeOfDay::Dusk
        } else {
            TimeOfDay::Noon
        });
        self.net = Some(net);
        self.apply_garage();
        self.apply_debug();
        self.session = Some(session);
        self.screen = Screen::Loading;
    }

    /// The Garage loadout onto your tank in an offline match (it takes on
    /// the next spawn).
    fn apply_garage(&mut self) {
        let Some(g) = self.garage.clone() else { return };
        let you = self.you();
        let Some(sim) = self.net.as_mut().and_then(|n| n.practice_sim()) else { return };
        let Some(you) = you else { return };
        let loadout = arena_core::garage::Loadout {
            chassis: match g.chassis {
                crate::ui::Chassis::Scout => arena_core::garage::ChassisClass::Scout,
                crate::ui::Chassis::Brawler => arena_core::garage::ChassisClass::Brawler,
                crate::ui::Chassis::Hauler => arena_core::garage::ChassisClass::Hauler,
            },
            paint: paint_index(&g.paint),
            emblem: ["wrench", "cog", "skull", "flame", "star", "drop"].iter().position(|e| *e == g.emblem).unwrap_or(0) as u8,
            horn: 0,
        };
        sim.set_loadout(you, loadout);
    }

    /// Your paint from the Garage (hex), for drawing your own tank.
    fn my_paint(&self) -> Option<u32> {
        let hex = self.garage.as_ref()?.paint.trim_start_matches('#').to_string();
        u32::from_str_radix(&hex, 16).ok()
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
        if let Some(n) = d.wave {
            sim.debug_jump_to_wave(n);
        }
        if let Some(weak) = d.boss {
            if let Some(id) = sim.debug_spawn_boss() {
                if weak {
                    if let Some(p) = sim.players.iter_mut().find(|p| p.id == id) {
                        for part in &mut p.tank.parts {
                            part.health = part.health.min(part.max_health * 0.01);
                        }
                        p.tank.health *= 0.03;
                    }
                }
            }
        }
        for p in sim.players.iter_mut().filter(|p| p.brain.is_none()) {
            if d.flare {
                p.tank.air_strike = true;
            }
            if d.fragile {
                p.tank.health *= 0.1;
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
        self.want_look = Some(TimeOfDay::GoldenHour);
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
                UiAction::PracticeMap(key) => self.practice_map = MapId::from_key(&key).unwrap_or_default(),
                UiAction::Host(cfg) => {
                    let mode = match cfg.mode.as_str() {
                        "raider_waves" => ModeKind::RaiderWaves,
                        "team_brawl" => ModeKind::TeamBrawl,
                        _ => ModeKind::ScrapBrawl,
                    };
                    let map = MapId::from_key(&cfg.map).unwrap_or_default();
                    self.start_online(Join::Host { public: !cfg.private, mode, map });
                }
                UiAction::Join { code } => self.start_online(Join::Code(code)),
                UiAction::RefreshGames => self.want_rooms = true,
                UiAction::Cancel => self.leave(),
                UiAction::SettingsChanged(s) => {
                    self.quality_auto = false;
                    self.apply_settings(s);
                    self.settings_dirty = true;
                }
                UiAction::GarageChanged(g) => {
                    self.garage = Some(g);
                    self.apply_garage();
                }
                UiAction::BranchPicked(i) => self.pick_branch(i),
                UiAction::Chat(text) => {
                    if let Some(n) = &mut self.net {
                        n.chat(&text);
                    }
                }
                UiAction::Again => match self.session.clone() {
                    Some(Session::Practice { mode }) => self.start(Session::Practice { mode }),
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
        self.render_settings(dt, wr);
        self.pad_frame(dt);
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

    fn render_settings(&mut self, dt: f32, wr: &mut WorldRenderer) {
        wr.renderer.note_frame_time(dt);
        if self.quality_auto && self.quality_probe.1 != u32::MAX {
            if self.time < 3.0 {
                if self.time > 0.5 {
                    self.quality_probe.0 += dt;
                    self.quality_probe.1 += 1;
                }
            } else {
                let (sum, n) = self.quality_probe;
                let avg = if n > 0 { sum / n as f32 } else { 0.0 };
                if avg > 0.025 {
                    log::info!("auto quality: {:.1} ms frames, dropping to Low", avg * 1000.0);
                    self.settings.quality = crate::ui::Quality::Low;
                    self.ui.apply_settings(&self.settings);
                    self.want_quality = Some(Quality::Low);
                }
                self.quality_probe.1 = u32::MAX;
            }
        }
        if let Some(q) = self.want_quality.take() {
            wr.renderer.set_quality(q);
        }
        if let Some(t) = self.want_look.take() {
            wr.renderer.look = Look::preset(t);
        }
    }

    /// Pad edges (Start pauses, Back is the board, D-pad picks a branch).
    fn pad_frame(&mut self, dt: f32) {
        let Some(p) = self.pad else { return };
        self.pad_idle = if p.active() { 0.0 } else { self.pad_idle + dt };
        let prev = self.prev_pad;
        let edge = |b: usize| p.down(b) && !prev.down(b);
        if self.screen == Screen::InGame {
            if edge(pad::START) {
                self.paused = !self.paused;
            }
            if self.paused && edge(pad::B) {
                self.leave();
            }
            if self.m.branch.is_some() {
                for (i, b) in [pad::LEFT, pad::UP, pad::RIGHT].into_iter().enumerate() {
                    if edge(b) {
                        self.pick_branch(i as u8);
                        self.ui.hide_branch();
                    }
                }
            }
        } else if self.screen == Screen::End && edge(pad::A) {
            self.ui.hide_endscreen();
            match self.session.clone() {
                Some(Session::Practice { mode }) => self.start(Session::Practice { mode }),
                _ => self.leave(),
            }
        } else if self.screen == Screen::Title && edge(pad::A) {
            self.start_practice();
        }
        self.prev_pad = p;
    }

    /// The pad was used in the last few seconds: it drives, not the mouse.
    fn pad_live(&self) -> bool {
        self.pad.is_some() && self.pad_idle < 5.0
    }

    fn use_map(&mut self, seed: u64, id: MapId, wr: &mut WorldRenderer) {
        if seed != self.map_seed || id != self.map.id {
            self.map = Map::by_id(id, seed);
            self.map_seed = seed;
            wr.set_map(&self.map);
        }
    }

    /// "the Sink", "the Refinery": the place for the end-screen titles.
    fn place(&self) -> String {
        self.map.name.replacen("The ", "the ", 1)
    }

    /// The live backdrop: a demo match under a slow orbit.
    fn title_frame(&mut self, dt: f32, wr: &mut WorldRenderer) -> RenderWorld {
        self.use_map(HOME_SEED, MapId::Sink, wr);
        wr.renderer.set_storm(0.0);
        let mut world = self.backdrop.advance(dt, wr).clone();
        // The fight: stick with the bot that has company (its nearest
        // neighbour closest) for a while, then pick again.
        let live: Vec<(PlayerId, Vec3)> = world.tanks.iter().filter(|t| t.alive).map(|t| (t.player, t.pos)).collect();
        self.title_follow.1 -= dt;
        let following = self.title_follow.0.filter(|id| live.iter().any(|(p, _)| p == id));
        if following.is_none() || self.title_follow.1 <= 0.0 {
            let lonely = |(id, pos): &(PlayerId, Vec3)| {
                live.iter().filter(|(o, _)| o != id).map(|(_, q)| pos.distance(*q)).fold(f32::MAX, f32::min)
            };
            let pick = live.iter().min_by(|a, b| lonely(a).total_cmp(&lonely(b))).map(|(id, _)| *id);
            if pick != following {
                // A cut, not a slow pan across the map.
                self.orbit_centre = Vec3::ZERO;
            }
            self.title_follow = (pick, 15.0);
        }
        let subject = self.title_follow.0.and_then(|id| live.iter().find(|(p, _)| *p == id)).map(|(_, pos)| *pos);
        if let Some(c) = subject {
            if self.orbit_centre == Vec3::ZERO {
                self.orbit_centre = c;
            }
            self.orbit_centre += (c - self.orbit_centre) * (1.0 - (-dt * 2.5).exp());
            let c = self.orbit_centre;
            // A slow circle; the action sits right of centre, clear of the
            // title's hazed left side.
            let a = self.time * 0.05;
            let eye = c + Vec3::new(a.cos() * 24.0, 6.5, a.sin() * 24.0);
            let eye = Vec3::new(eye.x, eye.y.max(self.map.height_at(eye.x, eye.z) + 4.0), eye.z);
            let fwd = (c - eye).normalize_or(Vec3::X);
            let right = fwd.cross(Vec3::Y).normalize_or(Vec3::Z);
            world.camera = Some(Camera::look(eye, c + Vec3::Y * 1.5 - right * 3.0, 46.0));
            self.camera_title = Some(eye);
        }
        self.sound.frame(world.camera.map_or(Vec3::ZERO, |c| c.eye), Vec3::X, &[], None, 0.0);
        self.sound.ambience(0.2, 0.0);
        world
    }

    fn loading_frame(&mut self, now: f64, dt: f32, wr: &mut WorldRenderer) -> RenderWorld {
        let Some(net) = &mut self.net else {
            self.leave();
            return self.title_frame(dt, wr);
        };
        net.update(now);
        let st = net.state();
        let (status, seed, has_world, code, map_id) = (st.status.clone(), st.seed, st.world.is_some(), st.code.clone(), st.map);
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
                    self.use_map(seed, map_id, wr);
                    self.screen = Screen::InGame;
                    self.ui.show_hud();
                    self.ui.set_ffa(true);
                    let map_name = map_id.name();
                    let mode = match (&self.session, &code) {
                        (Some(Session::Practice { mode: ModeKind::TeamBrawl }), _) => format!("Team Practice \u{b7} {map_name}"),
                        (Some(Session::Practice { .. }), _) | (_, None) => format!("Practice \u{b7} {map_name}"),
                        (Some(Session::Online(Join::Host { mode: ModeKind::TeamBrawl, .. })), Some(code)) => format!("Team Brawl \u{b7} {map_name} \u{b7} {code}"),
                        (Some(Session::Online(Join::Host { mode: ModeKind::RaiderWaves, .. })), Some(code)) => format!("Raider Waves \u{b7} {map_name} \u{b7} {code}"),
                        (_, Some(code)) => format!("Scrap Brawl \u{b7} {map_name} \u{b7} {code}"),
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
        let pad_live = self.pad_live();
        let live = self.screen == Screen::InGame && !self.paused && !typing && (self.controls.captured || pad_live);
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
        let pad = if pad_live { self.pad } else { None };
        if let (Some(p), true) = (pad, live) {
            // Right stick: the camera, slowed near a visible enemy (aim assist).
            let assist = self.aim_assist();
            let (rx, ry) = (p.axis(2), p.axis(3));
            let rate = 2.6 * sens * fov_k * assist;
            self.chase.yaw -= rx * rx.abs() * rate * dt;
            self.chase.pitch = (self.chase.pitch - ry * ry.abs() * 1.6 * sens * fov_k * assist * dt).clamp(-0.75, 0.5);
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
            input.horn = c.held("KeyH");
            if let Some(p) = pad {
                input.horn |= p.down(pad::LS);
                if input.throttle == 0.0 {
                    input.throttle = -p.axis(1);
                }
                if input.steer == 0.0 {
                    input.steer = p.axis(0);
                }
                input.fire |= p.down(pad::RT);
                input.alt_fire |= p.down(pad::RB);
                input.boost |= p.down(pad::LB);
                input.special |= p.down(pad::A) || p.down(pad::Y);
            }
        }
        let zoom = live && (c.any(&["ControlLeft", "ControlRight", "KeyZ"]) || pad.is_some_and(|p| p.buttons[pad::LT] > 0.4));
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
        if let (Some(c), Some(y), true) = (self.my_paint(), you, snap.teams.is_empty() && snap.raid.is_none()) {
            if let Some(t) = world.tanks.iter_mut().find(|t| t.player == y) {
                t.colour = c;
            }
        }
        if !snap.teams.is_empty() {
            for t in &mut world.tanks {
                if snap.tank(t.player).is_some_and(|s| s.team == 1) {
                    t.colour = crate::tank::DUST_SAINT;
                }
            }
        }
        world.time = self.time;
        world.dt = dt * self.juice.time_scale(dt);
        wr.renderer.set_storm(snap.storm);
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
        if !alive && self.m.alive {
            self.m.dead_for = 0.0;
            self.m.killcam_angle = self.chase.yaw + std::f32::consts::PI;
        }
        if !alive {
            self.m.dead_for += dt;
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
        if let (false, Some(k)) = (alive, &killer_view) {
            if self.m.dead_for < KILLCAM_SECS {
                // Kill-cam: a slow half-orbit round whoever did it.
                let a = self.m.killcam_angle + self.m.dead_for * 0.9;
                // Bigger machines (the War Rig) get a wider circle.
                let big = self
                    .net
                    .as_ref()
                    .and_then(|n| n.state().world.as_ref())
                    .and_then(|w| w.tank(k.player))
                    .is_some_and(|t| t.vehicle == arena_core::parts::VehicleKind::WarRig);
                let r = if big { 30.0 } else { 13.0 } - self.m.dead_for * 1.5;
                let eye = k.pos + Vec3::new(a.cos() * r, if big { 9.0 } else { 4.5 }, -a.sin() * r);
                let free = MapCollider(&self.map).free_fraction(k.pos + Vec3::Y * 2.0, eye);
                let eye = k.pos + Vec3::Y * 2.0 + (eye - k.pos - Vec3::Y * 2.0) * free.clamp(0.2, 1.0);
                world.camera = Some(Camera::look(eye, k.pos + Vec3::Y * 1.4, 48.0));
            }
        }
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
        // The wind: a steady desert breeze, stronger up high and at speed.
        let (speed, high) = my_view.as_ref().map_or((0.0, 0.0), |v| (v.speed, v.pos.y - self.map.height_at(0.0, 0.0)));
        let wind = 0.35 + (speed / 14.0).min(1.0) * 0.25 + (high / 6.0).clamp(0.0, 1.0) * 0.15;
        self.sound.ambience(wind, snap.storm);

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
        let board = self.screen == Screen::InGame
            && !typing
            && (self.controls.held("Tab") || self.pad.is_some_and(|p| p.down(pad::BACK)));
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
            self.crowd_watch(&snap, dt);
        }
        match snap.mode.phase {
            Phase::Defeated { waves } if !self.m.ended => {
                let fell = snap.raid.as_ref().is_some_and(|r| r.health == 0);
                self.end_raid(waves, fell);
            }
            Phase::TeamWon { team } if !self.m.ended => self.end_team(team),
            _ => {}
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

    /// Gamepad only: turning slows to under half when the view is within a
    /// few degrees of a visible enemy.
    fn aim_assist(&self) -> f32 {
        let (Some(cam), Some(net)) = (self.camera, &self.net) else { return 1.0 };
        let st = net.state();
        let (Some(snap), you) = (&st.world, st.you) else { return 1.0 };
        let fwd = (cam.target - cam.eye).normalize_or(Vec3::X);
        let mine = you.and_then(|y| snap.tank(y));
        let mut best = 1.0f32;
        for t in &snap.tanks {
            if Some(t.player) == you || !t.alive {
                continue;
            }
            if let (Some(m), Some(team)) = (mine, st.players.iter().find(|p| p.id == t.player).and_then(|p| p.team)) {
                // Raid: allies share your team.
                let my_team = st.players.iter().find(|p| p.id == m.player).and_then(|p| p.team);
                if my_team == Some(team) {
                    continue;
                }
            }
            let c = t.position() + Vec3::Y;
            let to = c - cam.eye;
            let d = to.length();
            if !(5.0..250.0).contains(&d) {
                continue;
            }
            let ang = fwd.angle_between(to / d);
            if ang < 0.07 {
                // Visible: the ray reaches about as far as the tank.
                let hit = aim::cast(&self.map, &[], you, cam.eye, to / d, 0.0);
                if hit.distance(cam.eye) > d - 3.0 {
                    best = best.min(0.4 + ang / 0.07 * 0.6);
                }
            }
        }
        best
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
        if snap.teams.is_empty() {
            // Free-for-all: plates hidden; your kills against the leader's.
            ui.set_ffa(true);
            let you = t.player;
            let mine = snap.scores.iter().find(|s| s.player == you).map_or(0, |s| s.kills);
            let best = snap.scores.iter().filter(|s| s.player != you).map(|s| s.kills).max().unwrap_or(0);
            ui.set_scores(u32::from(mine), u32::from(best));
        } else {
            ui.set_ffa(false);
            let k = |team: u8| snap.teams.iter().find(|s| s.team == team).map_or(0, |s| u32::from(s.kills));
            ui.set_scores(k(0), k(1));
        }
    }

    fn score_rows(&self, snap: &Snapshot, rtt: f32) -> Vec<ScoreRow> {
        let you = self.you();
        let players = self.net.as_ref().map(|n| n.state().players.clone()).unwrap_or_default();
        let mut rows: Vec<ScoreRow> = players
            .iter()
            .filter(|p| snap.raid.is_none() || p.team != Some(TEAM_RAIDERS))
            .map(|p| {
                let s = snap.scores.iter().find(|s| s.player == p.id);
                let t = snap.tank(p.id);
                let is_you = Some(p.id) == you;
                ScoreRow {
                    name: self.name_of(p.id),
                    team: self.team_of(p.id),
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

    /// Every few seconds: three or more crews piling into one named place.
    fn crowd_watch(&mut self, snap: &Snapshot, dt: f32) {
        self.m.crowd_check -= dt;
        if self.m.crowd_check > 0.0 {
            return;
        }
        self.m.crowd_check = 6.0;
        let you = self.you();
        let me = you.and_then(|y| snap.tank(y)).map(|t| t.position());
        let flat = |v: Vec3| glam::Vec2::new(v.x, v.z);
        for (i, r) in self.map.regions.iter().enumerate() {
            // Not where you already are: you can see that.
            if me.is_some_and(|m| flat(m).distance(flat(r.center)) < r.radius) {
                continue;
            }
            let n = snap
                .tanks
                .iter()
                .filter(|t| t.alive && Some(t.player) != you && flat(t.position()).distance(flat(r.center)) < r.radius)
                .count();
            let last = self.m.crowd_called.get(&i).copied().unwrap_or(-1e9);
            if n >= 3 && self.time - last > 45.0 {
                self.m.crowd_called.insert(i, self.time);
                let place = names::place(&self.map, r.center, r.center, false);
                self.radio.say(&radio::crowd(n, &place), false);
                return;
            }
        }
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
            let place = names::place(&self.map, pos, from, true);
            let lines = radio::bounty(&self.name_of(p), parts, &place, d.length());
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
                let kt = killer.filter(|k| k != victim).map_or(Team::Ffa, |k| self.team_of(k));
                let vt = self.team_of(*victim);
                self.ui.kill_feed(killer_name.as_deref().unwrap_or("The Sink"), kt, &victim_name, vt, wname);
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
                    } else if (n == 3 || n == 5 || n == 8 || n == 12) && !self.is_raider(k) {
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
                let place = names::place(&self.map, *pos, my_pos, true);
                self.radio.say(&radio::boss_spawned(&place), true);
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
                let place = names::place(&self.map, *target, my_pos, false);
                self.radio.say(&radio::air_strike(is_you(*player), &place), true);
            }
            Event::HazardExploded { pos, .. } => {
                let place = names::place(&self.map, *pos, my_pos, false);
                self.radio.say(&radio::hazard(&place, pos.distance(my_pos) < 25.0), false);
            }
            Event::Respawned { player, .. } if is_you(*player) => {
                self.ui.show_respawn(None, None);
                self.m.killer = None;
                self.m.killed_by = None;
            }
            Event::StormIncoming { in_seconds } => self.radio.say(&radio::storm_incoming(*in_seconds), true),
            Event::StormStarted { .. } => self.radio.say(&radio::storm_started(), false),
            Event::StormEnded => self.radio.say(&radio::storm_ended(), false),
            Event::MatchEnded { winner } => self.end_match(*winner),
            Event::TeamMatchEnded { team } => self.end_team(*team),
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
        let place = self.place();
        let title = match (&wname, you_won) {
            (_, true) => format!("You take {place}"),
            (Some(n), _) => format!("{n} takes {place}"),
            (None, _) => format!("{} takes everyone", self.map.name),
        };
        let note = match self.session {
            Some(Session::Online(_)) => "Next round rolls out in a few seconds",
            _ => "Again for another round",
        };
        self.show_end(&title, note);
        self.radio.say(&radio::match_over(wname.as_deref(), you_won), true);
    }

    fn end_team(&mut self, team: Option<u8>) {
        if self.m.ended {
            return;
        }
        let mine = self.you().map(|y| self.team_of(y));
        let place = self.place();
        let (title, winner) = match team {
            Some(0) => (format!("Rustbacks take {place}"), Some(Team::Rustbacks)),
            Some(1) => (format!("Dust Saints take {place}"), Some(Team::DustSaints)),
            _ => ("Dead even".to_string(), None),
        };
        let note = match self.session {
            Some(Session::Online(_)) => "Next round rolls out in a few seconds",
            _ => "Again for another round",
        };
        self.show_end_team(&title, note, winner);
        let won = winner.map(|w| Some(w) == mine);
        let name = match winner {
            Some(Team::Rustbacks) => "Rustbacks",
            Some(Team::DustSaints) => "Dust Saints",
            _ => "",
        };
        self.radio.say(&radio::team_over(won, name), true);
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
        self.show_end_team(title, note, None);
    }

    fn show_end_team(&mut self, title: &str, note: &str, winner: Option<Team>) {
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
                team: r.team,
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
        self.ui.show_endscreen(title, winner, &end, note);
    }

    /// Rustbacks / Dust Saints in Team Brawl; everyone else a crew.
    fn team_of(&self, id: PlayerId) -> Team {
        let Some(snap) = self.net.as_ref().and_then(|n| n.state().world.as_ref()) else { return Team::Ffa };
        if snap.teams.is_empty() {
            return Team::Ffa;
        }
        match snap.tank(id).map(|t| t.team) {
            Some(0) => Team::Rustbacks,
            Some(1) => Team::DustSaints,
            _ => Team::Ffa,
        }
    }

    fn is_raider(&self, id: PlayerId) -> bool {
        self.net.as_ref().is_some_and(|n| {
            let st = n.state();
            st.world.as_ref().is_some_and(|w| w.raid.is_some())
                && st.players.iter().any(|p| p.id == id && p.team == Some(TEAM_RAIDERS))
        })
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
        if self.screen == Screen::Title {
            let tanks: Vec<[i32; 2]> =
                self.backdrop.world.tanks.iter().filter(|t| t.alive).map(|t| [t.pos.x as i32, t.pos.z as i32]).collect();
            return format!("screen=Title centre={:?} cam={:?} tanks={tanks:?}", self.orbit_centre, self.camera_title);
        }
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

/// The nearest paint-set entry to a Garage colour.
fn paint_index(hex: &str) -> u8 {
    let Ok(c) = u32::from_str_radix(hex.trim_start_matches('#'), 16) else { return 0 };
    let rgb = |v: u32| [(v >> 16) & 255, (v >> 8) & 255, v & 255].map(|x| x as i32);
    let want = rgb(c);
    crate::world::PAINT_SET
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| {
            let q = rgb(**p);
            (0..3).map(|i| (q[i] - want[i]).pow(2)).sum::<i32>()
        })
        .map_or(0, |(i, _)| i as u8)
}

fn render_quality(q: crate::ui::Quality) -> Quality {
    match q {
        crate::ui::Quality::Low => Quality::Low,
        crate::ui::Quality::Medium => Quality::Medium,
        crate::ui::Quality::High | crate::ui::Quality::Ultra => Quality::High,
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
