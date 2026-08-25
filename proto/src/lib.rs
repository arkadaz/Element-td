//! Wire format shared by the game and the lobby server.
//!
//! The server is deliberately dumb: it stores who is in a room and the last
//! scoreboard line each of them sent, and it forwards that to everyone else.
//! It never simulates a wave, never validates a tower placement, and keeps no
//! history. Everything below is sized so a room costs roughly a kilobyte.

use serde::{Deserialize, Serialize};

/// Bumped whenever the messages below change shape.
pub const PROTOCOL: u16 = 1;

/// A room is a small fixed table, not a growable list.
pub const MAX_PLAYERS: usize = 8;

/// Hard caps, so a malicious client cannot make the server allocate.
pub const MAX_NAME: usize = 20;
pub const MAX_ROOM_ID: usize = 40;
pub const MAX_PASSWORD: usize = 64;
/// Longest frame the server will accept, in bytes.
pub const MAX_FRAME: usize = 2048;

/// Everything the other players need to see about one player. This is the only
/// thing that travels while a game is running, at a couple of times a second.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub wave: u16,
    pub lives: i16,
    pub gold: i32,
    pub net_worth: i32,
    pub kills: u32,
    pub leaked: u16,
    pub towers: u16,
    /// False once the player has been overrun.
    pub alive: bool,
    /// True once they have cleared the campaign and carried on.
    pub endless: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerView {
    pub slot: u8,
    pub name: String,
    pub ready: bool,
    pub connected: bool,
    pub snap: Snapshot,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RoomView {
    pub id: String,
    pub host: u8,
    pub started: bool,
    pub difficulty: u8,
    pub players: Vec<PlayerView>,
}

impl RoomView {
    pub fn player(&self, slot: u8) -> Option<&PlayerView> {
        self.players.iter().find(|p| p.slot == slot)
    }
    /// Everyone still standing, best wave first - the scoreboard order.
    pub fn ranked(&self) -> Vec<&PlayerView> {
        let mut v: Vec<&PlayerView> = self.players.iter().collect();
        v.sort_by(|a, b| {
            b.snap
                .alive
                .cmp(&a.snap.alive)
                .then(b.snap.wave.cmp(&a.snap.wave))
                .then(b.snap.net_worth.cmp(&a.snap.net_worth))
        });
        v
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientMsg {
    /// Open a new room. The server allocates the id; the password is chosen here.
    Create {
        protocol: u16,
        name: String,
        password: String,
        difficulty: u8,
    },
    /// Enter an existing room by id and password.
    Join {
        protocol: u16,
        room: String,
        password: String,
        name: String,
    },
    Ready(bool),
    /// Host only. Locks the room and deals everyone the same seed.
    Start,
    /// Sent a couple of times a second while playing.
    Update(Snapshot),
    Leave,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerMsg {
    /// You are in. `you` is your slot in the room table.
    Welcome { room: String, you: u8 },
    /// The room changed: someone joined, left, readied, or sent a snapshot.
    Room(RoomView),
    /// The host started. Every client seeds its own simulation with this, which
    /// is what makes the waves identical without the server simulating anything.
    Started { seed: u64, difficulty: u8 },
    /// Wrong password, room full, room gone, protocol mismatch.
    Rejected(String),
}

/// Trims and sanitises a display name. Never allocates beyond [`MAX_NAME`].
pub fn clean_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Player".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn encode<T: Serialize>(msg: &T) -> String {
    serde_json::to_string(msg).unwrap_or_default()
}

pub fn decode<T: for<'a> Deserialize<'a>>(text: &str) -> Option<T> {
    serde_json::from_str(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_bounded_and_never_empty() {
        assert_eq!(clean_name("   "), "Player");
        assert_eq!(clean_name(""), "Player");
        assert_eq!(clean_name("  Ark  "), "Ark");
        assert!(clean_name(&"x".repeat(500)).len() <= MAX_NAME);
        assert!(!clean_name("bad\u{0}name").contains('\u{0}'));
    }

    #[test]
    fn a_full_room_stays_small_on_the_wire() {
        let players: Vec<PlayerView> = (0..MAX_PLAYERS as u8)
            .map(|slot| PlayerView {
                slot,
                name: "WWWWWWWWWWWWWWWWWWWW".into(),
                ready: true,
                connected: true,
                snap: Snapshot {
                    wave: 999,
                    lives: 20,
                    gold: 999_999,
                    net_worth: 9_999_999,
                    kills: 999_999,
                    leaked: 999,
                    towers: 300,
                    alive: true,
                    endless: true,
                },
            })
            .collect();
        let view = RoomView {
            id: "8f4c1d2e-1111-2222-3333-444455556666".into(),
            host: 0,
            started: true,
            difficulty: 2,
            players,
        };
        let wire = encode(&ServerMsg::Room(view));
        // A full room broadcast at 2 Hz is what sets the server's bandwidth
        // floor; keep an eye on it.
        assert!(wire.len() < MAX_FRAME, "room frame is {} bytes", wire.len());
    }

    #[test]
    fn the_scoreboard_puts_the_living_and_the_furthest_first() {
        let mk = |slot: u8, wave: u16, alive: bool| PlayerView {
            slot,
            name: format!("p{slot}"),
            ready: true,
            connected: true,
            snap: Snapshot { wave, alive, ..Default::default() },
        };
        let view = RoomView {
            id: "r".into(),
            host: 0,
            started: true,
            difficulty: 0,
            players: vec![mk(0, 30, false), mk(1, 12, true), mk(2, 25, true)],
        };
        let ranked = view.ranked();
        assert_eq!(ranked[0].slot, 2, "furthest living player should lead");
        assert_eq!(ranked[1].slot, 1);
        assert_eq!(ranked[2].slot, 0, "the dead rank last however far they got");
    }

    #[test]
    fn messages_round_trip() {
        let m = ClientMsg::Join {
            protocol: PROTOCOL,
            room: "abc".into(),
            password: "hunter2".into(),
            name: "Ark".into(),
        };
        let wire = encode(&m);
        let back: ClientMsg = decode(&wire).expect("round trip");
        matches!(back, ClientMsg::Join { .. });
    }
}
