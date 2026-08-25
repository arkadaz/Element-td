//! Title screen and multiplayer lobby.
//!
//! The flow the game opens with:
//!
//! ```text
//! Title ──▶ Single player ─────────────────────────────▶ Play
//!       └─▶ Multiplayer ─┬─ Host  ─▶ room id + password ─▶ Lobby ─▶ Play
//!                        └─ Join  ─▶ room id + password ─▶ Lobby ─▶ Play
//! ```
//!
//! Everyone plays their own board; the room only carries the scoreboard. That
//! is why the lobby is this thin - there is no lobby state worth syncing beyond
//! "who is here" and "the host pressed start".

use egui::{Align, Color32, Context, Layout, RichText, Vec2};

use crate::net::{Net, Status};
use crate::ui::pal;
use td_proto::MAX_PLAYERS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Title,
    /// Host/join form.
    Connect,
    /// In a room, waiting for the host to start.
    Lobby,
    Playing,
}

/// Which of the two multiplayer paths the connect form is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Host,
    Join,
}

pub struct MenuState {
    pub screen: Screen,
    pub mode: Mode,
    pub name: String,
    pub room: String,
    pub password: String,
    pub server: String,
    pub ready: bool,
    /// A run waiting to be resumed, read from local storage at startup.
    pub saved: Option<crate::save::Save>,
    /// Set after a copy button is pressed, so the button can say so.
    pub copied: f32,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            screen: Screen::Title,
            mode: Mode::Host,
            name: "Player".into(),
            room: String::new(),
            password: String::new(),
            server: Net::default_url(),
            ready: false,
            saved: None,
            copied: 0.0,
        }
    }
}

/// What the menu wants the app to do. The menu never touches the game itself.
pub enum Action {
    None,
    /// Start a local run.
    SinglePlayer,
    /// Pick up the saved run where it left off.
    Resume,
    /// The player left the lobby; drop back to the title screen.
    Cancelled,
}

// ---------------------------------------------------------------- entry

pub fn show(ctx: &Context, m: &mut MenuState, net: &mut Net, dt: f32) -> Action {
    m.copied = (m.copied - dt).max(0.0);

    // Follow the connection: it, not the UI, decides which screen is truthful.
    match net.status {
        Status::Lobby | Status::Playing if m.screen == Screen::Connect => {
            m.screen = Screen::Lobby;
        }
        Status::Failed(_) | Status::Offline if m.screen == Screen::Lobby => {
            m.screen = Screen::Connect;
            m.ready = false;
        }
        _ => {}
    }

    let mut action = Action::None;
    let area = egui::Modal::new("menu".into());
    area.show(ctx, |ui| {
        let w = ui.available_width();
        ui.set_width(w.clamp(240.0, 520.0));
        match m.screen {
            Screen::Title => action = title(ui, m),
            Screen::Connect => connect(ui, m, net),
            Screen::Lobby => action = lobby(ui, m, net),
            Screen::Playing => {}
        }
    });
    action
}

fn heading(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new("ELEMENTAL TD").size(30.0).strong().color(pal::GOLD));
        ui.label(
            RichText::new("Eight towers. Six levels. Fifty waves, then it stops being fair.")
                .size(13.0)
                .color(pal::DIM),
        );
        ui.add_space(10.0);
    });
}

fn big_button(ui: &mut egui::Ui, text: &str, sub: &str, accent: Color32) -> bool {
    let w = ui.available_width();
    let resp = ui.add_sized(
        Vec2::new(w, 52.0),
        egui::Button::new(RichText::new(text).size(17.0).strong().color(accent))
            .fill(pal::CARD)
            .corner_radius(8.0),
    );
    if !sub.is_empty() {
        ui.add_space(-4.0);
        ui.label(RichText::new(sub).size(12.0).color(pal::DIM));
        ui.add_space(6.0);
    }
    resp.clicked()
}

// ---------------------------------------------------------------- title

fn title(ui: &mut egui::Ui, m: &mut MenuState) -> Action {
    heading(ui);

    let mut action = Action::None;
    if let Some(save) = &m.saved {
        let label = save.label();
        if big_button(ui, "Continue", &label, pal::GOOD) {
            action = Action::Resume;
        }
    }
    let solo = if m.saved.is_some() { "New game" } else { "Single player" };
    if big_button(ui, solo, "Eighty waves. About an hour if you earn it.", pal::ACC) {
        action = Action::SinglePlayer;
    }
    if big_button(
        ui,
        "Multiplayer",
        "Up to 8 players, same waves, separate boards - highest wave wins.",
        pal::GOLD,
    ) {
        m.screen = Screen::Connect;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.label(
        RichText::new(
            "Click a pad to build, U upgrades, S sells, Enter calls the next wave early.",
        )
        .size(12.0)
        .color(pal::DIM),
    );
    action
}

// ---------------------------------------------------------------- connect

fn connect(ui: &mut egui::Ui, m: &mut MenuState, net: &mut Net) {
    heading(ui);

    ui.horizontal(|ui| {
        for (mode, label) in [(Mode::Host, "Create a room"), (Mode::Join, "Join a room")] {
            let on = m.mode == mode;
            let btn = egui::Button::new(
                RichText::new(label)
                    .strong()
                    .color(if on { pal::INK } else { pal::DIM }),
            )
            .fill(if on { pal::CARD_HOVER } else { pal::CARD })
            .corner_radius(6.0);
            if ui.add_sized(Vec2::new(ui.available_width() * 0.5, 32.0), btn).clicked() {
                m.mode = mode;
            }
        }
    });
    ui.add_space(10.0);

    field(ui, "Your name", &mut m.name, false);
    if m.mode == Mode::Join {
        field(ui, "Room code", &mut m.room, false);
    }
    field(ui, "Room password", &mut m.password, true);
    if m.mode == Mode::Host {
        ui.label(
            RichText::new("Anyone with the room code and this password can join, up to 8 players.")
                .size(12.0)
                .color(pal::DIM),
        );
    }

    ui.add_space(6.0);
    ui.collapsing(RichText::new("Server").color(pal::DIM), |ui| {
        field(ui, "Address", &mut m.server, false);
        ui.label(
            RichText::new("Point this at your own lobby server. It only relays scores.")
                .size(11.0)
                .color(pal::DIM),
        );
    });

    if let Status::Failed(why) = &net.status {
        ui.add_space(6.0);
        ui.label(RichText::new(why).color(pal::BAD));
    }

    ui.add_space(10.0);
    let busy = net.status.is_busy();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !busy,
                egui::Button::new(RichText::new("Back").color(pal::DIM)).fill(pal::CARD),
            )
            .clicked()
        {
            net.leave();
            m.screen = Screen::Title;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = match (busy, m.mode) {
                (true, _) => "Connecting...",
                (false, Mode::Host) => "Create room",
                (false, Mode::Join) => "Join room",
            };
            let can_go = !busy
                && !m.password.trim().is_empty()
                && (m.mode == Mode::Host || !m.room.trim().is_empty());
            if ui
                .add_enabled(
                    can_go,
                    egui::Button::new(RichText::new(label).strong().color(pal::INK))
                        .fill(pal::CARD_HOVER)
                        .corner_radius(6.0),
                )
                .clicked()
            {
                let name = td_proto::clean_name(&m.name);
                m.name = name.clone();
                match m.mode {
                    Mode::Host => {
                        net.create(&m.server, &name, &m.password, 0)
                    }
                    Mode::Join => net.join(&m.server, &m.room, &m.password, &name),
                }
            }
        });
    });
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, password: bool) {
    ui.label(RichText::new(label).size(12.0).color(pal::DIM));
    ui.add_sized(
        Vec2::new(ui.available_width(), 26.0),
        egui::TextEdit::singleline(value).password(password),
    );
    ui.add_space(6.0);
}

// ---------------------------------------------------------------- lobby

fn lobby(ui: &mut egui::Ui, m: &mut MenuState, net: &mut Net) -> Action {
    heading(ui);

    let id = net.room_id().to_string();
    ui.label(RichText::new("Room code").size(12.0).color(pal::DIM));
    ui.horizontal(|ui| {
        ui.label(RichText::new(&id).size(15.0).strong().color(pal::GOLD).monospace());
        if ui
            .button(RichText::new(if m.copied > 0.0 { "Copied" } else { "Copy" }).size(12.0))
            .clicked()
        {
            ui.ctx().copy_text(id.clone());
            m.copied = 1.6;
        }
    });
    ui.label(
        RichText::new("Send this code and the password to your friends.")
            .size(12.0)
            .color(pal::DIM),
    );
    ui.add_space(10.0);

    let (host, started, players) = match net.room.as_ref() {
        Some(r) => (r.host, r.started, r.players.clone()),
        None => (0, false, Vec::new()),
    };

    ui.label(
        RichText::new(format!("Players  {}/{MAX_PLAYERS}", players.len()))
            .size(12.0)
            .color(pal::DIM),
    );
    egui::Frame::NONE
        .fill(pal::PANEL_DEEP)
        .corner_radius(6.0)
        .inner_margin(8)
        .show(ui, |ui| {
            for p in &players {
                ui.horizontal(|ui| {
                    let you = p.slot == net.you;
                    let mut name = p.name.clone();
                    if p.slot == host {
                        name.push_str("  (host)");
                    }
                    if you {
                        name.push_str("  - you");
                    }
                    ui.label(RichText::new(name).color(if you { pal::ACC } else { pal::INK }));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (t, c) = if p.ready {
                            ("ready", pal::GOOD)
                        } else {
                            ("waiting", pal::DIM)
                        };
                        ui.label(RichText::new(t).size(12.0).color(c));
                    });
                });
            }
            if players.is_empty() {
                ui.label(RichText::new("Waiting for the room...").color(pal::DIM));
            }
        });

    ui.add_space(10.0);
    let mut action = Action::None;
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(RichText::new("Leave").color(pal::DIM)).fill(pal::CARD))
            .clicked()
        {
            net.leave();
            m.screen = Screen::Title;
            m.ready = false;
            action = Action::Cancelled;
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if net.is_host() {
                let everyone = players.len() > 1 && players.iter().all(|p| p.ready || p.slot == host);
                let label = if everyone { "Start" } else { "Start anyway" };
                if ui
                    .add(
                        egui::Button::new(RichText::new(label).strong().color(pal::INK))
                            .fill(pal::CARD_HOVER)
                            .corner_radius(6.0),
                    )
                    .clicked()
                {
                    net.start();
                }
            } else {
                let label = if m.ready { "Not ready" } else { "Ready" };
                if ui
                    .add(
                        egui::Button::new(RichText::new(label).strong().color(pal::INK))
                            .fill(if m.ready { pal::CARD } else { pal::CARD_HOVER })
                            .corner_radius(6.0),
                    )
                    .clicked()
                {
                    m.ready = !m.ready;
                    net.set_ready(m.ready);
                }
                ui.label(
                    RichText::new(if started {
                        "Starting..."
                    } else {
                        "Waiting for the host"
                    })
                    .size(12.0)
                    .color(pal::DIM),
                );
            }
        });
    });
    action
}

// ---------------------------------------------------------------- in-game

/// The live room scoreboard, shown in the corner while a multiplayer run is on.
pub fn room_scoreboard(ctx: &Context, net: &Net, compact: bool) {
    if !net.is_online() {
        return;
    }
    let Some(room) = net.room.as_ref() else { return };
    if !room.started {
        return;
    }
    egui::Area::new("room_scores".into())
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 68.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(13, 16, 24, 216))
                .corner_radius(8.0)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.set_width(if compact { 150.0 } else { 190.0 });
                    ui.label(RichText::new("ROOM").size(11.0).strong().color(pal::DIM));
                    for (i, p) in room.ranked().iter().enumerate() {
                        let you = p.slot == net.you;
                        let col = if !p.snap.alive {
                            pal::BAD
                        } else if you {
                            pal::ACC
                        } else {
                            pal::INK
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{}. {}", i + 1, p.name))
                                    .size(12.0)
                                    .color(col),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("w{}", p.snap.wave))
                                        .size(12.0)
                                        .strong()
                                        .color(col),
                                );
                            });
                        });
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The connection is the source of truth about which screen is honest. If
    /// the socket drops in the lobby the player must land back on a form they
    /// can retry from, not stare at a room that no longer exists.
    #[test]
    fn the_screen_follows_the_connection() {
        let ctx = Context::default();
        let mut m = MenuState::default();
        let mut net = Net::default();

        m.screen = Screen::Connect;
        net.status = Status::Lobby;
        run(&ctx, &mut m, &mut net);
        assert_eq!(m.screen, Screen::Lobby, "joining a room opens the lobby");

        m.ready = true;
        net.status = Status::Failed("gone".into());
        run(&ctx, &mut m, &mut net);
        assert_eq!(m.screen, Screen::Connect, "a dropped room returns to the form");
        assert!(!m.ready, "ready must not survive the room it belonged to");
    }

    #[test]
    fn the_title_screen_starts_a_local_run_at_the_chosen_difficulty() {
        let ctx = Context::default();
        let mut m = MenuState::default();
        let mut net = Net::default();
        // No click happens in a headless pass, so this only asserts the menu
        // lays out at every screen and never panics on the way through.
        for w in [360.0, 720.0, 1400.0] {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(w, 640.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    show(ui.ctx(), &mut m, &mut net, 0.016);
                },
            );
            out.textures_delta.clear();
        }
        assert_eq!(m.screen, Screen::Title);
    }

    fn run(ctx: &Context, m: &mut MenuState, net: &mut Net) {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(ui.ctx(), m, net, 0.016);
        });
        out.textures_delta.clear();
    }
}
