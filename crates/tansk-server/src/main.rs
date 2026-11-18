//! tansk-server: the web client's files at `/` (and `/3d`, any unknown path
//! gets index.html), 2D game rooms on the WebSocket at `/ws` with the open
//! ones at `/api/rooms`, 3D rooms at `/ws3d` and `/api/rooms3d`, and `/health`.
//!
//! Environment: `PORT` (default 8080), `TANSK_STATIC_DIR` (default `dist`,
//! the web client as `trunk build` leaves it).

mod games;
mod room;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, HeaderValue};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use games::{Incoming, Outgoing, RoomGame};
use room::{Room, RoomCmd, RoomInfo, Rooms};
use tansk3d_sim::Game3d;
use tansk_sim::proto::{decode, encode};
use tansk_sim::Game;
use tokio::sync::mpsc;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

const MAX_ROOMS: usize = 500;
/// Room codes leave out letters that are easy to misread.
const CODE_LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

struct App {
    rooms: Rooms<Game>,
    rooms3d: Rooms<Game3d>,
    next_conn: AtomicU64,
    seed: Mutex<u64>,
}

impl App {
    fn random(&self) -> u64 {
        let mut s = self.seed.lock().unwrap();
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }

    fn room_count(&self) -> usize {
        self.rooms.rooms.lock().unwrap().len() + self.rooms3d.rooms.lock().unwrap().len()
    }

    fn new_code<G: RoomGame>(&self, registry: &Rooms<G>) -> String {
        let rooms = registry.rooms.lock().unwrap();
        loop {
            let mut r = self.random();
            let code: String = (0..4)
                .map(|_| {
                    let c = CODE_LETTERS[(r % CODE_LETTERS.len() as u64) as usize] as char;
                    r /= CODE_LETTERS.len() as u64;
                    c
                })
                .collect();
            if !rooms.contains_key(&code) {
                return code;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let dir = std::env::var("TANSK_STATIC_DIR").unwrap_or_else(|_| "dist".into());
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let app = Arc::new(App {
        rooms: Rooms::default(),
        rooms3d: Rooms::default(),
        next_conn: AtomicU64::new(1), seed: Mutex::new(now | 1) });

    let index = format!("{dir}/index.html");
    let files = ServeDir::new(&dir).fallback(ServeFile::new(index));
    let router = Router::new()
        .route("/ws", get(ws))
        .route("/api/rooms", get(list_rooms))
        .route("/ws3d", get(ws3d))
        .route("/api/rooms3d", get(list_rooms3d))
        .route("/health", get(|| async { "ok" }))
        .fallback_service(files)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
        .layer(CompressionLayer::new())
        .with_state(app);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    println!("tansk-server listening on http://{addr} (files from {dir})");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("serve");
}

async fn list_rooms(State(app): State<Arc<App>>) -> Json<Vec<RoomInfo>> {
    Json(app.rooms.list())
}

async fn list_rooms3d(State(app): State<Arc<App>>) -> Json<Vec<RoomInfo>> {
    Json(app.rooms3d.list())
}

async fn ws(ws: WebSocketUpgrade, State(app): State<Arc<App>>) -> impl IntoResponse {
    ws.max_message_size(16 * 1024).on_upgrade(move |s| connection::<Game>(s, app, |a| &a.rooms))
}

async fn ws3d(ws: WebSocketUpgrade, State(app): State<Arc<App>>) -> impl IntoResponse {
    ws.max_message_size(16 * 1024).on_upgrade(move |s| connection::<Game3d>(s, app, |a| &a.rooms3d))
}

/// One client's socket. `registry` picks the game's rooms, so a code from
/// one game never finds a room of the other.
async fn connection<G: RoomGame>(socket: WebSocket, app: Arc<App>, registry: fn(&App) -> &Rooms<G>) {
    let conn = app.next_conn.fetch_add(1, Ordering::Relaxed);
    let (mut sink, mut stream) = socket.split();
    let (out, mut out_rx) = mpsc::channel::<G::ServerMsg>(128);
    let rejected = |text: String| G::outgoing(Outgoing::Rejected(text));

    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let last = G::is_final(&msg);
            if sink.send(Message::Binary(encode(&msg).into())).await.is_err() || last {
                let _ = sink.close().await;
                break;
            }
        }
    });

    let mut room_tx: Option<mpsc::UnboundedSender<RoomCmd<G>>> = None;
    while let Some(Ok(frame)) = stream.next().await {
        let bytes = match frame {
            Message::Binary(b) => b,
            Message::Close(_) => break,
            _ => continue,
        };
        let Some(msg) = decode::<G::ClientMsg>(&bytes) else { continue };
        let msg = G::incoming(msg);
        if let Some(tx) = &room_tx {
            if tx.send(RoomCmd::Msg { conn, msg }).is_err() {
                break;
            }
            continue;
        }
        match msg {
            Incoming::Create { name, settings } => {
                if app.room_count() >= MAX_ROOMS {
                    let _ = out.try_send(rejected("Too many games running, try again later.".into()));
                    continue;
                }
                let code = app.new_code(registry(&app));
                let (tx, rx) = mpsc::unbounded_channel();
                let info = Arc::new(Mutex::new(RoomInfo {
                    code: code.clone(),
                    host: name.clone(),
                    koth: settings.koth,
                    players: 0,
                    max_players: G::MAX_PLAYERS,
                    started: false,
                }));
                registry(&app).rooms.lock().unwrap().insert(code.clone(), (tx.clone(), info.clone()));
                let room = Room::<G>::new(code.clone(), settings, app.random(), info);
                let app2 = app.clone();
                tokio::spawn(async move {
                    room.run(rx).await;
                    registry(&app2).rooms.lock().unwrap().remove(&code);
                });
                let _ = tx.send(RoomCmd::Join { conn, name, out: out.clone() });
                room_tx = Some(tx);
            }
            Incoming::Join { code, name } => {
                let code = code.trim().to_uppercase();
                let tx = registry(&app).rooms.lock().unwrap().get(&code).map(|(tx, _)| tx.clone());
                match tx {
                    Some(tx) if tx.send(RoomCmd::Join { conn, name, out: out.clone() }).is_ok() => {
                        room_tx = Some(tx);
                    }
                    _ => {
                        let _ = out.try_send(rejected(format!("No game with the code {code}.")));
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(tx) = room_tx {
        let _ = tx.send(RoomCmd::Leave { conn });
    }
    drop(out);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), writer).await;
}
