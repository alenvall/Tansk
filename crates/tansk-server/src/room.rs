//! One room: a `Game` stepped every tick, its players' sockets, its chat.
//! The original's ServerState, minus the window.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tansk_sim::proto::{ClientMsg, MsgLevel, Phase, ServerMsg, Settings};
use tansk_sim::{Game, Input, MAX_PLAYERS, TICK_MS};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

pub type Outbox = mpsc::Sender<ServerMsg>;

pub enum RoomCmd {
    Join { conn: u64, name: String, out: Outbox },
    Leave { conn: u64 },
    Msg { conn: u64, msg: ClientMsg },
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

struct Member {
    conn: u64,
    player: u32,
    name: String,
    color: u8,
    out: Outbox,
}

pub struct Room {
    code: String,
    settings: Settings,
    game: Game,
    members: Vec<Member>,
    info: Arc<Mutex<RoomInfo>>,
    seed: u64,
    announced_game_over: bool,
}

/// The four colours of the original's server pool.
const COLOR_POOL: [u8; 4] = [0, 1, 2, 3];
const MAX_NAME: usize = 10;
const MAX_CHAT: usize = 60;

impl Room {
    pub fn new(code: String, settings: Settings, seed: u64, info: Arc<Mutex<RoomInfo>>) -> Self {
        Room {
            game: Game::new(settings, seed),
            settings: settings.clamped(),
            code,
            members: Vec::new(),
            info,
            seed,
            announced_game_over: false,
        }
    }

    /// Runs the room until its last player leaves.
    pub async fn run(mut self, mut rx: mpsc::UnboundedReceiver<RoomCmd>) {
        let mut tick = tokio::time::interval(Duration::from_millis(TICK_MS as u64));
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
        let view = ServerMsg::View(self.game.view());
        for m in &self.members {
            // A client that can't keep up misses views rather than queueing them.
            let _ = m.out.try_send(view.clone());
        }
    }

    fn handle(&mut self, cmd: RoomCmd) {
        match cmd {
            RoomCmd::Join { conn, name, out } => self.join(conn, name, out),
            RoomCmd::Leave { conn } => self.leave(conn),
            RoomCmd::Msg { conn, msg } => self.message(conn, msg),
        }
        self.publish_info();
    }

    fn join(&mut self, conn: u64, name: String, out: Outbox) {
        if self.members.len() >= MAX_PLAYERS {
            let _ = out.try_send(ServerMsg::Rejected("Server full.".into()));
            return;
        }
        if self.game.started {
            let _ = out.try_send(ServerMsg::Rejected("Game started.".into()));
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
        let _ = out.try_send(ServerMsg::Welcome {
            code: self.code.clone(),
            you: player,
            host,
            settings: self.settings,
        });
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
                let _ = h.out.try_send(ServerMsg::Host(true));
                let text = format!("{} is now the host.", h.name);
                self.broadcast_info(&text);
            }
        }
    }

    fn message(&mut self, conn: u64, msg: ClientMsg) {
        let Some(i) = self.members.iter().position(|m| m.conn == conn) else { return };
        let is_host = i == 0;
        let player = self.members[i].player;
        match msg {
            ClientMsg::Input(input) => self.game.set_input(player, sanitize(input)),
            ClientMsg::Chat(text) => {
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
                self.broadcast(ServerMsg::Message { text: line, level: MsgLevel::Chat });
            }
            ClientMsg::Start if is_host && !self.game.started => {
                self.game.start();
                self.broadcast_info("Game started!");
            }
            ClientMsg::Restart if is_host && self.game.is_game_over() => self.restart(),
            _ => {}
        }
    }

    fn kick(&mut self, name: &str) {
        let Some(i) = self.members.iter().position(|m| m.name == name) else {
            if let Some(h) = self.members.first() {
                let _ = h.out.try_send(ServerMsg::Message {
                    text: format!("Player: {name} not found."),
                    level: MsgLevel::Error,
                });
            }
            return;
        };
        if i == 0 {
            return;
        }
        let m = self.members.remove(i);
        let _ = m.out.try_send(ServerMsg::Kicked);
        self.game.remove_player(m.player);
        self.broadcast_info(&format!("{} kicked.", m.name));
    }

    /// A new game with the same players and settings, back in the lobby.
    fn restart(&mut self) {
        self.seed = self.seed.wrapping_add(0x9E37_79B9);
        self.game = Game::new(self.settings, self.seed);
        self.announced_game_over = false;
        for (i, m) in self.members.iter_mut().enumerate() {
            m.player = self.game.add_player(&m.name, m.color);
            let _ = m.out.try_send(ServerMsg::Welcome {
                code: self.code.clone(),
                you: m.player,
                host: i == 0,
                settings: self.settings,
            });
        }
        self.broadcast_info("Back to the lobby.");
    }

    fn broadcast(&self, msg: ServerMsg) {
        for m in &self.members {
            let _ = m.out.try_send(msg.clone());
        }
    }

    fn broadcast_info(&self, text: &str) {
        self.broadcast(ServerMsg::Message { text: text.to_string(), level: MsgLevel::Info });
    }

    fn publish_info(&self) {
        let mut info = self.info.lock().unwrap();
        info.players = self.members.len();
        info.started = self.game.started;
        if let Some(h) = self.members.first() {
            info.host = h.name.clone();
        }
    }
}

/// Inputs come from the network: keep the aim point finite.
fn sanitize(mut i: Input) -> Input {
    if !i.aim.x.is_finite() || !i.aim.y.is_finite() {
        i.aim = tansk_sim::V2::new(512.0, 384.0);
    }
    i.aim.x = i.aim.x.clamp(-2000.0, 3000.0);
    i.aim.y = i.aim.y.clamp(-2000.0, 3000.0);
    i
}

/// The room registry.
#[derive(Default)]
pub struct Rooms {
    pub rooms: Mutex<HashMap<String, (mpsc::UnboundedSender<RoomCmd>, Arc<Mutex<RoomInfo>>)>>,
}

impl Rooms {
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
