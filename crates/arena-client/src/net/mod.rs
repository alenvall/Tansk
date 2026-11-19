//! Netcode: online play against the arena server, or offline Practice,
//! behind one API so the game loop has a single path.
//!
//! ```ignore
//! let mut net = NetClient::connect(&net::transport::default_url(), "Crew", Join::Play);
//! // or: NetClient::practice(seed, 7, 0.5)
//! // every frame:
//! net.send_input(input);            // current controls
//! net.update(performance_now_s);    // fixed 60 Hz ticks, network, prediction
//! let s = net.state();              // s.world: the Snapshot to draw
//! for e in net.drain_events() { /* sound, juice, kill feed */ }
//! ```
//!
//! `state().world` is a render-ready [`Snapshot`]: your tank predicted
//! (moves the instant you press a key), other tanks, projectiles and props
//! interpolated [`interp::DELAY`] behind the server, everything else
//! (pickups, scores, mode, ...) as of that same moment, except scores and
//! mode which are the newest.

// The game loop is wired up separately; until it uses every accessor.
#![allow(dead_code)]

pub mod interp;
pub mod predict;
pub mod transport;

use std::collections::VecDeque;

pub use arena_core::net::Join;
use arena_core::net::{decode, encode, ClientMsg, ServerMsg};
use arena_core::snapshot::{Snapshot, TankSnap};
use arena_core::{Event, Input, MapId, PlayerId, PlayerInfo, Sim, DT};

use interp::{lerp_snapshot, Interp};
use predict::Predictor;
use transport::{Incoming, Transport};

/// Most local ticks run in one `update` (after a stall the game skips ahead).
const MAX_TICKS_PER_UPDATE: u32 = 8;
const MAX_CHAT_LINES: usize = 100;
const MAX_RETRIES: u32 = 5;

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Offline, against local bots.
    Practice,
    Connecting,
    /// In a room (see `NetState::code`).
    Playing,
    /// The connection dropped; rejoining the held seat.
    Reconnecting,
    /// Gone for good, with the reason to show.
    Closed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatLine {
    /// `None` for server notices ("X joined.").
    pub from: Option<String>,
    pub text: String,
}

/// What the game draws and shows, refreshed by every `NetClient::update`.
#[derive(Clone, Debug)]
pub struct NetState {
    pub status: Status,
    /// The room's 4-letter code (online).
    pub code: Option<String>,
    pub public: bool,
    pub you: Option<PlayerId>,
    /// Map seed: build the scenery with `Map::by_id(map, seed)`.
    pub seed: Option<u64>,
    /// Which map the room plays (with the seed).
    pub map: MapId,
    /// The world to draw this frame (see the module docs).
    pub world: Option<Snapshot>,
    /// Everyone in the match: names and who is a bot.
    pub players: Vec<PlayerInfo>,
    pub chat: Vec<ChatLine>,
    /// Round trip, ms (0 offline).
    pub rtt_ms: f32,
    /// Inputs sent but not yet applied by the server.
    pub unacked: usize,
}

impl NetState {
    fn new(status: Status) -> Self {
        NetState {
            status,
            code: None,
            public: false,
            you: None,
            seed: None,
            map: MapId::Sink,
            world: None,
            players: Vec::new(),
            chat: Vec::new(),
            rtt_ms: 0.0,
            unacked: 0,
        }
    }

    /// Your tank as drawn (predicted online).
    pub fn my_tank(&self) -> Option<&TankSnap> {
        self.world.as_ref()?.tank(self.you?)
    }

    pub fn name_of(&self, id: PlayerId) -> Option<&str> {
        self.players.iter().find(|p| p.id == id).map(|p| p.name.as_str())
    }

    fn push_chat(&mut self, from: Option<String>, text: String) {
        self.chat.push(ChatLine { from, text });
        if self.chat.len() > MAX_CHAT_LINES {
            self.chat.remove(0);
        }
    }
}

type Factory = Box<dyn FnMut(&str) -> Box<dyn Transport>>;

struct Online {
    url: String,
    name: String,
    join: Join,
    factory: Factory,
    transport: Option<Box<dyn Transport>>,
    /// (code, token) of the seat to rejoin after a drop.
    resume: Option<(String, u64)>,
    predictor: Option<Predictor>,
    interp: Interp,
    seq: u32,
    sent: VecDeque<(u32, f64)>,
    retries: u32,
    retry_at: Option<f64>,
    /// Others' events, released when the interpolated view reaches their tick.
    delayed: VecDeque<(u32, Event)>,
}

struct Practice {
    sim: Sim,
    me: PlayerId,
    prev: Snapshot,
    cur: Snapshot,
}

enum Mode {
    Online(Box<Online>),
    Practice(Box<Practice>),
}

pub struct NetClient {
    mode: Mode,
    state: NetState,
    input: Input,
    last_now: Option<f64>,
    acc: f64,
    events: Vec<Event>,
}

impl NetClient {
    /// Online: connects to `url` (see [`transport::default_url`]) and joins by `join`.
    pub fn connect(url: &str, name: &str, join: Join) -> NetClient {
        Self::with_transport(url, name, join, Box::new(|u: &str| transport::open(u)))
    }

    /// Online over any transport (tests use an in-memory one).
    pub fn with_transport(url: &str, name: &str, join: Join, mut factory: Factory) -> NetClient {
        let transport = factory(url);
        NetClient {
            mode: Mode::Online(Box::new(Online {
                url: url.to_string(),
                name: name.to_string(),
                join,
                factory,
                transport: Some(transport),
                resume: None,
                predictor: None,
                interp: Interp::default(),
                seq: 0,
                sent: VecDeque::new(),
                retries: 0,
                retry_at: None,
                delayed: VecDeque::new(),
            })),
            state: NetState::new(Status::Connecting),
            input: Input::default(),
            last_now: None,
            acc: 0.0,
            events: Vec::new(),
        }
    }

    /// Offline: you and `bots` bots (difficulty 0..1) on map `map`.
    pub fn practice(seed: u64, bots: usize, difficulty: f32, map: MapId) -> NetClient {
        let (sim, me) = Sim::practice_on(map, seed, bots, difficulty);
        Self::offline(sim, me, seed)
    }

    /// Offline Raider Waves: you and `allies` bot allies hold the outpost.
    pub fn practice_raid(seed: u64, allies: usize, difficulty: f32, map: MapId) -> NetClient {
        let (sim, me) = Sim::practice_raid_on(map, seed, allies, difficulty);
        Self::offline(sim, me, seed)
    }

    /// Offline Team Brawl: you and `per_team - 1` bot allies against
    /// `per_team` bots (the sim puts each newcomer on the smaller team).
    pub fn practice_team(seed: u64, per_team: usize, difficulty: f32, map: MapId) -> NetClient {
        let mut sim = Sim::new_team_brawl(seed, arena_core::Map::by_id(map, seed), arena_core::Tuning::default());
        let me = sim.add_player("You");
        for i in 0..per_team * 2 - 1 {
            let names = arena_core::bots::BOT_NAMES;
            sim.add_bot(names[i % names.len()], difficulty);
        }
        Self::offline(sim, me, seed)
    }

    /// Offline Convoy: you and `per_team - 1` bot allies (escorting first)
    /// against `per_team` bots; sides swap for round 2.
    pub fn practice_convoy(seed: u64, per_team: usize, difficulty: f32, map: MapId) -> NetClient {
        let (sim, me) = Sim::practice_convoy_on(map, seed, per_team, difficulty);
        Self::offline(sim, me, seed)
    }

    fn offline(mut sim: Sim, me: PlayerId, seed: u64) -> NetClient {

        let events = sim.drain_events();
        let snap = sim.snapshot();
        let mut state = NetState::new(Status::Practice);
        state.you = Some(me);
        state.seed = Some(seed);
        state.map = sim.map.id;
        state.players = sim.players();
        state.world = Some(snap.clone());
        NetClient {
            mode: Mode::Practice(Box::new(Practice { sim, me, prev: snap.clone(), cur: snap })),
            state,
            input: Input::default(),
            last_now: None,
            acc: 0.0,
            events,
        }
    }

    /// Offline only: the local sim, for debug hooks (short matches, free
    /// scrap). `None` online.
    pub fn practice_sim(&mut self) -> Option<&mut Sim> {
        match &mut self.mode {
            Mode::Practice(p) => Some(&mut p.sim),
            Mode::Online(_) => None,
        }
    }

    /// The controls to apply from now on (sent once per 60 Hz tick, tagged
    /// with a sequence number).
    pub fn send_input(&mut self, input: Input) {
        self.input = input;
    }

    pub fn chat(&mut self, text: &str) {
        match &mut self.mode {
            Mode::Online(o) => {
                if let Some(t) = &mut o.transport {
                    t.send(encode(&ClientMsg::Chat(text.to_string())));
                }
            }
            Mode::Practice(_) => self.state.push_chat(Some("You".into()), text.to_string()),
        }
    }

    /// Leave the room (drops the socket).
    pub fn close(&mut self) {
        if let Mode::Online(o) = &mut self.mode {
            o.transport = None;
            o.resume = None;
        }
        self.state.status = Status::Closed("Left the game.".into());
    }

    pub fn state(&self) -> &NetState {
        &self.state
    }

    /// Events since the last call: yours at once, others' in step with the
    /// interpolated view.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Call once per frame with a monotonic clock in seconds.
    pub fn update(&mut self, now: f64) {
        let dt = self.last_now.map_or(0.0, |l| (now - l).clamp(0.0, 0.25));
        self.last_now = Some(now);
        self.acc += dt;
        let mut ticks = 0;
        while self.acc >= DT as f64 {
            self.acc -= DT as f64;
            ticks += 1;
        }
        if ticks > MAX_TICKS_PER_UPDATE {
            ticks = MAX_TICKS_PER_UPDATE;
        }
        let alpha = (self.acc / DT as f64) as f32;
        match &mut self.mode {
            Mode::Practice(p) => {
                for _ in 0..ticks {
                    p.sim.set_input(p.me, self.input);
                    p.sim.step();
                    self.events.extend(p.sim.drain_events());
                    p.prev = std::mem::replace(&mut p.cur, p.sim.snapshot());
                }
                if ticks > 0 {
                    self.state.players = p.sim.players();
                }
                self.state.world = Some(lerp_snapshot(&p.prev, &p.cur, alpha));
            }
            Mode::Online(o) => {
                o.update(&mut self.state, &mut self.events, self.input, now, dt as f32, ticks, alpha)
            }
        }
    }
}

/// Events about your own actions, played at once rather than 100 ms late.
fn own_event(e: &Event, you: PlayerId) -> bool {
    match e {
        Event::Fired { player, .. } | Event::Tracer { player, .. } | Event::BoostStarted { player } => *player == you,
        _ => false,
    }
}

impl Online {
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        state: &mut NetState,
        events: &mut Vec<Event>,
        input: Input,
        now: f64,
        dt: f32,
        ticks: u32,
        alpha: f32,
    ) {
        // Reconnect after a drop.
        if self.retry_at.is_some_and(|t| now >= t) {
            self.retry_at = None;
            self.retries += 1;
            self.transport = Some((self.factory)(&self.url));
        }

        let mut inbox = Vec::new();
        if let Some(t) = &mut self.transport {
            t.poll(&mut inbox);
        }
        for msg in inbox {
            self.receive(state, events, msg, now);
        }

        if state.status == Status::Playing {
            if let (Some(p), Some(t)) = (&mut self.predictor, &mut self.transport) {
                for _ in 0..ticks {
                    self.seq += 1;
                    t.send(encode(&ClientMsg::Input { seq: self.seq, input }));
                    self.sent.push_back((self.seq, now));
                    if self.sent.len() > 240 {
                        self.sent.pop_front();
                    }
                    p.tick(self.seq, input);
                }
                p.decay(dt);
                state.unacked = p.unacked();
            }
        }

        // The view: others interpolated, you predicted.
        if let Some(mut world) = self.interp.sample(now) {
            if let Some(latest) = self.interp.latest() {
                world.scores = latest.scores.clone();
                world.mode = latest.mode.clone();
                if let Some((p, server_me)) = self.predictor.as_ref().and_then(|p| Some((p, latest.tank(p.me)?))) {
                    let mine = p.view(server_me, alpha);
                    match world.tanks.iter_mut().find(|t| t.player == mine.player) {
                        Some(t) => *t = mine,
                        None => world.tanks.push(mine),
                    }
                }
            }
            state.world = Some(world);
        }
        if let Some(rt) = self.interp.render_tick(now) {
            while self.delayed.front().is_some_and(|(t, _)| *t as f64 <= rt) {
                events.push(self.delayed.pop_front().unwrap().1);
            }
        }
    }

    fn receive(&mut self, state: &mut NetState, events: &mut Vec<Event>, msg: Incoming, now: f64) {
        match msg {
            Incoming::Open => {
                let join = match &self.resume {
                    Some((code, token)) => Join::Rejoin { code: code.clone(), token: *token },
                    None => self.join.clone(),
                };
                if let Some(t) = &mut self.transport {
                    t.send(encode(&ClientMsg::Hello { name: self.name.clone(), join }));
                }
            }
            Incoming::Closed(why) => {
                self.transport = None;
                let can_retry = self.resume.is_some()
                    && matches!(state.status, Status::Playing | Status::Reconnecting)
                    && self.retries < MAX_RETRIES;
                if can_retry {
                    state.status = Status::Reconnecting;
                    self.retry_at = Some(now + 0.5 + self.retries as f64);
                } else if !matches!(state.status, Status::Closed(_)) {
                    state.status = Status::Closed(why);
                }
            }
            Incoming::Message(bytes) => {
                let Some(msg) = decode::<ServerMsg>(&bytes) else { return };
                match msg {
                    ServerMsg::Welcome { code, public, you, seed, token, map } => {
                        self.resume = Some((code.clone(), token));
                        self.retries = 0;
                        self.predictor = Some(Predictor::new(seed, you, map));
                        state.map = map;
                        self.interp.clear();
                        self.sent.clear();
                        self.delayed.clear();
                        state.status = Status::Playing;
                        state.code = Some(code);
                        state.public = public;
                        state.you = Some(you);
                        state.seed = Some(seed);
                    }
                    ServerMsg::Rejected(why) => {
                        self.resume = None;
                        state.status = Status::Closed(why);
                    }
                    ServerMsg::Players(p) => state.players = p,
                    ServerMsg::Tick { ack, snapshot, events: evs } => {
                        while self.sent.front().is_some_and(|(s, _)| *s < ack) {
                            self.sent.pop_front();
                        }
                        if let Some(&(s, at)) = self.sent.front() {
                            if s == ack {
                                let sample = ((now - at) * 1000.0) as f32;
                                state.rtt_ms = if state.rtt_ms == 0.0 { sample } else { state.rtt_ms * 0.9 + sample * 0.1 };
                            }
                        }
                        if let Some(p) = &mut self.predictor {
                            p.reconcile(&snapshot, ack);
                        }
                        let you = state.you;
                        for e in evs {
                            if you.is_some_and(|y| own_event(&e, y)) {
                                events.push(e);
                            } else {
                                self.delayed.push_back((snapshot.tick, e));
                            }
                        }
                        self.interp.push(snapshot, now);
                    }
                    ServerMsg::Chat { from, text } => state.push_chat(Some(from), text),
                    ServerMsg::Info(text) => state.push_chat(None, text),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
