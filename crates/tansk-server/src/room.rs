//! One room: a game (2D or 3D, see [`RoomGame`]) stepped every tick, its
//! players' sockets, its chat. The original's ServerState, minus the window.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::games::{Incoming, Outgoing, RoomGame};
use tansk_sim::proto::{MsgLevel, Phase, Settings};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

pub type Outbox<G> = mpsc::Sender<<G as RoomGame>::ServerMsg>;

pub enum RoomCmd<G: RoomGame> {
    Join { conn: u64, name: String, out: Outbox<G> },
    Leave { conn: u64 },
    Msg { conn: u64, msg: Incoming<G::Input> },
}

/// What the lobby list shows about a room.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RoomInfo {
    pub code: String,
    pub host: String,
    pub koth: bool,
    pub players: usize,
    pub max_players: usize,
    pub started: bool,
}

struct Member<G: RoomGame> {
    conn: u64,
    player: u32,
    name: String,
    color: u8,
    out: Outbox<G>,
}

impl<G: RoomGame> Member<G> {
    fn send(&self, msg: Outgoing) {
        let _ = self.out.try_send(G::outgoing(msg));
    }
}

pub struct Room<G: RoomGame> {
    code: String,
    settings: Settings,
    game: G,
    members: Vec<Member<G>>,
    info: Arc<Mutex<RoomInfo>>,
    seed: u64,
    announced_game_over: bool,
}

/// The four colours of the original's server pool.
const COLOR_POOL: [u8; 4] = [0, 1, 2, 3];
const MAX_NAME: usize = 10;
const MAX_CHAT: usize = 60;

impl<G: RoomGame> Room<G> {
    pub fn new(code: String, settings: Settings, seed: u64, info: Arc<Mutex<RoomInfo>>) -> Self {
        Room {
            game: G::new(settings, seed),
            settings: settings.clamped(),
            code,
            members: Vec::new(),
            info,
            seed,
            announced_game_over: false,
        }
    }

    /// Runs the room until its last player leaves.
    pub async fn run(mut self, mut rx: mpsc::UnboundedReceiver<RoomCmd<G>>) {
        let mut tick = tokio::time::interval(Duration::from_millis(G::TICK_MS as u64));
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    let Some(cmd) = cmd else { break };
                    self.handle(cmd);
                    if self.members.is_empty() {
                        break;
                    }
                }
                _ = tick.tick() => self.step(),
            }
        }
    }

    fn step(&mut self) {
        self.game.step();
        if self.game.phase() == Phase::GameOver && !self.announced_game_over {
            self.announced_game_over = true;
            self.broadcast_info("Game over!");
        }
        let view = self.game.view_msg();
        for m in &self.members {
            // A client that can't keep up misses views rather than queueing them.
            let _ = m.out.try_send(view.clone());
        }
    }

    fn handle(&mut self, cmd: RoomCmd<G>) {
        match cmd {
            RoomCmd::Join { conn, name, out } => self.join(conn, name, out),
            RoomCmd::Leave { conn } => self.leave(conn),
            RoomCmd::Msg { conn, msg } => self.message(conn, msg),
        }
        self.publish_info();
    }

    fn join(&mut self, conn: u64, name: String, out: Outbox<G>) {
        if self.members.len() >= G::MAX_PLAYERS {
            let _ = out.try_send(G::outgoing(Outgoing::Rejected("Server full.".into())));
            return;
        }
        if self.game.started() {
            let _ = out.try_send(G::outgoing(Outgoing::Rejected("Game started.".into())));
            return;
        }
        let name = self.unique_name(&name);
        let color = COLOR_POOL
            .iter()
            .copied()
            .find(|c| !self.members.iter().any(|m| m.color == *c))
            .unwrap_or(4);
        let player = self.game.add_player(&name, color);
        let host = self.members.is_empty();
        let _ = out.try_send(G::outgoing(Outgoing::Welcome {
            code: self.code.clone(),
            you: player,
            host,
            settings: self.settings,
        }));
        self.members.push(Member { conn, player, name: name.clone(), color, out });
        self.broadcast_info(&format!("{name} joined."));
    }

    /// HostMenu / ServerState.createPlayer: no "Unnamed" and no duplicates.
    fn unique_name(&mut self, wanted: &str) -> String {
        let mut name: String = wanted.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
        if name.is_empty() || name == "Unnamed" {
            name = format!("Unnamed{}", self.random(1000));
        }
        while self.members.iter().any(|m| m.name == name) {
            name = format!("Unnamed{}", self.random(1000));
        }
        name
    }

    fn random(&mut self, n: u64) -> u64 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.seed >> 33) % n
    }

    fn leave(&mut self, conn: u64) {
        let Some(i) = self.members.iter().position(|m| m.conn == conn) else { return };
        let was_host = i == 0;
        let m = self.members.remove(i);
        self.game.remove_player(m.player);
        self.broadcast_info(&format!("{} disconnected.", m.name));
        if was_host {
            if let Some(h) = self.members.first() {
                h.send(Outgoing::Host(true));
                let text = format!("{} is now the host.", h.name);
                self.broadcast_info(&text);
            }
        }
    }

    fn message(&mut self, conn: u64, msg: Incoming<G::Input>) {
        let Some(i) = self.members.iter().position(|m| m.conn == conn) else { return };
        let is_host = i == 0;
        let player = self.members[i].player;
        match msg {
            Incoming::Input(input) => self.game.set_input(player, input),
            Incoming::Chat(text) => {
                let text: String = text.chars().filter(|c| !c.is_control()).collect();
                let text = text.trim();
                if text.is_empty() {
                    return;
                }
                if let Some(target) = text.strip_prefix("/k ") {
                    if is_host {
                        self.kick(target.trim());
                    }
                    return;
                }
                let line: String = format!("{}: {}", self.members[i].name, text).chars().take(MAX_CHAT).collect();
                self.broadcast(Outgoing::Message { text: line, level: MsgLevel::Chat });
            }
            Incoming::Start if is_host && !self.game.started() => {
                self.game.start();
                self.broadcast_info("Game started!");
            }
            Incoming::Restart if is_host && self.game.is_game_over() => self.restart(),
            _ => {}
        }
    }

    fn kick(&mut self, name: &str) {
        let Some(i) = self.members.iter().position(|m| m.name == name) else {
            if let Some(h) = self.members.first() {
                h.send(Outgoing::Message { text: format!("Player: {name} not found."), level: MsgLevel::Error });
            }
            return;
        };
        if i == 0 {
            return;
        }
        let m = self.members.remove(i);
        m.send(Outgoing::Kicked);
        self.game.remove_player(m.player);
        self.broadcast_info(&format!("{} kicked.", m.name));
    }

    /// A new game with the same players and settings, back in the lobby.
    fn restart(&mut self) {
        self.seed = self.seed.wrapping_add(0x9E37_79B9);
        self.game = G::new(self.settings, self.seed);
        self.announced_game_over = false;
        for (i, m) in self.members.iter_mut().enumerate() {
            m.player = self.game.add_player(&m.name, m.color);
            m.send(Outgoing::Welcome { code: self.code.clone(), you: m.player, host: i == 0, settings: self.settings });
        }
        self.broadcast_info("Back to the lobby.");
    }

    fn broadcast(&self, msg: Outgoing) {
        let msg = G::outgoing(msg);
        for m in &self.members {
            let _ = m.out.try_send(msg.clone());
        }
    }

    fn broadcast_info(&self, text: &str) {
        self.broadcast(Outgoing::Message { text: text.to_string(), level: MsgLevel::Info });
    }

    fn publish_info(&self) {
        let mut info = self.info.lock().unwrap();
        info.players = self.members.len();
        info.started = self.game.started();
        if let Some(h) = self.members.first() {
            info.host = h.name.clone();
        }
    }
}

/// A room registry: one per game, so a code only finds rooms of its game.
pub struct Rooms<G: RoomGame> {
    pub rooms: Mutex<HashMap<String, RoomEntry<G>>>,
}

/// A room's command channel and its lobby-list entry.
pub type RoomEntry<G> = (mpsc::UnboundedSender<RoomCmd<G>>, Arc<Mutex<RoomInfo>>);

impl<G: RoomGame> Default for Rooms<G> {
    fn default() -> Self {
        Rooms { rooms: Mutex::default() }
    }
}

impl<G: RoomGame> Rooms<G> {
    pub fn list(&self) -> Vec<RoomInfo> {
        let rooms = self.rooms.lock().unwrap();
        let mut v: Vec<RoomInfo> = rooms
            .values()
            .filter(|(tx, _)| !tx.is_closed())
            .map(|(_, i)| i.lock().unwrap().clone())
            .filter(|i| i.players > 0)
            .collect();
        v.sort_by(|a, b| a.code.cmp(&b.code));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tansk_sim::proto::ServerMsg;
    use tansk_sim::Game;

    fn settings() -> Settings {
        tansk_sim::proto::Settings::default()
    }

    async fn next(rx: &mut mpsc::Receiver<ServerMsg>) -> ServerMsg {
        loop {
            match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.expect("timeout").expect("closed") {
                ServerMsg::View(_) => continue,
                m => return m,
            }
        }
    }

    async fn until(rx: &mut mpsc::Receiver<ServerMsg>, f: impl Fn(&ServerMsg) -> bool) -> ServerMsg {
        loop {
            let m = next(rx).await;
            if f(&m) {
                return m;
            }
        }
    }

    #[tokio::test]
    async fn rooms_2d_join_chat_kick_handover() {
        let info = Arc::new(Mutex::new(RoomInfo {
            code: "ABCD".into(),
            host: "a".into(),
            koth: false,
            players: 0,
            max_players: 4,
            started: false,
        }));
        let room = Room::<Game>::new("ABCD".into(), settings(), 7, info.clone());
        let (tx, rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(room.run(rx));

        let mut outs = Vec::new();
        for (conn, name) in [(1, "alice"), (2, "alice"), (3, "carol"), (4, "dave"), (5, "eve")] {
            let (out, orx) = mpsc::channel(1024);
            tx.send(RoomCmd::Join { conn, name: name.into(), out }).unwrap();
            outs.push(orx);
        }
        match next(&mut outs[0]).await {
            ServerMsg::Welcome { code, host, .. } => assert!(code == "ABCD" && host),
            m => panic!("{m:?}"),
        }
        assert!(matches!(next(&mut outs[1]).await, ServerMsg::Welcome { host: false, .. }));
        // Fifth player: room full.
        assert!(matches!(next(&mut outs[4]).await, ServerMsg::Rejected(_)));

        // Duplicate name got renamed; chat reaches everyone.
        tx.send(RoomCmd::Msg { conn: 3, msg: Incoming::Chat("hi".into()) }).unwrap();
        let chat = until(&mut outs[0], |m| matches!(m, ServerMsg::Message { level: MsgLevel::Chat, .. })).await;
        assert!(matches!(chat, ServerMsg::Message { text, .. } if text == "carol: hi"));

        // Only the host kicks.
        tx.send(RoomCmd::Msg { conn: 3, msg: Incoming::Chat("/k dave".into()) }).unwrap();
        tx.send(RoomCmd::Msg { conn: 1, msg: Incoming::Chat("/k carol".into()) }).unwrap();
        assert!(matches!(until(&mut outs[2], |m| matches!(m, ServerMsg::Kicked)).await, ServerMsg::Kicked));

        // Host leaves: the next player becomes host.
        tx.send(RoomCmd::Leave { conn: 1 }).unwrap();
        until(&mut outs[1], |m| matches!(m, ServerMsg::Host(true))).await;
        tx.send(RoomCmd::Msg { conn: 2, msg: Incoming::Start }).unwrap();
        until(&mut outs[1], |m| matches!(m, ServerMsg::Message { text, .. } if text == "Game started!")).await;
        {
            let i = info.lock().unwrap();
            assert_eq!(i.players, 2);
            assert!(i.started);
        }

        tx.send(RoomCmd::Leave { conn: 2 }).unwrap();
        tx.send(RoomCmd::Leave { conn: 4 }).unwrap();
        tokio::time::timeout(Duration::from_secs(2), task).await.expect("room ends").unwrap();
    }
}
