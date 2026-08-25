//! Lobby client.
//!
//! The server never simulates anything - it hands out a room id, checks a
//! password, and relays one scoreboard line per player. Everything here is
//! therefore *optional*: single player never touches this module, and if the
//! socket dies mid-run the game keeps playing, it just stops sharing scores.
//!
//! Traffic is deliberately tiny. A snapshot goes out at [`PUSH_HZ`], and only
//! when something in it actually changed, so eight idle players cost nothing.

use ewebsock::{WsEvent, WsMessage, WsReceiver, WsSender};
use td_proto::{ClientMsg, PROTOCOL, RoomView, ServerMsg, Snapshot, encode, decode};

/// How often a snapshot may leave the client, at most.
const PUSH_HZ: f32 = 2.0;

#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    /// Single player: nothing is connected and nothing will be.
    Offline,
    /// Socket opening, or waiting for the server to answer Create/Join.
    Connecting,
    /// In a room, waiting for the host.
    Lobby,
    /// The host started; everyone is playing their own board.
    Playing,
    /// Connection refused, dropped, or rejected. Carries what to tell the player.
    Failed(String),
}

impl Status {
    pub fn is_busy(&self) -> bool {
        matches!(self, Status::Connecting)
    }
    pub fn is_live(&self) -> bool {
        matches!(self, Status::Lobby | Status::Playing)
    }
}

/// What the app has to react to. Everything else is absorbed in here.
pub enum Event {
    /// The host started the run: seed the simulation with this and play.
    Started { seed: u64, difficulty: u8 },
}

#[derive(Default)]
pub struct Net {
    sender: Option<WsSender>,
    receiver: Option<WsReceiver>,
    /// Held until the socket opens, then sent as the first frame.
    pending: Option<ClientMsg>,
    pub status: Status,
    pub room: Option<RoomView>,
    /// Our slot in the room table.
    pub you: u8,
    /// The address we last dialled, kept so the UI can show and reuse it.
    pub url: String,
    last_sent: Snapshot,
    push_timer: f32,
}

impl Default for Status {
    fn default() -> Self {
        Status::Offline
    }
}

impl Net {
    /// Where the lobby server lives.
    ///
    /// Baked in at build time with `TD_SERVER` when there is one; otherwise it
    /// is guessed from the page the game was served from, which is right when
    /// the game and the server share a host and harmless when they do not - the
    /// player can always type a different address in the menu.
    pub fn default_url() -> String {
        if let Some(u) = option_env!("TD_SERVER") {
            return u.to_string();
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(w) = web_sys::window() {
                // ?server=... wins, so one build can point anywhere.
                if let Ok(search) = w.location().search() {
                    if let Some(v) = query_param(&search, "server") {
                        return v;
                    }
                }
                let proto = w.location().protocol().unwrap_or_default();
                let host = w.location().host().unwrap_or_default();
                if !host.is_empty() {
                    let scheme = if proto.starts_with("https") { "wss" } else { "ws" };
                    return format!("{scheme}://{host}/ws");
                }
            }
        }
        "ws://127.0.0.1:8080/ws".to_string()
    }

    pub fn is_online(&self) -> bool {
        self.status.is_live()
    }

    /// True when this client owns the room and may press Start.
    pub fn is_host(&self) -> bool {
        self.room.as_ref().is_some_and(|r| r.host == self.you)
    }

    pub fn players(&self) -> usize {
        self.room.as_ref().map_or(0, |r| r.players.len())
    }

    // ------------------------------------------------ connecting

    pub fn create(&mut self, url: &str, name: &str, password: &str, difficulty: u8) {
        self.dial(
            url,
            ClientMsg::Create {
                protocol: PROTOCOL,
                name: name.to_string(),
                password: password.to_string(),
                difficulty,
            },
        );
    }

    pub fn join(&mut self, url: &str, room: &str, password: &str, name: &str) {
        self.dial(
            url,
            ClientMsg::Join {
                protocol: PROTOCOL,
                room: room.trim().to_string(),
                password: password.to_string(),
                name: name.to_string(),
            },
        );
    }

    fn dial(&mut self, url: &str, first: ClientMsg) {
        self.disconnect();
        let url = url.trim().to_string();
        if url.is_empty() {
            self.status = Status::Failed("No server address".into());
            return;
        }
        let opts = ewebsock::Options {
            max_incoming_frame_size: td_proto::MAX_FRAME * 8,
            ..Default::default()
        };
        match ewebsock::connect(&url, opts) {
            Ok((sender, receiver)) => {
                self.sender = Some(sender);
                self.receiver = Some(receiver);
                self.pending = Some(first);
                self.status = Status::Connecting;
                self.url = url;
            }
            Err(e) => self.status = Status::Failed(short_error(&e)),
        }
    }

    /// Drops the socket without touching `status`, so callers decide what the
    /// player is told.
    fn disconnect(&mut self) {
        self.sender = None;
        self.receiver = None;
        self.pending = None;
        self.room = None;
        self.you = 0;
        self.last_sent = Snapshot::default();
    }

    /// Leaves the room politely and goes back to being offline.
    pub fn leave(&mut self) {
        self.send(&ClientMsg::Leave);
        self.disconnect();
        self.status = Status::Offline;
    }

    // ------------------------------------------------ traffic

    fn send(&mut self, msg: &ClientMsg) {
        if let Some(s) = self.sender.as_mut() {
            s.send(WsMessage::Text(encode(msg)));
        }
    }

    pub fn set_ready(&mut self, ready: bool) {
        self.send(&ClientMsg::Ready(ready));
    }

    pub fn start(&mut self) {
        self.send(&ClientMsg::Start);
    }

    /// Shares the scoreboard line. Rate-limited, and skipped entirely when
    /// nothing has changed - a paused player sends no traffic at all.
    pub fn push(&mut self, snap: Snapshot, dt: f32) {
        if self.status != Status::Playing {
            return;
        }
        // Clamped at zero: a change after a quiet spell goes out at once, but
        // idle time never banks credit for a burst of frames later.
        self.push_timer = (self.push_timer - dt).max(0.0);
        if self.push_timer > 0.0 || snap == self.last_sent {
            return;
        }
        self.push_timer = 1.0 / PUSH_HZ;
        self.last_sent = snap;
        self.send(&ClientMsg::Update(snap));
    }

    /// Drains the socket. Call once a frame; returns anything the app must act on.
    pub fn poll(&mut self) -> Option<Event> {
        let mut out = None;
        loop {
            let Some(rx) = self.receiver.as_ref() else { break };
            let Some(ev) = rx.try_recv() else { break };
            match ev {
                WsEvent::Opened => {
                    if let Some(first) = self.pending.take() {
                        self.send(&first);
                    }
                }
                WsEvent::Message(WsMessage::Text(t)) => {
                    if let Some(m) = decode::<ServerMsg>(&t) {
                        if let Some(e) = self.handle(m) {
                            out = Some(e);
                        }
                    }
                }
                WsEvent::Message(_) => {}
                WsEvent::Error(e) => {
                    self.disconnect();
                    self.status = Status::Failed(short_error(&e));
                }
                WsEvent::Closed => {
                    // Dropping mid-run must not end the run: the game is local.
                    let msg = if self.status == Status::Playing {
                        "Disconnected - your run continues offline"
                    } else {
                        "Connection closed"
                    };
                    self.disconnect();
                    self.status = Status::Failed(msg.into());
                }
            }
        }
        out
    }

    fn handle(&mut self, m: ServerMsg) -> Option<Event> {
        match m {
            ServerMsg::Welcome { room, you } => {
                self.you = you;
                self.status = Status::Lobby;
                if self.url.is_empty() {
                    self.url = room;
                }
                None
            }
            ServerMsg::Room(r) => {
                if r.started && self.status == Status::Lobby {
                    self.status = Status::Playing;
                }
                self.room = Some(r);
                None
            }
            ServerMsg::Started { seed, difficulty } => {
                self.status = Status::Playing;
                self.push_timer = 0.0;
                self.last_sent = Snapshot::default();
                Some(Event::Started { seed, difficulty })
            }
            ServerMsg::Rejected(why) => {
                self.disconnect();
                self.status = Status::Failed(why);
                None
            }
        }
    }

    /// The room id players share to invite each other.
    pub fn room_id(&self) -> &str {
        self.room.as_ref().map_or("", |r| r.id.as_str())
    }
}

/// Socket errors are long and full of addresses; the player needs one line.
fn short_error(e: &str) -> String {
    let first = e.lines().next().unwrap_or(e).trim();
    let mut s: String = first.chars().take(90).collect();
    if s.is_empty() {
        s = "Could not reach the server".into();
    }
    s
}

#[cfg(target_arch = "wasm32")]
fn query_param(search: &str, key: &str) -> Option<String> {
    for pair in search.trim_start_matches('?').split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key && !v.is_empty() {
            return Some(v.replace("%3A", ":").replace("%2F", "/"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_client_is_offline_and_silent() {
        let n = Net::default();
        assert_eq!(n.status, Status::Offline);
        assert!(!n.is_online());
        assert!(!n.is_host(), "nobody hosts a room they are not in");
        assert_eq!(n.players(), 0);
        assert_eq!(n.room_id(), "");
    }

    #[test]
    fn an_unreachable_server_fails_instead_of_hanging() {
        let mut n = Net::default();
        n.create("", "Ark", "pw", 0);
        assert!(matches!(n.status, Status::Failed(_)), "empty address must fail");
    }

    #[test]
    fn errors_are_trimmed_to_one_readable_line() {
        let long = format!("boom\nstack trace\n{}", "x".repeat(400));
        let s = short_error(&long);
        assert_eq!(s, "boom");
        assert!(short_error("").len() > 0, "an empty error still needs words");
        assert!(short_error(&"y".repeat(500)).len() <= 90);
    }

    #[test]
    fn snapshots_are_rate_limited_and_deduplicated() {
        let mut n = Net::default();
        // Offline clients never push, whatever happens.
        n.push(Snapshot { wave: 3, ..Default::default() }, 1.0);
        assert_eq!(n.last_sent, Snapshot::default());

        n.status = Status::Playing;
        let snap = Snapshot { wave: 3, ..Default::default() };
        n.push(snap, 0.0);
        assert_eq!(n.last_sent, snap, "the first change goes out immediately");

        // The same line again: nothing to say, so nothing is said.
        n.push(snap, 0.1);
        assert_eq!(n.last_sent, snap);

        // A new line, but too soon after the last one.
        let next = Snapshot { wave: 4, ..Default::default() };
        n.push(next, 0.1);
        assert_eq!(n.last_sent, snap, "pushes must respect the rate limit");

        // Once the window has passed it goes out.
        n.push(next, 1.0 / PUSH_HZ);
        assert_eq!(n.last_sent, next);

        // Idle time must not bank credit for a later burst.
        n.push(next, 3600.0);
        let a = Snapshot { wave: 5, ..Default::default() };
        let b = Snapshot { wave: 6, ..Default::default() };
        n.push(a, 0.0);
        n.push(b, 0.0);
        assert_eq!(n.last_sent, a, "an hour of quiet does not buy two sends at once");
    }
}
