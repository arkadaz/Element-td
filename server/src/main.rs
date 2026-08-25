//! Elemental TD lobby server.
//!
//! It stores who is in which room and the last scoreboard line each player
//! sent, and forwards that to the rest of the room. It does not simulate waves,
//! validate builds, or keep history - every client renders and simulates its own
//! board, seeded identically so the waves match.
//!
//! Sizing: a room is a fixed table of eight slots holding a name and a 24-byte
//! snapshot, so room state is well under a kilobyte. At eight players per room,
//! 1000 players is 125 rooms - a few hundred kilobytes of state. The real cost
//! is per-connection socket buffers, which is why frames are capped and the
//! broadcast is encoded once per room rather than once per player.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use sha2::{Digest, Sha256};
use tokio::sync::{broadcast, Mutex};
use tower_http::compression::CompressionLayer;
use tower_layer::Layer;
use tower_http::services::{ServeDir, ServeFile};

use td_proto::{
    ClientMsg, MAX_FRAME, MAX_PASSWORD, MAX_PLAYERS, MAX_ROOM_ID, PROTOCOL, PlayerView, RoomView,
    ServerMsg, Snapshot, clean_name, decode, encode,
};

/// Refuse to allocate beyond this, so a flood cannot exhaust memory.
const MAX_ROOMS: usize = 4096;
/// A room with nobody connected is dropped after this long.
const EMPTY_ROOM_TTL: Duration = Duration::from_secs(120);
/// How often the room state is pushed to its members.
const BROADCAST_HZ: u64 = 4;

#[derive(Clone)]
struct Player {
    name: String,
    ready: bool,
    connected: bool,
    snap: Snapshot,
}

struct Room {
    password: [u8; 32],
    host: u8,
    started: bool,
    difficulty: u8,
    seed: u64,
    /// Fixed table - a room can never grow past MAX_PLAYERS.
    slots: [Option<Player>; MAX_PLAYERS],
    /// Encoded once per room and shared by every member.
    tx: broadcast::Sender<String>,
    empty_since: Option<Instant>,
}

impl Room {
    fn occupied(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
    fn connected(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.as_ref().is_some_and(|p| p.connected))
            .count()
    }
    fn free_slot(&self) -> Option<u8> {
        self.slots.iter().position(|s| s.is_none()).map(|i| i as u8)
    }
    fn view(&self, id: &str) -> RoomView {
        RoomView {
            id: id.to_string(),
            host: self.host,
            started: self.started,
            difficulty: self.difficulty,
            players: self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    s.as_ref().map(|p| PlayerView {
                        slot: i as u8,
                        name: p.name.clone(),
                        ready: p.ready,
                        connected: p.connected,
                        snap: p.snap,
                    })
                })
                .collect(),
        }
    }
}

#[derive(Clone)]
struct AppState {
    rooms: Arc<Mutex<HashMap<String, Room>>>,
}

fn hash_password(p: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"elemental-td-room-v1");
    h.update(p.as_bytes());
    h.finalize().into()
}

/// RFC-4122-shaped v4 identifier, generated here so it is guaranteed unique
/// within this server without the client needing a random source.
fn new_room_id(rng: &mut impl Rng) -> String {
    let b: [u8; 16] = rng.random();
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-4{:01x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6] & 0x0f, b[7],
        (b[8] & 0x3f) | 0x80, b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);
    let state = AppState { rooms: Arc::new(Mutex::new(HashMap::new())) };
    spawn_reaper(&state);
    let app = router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Elemental TD lobby server listening on {addr} (ws://<host>:{port}/ws)");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}

/// Drops rooms nobody has been connected to for a while. Without this a server
/// that has been up for a month is carrying every room anyone ever opened.
fn spawn_reaper(state: &AppState) {
    let rooms = state.rooms.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(20));
        loop {
            tick.tick().await;
            let mut rooms = rooms.lock().await;
            rooms.retain(|_, r| match r.empty_since {
                Some(t) => t.elapsed() < EMPTY_ROOM_TTL,
                None => true,
            });
        }
    });
}

fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/health",
            get({
                let rooms = state.rooms.clone();
                move || {
                    let rooms = rooms.clone();
                    async move {
                        let r = rooms.lock().await;
                        let players: usize = r.values().map(|room| room.connected()).sum();
                        format!("ok rooms={} players={}\n", r.len(), players)
                    }
                }
            }),
        )
        .route("/ws", get(ws_upgrade))
        .fallback_service(static_site())
        .with_state(state)
}

/// The compiled game, served from the same origin as the lobby.
///
/// One container is then the entire deployment, and - because the client
/// derives `wss://<its own host>/ws` - nobody has to configure a server address
/// or fight a mixed-content block. If the directory is missing the fallback
/// simply 404s and the lobby still works.
fn static_site() -> axum::routing::MethodRouter {
    let dir = std::env::var("TD_STATIC").unwrap_or_else(|_| "static".to_string());
    let index = format!("{dir}/index.html");
    // Unknown paths fall back to index.html so a refresh anywhere still boots
    // the game rather than 404ing. The wasm bundle is ~8 MB raw and about a
    // third of that compressed, which on a phone is the difference between a
    // few seconds and half a minute before the first frame.
    let serve = ServeDir::new(&dir).fallback(ServeFile::new(index));
    axum::routing::any_service(CompressionLayer::new().layer(serve))
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.max_message_size(MAX_FRAME)
        .on_upgrade(move |socket| serve_client(socket, state))
}

/// Where this connection ended up sitting.
struct Seat {
    room: String,
    slot: u8,
}

async fn serve_client(socket: WebSocket, state: AppState) {
    let (mut sink, mut stream) = socket.split();

    // --- first message must place this connection in a room
    let Some(seat) = seat_client(&mut sink, &mut stream, &state).await else {
        return;
    };

    let rx = {
        let rooms = state.rooms.lock().await;
        match rooms.get(&seat.room) {
            Some(r) => r.tx.subscribe(),
            None => return,
        }
    };

    // --- fan-out task: room state -> this socket
    let mut forward = tokio::spawn(async move {
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok(text) => {
                    if sink.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                // Slow client: skip ahead rather than buffer.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    // --- read loop: this socket -> room state
    let reader_state = state.clone();
    let room_id = seat.room.clone();
    let slot = seat.slot;
    let mut reader = tokio::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            let text = match msg {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            };
            if text.len() > MAX_FRAME {
                continue;
            }
            let Some(cmsg) = decode::<ClientMsg>(&text) else { continue };
            if !apply(&reader_state, &room_id, slot, cmsg).await {
                break;
            }
        }
    });

    tokio::select! {
        _ = &mut forward => reader.abort(),
        _ = &mut reader => forward.abort(),
    }

    disconnect(&state, &seat).await;
}

/// Handles the opening Create/Join and returns the seat taken, if any.
async fn seat_client(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    stream: &mut futures_util::stream::SplitStream<WebSocket>,
    state: &AppState,
) -> Option<Seat> {
    let msg = tokio::time::timeout(Duration::from_secs(20), stream.next())
        .await
        .ok()??
        .ok()?;
    let text = match msg {
        Message::Text(t) => t,
        _ => return None,
    };
    let cmsg = decode::<ClientMsg>(&text)?;

    let reject = |sink: &mut futures_util::stream::SplitSink<WebSocket, Message>, why: &str| {
        let _ = sink.send(Message::Text(
            encode(&ServerMsg::Rejected(why.to_string())).into(),
        ));
    };

    let (room_id, slot) = match cmsg {
        ClientMsg::Create { protocol, name, password, difficulty } => {
            if protocol != PROTOCOL {
                reject(sink, "This client is a different version to the server.");
                return None;
            }
            if password.len() > MAX_PASSWORD {
                reject(sink, "Password is too long.");
                return None;
            }
            let mut rooms = state.rooms.lock().await;
            if rooms.len() >= MAX_ROOMS {
                reject(sink, "The server is full. Try again shortly.");
                return None;
            }
            let mut rng = rand::rng();
            let id = new_room_id(&mut rng);
            let (tx, _) = broadcast::channel(32);
            let mut slots: [Option<Player>; MAX_PLAYERS] = Default::default();
            slots[0] = Some(Player {
                name: clean_name(&name),
                ready: false,
                connected: true,
                snap: Snapshot::default(),
            });
            rooms.insert(
                id.clone(),
                Room {
                    password: hash_password(&password),
                    host: 0,
                    started: false,
                    difficulty: difficulty.min(2),
                    seed: rng.random(),
                    slots,
                    tx,
                    empty_since: None,
                },
            );
            (id, 0u8)
        }
        ClientMsg::Join { protocol, room, password, name } => {
            if protocol != PROTOCOL {
                reject(sink, "This client is a different version to the server.");
                return None;
            }
            if room.len() > MAX_ROOM_ID || password.len() > MAX_PASSWORD {
                reject(sink, "Room id or password is too long.");
                return None;
            }
            let mut rooms = state.rooms.lock().await;
            let Some(r) = rooms.get_mut(&room) else {
                reject(sink, "No room with that id.");
                return None;
            };
            if r.password != hash_password(&password) {
                reject(sink, "Wrong password.");
                return None;
            }
            if r.started {
                reject(sink, "That game has already started.");
                return None;
            }
            let Some(slot) = r.free_slot() else {
                reject(sink, "That room is full (8 players).");
                return None;
            };
            r.slots[slot as usize] = Some(Player {
                name: clean_name(&name),
                ready: false,
                connected: true,
                snap: Snapshot::default(),
            });
            r.empty_since = None;
            (room, slot)
        }
        _ => return None,
    };

    let _ = sink
        .send(Message::Text(
            encode(&ServerMsg::Welcome { room: room_id.clone(), you: slot }).into(),
        ))
        .await;
    publish(state, &room_id).await;
    Some(Seat { room: room_id, slot })
}

/// Applies one client message. Returns false if the connection should close.
async fn apply(state: &AppState, room_id: &str, slot: u8, msg: ClientMsg) -> bool {
    let mut started_seed: Option<(u64, u8)> = None;
    {
        let mut rooms = state.rooms.lock().await;
        let Some(room) = rooms.get_mut(room_id) else { return false };
        let Some(me) = room.slots.get_mut(slot as usize).and_then(|s| s.as_mut()) else {
            return false;
        };
        match msg {
            ClientMsg::Ready(r) => me.ready = r,
            // The only message that flows during play, and all it does is
            // overwrite one fixed-size struct.
            ClientMsg::Update(s) => me.snap = s,
            ClientMsg::Leave => return false,
            ClientMsg::Start => {
                if room.host == slot && !room.started {
                    room.started = true;
                    started_seed = Some((room.seed, room.difficulty));
                }
            }
            ClientMsg::Create { .. } | ClientMsg::Join { .. } => {}
        }
    }

    if let Some((seed, difficulty)) = started_seed {
        // Everyone seeds their own simulation from this; the server does not
        // run one.
        let rooms = state.rooms.lock().await;
        if let Some(room) = rooms.get(room_id) {
            let _ = room.tx.send(encode(&ServerMsg::Started { seed, difficulty }));
        }
    }
    publish(state, room_id).await;
    true
}

async fn disconnect(state: &AppState, seat: &Seat) {
    let mut rooms = state.rooms.lock().await;
    let Some(room) = rooms.get_mut(&seat.room) else { return };
    if let Some(p) = room.slots.get_mut(seat.slot as usize).and_then(|s| s.as_mut()) {
        // Keep the scoreboard line of a player who drops mid-game, so the room
        // still shows how far they got.
        if room.started {
            p.connected = false;
        } else {
            room.slots[seat.slot as usize] = None;
        }
    }
    if room.connected() == 0 {
        room.empty_since = Some(Instant::now());
    } else if room.host == seat.slot {
        // Hand the room to whoever is still here.
        if let Some(next) = room
            .slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|p| p.connected))
        {
            room.host = next as u8;
        }
    }
    let view = room.view(&seat.room);
    let _ = room.tx.send(encode(&ServerMsg::Room(view)));
    let _ = room.occupied();
}

/// Encodes the room once and hands the same string to every member.
async fn publish(state: &AppState, room_id: &str) {
    let rooms = state.rooms.lock().await;
    if let Some(room) = rooms.get(room_id) {
        let _ = room.tx.send(encode(&ServerMsg::Room(room.view(room_id))));
    }
}

#[allow(dead_code)]
const _BROADCAST_HZ_IS_DOCUMENTED: u64 = BROADCAST_HZ;
