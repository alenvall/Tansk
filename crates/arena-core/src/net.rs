//! The wire protocol between the arena server (`/ws/arena`) and the client.
//!
//! One bincode (varint) message per binary WebSocket frame, encoded with
//! [`encode`] and read with [`decode`]. Flow: the client sends [`ClientMsg::Hello`];
//! the server answers [`ServerMsg::Welcome`] (or `Rejected`, then closes),
//! then a [`ServerMsg::Players`] roster whenever it changes and a
//! [`ServerMsg::Tick`] at 30 Hz. The client sends one [`ClientMsg::Input`] per
//! simulation tick (60 Hz) with a rising `seq`; the server applies one per
//! tick and acks the last one applied in each `Tick`.
//!
//! The map is never sent: it is `Map::by_id(map, seed)` from the `Welcome`.
//!
//! Party rooms ([`Join::Party`]) add a lobby: the server also sends
//! [`ServerMsg::Lobby`] and takes [`ClientMsg::Lobby`] moves (ready, team,
//! the host's settings and Start, the map vote). Each time the room's world
//! is rebuilt (lobby warm-up, match, new map) every crew gets a fresh
//! `Welcome` (same code and token, a new `you` and maybe map). The lobby
//! messages are appended variants, so the older ones encode as before.
//!
//! The server reads `Hello` with [`decode_client`], which also accepts the
//! messages of clients from before maps were selectable.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::events::Event;
use crate::sim::PlayerInfo;
use crate::snapshot::Snapshot;
use crate::{Input, PlayerId};

/// Seats in a room: humans plus bots.
pub const ROOM_SEATS: usize = 8;
/// Snapshots per second (every other tick).
pub const SNAPSHOT_HZ: u32 = 30;
/// Largest message a client may send, bytes.
pub const MAX_CLIENT_MSG: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Join {
    /// Quick match: the fullest open public room, or a new one.
    Play,
    /// A new room; `public` rooms also take quick-match players (Scrap
    /// Brawl only). `mode` picks the game (Raider Waves is co-op PvE).
    Host {
        public: bool,
        #[serde(default)]
        mode: crate::raid::ModeKind,
        /// Which map (added with The Refinery; older clients host the Sink).
        #[serde(default)]
        map: crate::map::MapId,
    },
    /// A room by its 4-letter code.
    Code(String),
    /// Back into the seat held after a disconnect (from `Welcome`).
    Rejoin { code: String, token: u64 },
    /// A new room with a pre-match lobby (the "play with friends" flow):
    /// everyone warms up together until the host starts; after each match
    /// the crews come back to the lobby and vote on the next map.
    Party { public: bool, settings: RoomSettings },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello { name: String, join: Join },
    /// `seq` starts at 1 and rises by one per tick.
    Input { seq: u32, input: Input },
    Chat(String),
    /// Lobby moves (party rooms); `Loadout` works in any room.
    Lobby(LobbyCmd),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome {
        code: String,
        public: bool,
        you: PlayerId,
        /// Map and simulation seed: `Sim::new(seed, Map::by_id(map, seed), Tuning::default())`.
        seed: u64,
        /// For [`Join::Rejoin`] after a dropped connection.
        token: u64,
        /// The room's map (added with The Refinery, at the end of the message).
        #[serde(default)]
        map: crate::map::MapId,
    },
    Rejected(String),
    /// Everyone in the room (humans and bots), on every change.
    Players(Vec<PlayerInfo>),
    /// The world after server tick `snapshot.tick`, the events since the last
    /// `Tick`, and the last input `seq` of yours the server has applied.
    Tick { ack: u32, snapshot: Snapshot, events: Vec<Event> },
    Chat { from: String, text: String },
    /// A line for the chat box from the server ("X joined.").
    Info(String),
    /// A party room's lobby: who's in, who's ready, the host's settings and
    /// the map vote. Sent on every change (and each second while a
    /// countdown runs). A `Welcome` comes first whenever the room's world is
    /// rebuilt (match start, back to the lobby, a new map).
    Lobby(LobbyState),
}

// ------------------------------------------------------------------ lobby

/// Bot skill in a party room.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BotSkill {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl BotSkill {
    pub const ALL: [BotSkill; 3] = [BotSkill::Easy, BotSkill::Normal, BotSkill::Hard];

    /// `Brain` difficulty, 0..1.
    pub fn difficulty(self) -> f32 {
        match self {
            BotSkill::Easy => 0.25,
            BotSkill::Normal => 0.5,
            BotSkill::Hard => 0.85,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BotSkill::Easy => "Easy",
            BotSkill::Normal => "Normal",
            BotSkill::Hard => "Hard",
        }
    }
}

/// What the end-of-match vote offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VotePool {
    /// The three maps, same mode.
    #[default]
    Maps,
    /// Three mode + map combos.
    Mixed,
}

/// The host's rules for a party room.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSettings {
    pub mode: crate::raid::ModeKind,
    pub map: crate::map::MapId,
    /// Bots fill the empty seats.
    pub bots: bool,
    pub skill: BotSkill,
    /// Match length, seconds (the Scrap Brawl, Team Brawl and Wellhead clocks).
    pub match_secs: u16,
    pub vote: VotePool,
}

impl Default for RoomSettings {
    fn default() -> Self {
        RoomSettings {
            mode: crate::raid::ModeKind::ScrapBrawl,
            map: crate::map::MapId::Sink,
            bots: true,
            skill: BotSkill::Normal,
            match_secs: 7 * 60,
            vote: VotePool::Maps,
        }
    }
}

impl RoomSettings {
    pub const MIN_SECS: u16 = 20;
    pub const MAX_SECS: u16 = 30 * 60;

    /// In range (it came off the network).
    pub fn clamped(mut self) -> Self {
        self.match_secs = self.match_secs.clamp(Self::MIN_SECS, Self::MAX_SECS);
        self
    }
}

/// Which side a crew wants in a team mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TeamPick {
    /// Whichever side is short.
    #[default]
    Auto,
    Rustbacks,
    DustSaints,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LobbyPhase {
    /// Warming up: the host's settings, ready ticks, the vote.
    #[default]
    Lobby,
    Playing,
    /// The end screen; back to the lobby shortly.
    Ended,
}

/// One human in a party room.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crew {
    /// The player id in the room's current world.
    pub id: PlayerId,
    pub name: String,
    pub host: bool,
    pub ready: bool,
    pub team: TeamPick,
    pub loadout: crate::garage::Loadout,
    /// The card this crew voted for.
    pub vote: Option<u8>,
    /// Dropped; the seat is held for a rejoin.
    pub away: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteCard {
    pub mode: crate::raid::ModeKind,
    pub map: crate::map::MapId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vote {
    pub cards: Vec<VoteCard>,
    /// Seconds left.
    pub secs_left: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyState {
    pub phase: LobbyPhase,
    pub settings: RoomSettings,
    pub crew: Vec<Crew>,
    /// Seconds until the host may start without everyone ready (0: now).
    pub force_in: u8,
    /// The map vote, right after a match.
    pub vote: Option<Vote>,
}

impl LobbyState {
    pub fn host(&self) -> Option<&Crew> {
        self.crew.iter().find(|c| c.host)
    }

    /// Everyone here (not away) is ready; the host counts as ready.
    pub fn all_ready(&self) -> bool {
        self.crew.iter().filter(|c| !c.away).all(|c| c.ready || c.host)
    }

    /// The host's Start is live: no vote open, and everyone ready or the
    /// wait is over.
    pub fn can_start(&self) -> bool {
        self.phase == LobbyPhase::Lobby && self.vote.is_none() && (self.all_ready() || self.force_in == 0)
    }
}

/// What a crew does in the lobby.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LobbyCmd {
    Ready(bool),
    /// Host only.
    Settings(RoomSettings),
    Team(TeamPick),
    /// Host only: roll out.
    Start,
    /// A vote card, 0-based.
    Vote(u8),
    /// Your Garage pick (any room; a live tank changes when that's safe).
    Loadout(crate::garage::Loadout),
}

/// The winning card of a vote: most votes; a tie goes to the host's card
/// when the host voted for one of the tied ones, else the earliest card
/// (card 0 is the rematch). `votes` are the crews' picks, `host` the host's.
pub fn tally(cards: usize, votes: &[u8], host: Option<u8>) -> usize {
    if cards == 0 {
        return 0;
    }
    let mut count = vec![0usize; cards];
    for &v in votes {
        if let Some(c) = count.get_mut(v as usize) {
            *c += 1;
        }
    }
    let best = count.iter().copied().max().unwrap_or(0);
    if let Some(h) = host.map(usize::from).filter(|&h| h < cards && count[h] == best) {
        return h;
    }
    count.iter().position(|&c| c == best).unwrap_or(0)
}

fn opts() -> impl bincode::Options {
    use bincode::Options;
    bincode::DefaultOptions::new().with_varint_encoding().with_limit(1 << 20)
}

pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    use bincode::Options;
    opts().serialize(msg).expect("message serializes")
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    use bincode::Options;
    opts().deserialize(bytes).ok()
}

/// Reads a client message, also in the shape clients sent before maps were
/// selectable (`Join::Host` without `map`): those host the Sink.
pub fn decode_client(bytes: &[u8]) -> Option<ClientMsg> {
    decode::<ClientMsg>(bytes).or_else(|| decode::<legacy::ClientMsg>(bytes).map(Into::into))
}

/// The protocol as it was before `Join::Host { map }`.
mod legacy {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub enum Join {
        Play,
        Host { public: bool, mode: crate::raid::ModeKind },
        Code(String),
        Rejoin { code: String, token: u64 },
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub enum ClientMsg {
        Hello { name: String, join: Join },
        Input { seq: u32, input: crate::Input },
        Chat(String),
    }

    impl From<ClientMsg> for super::ClientMsg {
        fn from(m: ClientMsg) -> Self {
            match m {
                ClientMsg::Hello { name, join } => super::ClientMsg::Hello {
                    name,
                    join: match join {
                        Join::Play => super::Join::Play,
                        Join::Host { public, mode } => super::Join::Host { public, mode, map: crate::map::MapId::Sink },
                        Join::Code(c) => super::Join::Code(c),
                        Join::Rejoin { code, token } => super::Join::Rejoin { code, token },
                    },
                },
                ClientMsg::Input { seq, input } => super::ClientMsg::Input { seq, input },
                ClientMsg::Chat(t) => super::ClientMsg::Chat(t),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Map, Sim, Tuning};

    #[test]
    fn round_trip() {
        let mut sim = Sim::new(3, Map::the_sink(3), Tuning::default());
        sim.add_player("a");
        sim.add_bot("b", 0.5);
        sim.step();
        let msg = ServerMsg::Tick { ack: 7, snapshot: sim.snapshot(), events: sim.drain_events() };
        assert_eq!(decode::<ServerMsg>(&encode(&msg)), Some(msg));
        let c = ClientMsg::Hello { name: "x".into(), join: Join::Rejoin { code: "ABCD".into(), token: 9 } };
        assert_eq!(decode::<ClientMsg>(&encode(&c)), Some(c));
        assert_eq!(decode::<ClientMsg>(&[200, 1, 2]), None);
        assert_eq!(decode_client(&[200, 1, 2]), None);
        let h = ClientMsg::Hello { name: "x".into(), join: Join::Host { public: true, mode: crate::raid::ModeKind::TeamBrawl, map: crate::MapId::Refinery } };
        assert_eq!(decode_client(&encode(&h)), Some(h));
    }

    /// The party-room messages survive the wire, and the old messages keep
    /// their encodings (the new variants are appended).
    #[test]
    fn lobby_round_trip() {
        use crate::raid::ModeKind;
        let settings = RoomSettings {
            mode: ModeKind::Wellhead,
            map: crate::MapId::RustBelt,
            bots: false,
            skill: BotSkill::Hard,
            match_secs: 30,
            vote: VotePool::Mixed,
        };
        let hello = ClientMsg::Hello { name: "Ann".into(), join: Join::Party { public: false, settings } };
        assert_eq!(decode_client(&encode(&hello)), Some(hello));
        for cmd in [
            LobbyCmd::Ready(true),
            LobbyCmd::Settings(settings),
            LobbyCmd::Team(TeamPick::DustSaints),
            LobbyCmd::Start,
            LobbyCmd::Vote(2),
            LobbyCmd::Loadout(crate::garage::Loadout { chassis: crate::garage::ChassisClass::Scout, paint: 3, emblem: 1, horn: 0 }),
        ] {
            let m = ClientMsg::Lobby(cmd);
            assert_eq!(decode_client(&encode(&m)), Some(m));
        }
        let state = LobbyState {
            phase: LobbyPhase::Lobby,
            settings,
            crew: vec![
                Crew { id: 0, name: "Ann".into(), host: true, ready: false, team: TeamPick::Auto, loadout: Default::default(), vote: Some(1), away: false },
                Crew { id: 3, name: "Bo".into(), host: false, ready: true, team: TeamPick::Rustbacks, loadout: Default::default(), vote: None, away: true },
            ],
            force_in: 7,
            vote: Some(Vote { cards: vec![VoteCard { mode: ModeKind::ScrapBrawl, map: crate::MapId::Sink }; 3], secs_left: 12 }),
        };
        let m = ServerMsg::Lobby(state.clone());
        assert_eq!(decode::<ServerMsg>(&encode(&m)), Some(m));
        assert!(state.all_ready(), "the host and the away crew don't hold it up");
        assert!(!state.can_start(), "not while the vote is open");
        // Appended variants: the old ones keep their tags.
        assert_eq!(encode(&ClientMsg::Chat("x".into()))[0], 2);
        assert_eq!(encode(&ClientMsg::Lobby(LobbyCmd::Start))[0], 3);
        assert_eq!(encode(&ServerMsg::Info("x".into()))[0], 5);
        assert_eq!(encode(&ServerMsg::Lobby(state))[0], 6);
    }

    #[test]
    fn tally_majority_then_host() {
        assert_eq!(tally(3, &[2, 2, 1], Some(1)), 2, "majority wins");
        assert_eq!(tally(3, &[2, 1], Some(1)), 1, "a tie goes to the host");
        assert_eq!(tally(3, &[2, 1], None), 1, "a tie without the host: the earliest card");
        assert_eq!(tally(3, &[], None), 0, "nobody voted: the rematch");
        assert_eq!(tally(3, &[0, 2, 2], Some(0)), 2);
        assert_eq!(tally(3, &[9], None), 0, "bad picks don't count");
    }

    /// A Hello from a client that predates map choice still hosts (the Sink).
    #[test]
    fn old_host_hello_decodes() {
        let old = legacy::ClientMsg::Hello { name: "Ann".into(), join: legacy::Join::Host { public: false, mode: crate::raid::ModeKind::RaiderWaves } };
        let got = decode_client(&encode(&old)).expect("old hello decodes");
        assert_eq!(
            got,
            ClientMsg::Hello { name: "Ann".into(), join: Join::Host { public: false, mode: crate::raid::ModeKind::RaiderWaves, map: crate::MapId::Sink } }
        );
        let input = legacy::ClientMsg::Input { seq: 3, input: crate::Input::default() };
        assert!(matches!(decode_client(&encode(&input)), Some(ClientMsg::Input { seq: 3, .. })));
    }
}
