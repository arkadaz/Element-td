//! All egui chrome: the resource strip, the scoreboard, the minimap, the
//! command card and the build palette.
//!
//! The game state is the single source of truth - this module only reads it and
//! calls the same public `Game` methods a keyboard shortcut would.

use egui::{
    Align2, Color32, Context, CornerRadius, FontId, Id, Pos2, Rect, Response, RichText, Sense,
    Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::game::defs::*;
use crate::game::{FLOOD_LIMIT, Game, Phase, WAVE_PERIOD};
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

/// Warcraft III's own console: carved wood and stone under a gold rule.
///
/// The HUD used to be a flat blue-grey dashboard, which is a perfectly good
/// interface and looks nothing like the game this is a port of. Warcraft III's
/// console is warm, dark and bevelled - a wooden frame with stone insets, gold
/// trim, and parchment-coloured text - and the numbers on it are gold. These are
/// those colours.
pub mod pal {
    use egui::Color32;
    /// The wooden console body.
    pub const PANEL: Color32 = Color32::from_rgb(94, 79, 55);
    /// Stone inset: minimap wells, command slots, the leaderboard ground.
    pub const PANEL_DEEP: Color32 = Color32::from_rgb(28, 25, 19);
    /// A raised slot on the console.
    pub const CARD: Color32 = Color32::from_rgb(116, 98, 68);
    pub const CARD_HOVER: Color32 = Color32::from_rgb(146, 124, 84);
    /// The dark line that separates one carved piece from the next.
    pub const LINE: Color32 = Color32::from_rgb(38, 32, 23);
    /// The gold rule that runs along every edge of the console.
    pub const GOLD_LINE: Color32 = Color32::from_rgb(190, 152, 70);
    /// The bright top edge of a bevel, which is what makes wood look carved
    /// rather than painted.
    pub const BEVEL: Color32 = Color32::from_rgb(152, 130, 90);
    pub const INK: Color32 = Color32::from_rgb(236, 228, 206);
    pub const DIM: Color32 = Color32::from_rgb(186, 172, 140);
    pub const ACC: Color32 = Color32::from_rgb(96, 196, 255);
    pub const GOLD: Color32 = Color32::from_rgb(255, 206, 92);
    pub const BAD: Color32 = Color32::from_rgb(232, 88, 72);
    pub const GOOD: Color32 = Color32::from_rgb(126, 220, 104);
}

/// A carved panel: a dark ground, a lit top-left bevel, a shadowed bottom-right
/// one, and a gold rule around the whole thing.
///
/// Every box on the console goes through here, which is what makes the HUD read
/// as one carved object rather than as a dozen unrelated rectangles.
pub fn carved(ui: &Ui, r: Rect, fill: Color32, gold: bool) {
    let p = ui.painter();
    let cr = CornerRadius::same(4);
    p.rect_filled(r, cr, fill);
    // Lit edge along the top and left, shadow along the bottom and right.
    p.line_segment(
        [r.left_bottom() + vec2(1.0, -1.0), r.left_top() + vec2(1.0, 1.0)],
        Stroke::new(1.0, pal::BEVEL),
    );
    p.line_segment(
        [r.left_top() + vec2(1.0, 1.0), r.right_top() + vec2(-1.0, 1.0)],
        Stroke::new(1.0, pal::BEVEL),
    );
    p.line_segment(
        [
            r.right_top() + vec2(-1.0, 1.0),
            r.right_bottom() + vec2(-1.0, -1.0),
        ],
        Stroke::new(1.0, pal::LINE),
    );
    p.line_segment(
        [
            r.right_bottom() + vec2(-1.0, -1.0),
            r.left_bottom() + vec2(1.0, -1.0),
        ],
        Stroke::new(1.0, pal::LINE),
    );
    p.rect_stroke(
        r,
        cr,
        Stroke::new(
            1.0,
            if gold { pal::GOLD_LINE } else { pal::LINE },
        ),
        StrokeKind::Inside,
    );
}

/// The console itself: the wooden ground the strip and the command bar sit on.
///
/// egui fills a panel with one flat colour, which is what made the HUD read as a
/// dashboard. This paints the grain over it - a few darker bands, a gold rule
/// along the inside edge - so the console reads as a carved object.
pub fn console(ui: &Ui, r: Rect) {
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::ZERO, pal::PANEL);
    // Grain: a handful of darker bands across the wood.
    let n = (r.width() / 34.0) as i32;
    for k in 0..n {
        let x = r.left() + k as f32 * 34.0 + ((k * 37) % 11) as f32;
        p.rect_filled(
            Rect::from_min_size(pos2(x, r.top()), vec2(2.0, r.height())),
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(0, 0, 0, 18),
        );
    }
    // The gold rule along the edge that faces the board.
    let edge = if r.top() < 40.0 { r.bottom() } else { r.top() };
    p.line_segment(
        [pos2(r.left(), edge), pos2(r.right(), edge)],
        Stroke::new(2.0, pal::GOLD_LINE),
    );
}

/// A command slot: the square icon well a build or upgrade button sits in.
///
/// Warcraft III's command card is a four by three grid of these, and the frame
/// is what tells you a slot is a slot even when it is empty.
pub fn slot_frame(ui: &Ui, r: Rect, hot: bool, chosen: bool) {
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(3), pal::PANEL_DEEP);
    if hot {
        p.rect_filled(r.shrink(2.0), CornerRadius::same(2), pal::CARD_HOVER);
    }
    p.rect_stroke(
        r,
        CornerRadius::same(3),
        Stroke::new(
            if chosen { 2.0 } else { 1.0 },
            if chosen { pal::GOLD } else { pal::GOLD_LINE },
        ),
        StrokeKind::Inside,
    );
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
        style.visuals.window_corner_radius = CornerRadius::same(4);
        style.visuals.widgets.inactive.corner_radius = CornerRadius::same(3);
        style.visuals.widgets.hovered.corner_radius = CornerRadius::same(3);
        style.visuals.widgets.active.corner_radius = CornerRadius::same(3);
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, pal::GOLD_LINE);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, pal::GOLD);
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
    console(ui, full.expand(8.0));
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
    let send_bonus = (g.wave_timer * EARLY_BONUS_PER_SEC) as i32;
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
    // Not lives - there are none. The number that decides the run is how many
    // monsters are still going round, against what the ring will hold.
    let circling = format!("{}/{}", g.creeps.len(), FLOOD_LIMIT);
    let flood = g.flood();
    let value_size = if compact { 15.0 } else { 18.0 };
    let chips: [(&str, &str, Color32); 3] = [
        (
            "WAVE",
            wave.as_str(),
            if g.endless { pal::ACC } else { pal::INK },
        ),
        (
            "CIRCLING",
            circling.as_str(),
            // Green while there is room, amber once the ring is filling,
            // red when it is nearly over. The colour is the warning.
            if flood > 0.8 {
                pal::BAD
            } else if flood > 0.55 {
                pal::GOLD
            } else {
                pal::GOOD
            },
        ),
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
    carved(ui, r.shrink2(vec2(0.0, 3.0)), pal::PANEL_DEEP, true);
    let p = ui.painter();
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
    // The map tags its own waves - Air, Immune, Hero, Boss - and those four
    // words are the whole of what a player has to react to.
    let flagged = !w.tag.is_empty();
    carved(ui, rect, pal::PANEL_DEEP, flagged);
    let p = ui.painter();
    if flagged {
        p.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(2.0, c32(w.armour_type.color(), 0.95)),
            StrokeKind::Inside,
        );
    }

    let title = match g.phase {
        Phase::Combat if g.endless => format!("ENDLESS - WAVE {}", g.wave),
        Phase::Combat => format!("WAVE {} OF {}", g.wave, N_WAVES),
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

    let line = format!(
        "{} x{}  ·  {} armour {}",
        w.name,
        w.count,
        w.armour,
        w.armour_type.name()
    );
    let mut tx = rect.left() + 8.0;
    if flagged {
        // A filled badge, not a word in a sentence. Missing an Air wave with no
        // anti-air, or an Immune wave with no Chaos, decides the run.
        let tint = if w.flying {
            AIR_TINT
        } else {
            w.armour_type.color()
        };
        let label = w.tag.to_ascii_uppercase();
        let bw = 12.0 + label.len() as f32 * 7.0;
        let badge = Rect::from_min_size(pos2(tx, rect.top() + 17.0), vec2(bw, 14.0));
        p.rect_filled(badge, CornerRadius::same(4), c32(tint, 1.0));
        p.text(
            badge.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(9.0),
            Color32::from_rgb(10, 14, 24),
        );
        tx += bw + 6.0;
    }
    p.text(
        pos2(tx, rect.top() + 17.0),
        Align2::LEFT_TOP,
        elide(
            ui,
            &line,
            &FontId::proportional(12.0),
            rect.right() - tx - 8.0,
        ),
        FontId::proportional(12.0),
        c32(w.armour_type.color(), 1.0),
    );

    if g.wave < g.last_wave() {
        let frac = (g.wave_timer / WAVE_PERIOD.max(1.0)).clamp(0.0, 1.0);
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
            format!("{:.0}s", g.wave_timer.max(0.0)),
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
            RichText::new(w.name)
                .strong()
                .color(c32(w.armour_type.color(), 1.0)),
        );
        ui.label(
            RichText::new(format!(
                "{} health · {} armour ({:.0}% taken)",
                short(w.hp as f64),
                w.armour,
                armour_mult(w.armour) * 100.0
            ))
            .size(11.0)
            .color(pal::DIM),
        );
        ui.label(RichText::new(w.armour_type.counter()).size(11.0).color(
            if w.armour_type == ArmourType::Divine {
                pal::BAD
            } else {
                pal::GOOD
            },
        ));
        if w.flying {
            ui.label(
                RichText::new(
                    "Flies. Only Air Towers and the towers that cover both can reach it.",
                )
                .size(11.0)
                .color(c32(AIR_TINT, 1.0)),
            );
        }
        ui.label(
            RichText::new(format!(
                "Pays {} gold a kill",
                gold_str(bounty_of(&w) as i64)
            ))
            .size(11.0)
            .color(pal::GOLD),
        );
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
                "Bounty a kill",
                gold_str(bounty_for(g.wave.max(1)) as i64),
                pal::GOLD,
            ));
            rows.push((
                "Next wave pays",
                format!("+{}", gold_str(wave_clear_bonus(g.wave + 1) as i64)),
                pal::GOLD,
            ));
            rows.push(("", String::new(), pal::DIM));
            rows.push((
                "Circling",
                format!("{} / {}", g.creeps.len(), FLOOD_LIMIT),
                if g.flood() > 0.7 { pal::BAD } else { pal::GOOD },
            ));
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

            // Warcraft III's own leaderboard, headed by the map's name and the
            // line the map itself puts on it: "When enemies > 700, game over."
            ui.label(
                RichText::new("Green Circle TD")
                    .size(14.0)
                    .strong()
                    .color(pal::GOOD),
            );
            ui.label(
                RichText::new(format!("When enemies > {FLOOD_LIMIT}, game over."))
                    .size(10.0)
                    .color(pal::DIM),
            );
            ui.add_space(5.0);
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
    console(ui, ui.available_rect_before_wrap().expand(8.0));
    ui.horizontal(|ui| {
        // On a phone the board is the scarce resource: drop the minimap first,
        // then the selection panel, before ever shrinking the build palette.
        if !compact {
            minimap(g, ui, bar_h(compact));
            ui.add_space(6.0);
        }
        // What is coming in the next few waves. On a circuit there is no
        // build phase to plan in, so the only place to see an Air or an Immune
        // wave before it arrives is here.
        if !compact && width > TINY_WIDTH {
            wave_ladder(g, ui, bar_h(compact));
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
    // The minimap sits in a carved stone well, which is where Warcraft III puts
    // it and why it reads as part of the console rather than as a widget.
    carved(ui, rect, pal::PANEL, true);
    carved(ui, rect.shrink(5.0), pal::PANEL_DEEP, false);
    let p = ui.painter();

    // The player's arena, not the whole field. Seven eighths of the map belongs
    // to other players, and drawing it made the minimap a postage stamp of
    // arena in a sea of black.
    let a = crate::game::greentd_map::ARENA;
    let (aw, ah) = (a[2] - a[0], a[3] - a[1]);
    let inner = rect.shrink(10.0);
    let s = (inner.width() / aw).min(inner.height() / ah);
    let ox = inner.center().x - aw * s * 0.5 - a[0] * s;
    let oy = inner.center().y - ah * s * 0.5 - a[1] * s;
    let map = |w: [f32; 2]| pos2(ox + w[0] * s, oy + w[1] * s);

    // Ground plate.
    p.rect_filled(
        Rect::from_min_size(map([a[0], a[1]]), vec2(aw * s, ah * s)),
        CornerRadius::same(2),
        Color32::from_rgb(22, 40, 18),
    );
    // The corridors, so the minimap is recognisably the level.
    for ty in a[1] as i32..=a[3] as i32 {
        for tx in a[0] as i32..=a[2] as i32 {
            if crate::game::board::is_corridor(tx, ty) {
                p.rect_filled(
                    Rect::from_min_size(map([tx as f32, ty as f32]), vec2(s + 0.6, s + 0.6)),
                    CornerRadius::ZERO,
                    Color32::from_rgb(56, 50, 38),
                );
            }
        }
    }
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
        let r = if c.is_boss() { 3.5 } else { 2.0 };
        p.circle_filled(map(c.pos), r, c32(c.armour_type.color(), 1.0));
    }
    // Where monsters enter the ring. There is no exit - that is the game.
    p.circle_filled(map(g.board.sample(0.9)), 3.5, pal::BAD);
}

fn selection_panel(g: &mut Game, ui: &mut Ui, compact: bool) {
    let w = if compact { 268.0 } else { 340.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(w, bar_h(compact)), Sense::hover());
    carved(ui, rect, pal::PANEL, true);
    let p = ui.painter();

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
    let choices = tw.upgrades();

    // Portrait frame.
    // The portrait: a carved stone well with the tower's attack colour in it,
    // exactly where Warcraft III puts a unit's face.
    let port = Rect::from_min_size(rect.left_top() + vec2(10.0, 10.0), vec2(64.0, 64.0));
    carved(ui, port, pal::PANEL_DEEP, true);
    let p = ui.painter();
    p.rect_filled(
        Rect::from_center_size(port.center(), vec2(30.0, 30.0)),
        CornerRadius::same(3),
        c32(col, 1.0),
    );
    // A Siege Tower has twenty levels and a Poison Tower fifteen, so the step
    // is only meaningful next to the length of its own path.
    p.text(
        port.center_bottom() + vec2(0.0, -11.0),
        Align2::CENTER_CENTER,
        format!("{} / {}", tw.level(), tw.ladder_len()),
        FontId::monospace(10.5),
        pal::GOLD,
    );

    let tx = port.right() + 10.0;
    let name_font = FontId::proportional(15.0);
    p.text(
        pos2(tx, port.top() + 1.0),
        Align2::LEFT_TOP,
        elide(ui, tw.full_name(), &name_font, rect.right() - tx - 10.0),
        name_font,
        c32(col, 1.0),
    );
    p.text(
        pos2(tx, port.top() + 20.0),
        Align2::LEFT_TOP,
        format!(
            "{} · {} · {}",
            def.family.name(),
            def.attack.name(),
            def.targets.label()
        ),
        FontId::proportional(10.5),
        c32(col, 0.9),
    );

    let dps = tw.dmg() * tw.rate();
    let stats_line = if tw.is_support() {
        format!("aura radius {:.1}", def.abil.aura_range.max(tw.range()))
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

    // An aura tower has no damage number, so show what it HAS done - otherwise
    // it reads as a wasted pad.
    let contribution = if tw.is_support() {
        let n = buffed_count(g, ti);
        format!("buffing {n} tower{}", if n == 1 { "" } else { "s" })
    } else {
        format!(
            "{} kills   ·   {} dealt   ·   {} earned",
            tw.kills,
            short(tw.damage),
            gold_str(tw.gold_earned as i64)
        )
    };
    p.text(
        pos2(tx, port.top() + 52.0),
        Align2::LEFT_TOP,
        contribution,
        FontId::monospace(10.5),
        pal::DIM,
    );
    if tw.buff_dmg > 0.0 || tw.buff_rate > 0.0 {
        p.text(
            pos2(tx, port.top() + 68.0),
            Align2::LEFT_TOP,
            format!(
                "Aura: +{:.0}% damage, +{:.0}% rate",
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
        match choices.len() {
            0 => {
                let b = egui::Button::new(RichText::new("Fully upgraded").size(11.5))
                    .fill(pal::CARD)
                    .min_size(vec2(112.0, 44.0));
                ui.add_enabled(false, b);
            }
            1 => {
                let (into, cost) = choices[0];
                let can = g.can_afford(cost);
                let label = format!(
                    "{}\n{}g",
                    elide(ui, TOWERS[into].name, &FontId::proportional(12.0), 104.0),
                    gold_str(cost as i64)
                );
                let b = egui::Button::new(RichText::new(label).size(12.0).strong())
                    .fill(if can {
                        Color32::from_rgb(43, 110, 190)
                    } else {
                        pal::CARD
                    })
                    .min_size(vec2(112.0, 44.0));
                if ui
                    .add_enabled(can, b)
                    .on_hover_ui(|ui| tower_tooltip(ui, into))
                    .clicked()
                {
                    action = Some(Action::Upgrade);
                }
            }
            // A fork. The choice is too big for this panel, so the build
            // palette below turns into the branch list - which is exactly what
            // Warcraft III's own command card does.
            n => {
                let b = egui::Button::new(
                    RichText::new(format!("{n} ways up\nchoose below"))
                        .size(11.5)
                        .strong(),
                )
                .fill(Color32::from_rgb(96, 74, 26))
                .min_size(vec2(112.0, 44.0));
                ui.add_enabled(false, b);
            }
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

        let sb = egui::Button::new(
            RichText::new(format!("Sell\n{}g", gold_str(tw.sell_value() as i64))).size(11.5),
        )
        .fill(Color32::from_rgb(64, 32, 42))
        .min_size(vec2(90.0, 44.0));
        if ui
            .add(sb)
            .on_hover_text("The map refunds what you paid, in full")
            .clicked()
        {
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

/// How many towers this aura tower is currently feeding.
fn buffed_count(g: &Game, ti: usize) -> usize {
    let Some(t) = g.towers.get(ti) else { return 0 };
    let r = t.abil().aura_range.max(t.def().range);
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

/// The command card.
///
/// Normally the eleven towers that can be bought outright. When the selected
/// tower has more than one way up - the ten gold seed with its six families,
/// an Aura Tower choosing Damage or Speed, the King Tower opening the four
/// Supers - it becomes that list instead, because a fork with six branches
/// does not fit anywhere else and is the most important decision in the game.
fn build_palette(g: &mut Game, ui: &mut Ui, ust: &mut UiState) {
    let compact = ust.compact;
    let width = ui.available_width().max(card_w(compact) + 24.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, bar_h(compact)), Sense::hover());
    carved(ui, rect, pal::PANEL, true);
    let p = ui.painter();

    // What the card is showing right now.
    let branching = g
        .selected
        .filter(|&i| i < g.towers.len())
        .filter(|&i| g.towers[i].upgrades().len() > 1);
    let entries: Vec<usize> = match branching {
        Some(ti) => g.towers[ti]
            .upgrades()
            .into_iter()
            .map(|(i, _)| i)
            .collect(),
        None => shop_order(),
    };
    p.text(
        rect.left_top() + vec2(10.0, 4.0),
        Align2::LEFT_TOP,
        if branching.is_some() {
            "UPGRADE INTO"
        } else {
            "BUILD"
        },
        FontId::monospace(9.0),
        if branching.is_some() {
            pal::GOLD
        } else {
            pal::DIM
        },
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

    ust.hotkeys.clear();
    ust.card_rects.clear();
    ust.palette_rect = rect;

    if entries.is_empty() {
        ui.painter().text(
            well.center(),
            Align2::CENTER_CENTER,
            "Nothing to build",
            FontId::proportional(12.5),
            pal::DIM,
        );
        return;
    }

    // Shrink to fit, then page. A card the palette silently dropped is a tower
    // that cannot be built at all, which is far worse than a second page.
    // Wrapping to two rows was tried first: it halves the card height to below
    // a thumb, which fails for the same reason in the other axis.
    const MIN_W: f32 = 46.0;
    const PAGER_W: f32 = 26.0;
    let n = entries.len();
    let card_h = well.height().min(card_h(compact));

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

    let shown: Vec<usize> = entries
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
        tower_card(g, ui, card, i, branching, slot + 1);
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

/// The next few waves, so an Air or an Immune wave is never a surprise.
///
/// On a circuit there is no build phase to plan in - the wave clock never
/// stops - so the only place a player can see what is coming is a standing
/// panel like this one.
pub fn wave_ladder(g: &Game, ui: &mut Ui, h: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(122.0, h), Sense::hover());
    carved(ui, rect, pal::PANEL, true);
    let p = ui.painter();
    p.text(
        rect.left_top() + vec2(8.0, 3.0),
        Align2::LEFT_TOP,
        "COMING",
        FontId::monospace(8.0),
        pal::DIM,
    );

    const ROWS: u32 = 5;
    let row_h = (rect.height() - 18.0) / ROWS as f32;
    for k in 0..ROWS {
        let n = g.wave + 1 + k;
        if n > g.last_wave() {
            break;
        }
        let w = wave_at(n);
        let y = rect.top() + 16.0 + k as f32 * row_h;
        let col = c32(w.armour_type.color(), if k == 0 { 1.0 } else { 0.75 });
        p.text(
            pos2(rect.left() + 8.0, y + row_h * 0.5),
            Align2::LEFT_CENTER,
            format!("{n}"),
            FontId::monospace(if k == 0 { 11.5 } else { 10.0 }),
            if k == 0 { pal::INK } else { pal::DIM },
        );
        let tag = if !w.tag.is_empty() {
            w.tag
        } else {
            w.armour_type.name()
        };
        p.text(
            pos2(rect.left() + 28.0, y + row_h * 0.5),
            Align2::LEFT_CENTER,
            tag,
            FontId::proportional(if k == 0 { 11.5 } else { 10.0 }),
            col,
        );
        if w.flying {
            p.circle_filled(
                pos2(rect.right() - 10.0, y + row_h * 0.5),
                3.5,
                c32(AIR_TINT, 1.0),
            );
        }
    }

    let _ = ui
        .interact(rect, ui.id().with("wave_ladder"), Sense::hover())
        .on_hover_ui(|ui| {
            ui.set_max_width(300.0);
            ui.label(RichText::new("What is coming").strong().size(13.5));
            for k in 0..8u32 {
                let n = g.wave + 1 + k;
                if n > g.last_wave() {
                    break;
                }
                let w = wave_at(n);
                ui.label(
                    RichText::new(format!(
                        "{n}. {} x{}  ·  {} armour {}{}",
                        w.name,
                        w.count,
                        w.armour,
                        w.armour_type.name(),
                        if w.flying { "  ·  FLYING" } else { "" }
                    ))
                    .size(11.0)
                    .color(c32(w.armour_type.color(), 1.0)),
                );
            }
        });
}

/// Draws one card into `rect`. Everything is positioned as a fraction of the
/// card, so it stays correct at any size the palette hands it.
///
/// `upgrading` is the tower this card would upgrade, when the palette is
/// showing a fork rather than the shop.
fn tower_card(
    g: &mut Game,
    ui: &mut Ui,
    rect: Rect,
    def_i: usize,
    upgrading: Option<usize>,
    hotkey: usize,
) {
    let def = &TOWERS[def_i];
    let cost = def.gold;
    let affordable = g.can_afford(cost);
    let selected = upgrading.is_none() && g.build_choice.map(|(d, _)| d) == Some(def_i);
    let col = tower_color(def);

    let resp = ui.interact(rect, ui.id().with(("build_card", def_i)), Sense::click());
    slot_frame(ui, rect, resp.hovered(), selected);
    let p = ui.painter_at(rect);

    // Lay out down the card as fractions of its height. The icon is a square
    // well with the tower's attack colour in it, which is as close to Warcraft
    // III's hand-painted command icons as a renderer with no textures gets.
    let h = rect.height();
    let icon_side = (h * 0.40).min(rect.width() * 0.72);
    let icon = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + h * 0.28),
        vec2(icon_side, icon_side),
    );
    p.rect_filled(icon, CornerRadius::same(2), Color32::from_rgb(12, 11, 9));
    p.rect_filled(
        icon.shrink(icon_side * 0.16),
        CornerRadius::same(2),
        c32(col, if affordable { 1.0 } else { 0.42 }),
    );
    p.rect_stroke(
        icon,
        CornerRadius::same(2),
        Stroke::new(1.0, pal::GOLD_LINE),
        StrokeKind::Inside,
    );
    // Which layers it answers. Missing that a tower cannot reach the air is
    // what loses wave 7, and it belongs on the card rather than in a tooltip.
    let (tag, tint) = match def.targets {
        Targets::Both => ("AIR", AIR_TINT),
        Targets::AirOnly => ("AIR!", AIR_TINT),
        Targets::GroundOnly => ("GND", [0.72, 0.60, 0.42]),
        Targets::Nothing => ("", [0.0; 3]),
    };
    if !tag.is_empty() {
        p.text(
            pos2(rect.right() - 5.0, rect.top() + 3.0),
            Align2::RIGHT_TOP,
            tag,
            FontId::monospace(8.0),
            c32(tint, if affordable { 1.0 } else { 0.45 }),
        );
    }

    let name_font = FontId::proportional(11.5);
    p.text(
        pos2(rect.center().x, rect.top() + h * 0.53),
        Align2::CENTER_TOP,
        elide(ui, def.family.short(), &name_font, rect.width() - 6.0),
        name_font,
        pal::INK,
    );
    // Elided, not clipped. The palette shrinks its cards to fit, so a fixed
    // string will eventually be wider than the card - and a centred one then
    // loses characters off *both* ends.
    let role_font = FontId::proportional(9.0);
    p.text(
        pos2(rect.center().x, rect.top() + h * 0.68),
        Align2::CENTER_TOP,
        elide(ui, def.attack.name(), &role_font, rect.width() - 6.0),
        role_font,
        c32(def.attack.color(), 0.9),
    );
    let cost_c = pos2(rect.center().x, rect.bottom() - h * 0.10);
    p.circle_filled(pos2(cost_c.x - 22.0, cost_c.y), 4.0, pal::GOLD);
    p.text(
        pos2(cost_c.x - 14.0, cost_c.y),
        Align2::LEFT_CENTER,
        gold_str(cost as i64),
        FontId::monospace(11.5),
        if affordable { pal::GOLD } else { pal::BAD },
    );
    // The hotkey, in its own corner box. Warcraft III underlines a letter in the
    // tooltip; a corner box is the same information where a card is this small.
    let key = Rect::from_min_size(rect.left_top() + vec2(3.0, 3.0), vec2(13.0, 13.0));
    p.rect_filled(key, CornerRadius::same(2), pal::PANEL_DEEP);
    p.text(
        key.center(),
        Align2::CENTER_CENTER,
        format!("{hotkey}"),
        FontId::monospace(9.5),
        pal::GOLD,
    );

    if resp.clicked() {
        match upgrading {
            Some(ti) => g.upgrade_into(ti, def_i),
            None => {
                g.build_choice = Some((def_i, 1));
                g.selected = None;
            }
        }
    }
    resp.on_hover_ui(|ui| tower_tooltip(ui, def_i));
}

/// Shortens `text` until it fits `max_w`, ending in two dots.
///
/// Two dots rather than a single ellipsis character on purpose: the HUD's
/// font coverage is asserted by a test, and `…` is exactly the kind of glyph
/// that is missing from a bundled font and renders as a tofu box.
pub(crate) fn elide(ui: &Ui, text: &str, font: &FontId, max_w: f32) -> String {
    let width = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), pal::DIM)
            .rect
            .width()
    };
    if width(text) <= max_w {
        return text.to_string();
    }
    let mut best = String::new();
    for (i, _) in text.char_indices() {
        let candidate = format!("{}..", &text[..i]);
        if width(&candidate) > max_w {
            break;
        }
        best = candidate;
    }
    best
}

fn tower_tooltip(ui: &mut Ui, def_i: usize) {
    let def = &TOWERS[def_i];
    ui.set_max_width(280.0);
    ui.label(
        RichText::new(def.name)
            .strong()
            .size(14.0)
            .color(c32(tower_color(def), 1.0)),
    );
    ui.label(
        RichText::new(format!(
            "{} · step {} of {}",
            def.family.name(),
            def.step + 1,
            ladder_len(def.family)
        ))
        .size(10.5)
        .color(pal::DIM),
    );
    ui.label(
        RichText::new(format!(
            "{} damage - {}",
            def.attack.name(),
            def.attack.note()
        ))
        .size(10.5)
        .color(c32(def.attack.color(), 1.0)),
    );
    ui.label(
        RichText::new(def.targets.label())
            .size(10.5)
            .strong()
            .color(match def.targets {
                Targets::Both | Targets::AirOnly => c32(AIR_TINT, 1.0),
                Targets::GroundOnly => c32([0.86, 0.68, 0.42], 1.0),
                Targets::Nothing => pal::DIM,
            }),
    );
    ui.label(RichText::new(def.family.role()).size(11.5).color(pal::DIM));
    ui.separator();
    if def.attacks() {
        kv(ui, "Damage", &short(def.damage as f64));
        kv(
            ui,
            "Rate",
            &format!("{:.2}/s", 1.0 / def.cooldown.max(0.05)),
        );
        kv(ui, "DPS", &short(def.dps() as f64));
        let eff = def.effective_dps();
        if eff > def.dps() * 1.05 {
            kv(ui, "DPS (spread)", &short(eff as f64));
        }
    }
    kv(ui, "Range", &format!("{:.1}", def.range));
    if def.splash > 0.0 {
        kv(ui, "Splash", &format!("{:.2}", def.splash));
    }
    kv(ui, "Cost", &format!("{}g", gold_str(def.gold as i64)));
    kv(
        ui,
        "Sells for",
        &format!("{}g", gold_str(def.refund as i64)),
    );

    for line in ability_lines(&def.abil) {
        ui.label(
            RichText::new(format!("• {line}"))
                .size(11.0)
                .color(pal::GOOD),
        );
    }

    if !def.upgrades.is_empty() {
        ui.separator();
        let next: Vec<String> = def
            .upgrades
            .iter()
            .map(|&u| {
                let t = &TOWERS[u as usize];
                format!("{} ({}g)", t.name, gold_str(t.gold as i64))
            })
            .collect();
        ui.label(
            RichText::new(format!("Upgrades into: {}", next.join(", ")))
                .size(10.5)
                .color(pal::GOLD),
        );
    }
}

/// Everything an ability block does, in the map's own numbers.
pub fn ability_lines(a: &Abil) -> Vec<String> {
    let mut v = Vec::new();
    if a.crit_chance > 0.0 {
        v.push(format!(
            "{:.0}% chance of {:.0}x damage",
            a.crit_chance * 100.0,
            a.crit_mult
        ));
    }
    if a.multishot > 1 {
        v.push(format!("Attacks {} targets at once", a.multishot));
    }
    if a.bounce > 0 {
        v.push(format!("The shot leaps to {} more targets", a.bounce));
    }
    if a.poison_dps > 0.0 {
        v.push(format!(
            "Poison: {}/s and {:.0}% slower for {:.0}s",
            short(a.poison_dps as f64),
            a.poison_slow * 100.0,
            a.poison_dur
        ));
    }
    if a.slow_amt > 0.0 && a.slow_range > 0.0 {
        v.push(format!(
            "Everything within {:.1} moves {:.0}% slower",
            a.slow_range,
            a.slow_amt * 100.0
        ));
    }
    if a.dmg_aura > 0.0 {
        v.push(format!(
            "Towers within {:.1} deal +{:.0}% damage",
            a.aura_range,
            a.dmg_aura * 100.0
        ));
    }
    if a.speed_aura > 0.0 {
        v.push(format!(
            "Towers within {:.1} attack {:.0}% faster",
            a.aura_range,
            a.speed_aura * 100.0
        ));
    }
    if a.burn_dps > 0.0 {
        v.push(format!(
            "Burns everything within {:.1} for {}/s",
            a.burn_range,
            short(a.burn_dps as f64)
        ));
    }
    if a.root_chance > 0.0 {
        v.push(format!(
            "{:.0}% chance to root for {:.0}s",
            a.root_chance * 100.0,
            a.root_dur
        ));
    }
    if a.kill_chance > 0.0 {
        v.push(format!(
            "{:.0}% chance to kill outright",
            a.kill_chance * 100.0
        ));
    }
    if a.armour_pen > 0 {
        v.push(format!("Strips {} armour off what it hits", a.armour_pen));
    }
    if a.frenzy > 0.0 {
        v.push(format!(
            "Works itself up: +{:.0}% attack rate for {:.0}s every {:.0}s",
            a.frenzy * 100.0,
            a.frenzy_dur,
            a.frenzy_cd
        ));
    }
    v
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
    }
    if ust.show_help {
        help(ctx, ust);
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
                        format!("All {N_WAVES} waves cleared, and the ring emptied.",)
                    } else if g.endless {
                        format!("Endless run ended on wave {}.", g.wave)
                    } else {
                        format!("The ring overflowed on wave {}.", g.wave)
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
                            // Not "leaked" - nothing can leak off a circuit,
                            // and a stat that is always zero is worse than no
                            // stat at all. How full the ring ever got is the
                            // number that describes the run.
                            (
                                "Fullest ring",
                                format!("{} / {FLOOD_LIMIT}", g.stats.peak_circling),
                            ),
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
                ("The lane is a shuttle", "The map orders your creeps down a three-tile corridor and then sends them back up it, forever. There is no exit and no lives: what you defend is a RATE. Anything your towers cannot kill comes round again, and the field fills up."),
                ("Seven hundred and it is over", "The map's own leaderboard says it: when enemies pass seven hundred, the game ends. The CIRCLING gauge counts what is still walking against that. Green is comfortable, amber is falling behind, red is nearly over."),
                ("Waves never stop", "A wave arrives every forty-five to fifty seconds whether the last one is dead or not, and its whole count streams in evenly across forty-five of those. There is no build phase - you spend gold while the corridor is busy."),
                ("Eleven towers, and a graph behind them", "Eleven can be bought. The other hundred and twenty are reached by upgrading, and upgrading is a graph rather than a ladder: most towers have one next step, but three of them fork."),
                ("The ten gold seed", "The cheapest tower is a Single shot Tower at ten gold, and it is a door. It becomes a Slow, Poison, Critical, Troll, Fire or One-Strike Kill Tower, and none of those six can be bought at any price."),
                ("Every fifth wave is Immune", "An Immune wave takes five percent from everything - Normal, Siege, Magic, Spells, all of it - except Chaos, which it takes in full, and Hero, which is multiplied by a hundred. Chaos, Destruction, Troll and the One-Strike Kill Tower exist for those waves and for nothing else."),
                ("Armour is a number", "It is not a category. Each point takes six percent off what lands, with diminishing returns, and it climbs to seven hundred - which is two percent of the damage getting through. That is why the roster ends in six figures."),
                ("Something has to answer the air", "Waves 7, 17, 23, 27 and 35 fly. The Siege ladder is the longest in the game and never elevates; nine of the ten Air Towers can hit nothing else. Buy one before wave seven."),
                ("Waves are streams, not bursts", "A wave is up to a hundred and sixty monsters arriving one at a time. Splash, bouncing shots and multishot are worth far more than one enormous hit, because what kills you is throughput."),
                ("Thirty-six waves, then forever", "You win by surviving all thirty-six AND clearing the field - outlasting the last stream is not the same as killing it. You may keep going afterwards; endless waves grow faster than the purse does."),
            ] {
                ui.label(RichText::new(title).strong().size(13.0));
                ui.label(RichText::new(body).size(12.0).color(pal::DIM));
                ui.add_space(6.0);
            }
            ui.separator();
            ui.label(
                RichText::new("1-9 pick tower · Esc cancel · Space pause · F speed · Enter send wave · U upgrade · S sell · Shift+click keeps building · arrows or WASD scroll · wheel zooms")
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
        // Warcraft III's own convention: damage in red with an exclamation
        // mark, a critical in gold and larger, gold picked up in gold with a
        // plus. It is the loudest feedback the game has and it belongs on the
        // board rather than in a log.
        let (txt, col, size) = match t.kind {
            crate::game::TextKind::Damage => (
                format!("{}!", short(t.value as f64)),
                Color32::from_rgb(232, 72, 60),
                13.0,
            ),
            crate::game::TextKind::Crit => (
                format!("{}!", short(t.value as f64)),
                Color32::from_rgb(255, 214, 92),
                18.0,
            ),
            crate::game::TextKind::Gold => (format!("+{}", t.value as i64), pal::GOLD, 14.0),
        };
        // A dark outline, or a red number over a dark corridor is unreadable
        // and the same number over lit grass is a smear.
        let font = FontId::proportional(size);
        let shadow = Color32::from_black_alpha(190).gamma_multiply(a);
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            p.text(
                pos + vec2(dx, dy),
                Align2::CENTER_CENTER,
                &txt,
                font.clone(),
                shadow,
            );
        }
        p.text(
            pos,
            Align2::CENTER_CENTER,
            txt,
            font,
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
            RichText::new(format!(
                "{} · {}/{}",
                tw.full_name(),
                tw.level(),
                tw.ladder_len()
            ))
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
