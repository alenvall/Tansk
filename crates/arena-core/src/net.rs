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
//! The map is never sent: it is `Map::the_sink(seed)` from the `Welcome`.

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
        /// Map and simulation seed: `Sim::new(seed, Map::the_sink(seed), Tuning::default())`.
        seed: u64,
        /// For [`Join::Rejoin`] after a dropped connection.
        token: u64,
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
    }
}
