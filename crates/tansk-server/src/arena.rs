//! TANSK 2026 rooms: an authoritative `arena_core::Sim` at 60 Hz, snapshots
//! and events to every crew at 30 Hz, bots in every empty seat.
//!
//! Seats: always [`ROOM_SEATS`] tanks. A joining human takes a bot's seat; a
//! human who drops keeps their tank (a bot drives it) for [`HOLD_TICKS`] so
//! they can rejoin with their token, then it becomes an ordinary bot.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arena_core::bots::{Brain, BOT_NAMES};
use arena_core::net::{encode, ServerMsg, ROOM_SEATS};
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
        };
        room.fill_bots();
        room.sim.drain_events();
        room
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
        }
        self.publish_info();
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
            if self.humans.iter().filter(|h| h.conn.is_some()).count() >= self.seats() {
                send(&out, &ServerMsg::Rejected("That game is full.".into()));
                return;
            }
            self.free_seat();
            let name = self.unique_name(&name);
            let player = self.sim.add_player(&name);
            let token = self.random();
            self.broadcast(&ServerMsg::Info(format!("{name} joined.")));
            self.humans.push(Human {
                player,
                name,
                token,
                conn: Some(conn),
                out: Some(out),
                queue: VecDeque::new(),
                input: Input::default(),
                ack: 0,
                away: 0,
            });
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
        let (id, name) = (h.player, h.name.clone());
        let seed = self.random();
        if let Some(p) = self.sim.players.iter_mut().find(|p| p.id == id) {
            p.brain = Some(Brain::new(seed, BOT_DIFFICULTY));
        }
        self.broadcast(&ServerMsg::Info(format!("{name} left.")));
        self.broadcast(&ServerMsg::Players(self.sim.players()));
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
                let name = self.bot_name();
                if let Some(p) = self.sim.players.iter_mut().find(|p| p.id == h.player) {
                    p.name = name;
                }
                roster_changed = true;
            } else {
                i += 1;
            }
        }

        self.sim.step();
        self.events.extend(self.sim.drain_events());
        if matches!(self.sim.phase, Phase::Ended { .. } | Phase::TeamWon { .. }) {
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
        }

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
            RAID_SEATS
        } else {
            ROOM_SEATS
        }
    }

    fn fill_bots(&mut self) {
        while self.seats_used() < self.seats() {
            let name = self.bot_name();
            self.sim.add_bot(&name, BOT_DIFFICULTY);
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
    }
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
                !tx.is_closed() && i.public && i.mode == ModeKind::ScrapBrawl && i.humans > 0 && i.humans < i.max_humans
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
}
