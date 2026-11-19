//! HTML/CSS menus and HUD, driven from the game.
//!
//! The markup and styles live in `crates/arena-client/ui/` (`menus.html`,
//! `hud.html`, `tansk-ui.css`) and are compiled in with `include_str!`.
//! [`Ui::mount`] inserts them over the canvas; after that the game calls the
//! typed setters below (per frame or on events) and drains
//! [`Ui::poll_actions`] for what the player clicked or typed.
//!
//! Setters cache the last value and only touch the DOM when it changes, so
//! calling them every frame is fine. The HUD draws itself from CSS custom
//! properties and `data-*` attributes; Rust never builds per-frame markup.
//!
//! On native builds the DOM backend is a no-op stub with the same surface, so
//! game code can call the UI unconditionally.

// The API is wider than what the game wires up today.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::fmt::Write as _;

#[cfg(target_arch = "wasm32")]
mod dom_web;
#[cfg(target_arch = "wasm32")]
use dom_web::Dom;

#[cfg(not(target_arch = "wasm32"))]
mod dom_stub;
#[cfg(not(target_arch = "wasm32"))]
use dom_stub::Dom;

pub(crate) const CSS: &str = include_str!("../../ui/tansk-ui.css");
pub(crate) const MENUS_HTML: &str = include_str!("../../ui/menus.html");
pub(crate) const HUD_HTML: &str = include_str!("../../ui/hud.html");
/// The controls card (Settings > Controls and the pause menu).
pub(crate) const CONTROLS_HTML: &str = include_str!("../../ui/controls.html");

// ------------------------------------------------------------------ types

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    /// Oxide orange.
    Rustbacks,
    /// Bleached teal.
    DustSaints,
    /// Free-for-all crew (neutral cream in the HUD).
    Ffa,
}

impl Team {
    fn css(self) -> &'static str {
        match self {
            Team::Rustbacks => "a",
            Team::DustSaints => "b",
            Team::Ffa => "ffa",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Team::Rustbacks => "Rustbacks",
            Team::DustSaints => "Dust Saints",
            Team::Ffa => "Crews",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrapKind {
    Steel,
    Powder,
    Volts,
    Fuel,
}

impl ScrapKind {
    fn css(self) -> &'static str {
        match self {
            ScrapKind::Steel => "steel",
            ScrapKind::Powder => "powder",
            ScrapKind::Volts => "volts",
            ScrapKind::Fuel => "fuel",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackState {
    Ok,
    /// Blown off; `repair_in` seconds until the crew has it patched.
    Blown { repair_in: f32 },
    /// The crew is on it (hatch open, wrench out).
    Repairing { done_in: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    /// Bright clang: you hit a bolted-on part.
    Part,
    /// Dull thud: armour soaked it.
    Armour,
    /// Red stamp: that one's done.
    Kill,
}

#[derive(Clone, Debug)]
pub struct BranchOption {
    pub kind: ScrapKind,
    pub name: String,
    pub blurb: String,
}

#[derive(Clone, Debug)]
pub struct ScoreRow {
    pub name: String,
    pub team: Team,
    pub kills: u32,
    pub deaths: u32,
    /// Current tier per track: Steel, Powder, Volts, Fuel.
    pub tiers: [u8; 4],
    pub is_bot: bool,
    /// Round trip; `None` when unknown (bots, other crews).
    pub ping_ms: Option<u32>,
    pub is_you: bool,
    pub dead: bool,
}

#[derive(Clone, Debug)]
pub struct EndRow {
    /// Shown as #rank; tied rows share one.
    pub rank: u32,
    pub name: String,
    pub team: Team,
    pub kills: u32,
    pub deaths: u32,
    /// A ribbon under the name ("Top gun", "Biggest build").
    pub tag: Option<String>,
}

/// The Convoy bar under the clock.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvoyHud {
    /// Your side escorts the truck this round (else you stop it).
    pub escort: bool,
    /// Truck health and route progress, 0..1.
    pub health: f32,
    pub progress: f32,
    /// "Rolling", "Contested", "Repairing", "Setting up", "Burning"...
    pub state: String,
}

/// One derrick on the Wellhead HUD.
#[derive(Clone, Debug, PartialEq)]
pub struct WellHud {
    /// Who holds it.
    pub owner: Option<Team>,
    /// How full its ring is (0..1) and for whom.
    pub fill: f32,
    pub side: Option<Team>,
    pub contested: bool,
}

/// The Wellhead plate under the clock: both fuel tanks and the derricks.
#[derive(Clone, Debug, PartialEq)]
pub struct WellheadHud {
    pub wells: Vec<WellHud>,
    /// Rustbacks, Dust Saints.
    pub fuel: [u32; 2],
    pub target: u32,
    /// A line under the wells ("All three: double pumping").
    pub state: String,
    /// The line is bad news (flash it).
    pub alarm: bool,
}

/// The Last Crew plate under the clock.
#[derive(Clone, Debug, PartialEq)]
pub struct LastCrewHud {
    /// Where the safe ground is, relative to the view (0 = straight ahead, clockwise).
    pub arrow_deg: f32,
    /// "Inside", "142 m to safety"...
    pub dist: String,
    /// "Wall moves in 0:45"...
    pub line: String,
    pub crews: u8,
    /// safe | move | storm.
    pub state: &'static str,
}

#[derive(Clone, Debug)]
pub struct OpenGame {
    pub host: String,
    pub mode: String,
    pub map: String,
    pub players: u32,
    pub max_players: u32,
    pub ping_ms: u32,
    pub code: String,
}

/// The title-screen sub-panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    None,
    Host,
    Join,
    Settings,
    Garage,
    Controls,
    HowTo,
}

impl Panel {
    fn css(self) -> &'static str {
        match self {
            Panel::None => "none",
            Panel::Host => "host",
            Panel::Join => "join",
            Panel::Settings => "settings",
            Panel::Garage => "garage",
            Panel::Controls => "controls",
            Panel::HowTo => "howto",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
    Ultra,
}

impl Quality {
    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "low" => Quality::Low,
            "medium" => Quality::Medium,
            "ultra" => Quality::Ultra,
            _ => Quality::High,
        }
    }
    fn css(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::Medium => "medium",
            Quality::High => "high",
            Quality::Ultra => "ultra",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub name: String,
    /// Volumes in 0..=1.
    pub vol_master: f32,
    pub vol_sfx: f32,
    pub vol_music: f32,
    pub vol_radio: f32,
    /// Wind and storm.
    pub vol_ambience: f32,
    /// Multiplier, 0.1..=3.0.
    pub mouse_sens: f32,
    pub quality: Quality,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            name: "Scrapper".into(),
            vol_master: 0.8,
            vol_sfx: 0.9,
            vol_music: 0.5,
            vol_radio: 1.0,
            vol_ambience: 0.7,
            mouse_sens: 1.0,
            quality: Quality::High,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chassis {
    Scout,
    Brawler,
    Hauler,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Garage {
    pub chassis: Chassis,
    /// `#rrggbb`.
    pub paint: String,
    /// One of wrench, cog, skull, flame, star, drop.
    pub emblem: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostConfig {
    /// scrap_brawl | team_brawl (others are locked in the markup for now).
    pub mode: String,
    /// the_sink | refinery (`MapId::from_key`).
    pub map: String,
    pub bots: bool,
    pub private: bool,
    /// The code shown in the panel (set it with [`Ui::set_host_code`]).
    pub code: String,
}

/// The host's lobby settings as the lobby plate reads them.
#[derive(Clone, Debug, PartialEq)]
pub struct LobbyConfig {
    /// scrap_brawl | team_brawl | raider_waves | convoy | wellhead | last_crew.
    pub mode: String,
    /// sink | refinery | rustbelt.
    pub map: String,
    pub bots: bool,
    /// easy | normal | hard.
    pub skill: String,
    /// Match length in seconds; `None` keeps the current (a custom length).
    pub secs: Option<u16>,
    /// maps | mixed.
    pub vote: String,
}

/// One crew on the lobby plate.
#[derive(Clone, Debug, PartialEq)]
pub struct LobbyCrewRow {
    pub name: String,
    /// Paint, 0xrrggbb.
    pub colour: u32,
    /// Scout | Brawler | Hauler.
    pub chassis: &'static str,
    /// The side they picked (team modes; `Ffa` = auto).
    pub team: Option<Team>,
    pub host: bool,
    pub ready: bool,
    pub away: bool,
    pub you: bool,
}

/// One card of the map vote.
#[derive(Clone, Debug, PartialEq)]
pub struct VoteCardView {
    pub mode: String,
    pub map: String,
    /// `MapId::key`, for the card's colours.
    pub map_key: &'static str,
    pub votes: u8,
    /// The rematch card (this mode and map).
    pub again: bool,
}

/// Everything the lobby plate shows.
#[derive(Clone, Debug, PartialEq)]
pub struct LobbyView {
    pub code: String,
    /// The invite link (opens the game straight into the room).
    pub link: String,
    pub you_host: bool,
    pub host_name: String,
    pub phase: String,
    pub crew: Vec<LobbyCrewRow>,
    pub seats: u8,
    /// Team modes show the side pick.
    pub team_mode: bool,
    /// a | b | auto.
    pub my_team: &'static str,
    /// The host's settings: mode, map keys, bots, skill, length (s), vote pool.
    pub mode: &'static str,
    pub map: &'static str,
    pub bots: bool,
    pub skill: &'static str,
    pub secs: u16,
    pub vote_pool: &'static str,
    pub ready: bool,
    /// The host's Start is live, and its label.
    pub can_start: bool,
    pub start_label: String,
    /// The line by the buttons ("Waiting on 2 crews").
    pub status: String,
    /// The map vote: cards, seconds left, your pick.
    pub vote: Option<(Vec<VoteCardView>, u8, Option<u8>)>,
}

/// What the player asked for. Drained with [`Ui::poll_actions`].
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    Play,
    Practice,
    /// The title's map switch for Practice / Team / Raid (`sink`, `refinery`).
    PracticeMap(String),
    Host(HostConfig),
    Join { code: String },
    RefreshGames,
    /// Cancel on the loading / connecting card.
    Cancel,
    SettingsChanged(Settings),
    GarageChanged(Garage),
    /// The branch card picked, 0-based (keys 1/2/3 or a click).
    BranchPicked(u8),
    /// A chat line the player sent (Enter opens chat, Enter sends).
    Chat(String),
    /// End screen buttons.
    Again,
    Leave,
    /// Party lobby: the ready tick (toggles), the host's Start, the host's
    /// settings, your side, a vote card, the invite link copied.
    LobbyReady,
    LobbyStart,
    LobbySettings(LobbyConfig),
    LobbyTeam(String),
    LobbyVote(u8),
    InviteCopied,
    /// A button that only moves between title panels (for its click sound).
    Click,
    /// A press that can't go through (Join with no code): the error sound.
    Refused,
}

// --------------------------------------------------------------------- ui

const FEED_MAX: usize = 5;
const FEED_LIFE: f32 = 7.0;
const CHAT_MAX: usize = 7;

#[derive(Default)]
struct Cache {
    wells: Option<WellheadHud>,
    well_marks: [Option<(i32, i32, i32, u8, bool, i8)>; 3],
    last_crew: Option<LastCrewHud>,
    out: Option<(u8, String)>,
    convoy: Option<(bool, i32, i32, String)>,
    mark: Option<(i32, i32, i32, bool)>,
    health: Option<i32>,
    armour: Option<i32>,
    boost: Option<i32>,
    spread: Option<i32>,
    reload: Option<i32>,
    tracks: [Option<(u8, i32)>; 2],
    scrap: [Option<u8>; 4],
    tiers: [Option<u8>; 4],
    timer: Option<i32>,
    scores: Option<(u32, u32)>,
    ffa: Option<bool>,
    respawn: Option<i32>,
}

pub struct Ui {
    dom: Dom,
    cache: Cache,
    local_name: String,
    scores: (u32, u32),
    feed_ages: VecDeque<f32>,
    chat_lines: usize,
    radio_left: f32,
    dmg_slot: usize,
    board_title: (String, String),
    lobby: Option<LobbyView>,
    copied_left: f32,
}

/// Quantise a 0..1 value so tiny float jitter doesn't touch the DOM.
fn q(v: f32) -> i32 {
    (v.clamp(0.0, 1.0) * 1000.0).round() as i32
}

fn frac(v: i32) -> String {
    format!("{:.3}", v as f32 / 1000.0)
}

fn roman(tier: u8) -> &'static str {
    match tier {
        0 => "-",
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        _ => "V",
    }
}

/// The safe band for world markers, in rem (see `.t-wellmark` in the CSS):
/// a marker's foot sits at least this far from the top and the bottom.
pub const MARK_SAFE_TOP: f32 = 16.0;
pub const MARK_SAFE_BOTTOM: f32 = 18.5;
/// A marker's box, rem (icon over a "[A] 321 m" label), and one stack step.
const MARK_W: f32 = 6.0;
const MARK_H: f32 = 3.9;

/// Stack steps for screen markers (0..1 positions, `None` = hidden) so none
/// overlaps another: 0 stays, +n moves n rows down, -n up. Rows go toward the
/// middle of the safe band. Mirrors the CSS rem sizing (`html { font-size }`).
pub fn stack_marks(pts: &[Option<(f32, f32)>], screen: (u32, u32)) -> Vec<i8> {
    let (w, h) = (screen.0.max(1) as f32, screen.1.max(1) as f32);
    let rem = (0.0148 * h).min(0.0084 * w).clamp(12.0, 22.0);
    let (rx, ry) = (rem / w, rem / h);
    let (top, bottom) = (MARK_SAFE_TOP * ry, (1.0 - MARK_SAFE_BOTTOM * ry).max(MARK_SAFE_TOP * ry));
    let mut out = vec![0i8; pts.len()];
    // Lowest on screen first: it keeps its place, the others step away.
    let mut order: Vec<usize> = (0..pts.len()).filter(|&i| pts[i].is_some()).collect();
    order.sort_by(|&a, &b| pts[b].unwrap().1.total_cmp(&pts[a].unwrap().1));
    let mut placed: Vec<(f32, f32)> = Vec::new();
    for i in order {
        let (x, y) = pts[i].unwrap();
        let y = y.clamp(top, bottom);
        let dir: i8 = if y < (top + bottom) * 0.5 { 1 } else { -1 };
        let mut s = 0i8;
        while s.abs() < 4 && placed.iter().any(|&(px, py)| (px - x).abs() < MARK_W * rx && (py - (y + f32::from(s) * MARK_H * ry)).abs() < MARK_H * ry * 0.95) {
            s += dir;
        }
        placed.push((x, y + f32::from(s) * MARK_H * ry));
        out[i] = s;
    }
    out
}

/// Standard competition ranks for rows already sorted best first: equal
/// keys share a rank and the next rank skips (1, 1, 3).
pub fn shared_ranks<K: PartialEq>(keys: &[K]) -> Vec<u32> {
    let mut ranks = Vec::with_capacity(keys.len());
    for (i, k) in keys.iter().enumerate() {
        let r = if i > 0 && keys[i - 1] == *k { ranks[i - 1] } else { i as u32 + 1 };
        ranks.push(r);
    }
    ranks
}

/// 1st, 2nd, 3rd, 4th...
pub fn ordinal(n: u8) -> String {
    let s = match n % 10 {
        1 if n % 100 != 11 => "st",
        2 if n % 100 != 12 => "nd",
        3 if n % 100 != 13 => "rd",
        _ => "th",
    };
    format!("{n}{s}")
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

/// Escapes `text` and turns `[Key]` marks into `<kbd>Key</kbd>` caps.
fn keycaps(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(len) = rest[open..].find(']') else { break };
        out.push_str(&esc(&rest[..open]));
        let _ = write!(out, "<kbd>{}</kbd>", esc(&rest[open + 1..open + len]));
        rest = &rest[open + len + 1..];
    }
    out.push_str(&esc(rest));
    out
}

pub(crate) fn clock(secs: i32) -> String {
    let s = secs.max(0);
    format!("{:02}:{:02}", s / 60, s % 60)
}

impl Ui {
    /// Insert the UI over the page (wasm) and show the title screen.
    pub fn mount() -> Self {
        let mut ui = Ui {
            dom: Dom::mount(),
            cache: Cache::default(),
            local_name: Settings::default().name,
            scores: (0, 0),
            feed_ages: VecDeque::new(),
            chat_lines: 0,
            radio_left: 0.0,
            dmg_slot: 0,
            board_title: ("Scoreboard".into(), String::new()),
            lobby: None,
            copied_left: 0.0,
        };
        ui.show_title();
        ui
    }

    /// Everything the player did since the last call.
    pub fn poll_actions(&mut self) -> Vec<UiAction> {
        let actions = self.dom.drain();
        for a in &actions {
            if let UiAction::SettingsChanged(s) = a {
                self.local_name = s.name.clone();
                self.dom.text("t-crew-name", &s.name);
            }
            if let UiAction::InviteCopied = a {
                self.copied_left = 2.0;
                self.dom.attr("t-lobby", "data-copied", "1");
                self.dom.text("t-lobby-copy", "Link copied!");
            }
            if let UiAction::GarageChanged(g) = a {
                self.dom
                    .html("t-crew-emblem", &format!("<use href=\"#e-{}\"/>", esc(&g.emblem)));
            }
        }
        actions
    }

    /// Housekeeping: expire kill feed rows and the radio line. Call once a frame.
    pub fn tick(&mut self, dt: f32) {
        self.dom.flush();
        for age in self.feed_ages.iter_mut() {
            *age += dt;
        }
        while self.feed_ages.front().is_some_and(|a| *a > FEED_LIFE) {
            self.feed_ages.pop_front();
            self.dom.remove_first_child("t-feed");
        }
        if self.copied_left > 0.0 {
            self.copied_left -= dt;
            if self.copied_left <= 0.0 {
                self.dom.attr("t-lobby", "data-copied", "0");
                self.dom.text("t-lobby-copy", "Copy invite link");
            }
        }
        if self.radio_left > 0.0 {
            self.radio_left -= dt;
            if self.radio_left <= 0.0 {
                self.dom.attr("t-radio", "data-on", "0");
            }
        }
    }

    // ------------------------------------------------------------ screens

    pub fn show_title(&mut self) {
        self.dom.hidden("t-title", false);
        self.dom.hidden("t-loading", true);
        self.dom.hidden("t-hud", true);
        self.dom.set_hud_visible(false);
        self.show_panel(Panel::None);
    }

    pub fn show_panel(&mut self, panel: Panel) {
        self.dom.attr("t-title", "data-panel", panel.css());
    }

    /// The loading / connecting card. `progress` in 0..1, `None` keeps it as is.
    pub fn show_loading(&mut self, title: &str, sub: &str, progress: Option<f32>) {
        self.dom.hidden("t-title", true);
        self.dom.hidden("t-hud", true);
        self.dom.hidden("t-loading", false);
        self.dom.set_hud_visible(false);
        self.dom.text("t-loading-title", title);
        self.dom.text("t-loading-sub", sub);
        if let Some(p) = progress {
            self.dom.var("t-loading-bar", "--p", &frac(q(p)));
        }
    }

    pub fn set_loading_tip(&mut self, tip: &str) {
        self.dom.text("t-loading-tip", tip);
    }

    pub fn show_hud(&mut self) {
        self.dom.hidden("t-title", true);
        self.dom.hidden("t-loading", true);
        self.dom.hidden("t-hud", false);
        self.dom.set_hud_visible(true);
    }

    /// Hide everything (e.g. a photo mode).
    pub fn hide_all(&mut self) {
        for id in ["t-title", "t-loading", "t-hud"] {
            self.dom.hidden(id, true);
        }
        self.dom.set_hud_visible(false);
    }

    // ---------------------------------------------------------- menu data

    pub fn set_ticker(&mut self, text: &str) {
        self.dom.text("t-ticker-text", text);
    }

    pub fn set_host_code(&mut self, code: &str) {
        self.dom.text("t-host-code", code);
    }

    pub fn set_open_games(&mut self, games: &[OpenGame]) {
        let mut h = String::new();
        for g in games {
            let _ = write!(
                h,
                "<button class=\"t-game\" data-action=\"join-row\" data-code=\"{}\"><span>{}</span><span>{}</span><span>{}</span><span>{}/{}</span><span>{}</span></button>",
                esc(&g.code), esc(&g.host), esc(&g.mode), esc(&g.map), g.players, g.max_players, g.ping_ms
            );
        }
        self.dom.html("t-games-body", &h);
        let note = if games.is_empty() {
            "nobody on the band. Host one.".to_string()
        } else {
            format!("{} on the band", games.len())
        };
        self.dom.text("t-games-note", &note);
    }

    /// Put saved settings into the Settings panel.
    pub fn apply_settings(&mut self, s: &Settings) {
        self.local_name = s.name.clone();
        self.dom.text("t-crew-name", &s.name);
        self.dom.set_value("t-set-name", &s.name);
        let pct = |v: f32| ((v.clamp(0.0, 1.0) * 100.0).round() as i32).to_string();
        for (key, v) in [
            ("vol_master", pct(s.vol_master)),
            ("vol_sfx", pct(s.vol_sfx)),
            ("vol_music", pct(s.vol_music)),
            ("vol_radio", pct(s.vol_radio)),
            ("vol_ambience", pct(s.vol_ambience)),
        ] {
            self.dom.set_range(key, &v, &v);
        }
        let sens = (s.mouse_sens.clamp(0.1, 3.0) * 100.0).round() as i32;
        self.dom
            .set_range("mouse_sens", &sens.to_string(), &format!("{:.2}", sens as f32 / 100.0));
        self.dom.check(&format!("input[name=quality][value={}]", s.quality.css()));
    }

    // ------------------------------------------------------------- gauges

    /// Hull health, 0..1. Below 0.3 the dial face pulses; below 0.5 the
    /// soot vignette creeps in.
    pub fn set_health(&mut self, v: f32) {
        let v = q(v);
        if self.cache.health == Some(v) {
            return;
        }
        self.cache.health = Some(v);
        self.dom.var("t-hp", "--v", &frac(v));
        self.dom.text("t-hp-num", &((v as f32 / 10.0).round() as i32).to_string());
        self.dom.attr("t-hp", "data-low", if v < 300 { "1" } else { "0" });
        let low = ((500 - v) as f32 / 500.0).max(0.0);
        self.dom.var("t-vignette", "--low", &format!("{low:.3}"));
    }

    pub fn set_armour(&mut self, v: f32) {
        let v = q(v);
        if self.cache.armour.replace(v) != Some(v) {
            self.dom.var("t-armour", "--v", &frac(v));
        }
    }

    /// Boost fuel, 0..1 (the E..F needle).
    pub fn set_boost(&mut self, v: f32) {
        let v = q(v);
        if self.cache.boost.replace(v) != Some(v) {
            self.dom.var("t-boost", "--v", &frac(v));
        }
    }

    /// Left, right.
    pub fn set_tracks(&mut self, tracks: [TrackState; 2]) {
        for (i, (t, side)) in tracks.iter().zip(["l", "r"]).enumerate() {
            let (code, secs) = match *t {
                TrackState::Ok => (0u8, 0.0),
                TrackState::Blown { repair_in } => (1, repair_in),
                TrackState::Repairing { done_in } => (2, done_in),
            };
            let tenths = (secs.max(0.0) * 10.0).ceil() as i32;
            if self.cache.tracks[i] == Some((code, tenths)) {
                continue;
            }
            let prev_code = self.cache.tracks[i].map(|c| c.0);
            self.cache.tracks[i] = Some((code, tenths));
            let id = format!("t-track-{side}");
            if prev_code != Some(code) {
                self.dom.attr(&id, "data-state", ["ok", "blown", "repair"][code as usize]);
            }
            let label = if code == 0 { String::new() } else { format!("{:.1}s", tenths as f32 / 10.0) };
            self.dom.text(&format!("{id}-t"), &label);
        }
    }

    /// Scrap held toward the next tier (0..=3) and the current tier, per
    /// track: Steel, Powder, Volts, Fuel. A tier going up flashes its row.
    pub fn set_scrap(&mut self, counts: [u8; 4], tiers: [u8; 4]) {
        for i in 0..4 {
            let id = format!("t-scrap-{i}");
            let n = counts[i].min(3);
            if self.cache.scrap[i].replace(n) != Some(n) {
                self.dom.attr(&id, "data-n", &n.to_string());
            }
            let t = tiers[i];
            let prev = self.cache.tiers[i].replace(t);
            if prev != Some(t) {
                self.dom.attr(&id, "data-tier", &t.to_string());
                self.dom.text(&format!("t-tier-{i}"), roman(t));
                if prev.is_some_and(|p| t > p) {
                    self.dom.retrigger(&id, "data-flash", "1");
                }
            }
        }
    }

    /// The bolted-on part shown next to each track ("Big Bore", "Sensor mast").
    pub fn set_part_names(&mut self, names: [&str; 4]) {
        for (i, n) in names.iter().enumerate() {
            self.dom.text(&format!("t-part-{i}"), n);
        }
    }

    // ------------------------------------------------------------ gunsight

    /// `spread` 0..1 (bloom circle size), `reload_frac` 0..1 (1 = loaded).
    pub fn set_reticle(&mut self, spread: f32, reload_frac: f32) {
        let s = q(spread);
        if self.cache.spread.replace(s) != Some(s) {
            self.dom.var("t-reticle", "--spread", &frac(s));
        }
        let r = q(reload_frac);
        let prev = self.cache.reload.replace(r);
        if prev != Some(r) {
            self.dom.var("t-reticle", "--reload", &frac(r));
            let ready = r >= 1000;
            if prev.map(|p| p >= 1000) != Some(ready) {
                self.dom.attr("t-reticle", "data-ready", if ready { "1" } else { "0" });
            }
        }
    }

    pub fn hit_marker(&mut self, kind: HitKind) {
        let k = match kind {
            HitKind::Part => "part",
            HitKind::Armour => "armour",
            HitKind::Kill => "kill",
        };
        self.dom.retrigger("t-hit", "data-kind", k);
    }

    /// Where the hit came from, relative to the camera: 0 = ahead (top of the
    /// screen), positive clockwise, radians.
    pub fn damage_from(&mut self, angle_rad: f32) {
        let id = format!("t-dmg-{}", self.dmg_slot);
        self.dmg_slot = (self.dmg_slot + 1) % 4;
        self.dom.var(&id, "--a", &format!("{:.1}deg", angle_rad.to_degrees()));
        self.dom.retrigger(&id, "data-on", "1");
    }

    // -------------------------------------------------------------- feeds

    pub fn kill_feed(&mut self, killer: &str, killer_team: Team, victim: &str, victim_team: Team, weapon: &str) {
        let you = killer == self.local_name || victim == self.local_name;
        let h = format!(
            "<div class=\"t-kf\"{}><span class=\"t-name-{}\">{}</span><span class=\"t-kf-weapon\">{}</span><span class=\"t-name-{}\">{}</span></div>",
            if you { " data-you=\"1\"" } else { "" },
            killer_team.css(),
            esc(killer),
            esc(weapon),
            victim_team.css(),
            esc(victim)
        );
        self.dom.append_html("t-feed", &h);
        self.feed_ages.push_back(0.0);
        while self.feed_ages.len() > FEED_MAX {
            self.feed_ages.pop_front();
            self.dom.remove_first_child("t-feed");
        }
    }

    /// The caller, as a crackly subtitle. Stays up for a few seconds.
    pub fn radio(&mut self, text: &str) {
        self.dom.text("t-radio-text", text);
        self.dom.retrigger("t-radio", "data-on", "1");
        self.radio_left = 4.0 + text.len() as f32 * 0.05;
    }

    /// A crew-chief tip under the top plates; `[W]` marks become key caps.
    pub fn show_hint(&mut self, text: &str) {
        self.dom.html("t-hint-text", &keycaps(text));
        self.dom.retrigger("t-hint", "data-on", "1");
    }

    pub fn hide_hint(&mut self) {
        self.dom.attr("t-hint", "data-on", "0");
    }

    pub fn chat_line(&mut self, name: &str, team: Team, text: &str) {
        let h = format!(
            "<div class=\"t-chat-line\"><b class=\"t-name-{}\">{}:</b> {}</div>",
            team.css(),
            esc(name),
            esc(text)
        );
        self.push_chat(&h);
    }

    /// A system line in the chat (yellow, italic): pickups, joins, leaves.
    pub fn chat_system(&mut self, text: &str) {
        self.push_chat(&format!("<div class=\"t-chat-line\" data-sys=\"1\">{}</div>", esc(text)));
    }

    fn push_chat(&mut self, h: &str) {
        self.dom.append_html("t-chat-log", h);
        self.chat_lines += 1;
        while self.chat_lines > CHAT_MAX {
            self.chat_lines -= 1;
            self.dom.remove_first_child("t-chat-log");
        }
    }

    /// True while the chat input has the keyboard: don't drive the tank.
    pub fn chat_open(&self) -> bool {
        self.dom.chat_open()
    }

    // --------------------------------------------------------- match state

    /// Seconds left on the clock; under 30 s it blinks.
    pub fn set_timer(&mut self, secs_left: f32) {
        let s = secs_left.max(0.0).ceil() as i32;
        if self.cache.timer.replace(s) != Some(s) {
            self.dom.text("t-timer", &clock(s));
            self.dom.attr("t-clock", "data-low", if s <= 30 { "1" } else { "0" });
        }
    }

    /// Convoy: the truck's health and route bar with your role, or `None` to hide it.
    pub fn set_convoy(&mut self, hud: Option<&ConvoyHud>) {
        let key = hud.map(|h| (h.escort, q(h.health), q(h.progress), h.state.clone()));
        if self.cache.convoy == key && key.is_some() {
            return;
        }
        let was = self.cache.convoy.is_some();
        self.cache.convoy = key;
        let Some(h) = hud else {
            if was {
                self.dom.hidden("t-convoy", true);
                self.dom.hidden("t-convoy-mark", true);
            }
            return;
        };
        self.dom.hidden("t-convoy", false);
        self.dom.attr("t-convoy", "data-role", if h.escort { "escort" } else { "attack" });
        self.dom.text("t-convoy-role", if h.escort { "Escort" } else { "Stop the truck" });
        self.dom.var("t-convoy-hp", "--v", &format!("{:.3}", h.health.clamp(0.0, 1.0)));
        self.dom.var("t-convoy-route", "--v", &format!("{:.3}", h.progress.clamp(0.0, 1.0)));
        self.dom.text("t-convoy-state", &h.state);
        self.dom.attr("t-convoy", "data-state", &h.state.to_ascii_lowercase().replace(' ', "-"));
    }

    /// The truck's marker: screen position (0..1, top-left origin) and
    /// distance in metres, or `None` when it's behind you or gone.
    pub fn set_convoy_mark(&mut self, at: Option<(f32, f32, f32, bool)>) {
        let key = at.map(|(x, y, d, e)| (q(x), q(y), d as i32, e));
        if self.cache.mark == key {
            return;
        }
        self.cache.mark = key;
        match at {
            None => self.dom.hidden("t-convoy-mark", true),
            Some((x, y, d, escort)) => {
                self.dom.hidden("t-convoy-mark", false);
                self.dom.var("t-convoy-mark", "--x", &format!("{:.2}%", x * 100.0));
                self.dom.var("t-convoy-mark", "--y", &format!("{:.2}%", y * 100.0));
                self.dom.attr("t-convoy-mark", "data-role", if escort { "escort" } else { "attack" });
                self.dom.text("t-convoy-dist", &format!("{} m", d.round() as i32));
            }
        }
    }

    /// Under the clock, e.g. "Team Brawl · The Sink".
    pub fn set_mode_label(&mut self, text: &str) {
        self.dom.text("t-mode", text);
    }

    /// Rustbacks, Dust Saints.
    pub fn set_scores(&mut self, rustbacks: u32, dust_saints: u32) {
        self.scores = (rustbacks, dust_saints);
        if self.cache.scores.replace(self.scores) != Some(self.scores) {
            self.dom.text("t-score-a", &rustbacks.to_string());
            self.dom.text("t-score-b", &dust_saints.to_string());
        }
    }

    /// Free-for-all hides the team score plates.
    pub fn set_ffa(&mut self, ffa: bool) {
        if self.cache.ffa.replace(ffa) != Some(ffa) {
            self.dom.attr("t-top", "data-ffa", if ffa { "1" } else { "0" });
        }
    }

    /// Offer the tier-2 branch: 2-3 cards, keys 1/2/3, a countdown bar of
    /// `timeout_secs`. The game owns the timeout (and the default pick);
    /// a pick arrives as [`UiAction::BranchPicked`].
    pub fn offer_branch(&mut self, title: &str, options: &[BranchOption], default: usize, timeout_secs: f32) {
        let mut h = String::new();
        for (i, o) in options.iter().take(3).enumerate() {
            let _ = write!(
                h,
                "<button class=\"t-branch-card\" data-action=\"branch\" data-i=\"{i}\" style=\"--k:var(--{k})\">\
                 <span class=\"t-bc-top\"><span class=\"t-bc-key\">{n}</span><svg class=\"t-bc-ic\"><use href=\"#i-{k}\"/></svg></span>{d}\
                 <span class=\"t-bc-name\">{name}</span><span class=\"t-bc-blurb\">{blurb}</span></button>",
                k = o.kind.css(),
                n = i + 1,
                d = if i == default { "<span class=\"t-bc-def\">Default</span>" } else { "" },
                name = esc(&o.name),
                blurb = esc(&o.blurb),
            );
        }
        self.dom.text("t-branch-title", title);
        self.dom.html("t-branch-cards", &h);
        self.dom.remove_attr("t-branch", "data-picked");
        self.dom.var("t-branch", "--dur", &format!("{:.2}s", timeout_secs.max(0.1)));
        self.dom.retrigger("t-branch", "data-on", "1");
        self.dom.set_branch(options.len().min(3));
    }

    pub fn hide_branch(&mut self) {
        self.dom.attr("t-branch", "data-on", "0");
        self.dom.set_branch(0);
    }

    pub fn set_scoreboard_title(&mut self, title: &str, sub: &str) {
        self.board_title = (title.to_string(), sub.to_string());
    }

    /// The Tab board. Rows are grouped by team (Rustbacks left, Saints
    /// right) or listed in one column for free-for-all, in the order given.
    pub fn show_scoreboard(&mut self, visible: bool, rows: &[ScoreRow]) {
        self.dom.hidden("t-board", !visible);
        if !visible {
            return;
        }
        let ffa = rows.iter().all(|r| r.team == Team::Ffa);
        let mut h = String::new();
        let teams: &[(Team, u32)] = if ffa {
            &[(Team::Ffa, 0)]
        } else {
            &[(Team::Rustbacks, self.scores.0), (Team::DustSaints, self.scores.1)]
        };
        for &(team, score) in teams {
            let _ = write!(
                h,
                "<div class=\"t-bt\" data-team=\"{}\"><div class=\"t-bt-head\"><span>{}</span><b>{}</b></div>\
                 <div class=\"t-br t-br-h\"><span>Crew</span><span>K</span><span>D</span><span>Tiers</span><span>Ping</span></div>",
                team.css(),
                team.label(),
                if ffa { String::new() } else { score.to_string() }
            );
            for r in rows.iter().filter(|r| ffa || r.team == team) {
                let _ = write!(
                    h,
                    "<div class=\"t-br\"{}{}><span class=\"t-br-name\">{}{}</span><span>{}</span><span>{}</span><span class=\"t-br-tiers\">",
                    if r.is_you { " data-you=\"1\"" } else { "" },
                    if r.dead { " data-dead=\"1\"" } else { "" },
                    esc(&r.name),
                    if r.is_bot { "<em>BOT</em>" } else { "" },
                    r.kills,
                    r.deaths
                );
                for t in r.tiers {
                    let _ = write!(h, "<i>{t}</i>");
                }
                let ping = r.ping_ms.map_or("-".to_string(), |p| p.to_string());
                let _ = write!(h, "</span><span>{ping}</span></div>");
            }
            h.push_str("</div>");
        }
        self.dom.attr("t-board-body", "data-ffa", if ffa { "1" } else { "0" });
        self.dom.html("t-board-body", &h);
        let (title, sub) = self.board_title.clone();
        self.dom.text("t-board-title", &title);
        self.dom.text("t-board-sub", &sub);
    }

    /// The respawn overlay with a countdown; `None` hides it.
    /// `killed_by`: who and with what, shown under the stamp.
    pub fn show_respawn(&mut self, secs: Option<f32>, killed_by: Option<(&str, Team, &str)>) {
        if self.cache.out.take().is_some() {
            // Back from Last Crew's "out" card.
            self.dom.text("t-respawn-stamp", "Wrecked");
            self.dom.text("t-respawn-note", "You keep your first tier in every track.");
            self.dom.hidden("t-respawn-count", false);
            self.dom.hidden("t-out-count", true);
            self.cache.respawn = None;
            self.dom.hidden("t-respawn", true);
            self.dom.hidden("t-reticle", false);
        }
        let Some(secs) = secs else {
            if self.cache.respawn.take().is_some() {
                self.dom.hidden("t-respawn", true);
                self.dom.hidden("t-reticle", false);
            }
            return;
        };
        let s = secs.max(0.0).ceil() as i32;
        if self.cache.respawn.is_none() {
            let by = match killed_by {
                Some((name, team, weapon)) => format!(
                    "Opened up by <span class=\"t-name-{}\">{}</span>&rsquo;s {}",
                    team.css(),
                    esc(name),
                    esc(weapon)
                ),
                None => "The Sink took you".to_string(),
            };
            self.dom.html("t-respawn-by", &by);
            self.dom.hidden("t-respawn", false);
            self.dom.hidden("t-reticle", true);
        }
        if self.cache.respawn.replace(s) != Some(s) {
            self.dom.text("t-respawn-secs", &s.to_string());
        }
    }

    /// Last Crew Rolling: you're out for good. `place` is where you
    /// finished; `watching` names the crew the camera follows.
    pub fn show_out(&mut self, place: u8, killed_by: Option<(&str, Team, &str)>, watching: Option<&str>) {
        let key = (place, watching.unwrap_or("").to_string());
        if self.cache.out.as_ref() == Some(&key) {
            return;
        }
        let first = self.cache.out.is_none();
        self.cache.out = Some(key);
        if first {
            let by = match killed_by {
                Some((name, team, weapon)) => format!(
                    "Opened up by <span class=\"t-name-{}\">{}</span>&rsquo;s {}",
                    team.css(),
                    esc(name),
                    esc(weapon)
                ),
                None => "The storm took you".to_string(),
            };
            self.dom.html("t-respawn-by", &by);
            self.dom.text("t-respawn-stamp", "Out");
            self.dom.hidden("t-respawn-count", true);
            self.dom.hidden("t-out-count", false);
            self.dom.hidden("t-respawn", false);
            self.dom.hidden("t-reticle", true);
            self.cache.respawn = Some(-1);
        }
        self.dom.text("t-out-place", &ordinal(place));
        let note = match watching {
            Some(n) => format!("No respawns. Watching {n}."),
            None => "No respawns. Watching the last crews.".to_string(),
        };
        self.dom.text("t-respawn-note", &note);
    }

    /// Wellhead: the fuel tanks and the derricks under the clock, or `None` to hide it.
    pub fn set_wellhead(&mut self, hud: Option<&WellheadHud>) {
        if self.cache.wells.as_ref() == hud {
            return;
        }
        let Some(h) = hud else {
            self.cache.wells = None;
            self.dom.hidden("t-wells", true);
            for k in 0..3 {
                self.dom.hidden(&format!("t-wellmark-{k}"), true);
            }
            self.cache.well_marks = [None; 3];
            return;
        };
        if self.cache.wells.is_none() {
            self.dom.hidden("t-wells", false);
        }
        let team = |t: Option<Team>| match t {
            Some(Team::Rustbacks) => "a",
            Some(Team::DustSaints) => "b",
            _ => "none",
        };
        for (k, w) in h.wells.iter().take(3).enumerate() {
            let id = format!("t-well-{k}");
            self.dom.attr(&id, "data-owner", team(w.owner));
            self.dom.attr(&id, "data-side", team(w.side));
            self.dom.attr(&id, "data-contested", if w.contested { "1" } else { "0" });
            self.dom.var(&id, "--p", &format!("{:.3}", w.fill.clamp(0.0, 1.0)));
        }
        let t = h.target.max(1) as f32;
        self.dom.var("t-fuel-a", "--v", &format!("{:.3}", (h.fuel[0] as f32 / t).min(1.0)));
        self.dom.var("t-fuel-b", "--v", &format!("{:.3}", (h.fuel[1] as f32 / t).min(1.0)));
        self.dom.text("t-fuel-a-n", &h.fuel[0].to_string());
        self.dom.text("t-fuel-b-n", &h.fuel[1].to_string());
        self.dom.text("t-wells-state", &h.state);
        self.dom.attr("t-wells-state", "data-x", if h.alarm { "1" } else { "0" });
        self.cache.wells = Some(h.clone());
    }

    /// Wellhead: each derrick's marker (screen x, y in 0..1, distance m, owner,
    /// contested), `None` when it's behind you.
    /// `screen`: the canvas size in pixels, to tell when markers overlap
    /// (overlapping ones stack into rows).
    pub fn set_well_marks(&mut self, marks: &[Option<(f32, f32, f32, Option<Team>, bool)>], screen: (u32, u32)) {
        let owner = |t: Option<Team>| match t {
            Some(Team::Rustbacks) => 1u8,
            Some(Team::DustSaints) => 2,
            _ => 0,
        };
        let pts: Vec<Option<(f32, f32)>> = marks.iter().map(|m| m.map(|(x, y, ..)| (x, y))).collect();
        let stacks = stack_marks(&pts, screen);
        for k in 0..3 {
            let m = marks.get(k).copied().flatten();
            let s = stacks.get(k).copied().unwrap_or(0);
            let key = m.map(|(x, y, d, o, c)| (q(x), q(y), d as i32, owner(o), c, s));
            if self.cache.well_marks[k] == key {
                continue;
            }
            let id = format!("t-wellmark-{k}");
            match m {
                None => self.dom.hidden(&id, true),
                Some((x, y, d, o, c)) => {
                    if self.cache.well_marks[k].is_none() {
                        self.dom.hidden(&id, false);
                    }
                    self.dom.var(&id, "--x", &format!("{:.2}%", x * 100.0));
                    self.dom.var(&id, "--y", &format!("{:.2}%", y * 100.0));
                    self.dom.var(&id, "--stack", &s.to_string());
                    self.dom.attr(&id, "data-owner", ["none", "a", "b"][owner(o) as usize]);
                    self.dom.attr(&id, "data-contested", if c { "1" } else { "0" });
                    self.dom.text(&format!("{id}-d"), &format!("{} m", d.round() as i32));
                }
            }
            self.cache.well_marks[k] = key;
        }
    }

    /// Last Crew Rolling: the compass, the wall's clock and the crews left, or `None` to hide it.
    pub fn set_last_crew(&mut self, hud: Option<&LastCrewHud>) {
        if self.cache.last_crew.as_ref() == hud {
            return;
        }
        let Some(h) = hud else {
            self.cache.last_crew = None;
            self.dom.hidden("t-lastcrew", true);
            return;
        };
        let was = self.cache.last_crew.clone();
        if was.is_none() {
            self.dom.hidden("t-lastcrew", false);
        }
        if was.as_ref().is_none_or(|w| w.arrow_deg != h.arrow_deg) {
            self.dom.var("t-lc-compass", "--a", &format!("{:.0}deg", h.arrow_deg));
        }
        if was.as_ref().is_none_or(|w| w.dist != h.dist) {
            self.dom.text("t-lc-dist", &h.dist);
        }
        if was.as_ref().is_none_or(|w| w.line != h.line) {
            self.dom.text("t-lc-state", &h.line);
        }
        if was.as_ref().is_none_or(|w| w.crews != h.crews) {
            self.dom.text("t-lc-crews", &h.crews.to_string());
        }
        if was.as_ref().is_none_or(|w| w.state != h.state) {
            self.dom.attr("t-lastcrew", "data-state", h.state);
        }
        self.cache.last_crew = Some(h.clone());
    }

    /// The line over the end title: "Match over · <map>".
    pub fn set_end_kicker(&mut self, text: &str) {
        self.dom.text("t-end-kicker", text);
    }

    /// The end of match line-up. The 3D line-up renders behind the gap in
    /// the middle; `rows` are the cards under it, best first.
    pub fn show_endscreen(&mut self, title: &str, winner: Option<Team>, rows: &[EndRow], note: &str) {
        let mut h = String::new();
        for r in rows {
            let _ = write!(
                h,
                "<div class=\"t-ec plate\" data-team=\"{}\"><div class=\"t-ec-rank\">#{}</div><div class=\"t-ec-name\">{}</div>\
                 <div class=\"t-ec-stat\"><span><b>{}</b> {}</span><span><b>{}</b> {}</span></div>{}</div>",
                r.team.css(),
                r.rank,
                esc(&r.name),
                r.kills,
                if r.kills == 1 { "kill" } else { "kills" },
                r.deaths,
                if r.deaths == 1 { "death" } else { "deaths" },
                r.tag.as_deref().map_or(String::new(), |t| format!("<span class=\"t-ec-tag\">{}</span>", esc(t)))
            );
        }
        self.dom.text("t-end-title", title);
        self.dom.attr("t-end-title", "data-team", winner.map_or("ffa", Team::css));
        self.dom.html("t-end-rows", &h);
        self.dom.text("t-end-note", note);
        self.dom.hidden("t-end", false);
        self.dom.attr("t-hud", "data-end", "1");
    }

    /// The end screen's go button ("Again", "Back to the yard").
    pub fn set_end_again(&mut self, text: &str) {
        self.dom.text("t-end-again", text);
    }

    pub fn hide_endscreen(&mut self) {
        self.dom.hidden("t-end", true);
        self.dom.attr("t-hud", "data-end", "0");
    }

    // ------------------------------------------------------------- lobby

    /// The party lobby over the warm-up (None hides it). Only what changed
    /// touches the DOM, and the host's settings controls are only set when
    /// the settings themselves change (so a click isn't undone by a stale echo).
    pub fn show_lobby(&mut self, view: Option<&LobbyView>) {
        let Some(v) = view else {
            if self.lobby.take().is_some() {
                self.dom.hidden("t-lobby", true);
                self.dom.attr("t-hud", "data-lobby", "0");
            }
            return;
        };
        let old = self.lobby.take();
        if old.is_none() {
            self.dom.hidden("t-lobby", false);
            self.dom.attr("t-hud", "data-lobby", "1");
        }
        let o = old.as_ref();
        if o.map(|o| (&o.code, &o.link)) != Some((&v.code, &v.link)) {
            self.dom.text("t-lobby-code", &v.code);
            self.dom.text("t-lobby-link", &v.link);
        }
        if o.map(|o| &o.phase) != Some(&v.phase) {
            self.dom.text("t-lobby-phase", &v.phase);
        }
        if o.map(|o| (o.you_host, &o.host_name)) != Some((v.you_host, &v.host_name)) {
            self.dom.attr("t-lobby", "data-host", if v.you_host { "1" } else { "0" });
            let note = if v.you_host { "Your call".to_string() } else { format!("{} calls it", v.host_name) };
            self.dom.text("t-lobby-hostnote", &note);
        }
        if o.map(|o| (&o.crew, o.seats)) != Some((&v.crew, v.seats)) {
            let mut h = String::new();
            for c in &v.crew {
                let (tag_k, tag) = match (c.host, c.team) {
                    (true, _) => ("host", "Host"),
                    (_, Some(Team::Rustbacks)) => ("a", "Rustbacks"),
                    (_, Some(Team::DustSaints)) => ("b", "Saints"),
                    _ => ("none", "-"),
                };
                let side = match c.team {
                    Some(Team::Rustbacks) if c.host => "<small>Rustbacks</small>",
                    Some(Team::DustSaints) if c.host => "<small>Saints</small>",
                    _ => "",
                };
                let _ = write!(
                    h,
                    "<div class=\"t-crew\" data-you=\"{}\" data-away=\"{}\"><i class=\"t-crew-paint\" style=\"--c:#{:06x}\"></i>\
                     <span class=\"t-crew-name\">{}{}{}</span><span class=\"t-crew-chassis\">{}</span>\
                     <span class=\"t-crew-tag\" data-k=\"{tag_k}\">{tag}</span><span class=\"t-crew-ready\" data-on=\"{}\">&#10003;</span></div>",
                    u8::from(c.you),
                    u8::from(c.away),
                    c.colour & 0xff_ffff,
                    esc(&c.name),
                    if c.away { "<small>away</small>" } else if c.you { "<small>you</small>" } else { "" },
                    side,
                    c.chassis,
                    u8::from(c.ready || c.host),
                );
            }
            let free = (v.seats as usize).saturating_sub(v.crew.len());
            if free > 0 {
                let _ = write!(
                    h,
                    "<div class=\"t-crew-empty\">{free} seat{} open{}</div>",
                    if free == 1 { "" } else { "s" },
                    if v.bots { ": bots fill them at the start" } else { "" }
                );
            }
            self.dom.html("t-lobby-crew", &h);
            self.dom.text("t-lobby-count", &format!("{} / {}", v.crew.len(), v.seats));
        }
        if o.map(|o| (o.team_mode, o.my_team)) != Some((v.team_mode, v.my_team)) {
            self.dom.attr("t-lobby", "data-team", if v.team_mode { "1" } else { "0" });
            self.dom.check(&format!("input[name=lteam][value={}]", v.my_team));
        }
        let settings = |l: &LobbyView| (l.mode, l.map, l.bots, l.skill, l.secs, l.vote_pool);
        if o.map(settings) != Some(settings(v)) {
            self.dom.check(&format!("input[name=lmode][value={}]", v.mode));
            self.dom.check(&format!("input[name=lmap][value={}]", v.map));
            self.dom.set_checked("#t-lobby-bots", v.bots);
            self.dom.check(&format!("input[name=lskill][value={}]", v.skill));
            self.dom.check(&format!("input[name=lvote][value={}]", v.vote_pool));
            let custom = ![180, 300, 420, 600].contains(&v.secs);
            self.dom.hidden("t-lobby-len-custom", !custom);
            if custom {
                self.dom.text("t-lobby-len-custom-txt", &clock(i32::from(v.secs)).trim_start_matches('0').to_string());
                self.dom.check("#t-lobby-len-custom-in");
            } else {
                self.dom.check(&format!("input[name=llen][value=\"{}\"]", v.secs));
            }
        }
        if o.map(|o| o.ready) != Some(v.ready) {
            self.dom.attr("t-lobby", "data-ready", if v.ready { "1" } else { "0" });
            self.dom.text("t-lobby-ready", if v.ready { "Ready" } else { "Ready?" });
        }
        if o.map(|o| (o.can_start, &o.start_label, &o.status)) != Some((v.can_start, &v.start_label, &v.status)) {
            self.dom.attr("t-lobby-start", "data-on", if v.can_start { "1" } else { "0" });
            self.dom.text("t-lobby-start-label", &v.start_label);
            self.dom.text("t-lobby-status", &v.status);
        }
        if o.map(|o| &o.vote) != Some(&v.vote) {
            match &v.vote {
                None => self.dom.hidden("t-vote", true),
                Some((cards, secs, mine)) => {
                    let old_cards = o.and_then(|o| o.vote.as_ref()).map(|(c, _, m)| (c, m));
                    if old_cards != Some((cards, mine)) {
                        let top = cards.iter().map(|c| c.votes).max().unwrap_or(0);
                        let mut h = String::new();
                        for (i, c) in cards.iter().enumerate() {
                            let _ = write!(
                                h,
                                "<button class=\"t-vote-card\" data-action=\"vote\" data-i=\"{i}\" data-map=\"{}\" data-mine=\"{}\" data-lead=\"{}\">\
                                 <span class=\"t-vote-again\">{}</span><span class=\"t-vote-mode\">{}</span><span class=\"t-vote-map\">{}</span>\
                                 <span class=\"t-vote-pips\">{}</span></button>",
                                c.map_key,
                                u8::from(*mine == Some(i as u8)),
                                u8::from(top > 0 && c.votes == top),
                                if c.again { "Same again" } else { "" },
                                esc(&c.mode),
                                esc(&c.map),
                                "<i></i>".repeat(c.votes as usize),
                            );
                        }
                        self.dom.html("t-vote-cards", &h);
                    }
                    self.dom.text("t-vote-secs", &secs.to_string());
                    self.dom.hidden("t-vote", false);
                }
            }
        }
        self.lobby = Some(v.clone());
    }

    /// Driving about in the yard (the plate steps back).
    pub fn set_lobby_drive(&mut self, on: bool) {
        self.dom.attr("t-lobby", "data-drive", if on { "1" } else { "0" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(clock(402), "06:42");
        assert_eq!(roman(2), "II");
        assert_eq!(esc("<a&\"b\">"), "&lt;a&amp;&quot;b&quot;&gt;");
        assert_eq!(q(1.7), 1000);
        assert_eq!(shared_ranks(&[(2, 0), (1, 0), (1, 0), (0, 1)]), vec![1, 2, 2, 4]);
        assert_eq!(shared_ranks(&[(1, 0), (1, 0)]), vec![1, 1]);
        assert!(shared_ranks::<u8>(&[]).is_empty());
        // Two derricks on top of each other: one steps a row; a far one stays.
        let s = stack_marks(&[Some((0.5, 0.5)), Some((0.51, 0.5)), Some((0.1, 0.5)), None], (1280, 720));
        assert_eq!(s[2], 0);
        assert_eq!(s[0].abs() + s[1].abs(), 1);
        // Near the top of the band they stack downward.
        let s = stack_marks(&[Some((0.5, 0.0)), Some((0.5, 0.01))], (1280, 720));
        assert!(s.iter().all(|&k| k >= 0) && s.contains(&1));
    }

    #[test]
    fn stub_accepts_everything() {
        let mut ui = Ui::mount();
        ui.show_hud();
        ui.set_health(0.4);
        ui.set_tracks([TrackState::Ok, TrackState::Blown { repair_in: 3.2 }]);
        ui.set_scrap([1, 3, 0, 2], [1, 2, 1, 1]);
        ui.offer_branch("Powder tier 2", &[], 0, 4.0);
        ui.show_scoreboard(true, &[]);
        ui.tick(10.0);
        assert!(ui.poll_actions().is_empty());
    }
}

#[cfg(test)]
mod mode_tests {
    use super::*;

    #[test]
    fn ordinals_read_right() {
        let got: Vec<String> = [1u8, 2, 3, 4, 8, 11, 12, 13, 21, 22].iter().map(|n| ordinal(*n)).collect();
        assert_eq!(got, ["1st", "2nd", "3rd", "4th", "8th", "11th", "12th", "13th", "21st", "22nd"]);
    }

    /// The HUD setters for the new modes run (native: a no-op DOM) and cache.
    #[test]
    fn wellhead_and_last_crew_huds_take_their_state() {
        let mut ui = Ui::mount();
        let hud = WellheadHud {
            wells: vec![
                WellHud { owner: Some(Team::Rustbacks), fill: 1.0, side: Some(Team::Rustbacks), contested: false },
                WellHud { owner: None, fill: 0.4, side: Some(Team::DustSaints), contested: true },
                WellHud { owner: Some(Team::DustSaints), fill: 1.0, side: Some(Team::DustSaints), contested: false },
            ],
            fuel: [120, 98],
            target: 300,
            state: "Fight at the outpost".into(),
            alarm: false,
        };
        ui.set_wellhead(Some(&hud));
        assert_eq!(ui.cache.wells.as_ref(), Some(&hud));
        ui.set_well_marks(&[Some((0.5, 0.4, 80.0, Some(Team::Rustbacks), false)), None, Some((0.9, 0.3, 200.0, None, true))], (1280, 720));
        assert!(ui.cache.well_marks[0].is_some() && ui.cache.well_marks[1].is_none());
        ui.set_wellhead(None);
        assert!(ui.cache.wells.is_none() && ui.cache.well_marks.iter().all(|m| m.is_none()));
        let lc = LastCrewHud { arrow_deg: 45.0, dist: "120 m to safety".into(), line: "Wall moves in 0:30".into(), crews: 5, state: "move" };
        ui.set_last_crew(Some(&lc));
        assert_eq!(ui.cache.last_crew.as_ref(), Some(&lc));
        ui.show_out(6, None, Some("Rusty"));
        assert_eq!(ui.cache.out, Some((6, "Rusty".to_string())));
        ui.show_respawn(None, None);
        assert!(ui.cache.out.is_none());
    }
}
