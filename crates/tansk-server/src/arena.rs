//! TANSK 2026 rooms: an authoritative `arena_core::Sim` at 60 Hz, snapshots
//! and events to every crew at 30 Hz, bots in every empty seat.
//!
//! Seats: always [`ROOM_SEATS`] tanks. A joining human takes a bot's seat; a
//! human who drops keeps their tank (a bot drives it) for [`HOLD_TICKS`] so
//! they can rejoin with their token, then it becomes an ordinary bot.
//!
//! Party rooms ([`ArenaRoom::new_party`]) run a lobby around the matches:
//! Lobby (a warm-up world of just the crews, no scoring; ready ticks, the
//! host's settings, team picks) -> Playing (the host's Start rebuilds the
//! world with the chosen mode, map and bots) -> Ended (the end screen) ->
//! Lobby again, with a 15 s vote on the next map. If the host leaves, the
//! next human in the room becomes host.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arena_core::bots::{Brain, BOT_NAMES};
use arena_core::garage::Loadout;
use arena_core::net::{
    encode, tally, Crew, LobbyCmd, LobbyPhase, LobbyState, RoomSettings, ServerMsg, TeamPick, Vote, VoteCard, VotePool, ROOM_SEATS,
};
use arena_core::raid::{ModeKind, TEAM_RAIDERS};
use arena_core::{Event, Input, Map, MapId, Phase, PlayerId, Sim, Tuning, Vec3, TICK_HZ};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

/// How long a dropped human's seat is held for a rejoin.
pub const HOLD_TICKS: u32 = 60 * TICK_HZ;
/// The end screen, before the next match starts.
const END_TICKS: u32 = 10 * TICK_HZ;
/// Defenders in a Raider Waves room (humans and ally bots); the sim adds the raiders.
pub const RAID_SEATS: usize = 4;
/// Inputs queued beyond this are dropped (keeps a fast client's lag bounded).
const MAX_QUEUE: usize = 4;
const BOT_DIFFICULTY: f32 = 0.5;
const MAX_NAME: usize = 16;
const MAX_CHAT: usize = 120;
/// Party rooms: the map vote after a match.
pub const VOTE_TICKS: u32 = 15 * TICK_HZ;
/// Party rooms: the host may start without everyone ready after this long.
pub const FORCE_TICKS: u32 = 10 * TICK_HZ;
/// The modes a mixed vote draws from.
const VOTE_MODES: [ModeKind; 6] =
    [ModeKind::ScrapBrawl, ModeKind::TeamBrawl, ModeKind::RaiderWaves, ModeKind::Convoy, ModeKind::Wellhead, ModeKind::LastCrew];

/// One frame for a client's socket; `last` closes it after sending.
pub struct Frame {
    pub bytes: Vec<u8>,
    pub last: bool,
}

pub type Outbox = mpsc::Sender<Frame>;

fn send(out: &Outbox, msg: &ServerMsg) {
    let last = matches!(msg, ServerMsg::Rejected(_));
    // A client that can't keep up misses messages rather than queueing them.
    let _ = out.try_send(Frame { bytes: encode(msg), last });
}

pub enum ArenaCmd {
    Join { conn: u64, name: String, rejoin: Option<u64>, out: Outbox },
    Leave { conn: u64 },
    Input { conn: u64, seq: u32, input: Input },
    Chat { conn: u64, text: String },
    Lobby { conn: u64, cmd: LobbyCmd },
}

/// What the lobby list shows about a room.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ArenaInfo {
    pub code: String,
    pub public: bool,
    /// Connected humans in the room. Dropped ones whose seat is held for a
    /// rejoin don't count: the lobby and quick match must not see ghosts.
    pub humans: usize,
    pub max_humans: usize,
    pub ended: bool,
    pub mode: ModeKind,
    pub map: MapId,
    /// A party room (pre-match lobby): quick match leaves it alone.
    pub party: bool,
}

struct Human {
    player: PlayerId,
    name: String,
    token: u64,
    conn: Option<u64>,
    out: Option<Outbox>,
    queue: VecDeque<(u32, Input)>,
    input: Input,
    ack: u32,
    away: u32,
    /// Party rooms.
    ready: bool,
    team: TeamPick,
    vote: Option<u8>,
    loadout: Loadout,
}

impl Human {
    fn new(player: PlayerId, name: String, token: u64, conn: u64, out: Outbox) -> Self {
        Human {
            player,
            name,
            token,
            conn: Some(conn),
            out: Some(out),
            queue: VecDeque::new(),
            input: Input::default(),
            ack: 0,
            away: 0,
            ready: false,
            team: TeamPick::Auto,
            vote: None,
            loadout: Loadout::default(),
        }
    }
}

/// A party room's lobby state machine.
struct Party {
    settings: RoomSettings,
    phase: LobbyPhase,
    /// The host's token.
    host: Option<u64>,
    /// Ticks since the lobby opened (or its vote closed).
    lobby_ticks: u32,
    /// The open vote: cards and ticks left.
    vote: Option<(Vec<VoteCard>, u32)>,
    /// The last state sent, to send only changes.
    sent: Option<LobbyState>,
}

pub struct ArenaRoom {
    code: String,
    public: bool,
    seed: u64,
    map: MapId,
    rng: u64,
    pub sim: Sim,
    humans: Vec<Human>,
    events: Vec<Event>,
    ended_for: u32,
    info: Arc<Mutex<ArenaInfo>>,
    party: Option<Party>,
}

impl ArenaRoom {
    pub fn new(code: String, public: bool, seed: u64, info: Arc<Mutex<ArenaInfo>>, mode: ModeKind, map: MapId) -> Self {
        let sim = Sim::with_mode(seed, Map::by_id(map, seed), Tuning::default(), mode);
        let mut room = ArenaRoom {
            code,
            public,
            seed,
            map,
            rng: seed ^ 0x5DEE_CE66_D1CE_5EED,
            sim,
            humans: Vec::new(),
            events: Vec::new(),
            ended_for: 0,
            info,
            party: None,
        };
        room.fill_bots();
        room.sim.drain_events();
        room
    }

    /// A room with a lobby: everyone warms up until the host starts.
    pub fn new_party(code: String, public: bool, seed: u64, info: Arc<Mutex<ArenaInfo>>, settings: RoomSettings) -> Self {
        let settings = settings.clamped();
        let mut room = ArenaRoom {
            code,
            public,
            seed,
            map: settings.map,
            rng: seed ^ 0x5DEE_CE66_D1CE_5EED,
            sim: warmup_sim(seed, settings.map),
            humans: Vec::new(),
            events: Vec::new(),
            ended_for: 0,
            info,
            party: Some(Party { settings, phase: LobbyPhase::Lobby, host: None, lobby_ticks: 0, vote: None, sent: None }),
        };
        room.sim.drain_events();
        room.publish_info();
        room
    }

    fn in_lobby(&self) -> bool {
        self.party.as_ref().is_some_and(|p| p.phase == LobbyPhase::Lobby)
    }

    /// Runs the room until no human (connected or held) is left.
    pub async fn run(mut self, mut rx: mpsc::UnboundedReceiver<ArenaCmd>) {
        let mut tick = tokio::time::interval(Duration::from_nanos(1_000_000_000 / TICK_HZ as u64));
        // Burst: a late tick is caught up, so the room keeps 60 Hz on average.
        tick.set_missed_tick_behavior(MissedTickBehavior::Burst);
        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    let Some(cmd) = cmd else { break };
                    self.handle(cmd);
                }
                _ = tick.tick() => {
                    self.tick();
                    if self.humans.is_empty() {
                        break;
                    }
                }
            }
        }
    }

    pub fn handle(&mut self, cmd: ArenaCmd) {
        match cmd {
            ArenaCmd::Join { conn, name, rejoin, out } => self.join(conn, name, rejoin, out),
            ArenaCmd::Leave { conn } => self.leave(conn),
            ArenaCmd::Input { conn, seq, input } => {
                if let Some(h) = self.humans.iter_mut().find(|h| h.conn == Some(conn)) {
                    if seq > h.queue.back().map_or(h.ack, |q| q.0) {
                        h.queue.push_back((seq, sanitize(input)));
                    }
                }
            }
            ArenaCmd::Chat { conn, text } => {
                let Some(h) = self.humans.iter().find(|h| h.conn == Some(conn)) else { return };
                let text: String = text.chars().filter(|c| !c.is_control()).take(MAX_CHAT).collect();
                let text = text.trim();
                if !text.is_empty() {
                    let msg = ServerMsg::Chat { from: h.name.clone(), text: text.to_string() };
                    self.broadcast(&msg);
                }
            }
            ArenaCmd::Lobby { conn, cmd } => self.lobby_cmd(conn, cmd),
        }
        self.publish_info();
        self.send_lobby(false);
    }

    fn join(&mut self, conn: u64, name: String, rejoin: Option<u64>, out: Outbox) {
        let held = rejoin.and_then(|t| self.humans.iter().position(|h| h.token == t && h.conn.is_none()));
        let mut back = None;
        let i = if let Some(i) = held {
            let h = &mut self.humans[i];
            h.conn = Some(conn);
            h.out = Some(out);
            h.queue.clear();
            h.input = Input::default();
            h.ack = 0;
            h.away = 0;
            if let Some(p) = self.sim.players.iter_mut().find(|p| p.id == h.player) {
                p.brain = None;
            }
            back = Some(format!("{} is back.", h.name));
            i
        } else {
            if self.humans.iter().filter(|h| h.conn.is_some()).count() >= self.max_humans() {
                send(&out, &ServerMsg::Rejected("That game is full.".into()));
                return;
            }
            self.free_seat();
            let name = self.unique_name(&name);
            let player = self.sim.add_player(&name);
            let token = self.random();
            self.broadcast(&ServerMsg::Info(format!("{name} joined.")));
            self.humans.push(Human::new(player, name, token, conn, out));
            self.humans.len() - 1
        };
        let h = &self.humans[i];
        if let Some(out) = &h.out {
            send(
                out,
                &ServerMsg::Welcome {
                    code: self.code.clone(),
                    public: self.public,
                    you: h.player,
                    seed: self.seed,
                    token: h.token,
                    map: self.map,
                },
            );
        }
        if let Some(text) = back {
            self.broadcast(&ServerMsg::Info(text));
        }
        self.broadcast(&ServerMsg::Players(self.sim.players()));
        if self.party.is_some() {
            self.fix_host();
            // The newcomer needs the whole lobby, changed or not.
            self.send_lobby(true);
        }
    }

    /// Makes room for one more tank: a bot leaves, else the longest-held
    /// dropped human's seat goes.
    fn free_seat(&mut self) {
        if self.seats_used() < self.seats() {
            return;
        }
        // In team modes the bot leaves the bigger team, so the newcomer (who
        // joins the smaller one) keeps the sides even.
        let raid = self.sim.mode == ModeKind::RaiderWaves;
        let crews = |t: u8| self.sim.players.iter().filter(|p| p.team == Some(t) && !self.sim.is_convoy_truck(p.id)).count();
        let bigger = if raid { None } else { Some(if crews(1) > crews(0) { 1 } else { 0 }) };
        let candidates: Vec<&arena_core::sim::Player> = self
            .sim
            .players
            .iter()
            .rev()
            .filter(|p| p.brain.is_some() && !(raid && p.team == Some(TEAM_RAIDERS)) && !self.humans.iter().any(|h| h.player == p.id))
            .collect();
        // Last Crew Rolling: a bot that's already out makes way first.
        let out = candidates.iter().find(|p| self.sim.last_crew.is_some() && !p.tank.alive).map(|p| p.id);
        let bot = out.or_else(|| {
            candidates
                .iter()
                .find(|p| bigger.is_none() || p.team.is_none() || p.team == bigger)
                .or(candidates.first())
                .map(|p| p.id)
        });

        let victim = bot.or_else(|| {
            let i = self.humans.iter().enumerate().filter(|(_, h)| h.conn.is_none()).max_by_key(|(_, h)| h.away)?.0;
            Some(self.humans.remove(i).player)
        });
        if let Some(id) = victim {
            self.sim.remove_player(id);
        }
    }

    fn leave(&mut self, conn: u64) {
        let Some(h) = self.humans.iter_mut().find(|h| h.conn == Some(conn)) else { return };
        h.conn = None;
        h.out = None;
        h.queue.clear();
        h.away = 0;
        h.ready = false;
        h.vote = None;
        let (id, name) = (h.player, h.name.clone());
        let seed = self.random();
        // In the lobby an empty tank just idles; in a match a bot drives it.
        let brain = (!self.in_lobby()).then(|| Brain::new(seed, self.bot_difficulty()));
        if let Some(p) = self.sim.players.iter_mut().find(|p| p.id == id) {
            p.input = Input::default();
            p.brain = brain;
        }
        self.broadcast(&ServerMsg::Info(format!("{name} left.")));
        self.broadcast(&ServerMsg::Players(self.sim.players()));
        if self.party.is_some() {
            self.fix_host();
        }
    }

    /// One 60 Hz tick: inputs, step, the end-of-match timer, and every
    /// other tick a snapshot to everyone.
    pub fn tick(&mut self) {
        let mut roster_changed = false;
        for h in &mut self.humans {
            if h.conn.is_some() {
                while h.queue.len() > MAX_QUEUE {
                    h.queue.pop_front();
                }
                if let Some((seq, input)) = h.queue.pop_front() {
                    h.ack = seq;
                    h.input = input;
                }
                self.sim.set_input(h.player, h.input);
            } else {
                h.away += 1;
            }
        }
        // Held seats that ran out become plain bots.
        let mut i = 0;
        while i < self.humans.len() {
            if self.humans[i].conn.is_none() && self.humans[i].away > HOLD_TICKS {
                let h = self.humans.remove(i);
                if self.in_lobby() {
                    // No bots in the warm-up: the tank goes with its crew.
                    self.sim.remove_player(h.player);
                } else {
                    let name = self.bot_name();
                    if let Some(p) = self.sim.players.iter_mut().find(|p| p.id == h.player) {
                        p.name = name;
                    }
                }
                roster_changed = true;
            } else {
                i += 1;
            }
        }

        self.sim.step();
        self.events.extend(self.sim.drain_events());
        if self.party.is_some() {
            self.party_tick();
        } else if matches!(self.sim.phase, Phase::Ended { .. } | Phase::TeamWon { .. }) {
            self.ended_for += 1;
            if self.ended_for >= END_TICKS {
                self.ended_for = 0;
                self.sim.reset_match();
                self.events.extend(self.sim.drain_events());
                self.publish_info();
            }
        }
        if roster_changed {
            self.broadcast(&ServerMsg::Players(self.sim.players()));
            self.publish_info();
            if self.party.is_some() {
                self.fix_host();
            }
        }
        self.send_lobby(false);

        if self.sim.tick.is_multiple_of(TICK_HZ / arena_core::net::SNAPSHOT_HZ) {
            let snapshot = self.sim.snapshot();
            let events = std::mem::take(&mut self.events);
            for h in &self.humans {
                if let Some(out) = &h.out {
                    send(out, &ServerMsg::Tick { ack: h.ack, snapshot: snapshot.clone(), events: events.clone() });
                }
            }
        }
    }

    /// Seats taken: every tank, or in a raid only the defenders (the Convoy
    /// truck takes no seat either). Team 1 is only "the raiders" in Raider
    /// Waves: in Team Brawl and Convoy it is the Dust Saints and takes seats.
    fn seats_used(&self) -> usize {
        let raid = self.sim.mode == ModeKind::RaiderWaves;
        self.sim.players.iter().filter(|p| !(raid && p.team == Some(TEAM_RAIDERS)) && !self.sim.is_convoy_truck(p.id)).count()
    }



    fn seats(&self) -> usize {
        if self.sim.mode == ModeKind::RaiderWaves {
            // A party of more than four all defend.
            RAID_SEATS.max(if self.party.is_some() { self.humans.len() } else { 0 })
        } else {
            ROOM_SEATS
        }
    }

    /// Humans the room takes at once.
    fn max_humans(&self) -> usize {
        if self.party.is_some() {
            ROOM_SEATS
        } else {
            self.seats()
        }
    }

    fn bot_difficulty(&self) -> f32 {
        self.party.as_ref().map_or(BOT_DIFFICULTY, |p| p.settings.skill.difficulty())
    }

    /// Bots fill empty seats, except in a party's warm-up or with the host's bots off.
    fn wants_bots(&self) -> bool {
        self.party.as_ref().is_none_or(|p| p.phase != LobbyPhase::Lobby && p.settings.bots)
    }

    fn fill_bots(&mut self) {
        if !self.wants_bots() {
            return;
        }
        let difficulty = self.bot_difficulty();
        while self.seats_used() < self.seats() {
            let name = self.bot_name();
            self.sim.add_bot(&name, difficulty);
        }
    }

    fn bot_name(&mut self) -> String {
        let free: Vec<&str> =
            BOT_NAMES.iter().copied().filter(|n| !self.sim.players.iter().any(|p| p.name == *n)).collect();
        if free.is_empty() {
            format!("Bot{}", self.random() % 100)
        } else {
            free[(self.random() % free.len() as u64) as usize].to_string()
        }
    }

    fn unique_name(&mut self, wanted: &str) -> String {
        let base: String = wanted.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(MAX_NAME).collect();
        let base = if base.is_empty() { "Crew".to_string() } else { base };
        let mut name = base.clone();
        let mut n = 2;
        while self.sim.players.iter().any(|p| p.name == name) {
            name = format!("{base} {n}");
            n += 1;
        }
        name
    }

    fn random(&mut self) -> u64 {
        // splitmix64
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn broadcast(&self, msg: &ServerMsg) {
        let bytes = encode(msg);
        for out in self.humans.iter().filter_map(|h| h.out.as_ref()) {
            let _ = out.try_send(Frame { bytes: bytes.clone(), last: false });
        }
    }

    fn publish_info(&mut self) {
        self.fill_bots();
        let mut info = self.info.lock().unwrap();
        info.humans = self.humans.iter().filter(|h| h.conn.is_some()).count();
        info.ended = matches!(self.sim.phase, Phase::Ended { .. } | Phase::Defeated { .. } | Phase::TeamWon { .. });
        if let Some(p) = &self.party {
            info.party = true;
            info.mode = p.settings.mode;
            info.map = self.map;
            info.max_humans = ROOM_SEATS;
            info.ended = p.phase == LobbyPhase::Ended;
        }
    }

    // ------------------------------------------------------------ party

    fn lobby_cmd(&mut self, conn: u64, cmd: LobbyCmd) {
        let Some(i) = self.humans.iter().position(|h| h.conn == Some(conn)) else { return };
        if let LobbyCmd::Loadout(l) = cmd {
            let l = Loadout { paint: l.paint % 8, emblem: l.emblem % 8, horn: l.horn % 8, ..l };
            self.humans[i].loadout = l;
            let id = self.humans[i].player;
            self.sim.set_loadout(id, l);
            if self.in_lobby() {
                // Warming up: the chassis changes on the spot.
                let tuning = self.sim.tuning.clone();
                if let Some(p) = self.sim.players.iter_mut().find(|p| p.id == id) {
                    if p.tank.alive && p.tank.vehicle == arena_core::parts::VehicleKind::Tank && p.tank.class != l.chassis {
                        p.tank.set_class(&mut self.sim.phys, &tuning, l.chassis);
                    }
                }
            }
            self.broadcast(&ServerMsg::Players(self.sim.players()));
            return;
        }
        let token = self.humans[i].token;
        let Some(p) = &mut self.party else { return };
        if p.phase != LobbyPhase::Lobby {
            return;
        }
        let is_host = p.host == Some(token);
        match cmd {
            LobbyCmd::Ready(r) => self.humans[i].ready = r,
            LobbyCmd::Team(t) => self.humans[i].team = t,
            LobbyCmd::Vote(v) => {
                if p.vote.as_ref().is_some_and(|(cards, _)| (v as usize) < cards.len()) {
                    self.humans[i].vote = Some(v);
                }
            }
            LobbyCmd::Settings(s) if is_host => {
                let s = s.clamped();
                let new_map = s.map != p.settings.map;
                p.settings = s;
                if new_map {
                    self.rebuild(false);
                }
            }
            LobbyCmd::Start if is_host => {
                if self.lobby_state().is_some_and(|st| st.can_start()) {
                    self.start_match();
                }
            }
            LobbyCmd::Settings(_) | LobbyCmd::Start | LobbyCmd::Loadout(_) => {}
        }
    }

    /// The host must be a connected human: if not, the next one takes over.
    fn fix_host(&mut self) {
        let Some(p) = &self.party else { return };
        if p.host.is_some_and(|t| self.humans.iter().any(|h| h.token == t && h.conn.is_some())) {
            return;
        }
        let Some(next) = self.humans.iter().find(|h| h.conn.is_some()) else { return };
        let (token, name) = (next.token, next.name.clone());
        let had = p.host.is_some();
        if let Some(p) = &mut self.party {
            p.host = Some(token);
        }
        if had {
            self.broadcast(&ServerMsg::Info(format!("{name} is the host now.")));
        }
    }

    fn lobby_state(&self) -> Option<LobbyState> {
        let p = self.party.as_ref()?;
        let crew = self
            .humans
            .iter()
            .map(|h| Crew {
                id: h.player,
                name: h.name.clone(),
                host: p.host == Some(h.token),
                ready: h.ready,
                team: h.team,
                loadout: h.loadout,
                vote: h.vote,
                away: h.conn.is_none(),
            })
            .collect();
        let secs = |ticks: u32| ticks.div_ceil(TICK_HZ).min(255) as u8;
        Some(LobbyState {
            phase: p.phase,
            settings: p.settings,
            crew,
            force_in: secs(FORCE_TICKS.saturating_sub(p.lobby_ticks)),
            vote: p.vote.as_ref().map(|(cards, left)| Vote { cards: cards.clone(), secs_left: secs(*left) }),
        })
    }

    /// Sends the lobby to everyone if it changed (or `force`).
    fn send_lobby(&mut self, force: bool) {
        let Some(state) = self.lobby_state() else { return };
        let Some(p) = self.party.as_mut() else { return };
        if !force && p.sent.as_ref() == Some(&state) {
            return;
        }
        p.sent = Some(state.clone());
        self.broadcast(&ServerMsg::Lobby(state));
    }

    /// The party's per-tick rules: no scoring in the warm-up, the vote and
    /// start timers, the end screen back into the lobby.
    fn party_tick(&mut self) {
        let Some(p) = &mut self.party else { return };
        match p.phase {
            LobbyPhase::Lobby => {
                for pl in &mut self.sim.players {
                    pl.kills = 0;
                    pl.deaths = 0;
                }
                let mut close_vote = false;
                if let Some((_, left)) = &mut p.vote {
                    *left = left.saturating_sub(1);
                    let mut voters = self.humans.iter().filter(|h| h.conn.is_some()).peekable();
                    let all_voted = voters.peek().is_some() && voters.all(|h| h.vote.is_some());
                    close_vote = *left == 0 || all_voted;
                } else {
                    p.lobby_ticks = p.lobby_ticks.saturating_add(1);
                }
                if close_vote {
                    self.close_vote();
                }
            }
            LobbyPhase::Playing => {
                if !matches!(self.sim.phase, Phase::Playing) {
                    p.phase = LobbyPhase::Ended;
                    self.ended_for = 0;
                    self.publish_info();
                }
            }
            LobbyPhase::Ended => {
                self.ended_for += 1;
                if self.ended_for >= END_TICKS {
                    self.ended_for = 0;
                    self.back_to_lobby();
                }
            }
        }
    }

    /// The host's Start: the match world with the chosen mode, map and bots.
    fn start_match(&mut self) {
        if let Some(p) = &mut self.party {
            p.phase = LobbyPhase::Playing;
        }
        self.rebuild(true);
        self.broadcast(&ServerMsg::Info("Rolling out!".into()));
    }

    /// After the end screen: everyone back to the lobby, and the map vote.
    fn back_to_lobby(&mut self) {
        let cards = self.vote_cards();
        if let Some(p) = &mut self.party {
            p.phase = LobbyPhase::Lobby;
            p.lobby_ticks = 0;
            p.vote = Some((cards, VOTE_TICKS));
        }
        for h in &mut self.humans {
            h.ready = false;
            h.vote = None;
        }
        self.rebuild(false);
        self.broadcast(&ServerMsg::Info("Back in the yard. Vote for the next one.".into()));
    }

    /// Three cards; card 0 is the rematch (this mode and map).
    fn vote_cards(&mut self) -> Vec<VoteCard> {
        let Some(s) = self.party.as_ref().map(|p| p.settings) else { return Vec::new() };
        let here = VoteCard { mode: s.mode, map: s.map };
        match s.vote {
            VotePool::Maps => {
                vec![here, VoteCard { mode: s.mode, map: s.map.next() }, VoteCard { mode: s.mode, map: s.map.next().next() }]
            }
            VotePool::Mixed => {
                let mut cards = vec![here];
                for _ in 0..100 {
                    if cards.len() == 3 {
                        break;
                    }
                    let mode = VOTE_MODES[(self.random() % VOTE_MODES.len() as u64) as usize];
                    let map = MapId::ALL[(self.random() % MapId::ALL.len() as u64) as usize];
                    if !cards.iter().any(|c| c.mode == mode) {
                        cards.push(VoteCard { mode, map });
                    }
                }
                cards
            }
        }
    }

    fn close_vote(&mut self) {
        let Some(p) = &mut self.party else { return };
        let Some((cards, _)) = p.vote.take() else { return };
        p.lobby_ticks = 0;
        let votes: Vec<u8> = self.humans.iter().filter(|h| h.conn.is_some()).filter_map(|h| h.vote).collect();
        let host = self.humans.iter().find(|h| Some(h.token) == p.host).and_then(|h| h.vote);
        let Some(&card) = cards.get(tally(cards.len(), &votes, host)) else { return };
        let new_map = card.map != p.settings.map;
        p.settings.mode = card.mode;
        p.settings.map = card.map;
        for h in &mut self.humans {
            h.vote = None;
        }
        if new_map {
            self.rebuild(false);
        }
        self.broadcast(&ServerMsg::Info(format!("Next up: {} on {}.", card.mode.name(), card.map.name())));
    }

    /// A fresh world for the party (the warm-up, or the match when `play`):
    /// every crew gets a tank in it and a new `Welcome`.
    fn rebuild(&mut self, play: bool) {
        let Some(s) = self.party.as_ref().map(|p| p.settings) else { return };
        self.sim = if play { match_sim(self.seed, &s) } else { warmup_sim(self.seed, s.map) };
        self.map = s.map;
        self.events.clear();
        self.ended_for = 0;
        let difficulty = s.skill.difficulty();
        for i in 0..self.humans.len() {
            let seed = self.random();
            let h = &mut self.humans[i];
            h.player = self.sim.add_player_with(&h.name, h.loadout);
            h.queue.clear();
            h.input = Input::default();
            if play && h.conn.is_none() {
                if let Some(p) = self.sim.players.iter_mut().find(|p| p.id == h.player) {
                    p.brain = Some(Brain::new(seed, difficulty));
                }
            }
        }
        if play && s.mode.is_team() {
            // Picks first, then the "auto" crews onto the short side; the
            // bots (added next) fill the smaller team.
            let mut count = [0usize; 2];
            let mut sides: Vec<Option<u8>> = self
                .humans
                .iter()
                .map(|h| match h.team {
                    TeamPick::Rustbacks => Some(0),
                    TeamPick::DustSaints => Some(1),
                    TeamPick::Auto => None,
                })
                .collect();
            for t in sides.iter().flatten() {
                count[*t as usize] += 1;
            }
            for side in sides.iter_mut().filter(|s| s.is_none()) {
                let t = u8::from(count[1] < count[0]);
                count[t as usize] += 1;
                *side = Some(t);
            }
            for (h, t) in self.humans.iter().zip(&sides) {
                if let Some(p) = self.sim.players.iter_mut().find(|p| p.id == h.player) {
                    p.team = *t;
                }
            }
        }
        if play {
            self.fill_bots();
            // Everyone to their side's spawns, a clean start.
            self.sim.reset_match();
        }
        self.sim.drain_events();
        for h in &self.humans {
            if let Some(out) = &h.out {
                send(
                    out,
                    &ServerMsg::Welcome {
                        code: self.code.clone(),
                        public: self.public,
                        you: h.player,
                        seed: self.seed,
                        token: h.token,
                        map: self.map,
                    },
                );
            }
        }
        self.broadcast(&ServerMsg::Players(self.sim.players()));
        self.publish_info();
        self.send_lobby(true);
    }
}

/// A party's warm-up world: Scrap Brawl rules on the map with no clock and
/// no kill limit (the room zeroes the scores), and no bots.
fn warmup_sim(seed: u64, map: MapId) -> Sim {
    let mut t = Tuning::default();
    t.mode.time_limit = 1.0e7;
    t.mode.kill_limit = 0;
    Sim::with_mode(seed, Map::by_id(map, seed), t, ModeKind::ScrapBrawl)
}

/// A party's match world, on the host's clock.
fn match_sim(seed: u64, s: &RoomSettings) -> Sim {
    let mut t = Tuning::default();
    let secs = f32::from(s.match_secs);
    t.mode.time_limit = secs;
    t.wellhead.time_limit = secs;
    Sim::with_mode(seed, Map::by_id(s.map, seed), t, s.mode)
}

/// Inputs come from the network: keep them finite and in range.
fn sanitize(mut i: Input) -> Input {
    let unit = |x: f32| if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 };
    i.throttle = unit(i.throttle);
    i.steer = unit(i.steer);
    i.aim_point = if i.aim_point.is_finite() { i.aim_point.clamp(Vec3::splat(-5000.0), Vec3::splat(5000.0)) } else { Vec3::ZERO };
    i
}

pub type ArenaEntry = (mpsc::UnboundedSender<ArenaCmd>, Arc<Mutex<ArenaInfo>>);

/// The arena room registry.
#[derive(Default)]
pub struct ArenaRooms {
    pub rooms: Mutex<HashMap<String, ArenaEntry>>,
}

impl ArenaRooms {
    /// Open public rooms with someone in them, for the lobby list.
    pub fn list(&self) -> Vec<ArenaInfo> {
        let rooms = self.rooms.lock().unwrap();
        let mut v: Vec<ArenaInfo> = rooms
            .values()
            .filter(|(tx, _)| !tx.is_closed())
            .map(|(_, i)| i.lock().unwrap().clone())
            .filter(|i| i.public && i.humans > 0)
            .collect();
        v.sort_by(|a, b| b.humans.cmp(&a.humans).then(a.code.cmp(&b.code)));
        v
    }

    /// Quick match: the fullest public room with someone connected and a free
    /// seat (a room of only held seats is a ghost; start a fresh one). Reserves the
    /// seat in its info so two players at once don't both take the last one.
    pub fn quick_match(&self) -> Option<mpsc::UnboundedSender<ArenaCmd>> {
        let rooms = self.rooms.lock().unwrap();
        let (tx, info) = rooms
            .values()
            .filter(|(tx, i)| {
                let i = i.lock().unwrap();
                !tx.is_closed() && i.public && !i.party && i.mode == ModeKind::ScrapBrawl && i.humans > 0 && i.humans < i.max_humans
            })
            .max_by_key(|(_, i)| i.lock().unwrap().humans)?;
        info.lock().unwrap().humans += 1;
        Some(tx.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(public: bool) -> ArenaRoom {
        let info = Arc::new(Mutex::new(ArenaInfo {
            code: "TEST".into(),
            public,
            humans: 0,
            max_humans: ROOM_SEATS,
            ended: false,
            mode: ModeKind::ScrapBrawl,
            map: MapId::Sink,
            party: false,
        }));
        ArenaRoom::new("TEST".into(), public, 11, info, ModeKind::ScrapBrawl, MapId::Sink)
    }

    /// A Refinery room builds the Refinery and says so in its Welcome.
    #[test]
    fn refinery_room_welcomes_with_its_map() {
        let info = Arc::new(Mutex::new(ArenaInfo {
            code: "REFI".into(),
            public: false,
            humans: 0,
            max_humans: ROOM_SEATS,
            ended: false,
            mode: ModeKind::TeamBrawl,
            map: MapId::Refinery,
            party: false,
        }));
        let mut r = ArenaRoom::new("REFI".into(), false, 5, info, ModeKind::TeamBrawl, MapId::Refinery);
        assert_eq!(r.sim.map.id, MapId::Refinery);
        let (out, mut rx) = mpsc::channel(64);
        r.handle(ArenaCmd::Join { conn: 1, name: "Ann".into(), rejoin: None, out });
        let first = arena_core::net::decode::<ServerMsg>(&rx.try_recv().unwrap().bytes).unwrap();
        assert!(matches!(first, ServerMsg::Welcome { map: MapId::Refinery, seed: 5, .. }), "{first:?}");
        for _ in 0..10 * TICK_HZ {
            r.tick();
        }
    }

    /// A Rust Belt room builds the Rust Belt and says so in its Welcome;
    /// the quick-match rotation reaches it.
    #[test]
    fn rust_belt_room_welcomes_with_its_map() {
        assert_eq!(MapId::Refinery.next(), MapId::RustBelt);
        assert_eq!(MapId::RustBelt.next(), MapId::Sink);
        let info = Arc::new(Mutex::new(ArenaInfo {
            code: "RUST".into(),
            public: false,
            humans: 0,
            max_humans: ROOM_SEATS,
            ended: false,
            mode: ModeKind::ScrapBrawl,
            map: MapId::RustBelt,
            party: false,
        }));
        let mut r = ArenaRoom::new("RUST".into(), false, 9, info, ModeKind::ScrapBrawl, MapId::RustBelt);
        assert_eq!(r.sim.map.id, MapId::RustBelt);
        let (out, mut rx) = mpsc::channel(64);
        r.handle(ArenaCmd::Join { conn: 1, name: "Ann".into(), rejoin: None, out });
        let first = arena_core::net::decode::<ServerMsg>(&rx.try_recv().unwrap().bytes).unwrap();
        assert!(matches!(first, ServerMsg::Welcome { map: MapId::RustBelt, seed: 9, .. }), "{first:?}");
        for _ in 0..10 * TICK_HZ {
            r.tick();
        }
    }

    /// A Convoy room seats 4 v 4 crews; the truck takes no seat; a joining
    /// human takes a bot's seat on a team.
    #[test]
    fn convoy_room_seats_four_a_side_plus_the_truck() {
        let info = Arc::new(Mutex::new(ArenaInfo {
            code: "CNVY".into(),
            public: false,
            humans: 0,
            max_humans: ROOM_SEATS,
            ended: false,
            mode: ModeKind::Convoy,
            map: MapId::Sink,
            party: false,
        }));
        let mut r = ArenaRoom::new("CNVY".into(), false, 9, info, ModeKind::Convoy, MapId::Sink);
        let crews = |r: &ArenaRoom, t: u8| r.sim.players.iter().filter(|p| p.team == Some(t) && !r.sim.is_convoy_truck(p.id)).count();
        assert_eq!(r.sim.players.len(), ROOM_SEATS + 1);
        assert_eq!((crews(&r, 0), crews(&r, 1)), (4, 4));
        let (out, mut rx) = mpsc::channel(64);
        r.handle(ArenaCmd::Join { conn: 1, name: "Ann".into(), rejoin: None, out });
        let first = arena_core::net::decode::<ServerMsg>(&rx.try_recv().unwrap().bytes).unwrap();
        assert!(matches!(first, ServerMsg::Welcome { .. }), "{first:?}");
        assert_eq!(r.sim.players.len(), ROOM_SEATS + 1);
        assert_eq!((crews(&r, 0), crews(&r, 1)), (4, 4));
        // More humans: bots make way from the bigger side, the sides stay even.
        let mut keep = Vec::new();
        for conn in 2..6u64 {
            let (out, rx) = mpsc::channel(64);
            keep.push(rx);
            r.handle(ArenaCmd::Join { conn, name: format!("P{conn}"), rejoin: None, out });
            assert_eq!(r.sim.players.len(), ROOM_SEATS + 1);
            assert!(crews(&r, 0).abs_diff(crews(&r, 1)) <= 1, "{} v {}", crews(&r, 0), crews(&r, 1));
        }
        assert_eq!((crews(&r, 0), crews(&r, 1)), (4, 4));
        for _ in 0..5 * TICK_HZ {

            r.tick();
        }
        let snap = r.sim.snapshot();
        assert!(snap.convoy.is_some());
    }

    fn mode_room(code: &str, mode: ModeKind, map: MapId) -> ArenaRoom {
        let info = Arc::new(Mutex::new(ArenaInfo {
            code: code.into(),
            public: false,
            humans: 0,
            max_humans: ROOM_SEATS,
            ended: false,
            mode,
            map,
            party: false,
        }));
        ArenaRoom::new(code.into(), false, 13, info, mode, map)
    }

    /// A Wellhead room seats 4 v 4; joining humans keep the sides even; the
    /// snapshot carries the wells and the tanks.
    #[test]
    fn wellhead_room_seats_four_a_side_and_pumps() {
        for map in MapId::ALL {
            let mut r = mode_room("WELL", ModeKind::Wellhead, map);
            let crews = |r: &ArenaRoom, t: u8| r.sim.players.iter().filter(|p| p.team == Some(t)).count();
            assert_eq!(r.sim.players.len(), ROOM_SEATS);
            assert_eq!((crews(&r, 0), crews(&r, 1)), (4, 4));
            let mut keep = Vec::new();
            for conn in 1..4u64 {
                let (out, rx) = mpsc::channel(4096);
                keep.push(rx);
                r.handle(ArenaCmd::Join { conn, name: format!("P{conn}"), rejoin: None, out });
                assert!(crews(&r, 0).abs_diff(crews(&r, 1)) <= 1);
            }
            assert_eq!(r.sim.players.len(), ROOM_SEATS);
            for _ in 0..90 * TICK_HZ {
                r.tick();
            }
            let snap = r.sim.snapshot();
            let w = snap.wellhead.expect("wellhead snapshot");
            assert_eq!(w.wells.len(), 3);
            assert!(w.wells.iter().any(|x| x.owner.is_some()), "{map:?}: bots took no well in 90 s: {w:?}");
            assert!(!snap.teams.is_empty(), "team scores for the HUD");
        }
    }

    /// A Last Crew room: nobody respawns, the room ends with a winner and the
    /// next match starts with everyone rolling again.
    #[test]
    fn last_crew_room_ends_and_restarts() {
        let mut r = mode_room("LAST", ModeKind::LastCrew, MapId::Sink);
        assert_eq!(r.sim.players.len(), ROOM_SEATS);
        // Speed it up: short phases, a hard storm.
        r.sim.tuning.last_crew.hold = vec![3.0, 3.0];
        r.sim.tuning.last_crew.shrink = vec![5.0, 5.0];
        r.sim.tuning.last_crew.factor = vec![0.3, 0.0];
        r.sim.tuning.last_crew.damage_base = 200.0;
        let mut ended = false;
        for _ in 0..120 * TICK_HZ {
            r.tick();
            if let Phase::Ended { winner } = r.sim.phase {
                assert!(winner.is_some());
                ended = true;
                break;
            }
            // Nobody comes back mid-match.
            for p in &r.sim.players {
                assert!(p.tank.alive || p.tank.respawn_in.is_infinite());
            }
        }
        assert!(ended, "the storm ends it");
        // A late human waits for the next match; then everyone rolls.
        let (out, _rx) = mpsc::channel(4096);
        r.handle(ArenaCmd::Join { conn: 1, name: "Late".into(), rejoin: None, out });
        assert_eq!(r.sim.players.len(), ROOM_SEATS);
        for _ in 0..(END_TICKS + 2) {
            r.tick();
        }
        assert_eq!(r.sim.phase, Phase::Playing);
        assert!(r.sim.players.iter().all(|p| p.tank.alive), "everyone rolls in the new match");
        assert!(r.sim.snapshot().storm_circle.is_some());
    }

    /// A room of 8 bots runs a minute of play.

    #[test]
    fn eight_bots_for_a_minute() {
        let mut r = room(true);
        assert_eq!(r.sim.players.len(), ROOM_SEATS);
        let start: Vec<Vec3> = r.sim.players.iter().map(|p| r.sim.tank_pose(p.id).unwrap().0).collect();
        for _ in 0..60 * TICK_HZ {
            r.tick();
        }
        assert_eq!(r.sim.players.len(), ROOM_SEATS);
        let moved = r
            .sim
            .players
            .iter()
            .zip(&start)
            .filter(|(p, s)| r.sim.tank_pose(p.id).unwrap().0.distance(**s) > 5.0)
            .count();
        assert!(moved >= 4, "only {moved} bots moved");
        let snap = r.sim.snapshot();
        assert!(snap.tanks.iter().all(|t| t.pos.iter().all(|c| c.is_finite())));
    }

    /// A dropped human's held seat isn't counted in the room info, so the lobby
    /// and quick match skip the room; a rejoin counts again.
    #[test]
    fn held_seats_are_not_listed_humans() {
        let info = Arc::new(Mutex::new(ArenaInfo {
            code: "HELD".into(),
            public: true,
            humans: 0,
            max_humans: ROOM_SEATS,
            ended: false,
            mode: ModeKind::ScrapBrawl,
            map: MapId::Sink,
            party: false,
        }));
        let mut r = ArenaRoom::new("HELD".into(), true, 5, info.clone(), ModeKind::ScrapBrawl, MapId::Sink);
        let (tx, _rx_cmd) = mpsc::unbounded_channel();
        let rooms = ArenaRooms::default();
        rooms.rooms.lock().unwrap().insert("HELD".into(), (tx, info.clone()));
        let (out, mut rx) = mpsc::channel(4096);
        r.handle(ArenaCmd::Join { conn: 1, name: "Ann".into(), rejoin: None, out });
        assert_eq!(info.lock().unwrap().humans, 1);
        assert_eq!(rooms.list().len(), 1);
        let ServerMsg::Welcome { token, .. } = arena_core::net::decode::<ServerMsg>(&rx.try_recv().unwrap().bytes).unwrap() else { panic!() };
        r.handle(ArenaCmd::Leave { conn: 1 });
        r.tick();
        assert_eq!(r.humans.len(), 1, "the seat is still held");
        assert_eq!(info.lock().unwrap().humans, 0);
        assert!(rooms.list().is_empty());
        assert!(rooms.quick_match().is_none());
        let (out, _rx) = mpsc::channel(4096);
        r.handle(ArenaCmd::Join { conn: 2, name: "Ann".into(), rejoin: Some(token), out });
        assert_eq!(info.lock().unwrap().humans, 1);
        assert!(rooms.quick_match().is_some());
    }

    #[test]
    fn humans_take_bot_seats_and_rejoin() {
        let mut r = room(false);
        let (out, mut rx) = mpsc::channel(4096);
        r.handle(ArenaCmd::Join { conn: 1, name: "Ann".into(), rejoin: None, out });
        assert_eq!(r.sim.players.len(), ROOM_SEATS);
        let first = arena_core::net::decode::<ServerMsg>(&rx.try_recv().unwrap().bytes).unwrap();
        let ServerMsg::Welcome { you, token, .. } = first else { panic!("{first:?}") };
        let me = r.sim.players.iter().find(|p| p.id == you).unwrap();
        assert!(me.brain.is_none() && me.name == "Ann");

        r.handle(ArenaCmd::Input { conn: 1, seq: 1, input: Input { throttle: 1.0, ..Input::default() } });
        r.tick();
        r.tick();
        assert_eq!(r.humans[0].ack, 1);

        // Drop: a bot drives the tank; rejoin takes it back.
        r.handle(ArenaCmd::Leave { conn: 1 });
        assert!(r.sim.players.iter().find(|p| p.id == you).unwrap().brain.is_some());
        let (out, mut rx) = mpsc::channel(4096);
        r.handle(ArenaCmd::Join { conn: 2, name: "Ann".into(), rejoin: Some(token), out });
        let again = arena_core::net::decode::<ServerMsg>(&rx.try_recv().unwrap().bytes).unwrap();
        assert!(matches!(again, ServerMsg::Welcome { you: y, .. } if y == you));
        assert!(r.sim.players.iter().find(|p| p.id == you).unwrap().brain.is_none());
        assert_eq!(r.sim.players.len(), ROOM_SEATS);

        // Eight humans fill the room; a ninth is turned away.
        let mut keep = Vec::new();
        for c in 3..10 {
            let (out, rx) = mpsc::channel(4096);
            keep.push(rx);
            r.handle(ArenaCmd::Join { conn: c, name: "Ann".into(), rejoin: None, out });
        }
        assert_eq!(r.humans.len(), ROOM_SEATS);
        assert_eq!(r.sim.players.len(), ROOM_SEATS);
        assert!(r.sim.players.iter().all(|p| p.brain.is_none()));
        let (out, mut rx) = mpsc::channel(16);
        r.handle(ArenaCmd::Join { conn: 10, name: "Late".into(), rejoin: None, out });
        let msg = arena_core::net::decode::<ServerMsg>(&rx.try_recv().unwrap().bytes).unwrap();
        assert!(matches!(msg, ServerMsg::Rejected(_)));
    }

    // -------------------------------------------------------------- party

    use arena_core::net::{decode, BotSkill};

    fn party_room(settings: RoomSettings) -> (ArenaRoom, Arc<Mutex<ArenaInfo>>) {
        let info = Arc::new(Mutex::new(ArenaInfo {
            code: "PRTY".into(),
            public: false,
            humans: 0,
            max_humans: ROOM_SEATS,
            ended: false,
            mode: settings.mode,
            map: settings.map,
            party: true,
        }));
        (ArenaRoom::new_party("PRTY".into(), false, 21, info.clone(), settings), info)
    }

    /// A crew's socket in a test: everything the room sent it.
    struct Sock {
        conn: u64,
        rx: mpsc::Receiver<Frame>,
        lobby: Option<LobbyState>,
        you: Option<PlayerId>,
        map: Option<MapId>,
        welcomes: u32,
        infos: Vec<String>,
    }

    impl Sock {
        fn join(r: &mut ArenaRoom, conn: u64, name: &str) -> Sock {
            let (out, rx) = mpsc::channel(1 << 16);
            r.handle(ArenaCmd::Join { conn, name: name.into(), rejoin: None, out });
            let mut s = Sock { conn, rx, lobby: None, you: None, map: None, welcomes: 0, infos: Vec::new() };
            s.read();
            s
        }
        fn read(&mut self) {
            while let Ok(f) = self.rx.try_recv() {
                match decode::<ServerMsg>(&f.bytes).unwrap() {
                    ServerMsg::Lobby(l) => self.lobby = Some(l),
                    ServerMsg::Welcome { you, map, .. } => {
                        self.you = Some(you);
                        self.map = Some(map);
                        self.welcomes += 1;
                    }
                    ServerMsg::Info(t) => self.infos.push(t),
                    _ => {}
                }
            }
        }
        fn lobby(&mut self) -> LobbyState {
            self.read();
            self.lobby.clone().expect("a lobby state")
        }
        fn send(&self, r: &mut ArenaRoom, cmd: LobbyCmd) {
            r.handle(ArenaCmd::Lobby { conn: self.conn, cmd });
        }
    }

    fn ticks(r: &mut ArenaRoom, n: u32) {
        for _ in 0..n {
            r.tick();
        }
    }

    /// Lobby (warm-up, no bots, no scoring) -> the host starts once everyone's
    /// ready -> the match with bots and the host's clock -> the end screen ->
    /// back in the lobby with the same crews and a map vote -> the majority's
    /// map.
    #[test]
    fn party_lobby_start_end_vote_lobby() {
        let settings = RoomSettings { match_secs: 20, skill: BotSkill::Hard, ..RoomSettings::default() };
        let (mut r, info) = party_room(settings);
        let mut ann = Sock::join(&mut r, 1, "Ann");
        let mut bo = Sock::join(&mut r, 2, "Bo");
        let mut cy = Sock::join(&mut r, 3, "Cy");
        assert!(info.lock().unwrap().party);
        // Warm-up: only the crews, no bots.
        assert_eq!(r.sim.players.len(), 3);
        assert!(r.sim.players.iter().all(|p| p.brain.is_none()));
        let l = ann.lobby();
        assert_eq!(l.phase, LobbyPhase::Lobby);
        assert_eq!(l.crew.len(), 3);
        assert!(l.crew[0].host && !l.crew[1].host && !l.crew[2].host, "the first in hosts");
        assert_eq!(cy.lobby().crew.len(), 3, "everyone sees the whole crew");
        assert!(!l.can_start(), "Bo and Cy aren't ready");

        // No scoring in the warm-up.
        r.sim.players[1].kills = 3;
        r.tick();
        assert!(r.sim.players.iter().all(|p| p.kills == 0));

        // Only the host changes the settings, and everyone sees them.
        let mut wanted = settings;
        wanted.bots = false;
        bo.send(&mut r, LobbyCmd::Settings(wanted));
        assert!(cy.lobby().settings.bots, "Bo is not the host");
        wanted.bots = true;
        wanted.map = MapId::Refinery;
        ann.send(&mut r, LobbyCmd::Settings(wanted));
        assert_eq!(cy.lobby().settings.map, MapId::Refinery);
        assert_eq!(r.sim.map.id, MapId::Refinery, "the warm-up moves to the new map");
        bo.read();
        assert_eq!(bo.map, Some(MapId::Refinery), "with a fresh Welcome");

        // Start before everyone's ready does nothing; Bo isn't the host.
        ann.send(&mut r, LobbyCmd::Start);
        assert_eq!(ann.lobby().phase, LobbyPhase::Lobby);
        bo.send(&mut r, LobbyCmd::Ready(true));
        cy.send(&mut r, LobbyCmd::Ready(true));
        bo.send(&mut r, LobbyCmd::Start);
        assert_eq!(ann.lobby().phase, LobbyPhase::Lobby);
        assert!(ann.lobby().can_start());
        bo.read();
        let before = bo.welcomes;
        ann.send(&mut r, LobbyCmd::Start);
        assert_eq!(bo.lobby().phase, LobbyPhase::Playing);
        assert_eq!(bo.welcomes, before + 1, "the match world comes with a Welcome");
        assert_eq!(r.sim.players.len(), ROOM_SEATS, "bots fill the seats");
        assert_eq!(r.sim.map.id, MapId::Refinery);
        assert!((r.sim.time_left - 20.0).abs() < 0.1, "the host's clock: {}", r.sim.time_left);
        let me = r.sim.players.iter().find(|p| Some(p.id) == ann.you).unwrap();
        assert!(me.brain.is_none() && me.name == "Ann");

        // The clock runs out -> the end screen -> the lobby with a vote.
        ticks(&mut r, 21 * TICK_HZ);
        assert!(!matches!(r.sim.phase, Phase::Playing));
        assert_eq!(ann.lobby().phase, LobbyPhase::Ended);
        ticks(&mut r, END_TICKS + 1);
        let l = cy.lobby();
        assert_eq!(l.phase, LobbyPhase::Lobby);
        assert_eq!(l.crew.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Ann", "Bo", "Cy"], "the same crews");
        assert!(l.crew.iter().all(|c| !c.ready), "ready again for the rematch");
        assert_eq!(r.sim.players.len(), 3, "back to the warm-up");
        let vote = l.vote.clone().expect("the map vote");
        assert_eq!(vote.cards.len(), 3);
        assert_eq!(vote.cards[0].map, MapId::Refinery, "card 0 is the rematch");
        assert!(!l.can_start(), "no start while the vote is open");

        // Two vote for the Rust Belt, the host for the Sink: the majority wins.
        let pick = |m: MapId| vote.cards.iter().position(|c| c.map == m).unwrap() as u8;
        ann.send(&mut r, LobbyCmd::Vote(pick(MapId::Sink)));
        bo.send(&mut r, LobbyCmd::Vote(pick(MapId::RustBelt)));
        assert!(cy.lobby().vote.is_some(), "still open");
        assert_eq!(cy.lobby().crew[1].vote, Some(pick(MapId::RustBelt)), "votes are shown");
        cy.send(&mut r, LobbyCmd::Vote(pick(MapId::RustBelt)));
        r.tick();
        let l = ann.lobby();
        assert!(l.vote.is_none(), "everyone voted: closed early");
        assert_eq!(l.settings.map, MapId::RustBelt);
        assert_eq!(r.sim.map.id, MapId::RustBelt);
        assert_eq!(ann.map, Some(MapId::RustBelt));
        assert!(ann.infos.iter().any(|t| t.contains("Rust Belt")), "{:?}", ann.infos);
    }

    /// A tie goes to the host's card; nobody voting at all keeps the map.
    #[test]
    fn party_vote_tie_goes_to_host_and_times_out() {
        let (mut r, _) = party_room(RoomSettings { match_secs: 20, bots: false, ..RoomSettings::default() });
        let mut ann = Sock::join(&mut r, 1, "Ann");
        let bo = Sock::join(&mut r, 2, "Bo");
        bo.send(&mut r, LobbyCmd::Ready(true));
        ann.send(&mut r, LobbyCmd::Start);
        assert_eq!(r.sim.players.len(), 2, "bots off: just the crews");
        ticks(&mut r, 21 * TICK_HZ + END_TICKS + 1);
        let vote = ann.lobby().vote.expect("vote");
        bo.send(&mut r, LobbyCmd::Vote(1));
        ann.send(&mut r, LobbyCmd::Vote(2));
        r.tick();
        assert_eq!(ann.lobby().settings.map, vote.cards[2].map, "1 v 1: the host's pick");

        // Next round nobody votes: 15 s, then the rematch card.
        bo.send(&mut r, LobbyCmd::Ready(true));
        ann.send(&mut r, LobbyCmd::Start);
        ticks(&mut r, 21 * TICK_HZ + END_TICKS + 1);
        let map = ann.lobby().settings.map;
        assert!(ann.lobby().vote.as_ref().unwrap().secs_left >= 13);
        ticks(&mut r, VOTE_TICKS - 2 * TICK_HZ);
        assert!(ann.lobby().vote.is_some());
        ticks(&mut r, 2 * TICK_HZ + 1);
        assert!(ann.lobby().vote.is_none());
        assert_eq!(ann.lobby().settings.map, map);
    }

    /// Start waits for everyone's ready tick, or 10 s for "start anyway".
    #[test]
    fn party_ready_gating_and_start_anyway() {
        let (mut r, _) = party_room(RoomSettings::default());
        let mut ann = Sock::join(&mut r, 1, "Ann");
        let mut bo = Sock::join(&mut r, 2, "Bo");
        let l = bo.lobby();
        assert!(!l.can_start());
        assert_eq!(l.force_in, 10);
        ann.send(&mut r, LobbyCmd::Start);
        assert_eq!(ann.lobby().phase, LobbyPhase::Lobby);
        // Ready, then not: still gated.
        bo.send(&mut r, LobbyCmd::Ready(true));
        assert!(ann.lobby().can_start());
        bo.send(&mut r, LobbyCmd::Ready(false));
        assert!(!ann.lobby().can_start());
        ticks(&mut r, FORCE_TICKS - TICK_HZ);
        assert_eq!(ann.lobby().force_in, 1);
        ann.send(&mut r, LobbyCmd::Start);
        assert_eq!(ann.lobby().phase, LobbyPhase::Lobby);
        ticks(&mut r, TICK_HZ);
        let l = ann.lobby();
        assert_eq!(l.force_in, 0);
        assert!(l.can_start(), "start anyway");
        ann.send(&mut r, LobbyCmd::Start);
        assert_eq!(bo.lobby().phase, LobbyPhase::Playing);
        // Lobby moves don't work mid-match.
        bo.send(&mut r, LobbyCmd::Ready(true));
        assert!(!bo.lobby().crew[1].ready);
    }

    /// The host leaves: the next human hosts, and only they change things.
    /// A dropped crew in the lobby idles (no bot), and the held seat goes
    /// with its tank when the hold runs out.
    #[test]
    fn party_host_migration() {
        let (mut r, _) = party_room(RoomSettings::default());
        let mut ann = Sock::join(&mut r, 1, "Ann");
        let mut bo = Sock::join(&mut r, 2, "Bo");
        let mut cy = Sock::join(&mut r, 3, "Cy");
        assert!(bo.lobby().crew[0].host);
        let ann_id = ann.you.unwrap();
        r.handle(ArenaCmd::Leave { conn: 1 });
        let l = cy.lobby();
        assert!(l.crew[1].host && !l.crew[0].host, "Bo hosts now: {:?}", l.crew);
        assert!(l.crew[0].away);
        assert!(cy.infos.iter().any(|t| t == "Bo is the host now."), "{:?}", cy.infos);
        let tank = r.sim.players.iter().find(|p| p.id == ann_id).unwrap();
        assert!(tank.brain.is_none(), "no bot in the warm-up");
        // Bo's settings go through now.
        let s = RoomSettings { mode: ModeKind::TeamBrawl, ..RoomSettings::default() };
        bo.send(&mut r, LobbyCmd::Settings(s));
        assert_eq!(cy.lobby().settings.mode, ModeKind::TeamBrawl);
        // Team picks: Cy to the Saints, Bo auto.
        cy.send(&mut r, LobbyCmd::Team(TeamPick::DustSaints));
        assert_eq!(bo.lobby().crew[2].team, TeamPick::DustSaints);
        // The hold runs out: Ann's tank goes, the crew is two.
        ticks(&mut r, HOLD_TICKS + 2);
        assert_eq!(r.sim.players.len(), 2);
        assert_eq!(bo.lobby().crew.len(), 2);
        // Bo leaves too: Cy hosts and starts alone (everyone left is ready).
        r.handle(ArenaCmd::Leave { conn: 2 });
        let l = cy.lobby();
        assert!(l.crew.iter().find(|c| c.name == "Cy").unwrap().host);
        cy.send(&mut r, LobbyCmd::Start);
        assert_eq!(cy.lobby().phase, LobbyPhase::Playing);
        let me = r.sim.players.iter().find(|p| Some(p.id) == cy.you).unwrap();
        assert_eq!(me.team, Some(1), "Cy picked the Dust Saints");
        let bo_tank = r.sim.players.iter().find(|p| p.name == "Bo").unwrap();
        assert!(bo_tank.brain.is_some(), "a bot drives a dropped crew's tank in the match");
        assert_eq!(r.sim.players.len(), ROOM_SEATS);
        let side = |t: u8| r.sim.players.iter().filter(|p| p.team == Some(t)).count();
        assert_eq!((side(0), side(1)), (4, 4));
        let _ = ann.lobby();
    }

    /// Party rooms stay out of quick match; a Garage pick reaches the room.
    #[test]
    fn party_rooms_skip_quick_match_and_take_loadouts() {
        let (mut r, info) = party_room(RoomSettings::default());
        let rooms = ArenaRooms::default();
        let (tx, _rx) = mpsc::unbounded_channel();
        rooms.rooms.lock().unwrap().insert("PRTY".into(), (tx, info.clone()));
        let mut ann = Sock::join(&mut r, 1, "Ann");
        let mut wanted = info.lock().unwrap().clone();
        wanted.public = true;
        *info.lock().unwrap() = wanted;
        assert!(rooms.quick_match().is_none());
        assert_eq!(rooms.list().len(), 1, "but listed when public");
        let l = Loadout { chassis: arena_core::garage::ChassisClass::Hauler, paint: 5, emblem: 2, horn: 1 };
        ann.send(&mut r, LobbyCmd::Loadout(l));
        assert_eq!(ann.lobby().crew[0].loadout, l);
        let me = r.sim.players.iter().find(|p| Some(p.id) == ann.you).unwrap();
        assert_eq!(me.tank.class, arena_core::garage::ChassisClass::Hauler, "changed on the spot in the warm-up");
    }
}
