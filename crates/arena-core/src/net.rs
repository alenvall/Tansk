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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello { name: String, join: Join },
    /// `seq` starts at 1 and rises by one per tick.
    Input { seq: u32, input: Input },
    Chat(String),
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
