//! tansk-server: the web client's files at `/` (any unknown path gets
//! index.html; trunk's hashed .js/.wasm/.css are cached for a year, the rest
//! is revalidated), 2D game rooms on the WebSocket at `/ws` with the open
//! ones at `/api/rooms`, 3D rooms at `/ws3d` and `/api/rooms3d`, TANSK 2026
//! (arena) rooms at `/ws/arena` and `/api/arena/rooms`, and `/health`.
//!
//! The idea box: `POST /api/ideas` (see `ideas.rs`), and `GET /api/ideas`
//! with the right bearer token.
//!
//! Environment: `PORT` (default 8080), `TANSK_STATIC_DIR` (default `dist`,
//! the web client as `trunk build` leaves it), `IDEAS_DIR` (default `data`)
//! and `IDEAS_TOKEN` (unset: no listing).

mod arena;
mod games;
mod ideas;
mod room;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use arena::{ArenaCmd, ArenaInfo, ArenaRoom, ArenaRooms, Frame};
use arena_core::net::RoomSettings;
use arena_core::raid::ModeKind;
use arena_core::MapId;
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

const MAX_ROOMS: usize = 500;
/// Room codes leave out letters that are easy to misread.
const CODE_LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

struct App {
    rooms: Rooms<Game>,
    rooms3d: Rooms<Game3d>,
    arena: ArenaRooms,
    next_conn: AtomicU64,
    seed: Mutex<u64>,
    /// The map the next quick-match room gets (they alternate).
    quick_map: Mutex<MapId>,
    ideas: ideas::Ideas,
}

impl App {
    /// The map for a new quick-match room: the Sink, the Refinery, the Sink...
    fn next_quick_map(&self) -> MapId {
        let mut m = self.quick_map.lock().unwrap();
        let this = *m;
        *m = this.next();
        this
    }

    fn random(&self) -> u64 {
        let mut s = self.seed.lock().unwrap();
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }

    fn room_count(&self) -> usize {
        self.rooms.rooms.lock().unwrap().len()
            + self.rooms3d.rooms.lock().unwrap().len()
            + self.arena.rooms.lock().unwrap().len()
    }

    /// A fresh 4-letter code; `taken` says which are in use.
    fn new_code(&self, taken: impl Fn(&str) -> bool) -> String {
        loop {
            let mut r = self.random();
            let code: String = (0..4)
                .map(|_| {
                    let c = CODE_LETTERS[(r % CODE_LETTERS.len() as u64) as usize] as char;
                    r /= CODE_LETTERS.len() as u64;
                    c
                })
                .collect();
            if !taken(&code) {
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
    let router = router(App::new(now), &dir);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    println!("tansk-server listening on http://{addr} (files from {dir})");
    // Peer addresses only key the idea box's rate limit (behind a proxy,
    // X-Forwarded-For does).
    axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("serve");
}

impl App {
    fn new(seed: u64) -> Arc<App> {
        App::with_ideas(seed, ideas::Ideas::from_env())
    }

    fn with_ideas(seed: u64, ideas: ideas::Ideas) -> Arc<App> {
        Arc::new(App {
            ideas,
            rooms: Rooms::default(),
            rooms3d: Rooms::default(),
            arena: ArenaRooms::default(),
            next_conn: AtomicU64::new(1),
            seed: Mutex::new(seed | 1),
            quick_map: Mutex::new(MapId::Sink),
        })
    }
}

fn router(app: Arc<App>, dir: &str) -> Router {
    let index = format!("{dir}/index.html");
    // Serves `x.wasm.br` / `x.wasm.gz` when the build left them there.
    let files = ServeDir::new(dir).precompressed_br().precompressed_gzip().fallback(ServeFile::new(index));
    Router::new()
        .route("/ws", get(ws))
        .route("/api/rooms", get(list_rooms))
        .route("/ws3d", get(ws3d))
        .route("/api/rooms3d", get(list_rooms3d))
        .route("/ws/arena", get(ws_arena))
        .route("/api/arena/rooms", get(list_arena))
        .route("/api/ideas", get(list_ideas).post(post_idea))
        .route("/health", get(|| async { "ok" }))
        .fallback_service(files)
        .layer(middleware::from_fn(cache_headers))
        .layer(CompressionLayer::new())
        .with_state(app)
}

/// Who's posting, for the rate limit only: the proxy's X-Forwarded-For,
/// else the peer address.
fn client_key(req: &Request) -> String {
    let fwd = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|v| v.split(',').next()).map(|s| s.trim().to_string());
    fwd.filter(|s| !s.is_empty())
        .or_else(|| req.extensions().get::<axum::extract::ConnectInfo<SocketAddr>>().map(|c| c.0.ip().to_string()))
        .unwrap_or_else(|| "local".into())
}

async fn post_idea(State(app): State<Arc<App>>, req: Request) -> Response {
    use axum::http::StatusCode;
    let who = client_key(&req);
    let reply = |code: StatusCode, msg: &str| (code, Json(serde_json::json!({ "ok": code.is_success(), "message": msg }))).into_response();
    let Ok(body) = axum::body::to_bytes(req.into_body(), ideas::MAX_BODY).await else {
        return reply(StatusCode::PAYLOAD_TOO_LARGE, "That's too long.");
    };
    let Ok(input) = serde_json::from_slice::<ideas::IdeaIn>(&body) else {
        return reply(StatusCode::BAD_REQUEST, "That didn't come through right.");
    };
    match app.ideas.post(&who, input) {
        Ok(_) => reply(StatusCode::CREATED, "Thanks, the crew will look at it."),
        Err(e) => {
            let code = match e {
                ideas::Refused::Invalid(_) => StatusCode::BAD_REQUEST,
                ideas::Refused::TooMany => StatusCode::TOO_MANY_REQUESTS,
                ideas::Refused::Full => StatusCode::INSUFFICIENT_STORAGE,
                ideas::Refused::Storage => StatusCode::INTERNAL_SERVER_ERROR,
            };
            reply(code, e.message())
        }
    }
}

async fn list_ideas(State(app): State<Arc<App>>, headers: axum::http::HeaderMap) -> Response {
    use axum::http::StatusCode;
    if !app.ideas.listing_enabled() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let auth = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    if !app.ideas.authorized(auth) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut res = Json(app.ideas.all()).into_response();
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// Hashed build outputs never change; everything else is revalidated.
async fn cache_headers(req: Request, next: Next) -> Response {
    let hashed = is_hashed(req.uri().path());
    let mut res = next.run(req).await;
    if !res.headers().contains_key(header::CACHE_CONTROL) {
        // The index.html fallback for a missing asset must not be cached as one.
        let html = res.headers().get(header::CONTENT_TYPE).is_some_and(|v| v.as_bytes().starts_with(b"text/html"));
        let value = if hashed && !html && res.status().is_success() { "public, max-age=31536000, immutable" } else { "no-cache" };
        res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    }
    res
}

/// trunk's `name-<16 hex digits>[_bg].ext` files.
fn is_hashed(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or("");
    let Some((stem, _ext)) = name.split_once('.') else { return false };
    let Some((_, hash)) = stem.rsplit_once('-') else { return false };
    let (hex, rest) = hash.split_at(hash.len().min(16));
    hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()) && (rest.is_empty() || rest == "_bg")
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
                let code = {
                    let rooms = registry(&app).rooms.lock().unwrap();
                    app.new_code(|c| rooms.contains_key(c))
                };
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

async fn list_arena(State(app): State<Arc<App>>) -> Json<Vec<ArenaInfo>> {
    Json(app.arena.list())
}

async fn ws_arena(ws: WebSocketUpgrade, State(app): State<Arc<App>>) -> impl IntoResponse {
    ws.max_message_size(arena_core::net::MAX_CLIENT_MSG).on_upgrade(move |s| arena_connection(s, app))
}

/// A new arena room, registered and running; returns its command channel.
/// `reserve` counts the joining player in its info right away (quick match).
fn create_arena(app: &Arc<App>, public: bool, reserve: bool, mode: ModeKind, map: MapId) -> mpsc::UnboundedSender<ArenaCmd> {
    create_room(app, public, reserve, mode, map, None)
}

/// A new party room (a lobby before each match), registered and running.
fn create_party(app: &Arc<App>, public: bool, settings: RoomSettings) -> mpsc::UnboundedSender<ArenaCmd> {
    create_room(app, public, false, settings.mode, settings.map, Some(settings))
}

fn create_room(
    app: &Arc<App>,
    public: bool,
    reserve: bool,
    mode: ModeKind,
    map: MapId,
    party: Option<RoomSettings>,
) -> mpsc::UnboundedSender<ArenaCmd> {
    let mut rooms = app.arena.rooms.lock().unwrap();
    let code = app.new_code(|c| rooms.contains_key(c));
    let (tx, rx) = mpsc::unbounded_channel();
    let info = Arc::new(Mutex::new(ArenaInfo {
        code: code.clone(),
        public,
        humans: reserve as usize,
        max_humans: if mode == ModeKind::RaiderWaves { crate::arena::RAID_SEATS } else { arena_core::net::ROOM_SEATS },
        ended: false,
        mode,
        map,
        party: party.is_some(),
    }));
    rooms.insert(code.clone(), (tx.clone(), info.clone()));
    drop(rooms);
    let room = match party {
        Some(settings) => ArenaRoom::new_party(code.clone(), public, app.random(), info, settings),
        None => ArenaRoom::new(code.clone(), public, app.random(), info, mode, map),
    };
    let app2 = app.clone();
    tokio::spawn(async move {
        room.run(rx).await;
        app2.arena.rooms.lock().unwrap().remove(&code);
    });
    tx
}

async fn arena_connection(socket: WebSocket, app: Arc<App>) {
    use arena_core::net::{decode_client, encode, ClientMsg, Join, ServerMsg};
    let conn = app.next_conn.fetch_add(1, Ordering::Relaxed);
    let (mut sink, mut stream) = socket.split();
    let (out, mut out_rx) = mpsc::channel::<Frame>(256);

    let writer = tokio::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            if sink.send(Message::Binary(f.bytes.into())).await.is_err() || f.last {
                let _ = sink.close().await;
                break;
            }
        }
    });
    let reject = |text: String| {
        let _ = out.try_send(Frame { bytes: encode(&ServerMsg::Rejected(text)), last: true });
    };
    const BUSY: &str = "Too many games running, try again later.";

    let mut room_tx: Option<mpsc::UnboundedSender<ArenaCmd>> = None;
    while let Some(Ok(frame)) = stream.next().await {
        let bytes = match frame {
            Message::Binary(b) => b,
            Message::Close(_) => break,
            _ => continue,
        };
        let Some(msg) = decode_client(&bytes) else { continue };
        let cmd = match msg {
            ClientMsg::Input { seq, input } => ArenaCmd::Input { conn, seq, input },
            ClientMsg::Chat(text) => ArenaCmd::Chat { conn, text },
            ClientMsg::Lobby(cmd) => ArenaCmd::Lobby { conn, cmd },
            ClientMsg::Hello { .. } if room_tx.is_some() => continue,
            ClientMsg::Hello { name, join } => {
                let lookup = |code: &str| {
                    app.arena.rooms.lock().unwrap().get(code).map(|(tx, _)| tx.clone())
                };
                let busy = app.room_count() >= MAX_ROOMS;
                let (tx, rejoin) = match join {
                    Join::Play => match app.arena.quick_match() {
                        Some(tx) => (tx, None),
                        None if busy => {
                            reject(BUSY.into());
                            break;
                        }
                        None => (create_arena(&app, true, true, ModeKind::ScrapBrawl, app.next_quick_map()), None),
                    },
                    Join::Host { .. } if busy => {
                            reject(BUSY.into());
                            break;
                        }
                    Join::Host { public, mode, map } => (create_arena(&app, public, false, mode, map), None),
                    Join::Party { .. } if busy => {
                        reject(BUSY.into());
                        break;
                    }
                    Join::Party { public, settings } => (create_party(&app, public, settings), None),
                    Join::Code(code) => {
                        let code = code.trim().to_uppercase();
                        match lookup(&code) {
                            Some(tx) => (tx, None),
                            None => {
                            reject(format!("No game with the code {code}."));
                            break;
                        }
                        }
                    }
                    Join::Rejoin { code, token } => match lookup(code.trim()) {
                        Some(tx) => (tx, Some(token)),
                        None => {
                            reject("That game has ended.".into());
                            break;
                        }
                    },
                };
                room_tx = Some(tx);
                ArenaCmd::Join { conn, name, rejoin, out: out.clone() }
            }
        };
        match &room_tx {
            Some(tx) if tx.send(cmd).is_ok() => {}
            Some(_) => break,
            None => {}
        }
    }
    if let Some(tx) = room_tx {
        let _ = tx.send(ArenaCmd::Leave { conn });
    }
    drop(out);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), writer).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::net::{decode, encode, ClientMsg, Join, ServerMsg};
    use arena_core::{Input, PlayerId, Vec3};
    use std::time::Duration;
    use tokio_tungstenite::tungstenite::Message as WsMsg;

    type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

    async fn start_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = router(App::new(42), "no-such-dir");
        tokio::spawn(async move { axum::serve(listener, router).await });
        format!("ws://{addr}/ws/arena")
    }

    async fn send(ws: &mut Ws, msg: &ClientMsg) {
        ws.send(WsMsg::Binary(encode(msg).into())).await.unwrap();
    }

    async fn recv<S>(ws: &mut S) -> ServerMsg
    where
        S: futures_util::Stream<Item = Result<WsMsg, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.expect("timeout").unwrap().unwrap();
            if let WsMsg::Binary(b) = frame {
                return decode(&b).expect("decodes");
            }
        }
    }

    /// Connects and joins; returns the socket, room code and player id.
    async fn join(url: &str, name: &str, join: Join) -> (Ws, String, PlayerId) {
        let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        send(&mut ws, &ClientMsg::Hello { name: name.into(), join }).await;
        loop {
            match recv(&mut ws).await {
                ServerMsg::Welcome { code, you, .. } => return (ws, code, you),
                ServerMsg::Rejected(r) => panic!("rejected: {r}"),
                _ => {}
            }
        }
    }

    /// Two clients over real WebSockets: each drives, each sees the other's
    /// tank move; acks rise; bots fill the other seats.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn arena_loopback() {
        let url = start_server().await;
        let (a, code, id_a) = join(&url, "Ann", Join::Host { public: false, mode: ModeKind::ScrapBrawl, map: MapId::Refinery }).await;
        let (b, code_b, id_b) = join(&url, "Bob", Join::Code(code.to_lowercase())).await;
        assert_eq!(code, code_b);
        assert_ne!(id_a, id_b);

        let drive = |ws: Ws, other: PlayerId| async move {
            let (mut tx, mut rx) = ws.split();
            let mut seq = 0;
            let mut first: Option<Vec3> = None;
            let mut last = Vec3::ZERO;
            let mut ack = 0;
            let mut roster = 0;
            let mut every = tokio::time::interval(Duration::from_micros(16_667));
            let end = tokio::time::Instant::now() + Duration::from_secs(4);
            while tokio::time::Instant::now() < end {
                tokio::select! {
                    _ = every.tick() => {
                        seq += 1;
                        let input = Input { throttle: 1.0, steer: 0.2, ..Input::default() };
                        tx.send(WsMsg::Binary(encode(&ClientMsg::Input { seq, input }).into())).await.unwrap();
                    }
                    msg = recv(&mut rx) => match msg {
                        ServerMsg::Tick { ack: a, snapshot, .. } => {
                            ack = a;
                            if let Some(t) = snapshot.tank(other) {
                                first.get_or_insert(t.position());
                                last = t.position();
                            }
                        }
                        ServerMsg::Players(p) => roster = p.len(),
                        _ => {}
                    }
                }
            }
            (first.expect("saw the other tank").distance(last), ack, seq, roster)
        };
        let (ra, rb) = tokio::join!(tokio::spawn(drive(a, id_b)), tokio::spawn(drive(b, id_a)));
        for (moved, ack, sent, roster) in [ra.unwrap(), rb.unwrap()] {
            assert!(moved > 3.0, "other tank moved only {moved} m");
            assert!(ack > sent / 3, "acked {ack} of {sent}");
            assert_eq!(roster, arena_core::net::ROOM_SEATS);
        }
    }

    #[test]
    fn hashed_names() {
        assert!(is_hashed("/arena-client-f6b75cdc2442c127.js"));
        assert!(is_hashed("/arena-client-f6b75cdc2442c127_bg.wasm"));
        assert!(!is_hashed("/index.html"));
        assert!(!is_hashed("/tank-green.png"));
        assert!(!is_hashed("/api/arena/rooms"));
    }

    /// One HTTP request to the test server; returns the status code and body.
    async fn http(addr: &str, req: String) -> (u16, String) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        let code = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        (code, out)
    }

    fn post(addr: &str, body: &str, from: &str) -> String {
        format!(
            "POST /api/ideas HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nX-Forwarded-For: {from}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    /// The idea box over HTTP: posts land, bad ones and floods bounce, the
    /// listing needs the token.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn idea_box_endpoint() {
        let dir = std::env::temp_dir().join(format!("tansk-ideas-http-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let app = App::with_ideas(1, ideas::Ideas::new(&dir, Some("tok".into())));
        tokio::spawn(async move { axum::serve(listener, router(app, "no-such-dir")).await });
        let good = r#"{"category":"tuning","text":"Big Bore reloads too slow","name":"Ann","version":"0.1.0","map":"sink","mode":"ScrapBrawl"}"#;
        let (code, body) = http(&addr, post(&addr, good, "10.0.0.1")).await;
        assert_eq!(code, 201, "{body}");
        assert!(body.contains("Thanks, the crew will look at it."));
        assert_eq!(http(&addr, post(&addr, r#"{"category":"meh","text":"hello"}"#, "10.0.0.1")).await.0, 400);
        assert_eq!(http(&addr, post(&addr, "not json", "10.0.0.1")).await.0, 400);
        let huge = format!(r#"{{"category":"idea","text":"{}"}}"#, "x".repeat(ideas::MAX_BODY));
        assert_eq!(http(&addr, post(&addr, &huge, "10.0.0.1")).await.0, 413);
        for _ in 1..ideas::PER_HOUR {
            assert_eq!(http(&addr, post(&addr, good, "10.0.0.1")).await.0, 201);
        }
        assert_eq!(http(&addr, post(&addr, good, "10.0.0.1")).await.0, 429, "rate limited");
        assert_eq!(http(&addr, post(&addr, good, "10.0.0.2")).await.0, 201, "another address is fine");
        let get = |auth: &str| format!("GET /api/ideas HTTP/1.1\r\nHost: {addr}\r\n{auth}Connection: close\r\n\r\n");
        assert_eq!(http(&addr, get("")).await.0, 401);
        assert_eq!(http(&addr, get("Authorization: Bearer nope\r\n")).await.0, 401);
        let (code, body) = http(&addr, get("Authorization: Bearer tok\r\n")).await;
        assert_eq!(code, 200);
        assert_eq!(body.matches("Big Bore reloads too slow").count(), ideas::PER_HOUR + 1, "{body}");
        assert!(!body.contains("10.0.0."), "no addresses");
        let _ = std::fs::remove_dir_all(&dir);

        // No token set: no listing at all.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener.local_addr().unwrap().to_string();
        let app = App::with_ideas(1, ideas::Ideas::new(std::env::temp_dir().join("tansk-ideas-off"), None));
        tokio::spawn(async move { axum::serve(listener, router(app, "no-such-dir")).await });
        let req = format!("GET /api/ideas HTTP/1.1\r\nHost: {addr2}\r\nAuthorization: Bearer \r\nConnection: close\r\n\r\n");
        assert_eq!(http(&addr2, req).await.0, 404);
    }

    /// Two quick-match players land in the same public room, listed in the lobby.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn arena_quick_match() {
        let url = start_server().await;
        let (_a, code_a, _) = join(&url, "Ann", Join::Play).await;
        let (_b, code_b, _) = join(&url, "Bob", Join::Play).await;
        assert_eq!(code_a, code_b);
        let (mut c, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        send(&mut c, &ClientMsg::Hello { name: "Cy".into(), join: Join::Code("ZZZZ".into()) }).await;
        assert!(matches!(recv(&mut c).await, ServerMsg::Rejected(_)));
        let http = url.replace("ws://", "").replace("/ws/arena", "");
        let mut s = tokio::net::TcpStream::connect(&http).await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        s.write_all(format!("GET /api/arena/rooms HTTP/1.1\r\nHost: {http}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).await.unwrap();
        assert!(body.contains(&format!("\"code\":\"{code_a}\"")) && body.contains("\"humans\":2"), "{body}");
    }
}
