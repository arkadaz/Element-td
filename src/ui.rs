//! All egui chrome: the resource strip, the scoreboard, the minimap, the
//! command card and the build palette.
//!
//! The game state is the single source of truth - this module only reads it and
//! calls the same public `Game` methods a keyboard shortcut would.

use egui::{
    Align2, Color32, Context, CornerRadius, FontId, Id, Pos2, Rect, Response, RichText, Sense,
    Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::game::board::{BH, BW};
use crate::game::defs::*;
use crate::game::{BUILD_TIME_FIRST, Game, Phase};
use crate::math::{Camera, v3};

pub const TOP_H: f32 = 58.0;
pub const COMMAND_H: f32 = 178.0;

/// Below this width the HUD switches to a compact layout: shorter bars, smaller
/// cards, no minimap. Phones in landscape are typically 650-900 points wide.
pub const COMPACT_WIDTH: f32 = 1000.0;
/// Narrower still and the selection panel goes too, leaving build + board.
pub const TINY_WIDTH: f32 = 720.0;

pub fn compact_for(width: f32) -> bool {
    width < COMPACT_WIDTH
}
pub fn top_h(compact: bool) -> f32 {
    if compact { 46.0 } else { TOP_H }
}
pub fn command_h(compact: bool) -> f32 {
    if compact { 128.0 } else { COMMAND_H }
}
fn bar_h(compact: bool) -> f32 {
    if compact { 108.0 } else { BAR_H }
}
fn card_h(compact: bool) -> f32 {
    if compact { 82.0 } else { CARD_H }
}
fn card_w(compact: bool) -> f32 {
    if compact { 70.0 } else { CARD_W }
}
/// Height every section of the command bar is laid out to. Keeping one number
/// here is what stops the palette from overflowing its panel.
pub const BAR_H: f32 = 156.0;
pub const CARD_W: f32 = 86.0;
pub const CARD_H: f32 = 100.0;

// ---------------------------------------------------------------- palette

pub mod pal {
    use egui::Color32;
    pub const PANEL: Color32 = Color32::from_rgb(19, 22, 32);
    pub const PANEL_DEEP: Color32 = Color32::from_rgb(13, 16, 24);
    pub const CARD: Color32 = Color32::from_rgb(27, 32, 46);
    pub const CARD_HOVER: Color32 = Color32::from_rgb(38, 46, 66);
    pub const LINE: Color32 = Color32::from_rgb(46, 54, 76);
    pub const GOLD_LINE: Color32 = Color32::from_rgb(92, 78, 44);
    pub const INK: Color32 = Color32::from_rgb(232, 237, 248);
    pub const DIM: Color32 = Color32::from_rgb(140, 152, 178);
    pub const ACC: Color32 = Color32::from_rgb(90, 209, 255);
    pub const GOLD: Color32 = Color32::from_rgb(255, 206, 92);
    pub const BAD: Color32 = Color32::from_rgb(255, 92, 114);
    pub const GOOD: Color32 = Color32::from_rgb(110, 231, 135);
}

/// The colour air belongs to, used on the wave badge and on tower cards, so the
/// two always agree about what "flies" looks like.
pub const AIR_TINT: [f32; 3] = [0.56, 0.82, 1.00];

pub fn c32(c: [f32; 3], a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
        (a.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// Exact gold with thousands separators. Money must never be abbreviated - you
/// cannot decide whether to buy a 2,240g tower when the HUD says "2.2k".
pub fn gold_str(v: i64) -> String {
    let neg = v < 0;
    let digits = v.abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if neg {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn short(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1e6)
    } else if v >= 10_000.0 {
        format!("{:.0}k", v / 1e3)
    } else if v >= 1_000.0 {
        format!("{:.1}k", v / 1e3)
    } else {
        format!("{:.0}", v)
    }
}

// ---------------------------------------------------------------- state

pub struct UiState {
    pub show_help: bool,
    pub build_tier: u32,
    pub hotkeys: Vec<usize>,
    /// Where the build cards were drawn last frame, and the panel they must fit
    /// inside. A layout test asserts containment - nested egui layouts had been
    /// quietly adding space and pushing the cards out of their panel.
    pub card_rects: Vec<Rect>,
    pub palette_rect: Rect,
    /// Set from the viewport each frame; drives the compact HUD.
    pub compact: bool,
    pub quality: crate::gfx::Quality,
    pub quality_dirty: bool,
    /// Frames spent below the target rate, used to step quality down on its own.
    /// Which page of the build palette is showing, when they do not all fit.
    pub palette_page: usize,
    pub slow_frames: u32,
    /// Frames in a row that came in comfortably under budget.
    pub fast_frames: u32,
    pub auto_quality: bool,
    /// The highest preset auto-tuning may climb back to. It starts at the top
    /// and drops permanently the first time a preset proves too slow, so the
    /// tuner can recover from a bad first few seconds without ever oscillating
    /// between two presets for the rest of the run.
    pub quality_ceiling: crate::gfx::Quality,
    /// Set from the connection each frame. Online runs share a seed, so the
    /// controls that would silently restart the game are taken away.
    pub online: bool,
    /// Raised by the Menu button; the app acts on it and clears it.
    pub want_menu: bool,
    /// The perf readout, rebuilt a few times a second rather than every frame.
    pub perf: String,
    pub perf_ticks: u32,
    /// Frames per second to aim for. Drawing faster than the display refreshes
    /// is pure waste - heat, fan noise and battery for pixels nobody sees.
    pub fps_cap: u32,
    /// The strip the top bar drew into last frame, for the layout test.
    pub top_content: Rect,
    /// Where the wave / lives / gold readouts landed, and where the leftmost
    /// control button landed. A layout test asserts they never collide - the
    /// stats being silently pushed off the edge by the controls is a bug that
    /// has shipped here once already.
    pub stat_rects: Vec<Rect>,
    pub controls_left: f32,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_help: false,
            build_tier: 1,
            hotkeys: Vec::new(),
            card_rects: Vec::new(),
            palette_rect: Rect::NOTHING,
            compact: false,
            // A browser is the constrained target by definition, so it opens
            // on the two-pass preset and climbs only if the frames say it can.
            // Starting high and falling meant several seconds of bad frames on
            // every phone before the tuner noticed.
            quality: if cfg!(target_arch = "wasm32") {
                crate::gfx::Quality::Performance
            } else {
                crate::gfx::Quality::Balanced
            },
            quality_dirty: false,
            palette_page: 0,
            slow_frames: 0,
            fast_frames: 0,
            auto_quality: true,
            quality_ceiling: crate::gfx::Quality::Ultra,
            online: false,
            want_menu: false,
            perf: String::new(),
            perf_ticks: 0,
            fps_cap: 60,
            top_content: Rect::NOTHING,
            stat_rects: Vec::new(),
            controls_left: f32::MAX,
        }
    }
}

pub fn install_style(ctx: &Context) {
    ctx.all_styles_mut(|style| {
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = pal::PANEL;
        style.visuals.window_fill = pal::PANEL;
        style.visuals.window_stroke = Stroke::new(1.0, pal::LINE);
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal::LINE);
        style.visuals.widgets.inactive.weak_bg_fill = pal::CARD;
        style.visuals.widgets.hovered.weak_bg_fill = pal::CARD_HOVER;
        style.visuals.override_text_color = Some(pal::INK);
        style.visuals.window_corner_radius = CornerRadius::same(10);
        style.spacing.item_spacing = vec2(7.0, 5.0);
        style.spacing.button_padding = vec2(9.0, 4.0);
    });
}

// ---------------------------------------------------------------- top strip

/// The resource strip.
///
/// Laid out by explicit budget rather than by nested egui layouts. The previous
/// version put the controls in a right-to-left sub-layout, which claims all the
/// remaining width - so on a narrow window the wave, lives and gold were pushed
/// clean off the left edge and the *performance readout* was the only thing you
/// could still see. Money and lives are the game; they are the last things to
/// go, not the first. Here the controls are measured first, whatever is left
/// belongs to the stats, and optional items drop in a fixed priority order.
pub fn top_bar(g: &mut Game, ui: &mut Ui, ust: &mut UiState, perf: &str) {
    let compact = ust.compact;
    let full = ui.available_rect_before_wrap();
    let h = full.height();
    let pad = 6.0;

    // ---- right-hand controls.
    #[derive(Clone, Copy, PartialEq)]
    enum Cmd {
        Send,
        Pause,
        Speed,
        Quality,
        Menu,
        Help,
    }
    let send_bonus = (g.build_timer * EARLY_BONUS_PER_SEC) as i32;
    let send_label = if compact {
        format!("Send +{send_bonus}")
    } else {
        format!("Send  +{send_bonus}g")
    };
    let speed_label = format!("{:.0}x", g.speed);
    let quality_label = if compact {
        ust.quality.short()
    } else {
        ust.quality.label()
    };
    // The pause icon is painted, not typed. egui bundles a Latin font and an
    // emoji font; U+25B6 and U+2759 are in neither, so this button rendered as
    // two empty tofu boxes. Two spaces reserve the width and the glyph is drawn
    // below - which also means it always matches the button's ink colour.
    let pause_label = "  ";

    let mut cmds: Vec<(Cmd, &str)> = Vec::with_capacity(6);
    if g.phase == Phase::Build {
        cmds.push((Cmd::Send, send_label.as_str()));
    }
    cmds.push((Cmd::Pause, pause_label));
    cmds.push((Cmd::Speed, speed_label.as_str()));
    cmds.push((Cmd::Quality, quality_label));
    cmds.push((Cmd::Menu, "Menu"));
    cmds.push((Cmd::Help, "?"));

    let text_w = |ui: &Ui, text: &str, font: FontId| -> f32 {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font, pal::INK)
            .rect
            .width()
    };
    let mut widths: Vec<f32> = cmds
        .iter()
        .map(|(_, t)| (text_w(ui, t, FontId::proportional(13.0)) + 20.0).max(30.0))
        .collect();

    // ---- stats always come first, and always fit.
    let gold = gold_str(g.gold);
    let wave = if g.endless {
        format!("{}", g.wave)
    } else {
        format!("{}/{}", g.wave, N_WAVES)
    };
    let lives = g.lives.to_string();
    let value_size = if compact { 15.0 } else { 18.0 };
    let chips: [(&str, &str, Color32); 3] = [
        (
            "WAVE",
            wave.as_str(),
            if g.endless { pal::ACC } else { pal::INK },
        ),
        ("LIVES", lives.as_str(), pal::BAD),
        ("GOLD", gold.as_str(), pal::GOLD),
    ];
    let chip_ws: Vec<f32> = chips
        .iter()
        .map(|(l, v, _)| {
            let a = text_w(ui, l, FontId::monospace(9.0));
            let b = text_w(ui, v, FontId::monospace(value_size));
            a.max(b) + 18.0
        })
        .collect();
    let stats_w: f32 = chip_ws.iter().sum::<f32>() + pad * 2.0;

    // On a phone there is not room for everything. Drop controls, least useful
    // first, until they fit beside the stats - the stats themselves are never
    // dropped, because losing sight of your gold mid-wave is worse than losing
    // any button here.
    let rank = |c: Cmd| match c {
        Cmd::Quality => 0,
        Cmd::Help => 1,
        Cmd::Speed => 2,
        Cmd::Pause => 3,
        Cmd::Menu => 4,
        Cmd::Send => 5,
    };
    let width_of = |ws: &[f32]| ws.iter().sum::<f32>() + pad * (ws.len() as f32 - 1.0).max(0.0);
    while cmds.len() > 1 && stats_w + width_of(&widths) + pad * 2.0 > full.width() {
        let (worst, _) = cmds
            .iter()
            .enumerate()
            .min_by_key(|(_, (c, _))| rank(*c))
            .expect("cmds is non-empty");
        cmds.remove(worst);
        widths.remove(worst);
    }
    let controls_w: f32 = width_of(&widths);

    // ---- fit the optional extras into whatever is left over.
    let mut spare = full.width() - stats_w - controls_w - pad * 2.0;
    let preview_w = 268.0;
    let show_preview = !compact && spare > preview_w + 8.0;
    if show_preview {
        spare -= preview_w + pad;
    }
    let perf_w = text_w(ui, perf, FontId::monospace(10.0));
    let show_perf = !compact && spare > perf_w + 12.0;

    // ---- paint. Nothing here can overflow: every rect came out of the budget.
    ust.stat_rects.clear();
    let mut x = full.left();
    for (i, (label, value, col)) in chips.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, full.top()), vec2(chip_ws[i], h));
        stat_chip(ui, r, label, value, *col, value_size);
        ust.stat_rects.push(r);
        x += chip_ws[i] + pad;
    }
    if show_preview {
        let r = Rect::from_min_size(
            pos2(x, full.top() + 2.0),
            vec2(preview_w, (h - 4.0).min(40.0)),
        );
        wave_preview(g, ui, r);
        x += preview_w + pad;
    }
    if show_perf {
        let r = Rect::from_min_size(pos2(x, full.top()), vec2(perf_w + 8.0, h));
        ui.put(
            r,
            egui::Label::new(RichText::new(perf).monospace().size(10.0).color(pal::DIM)),
        );
    }

    let mut cx = full.right();
    for (i, (cmd, text)) in cmds.iter().enumerate().rev() {
        cx -= widths[i];
        let r = Rect::from_min_size(
            pos2(cx, full.top() + 6.0),
            vec2(widths[i], (h - 12.0).max(20.0)),
        );
        let fill = match cmd {
            Cmd::Send => Color32::from_rgb(38, 106, 68),
            Cmd::Menu => pal::CARD_HOVER,
            _ => pal::CARD,
        };
        let mut label = RichText::new(*text).size(13.0).color(pal::INK);
        if matches!(cmd, Cmd::Send | Cmd::Menu) {
            label = label.strong();
        }
        let resp = ui.put(r, egui::Button::new(label).fill(fill).corner_radius(6.0));
        if *cmd == Cmd::Pause {
            paint_transport(ui, r, g.paused);
        }
        let resp = match cmd {
            Cmd::Send => resp
                .on_hover_text("Call the wave early (Enter). The bonus is the time you give up."),
            Cmd::Pause => resp.on_hover_text("Pause (Space)"),
            Cmd::Speed => resp.on_hover_text("Game speed (F)"),
            Cmd::Quality => {
                resp.on_hover_text("Graphics quality (B). Lower it if the frame rate drags.")
            }
            Cmd::Menu => resp.on_hover_text("Back to the menu. This ends the run."),
            Cmd::Help => resp.on_hover_text("How to play (H)"),
        };
        if resp.clicked() {
            match cmd {
                Cmd::Send => g.send_wave(),
                Cmd::Pause => g.paused = !g.paused,
                Cmd::Speed => {
                    g.speed = match g.speed as i32 {
                        1 => 2.0,
                        2 => 3.0,
                        _ => 1.0,
                    }
                }
                Cmd::Quality => {
                    ust.quality = ust.quality.lower().unwrap_or(crate::gfx::Quality::Ultra);
                    ust.quality_dirty = true;
                    ust.auto_quality = false;
                }
                Cmd::Menu => ust.want_menu = true,
                Cmd::Help => ust.show_help = true,
            }
        }
        cx -= pad;
    }
    ust.controls_left = cx + pad;
    ust.top_content = full;
}

/// Play and pause, drawn rather than typed. See the note in [`top_bar`].
fn paint_transport(ui: &Ui, r: Rect, paused: bool) {
    let p = ui.painter();
    let c = r.center();
    let h = (r.height() * 0.40).min(8.0);
    if paused {
        // Play: a right-pointing triangle.
        p.add(egui::Shape::convex_polygon(
            vec![
                pos2(c.x - h * 0.55, c.y - h),
                pos2(c.x - h * 0.55, c.y + h),
                pos2(c.x + h * 0.85, c.y),
            ],
            pal::INK,
            Stroke::NONE,
        ));
    } else {
        // Pause: two bars.
        let w = h * 0.44;
        for s in [-1.0f32, 1.0] {
            p.rect_filled(
                Rect::from_center_size(pos2(c.x + s * w * 1.6, c.y), vec2(w, h * 2.0)),
                CornerRadius::same(1),
                pal::INK,
            );
        }
    }
}

/// One resource readout: a small caps label over a large monospace number, on a
/// card. Monospace so a gold figure does not jitter sideways as it ticks.
fn stat_chip(ui: &mut Ui, r: Rect, label: &str, value: &str, color: Color32, value_size: f32) {
    let p = ui.painter();
    p.rect_filled(
        r.shrink2(vec2(0.0, 3.0)),
        CornerRadius::same(7),
        pal::PANEL_DEEP,
    );
    p.text(
        pos2(r.center().x, r.top() + 12.0),
        Align2::CENTER_CENTER,
        label,
        FontId::monospace(9.0),
        pal::DIM,
    );
    p.text(
        pos2(r.center().x, r.bottom() - value_size * 0.85),
        Align2::CENTER_CENTER,
        value,
        FontId::monospace(value_size),
        color,
    );
}

/// What is coming next, and what it punishes.
fn wave_preview(g: &mut Game, ui: &mut Ui, rect: Rect) {
    let w = g.next_wave_def();
    let upcoming = (g.wave + 1).min(N_WAVES);
    let resp = ui.interact(rect, Id::new("wave_preview"), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(7), pal::CARD);
    let air = w.kind.flying();
    p.rect_stroke(
        rect,
        CornerRadius::same(7),
        Stroke::new(
            if air { 2.0 } else { 1.0 },
            c32(w.armor().color(), if air { 0.95 } else { 0.5 }),
        ),
        StrokeKind::Inside,
    );

    let title = match g.phase {
        Phase::Combat if g.endless => format!("ENDLESS - WAVE {}", g.wave),
        Phase::Combat => format!("WAVE {} INCOMING", g.wave),
        Phase::Victory => "ALL WAVES CLEARED".to_string(),
        Phase::Defeat => "OVERRUN".to_string(),
        _ => format!("NEXT: WAVE {upcoming}"),
    };
    p.text(
        rect.left_top() + vec2(8.0, 5.0),
        Align2::LEFT_TOP,
        title,
        FontId::monospace(9.0),
        pal::DIM,
    );

    let mods = w.modifiers();
    let line = if mods.is_empty() {
        format!("{} x{}  ·  {}", w.kind.name(), w.count, w.armor().name())
    } else {
        format!("{} x{}  ·  {}", w.kind.name(), w.count, mods.join(", "))
    };
    let mut tx = rect.left() + 8.0;
    if air {
        // A filled badge, not a word in a sentence. Missing this costs lives.
        let badge = Rect::from_min_size(pos2(tx, rect.top() + 17.0), vec2(34.0, 14.0));
        p.rect_filled(badge, CornerRadius::same(4), c32(AIR_TINT, 1.0));
        p.text(
            badge.center(),
            Align2::CENTER_CENTER,
            "AIR",
            FontId::monospace(9.0),
            Color32::from_rgb(10, 14, 24),
        );
        tx += 40.0;
    }
    p.text(
        pos2(tx, rect.top() + 17.0),
        Align2::LEFT_TOP,
        line,
        FontId::proportional(12.0),
        c32(w.armor().color(), 1.0),
    );

    if g.phase == Phase::Build {
        let frac = (g.build_timer / BUILD_TIME_FIRST.max(1.0)).clamp(0.0, 1.0);
        let bar = Rect::from_min_size(rect.left_bottom() + vec2(8.0, -6.0), vec2(252.0, 3.0));
        p.rect_filled(bar, CornerRadius::same(2), pal::LINE);
        p.rect_filled(
            Rect::from_min_size(bar.min, vec2(bar.width() * frac, 3.0)),
            CornerRadius::same(2),
            pal::ACC,
        );
        p.text(
            rect.right_top() + vec2(-8.0, 5.0),
            Align2::RIGHT_TOP,
            format!("{:.0}s", g.build_timer.max(0.0)),
            FontId::monospace(10.0),
            pal::ACC,
        );
    } else if g.phase == Phase::Combat {
        p.text(
            rect.right_top() + vec2(-8.0, 5.0),
            Align2::RIGHT_TOP,
            format!("{} left", g.creeps.len() as u32 + g.spawn_left),
            FontId::monospace(10.0),
            pal::INK,
        );
    }

    resp.on_hover_ui(|ui| {
        ui.label(
            RichText::new(w.kind.name())
                .strong()
                .color(c32(w.armor().color(), 1.0)),
        );
        ui.label(
            RichText::new(format!("{} armour", w.armor().name()))
                .size(11.0)
                .color(pal::DIM),
        );
        let counter = match w.armor() {
            Armor::Plated => "Magic +25%, Fire -15%, Physical only 55%",
            Armor::Warded => "Physical +25%, Magic only 55%",
            Armor::Ethereal => "Magic +30%. Physical 70%, Fire only 60%",
            Armor::Boss => "Everything but Toxic is taxed 15%",
            Armor::Unarmoured => "Fire +15%, nothing else resisted",
        };
        ui.label(RichText::new(counter).size(11.0).color(pal::GOOD));
        if !w.kind.tell().is_empty() {
            ui.label(RichText::new(w.kind.tell()).size(11.0).color(pal::BAD));
        }
    });
}

// ---------------------------------------------------------------- scoreboard

/// The Element-TD style panel: level, interest, income, net worth.
pub fn scoreboard(g: &Game, ctx: &Context) {
    egui::Window::new("scoreboard")
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .anchor(Align2::RIGHT_TOP, vec2(-10.0, TOP_H + 10.0))
        .frame(
            egui::Frame::NONE
                .fill(pal::PANEL_DEEP)
                .stroke(Stroke::new(1.0, pal::GOLD_LINE))
                .corner_radius(CornerRadius::same(9))
                .inner_margin(10.0),
        )
        .show(ctx, |ui| {
            // Rows are laid out against a shared column width so a long number
            // widens the whole panel instead of running underneath its label.
            // The panel used to be a fixed 196 points wide, which meant a large
            // interest figure printed straight through the word "Interest".
            let mut rows: Vec<(&str, String, Color32)> = Vec::with_capacity(9);
            rows.push(("Current wave", g.wave.to_string(), pal::INK));
            if !g.endless {
                rows.push(("Next wave", (g.wave + 1).min(N_WAVES).to_string(), pal::DIM));
            }
            rows.push((
                "Interest",
                format!(
                    "+{} ({:.0}%)",
                    gold_str(g.projected_interest()),
                    g.interest_rate() * 100.0
                ),
                pal::GOLD,
            ));
            let income = g.projected_income();
            if income > 0 {
                rows.push((
                    "Tombstone income",
                    format!("+{}", gold_str(income)),
                    pal::GOLD,
                ));
            }
            rows.push(("", String::new(), pal::DIM));
            rows.push(("Lives", g.lives.to_string(), pal::BAD));
            rows.push(("Gold", gold_str(g.gold), pal::GOLD));
            rows.push(("Net worth", gold_str(g.net_worth()), pal::GOOD));
            rows.push(("Towers", g.towers.len().to_string(), pal::DIM));
            rows.push(("Kills", short(g.stats.kills as f64), pal::DIM));

            let label_font = FontId::proportional(11.0);
            let value_font = FontId::monospace(11.5);
            let measure = |ui: &Ui, t: &str, f: FontId| {
                ui.painter()
                    .layout_no_wrap(t.to_owned(), f, pal::INK)
                    .rect
                    .width()
            };
            let label_w = rows
                .iter()
                .map(|(k, _, _)| measure(ui, k, label_font.clone()))
                .fold(0.0f32, f32::max);
            let value_w = rows
                .iter()
                .map(|(_, v, _)| measure(ui, v, value_font.clone()))
                .fold(0.0f32, f32::max);
            let width = (label_w + value_w + 14.0).max(176.0);
            ui.set_width(width);

            ui.label(
                RichText::new("STATUS")
                    .size(9.0)
                    .monospace()
                    .color(pal::DIM),
            );
            ui.add_space(3.0);
            for (k, v, col) in &rows {
                if k.is_empty() {
                    ui.separator();
                    continue;
                }
                let (r, _) = ui.allocate_exact_size(vec2(width, 15.0), Sense::hover());
                let p = ui.painter();
                p.text(
                    pos2(r.left(), r.center().y),
                    Align2::LEFT_CENTER,
                    k,
                    label_font.clone(),
                    pal::DIM,
                );
                p.text(
                    pos2(r.right(), r.center().y),
                    Align2::RIGHT_CENTER,
                    v,
                    value_font.clone(),
                    *col,
                );
            }
        });
}

// ---------------------------------------------------------------- command bar

pub fn command_bar(g: &mut Game, ui: &mut Ui, ust: &mut UiState) {
    let width = ui.available_width();
    let compact = ust.compact;
    ui.horizontal(|ui| {
        // On a phone the board is the scarce resource: drop the minimap first,
        // then the selection panel, before ever shrinking the build palette.
        if !compact {
            minimap(g, ui, bar_h(compact));
            ui.add_space(6.0);
        }
        // The essence counters are the shape of the whole run, so they sit
        // beside the board rather than in a menu. They are the first thing to
        // go on a phone, though - the build cards below already carry the
        // element pips that matter in the moment.
        if !compact && width > TINY_WIDTH {
            essence_strip(g, ui, bar_h(compact));
            ui.add_space(6.0);
        }
        if width > TINY_WIDTH {
            selection_panel(g, ui, compact);
            ui.add_space(6.0);
        }
        build_palette(g, ui, ust);
    });
}

fn minimap(g: &Game, ui: &mut Ui, h: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(h, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), pal::PANEL_DEEP);
    p.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, pal::LINE),
        StrokeKind::Inside,
    );

    let inner = rect.shrink(8.0);
    let sx = inner.width() / BW;
    let sy = inner.height() / BH;
    let s = sx.min(sy);
    let ox = inner.center().x - BW * s * 0.5;
    let oy = inner.center().y - BH * s * 0.5;
    let map = |w: [f32; 2]| pos2(ox + w[0] * s, oy + w[1] * s);

    // Ground plate.
    p.rect_filled(
        Rect::from_min_size(pos2(ox, oy), vec2(BW * s, BH * s)),
        CornerRadius::same(3),
        Color32::from_rgb(24, 34, 30),
    );
    // Road.
    for w in g.board.path.windows(2) {
        p.line_segment(
            [map(w[0]), map(w[1])],
            Stroke::new(2.5, Color32::from_rgb(86, 72, 56)),
        );
    }
    // Free pads.
    for slot in &g.board.slots {
        if slot.tower.is_none() {
            p.rect_filled(
                Rect::from_center_size(map(slot.pos), vec2(2.5, 2.5)),
                CornerRadius::ZERO,
                Color32::from_rgb(58, 66, 88),
            );
        }
    }
    // Towers.
    for t in &g.towers {
        p.rect_filled(
            Rect::from_center_size(map(t.pos), vec2(4.5, 4.5)),
            CornerRadius::same(1),
            c32(tower_color(t.def()), 1.0),
        );
    }
    // Monsters.
    for c in &g.creeps {
        let r = if c.kind == Kind::Boss { 3.5 } else { 2.0 };
        p.circle_filled(map(c.pos), r, c32(c.armor.color(), 1.0));
    }
    // Gates.
    p.circle_filled(map(g.board.sample(0.9)), 3.0, pal::BAD);
    p.circle_filled(map(g.board.sample(g.board.total - 0.9)), 3.0, pal::GOOD);
}

fn selection_panel(g: &mut Game, ui: &mut Ui, compact: bool) {
    let w = if compact { 268.0 } else { 340.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(w, bar_h(compact)), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), pal::PANEL_DEEP);
    p.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, pal::LINE),
        StrokeKind::Inside,
    );

    let Some(ti) = g.selected.filter(|&i| i < g.towers.len()) else {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Select a tower, or pick one to build",
            FontId::proportional(12.5),
            pal::DIM,
        );
        return;
    };

    let tw = g.towers[ti].clone();
    let def = tw.def();
    let col = tower_color(def);
    // The ceiling is the essences held, not MAX_TIER, and the difference is the
    // most important number on this panel - it is the whole reason the player is
    // about to spend an essence one way rather than another.
    let cap = g.tier_cap_of(tw.def);

    // Portrait frame.
    let port = Rect::from_min_size(rect.left_top() + vec2(10.0, 10.0), vec2(64.0, 64.0));
    p.rect_filled(port, CornerRadius::same(6), pal::CARD);
    p.rect_stroke(
        port,
        CornerRadius::same(6),
        Stroke::new(1.0, c32(col, 0.8)),
        StrokeKind::Inside,
    );
    p.rect_filled(
        Rect::from_center_size(port.center(), vec2(26.0, 26.0)),
        CornerRadius::same(4),
        c32(col, 1.0),
    );
    // Level as a number - six pips would not fit and would not read anyway.
    p.text(
        port.center_bottom() + vec2(0.0, -11.0),
        Align2::CENTER_CENTER,
        format!("Lv {}/{}", tw.tier, cap.max(tw.tier)),
        FontId::monospace(10.5),
        pal::GOLD,
    );

    let tx = port.right() + 10.0;
    p.text(
        pos2(tx, port.top() + 1.0),
        Align2::LEFT_TOP,
        tw.full_name(),
        FontId::proportional(15.0),
        c32(col, 1.0),
    );
    p.text(
        pos2(tx, port.top() + 20.0),
        Align2::LEFT_TOP,
        match tw.rank() {
            Some(r) => format!("{} · {} · {r}", def.element_label(), def.role),
            None => format!(
                "{} · {} · {}",
                def.element_label(),
                def.role,
                def.dtype.name()
            ),
        },
        FontId::proportional(10.5),
        if tw.rank().is_some() {
            c32(col, 0.9)
        } else {
            pal::DIM
        },
    );

    let dps = tw.dmg() * tw.rate();
    let stats_line = if tw.is_support() {
        format!("aura radius {:.1}", tw.range())
    } else {
        format!("{} dps   ·   range {:.1}", short(dps as f64), tw.range())
    };
    p.text(
        pos2(tx, port.top() + 36.0),
        Align2::LEFT_TOP,
        stats_line,
        FontId::monospace(11.5),
        pal::INK,
    );

    // A Grove or a Tombstone has no damage number, so show what it HAS done -
    // otherwise it reads as a wasted pad.
    let contribution = if tw.is_support() {
        let n = buffed_count(g, ti);
        format!("buffing {n} tower{}", if n == 1 { "" } else { "s" })
    } else if tw.gold_earned > 0 {
        format!(
            "{} kills   ·   {} gold earned",
            tw.kills,
            gold_str(tw.gold_earned as i64)
        )
    } else {
        format!("{} kills   ·   {} dealt", tw.kills, short(tw.damage))
    };
    p.text(
        pos2(tx, port.top() + 52.0),
        Align2::LEFT_TOP,
        contribution,
        FontId::monospace(10.5),
        if tw.is_support() || tw.gold_earned > 0 {
            pal::GOLD
        } else {
            pal::DIM
        },
    );
    if tw.buff_dmg > 0.0 {
        p.text(
            pos2(tx, port.top() + 68.0),
            Align2::LEFT_TOP,
            format!(
                "Grove: +{:.0}% dmg, +{:.0}% rate",
                tw.buff_dmg * 100.0,
                tw.buff_rate * 100.0
            ),
            FontId::proportional(10.5),
            pal::GOOD,
        );
    }

    // Command buttons along the bottom of the panel.
    let mut action: Option<Action> = None;
    let bar = Rect::from_min_size(rect.left_bottom() + vec2(10.0, -58.0), vec2(320.0, 48.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar));
    child.horizontal(|ui| {
        if let Some(cost) = tw.upgrade_cost_capped(cap) {
            let can = g.can_afford(cost);
            let next = tw.tier + 1;
            let milestone = match next {
                ASCEND_TIER => "  * Ascendant",
                ATTUNE_TIER => "  + Attuned",
                _ => "",
            };
            let label = format!("Level {next}{milestone}\n{cost}g");
            let b = egui::Button::new(RichText::new(label).size(12.0).strong())
                .fill(if can {
                    Color32::from_rgb(43, 110, 190)
                } else {
                    pal::CARD
                })
                .min_size(vec2(112.0, 44.0));
            if ui.add_enabled(can, b).clicked() {
                action = Some(Action::Upgrade);
            }
        } else if cap < MAX_TIER {
            // Not the end of the ladder - the end of what these essences buy.
            // Saying so is the difference between the player understanding the
            // draft and thinking the tower is simply finished.
            let b = egui::Button::new(
                RichText::new(format!("Capped at {cap}\nneeds {}", def.element_label())).size(10.5),
            )
            .fill(pal::CARD)
            .min_size(vec2(112.0, 44.0));
            ui.add_enabled(false, b).on_disabled_hover_text(format!(
                "Every {} essence you draft raises this tower's ceiling by one level.",
                def.element_label()
            ));
        } else {
            let b = egui::Button::new(RichText::new("Fully upgraded").size(11.5))
                .fill(pal::CARD)
                .min_size(vec2(112.0, 44.0));
            ui.add_enabled(false, b);
        }

        let tb =
            egui::Button::new(RichText::new(format!("Target\n{}", tw.mode.label())).size(11.5))
                .fill(pal::CARD)
                .min_size(vec2(96.0, 44.0));
        if ui
            .add(tb)
            .on_hover_text("Cycle targeting priority")
            .clicked()
        {
            action = Some(Action::Target);
        }

        let sb = egui::Button::new(RichText::new(format!("Sell\n{}g", tw.sell_value())).size(11.5))
            .fill(Color32::from_rgb(64, 32, 42))
            .min_size(vec2(90.0, 44.0));
        if ui.add(sb).clicked() {
            action = Some(Action::Sell);
        }
    });

    match action {
        Some(Action::Upgrade) => g.upgrade(ti),
        Some(Action::Target) => g.towers[ti].mode = g.towers[ti].mode.next(),
        Some(Action::Sell) => g.sell(ti),
        None => {}
    }
}

/// How many towers this support tower is currently feeding.
fn buffed_count(g: &Game, ti: usize) -> usize {
    let Some(t) = g.towers.get(ti) else { return 0 };
    let r = t.stats().range;
    g.towers
        .iter()
        .enumerate()
        .filter(|(i, o)| {
            *i != ti
                && !o.is_support()
                && (o.pos[0] - t.pos[0]).powi(2) + (o.pos[1] - t.pos[1]).powi(2) <= r * r
        })
        .count()
}

enum Action {
    Upgrade,
    Target,
    Sell,
}

fn build_palette(g: &mut Game, ui: &mut Ui, ust: &mut UiState) {
    let compact = ust.compact;
    let width = ui.available_width().max(card_w(compact) + 24.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, bar_h(compact)), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), pal::PANEL_DEEP);
    p.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, pal::LINE),
        StrokeKind::Inside,
    );
    p.text(
        rect.left_top() + vec2(10.0, 4.0),
        Align2::LEFT_TOP,
        "BUILD",
        FontId::monospace(9.0),
        pal::DIM,
    );

    // Cards are placed by hand inside a well computed from this panel. Nested
    // Uis and scroll areas kept centring the row and inventing vertical space,
    // which is what pushed the cards past the bottom edge.
    const PAD: f32 = 8.0;
    const LABEL_H: f32 = 16.0;
    const GAP: f32 = 6.0;
    let well = Rect::from_min_max(
        rect.left_top() + vec2(PAD, LABEL_H),
        rect.right_bottom() - vec2(PAD, PAD),
    );

    // Only what the draft has actually unlocked. Twenty-one cards of which
    // eighteen are unbuildable is not information, it is noise - what a locked
    // tower would cost you belongs in the draft screen, where the decision is.
    let unlocked: Vec<usize> = shop_order()
        .into_iter()
        .filter(|&i| g.unlocked(i))
        .collect();

    ust.hotkeys.clear();
    ust.card_rects.clear();
    ust.palette_rect = rect;

    if unlocked.is_empty() {
        ui.painter().text(
            well.center(),
            Align2::CENTER_CENTER,
            "Draft an essence to unlock a tower",
            FontId::proportional(12.5),
            pal::DIM,
        );
        return;
    }

    // Shrink to fit, then page. A player who drafts every element owns
    // twenty-one towers, and twenty-one cards do not fit across a phone at any
    // width still worth tapping - but a card the palette silently dropped is a
    // tower that cannot be built at all, which is far worse than a second page.
    // Wrapping to two rows was tried first: it halves the card height to below
    // a thumb, which fails for the same reason in the other axis.
    const MIN_W: f32 = 46.0;
    const PAGER_W: f32 = 26.0;
    let n = unlocked.len();
    let card_h = well.height().min(card_h(compact));

    // How many fit at the narrowest tappable size, leaving room for the pager
    // if one turns out to be needed.
    let room = |w: f32| (((w + GAP) / (MIN_W + GAP)).floor() as usize).max(1);
    let paged = room(well.width()) < n;
    let strip = if paged {
        well.width() - PAGER_W - GAP
    } else {
        well.width()
    };
    let per_page = room(strip).min(n.max(1));

    let pages = n.div_ceil(per_page.max(1)).max(1);
    let page = ust.palette_page.min(pages - 1);
    ust.palette_page = page;

    let shown: Vec<usize> = unlocked
        .iter()
        .skip(page * per_page)
        .take(per_page)
        .copied()
        .collect();
    let card_w = (((strip - GAP * (shown.len().saturating_sub(1)) as f32) / shown.len() as f32)
        .floor())
    .clamp(MIN_W, card_w(compact));

    for (slot, i) in shown.into_iter().enumerate() {
        ust.hotkeys.push(i);
        let x = well.left() + slot as f32 * (card_w + GAP);
        let card = Rect::from_min_size(pos2(x, well.top()), vec2(card_w, card_h));
        if card.right() > well.right() + 0.5 {
            break;
        }
        ust.card_rects.push(card);
        tower_card(g, ui, card, i, ust.build_tier, slot + 1);
    }

    if paged {
        let pager = Rect::from_min_size(
            pos2(well.right() - PAGER_W, well.top()),
            vec2(PAGER_W, card_h),
        );
        let resp = ui.interact(pager, ui.id().with("palette_page"), Sense::click());
        let p = ui.painter_at(pager);
        p.rect_filled(
            pager,
            CornerRadius::same(6),
            if resp.hovered() {
                pal::CARD_HOVER
            } else {
                pal::CARD
            },
        );
        p.text(
            pager.center() - vec2(0.0, 6.0),
            Align2::CENTER_CENTER,
            ">",
            FontId::monospace(14.0),
            pal::INK,
        );
        p.text(
            pager.center() + vec2(0.0, 10.0),
            Align2::CENTER_CENTER,
            format!("{}/{}", page + 1, pages),
            FontId::monospace(8.5),
            pal::DIM,
        );
        if resp.on_hover_text("More towers").clicked() {
            ust.palette_page = (page + 1) % pages;
        }
    }
}

/// The six essence counters. Small, always visible, and the only place the
/// player can see the shape of the run they have been drafting.
pub fn essence_strip(g: &Game, ui: &mut Ui, h: f32) {
    let chip = (h - 10.0) / 3.0;
    let (rect, _) = ui.allocate_exact_size(vec2(chip * 2.4, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), pal::PANEL_DEEP);
    p.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, pal::LINE),
        StrokeKind::Inside,
    );
    p.text(
        rect.left_top() + vec2(8.0, 3.0),
        Align2::LEFT_TOP,
        "ESSENCE",
        FontId::monospace(8.0),
        pal::DIM,
    );

    let cols = 2;
    let cw = (rect.width() - 14.0) / cols as f32;
    let ch = (rect.height() - 18.0) / 3.0;
    for (i, e) in ELEMENTS.iter().enumerate() {
        let n = g.essence[e.idx()];
        let cell = Rect::from_min_size(
            rect.left_top() + vec2(7.0 + (i % cols) as f32 * cw, 15.0 + (i / cols) as f32 * ch),
            vec2(cw - 2.0, ch - 2.0),
        );
        let lit = n > 0;
        p.circle_filled(
            pos2(cell.left() + 7.0, cell.center().y),
            4.5,
            c32(e.color(), if lit { 1.0 } else { 0.22 }),
        );
        p.text(
            pos2(cell.left() + 15.0, cell.center().y),
            Align2::LEFT_CENTER,
            format!("{} {}", e.glyph(), n),
            FontId::monospace(10.5),
            if lit { pal::INK } else { pal::DIM },
        );
    }

    let _ = ui
        .interact(rect, ui.id().with("essence_strip"), Sense::hover())
        .on_hover_ui(|ui| {
            ui.set_max_width(280.0);
            ui.label(
                RichText::new(format!(
                    "Essences  ({} of {})",
                    g.essences_held(),
                    ESSENCE_WAVES.len()
                ))
                .strong()
                .size(13.5),
            );
            ui.label(
                RichText::new(
                    "Every essence of an element raises the level ceiling of its pure tower, \
                 and of every dual tower that uses it, by one.",
                )
                .size(11.0)
                .color(pal::DIM),
            );
            for e in ELEMENTS {
                let n = g.essence[e.idx()];
                ui.label(
                    RichText::new(format!(
                        "{}  {}  -  ceiling level {}",
                        e.glyph(),
                        e.name(),
                        if n == 0 {
                            0
                        } else {
                            (FREE_TIERS + n as u32).min(MAX_TIER)
                        }
                    ))
                    .size(11.0)
                    .color(if n > 0 { c32(e.color(), 1.0) } else { pal::DIM }),
                );
            }
            match g.next_essence_wave() {
                Some(w) => ui.label(
                    RichText::new(format!("Next essence on wave {w}"))
                        .size(11.0)
                        .color(pal::GOLD),
                ),
                None => ui.label(
                    RichText::new("All essences drafted")
                        .size(11.0)
                        .color(pal::DIM),
                ),
            };
        });
}

/// Draws one build card into `rect`. Everything is positioned as a fraction of
/// the card, so it stays correct at any size the palette hands it.
fn tower_card(g: &mut Game, ui: &mut Ui, rect: Rect, def_i: usize, tier: u32, hotkey: usize) {
    let def = &TOWERS[def_i];
    let cap = g.tier_cap_of(def_i);
    // Never offer to build above the ceiling: a card the player can click and
    // then be silently refused is worse than one that shows the real number.
    let tier = tier.clamp(1, cap.max(1));
    let cost = def.cost_at(tier);
    let affordable = g.can_afford(cost);
    let selected = g.build_choice.map(|(d, _)| d) == Some(def_i);
    let col = tower_color(def);

    let resp = ui.interact(rect, ui.id().with(("build_card", def_i)), Sense::click());
    let p = ui.painter_at(rect);
    let bg = if selected || resp.hovered() {
        pal::CARD_HOVER
    } else {
        pal::CARD
    };
    p.rect_filled(rect, CornerRadius::same(7), bg);
    p.rect_stroke(
        rect,
        CornerRadius::same(7),
        Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected { pal::ACC } else { c32(col, 0.45) },
        ),
        StrokeKind::Inside,
    );

    // Lay out down the card as fractions of its height.
    let h = rect.height();
    let icon_side = (h * 0.36).min(rect.width() * 0.5);
    let icon = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + h * 0.27),
        vec2(icon_side, icon_side),
    );
    p.rect_filled(icon, CornerRadius::same(5), pal::PANEL_DEEP);
    p.rect_filled(
        Rect::from_center_size(icon.center(), vec2(icon_side * 0.52, icon_side * 0.52)),
        CornerRadius::same(3),
        c32(col, if affordable { 1.0 } else { 0.5 }),
    );
    // The one or two elements that unlock it, as pips on the icon. This is how
    // a player scanning the palette sees which of their essences a tower is
    // spending, without reading a word.
    for (n, e) in def.elements().enumerate() {
        p.circle_filled(
            pos2(icon.right() - 5.0, icon.top() + 5.0 + n as f32 * 8.0),
            3.5,
            c32(e.color(), 1.0),
        );
    }
    // Which layers it answers. A player choosing a tower mid-build phase needs
    // this at a glance, not buried in a tooltip.
    match def.targets {
        Targets::Both => {
            p.text(
                pos2(rect.right() - 5.0, rect.top() + 3.0),
                Align2::RIGHT_TOP,
                "AIR",
                FontId::monospace(8.0),
                c32(AIR_TINT, if affordable { 1.0 } else { 0.45 }),
            );
        }
        Targets::GroundOnly => {
            p.text(
                pos2(rect.right() - 5.0, rect.top() + 3.0),
                Align2::RIGHT_TOP,
                "GND",
                FontId::monospace(8.0),
                c32([0.72, 0.60, 0.42], if affordable { 1.0 } else { 0.45 }),
            );
        }
        Targets::Nothing => {}
    }

    p.text(
        pos2(rect.center().x, rect.top() + h * 0.53),
        Align2::CENTER_TOP,
        def.name,
        FontId::proportional(11.5),
        pal::INK,
    );
    p.text(
        pos2(rect.center().x, rect.top() + h * 0.68),
        Align2::CENTER_TOP,
        def.role,
        FontId::proportional(9.0),
        pal::DIM,
    );
    p.text(
        pos2(rect.center().x, rect.bottom() - h * 0.06),
        Align2::CENTER_BOTTOM,
        format!("{}g", gold_str(cost as i64)),
        FontId::monospace(11.5),
        if affordable { pal::GOLD } else { pal::BAD },
    );
    // The ceiling this tower can reach right now, so the choice between a
    // capped-at-3 dual and a capped-at-6 pure is visible before buying either.
    p.text(
        pos2(rect.right() - 5.0, rect.bottom() - h * 0.06),
        Align2::RIGHT_BOTTOM,
        format!("/{cap}"),
        FontId::monospace(9.0),
        pal::DIM,
    );
    p.text(
        rect.left_top() + vec2(5.0, 3.0),
        Align2::LEFT_TOP,
        format!("{hotkey}"),
        FontId::monospace(9.0),
        pal::DIM,
    );

    if resp.clicked() {
        g.build_choice = Some((def_i, tier));
        g.selected = None;
    }
    resp.on_hover_ui(|ui| tower_tooltip(ui, def_i, tier, cap));
}

fn tower_tooltip(ui: &mut Ui, def_i: usize, tier: u32, cap: u32) {
    let def = &TOWERS[def_i];
    ui.set_max_width(260.0);
    ui.label(
        RichText::new(def.name)
            .strong()
            .size(14.0)
            .color(c32(tower_color(def), 1.0)),
    );
    ui.horizontal(|ui| {
        for e in def.elements() {
            ui.label(
                RichText::new(e.name())
                    .size(10.5)
                    .strong()
                    .color(c32(e.color(), 1.0)),
            );
        }
    });
    ui.label(
        RichText::new(format!("{} · {} damage", def.role, def.dtype.name()))
            .size(10.5)
            .color(c32(def.dtype.color(), 1.0)),
    );
    ui.label(
        RichText::new(def.targets.label())
            .size(10.5)
            .strong()
            .color(match def.targets {
                Targets::Both => c32(AIR_TINT, 1.0),
                Targets::GroundOnly => c32([0.86, 0.68, 0.42], 1.0),
                Targets::Nothing => pal::DIM,
            }),
    );
    ui.label(RichText::new(def.desc).size(11.5).color(pal::DIM));
    ui.separator();
    let st = def.stats(tier);
    if def.dtype != Damage::None {
        kv(ui, "Damage", &short(st.dmg as f64));
        kv(ui, "Rate", &format!("{:.2}/s", st.rate));
        kv(ui, "DPS", &short((st.dmg * st.rate) as f64));
        // Chain and splash towers hit far more than one thing, so quote both.
        let eff = def.effective_dps_at(tier);
        if eff > st.dmg * st.rate * 1.05 {
            kv(ui, "DPS (spread)", &short(eff as f64));
        }
    }
    kv(ui, "Range", &format!("{:.1}", st.range));
    if let Delivery::Chain { bounces, hop, .. } = st.delivery {
        kv(ui, "Chain", &format!("{bounces} leaps, {hop:.1} reach"));
    }
    if let Delivery::Zone { radius, dur } = st.delivery {
        kv(
            ui,
            "Ground held",
            &format!("{radius:.1} across, lasts {dur:.1}s"),
        );
    }
    if st.splash > 0.0 {
        kv(ui, "Splash", &format!("{:.2}", st.splash));
    }
    kv(
        ui,
        "Cost",
        &format!("{}g", gold_str(def.cost_at(tier) as i64)),
    );
    for s in def.specials_for().iter() {
        ui.label(
            RichText::new(format!("• {}", s.describe(TowerDef::scale(tier))))
                .size(11.0)
                .color(pal::GOOD),
        );
    }
    ui.separator();
    let ceiling = def.stats(cap.max(1));
    ui.label(
        RichText::new(format!(
            "Your ceiling is level {cap}: about {} dps for {}g total.",
            short((ceiling.dmg * ceiling.rate) as f64),
            gold_str(def.cost_at(cap.max(1)) as i64)
        ))
        .size(10.5)
        .color(if cap >= MAX_TIER {
            pal::GOOD
        } else {
            pal::GOLD
        }),
    );
    if cap < MAX_TIER {
        let maxed = def.stats(MAX_TIER);
        ui.label(
            RichText::new(format!(
                "At level {MAX_TIER} it would do about {} dps. Draft {} to get there.",
                short((maxed.dmg * maxed.rate) as f64),
                def.element_label()
            ))
            .size(10.5)
            .color(pal::DIM),
        );
    }
    ui.label(
        RichText::new(format!(
            "Level {ATTUNE_TIER} attunes (stronger effect), level {ASCEND_TIER} ascends (+32% damage)."
        ))
        .size(10.0)
        .color(pal::DIM),
    );
}

fn kv(ui: &mut Ui, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(k).size(11.0).color(pal::DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(v).size(11.0).monospace().strong());
        });
    });
}

// ---------------------------------------------------------------- modals

pub fn modals(g: &mut Game, ctx: &Context, ust: &mut UiState) {
    if matches!(g.phase, Phase::Defeat | Phase::Victory) {
        game_over(g, ctx);
    } else if g.pending_draft.is_some() {
        // Ahead of the help overlay: the wave cannot start until this is
        // answered, so nothing else on screen matters yet.
        draft(g, ctx);
    }
    if ust.show_help {
        help(ctx, ust);
    }
}

/// The essence draft. Three elements, one choice, and the only screen in the
/// game the player cannot dismiss - the run is paused behind it because the
/// decision decides what the rest of the run is allowed to be.
fn draft(g: &mut Game, ctx: &Context) {
    let Some(offer) = g.pending_draft else { return };
    let mut taken: Option<usize> = None;

    egui::Modal::new(Id::new("essence_draft")).show(ctx, |ui| {
        ui.set_width(620.0);
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new("CHOOSE AN ESSENCE")
                    .size(20.0)
                    .strong()
                    .color(pal::INK),
            );
            ui.label(
                RichText::new(format!(
                    "Essence {} of {}  ·  before wave {}",
                    g.drafts_taken + 1,
                    ESSENCE_WAVES.len(),
                    g.wave + 1
                ))
                .size(11.5)
                .color(pal::DIM),
            );
        });
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            for (i, e) in offer.iter().enumerate() {
                let e = *e;
                let have = g.essence[e.idx()] as u32;
                let card = egui::Frame::NONE
                    .fill(pal::CARD)
                    .stroke(Stroke::new(1.5, c32(e.color(), 0.75)))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(12));
                card.show(ui, |ui| {
                    ui.set_width(178.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(e.name())
                                .size(17.0)
                                .strong()
                                .color(c32(e.color(), 1.0)),
                        );
                        ui.label(RichText::new(e.flavour()).size(11.0).color(pal::DIM));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("You hold {have}  ->  {}", have + 1))
                                .size(11.5)
                                .monospace()
                                .color(pal::GOLD),
                        );
                        ui.add_space(6.0);
                        draft_effects(ui, g, e);
                        ui.add_space(8.0);
                        let b = egui::Button::new(RichText::new("Take").size(13.0).strong())
                            .fill(c32(e.color(), 0.30))
                            .min_size(vec2(154.0, 32.0));
                        if ui.add(b).clicked() {
                            taken = Some(i);
                        }
                    });
                });
            }
        });
    });

    if let Some(i) = taken {
        g.take_essence(i);
    }
}

/// What taking this element would actually do: what it unlocks, and what it
/// raises. Without this the draft is a choice between three coloured words.
fn draft_effects(ui: &mut Ui, g: &Game, e: Element) {
    let held = g.essence[e.idx()] as u32;
    let mut after = g.essence;
    after[e.idx()] = after[e.idx()].saturating_add(1);

    // The ceiling line comes first, and it is the important one. An offer that
    // says only "unlocks five towers" sells the losing move: spreading into a
    // sixth element unlocks the most and caps everything at level five. A
    // simulated player who drafted that way died on wave 43 holding a board
    // with an answer to everything and the numbers to kill nothing.
    let ceiling = |n: u32| {
        if n == 0 {
            0
        } else {
            (FREE_TIERS + n).min(MAX_TIER)
        }
    };
    let (from, to) = (ceiling(held), ceiling(held + 1));
    ui.label(
        RichText::new(if from == to {
            format!("ceiling stays level {to}")
        } else {
            format!("ceiling  {from} -> level {to}")
        })
        .size(11.5)
        .monospace()
        .color(if to > from { pal::ACC } else { pal::DIM }),
    );

    let mut unlocks: Vec<&str> = Vec::new();
    let mut raises = 0usize;
    let mut raises_built = 0usize;
    for (i, d) in TOWERS.iter().enumerate() {
        let before = g.tier_cap_of(i);
        let now = tier_cap(&after, d);
        if before == 0 && now > 0 {
            unlocks.push(d.name);
        } else if now > before {
            raises += 1;
            raises_built += g.towers.iter().filter(|t| t.def == i).count();
        }
    }

    // What it raises on the board you already have beats what it unlocks on the
    // board you might build, so it is said first and in the brighter colour.
    if raises_built > 0 {
        ui.label(
            RichText::new(format!(
                "Raises {raises_built} tower{} you own",
                if raises_built == 1 { "" } else { "s" }
            ))
            .size(11.5)
            .strong()
            .color(pal::GOOD),
        );
    } else if raises > 0 {
        ui.label(
            RichText::new(format!("Raises the ceiling of {raises} towers"))
                .size(11.0)
                .color(pal::ACC),
        );
    }

    if unlocks.is_empty() {
        ui.label(
            RichText::new("Unlocks nothing new")
                .size(11.0)
                .color(pal::DIM),
        );
    } else {
        ui.label(
            RichText::new(format!("Unlocks {}", unlocks.len()))
                .size(10.0)
                .color(pal::DIM),
        );
        for n in unlocks.iter().take(3) {
            ui.label(RichText::new(format!("  {n}")).size(11.0).color(pal::INK));
        }
        if unlocks.len() > 3 {
            ui.label(
                RichText::new(format!("  and {} more", unlocks.len() - 3))
                    .size(10.5)
                    .color(pal::DIM),
            );
        }
    }
}

fn game_over(g: &mut Game, ctx: &Context) {
    let won = g.phase == Phase::Victory;
    let mut again = false;
    let mut carry_on = false;
    egui::Modal::new(Id::new("game_over"))
        .frame(
            egui::Frame::NONE
                .fill(pal::PANEL)
                .stroke(Stroke::new(1.0, pal::LINE))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(26.0),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(if won { "Victory" } else { "Overrun" })
                        .strong()
                        .size(30.0)
                        .color(if won { pal::GOOD } else { pal::BAD }),
                );
                ui.label(
                    RichText::new(if won {
                        format!("All {N_WAVES} waves cleared with {} lives left.", g.lives)
                    } else if g.endless {
                        format!("Endless run ended on wave {}.", g.wave)
                    } else {
                        format!("The road fell on wave {}.", g.wave)
                    })
                    .size(13.0)
                    .color(pal::DIM),
                );
                ui.add_space(12.0);
                egui::Grid::new("stats")
                    .spacing(vec2(24.0, 4.0))
                    .show(ui, |ui| {
                        for (k, v) in [
                            ("Kills", short(g.stats.kills as f64)),
                            ("Damage", short(g.stats.damage)),
                            ("Gold earned", gold_str(g.stats.gold_earned as i64)),
                            ("Net worth", gold_str(g.net_worth())),
                            ("Towers built", g.stats.towers_built.to_string()),
                            ("Leaked", g.stats.leaked.to_string()),
                        ] {
                            ui.label(RichText::new(k).color(pal::DIM));
                            ui.label(RichText::new(v).monospace());
                            ui.end_row();
                        }
                    });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if won {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Keep going (endless)").strong().size(14.0),
                                )
                                .fill(Color32::from_rgb(38, 120, 78))
                                .min_size(vec2(200.0, 34.0)),
                            )
                            .on_hover_text(
                                "The waves keep coming and keep growing. Score is how far you get.",
                            )
                            .clicked()
                        {
                            carry_on = true;
                        }
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Play again").strong().size(14.0))
                                .fill(Color32::from_rgb(43, 110, 190))
                                .min_size(vec2(150.0, 34.0)),
                        )
                        .clicked()
                    {
                        again = true;
                    }
                });
            });
        });
    if carry_on {
        g.continue_endless();
    } else if again {
        g.reset();
    }
}

fn help(ctx: &Context, ust: &mut UiState) {
    let mut close = false;
    egui::Modal::new(Id::new("help"))
        .frame(
            egui::Frame::NONE
                .fill(pal::PANEL)
                .stroke(Stroke::new(1.0, pal::LINE))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(22.0),
        )
        .show(ctx, |ui| {
            ui.set_max_width(600.0);
            ui.label(RichText::new("How to play").strong().size(20.0));
            ui.add_space(6.0);
            for (title, body) in [
                ("Essences decide your run", "Twenty times over the campaign you choose one element from three. Every element unlocks its own tower, and every PAIR of elements you hold unlocks the tower between them - six pure towers and fifteen duals, twenty-one in all."),
                ("Depth or breadth", "Each essence of an element raises the level ceiling of every tower using it by one, and a dual tower reads whichever of its two elements you have fewer of. Six of one element maxes its pure tower; six of each of two maxes the dual between them. Spreading wide unlocks answers, stacking deep buys numbers - you cannot have both."),
                ("Armour beats damage type", "Physical bounces off Plated but shreds Warded. Magic is the reverse, and it is the only real answer to Ethereal. Fire loves an unarmoured crowd and barely scratches a ghost. Toxic is never resisted and ignores shields. Read the wave preview before you buy."),
                ("Five towers cannot shoot up", "Boulder, Mire, Thornwall, Magma and Silt own the road and pay for it. Something in your board has to answer the air, and from wave 45 every escorted wave crosses the layers."),
                ("Interest, and calling early", "You earn 5% of the gold in hand every wave, up to a ceiling. Sending a wave early pays 2 gold for every second you skip - the only speed control that is also a decision."),
                ("Eighty waves, then forever", "Clearing wave 80 wins. You may keep going afterwards: endless waves grow faster than the purse does, so the only question is how far."),
            ] {
                ui.label(RichText::new(title).strong().size(13.0));
                ui.label(RichText::new(body).size(12.0).color(pal::DIM));
                ui.add_space(6.0);
            }
            ui.separator();
            ui.label(
                RichText::new("1-9 pick tower · Esc cancel · Space pause · F speed · Enter send wave · U upgrade · S sell · Shift+click keeps building")
                    .size(11.0)
                    .color(pal::DIM),
            );
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Got it").strong())
                            .fill(Color32::from_rgb(43, 110, 190))
                            .min_size(vec2(130.0, 30.0)),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
        });
    if close {
        ust.show_help = false;
    }
}

// ---------------------------------------------------------------- board overlay

/// Floating damage numbers and the transient toast, drawn over the 3D canvas.
pub fn board_text(g: &Game, ui: &Ui, cam: &Camera, rect: Rect) {
    let p = ui.painter_at(rect);
    for t in &g.texts {
        let Some(s) = cam.to_screen(v3(t.pos[0], t.pos[1], t.pos[2])) else {
            continue;
        };
        if !(0.0..=1.0).contains(&s[0]) || !(0.0..=1.0).contains(&s[1]) {
            continue;
        }
        let pos = pos2(
            rect.left() + s[0] * rect.width(),
            rect.top() + s[1] * rect.height(),
        );
        let a = t.t.clamp(0.0, 1.0);
        let (txt, col, size) = match t.kind {
            crate::game::TextKind::Damage => (
                short(t.value as f64),
                Color32::from_rgb(235, 240, 250),
                12.0,
            ),
            crate::game::TextKind::Crit => {
                (short(t.value as f64), Color32::from_rgb(255, 214, 92), 16.0)
            }
            crate::game::TextKind::Gold => (format!("+{}g", t.value as i64), pal::GOLD, 14.0),
            crate::game::TextKind::Life => ("+1 life".to_string(), pal::GOOD, 14.0),
            crate::game::TextKind::Leak => (format!("-{}", t.value as i64), pal::BAD, 17.0),
        };
        p.text(
            pos,
            Align2::CENTER_CENTER,
            txt,
            FontId::proportional(size),
            col.gamma_multiply(a),
        );
    }

    if let Some((msg, t)) = &g.toast {
        let a = (t / 2.2).clamp(0.0, 1.0);
        p.text(
            pos2(rect.center().x, rect.top() + 30.0),
            Align2::CENTER_CENTER,
            msg,
            FontId::proportional(15.0),
            pal::BAD.gamma_multiply(a),
        );
    }
}

/// Hover tooltip for a tower already on the board.
pub fn board_hover(g: &Game, resp: &Response, cam: &Camera, rect: Rect) {
    let Some(hover) = resp.hover_pos() else {
        return;
    };
    let u = (hover.x - rect.left()) / rect.width().max(1.0);
    let v = (hover.y - rect.top()) / rect.height().max(1.0);
    let Some(w) = cam.ground_pick(u, v) else {
        return;
    };
    let Some(slot) = g.board.slot_at(w) else {
        return;
    };
    let Some(ti) = g.tower_in_slot(slot) else {
        return;
    };
    if g.selected == Some(ti) {
        return;
    }
    let tw = &g.towers[ti];
    let def = tw.def();
    resp.show_tooltip_ui(|ui| {
        ui.label(
            RichText::new(format!("{} · tier {}", tw.full_name(), tw.tier))
                .strong()
                .color(c32(tower_color(def), 1.0)),
        );
        if !tw.is_support() {
            kv(ui, "DPS", &short((tw.dmg() * tw.rate()) as f64));
        }
        kv(ui, "Kills", &tw.kills.to_string());
        ui.label(RichText::new("Click to inspect").size(10.5).color(pal::DIM));
    });
}

pub fn _unused(_: Pos2) {}
