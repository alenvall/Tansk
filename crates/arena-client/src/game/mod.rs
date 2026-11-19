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
pub mod camcol;
pub mod controls;
pub mod hints;
pub mod juice;
pub mod names;
pub mod party;
pub mod radio;
pub mod sound;

use std::collections::HashMap;

use arena_core::events::{HitLocation, RepairKind};
use arena_core::map::{Map, MapId};
use arena_core::net::{Join, LobbyCmd};
use arena_core::snapshot::{Snapshot, TankSnap};
use arena_core::raid::{ModeKind, RaidPhase, TEAM_RAIDERS};
use arena_core::{Event, Input, Phase, PlayerId};
use glam::Vec3;

use crate::camera::{Camera, ChaseCam};
use crate::renderer::{Look, Quality, TimeOfDay};
use crate::camera::CameraCollider;
use crate::demo::Demo;
use crate::net::{self, ChatLine, NetClient, Status};
use crate::ui::{BranchOption, EndRow, HitKind, OpenGame, ScoreRow, Settings, Team, TrackState, Ui, UiAction};
use crate::world::{paint_for, RenderWorld, TankView, WorldRenderer};

pub use controls::{Controls, Pad};
use controls::pad;
use juice::Juice;
use radio::Radio;
use sound::{MusicTrack, Sound, Stinger};

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
/// Mouse movement ignored after a screen change or pointer lock, s.
const MOUSE_SETTLE: f32 = 0.25;
/// The biggest mouse step one frame takes, px (a bigger one is a glitch).
const MOUSE_MAX_STEP: f32 = 160.0;
/// Camera pitch range and resting pitch, radians.
const PITCH_MIN: f32 = -0.6;
const PITCH_MAX: f32 = 0.22;
const PITCH_REST: f32 = -0.12;
/// Untouched this long, the camera eases back to its resting pitch, s.
const PITCH_IDLE: f32 = 2.5;
/// The radio calls the Convoy truck stopped at most this often, s.
const TRUCK_STOPPED_GAP: f32 = 20.0;

/// Test hooks from URL flags; they only touch the offline Practice sim.
#[derive(Clone, Debug, Default)]
pub struct DebugFlags {
    /// `?landship=1`: THE LAND-SHIP rolls in at once (raid practice).
    pub landship: bool,
    /// `?matchtime=45`: Scrap Brawl (and Wellhead) length, seconds.
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
    /// `?wallfast=1`: Last Crew practice's wall starts closing at once, quickly.
    pub wall_fast: bool,
    /// `?tod=golden|dusk|noon`: every match at that time of day (else rolled).
    pub tod: Option<TimeOfDay>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Loading,
    InGame,
    End,
    /// A party room's lobby: the warm-up world under the lobby plate.
    Lobby,
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
    /// Seconds since you died (kill-cam), and since you last spawned.
    dead_for: f32,
    alive_for: f32,
    /// When the radio last called the Convoy truck stopped.
    truck_stopped_said: Option<f32>,
    killcam_angle: f32,
    /// Build the podium on the next frame (the end screen just opened).
    podium_pending: bool,
    /// Raid: the mode label last written, the outpost's max health, and the
    /// last outpost quarter the radio called out.
    raid_label: String,
    outpost_max: f32,
    outpost_quarter: u8,
    /// Music: recent action (decays) and the smoothed intensity.
    heat: f32,
    intensity: f32,
    /// The round-start stinger has played.
    started: bool,
    /// Last Crew: where you finished once you're out, the order crews went
    /// out (first out first), the "crews left" calls made, and the last
    /// "you're in the storm" nag.
    out_place: Option<u8>,
    out_order: Vec<PlayerId>,
    crews_called: Vec<u8>,
    storm_nag: f32,
    /// Wellhead: "nearly full" called per team.
    fuel_called: [bool; 2],
    /// The last snapshot showed a finished match (a new one resets the above).
    was_over: bool,
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
    /// Last frame's network/simulation update, ms (offline: the sim steps).
    pub prof_net_ms: f64,
    /// Last frame's game phases, ms: world, camera+aim, events, sound, HUD.
    pub prof_game: [f64; 8],
    quality_probe: (f32, u32),
    want_quality: Option<Quality>,
    want_look: Option<TimeOfDay>,
    juice: Juice,
    heart: f32,
    /// Onboarding tips, once per browser.
    pub hints: hints::Hints,
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
    /// Convoy: the truck's road (as the sim joins it) for this map, and the
    /// distance along it at each point.
    convoy_route: Option<((u64, MapId), Vec<Vec3>, Vec<f32>)>,
    /// The page's address without the query (web), for invite links.
    pub invite_base: String,
    /// The party room's Welcomes handled (a new one = a rebuilt world).
    welcomes_seen: u32,
    /// In the lobby, driving about with the pointer held.
    lobby_drive: bool,
    /// The party's mode label as last written.
    party_label: String,
    /// Mouse movement is ignored this much longer (a screen change or a
    /// fresh pointer lock delivers one bogus jump).
    mouse_settle: f32,
    last_screen: Screen,
    /// Seconds since the player last turned the camera up or down.
    pitch_idle: f32,
    /// The Garage pick changed: the platform may save it.
    garage_dirty: bool,
}

impl Game {
    pub fn new() -> Self {
        let backdrop = Demo::new(HOME_SEED, PRACTICE_BOTS);
        let map = Map::the_sink(HOME_SEED);
        let mut ui = Ui::mount();
        ui.set_ticker(&ticker(MapId::Sink));
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
            convoy_route: None,
            debug: DebugFlags::default(),
            invite_base: String::new(),
            welcomes_seen: 0,
            lobby_drive: false,
            party_label: String::new(),
            mouse_settle: 0.0,
            last_screen: Screen::Title,
            pitch_idle: 0.0,
            garage_dirty: false,
            practice_map: MapId::Sink,
            garage: None,
            pad: None,
            prev_pad: Pad::default(),
            pad_idle: 1e9,
            quality_auto: false,
            prof_net_ms: 0.0,
            prof_game: [0.0; 8],
            quality_probe: (0.0, 0),
            want_quality: None,
            want_look: Some(TimeOfDay::GoldenHour),
            juice: Juice::default(),
            heart: 0.0,
            hints: hints::Hints::default(),
        }
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The platform should hold the pointer lock now.
    pub fn wants_pointer(&self) -> bool {
        (self.screen == Screen::InGame && !self.paused) || (self.screen == Screen::Lobby && self.lobby_drive)
    }

    /// A click on the world in the lobby: take the wheel (the platform then
    /// locks the pointer). Esc hands it back.
    pub fn lobby_take_wheel(&mut self) {
        if self.screen == Screen::Lobby {
            self.lobby_drive = true;
            self.ui.set_lobby_drive(true);
        }
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
        if self.lobby_drive {
            self.lobby_drive = false;
            self.ui.set_lobby_drive(false);
        }
    }

    pub fn pointer_locked(&mut self) {
        self.controls.captured = true;
        self.paused = false;
        self.mouse_settle = MOUSE_SETTLE;
    }

    /// The saved Garage pick (startup): on the menus and on every join.
    pub fn set_garage(&mut self, g: crate::ui::Garage) {
        self.ui.apply_garage(&g);
        self.garage = Some(g);
    }

    pub fn garage(&self) -> Option<&crate::ui::Garage> {
        self.garage.as_ref()
    }

    /// The Garage pick changed: the platform may persist it.
    pub fn garage_dirty(&mut self) -> bool {
        std::mem::take(&mut self.garage_dirty)
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
        self.sound.set_volumes(s.vol_master, s.vol_sfx, s.vol_ambience, s.vol_music);
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
                    "Convoy" => "Convoy",
                    "Wellhead" => "Wellhead",
                    "LastCrew" => "Last Crew",
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

    /// Offline Convoy: you and 3 bot allies escort first, then stop theirs.
    pub fn start_practice_convoy(&mut self) {
        self.start(Session::Practice { mode: ModeKind::Convoy });
    }

    /// Offline Team Brawl: you and 3 bot allies against 4 bots.
    pub fn start_practice_team(&mut self) {
        self.start(Session::Practice { mode: ModeKind::TeamBrawl });
    }

    /// Offline Wellhead: you and 3 bot allies against 4 bots over the derricks.
    pub fn start_practice_wellhead(&mut self) {
        self.start(Session::Practice { mode: ModeKind::Wellhead });
    }

    /// Offline Last Crew Rolling: you and 7 bots, no respawns, the wall closing in.
    pub fn start_practice_last_crew(&mut self) {
        self.start(Session::Practice { mode: ModeKind::LastCrew });
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
        self.welcomes_seen = 0;
        self.party_label.clear();
        self.lobby_drive = false;
        self.ui.show_lobby(None);
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
            Session::Practice { mode: ModeKind::Convoy } => {
                self.ui.show_loading("Fuelling the tanker", "Get it down the road. Then stop theirs.", Some(0.6));
                NetClient::practice_convoy(rand_seed(self.time), 4, PRACTICE_DIFFICULTY, self.practice_map)
            }
            Session::Practice { mode: ModeKind::Wellhead } => {
                self.ui.show_loading("Priming the pumps", "Three derricks. Hold them, fill the tank first.", Some(0.6));
                NetClient::practice_wellhead(rand_seed(self.time), 4, PRACTICE_DIFFICULTY, self.practice_map)
            }
            Session::Practice { mode: ModeKind::LastCrew } => {
                self.ui.show_loading("Watching the horizon", "Eight crews, no respawns, a storm wall closing in.", Some(0.6));
                NetClient::practice_last_crew(rand_seed(self.time), PRACTICE_BOTS, PRACTICE_DIFFICULTY, self.practice_map)
            }

            Session::Online(join) => {
                let sub = match join {
                    Join::Code(c) => format!("Tuning the radio to {c}..."),
                    Join::Host { mode: ModeKind::RaiderWaves, .. } => "Fortifying the outpost...".to_string(),
                    Join::Host { mode: ModeKind::TeamBrawl, .. } => "Painting the hulls...".to_string(),
                    Join::Host { mode: ModeKind::Convoy, .. } => "Fuelling the tanker...".to_string(),
                    Join::Host { mode: ModeKind::Wellhead, .. } => "Priming the pumps...".to_string(),
                    Join::Host { mode: ModeKind::LastCrew, .. } => "Watching the horizon...".to_string(),
                    Join::Party { .. } => "Opening the yard...".to_string(),
                    Join::Host { .. } => "Opening the gates...".to_string(),
                    _ => "Looking for a crew to fight...".to_string(),
                };
                self.ui.show_loading("Connecting", &sub, Some(0.2));
                NetClient::connect(&net::transport::default_url(), &self.settings.name, join.clone())
            }
        };
        self.ui.set_loading_tip(tip(self.time));
        let roll = (self.time * 977.0).fract();
        self.want_look = Some(if let Some(t) = self.debug.tod {
            t
        } else if roll < 0.6 {
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
            horn: g.horn,
        };
        sim.set_loadout(you, loadout);
        // `set_loadout` only takes effect at the next respawn; offline the tank
        // is already on the field, so re-class it now (else every Garage
        // chassis rolls out as a Brawler until the first death).
        let tuning = &sim.tuning;
        if let Some(p) = sim.players.iter_mut().find(|p| p.id == you) {
            if p.tank.class != loadout.chassis {
                p.tank.set_class(&mut sim.phys, tuning, loadout.chassis);
            }
        }
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
            sim.tuning.wellhead.time_limit = t;
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
        if d.landship {
            sim.debug_spawn_landship();
        }
        if d.wall_fast {
            sim.tuning.last_crew.hold = vec![2.0, 6.0, 6.0, 6.0, 6.0];
            sim.tuning.last_crew.shrink = vec![12.0, 10.0, 10.0, 10.0, 15.0];
            if let Some(lc) = &mut sim.last_crew {
                lc.timer = lc.timer.min(2.0);
            }
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
        self.lobby_drive = false;
        self.ui.show_lobby(None);
        self.sound.clear_engines();
        self.hints.clear_queue(&mut self.ui);
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
            // Every press clicks; starting a match confirms once it's up
            // (`loading_frame`), a branch pick confirms, a refusal errors.
            match &a {
                UiAction::Chat(_) | UiAction::BranchPicked(_) => {}
                UiAction::Refused => self.sound.ui_error(),
                _ => self.sound.ui_click(),
            }
            match a {
                UiAction::Click => {}
                UiAction::Refused => {}
                UiAction::Play => self.start_online(Join::Play),
                UiAction::Practice => self.start_practice(),
                UiAction::PracticeMap(key) => {
                    self.practice_map = MapId::from_key(&key).unwrap_or_default();
                    self.ui.set_ticker(&ticker(self.practice_map));
                }
                UiAction::Host(cfg) => {
                    let mode = match cfg.mode.as_str() {
                        "raider_waves" => ModeKind::RaiderWaves,
                        "team_brawl" => ModeKind::TeamBrawl,
                        "convoy" => ModeKind::Convoy,
                        "wellhead" => ModeKind::Wellhead,
                        "last_crew" => ModeKind::LastCrew,
                        _ => ModeKind::ScrapBrawl,
                    };
                    let map = MapId::from_key(&cfg.map).unwrap_or_default();
                    self.host_party(!cfg.private, mode, map, cfg.bots);
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
                    self.garage_dirty = true;
                    self.apply_garage();
                    self.send_loadout();
                }
                UiAction::LobbyReady => {
                    let ready = self.my_crew().is_some_and(|c| c.ready);
                    self.lobby_cmd(LobbyCmd::Ready(!ready));
                }
                UiAction::LobbyStart => {
                    if self.net.as_ref().and_then(|n| n.state().lobby.as_ref()).is_some_and(|l| l.can_start()) {
                        self.lobby_cmd(LobbyCmd::Start);
                    } else {
                        self.sound.ui_error();
                    }
                }
                UiAction::LobbySettings(cfg) => {
                    if let Some(s) = self.lobby_settings(&cfg) {
                        self.lobby_cmd(LobbyCmd::Settings(s));
                    }
                }
                UiAction::LobbyTeam(t) => self.lobby_cmd(LobbyCmd::Team(party::team_pick(&t))),
                UiAction::LobbyVote(i) => self.lobby_cmd(LobbyCmd::Vote(i)),
                UiAction::InviteCopied => {}
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
            self.sound.ui_confirm();
        }
    }

    /// The click of a button outside the game's own UI (the pause card).
    pub fn ui_click(&mut self) {
        self.sound.ui_click();
    }

    /// One frame: `now` a monotonic clock in seconds, `dt` the frame time.
    pub fn frame(&mut self, now: f64, dt: f32, wr: &mut WorldRenderer) -> RenderWorld {
        let t0 = crate::world::clock_ms();
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
        if matches!(self.screen, Screen::Title | Screen::Loading) {
            self.sound.tick();
        }
        let t1 = crate::world::clock_ms();
        self.prof_game[0] = t1 - t0;

        let world = match self.screen {
            Screen::Title => self.title_frame(dt, wr),
            Screen::Loading => self.loading_frame(now, dt, wr),
            Screen::InGame | Screen::End | Screen::Lobby => self.game_frame(now, dt, wr),
        };
        let t2 = crate::world::clock_ms();
        self.prof_game[5] = t2 - t1;
        if let Some(line) = self.radio.take() {
            self.ui.radio(&line);
        }
        self.prof_game[6] = crate::world::clock_ms() - t2;
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
        self.sound.music(MusicTrack::Title, 0.5);
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
        // Joiners (by code or quick match) don't know the room's mode
        // from the session: read it off the first snapshot.
        let room_mode = st.world.as_ref().map_or(ModeKind::ScrapBrawl, |w| {
            if w.raid.is_some() {
                ModeKind::RaiderWaves
            } else if w.convoy.is_some() {
                ModeKind::Convoy
            } else if w.wellhead.is_some() {
                ModeKind::Wellhead
            } else if w.storm_circle.is_some() {
                ModeKind::LastCrew
            } else if !w.teams.is_empty() {
                ModeKind::TeamBrawl
            } else {
                ModeKind::ScrapBrawl
            }
        });
        match &status {
            Status::Closed(why) => {
                let why = why.clone();
                if self.status_note != why {
                    self.ui.show_loading("No signal", &why, Some(0.0));
                    self.status_note = why;
                    self.sound.ui_error();
                }
            }
            Status::Connecting | Status::Reconnecting => {}
            Status::Practice | Status::Playing => {
                if let (Some(seed), true) = (seed, has_world) {
                    self.status_note.clear();
                    self.use_map(seed, map_id, wr);
                    self.screen = Screen::InGame;
                    self.sound.ui_confirm();
                    self.ui.show_hud();
                    self.ui.set_ffa(true);
                    let map_name = map_id.name();
                    let mode = match (&self.session, &code) {
                        (Some(Session::Practice { mode: ModeKind::TeamBrawl }), _) => format!("Team Practice \u{b7} {map_name}"),
                        (Some(Session::Practice { mode: ModeKind::Convoy }), _) => format!("Convoy Practice \u{b7} {map_name}"),
                        (Some(Session::Practice { mode: ModeKind::RaiderWaves }), _) => format!("Raid Practice \u{b7} {map_name}"),
                        (Some(Session::Practice { mode: m @ (ModeKind::Wellhead | ModeKind::LastCrew) }), _) => format!("{} Practice \u{b7} {map_name}", party::mode_name(*m)),
                        (Some(Session::Practice { .. }), _) | (_, None) => format!("Practice \u{b7} {map_name}"),
                        (Some(Session::Online(Join::Host { mode, .. })), Some(code)) => format!("{} \u{b7} {map_name} \u{b7} {code}", party::mode_name(*mode)),
                        (_, Some(code)) => format!("{} \u{b7} {map_name} \u{b7} {code}", party::mode_name(room_mode)),
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
        // The lobby: the keys drive about at once; the guns only with the wheel taken.
        let lobby = self.screen == Screen::Lobby;
        let live = ((self.screen == Screen::InGame && !self.paused && (self.controls.captured || pad_live)) || lobby) && !typing;
        let sens = self.settings.mouse_sens.clamp(0.1, 3.0);
        let fov_k = self.camera.map_or(1.0, |c| c.fov_y / 62f32.to_radians());
        if self.screen != self.last_screen {
            self.last_screen = self.screen;
            self.mouse_settle = MOUSE_SETTLE;
        }
        self.mouse_settle -= dt;
        let pitch0 = self.chase.pitch;
        // In the yard the mouse is for the plate until the wheel is taken.
        if self.controls.captured && !self.paused && (self.screen != Screen::Lobby || self.lobby_drive) && self.mouse_settle <= 0.0 {
            let (mx, my) = self.controls.mouse;
            let (mx, my) = (mx.clamp(-MOUSE_MAX_STEP, MOUSE_MAX_STEP), my.clamp(-MOUSE_MAX_STEP, MOUSE_MAX_STEP));
            self.chase.yaw -= mx * 0.0024 * sens * fov_k;
            self.chase.pitch = (self.chase.pitch - my * 0.0024 * sens * fov_k).clamp(PITCH_MIN, PITCH_MAX);
        }
        if !typing && !self.paused {
            self.chase.yaw += self.controls.axis(&["ArrowRight"], &["ArrowLeft"]) * 1.8 * dt;
            self.chase.pitch =
                (self.chase.pitch + self.controls.axis(&["ArrowDown"], &["ArrowUp"]) * 1.0 * dt).clamp(PITCH_MIN, PITCH_MAX);
        }
        let pad = if pad_live { self.pad } else { None };
        if let (Some(p), true) = (pad, live) {
            // Right stick: the camera, slowed near a visible enemy (aim assist).
            let assist = self.aim_assist();
            let (rx, ry) = (p.axis(2), p.axis(3));
            let rate = 2.6 * sens * fov_k * assist;
            self.chase.yaw -= rx * rx.abs() * rate * dt;
            self.chase.pitch = (self.chase.pitch - ry * ry.abs() * 1.6 * sens * fov_k * assist * dt).clamp(PITCH_MIN, PITCH_MAX);
        }
        // Left alone, the view drifts back to the resting pitch.
        if (self.chase.pitch - pitch0).abs() > 1e-4 {
            self.pitch_idle = 0.0;
        } else {
            self.pitch_idle += dt;
            if self.pitch_idle > PITCH_IDLE && !self.paused {
                self.chase.pitch += (PITCH_REST - self.chase.pitch) * (1.0 - (-dt * 1.5).exp());
            }
        }
        let c = &self.controls;
        let mut input = Input { aim_point: self.input.aim_point, ..Input::default() };
        if live {
            input.throttle = c.axis(&["KeyS"], &["KeyW"]);
            input.steer = c.axis(&["KeyA"], &["KeyD"]);
            let guns = !lobby || self.lobby_drive || self.free_mouse;
            input.fire = c.buttons[0] && guns;
            input.alt_fire = c.buttons[2] && guns;
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
        let t_net = crate::world::clock_ms();
        net.update(now);
        self.prof_net_ms = crate::world::clock_ms() - t_net;
        let events = net.drain_events();
        let st = net.state();
        if let Status::Closed(why) = &st.status {
            let why = why.clone();
            self.leave();
            self.sound.ui_error();
            self.ui.show_loading("Signal lost", &why, Some(0.0));
            self.screen = Screen::Loading;
            self.status_note = why;
            return self.title_frame(dt, wr);
        }
        let (welcomes, seed, map, party) = (st.welcomes, st.seed, st.map, st.lobby.is_some());
        if welcomes != self.welcomes_seen {
            // A party room rebuilt its world (match, lobby, a new map).
            self.welcomes_seen = welcomes;
            if let Some(seed) = seed {
                self.use_map(seed, map, wr);
            }
            if party {
                self.party_new_world();
            }
            self.send_loadout();
        }
        self.party_frame();
        let Some(net) = &self.net else { return RenderWorld::default() };
        let st = net.state();
        let Some(snap) = st.world.clone() else { return RenderWorld::default() };
        let rtt = st.rtt_ms;
        for p in &st.players {
            if !self.m.names.contains_key(&p.id) {
                self.m.names.insert(p.id, p.name.clone());
            }
        }
        self.sync_chat();

        self.debug_scrap(dt);
        let mut lap = crate::world::clock_ms();
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
        } else if snap.raid.is_none() {
            self.paint_crews(&mut world, you);
        }
        world.time = self.time;
        world.dt = dt * self.juice.time_scale(dt);
        let storm = self.storm_level(&snap, you, &world);
        wr.renderer.set_storm(storm);
        let me = you.and_then(|y| snap.tank(y)).cloned();
        let my_view = you.and_then(|y| world.tanks.iter().find(|t| t.player == y)).cloned();

        // ---- Camera.
        let alive = me.as_ref().is_some_and(|t| t.alive);
        if let Some(v) = &my_view {
            if (alive && !self.m.alive) || !self.m.aligned {
                // Spawned: look along the hull.
                let fwd = v.rot * Vec3::X;
                self.chase.yaw = (-fwd.z).atan2(fwd.x);
                self.chase.pitch = PITCH_REST;
                self.m.aligned = true;
                if self.screen == Screen::Lobby {
                    // The yard: swing the boom round to open ground and start it out.
                    self.chase.yaw = party::clear_yaw(&self.map, &self.chase, v.pos);
                    self.chase.reset_boom();
                }
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
            self.m.alive_for = 0.0;
        } else {
            self.m.alive_for += dt;
        }
        self.m.alive = alive;
        let killer_view = self.m.killer.and_then(|k| world.tanks.iter().find(|t| t.player == k && t.alive)).cloned();
        // Last Crew: out for good. After the kill-cam, ride along with the leader.
        let killer_view = if !alive && snap.storm_circle.is_some() && (self.m.dead_for >= KILLCAM_SECS || killer_view.is_none()) {
            self.leader(&snap, you).and_then(|l| world.tanks.iter().find(|t| t.player == l && t.alive)).cloned().or(killer_view)
        } else {
            killer_view
        };
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
        let mut cam = self.chase.update(dt, self.time, pivot, &camcol::SceneCollider(&self.map));
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
                    .is_some_and(|t| matches!(t.vehicle, arena_core::parts::VehicleKind::WarRig | arena_core::parts::VehicleKind::LandShip));
                let r = if big { 30.0 } else { 13.0 } - self.m.dead_for * 1.5;
                let eye = k.pos + Vec3::new(a.cos() * r, if big { 9.0 } else { 4.5 }, -a.sin() * r);
                let free = camcol::SceneCollider(&self.map).free_fraction(k.pos + Vec3::Y * 2.0, eye);
                let eye = k.pos + Vec3::Y * 2.0 + (eye - k.pos - Vec3::Y * 2.0) * free.clamp(0.2, 1.0);
                world.camera = Some(Camera::look(eye, k.pos + Vec3::Y * 1.4, 48.0));
            }
        }
        // ---- Aim: the screen centre onto the world.
        let dir = (cam.target - cam.eye).normalize_or(Vec3::X);
        let skip = (pivot + Vec3::Y * self.chase.pivot_height - cam.eye).dot(dir).max(0.0);
        self.input.aim_point = aim::cast(&self.map, &world.tanks, you, cam.eye, dir, skip);

        let t = crate::world::clock_ms();
        self.prof_game[1] = t - lap;
        lap = t;
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
            self.music_event(e, you);
            self.on_event(e, you, &world, cam);
        }
        let t = crate::world::clock_ms();
        self.prof_game[2] = t - lap;
        lap = t;
        self.sound.frame(listener, dir, &world.tanks, you, input.throttle);
        // The wind: a steady desert breeze, stronger up high and at speed.
        let (speed, high) = my_view.as_ref().map_or((0.0, 0.0), |v| (v.speed, v.pos.y - self.map.height_at(0.0, 0.0)));
        let wind = 0.35 + (speed / 14.0).min(1.0) * 0.25 + (high / 6.0).clamp(0.0, 1.0) * 0.15;
        self.sound.ambience(wind, storm);
        self.music_frame(dt, &snap, &world, you);
        let t = crate::world::clock_ms();
        self.prof_game[3] = t - lap;
        lap = t;

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
        self.wellhead_hud(&snap, &world, you, wr);
        self.last_crew_hud(&snap, &world, you);
        self.convoy_hud(&snap, &mut world, you, wr);
        self.onboarding(me.as_ref(), &snap, &world, dt);
        self.prof_game[4] = crate::world::clock_ms() - lap;
        if let Some(left) = &mut self.m.branch {
            *left -= dt;
            let closed = me.as_ref().is_some_and(|t| t.choice.is_none());
            if *left <= 0.0 || (closed && *left < BRANCH_SECS - 0.3) {
                self.m.branch = None;
                self.ui.hide_branch();
            }
        }
        if let Some(t) = me.as_ref().filter(|t| !t.alive && snap.storm_circle.is_some()) {
            // Last Crew: no countdown, just where you finished and who you're watching.
            let by = self.m.killed_by.clone();
            let place = self.m.out_place.unwrap_or_else(|| snap.storm_circle.as_ref().map_or(0, |c| c.alive.saturating_add(1)));
            let watching = self.leader(&snap, you).map(|l| self.name_of(l));
            let _ = t;
            if !self.m.ended {
                self.ui.show_out(place, by.as_ref().map(|(n, w)| (n.as_str(), Team::Ffa, w.as_str())), watching.as_deref());
            }
        } else if let Some(t) = &me {
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
        let over = !matches!(snap.mode.phase, Phase::Playing);
        if self.m.was_over && !over {
            // The server rolled the next match: per-match calls start over.
            self.m.out_place = None;
            self.m.out_order.clear();
            self.m.crews_called.clear();
            self.m.fuel_called = [false; 2];
            self.m.first_blood = false;
        }
        self.m.was_over = over;
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
            if snap.storm_circle.is_some() {
                // Last Crew: by where they finished, the last one rolling first.
                ids.sort_by_key(|(id, _)| self.finish_rank(*id, &snap));
            }
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
        if snap.storm_circle.is_none() {
            // (Last Crew's clock is the wall's: `last_crew_hud`.)
            ui.set_timer(snap.mode.ticks_left as f32 / 60.0);
        }
        if snap.teams.is_empty() {
            // Free-for-all: plates hidden; your kills against the leader's.
            ui.set_ffa(true);
            let you = t.player;
            let mine = snap.scores.iter().find(|s| s.player == you).map_or(0, |s| s.kills);
            let best = snap.scores.iter().filter(|s| s.player != you).map(|s| s.kills).max().unwrap_or(0);
            ui.set_scores(u32::from(mine), u32::from(best));
        } else {
            ui.set_ffa(false);
            if snap.wellhead.is_none() {
                // (Wellhead's plates show fuel: `wellhead_hud`.)
                let k = |team: u8| snap.teams.iter().find(|s| s.team == team).map_or(0, |s| u32::from(s.kills));
                ui.set_scores(k(0), k(1));
            }
        }
    }

    fn score_rows(&self, snap: &Snapshot, rtt: f32) -> Vec<ScoreRow> {
        let you = self.you();
        let players = self.net.as_ref().map(|n| n.state().players.clone()).unwrap_or_default();
        let mut rows: Vec<ScoreRow> = players
            .iter()
            .filter(|p| snap.raid.is_none() || p.team != Some(TEAM_RAIDERS))
            .filter(|p| snap.convoy.as_ref().is_none_or(|c| c.truck != p.id))
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
                    is_bot: p.is_bot,
                    // Only your own round trip is known (none offline).
                    ping_ms: (is_you && !p.is_bot && rtt > 0.0).then(|| rtt.round() as u32),
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

    /// Onboarding tips the situation calls for (each shows once per browser).
    fn onboarding(&mut self, me: Option<&TankSnap>, snap: &Snapshot, world: &RenderWorld, dt: f32) {
        let alive = me.is_some_and(|t| t.alive);
        if !alive {
            self.hints.drop_tip(hints::LOW_HEALTH, &mut self.ui);
        }
        if let (Some(t), true) = (me, alive) {
            // Mode first, then the controls, as you roll out.
            if snap.wellhead.is_some() {
                self.hints.want(hints::WELLHEAD);
            } else if snap.storm_circle.is_some() {
                self.hints.want(hints::LAST_CREW);
            } else if snap.convoy.is_some() {
                self.hints.want(hints::CONVOY);
            } else if self.snap_is_raid(snap) {
                self.hints.want(hints::RAID);
            }
            self.hints.want(hints::DRIVE);
            let pos = Vec3::from(t.pos);
            if self.hints.seen(hints::DRIVE)
                && world.pickups.iter().any(|p| matches!(p.kind, arena_core::PickupKind::Scrap(_)) && p.pos.distance(pos) < 30.0)
            {
                self.hints.want(hints::SCRAP);
            }
            if t.rare_ammo > 0 || t.air_strike {
                self.hints.want(hints::SPECIAL);
            }
            // Only for a hull failing now, not a fresh spawn.
            let low = (t.health as f32) < 0.3 * f32::from(t.max_health.max(1));
            if low && self.m.alive_for > 2.0 {
                self.hints.want(hints::LOW_HEALTH);
            } else {
                self.hints.drop_tip(hints::LOW_HEALTH, &mut self.ui);
            }
        }
        let can_show = alive && self.screen == Screen::InGame && !self.paused && !self.m.ended;
        let pad = self.pad_live();
        self.hints.tick(dt, can_show, pad, &mut self.ui);
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
                self.hints.want_now(hints::BRANCH);
            }
            Event::BranchChosen { player, part, .. } if is_you(*player) => {
                self.ui.hide_branch();
                self.m.branch = None;
                self.radio.say(&radio::branch_taken(names::part_name(*part)), false);
            }
            Event::PartBrokeOff { player, part, .. } if is_you(*player) => {
                self.radio.say(&radio::part_lost(names::part_name(*part)), false);
                self.hints.want(hints::PART_LOST);
            }
            Event::PickedUp { player, kind: arena_core::PickupKind::Rare(_) | arena_core::PickupKind::AirStrike, .. } if is_you(*player) => {
                self.hints.want(hints::SPECIAL);
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
            Event::ConvoyRoundStarted { round, escort_team, .. } => {
                let mine = you.and_then(|y| self.net.as_ref().and_then(|n| n.state().world.as_ref()?.tank(y).map(|t| t.team)));
                self.radio.say(&radio::convoy_round(*round, mine == Some(*escort_team)), true);
            }
            Event::ConvoyMoving { .. } => self.radio.say(&radio::convoy_moving(), false),
            Event::ConvoyStopped { .. } => {
                if self.m.truck_stopped_said.is_none_or(|t| self.time - t >= TRUCK_STOPPED_GAP) {
                    self.m.truck_stopped_said = Some(self.time);
                    let escort = self.convoy_escort();
                    self.radio.say(&radio::convoy_stopped(escort), true);
                }
            }
            Event::ConvoyHalfway { .. } => self.radio.say(&radio::convoy_halfway(), true),
            Event::ConvoyDestroyed { .. } => self.radio.say(&radio::convoy_destroyed(), true),
            Event::ConvoyDelivered { .. } => self.radio.say(&radio::convoy_delivered(), true),
            Event::WellCaptured { well, team } => {
                let place = self.well_name(*well);
                let ours = self.my_team() == Some(*team);
                self.radio.say(&radio::well_captured(&place, ours), true);
            }
            Event::WellLost { well, team } => {
                let place = self.well_name(*well);
                let ours = self.my_team() == Some(*team);
                self.radio.say(&radio::well_lost(&place, ours), ours);
            }
            Event::WellContested { well } => {
                let place = self.well_name(*well);
                self.radio.say(&radio::well_contested(&place), false);
            }
            Event::AllWellsHeld { team } => {
                let ours = self.my_team() == Some(*team);
                self.radio.say(&radio::all_wells(ours), true);
            }
            Event::CircleShrinking { phase, seconds, .. } => {
                let last = self.net.as_ref().and_then(|n| n.state().world.as_ref()).and_then(|w| w.storm_circle.as_ref()).is_some_and(|c| *phase >= c.phases);
                self.radio.say(&radio::wall_moving(*seconds, last), true);
            }
            Event::CircleClosed { .. } => self.radio.say(&radio::wall_closed(), false),
            Event::Eliminated { player, place } => {
                self.m.out_order.push(*player);
                if is_you(*player) {
                    self.m.out_place = Some(*place);
                    let storm = self.m.killer.is_none();
                    self.radio.say(&radio::out_of_it(*place, storm), true);
                } else {
                    let left = place.saturating_sub(1);
                    if matches!(left, 2 | 3 | 5) && !self.m.crews_called.contains(&left) {
                        self.m.crews_called.push(left);
                        self.radio.say(&radio::crews_left(left), left <= 3);
                    }
                }
            }
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
        let last_crew = self.net.as_ref().and_then(|n| n.state().world.as_ref()).is_some_and(|w| w.storm_circle.is_some());
        let title = match (&wname, you_won, last_crew) {
            (_, true, true) => "Last crew rolling: you".to_string(),
            (Some(n), _, true) => format!("Last crew rolling: {n}"),
            (_, true, _) => format!("You take {place}"),
            (Some(n), _, _) => format!("{n} takes {place}"),
            (None, _, _) => format!("{} takes everyone", self.map.name),
        };
        let note = self.next_round_note();
        let note = match (last_crew, you_won, self.m.out_place) {
            (true, false, Some(p)) => format!("You finished {}  \u{b7}  {note}", crate::ui::ordinal(p)),
            (true, true, _) => format!("Last of eight  \u{b7}  {note}"),
            _ => note.to_string(),
        };
        self.show_end(&title, &note);
        self.sound.stinger(if you_won { Stinger::Victory } else { Stinger::Defeat });
        if last_crew {
            self.radio.say(&radio::last_crew(wname.as_deref().unwrap_or("Nobody"), you_won), true);
        } else {
            self.radio.say(&radio::match_over(wname.as_deref(), you_won), true);
        }
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
        let note = self.next_round_note();
        let note = match (self.convoy_results(), self.wellhead_results()) {
            (Some(r), _) | (None, Some(r)) => format!("{r}  \u{b7}  {note}"),
            (None, None) => note.to_string(),
        };
        self.show_end_team(&title, &note, winner);
        let won = winner.map(|w| Some(w) == mine);
        self.sound.stinger(if won == Some(true) { Stinger::Victory } else { Stinger::Defeat });
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
        self.sound.stinger(Stinger::Defeat);
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
        if let Some(s) = snap.as_ref().filter(|s| s.storm_circle.is_some()) {
            // Last Crew: the table reads in finishing order.
            let players = self.net.as_ref().map(|n| n.state().players.clone()).unwrap_or_default();
            let rank = |name: &str| players.iter().find(|p| self.name_of(p.id) == name).map_or(u32::MAX, |p| self.finish_rank(p.id, s));
            rows.sort_by_key(|r| rank(&r.name));
        }
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
        let last_crew = snap.as_ref().is_some_and(|s| s.storm_circle.is_some());
        // Ties share a rank (#1 / #1); Last Crew's finishing order never ties.
        let ranks = if last_crew {
            (1..=rows.len() as u32).collect()
        } else {
            // By wrecks: crews level on kills share the rank (deaths don't split them).
            crate::ui::shared_ranks(&rows.iter().map(|r| r.kills).collect::<Vec<_>>())
        };
        let end: Vec<EndRow> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| EndRow {
                rank: ranks[i],
                name: r.name.clone(),
                team: r.team,
                kills: r.kills,
                deaths: r.deaths,
                tag: if i == 0 && last_crew {
                    Some("Last one rolling".into())
                } else if ranks[i] == 1 && !last_crew && r.kills > 0 {
                    Some("Top gun".into())
                } else if Some(i) == biggest {
                    Some("Biggest build".into())
                } else {
                    None
                },
            })
            .collect();
        self.ui.set_end_again(if self.lobby().is_some() { "Back to the yard" } else { "Again" });
        self.ui.show_endscreen(title, winner, &end, note);
        self.ui.set_end_kicker(&format!("Match over \u{b7} {}", self.map.name));
    }

    /// The battle music's intensity from the action round you: recent hits
    /// dealt or taken, kills and shots (decaying), enemies close by, a boss on
    /// the field, low health. Smoothed; 0.1 calm .. 1 in a brawl.
    fn music_frame(&mut self, dt: f32, snap: &Snapshot, world: &RenderWorld, you: Option<PlayerId>) {
        if !self.m.started {
            self.m.started = true;
            if snap.convoy.is_none() {
                self.sound.stinger(Stinger::RoundStart);
            }
        }
        if self.screen == Screen::End {
            self.sound.music(MusicTrack::Silence, 0.0);
            return;
        }
        self.m.heat *= (-dt / 7.0).exp();
        let me = you.and_then(|y| snap.tank(y));
        let my_pos = you.and_then(|y| world.tanks.iter().find(|t| t.player == y)).map(|t| t.pos);
        let my_team = me.map(|t| t.team);
        let near = my_pos.map_or(0, |p| {
            snap.tanks
                .iter()
                .filter(|t| t.alive && Some(t.player) != you && (my_team.is_none_or(|m| m == 255) || Some(t.team) != my_team))
                .filter(|t| t.position().distance(p) < 70.0)
                .count()
        });
        let hp = me.map_or(1.0, |t| t.health as f32 / f32::from(t.max_health.max(1)));
        let mut target = 0.1 + self.m.heat.min(0.55) + (near as f32 * 0.09).min(0.3);
        if !snap.bosses.is_empty() {
            target += 0.25;
        }
        if me.is_some_and(|t| t.alive) && hp < 0.35 {
            target += 0.2;
        }
        let target = target.clamp(0.1, 1.0);
        let rate = if target > self.m.intensity { 1.2 } else { 0.25 };
        self.m.intensity += (target - self.m.intensity) * (1.0 - (-dt * rate).exp());
        self.sound.music(MusicTrack::Battle, self.m.intensity);
    }

    /// Music heat from one event near you.
    fn music_event(&mut self, e: &Event, you: Option<PlayerId>) {
        let is_you = |p: &PlayerId| Some(*p) == you;
        let heat = match e {
            Event::Hit { target, attacker, .. } if is_you(target) || attacker.as_ref().is_some_and(is_you) => 0.18,
            Event::Destroyed { victim, killer, .. } if is_you(victim) || killer.as_ref().is_some_and(is_you) => 0.3,
            Event::Fired { player, .. } if is_you(player) => 0.04,
            Event::HazardExploded { .. } => 0.08,
            _ => 0.0,
        };
        self.m.heat += heat;
        match e {
            Event::WaveStarted { boss: false, .. } => self.sound.stinger(Stinger::WaveStart),
            Event::BossSpawned { .. } => self.sound.stinger(Stinger::BossAppears),
            Event::ConvoyRoundStarted { .. } => self.sound.stinger(Stinger::RoundStart),
            _ => {}
        }
    }

    /// Convoy: does your team escort the truck this round?
    fn convoy_escort(&self) -> bool {
        let Some(snap) = self.net.as_ref().and_then(|n| n.state().world.as_ref()) else { return false };
        let Some(c) = &snap.convoy else { return false };
        self.you().and_then(|y| snap.tank(y)).is_some_and(|t| t.team == c.side)
    }

    /// Convoy: the round results for the end screen ("R1 Rustbacks delivered
    /// in 3:12 · R2 Dust Saints stopped at 64%").
    fn convoy_results(&self) -> Option<String> {
        let snap = self.net.as_ref()?.state().world.as_ref()?;
        let c = snap.convoy.as_ref()?;
        let team = |t: u8| if t == 0 { "Rustbacks" } else { "Dust Saints" };
        let parts: Vec<String> = c
            .rounds
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let s = r.seconds.max(0.0) as i32;
                if r.delivered {
                    format!("R{} {} delivered in {}:{:02}", i + 1, team(r.escort_team), s / 60, s % 60)
                } else {
                    format!("R{} {} got {:.0}% down the road", i + 1, team(r.escort_team), 100.0 * r.distance / c.length.max(1.0))
                }
            })
            .collect();
        (!parts.is_empty()).then(|| parts.join("  \u{b7}  "))
    }

    /// The truck's road for this map, joined on the nav grid exactly as the
    /// sim does (`Sim::convoy_path`), with cumulative distances.
    fn route(&mut self) {
        let key = (self.map_seed, self.map.id);
        if self.convoy_route.as_ref().is_none_or(|r| r.0 != key) {
            let nav = arena_core::bots::NavGrid::build(&self.map, arena_core::Tuning::default().bots.nav_cell);
            let mut ctrl = self.map.convoy_route.clone();
            if ctrl.len() < 2 {
                let h = self.map.play_half - 30.0;
                ctrl = vec![Vec3::new(-h, 0.0, 0.0), Vec3::new(h, 0.0, 0.0)];
            }
            let flat = |v: Vec3| glam::Vec2::new(v.x, v.z);
            let mut pts = vec![flat(ctrl[0])];
            for w in ctrl.windows(2) {
                let from = *pts.last().unwrap();
                pts.extend(nav.path(from, flat(w[1])));
            }
            pts.dedup_by(|a, b| a.distance(*b) < 0.5);
            let path: Vec<Vec3> = pts.iter().map(|p| Vec3::new(p.x, self.map.height_at(p.x, p.y), p.y)).collect();
            let mut cum = vec![0.0];
            for w in path.windows(2) {
                let d = glam::Vec2::new(w[1].x - w[0].x, w[1].z - w[0].z).length();
                cum.push(cum.last().unwrap() + d);
            }
            self.convoy_route = Some((key, path, cum));
        }
    }

    /// Convoy: the bar under the clock, the clock and label, the truck's
    /// marker (seen through walls: it's the objective) and, for the escorts,
    /// chevrons on the road ahead of it.
    fn convoy_hud(&mut self, snap: &Snapshot, world: &mut RenderWorld, you: Option<PlayerId>, wr: &WorldRenderer) {
        let Some(c) = snap.convoy.clone() else {
            self.ui.set_convoy(None);
            return;
        };
        let escort = you.and_then(|y| snap.tank(y)).is_some_and(|t| t.team == c.side);
        use arena_core::convoy::ConvoyPhase;
        let state = match c.phase {
            ConvoyPhase::Setup => "Setting up",
            ConvoyPhase::RoundOver => "Round over",
            ConvoyPhase::Done => "Done",
            ConvoyPhase::Rolling if !c.alive => "Burning",
            ConvoyPhase::Rolling if c.contested => "Contested",
            ConvoyPhase::Rolling if c.repairing => "Repairing",
            ConvoyPhase::Rolling if c.moving => "Rolling",
            ConvoyPhase::Rolling => "Stopped",
        };
        self.ui.set_convoy(Some(&crate::ui::ConvoyHud {
            escort,
            health: f32::from(c.health) / f32::from(c.max_health.max(1)),
            progress: c.progress,
            state: state.to_string(),
        }));
        self.ui.set_timer(c.ticks_left as f32 / 60.0);
        let label = format!("Convoy \u{b7} Round {}/2 \u{b7} {}", c.round, self.map.name);
        if label != self.m.raid_label {
            self.ui.set_mode_label(&label);
            self.m.raid_label = label;
        }
        // The marker over the truck, projected to the screen.
        let mark = world.camera.filter(|_| c.alive && self.screen == Screen::InGame).and_then(|cam| {
            let (w, h) = wr.renderer.size();
            let aspect = w as f32 / h.max(1) as f32;
            let vp = glam::camera::rh::proj::directx::perspective(cam.fov_y, aspect, 0.1, 3000.0) * glam::camera::rh::view::look_at_mat4(cam.eye, cam.target, Vec3::Y);
            let p = Vec3::from(c.pos) + Vec3::Y * 5.0;
            let clip = vp * p.extend(1.0);
            if clip.w <= 0.1 {
                return None;
            }
            let ndc = clip.truncate() / clip.w;
            let x = (ndc.x * 0.5 + 0.5).clamp(0.03, 0.97);
            let y = (0.5 - ndc.y * 0.5).clamp(0.08, 0.95);
            Some((x, y, p.distance(cam.eye), escort))
        });
        self.ui.set_convoy_mark(mark);
        // Chevrons on the road ahead, for the escorts.
        if escort && c.alive && matches!(c.phase, ConvoyPhase::Setup | ConvoyPhase::Rolling) {
            let start = c.distance;
            self.route();
            let Some((_, path, cum)) = self.convoy_route.as_ref() else { return };
            let mut d = (start / 8.0).floor() * 8.0 + 8.0;
            let mut seg = 0;
            while d < start + 140.0 && d < *cum.last().unwrap_or(&0.0) {
                while seg + 1 < cum.len() - 1 && cum[seg + 1] < d {
                    seg += 1;
                }
                let (a, b) = (path[seg], path[seg + 1]);
                let t = ((d - cum[seg]) / (cum[seg + 1] - cum[seg]).max(1e-3)).clamp(0.0, 1.0);
                let p = a.lerp(b, t);
                let dir = b - a;
                let ground = self.map.height_at(p.x, p.z);
                world.route.push((Vec3::new(p.x, ground + 0.06, p.z), (-dir.z).atan2(dir.x), 1.0 - (d - start) / 140.0));
                d += 8.0;
            }
        }
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

    /// Your team (Wellhead, Convoy, Team Brawl), if you're on one.
    fn my_team(&self) -> Option<u8> {
        let snap = self.net.as_ref()?.state().world.as_ref()?;
        self.you().and_then(|y| snap.tank(y)).map(|t| t.team).filter(|t| *t < 2)
    }

    /// "the gas station" for Wellhead's derrick `k`.
    fn well_name(&self, k: u8) -> String {
        self.map.wells.get(k as usize).map_or_else(|| format!("well {}", (b'A' + k) as char), |w| w.name.clone())
    }

    /// Last Crew: where a crew stands in the finishing order (0 = still
    /// rolling with the most kills; the first out ranks last).
    fn finish_rank(&self, id: PlayerId, snap: &Snapshot) -> u32 {
        let alive = snap.tank(id).is_some_and(|t| t.alive);
        let kills = snap.scores.iter().find(|s| s.player == id).map_or(0, |s| u32::from(s.kills));
        match self.m.out_order.iter().position(|p| *p == id) {
            _ if alive => 100 - kills.min(99),
            Some(i) => 1000 - i as u32,
            None => 2000,
        }
    }

    /// Last Crew: who an eliminated crew watches (the living tank with the
    /// most kills, then the biggest build).
    fn leader(&self, snap: &Snapshot, you: Option<PlayerId>) -> Option<PlayerId> {
        snap.tanks
            .iter()
            .filter(|t| t.alive && Some(t.player) != you)
            .max_by_key(|t| {
                let kills = snap.scores.iter().find(|s| s.player == t.player).map_or(0, |s| s.kills);
                (kills, t.tiers.iter().map(|x| u32::from(*x)).sum::<u32>(), t.player)
            })
            .map(|t| t.player)
    }

    /// The sandstorm the renderer and the wind should show: the brawl storm,
    /// or in Last Crew how close you are to the wall (blinding outside it).
    fn storm_level(&self, snap: &Snapshot, you: Option<PlayerId>, world: &RenderWorld) -> f32 {
        let Some(c) = &snap.storm_circle else { return snap.storm };
        let at = you
            .and_then(|y| world.tanks.iter().find(|t| t.player == y && t.alive))
            .map(|t| t.pos)
            .or(self.camera.map(|c| c.eye))
            .unwrap_or(Vec3::ZERO);
        let inside = c.radius - glam::Vec2::new(at.x - c.center[0], at.z - c.center[1]).length();
        let level = if inside < 0.0 { 0.7 + (-inside / 40.0).min(0.3) } else { (1.0 - inside / 35.0).clamp(0.0, 1.0) * 0.4 };
        level.max(snap.storm)
    }

    /// Screen position (0..1 from top left, clamped to the edges) and
    /// distance of a world point, or `None` behind the camera.
    fn project(&self, wr: &WorldRenderer, cam: Camera, p: Vec3) -> Option<(f32, f32, f32)> {
        let (w, h) = wr.renderer.size();
        let aspect = w as f32 / h.max(1) as f32;
        let vp = glam::camera::rh::proj::directx::perspective(cam.fov_y, aspect, 0.1, 3000.0) * glam::camera::rh::view::look_at_mat4(cam.eye, cam.target, Vec3::Y);
        let clip = vp * p.extend(1.0);
        if clip.w <= 0.1 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(((ndc.x * 0.5 + 0.5).clamp(0.03, 0.97), (0.5 - ndc.y * 0.5).clamp(0.08, 0.95), p.distance(cam.eye)))
    }

    /// Wellhead: the fuel tanks and derricks under the clock, the score
    /// plates show fuel, and a marker over each derrick (seen through walls).
    fn wellhead_hud(&mut self, snap: &Snapshot, world: &RenderWorld, you: Option<PlayerId>, wr: &WorldRenderer) {
        let Some(w) = snap.wellhead.clone() else {
            self.ui.set_wellhead(None);
            return;
        };
        let mine = you.and_then(|y| snap.tank(y)).map(|t| t.team).filter(|t| *t < 2);
        let team = |t: u8| if t == 0 { Team::Rustbacks } else { Team::DustSaints };
        let wells: Vec<crate::ui::WellHud> = w
            .wells
            .iter()
            .map(|x| crate::ui::WellHud {
                owner: x.owner.map(team),
                fill: (x.progress.abs() * 40.0).round() / 40.0,
                side: if x.progress < -0.01 {
                    Some(Team::Rustbacks)
                } else if x.progress > 0.01 {
                    Some(Team::DustSaints)
                } else {
                    None
                },
                contested: x.contested,
            })
            .collect();
        let held = |t: u8| w.wells.iter().filter(|x| x.owner == Some(t)).count();
        let (state, alarm) = match (w.all_held, mine) {
            (Some(t), Some(m)) if t == m => ("All three ours \u{b7} double pumping".to_string(), false),
            (Some(_), _) => ("They hold all three \u{b7} break one!".to_string(), true),
            _ => match w.wells.iter().position(|x| x.contested) {
                Some(k) => (format!("Fight at {}", self.well_name(k as u8)), false),
                None => (format!("Pumping {} : {}", held(0), held(1)), false),
            },
        };
        self.ui.set_wellhead(Some(&crate::ui::WellheadHud { wells, fuel: w.fuel, target: w.target, state, alarm }));
        self.ui.set_scores(w.fuel[0], w.fuel[1]);
        let label = format!("Wellhead \u{b7} first to {} \u{b7} {}", w.target, self.map.name);
        if label != self.m.raid_label {
            self.ui.set_mode_label(&label);
            self.m.raid_label = label;
        }
        for t in 0..2u8 {
            if !self.m.fuel_called[t as usize] && w.fuel[t as usize] as f32 >= w.target as f32 * 0.85 {
                self.m.fuel_called[t as usize] = true;
                self.radio.say(&radio::fuel_nearly(mine == Some(t)), true);
            }
        }
        let marks: Vec<Option<(f32, f32, f32, Option<Team>, bool)>> = match world.camera.filter(|_| self.screen == Screen::InGame) {
            Some(cam) => w
                .wells
                .iter()
                .map(|x| self.project(wr, cam, Vec3::from(x.pos) + Vec3::Y * 12.5).map(|(sx, sy, d)| (sx, sy, d, x.owner.map(team), x.contested)))
                .collect(),
            None => Vec::new(),
        };
        self.ui.set_well_marks(&marks, wr.renderer.size());
    }

    /// Last Crew Rolling: the compass to safe ground, the wall's clock, crews
    /// left, the storm nag when you're outside.
    fn last_crew_hud(&mut self, snap: &Snapshot, world: &RenderWorld, you: Option<PlayerId>) {
        let Some(c) = snap.storm_circle.clone() else {
            self.ui.set_last_crew(None);
            return;
        };
        let flat = |v: Vec3| glam::Vec2::new(v.x, v.z);
        let me = you.and_then(|y| world.tanks.iter().find(|t| t.player == y && t.alive)).map(|t| t.pos);
        let cam = self.camera;
        let at = me.or(cam.map(|c| c.target)).unwrap_or(Vec3::ZERO);
        let closing = c.shrinking || c.ticks_left < 15 * 60;
        let (sc, sr) = if closing { (glam::Vec2::from(c.next_center), c.next_radius) } else { (glam::Vec2::from(c.center), c.radius) };
        let wall = c.radius - flat(at).distance(glam::Vec2::from(c.center));
        let to_safe = flat(at).distance(sc) - sr;
        let state = if wall < 0.0 {
            "storm"
        } else if to_safe > 0.0 {
            "move"
        } else {
            "safe"
        };
        let dist = match state {
            "storm" => format!("{} m to the wall", (-wall).ceil() as i32),
            "move" => format!("{} m to safety", to_safe.ceil() as i32),
            _ => "Inside".to_string(),
        };
        let arrow = cam.map_or(0.0, |cam| {
            let fwd = flat(cam.target - cam.eye).normalize_or(glam::Vec2::X);
            let right = glam::Vec2::new(-fwd.y, fwd.x);
            let to = (sc - flat(at)).normalize_or(fwd);
            to.dot(right).atan2(to.dot(fwd)).to_degrees()
        });
        let secs = (c.ticks_left as f32 / 60.0).ceil() as i32;
        let line = if c.phase > c.phases {
            format!("Wall closed \u{b7} {:.0} hp/s outside", c.dps)
        } else if c.shrinking {
            format!("Wall moving \u{b7} {}", crate::ui::clock(secs))
        } else {
            format!("Wall moves in {}", crate::ui::clock(secs))
        };
        if !(me.is_none() && self.m.ended) {
            self.ui.set_last_crew(Some(&crate::ui::LastCrewHud { arrow_deg: (arrow / 3.0).round() * 3.0, dist, line, crews: c.alive, state }));
        }
        self.ui.set_timer(if c.phase > c.phases { 0.0 } else { c.ticks_left as f32 / 60.0 });
        // FFA plates: your kills against the leader's are in `hud`; the label carries the phase.
        let label = format!("Last Crew \u{b7} wall {}/{} \u{b7} {}", c.phase.min(c.phases), c.phases, self.map.name);
        if label != self.m.raid_label {
            self.ui.set_mode_label(&label);
            self.m.raid_label = label;
        }
        self.m.storm_nag -= 1.0 / 60.0;
        if me.is_some() && wall < 0.0 && self.m.storm_nag <= 0.0 {
            self.m.storm_nag = 9.0;
            self.radio.say(&radio::outside_wall(), true);
        }
    }

    /// Wellhead: the final tanks for the end screen ("Rustbacks 500 · Dust Saints 214 fuel").
    fn wellhead_results(&self) -> Option<String> {
        let w = self.net.as_ref()?.state().world.as_ref()?.wellhead.as_ref()?;
        Some(format!("Rustbacks {} \u{b7} Dust Saints {} fuel", w.fuel[0], w.fuel[1]))
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
                let n = r.raiders_alive + r.raiders_queued;
                format!("Wave {} \u{b7} {n} {} \u{b7} Outpost {:.0}%", r.wave, if n == 1 { "raider" } else { "raiders" }, outpost)
            }
        };
        if let Some(b) = snap.bosses.first() {
            let hp = f32::from(b.health) / f32::from(b.max_health.max(1)) * 100.0;
            let left = b.modules.iter().filter(|m| m.alive).count();
            let core = if b.core_exposed { " \u{b7} CORE OPEN" } else { "" };
            let name = match b.kind {
                arena_core::raid::BossKind::WarRig => "War Rig",
                arena_core::raid::BossKind::LandShip => "Land-Ship",
            };
            label.push_str(&format!(" \u{b7} {name} {hp:.0}% \u{b7} {left}/{} modules{core}", b.modules.len()));
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
        crate::ui::Quality::High => Quality::High,
        crate::ui::Quality::Ultra => Quality::Ultra,
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

/// The title's ticker, naming the picked map.
pub fn ticker(map: MapId) -> String {
    format!("Scrap Brawl on {}  //  three of a kind bolts on  //  shoot it off, take it home", map.name())
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
