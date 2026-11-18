//! The games a room can run: the 2D game at `/ws` and the 3D one at `/ws3d`.
//! Each maps its own wire messages to and from the room's common ones.

use serde::de::DeserializeOwned;
use serde::Serialize;
use tansk_sim::proto::{ClientMsg, MsgLevel, Phase, ServerMsg, Settings};
use tansk_sim::{Game, Input, V2};
use tansk3d_sim::proto::{ClientMsg3d, Input3d, ServerMsg3d, MAX_PITCH, MIN_PITCH};
use tansk3d_sim::Game3d;

/// What a client asks for, whichever game it plays.
pub enum Incoming<I> {
    Create { name: String, settings: Settings },
    Join { code: String, name: String },
    Input(I),
    Chat(String),
    Start,
    Restart,
}

/// What the room tells a client, other than views.
pub enum Outgoing {
    Welcome { code: String, you: u32, host: bool, settings: Settings },
    Rejected(String),
    Message { text: String, level: MsgLevel },
    Host(bool),
    Kicked,
}

pub trait RoomGame: Send + 'static {
    type Input: Send + 'static;
    type ClientMsg: DeserializeOwned + Send + 'static;
    type ServerMsg: Serialize + Clone + Send + 'static;
    const MAX_PLAYERS: usize;
    const TICK_MS: i32;

    fn new(settings: Settings, seed: u64) -> Self;
    fn add_player(&mut self, name: &str, color: u8) -> u32;
    fn remove_player(&mut self, id: u32);
    /// Inputs come from the network: implementations sanitize them.
    fn set_input(&mut self, id: u32, input: Self::Input);
    fn start(&mut self);
    fn step(&mut self);
    fn phase(&self) -> Phase;
    fn is_game_over(&self) -> bool;
    fn started(&self) -> bool;
    fn view_msg(&mut self) -> Self::ServerMsg;

    fn incoming(msg: Self::ClientMsg) -> Incoming<Self::Input>;
    fn outgoing(msg: Outgoing) -> Self::ServerMsg;
    /// Kicked and Rejected end the connection once sent.
    fn is_final(msg: &Self::ServerMsg) -> bool;
}

impl RoomGame for Game {
    type Input = Input;
    type ClientMsg = ClientMsg;
    type ServerMsg = ServerMsg;
    const MAX_PLAYERS: usize = tansk_sim::MAX_PLAYERS;
    const TICK_MS: i32 = tansk_sim::TICK_MS;

    fn new(settings: Settings, seed: u64) -> Self {
        Game::new(settings, seed)
    }
    fn add_player(&mut self, name: &str, color: u8) -> u32 {
        Game::add_player(self, name, color)
    }
    fn remove_player(&mut self, id: u32) {
        Game::remove_player(self, id)
    }
    fn set_input(&mut self, id: u32, mut i: Input) {
        // Keep the aim point finite.
        if !i.aim.x.is_finite() || !i.aim.y.is_finite() {
            i.aim = V2::new(512.0, 384.0);
        }
        i.aim.x = i.aim.x.clamp(-2000.0, 3000.0);
        i.aim.y = i.aim.y.clamp(-2000.0, 3000.0);
        Game::set_input(self, id, i)
    }
    fn start(&mut self) {
        Game::start(self)
    }
    fn step(&mut self) {
        Game::step(self)
    }
    fn phase(&self) -> Phase {
        Game::phase(self)
    }
    fn is_game_over(&self) -> bool {
        Game::is_game_over(self)
    }
    fn started(&self) -> bool {
        self.started
    }
    fn view_msg(&mut self) -> ServerMsg {
        ServerMsg::View(self.view())
    }

    fn incoming(msg: ClientMsg) -> Incoming<Input> {
        match msg {
            ClientMsg::Create { name, settings } => Incoming::Create { name, settings },
            ClientMsg::Join { code, name } => Incoming::Join { code, name },
            ClientMsg::Input(i) => Incoming::Input(i),
            ClientMsg::Chat(t) => Incoming::Chat(t),
            ClientMsg::Start => Incoming::Start,
            ClientMsg::Restart => Incoming::Restart,
        }
    }
    fn outgoing(msg: Outgoing) -> ServerMsg {
        match msg {
            Outgoing::Welcome { code, you, host, settings } => ServerMsg::Welcome { code, you, host, settings },
            Outgoing::Rejected(t) => ServerMsg::Rejected(t),
            Outgoing::Message { text, level } => ServerMsg::Message { text, level },
            Outgoing::Host(h) => ServerMsg::Host(h),
            Outgoing::Kicked => ServerMsg::Kicked,
        }
    }
    fn is_final(msg: &ServerMsg) -> bool {
        matches!(msg, ServerMsg::Kicked | ServerMsg::Rejected(_))
    }
}

impl RoomGame for Game3d {
    type Input = Input3d;
    type ClientMsg = ClientMsg3d;
    type ServerMsg = ServerMsg3d;
    const MAX_PLAYERS: usize = tansk3d_sim::MAX_PLAYERS;
    const TICK_MS: i32 = tansk3d_sim::TICK_MS;

    fn new(settings: Settings, seed: u64) -> Self {
        Game3d::new(settings, seed)
    }
    fn add_player(&mut self, name: &str, color: u8) -> u32 {
        Game3d::add_player(self, name, color)
    }
    fn remove_player(&mut self, id: u32) {
        Game3d::remove_player(self, id)
    }
    fn set_input(&mut self, id: u32, mut i: Input3d) {
        // Keep the aim angles finite and in range.
        i.aim_yaw = if i.aim_yaw.is_finite() { i.aim_yaw.rem_euclid(360.0) } else { 0.0 };
        i.aim_pitch = if i.aim_pitch.is_finite() { i.aim_pitch.clamp(MIN_PITCH, MAX_PITCH) } else { 0.0 };
        Game3d::set_input(self, id, i)
    }
    fn start(&mut self) {
        Game3d::start(self)
    }
    fn step(&mut self) {
        Game3d::step(self)
    }
    fn phase(&self) -> Phase {
        Game3d::phase(self)
    }
    fn is_game_over(&self) -> bool {
        Game3d::is_game_over(self)
    }
    fn started(&self) -> bool {
        self.started
    }
    fn view_msg(&mut self) -> ServerMsg3d {
        ServerMsg3d::View(self.view())
    }

    fn incoming(msg: ClientMsg3d) -> Incoming<Input3d> {
        match msg {
            ClientMsg3d::Create { name, settings } => Incoming::Create { name, settings },
            ClientMsg3d::Join { code, name } => Incoming::Join { code, name },
            ClientMsg3d::Input(i) => Incoming::Input(i),
            ClientMsg3d::Chat(t) => Incoming::Chat(t),
            ClientMsg3d::Start => Incoming::Start,
            ClientMsg3d::Restart => Incoming::Restart,
        }
    }
    fn outgoing(msg: Outgoing) -> ServerMsg3d {
        match msg {
            Outgoing::Welcome { code, you, host, settings } => ServerMsg3d::Welcome { code, you, host, settings },
            Outgoing::Rejected(t) => ServerMsg3d::Rejected(t),
            Outgoing::Message { text, level } => ServerMsg3d::Message { text, level },
            Outgoing::Host(h) => ServerMsg3d::Host(h),
            Outgoing::Kicked => ServerMsg3d::Kicked,
        }
    }
    fn is_final(msg: &ServerMsg3d) -> bool {
        matches!(msg, ServerMsg3d::Kicked | ServerMsg3d::Rejected(_))
    }
}
