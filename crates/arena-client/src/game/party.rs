//! The "play with friends" flow on the client: hosting a party room, the
//! lobby plate over the warm-up (crews, ready ticks, the host's settings,
//! team picks, Start), the map vote after each match, and your Garage pick
//! sent to the room.

use arena_core::garage::{ChassisClass, Loadout};
use arena_core::map::MapId;
use arena_core::net::{BotSkill, Crew, Join, LobbyCmd, LobbyPhase, LobbyState, RoomSettings, TeamPick, VotePool};
use arena_core::raid::ModeKind;

use super::{Game, Match, Screen};
use crate::ui::{LobbyConfig, LobbyCrewRow, LobbyView, Team, VoteCardView};
use crate::world::{RenderWorld, PAINT_SET};

/// A look yaw near the chase cam's whose boom has open ground behind it
/// (a lobby spawn can sit with its back against a dune).
pub fn clear_yaw(map: &arena_core::map::Map, chase: &crate::camera::ChaseCam, pos: glam::Vec3) -> f32 {
    use crate::camera::CameraCollider;
    let col = crate::demo::MapCollider(map);
    let pivot = pos + glam::Vec3::Y * chase.pivot_height;
    let mut best = (f32::MIN, chase.yaw);
    for k in 0..12 {
        // 0, +30, -30, +60, ... degrees from the hull's heading.
        let step = (k as f32 / 2.0).ceil() * if k % 2 == 0 { -1.0 } else { 1.0 };
        let yaw = chase.yaw + step * std::f32::consts::TAU / 12.0;
        let fwd = glam::Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        let eye = pivot - fwd * chase.distance + glam::Vec3::Y * 1.5;
        let free = col.free_fraction(pivot, eye);
        if free > 0.95 {
            return yaw;
        }
        if free > best.0 {
            best = (free, yaw);
        }
    }
    best.1
}

/// The mode a world runs, read off its snapshot (the warm-up reads as
/// Scrap Brawl, its rules).
pub fn snapshot_mode(w: &arena_core::snapshot::Snapshot) -> ModeKind {
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
}

/// The lobby's mode keys (the plate's radio values).
pub fn mode_key(m: ModeKind) -> &'static str {
    match m {
        ModeKind::ScrapBrawl => "scrap_brawl",
        ModeKind::TeamBrawl => "team_brawl",
        ModeKind::RaiderWaves => "raider_waves",
        ModeKind::Convoy => "convoy",
        ModeKind::Wellhead => "wellhead",
        ModeKind::LastCrew => "last_crew",
    }
}

pub fn mode_of(key: &str) -> Option<ModeKind> {
    Some(match key {
        "scrap_brawl" => ModeKind::ScrapBrawl,
        "team_brawl" => ModeKind::TeamBrawl,
        "raider_waves" => ModeKind::RaiderWaves,
        "convoy" => ModeKind::Convoy,
        "wellhead" => ModeKind::Wellhead,
        "last_crew" => ModeKind::LastCrew,
        _ => return None,
    })
}

pub fn team_pick(key: &str) -> TeamPick {
    match key {
        "a" => TeamPick::Rustbacks,
        "b" => TeamPick::DustSaints,
        _ => TeamPick::Auto,
    }
}

fn chassis_name(c: ChassisClass) -> &'static str {
    match c {
        ChassisClass::Scout => "Scout",
        ChassisClass::Brawler => "Brawler",
        ChassisClass::Hauler => "Hauler",
    }
}

/// The host's settings with the plate's changes on top (`None`: nothing new).
pub fn apply_config(current: RoomSettings, cfg: &LobbyConfig) -> RoomSettings {
    RoomSettings {
        mode: mode_of(&cfg.mode).unwrap_or(current.mode),
        map: MapId::from_key(&cfg.map).unwrap_or(current.map),
        bots: cfg.bots,
        skill: match cfg.skill.as_str() {
            "easy" => BotSkill::Easy,
            "hard" => BotSkill::Hard,
            "normal" => BotSkill::Normal,
            _ => current.skill,
        },
        match_secs: cfg.secs.unwrap_or(current.match_secs),
        vote: match cfg.vote.as_str() {
            "mixed" => VotePool::Mixed,
            "maps" => VotePool::Maps,
            _ => current.vote,
        },
    }
    .clamped()
}

/// Everything the lobby plate shows, from the room's state.
pub fn lobby_view(l: &LobbyState, you: Option<arena_core::PlayerId>, code: &str, link: &str, my_paint: Option<u32>) -> LobbyView {
    let me = l.crew.iter().find(|c| Some(c.id) == you);
    let you_host = me.is_some_and(|c| c.host);
    let host_name = l.host().map(|c| c.name.clone()).unwrap_or_default();
    let team_mode = l.settings.mode.is_team();
    let crew: Vec<LobbyCrewRow> = l
        .crew
        .iter()
        .map(|c| {
            let mine = Some(c.id) == you;
            LobbyCrewRow {
                name: c.name.clone(),
                colour: if mine { my_paint } else { None }.unwrap_or(PAINT_SET[c.loadout.paint as usize % PAINT_SET.len()]),
                chassis: chassis_name(c.loadout.chassis),
                team: match (team_mode, c.team) {
                    (true, TeamPick::Rustbacks) => Some(Team::Rustbacks),
                    (true, TeamPick::DustSaints) => Some(Team::DustSaints),
                    _ => None,
                },
                host: c.host,
                ready: c.ready,
                away: c.away,
                you: mine,
            }
        })
        .collect();
    let seats = if l.settings.mode == ModeKind::RaiderWaves { 4.max(l.crew.len()) } else { arena_core::net::ROOM_SEATS } as u8;
    let waiting = l.crew.iter().filter(|c| !c.away && !c.ready && !c.host).count();
    let start_label = if l.vote.is_some() {
        "Vote first".to_string()
    } else if l.all_ready() {
        "Start".to_string()
    } else if l.force_in == 0 {
        "Start anyway".to_string()
    } else {
        format!("Start ({})", l.force_in)
    };
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let status = match (&l.vote, you_host, me.is_some_and(|c| c.ready)) {
        (Some(v), _, _) => format!("Voting: {} s left", v.secs_left),
        (None, true, _) if waiting == 0 => "Everyone's set. Roll out!".to_string(),
        (None, true, _) if l.force_in > 0 => format!("Waiting on {waiting} crew{}", plural(waiting)),
        (None, true, _) => format!("{waiting} crew{} not ready", plural(waiting)),
        (None, false, true) => format!("Ready. {host_name} starts it."),
        (None, false, false) => "Tick Ready when you're set".to_string(),
    };
    let vote = l.vote.as_ref().map(|v| {
        let cards = v
            .cards
            .iter()
            .enumerate()
            .map(|(i, c)| VoteCardView {
                mode: c.mode.name().to_string(),
                map: c.map.name().to_string(),
                map_key: c.map.key(),
                votes: l.crew.iter().filter(|k| k.vote == Some(i as u8)).count() as u8,
                again: i == 0,
            })
            .collect();
        (cards, v.secs_left, me.and_then(|c| c.vote))
    });
    LobbyView {
        code: code.to_string(),
        link: link.to_string(),
        you_host,
        host_name,
        phase: if l.vote.is_some() { "Vote" } else { "Warming up" }.to_string(),
        crew,
        seats,
        team_mode,
        my_team: match me.map(|c| c.team) {
            Some(TeamPick::Rustbacks) => "a",
            Some(TeamPick::DustSaints) => "b",
            _ => "auto",
        },
        mode: mode_key(l.settings.mode),
        map: l.settings.map.key(),
        bots: l.settings.bots,
        skill: match l.settings.skill {
            BotSkill::Easy => "easy",
            BotSkill::Normal => "normal",
            BotSkill::Hard => "hard",
        },
        secs: l.settings.match_secs,
        vote_pool: match l.settings.vote {
            VotePool::Maps => "maps",
            VotePool::Mixed => "mixed",
        },
        ready: me.is_some_and(|c| c.ready),
        can_start: l.can_start(),
        start_label,
        status,
        vote,
    }
}

impl Game {
    /// Host a party room: the crews meet in the lobby before every match.
    /// `?matchtime=` sets its length (testing).
    pub fn host_party(&mut self, public: bool, mode: ModeKind, map: MapId, bots: bool) {
        let mut settings = RoomSettings { mode, map, bots, ..RoomSettings::default() };
        if let Some(t) = self.debug.match_time {
            settings.match_secs = t.round().clamp(0.0, f32::from(u16::MAX)) as u16;
        }
        self.start_online(Join::Party { public, settings: settings.clamped() });
    }

    pub(super) fn lobby(&self) -> Option<&LobbyState> {
        self.net.as_ref()?.state().lobby.as_ref()
    }

    pub(super) fn my_crew(&self) -> Option<Crew> {
        let you = self.you()?;
        self.lobby()?.crew.iter().find(|c| c.id == you).cloned()
    }

    pub(super) fn lobby_cmd(&mut self, cmd: LobbyCmd) {
        if let Some(n) = &mut self.net {
            n.lobby(cmd);
        }
    }

    /// The plate's settings, as the host's next `Settings` move.
    pub(super) fn lobby_settings(&self, cfg: &LobbyConfig) -> Option<RoomSettings> {
        let l = self.lobby()?;
        let s = apply_config(l.settings, cfg);
        (s != l.settings).then_some(s)
    }

    /// Your Garage pick for the room (online). Without one, the paint your
    /// tank would have anyway.
    pub(super) fn send_loadout(&mut self) {
        let Some(you) = self.you() else { return };
        if self.net.as_mut().and_then(|n| n.practice_sim()).is_some() {
            return;
        }
        let loadout = match &self.garage {
            Some(g) => Loadout {
                chassis: match g.chassis {
                    crate::ui::Chassis::Scout => ChassisClass::Scout,
                    crate::ui::Chassis::Brawler => ChassisClass::Brawler,
                    crate::ui::Chassis::Hauler => ChassisClass::Hauler,
                },
                paint: super::paint_index(&g.paint),
                emblem: ["wrench", "cog", "skull", "flame", "star", "drop"].iter().position(|e| *e == g.emblem).unwrap_or(0) as u8,
                horn: 0,
            },
            None => match self.my_crew() {
                // Already told this room: keep it.
                Some(c) if c.loadout != Loadout::default() => return,
                _ => Loadout { paint: you % PAINT_SET.len() as u8, ..Loadout::default() },
            },
        };
        self.lobby_cmd(LobbyCmd::Loadout(loadout));
    }

    /// A party room rebuilt its world: per-match state starts over (the chat
    /// log keeps its place).
    pub(super) fn party_new_world(&mut self) {
        let (seen, last) = (self.m.chat_seen, self.m.chat_last.take());
        self.m = Match { chat_seen: seen, chat_last: last, ..Match::default() };
        self.close_podium();
        self.ui.hide_endscreen();
        self.ui.hide_branch();
        self.ui.show_respawn(None, None);
        self.ui.show_scoreboard(false, &[]);
        self.sound.clear_engines();
        // The new spawn: the boom starts out, not pulled in from the old spot.
        self.chase.reset_boom();
    }

    /// The HUD's mode label follows the room every frame: the yard while
    /// warming up, else the mode the world actually runs (read off the
    /// snapshot, so a Welcome that beats the phase change can't leave it stale).
    fn party_label(&mut self, l: &LobbyState) {
        let Some(net) = &self.net else { return };
        let st = net.state();
        let code = st.code.clone().unwrap_or_default();
        let map = st.map.name();
        let label = match (l.phase, &st.world) {
            (LobbyPhase::Lobby, _) => format!("Crew yard \u{b7} {map} \u{b7} {code}"),
            (_, Some(w)) => format!("{} \u{b7} {map} \u{b7} {code}", snapshot_mode(w).name()),
            (_, None) => format!("{} \u{b7} {map} \u{b7} {code}", l.settings.mode.name()),
        };
        if label != self.party_label {
            self.ui.set_mode_label(&label);
            self.ui.set_scoreboard_title("Scoreboard", &label);
            self.party_label = label;
        }
    }

    /// Follows the room's lobby phase (screens) and keeps the plate current.
    pub(super) fn party_frame(&mut self) {
        let Some(l) = self.lobby().cloned() else {
            self.ui.show_lobby(None);
            return;
        };
        match l.phase {
            LobbyPhase::Lobby if self.screen != Screen::Lobby => {
                self.close_podium();
                self.ui.hide_endscreen();
                self.m.ended = false;
                self.paused = false;
                self.lobby_drive = false;
                self.ui.set_lobby_drive(false);
                self.screen = Screen::Lobby;
            }
            LobbyPhase::Playing if self.screen == Screen::Lobby => {
                self.screen = Screen::InGame;
                self.sound.ui_confirm();
                self.hints.clear_queue(&mut self.ui);
            }
            _ => {}
        }
        self.party_label(&l);
        if self.screen != Screen::Lobby {
            self.ui.show_lobby(None);
            return;
        }
        let code = self.net.as_ref().and_then(|n| n.state().code.clone()).unwrap_or_default();
        let link = if self.invite_base.is_empty() { format!("?room={code}") } else { format!("{}?room={code}", self.invite_base) };
        let view = lobby_view(&l, self.you(), &code, &link, self.my_paint());
        self.ui.show_lobby(Some(&view));
    }

    /// Party rooms: every crew in their own paint (the Garage pick they sent).
    pub(super) fn paint_crews(&self, world: &mut RenderWorld, you: Option<arena_core::PlayerId>) {
        let Some(net) = &self.net else { return };
        let st = net.state();
        if st.lobby.is_none() {
            return;
        }
        let mine = self.my_paint();
        for t in &mut world.tanks {
            if Some(t.player) == you && mine.is_some() {
                continue;
            }
            if let Some(p) = st.players.iter().find(|p| p.id == t.player && !p.is_bot) {
                t.colour = PAINT_SET[p.loadout.paint as usize % PAINT_SET.len()];
            }
        }
    }

    /// The end screen's last line.
    pub(super) fn next_round_note(&self) -> &'static str {
        match (&self.session, self.lobby().is_some()) {
            (_, true) => "Back to the yard in a few seconds: vote for the next one",
            (Some(super::Session::Online(_)), _) => "Next round rolls out in a few seconds",
            _ => "Again for another round",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::net::{Vote, VoteCard};

    fn crew(id: u8, name: &str, host: bool, ready: bool) -> Crew {
        Crew { id, name: name.into(), host, ready, team: TeamPick::Auto, loadout: Loadout::default(), vote: None, away: false }
    }

    #[test]
    fn the_plate_reads_the_room() {
        let mut l = LobbyState {
            phase: LobbyPhase::Lobby,
            settings: RoomSettings { match_secs: 30, ..RoomSettings::default() },
            crew: vec![crew(0, "Ann", true, false), crew(1, "Bo", false, false)],
            force_in: 6,
            vote: None,
        };
        let v = lobby_view(&l, Some(1), "ABCD", "http://x/?room=ABCD", None);
        assert!(!v.you_host && v.host_name == "Ann");
        assert!(!v.can_start && v.start_label == "Start (6)");
        assert_eq!(v.status, "Tick Ready when you're set");
        assert_eq!(v.secs, 30);
        assert!(!v.team_mode);
        l.crew[1].ready = true;
        let v = lobby_view(&l, Some(0), "ABCD", "", None);
        assert!(v.you_host && v.can_start && v.start_label == "Start");
        assert!(v.crew[0].you && v.crew[0].host);
        l.crew[1].ready = false;
        l.force_in = 0;
        assert_eq!(lobby_view(&l, Some(0), "", "", None).start_label, "Start anyway");
        l.vote = Some(Vote { cards: vec![VoteCard { mode: ModeKind::ScrapBrawl, map: MapId::Sink }; 3], secs_left: 9 });
        l.crew[0].vote = Some(2);
        l.crew[1].vote = Some(2);
        let v = lobby_view(&l, Some(0), "", "", None);
        assert!(!v.can_start);
        let (cards, secs, mine) = v.vote.unwrap();
        assert_eq!((cards[2].votes, secs, mine), (2, 9, Some(2)));
        assert!(cards[0].again);
    }

    #[test]
    fn plate_settings_apply() {
        let cur = RoomSettings::default();
        let cfg = LobbyConfig {
            mode: "wellhead".into(),
            map: "rustbelt".into(),
            bots: false,
            skill: "hard".into(),
            secs: None,
            vote: "mixed".into(),
        };
        let s = apply_config(cur, &cfg);
        assert_eq!((s.mode, s.map, s.bots, s.skill, s.match_secs, s.vote), (ModeKind::Wellhead, MapId::RustBelt, false, BotSkill::Hard, cur.match_secs, VotePool::Mixed));
        assert_eq!(apply_config(cur, &LobbyConfig { secs: Some(180), ..cfg }).match_secs, 180);
        for m in [ModeKind::ScrapBrawl, ModeKind::TeamBrawl, ModeKind::RaiderWaves, ModeKind::Convoy, ModeKind::Wellhead, ModeKind::LastCrew] {
            assert_eq!(mode_of(mode_key(m)), Some(m));
        }
    }
}
