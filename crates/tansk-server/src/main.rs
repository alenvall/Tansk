//! tansk-server: the web client's files at `/`, game rooms on the WebSocket
//! at `/ws`, the open rooms at `/api/rooms`, and `/health`.
//!
//! Environment: `PORT` (default 8080), `TANSK_STATIC_DIR` (default `dist`,
//! the web client as `trunk build` leaves it).

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
use room::{Room, RoomCmd, RoomInfo, Rooms};
use tansk_sim::proto::{decode, encode, ClientMsg, ServerMsg};
use tansk_sim::MAX_PLAYERS;
use tokio::sync::mpsc;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

const MAX_ROOMS: usize = 500;
/// Room codes leave out letters that are easy to misread.
const CODE_LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

struct App {
    rooms: Rooms,
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

    fn new_code(&self) -> String {
        let rooms = self.rooms.rooms.lock().unwrap();
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
    let app = Arc::new(App { rooms: Rooms::default(), next_conn: AtomicU64::new(1), seed: Mutex::new(now | 1) });

    let index = format!("{dir}/index.html");
    let files = ServeDir::new(&dir).fallback(ServeFile::new(index));
    let router = Router::new()
        .route("/ws", get(ws))
        .route("/api/rooms", get(list_rooms))
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

async fn ws(ws: WebSocketUpgrade, State(app): State<Arc<App>>) -> impl IntoResponse {
    ws.max_message_size(16 * 1024).on_upgrade(move |s| connection(s, app))
}

async fn connection(socket: WebSocket, app: Arc<App>) {
    let conn = app.next_conn.fetch_add(1, Ordering::Relaxed);
    let (mut sink, mut stream) = socket.split();
    let (out, mut out_rx) = mpsc::channel::<ServerMsg>(128);

    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let last = matches!(msg, ServerMsg::Kicked | ServerMsg::Rejected(_));
            if sink.send(Message::Binary(encode(&msg).into())).await.is_err() || last {
                let _ = sink.close().await;
                break;
            }
        }
    });

    let mut room_tx: Option<mpsc::UnboundedSender<RoomCmd>> = None;
    while let Some(Ok(frame)) = stream.next().await {
        let bytes = match frame {
            Message::Binary(b) => b,
            Message::Close(_) => break,
            _ => continue,
        };
        let Some(msg) = decode::<ClientMsg>(&bytes) else { continue };
        if let Some(tx) = &room_tx {
            if tx.send(RoomCmd::Msg { conn, msg }).is_err() {
                break;
            }
            continue;
        }
        match msg {
            ClientMsg::Create { name, settings } => {
                if app.rooms.rooms.lock().unwrap().len() >= MAX_ROOMS {
                    let _ = out.try_send(ServerMsg::Rejected("Too many games running, try again later.".into()));
                    continue;
                }
                let code = app.new_code();
                let (tx, rx) = mpsc::unbounded_channel();
                let info = Arc::new(Mutex::new(RoomInfo {
                    code: code.clone(),
                    host: name.clone(),
                    koth: settings.koth,
                    players: 0,
                    max_players: MAX_PLAYERS,
                    started: false,
                }));
                app.rooms.rooms.lock().unwrap().insert(code.clone(), (tx.clone(), info.clone()));
                let room = Room::new(code.clone(), settings, app.random(), info);
                let app2 = app.clone();
                tokio::spawn(async move {
                    room.run(rx).await;
                    app2.rooms.rooms.lock().unwrap().remove(&code);
                });
                let _ = tx.send(RoomCmd::Join { conn, name, out: out.clone() });
                room_tx = Some(tx);
            }
            ClientMsg::Join { code, name } => {
                let code = code.trim().to_uppercase();
                let tx = app.rooms.rooms.lock().unwrap().get(&code).map(|(tx, _)| tx.clone());
                match tx {
                    Some(tx) if tx.send(RoomCmd::Join { conn, name, out: out.clone() }).is_ok() => {
                        room_tx = Some(tx);
                    }
                    _ => {
                        let _ = out.try_send(ServerMsg::Rejected(format!("No game with the code {code}.")));
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
