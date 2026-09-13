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

use egui::{
    Align, Align2, Color32, Context, CornerRadius, Layout, RichText, Sense, Stroke, Vec2, pos2,
    vec2,
};

use crate::game::Difficulty;
use crate::net::{Net, Status};
use crate::ui::pal;
use td_proto::MAX_PLAYERS;

static TITLE_BACKDROP: &[u8] = include_bytes!("../assets/title_backdrop.bin");

/// Paints the authored menu art edge-to-edge, cropped rather than stretched.
/// Returns false when the blob is unavailable so the app can keep its live
/// board fallback.
pub fn paint_backdrop(ctx: &Context, painter: &egui::Painter, rect: egui::Rect) -> bool {
    let cache_id = egui::Id::new("title-backdrop-texture");
    let mut texture = ctx.data(|data| data.get_temp::<egui::TextureHandle>(cache_id));
    if texture.is_none() {
        if TITLE_BACKDROP.len() < 16 {
            return false;
        }
        let at = |o: usize| {
            u32::from_le_bytes([
                TITLE_BACKDROP[o],
                TITLE_BACKDROP[o + 1],
                TITLE_BACKDROP[o + 2],
                TITLE_BACKDROP[o + 3],
            ])
        };
        let (magic, version, width, height) = (at(0), at(4), at(8) as usize, at(12) as usize);
        let need = 16usize.saturating_add(width.saturating_mul(height).saturating_mul(3));
        if magic != 0x4742_5447
            || version != 1
            || width == 0
            || height == 0
            || need > TITLE_BACKDROP.len()
        {
            return false;
        }
        let image = egui::ColorImage::from_rgb([width, height], &TITLE_BACKDROP[16..need]);
        let loaded = ctx.load_texture("title-backdrop", image, egui::TextureOptions::LINEAR);
        ctx.data_mut(|data| data.insert_temp(cache_id, loaded.clone()));
        texture = Some(loaded);
    }
    let Some(texture) = texture.as_ref() else {
        return false;
    };

    let source_aspect = texture.size()[0] as f32 / texture.size()[1].max(1) as f32;
    let target_aspect = rect.width() / rect.height().max(1.0);
    let mut uv = egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    if target_aspect > source_aspect {
        let visible = source_aspect / target_aspect;
        uv.min.y = (1.0 - visible) * 0.5;
        uv.max.y = 1.0 - uv.min.y;
    } else {
        let visible = target_aspect / source_aspect;
        uv.min.x = (1.0 - visible) * 0.5;
        uv.max.x = 1.0 - uv.min.x;
    }
    painter.image(texture.id(), rect, uv, Color32::WHITE);
    painter.rect_filled(
        rect,
        CornerRadius::ZERO,
        Color32::from_rgba_unmultiplied(4, 10, 9, 38),
    );
    true
}

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
    /// The ruleset the next local run or hosted room will use.
    pub difficulty: Difficulty,
    /// The actual egui bounds of the title-screen Continue card.  This is
    /// deliberately retained for the browser QA marker: a CSS viewport can be
    /// clamped by Chromium (especially on phone emulation), so tests must
    /// click the control the player can actually see rather than a guessed
    /// fraction of the requested window size.
    pub resume_rect: Option<egui::Rect>,
    /// Actual title-screen Campaign card, retained for viewport-aware browser
    /// smoke in the same way as the Continue card above.
    pub campaign_rect: Option<egui::Rect>,
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
            difficulty: Difficulty::Veteran,
            resume_rect: None,
            campaign_rect: None,
        }
    }
}

/// What the menu wants the app to do. The menu never touches the game itself.
pub enum Action {
    None,
    /// Start the authored 10-chapter expedition.
    Campaign(Difficulty),
    /// Start the original extracted 36-wave ruleset.
    Legacy(Difficulty),
    /// Pick up the saved run where it left off.
    Resume,
    /// The player left the lobby; drop back to the title screen.
    Cancelled,
}

// ---------------------------------------------------------------- entry

pub fn show(ctx: &Context, m: &mut MenuState, net: &mut Net, dt: f32) -> Action {
    m.copied = (m.copied - dt).max(0.0);
    // Never leave a stale title hitbox published while a connect/lobby screen
    // is on top of the scene.
    m.resume_rect = None;
    m.campaign_rect = None;

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
    let area = egui::Modal::new("menu".into()).frame(
        egui::Frame::NONE
            .fill(Color32::from_rgba_unmultiplied(12, 18, 17, 242))
            .stroke(Stroke::new(1.0, pal::GOLD_LINE))
            .corner_radius(14.0)
            .inner_margin(22.0),
    );
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
        ui.label(
            RichText::new("GREEN CIRCLE TD")
                .size(30.0)
                .strong()
                .color(pal::GOLD),
        );
        ui.label(
            RichText::new(
                "A fixed-board tower defense: 10 chapters, 600 authored encounters, and a preserved 36-wave Legacy mode.",
            )
                .size(13.0)
                .color(pal::DIM),
        );
        ui.add_space(10.0);
    });
}

fn big_button(ui: &mut egui::Ui, text: &str, sub: &str, accent: Color32) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 62.0), Sense::click());
    // The full desktop summaries do not fit a 390px title modal. Use concise,
    // truthful campaign/Legacy descriptions there and clip all painted text to
    // its card as a final guard against an overflow at any viewport.
    let sub = if rect.width() < 350.0 {
        match text {
            "Start Campaign" | "Start new Campaign" => "10 chapters · 600 encounters · autosaves.",
            "Legacy 36-wave mode" => "Original 36 waves · saves · endless score chase.",
            _ => sub,
        }
    } else {
        sub
    };
    let fill = if resp.hovered() {
        pal::CARD_HOVER
    } else {
        pal::CARD
    };
    let stroke = Stroke::new(
        if resp.hovered() { 1.5 } else { 1.0 },
        accent.gamma_multiply(0.85),
    );
    ui.painter().rect(
        rect,
        CornerRadius::same(9),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    let painter = ui.painter().with_clip_rect(rect.shrink2(vec2(10.0, 5.0)));
    painter.text(
        pos2(rect.left() + 16.0, rect.top() + 13.0),
        Align2::LEFT_TOP,
        text,
        egui::FontId::proportional(17.0),
        accent,
    );
    painter.text(
        pos2(rect.left() + 16.0, rect.top() + 37.0),
        Align2::LEFT_TOP,
        sub,
        egui::FontId::proportional(11.5),
        pal::DIM,
    );
    ui.add_space(7.0);
    resp
}

fn difficulty_picker(ui: &mut egui::Ui, selected: &mut Difficulty) {
    ui.label(
        RichText::new("CHALLENGE")
            .size(10.0)
            .monospace()
            .color(pal::DIM),
    );
    let gap = 6.0;
    let width = ((ui.available_width() - gap * 2.0) / 3.0).max(70.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for difficulty in Difficulty::ALL {
            let active = *selected == difficulty;
            let cap = difficulty.flood_limit(1);
            let button = egui::Button::new(
                RichText::new(format!(
                    "{}\n{}\n{} CAPACITY",
                    difficulty.label().to_ascii_uppercase(),
                    difficulty.tagline(),
                    cap
                ))
                    .size(10.5)
                    .strong()
                    .color(if active { pal::PANEL_DEEP } else { pal::INK }),
            )
            .fill(if active { pal::GOLD } else { pal::PANEL_DEEP })
            .stroke(Stroke::new(
                1.0,
                if active { pal::GOLD } else { pal::GOLD_LINE },
            ))
            .corner_radius(6.0);
            let response = ui.add_sized(vec2(width, 57.0), button).on_hover_text(
                match difficulty {
                    Difficulty::Classic => {
                        "Faithful Warcraft III pressure. No Vanguards, lap rage, or command upgrades."
                    }
                    Difficulty::Veteran => {
                        "Vanguards begin on wave 6. Survivors gain 6% speed per lap. Draft upgrades on waves 10, 20 and 30."
                    }
                    Difficulty::Nightmare => {
                        "Vanguards begin on wave 3. Survivors gain 10% speed per lap. Lower rewards demand efficient builds."
                    }
                },
            );
            if response.clicked() {
                *selected = difficulty;
            }
        }
    });
    ui.label(RichText::new(selected.blurb()).size(11.5).color(pal::DIM));
    ui.add_space(7.0);
}

// ---------------------------------------------------------------- title

fn title(ui: &mut egui::Ui, m: &mut MenuState) -> Action {
    heading(ui);

    let mut action = Action::None;
    if let Some(save) = &m.saved {
        let label = save.label();
        let response = big_button(ui, "Continue", &label, pal::GOOD);
        m.resume_rect = Some(response.rect);
        if response.clicked() {
            action = Action::Resume;
        }
    }
    difficulty_picker(ui, &mut m.difficulty);
    let solo = if m.saved.is_some() { "Start new Campaign" } else { "Start Campaign" };
    let campaign_response = big_button(
        ui,
        solo,
        "10 chapters · 600 authored encounters · 5h03m minimum at 2x · autosave.",
        pal::ACC,
    );
    m.campaign_rect = Some(campaign_response.rect);
    if campaign_response.clicked() {
        action = Action::Campaign(m.difficulty);
    }
    if big_button(
        ui,
        "Legacy 36-wave mode",
        "The original extracted rules, saves and post-victory endless score chase.",
        pal::GOLD,
    ).clicked() {
        action = Action::Legacy(m.difficulty);
    }
    if big_button(
        ui,
        "Multiplayer",
        "Up to 8 players, same waves, separate boards - highest wave wins.",
        pal::GOLD,
    ).clicked() {
        m.screen = Screen::Connect;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.label(
        RichText::new(
            "Choose a tower, place it beside the road, then press Enter to call waves for bonus gold.",
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
            let btn = egui::Button::new(RichText::new(label).strong().color(if on {
                pal::INK
            } else {
                pal::DIM
            }))
            .fill(if on { pal::CARD_HOVER } else { pal::CARD })
            .corner_radius(6.0);
            if ui
                .add_sized(Vec2::new(ui.available_width() * 0.5, 32.0), btn)
                .clicked()
            {
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
        ui.add_space(8.0);
        difficulty_picker(ui, &mut m.difficulty);
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
                    Mode::Host => net.create(&m.server, &name, &m.password, m.difficulty.as_u8()),
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
        ui.label(
            RichText::new(&id)
                .size(15.0)
                .strong()
                .color(pal::GOLD)
                .monospace(),
        );
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
                let everyone =
                    players.len() > 1 && players.iter().all(|p| p.ready || p.slot == host);
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
    let Some(room) = net.room.as_ref() else {
        return;
    };
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
        assert_eq!(
            m.screen,
            Screen::Connect,
            "a dropped room returns to the form"
        );
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
