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
    /// `None` marks a bot.
    pub ping_ms: Option<u32>,
    pub is_you: bool,
    pub dead: bool,
}

#[derive(Clone, Debug)]
pub struct EndRow {
    pub name: String,
    pub team: Team,
    pub kills: u32,
    pub deaths: u32,
    /// A ribbon under the name ("Top gun", "Biggest build").
    pub tag: Option<String>,
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
}

impl Panel {
    fn css(self) -> &'static str {
        match self {
            Panel::None => "none",
            Panel::Host => "host",
            Panel::Join => "join",
            Panel::Settings => "settings",
            Panel::Garage => "garage",
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
    fn parse(s: &str) -> Self {
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
}

// --------------------------------------------------------------------- ui

const FEED_MAX: usize = 5;
const FEED_LIFE: f32 = 7.0;
const CHAT_MAX: usize = 7;

#[derive(Default)]
struct Cache {
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

fn clock(secs: i32) -> String {
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
            if let UiAction::GarageChanged(g) = a {
                self.dom
                    .html("t-crew-emblem", &format!("<use href=\"#e-{}\"/>", esc(&g.emblem)));
            }
        }
        actions
    }

    /// Housekeeping: expire kill feed rows and the radio line. Call once a frame.
    pub fn tick(&mut self, dt: f32) {
        for age in self.feed_ages.iter_mut() {
            *age += dt;
        }
        while self.feed_ages.front().is_some_and(|a| *a > FEED_LIFE) {
            self.feed_ages.pop_front();
            self.dom.remove_first_child("t-feed");
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
                    if r.ping_ms.is_none() { "<em>BOT</em>" } else { "" },
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

    /// The end of match line-up. The 3D line-up renders behind the gap in
    /// the middle; `rows` are the cards under it, best first.
    pub fn show_endscreen(&mut self, title: &str, winner: Option<Team>, rows: &[EndRow], note: &str) {
        let mut h = String::new();
        for (i, r) in rows.iter().enumerate() {
            let _ = write!(
                h,
                "<div class=\"t-ec plate\" data-team=\"{}\"><div class=\"t-ec-rank\">#{}</div><div class=\"t-ec-name\">{}</div>\
                 <div class=\"t-ec-stat\"><span><b>{}</b> kills</span><span><b>{}</b> deaths</span></div>{}</div>",
                r.team.css(),
                i + 1,
                esc(&r.name),
                r.kills,
                r.deaths,
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

    pub fn hide_endscreen(&mut self) {
        self.dom.hidden("t-end", true);
        self.dom.attr("t-hud", "data-end", "0");
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
