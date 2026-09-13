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
use crate::game::{
    BOSS_HP_MULT, BOSS_MENDER_PER_SEC, BOSS_REWARD_MULT, Cue, Doctrine, Game, Phase, ToastTone,
};
use crate::math::{Camera, v3};

static TOWER_ICONS: &[u8] = include_bytes!("../assets/tower_icons.bin");

/// Desktop keeps resources in a slim RTS strip rather than a dashboard header.
// The desktop live-preview is two real text rows plus a timer/progress line.
// Forty-two outer pixels left only 34--38 pixels after the HUD inset, which
// clipped its second row against the lower frame at ordinary 1440px play.
pub const TOP_H: f32 = 50.0;
/// The bottom command console is tall enough for a 4x3 grid of real 50px
/// command targets, but no taller: the whole fixed board remains the focus.
pub const COMMAND_H: f32 = 208.0;
/// The desktop command dock is deliberately narrow: it leaves the battle map
/// visible at its complete, fixed overview instead of turning the player into
/// a camera operator.
pub const COMMAND_DOCK_W: f32 = 304.0;
/// Height used by the dock's instruments before its containing panel margins.
///
/// The desktop stage can be only 650px tall, so this is intentionally a short
/// decision rail rather than a second dashboard: next threat, the current
/// tower (or placement instruction), then twelve readable command targets.
pub const COMMAND_DOCK_CONTENT_H: f32 = 552.0;
/// A dock must have room for all of its controls. Short desktop windows fall
/// back to the compact bottom bar instead of clipping the lower command cards.
/// A 650px-tall desktop still has room for the complete fixed stage, including
/// its 52px HUD and breathing room. Below this, retain the horizontal compact
/// HUD instead of making rail controls too small to use.
pub const MIN_DESKTOP_DOCK_H: f32 = 650.0;

/// Below this width the HUD switches to a compact layout: shorter bars, smaller
/// cards, no minimap. The board must be at least as tall as the *complete*
/// rail, including its 8px top and bottom inset. The old 848px breakpoint left
/// a 520px board beside 548px of dock content: the last build row was clipped
/// exactly when a small desktop should have been the most trustworthy.
pub const MIN_DESKTOP_BOARD: f32 = COMMAND_DOCK_CONTENT_H + 16.0;
/// 304px rail + 564px complete board + 24px stage breathing room.
pub const COMPACT_WIDTH: f32 = COMMAND_DOCK_W + MIN_DESKTOP_BOARD + 24.0;
/// Narrower still and the selection panel goes too, leaving build + board.
pub const TINY_WIDTH: f32 = 720.0;

pub fn compact_for(width: f32) -> bool {
    width < COMPACT_WIDTH
}
/// Decide responsive layout from both dimensions. A width-only breakpoint left
/// a narrow vertical command rail with its final row off-screen on laptop-ish
/// browser windows.
pub fn compact_for_view(width: f32, height: f32) -> bool {
    compact_for(width) || height < MIN_DESKTOP_DOCK_H
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
pub const BAR_H: f32 = 192.0;
pub const CARD_W: f32 = 86.0;
pub const CARD_H: f32 = 100.0;

// ---------------------------------------------------------------- palette

/// A modernised Warcraft console: deep forest slate under a restrained gold rule.
///
/// The HUD used to be a flat blue-grey dashboard, which is a perfectly good
/// The information architecture remains recognisably RTS, but the large brown
/// blocks are cooled and darkened so the battlefield owns the colour and the
/// interface reads as one quiet frame around it.
pub mod pal {
    use egui::Color32;
    /// The console body.
    pub const PANEL: Color32 = Color32::from_rgb(31, 40, 37);
    /// Stone inset: minimap wells, command slots, the leaderboard ground.
    pub const PANEL_DEEP: Color32 = Color32::from_rgb(13, 18, 18);
    /// A raised slot on the console.
    pub const CARD: Color32 = Color32::from_rgb(43, 55, 50);
    pub const CARD_HOVER: Color32 = Color32::from_rgb(59, 78, 68);
    /// The dark line that separates one carved piece from the next.
    pub const LINE: Color32 = Color32::from_rgb(8, 13, 13);
    /// The gold rule that runs along every edge of the console.
    pub const GOLD_LINE: Color32 = Color32::from_rgb(156, 126, 61);
    /// The bright top edge of a bevel, which is what makes wood look carved
    /// rather than painted.
    pub const BEVEL: Color32 = Color32::from_rgb(84, 108, 94);
    pub const INK: Color32 = Color32::from_rgb(232, 239, 232);
    pub const DIM: Color32 = Color32::from_rgb(157, 176, 162);
    pub const ACC: Color32 = Color32::from_rgb(87, 194, 221);
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
    p.rect_filled(
        r.translate(vec2(0.0, 2.0)),
        cr,
        Color32::from_rgba_unmultiplied(0, 0, 0, 110),
    );
    p.rect_filled(r, cr, fill);
    if let Some(tex) = material(ui.ctx(), Mat::Stone) {
        let tiles = vec2((r.width() / 150.0).max(0.5), (r.height() / 150.0).max(0.5));
        p.image(
            tex.id(),
            r.shrink(1.0),
            Rect::from_min_size(pos2(0.0, 0.0), tiles),
            Color32::from_rgba_unmultiplied(78, 92, 84, 38),
        );
    }
    // Lit edge along the top and left, shadow along the bottom and right.
    p.line_segment(
        [
            r.left_bottom() + vec2(1.0, -1.0),
            r.left_top() + vec2(1.0, 1.0),
        ],
        Stroke::new(1.0, pal::BEVEL),
    );
    p.line_segment(
        [
            r.left_top() + vec2(1.0, 1.0),
            r.right_top() + vec2(-1.0, 1.0),
        ],
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
        Stroke::new(1.0, if gold { pal::GOLD_LINE } else { pal::LINE }),
        StrokeKind::Inside,
    );
    p.rect_stroke(
        r.shrink(3.0),
        CornerRadius::same(2),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(178, 202, 181, 24)),
        StrokeKind::Inside,
    );
}

/// A surface the HUD is made of, and its layer in `assets/textures.bin`.
///
/// The order is the blob's, set by `LAYERS` in `tools/bake_textures.py`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mat {
    Wood = 2,
    Stone = 3,
}

/// One of the baked materials, as an egui texture.
///
/// Registered once per material and kept for the life of the context. Returns
/// `None` if the blob is missing or too short, and every caller then paints the
/// flat colour it was painting before textures existed - a bad asset costs the
/// grain, not the HUD.
pub fn material(ctx: &egui::Context, which: Mat) -> Option<egui::TextureHandle> {
    let layer = which as usize;
    let cache_id = Id::new(("hud-material", layer));
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(cache_id)) {
        return Some(texture);
    }

    let blob = crate::gfx::GROUND_BLOB;
    if blob.len() < 16 {
        return None;
    }
    let at = |o: usize| u32::from_le_bytes([blob[o], blob[o + 1], blob[o + 2], blob[o + 3]]);
    if at(0) != 0x5845_5447 || at(4) != 2 {
        return None;
    }
    let size = at(8) as usize;
    let layers = at(12) as usize;
    if size == 0 || layer >= layers {
        return None;
    }
    let stride = size * size * 4;
    let start = 16 + layer * stride;
    if start + stride > blob.len() {
        return None;
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([size, size], &blob[start..start + stride]);
    let handle = ctx.load_texture(
        format!("mat{layer}"),
        img,
        egui::TextureOptions {
            // Repeating, because the console is tiled rather than stretched.
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..egui::TextureOptions::LINEAR
        },
    );
    ctx.data_mut(|data| data.insert_temp(cache_id, handle.clone()));
    Some(handle)
}

/// The 24x4 atlas is rendered from the exact staged 3D assemblies used on the
/// battlefield. A malformed optional asset falls back to the old colour swatch
/// rather than taking the interface down with it.
fn tower_icons(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let cache_id = Id::new("tower-icon-atlas");
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(cache_id)) {
        return Some(texture);
    }
    if TOWER_ICONS.len() < 16 {
        return None;
    }
    let at = |o: usize| {
        u32::from_le_bytes([
            TOWER_ICONS[o],
            TOWER_ICONS[o + 1],
            TOWER_ICONS[o + 2],
            TOWER_ICONS[o + 3],
        ])
    };
    let (magic, version, width, height) = (at(0), at(4), at(8) as usize, at(12) as usize);
    let need = 16usize.saturating_add(width.saturating_mul(height).saturating_mul(3));
    if magic != 0x4149_5447 || version != 1 || width == 0 || height == 0 || need > TOWER_ICONS.len()
    {
        return None;
    }
    let image = egui::ColorImage::from_rgb([width, height], &TOWER_ICONS[16..need]);
    let texture = ctx.load_texture("tower-icons", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(cache_id, texture.clone()));
    Some(texture)
}

fn tower_icon_uv(def: &TowerLevel) -> Rect {
    let col = def.family.icon_slot() as f32;
    let row = tower_visual_stage(def) as f32;
    // Stay inside the cell so filtered sampling never borrows a neighbour.
    let inset_x = 0.75 / 24.0 / 64.0;
    let inset_y = 0.75 / 4.0 / 64.0;
    Rect::from_min_max(
        pos2(col / 24.0 + inset_x, row / 4.0 + inset_y),
        pos2((col + 1.0) / 24.0 - inset_x, (row + 1.0) / 4.0 - inset_y),
    )
}

/// The console itself: the wooden ground the strip and the command bar sit on.
///
/// egui fills a panel with one flat colour, which is what made the HUD read as a
/// dashboard. This paints the grain over it - a few darker bands, a gold rule
/// along the inside edge - so the console reads as a carved object.
pub fn console(ui: &Ui, r: Rect) {
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::ZERO, pal::PANEL);
    if let Some(tex) = material(ui.ctx(), Mat::Wood) {
        // Tiled across the panel at the texture's own scale rather than
        // stretched to fit: a plank stretched across a fifteen-hundred point
        // console is a smear, and the grain is the entire point.
        let tiles = vec2(r.width() / 220.0, r.height() / 220.0);
        p.image(
            tex.id(),
            r,
            Rect::from_min_size(pos2(0.0, 0.0), tiles),
            // A low-alpha green-grey tint keeps the grain tactile without
            // turning the entire information surface into a brown slab.
            Color32::from_rgba_unmultiplied(96, 118, 104, 72),
        );
    }
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
        Stroke::new(3.0, pal::GOLD_LINE),
    );
    let inward = if edge == r.top() { 5.0 } else { -5.0 };
    p.line_segment(
        [
            pos2(r.left(), edge + inward),
            pos2(r.right(), edge + inward),
        ],
        Stroke::new(1.0, pal::BEVEL),
    );
    // Small metal studs turn the long strip into one constructed console and
    // also break up a wide desktop without adding another information layer.
    let studs = (r.width() / 180.0).floor() as usize;
    for i in 0..=studs {
        let x = r.left() + (i as f32 + 0.5) * r.width() / (studs + 1) as f32;
        p.circle_filled(pos2(x, edge + inward * 0.48), 2.2, pal::LINE);
        p.circle_filled(
            pos2(x - 0.5, edge + inward * 0.48 - 0.5),
            1.1,
            pal::GOLD_LINE,
        );
    }
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
    pub show_threat_intel: bool,
    pub threat_intel_rect: Option<Rect>,
    pub sound_enabled: bool,
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
    /// Raised by the visible reset-view command; the app owns camera state so
    /// this UI module never mutates a simulation or camera directly.
    pub reset_view: bool,
    /// Compact top-bar overflow alternates between tempo and the essential
    /// touch actions that do not fit beside the three resource chips.
    pub compact_overflow: bool,
    /// Live bounds of the visible reset-view button. The browser interaction
    /// check clicks this actual control after panning instead of assuming that
    /// a synthetic keyboard event reached egui on every mobile Chromium build.
    pub view_rect: Rect,
    /// Live bounds of the player-visible tempo button.  The rapid-only ladder
    /// is a real touch control, not a keyboard-only escape hatch: compact
    /// layouts keep this rect populated so browser QA can press it normally.
    pub speed_rect: Rect,
    /// Semantic bounds of the top command strip. This is browser-QA
    /// provenance for real compact touch controls, not an alternate input
    /// path; the smoke still clicks these coordinates normally.
    pub command_rects: Vec<(&'static str, Rect)>,
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
            show_threat_intel: false,
            threat_intel_rect: None,
            sound_enabled: true,
            build_tier: 1,
            hotkeys: Vec::new(),
            card_rects: Vec::new(),
            palette_rect: Rect::NOTHING,
            compact: false,
            // Desktop browsers open with real-time shadows and native-looking
            // resolution. `App::ui` still moves compact or very-high-DPI
            // screens to Performance on their first frame, while auto-quality
            // can promote a sustained 60 fps desktop to the full HDR pass.
            quality: crate::gfx::Quality::Balanced,
            quality_dirty: false,
            palette_page: 0,
            slow_frames: 0,
            fast_frames: 0,
            auto_quality: true,
            quality_ceiling: crate::gfx::Quality::Ultra,
            online: false,
            want_menu: false,
            reset_view: false,
            compact_overflow: false,
            view_rect: Rect::NOTHING,
            speed_rect: Rect::NOTHING,
            command_rects: Vec::new(),
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
    ust.view_rect = Rect::NOTHING;
    ust.speed_rect = Rect::NOTHING;
    ust.command_rects.clear();
    console(ui, full.expand(8.0));
    let h = full.height();
    let pad = 6.0;

    // ---- right-hand controls.
    #[derive(Clone, Copy, PartialEq)]
    enum Cmd {
        Send,
        Cancel,
        Pause,
        View,
        Speed,
        Sound,
        Quality,
        Menu,
        Help,
        More,
    }
    let opening_ready = g.prep;
    let send_bonus = if g.is_campaign() || opening_ready {
        0
    } else {
        (g.wave_timer * EARLY_BONUS_PER_SEC) as i32
    };
    let deploying_stream = g.wave > 0 && g.spawn_left > 0;
    let send_label = if g.is_campaign() {
        let encounter = g.campaign_encounter().unwrap_or(1);
        if deploying_stream {
            if compact {
                format!("Packet {}", g.spawn_left)
            } else {
                format!("Deploying packet  {} left", g.spawn_left)
            }
        } else if opening_ready {
            if compact { "Start".to_owned() } else { "Start Campaign".to_owned() }
        } else if g.phase == Phase::Build {
            if compact { "Start now".to_owned() } else { "Start encounter now".to_owned() }
        } else if g.phase == Phase::Combat {
            if g.campaign.as_ref().is_some_and(|state| state.can_rush()) {
                if compact { "Rush".to_owned() } else { "Rush recovery tail".to_owned() }
            } else if compact {
                "Auto".to_owned()
            } else {
                "Auto-streaming".to_owned()
            }
        } else if compact {
            format!("Start {encounter}")
        } else {
            format!("Start encounter {encounter}")
        }
    } else if deploying_stream {
        if compact {
            format!("Deploy {}", g.spawn_left)
        } else {
            format!("Deploying  {} left", g.spawn_left)
        }
    } else if opening_ready {
        if compact {
            "Start".to_owned()
        } else {
            "Start wave".to_owned()
        }
    } else if compact {
        format!("{} +{send_bonus}", if g.wave > 0 { "Rush" } else { "Send" })
    } else {
        format!(
            "{}  +{send_bonus}g",
            if g.wave > 0 { "Rush" } else { "Send" }
        )
    };
    let speed_label = if g.is_campaign() {
        format!("{}x", g.speed.round())
    } else {
        format!("{:.0}x", g.speed)
    };
    let quality_label = if compact {
        ust.quality.short()
    } else {
        ust.quality.label()
    };
    let sound_label = if ust.sound_enabled {
        "SFX ON"
    } else {
        "SFX OFF"
    };
    // The pause icon is painted, not typed. egui bundles a Latin font and an
    // emoji font; U+25B6 and U+2759 are in neither, so this button rendered as
    // two empty tofu boxes. Two spaces reserve the width and the glyph is drawn
    // below - which also means it always matches the button's ink colour.
    let pause_label = "  ";

    let mut cmds: Vec<(Cmd, &str)> = Vec::with_capacity(7);
    if g.wave < g.last_wave()
        && !matches!(g.phase, Phase::Defeat | Phase::Victory)
        && !g.pending_doctrine
    {
        cmds.push((Cmd::Send, send_label.as_str()));
    }
    if compact && g.build_choice.is_some() {
        cmds.push((Cmd::Cancel, "Cancel"));
    }
    cmds.push((Cmd::Pause, pause_label));
    cmds.push((Cmd::View, "VIEW"));
    // Campaign has a rapid-only 10x/25x/50x/100x ladder. It never cycles
    // through the old slow values just to reach the requested max mode.
    cmds.push((Cmd::Speed, speed_label.as_str()));
    cmds.push((Cmd::Sound, sound_label));
    cmds.push((Cmd::Quality, quality_label));
    cmds.push((Cmd::Menu, "Menu"));
    cmds.push((Cmd::Help, "?"));
    if compact {
        // Keep the touch bar intentionally small. The overflow is a toggle,
        // not a hidden keyboard shortcut: its second state contains Pause,
        // View reset, and Menu as visible buttons.
        cmds.retain(|(cmd, _)| {
            if ust.compact_overflow {
                // The overflow temporarily gives its entire strip to safety
                // controls. Hiding the resource chips for this one tap means
                // Pause, View, Menu and placement Cancel all remain real touch
                // targets at 390px rather than being silently trimmed away.
                matches!(cmd, Cmd::Pause | Cmd::View | Cmd::Menu | Cmd::Cancel)
            } else {
                matches!(cmd, Cmd::Send | Cmd::Cancel | Cmd::Speed)
            }
        });
        cmds.push((Cmd::More, "···"));
    }

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
    let wave = if g.is_campaign() {
        g.campaign.as_ref().map_or_else(
            || "C1 1/600".to_owned(),
            |state| {
                let id = crate::game::campaign::encounter_id(state.encounter);
                // The threat card carries the verbose encounter title. This
                // small always-visible resource chip needs to read at a glance
                // without its 600-run denominator crowding the label above.
                format!("C{} {}/600", id.chapter, state.encounter)
            },
        )
    } else if g.endless {
        format!("{}", g.wave)
    } else {
        format!("{}/{}", g.wave, N_WAVES)
    };
    // Not lives - there are none. The number that decides the run is how many
    // monsters are still going round, against what the ring will hold.
    let circling = if g.is_campaign() {
        format!(
            "{}/{}",
            g.campaign_pressure().max(0.0).round() as u32,
            g.campaign_pressure_capacity().max(0.0).round() as u32
        )
    } else {
        format!("{}/{}", g.creeps.len(), g.flood_limit())
    };
    let flood = g.flood();
    let value_size = if compact { 15.0 } else { 18.0 };
    let chips: [(&str, &str, Color32); 3] = [
        (
            if g.is_campaign() { "CAMPAIGN" } else { "WAVE" },
            wave.as_str(),
            if g.endless { pal::ACC } else { pal::INK },
        ),
        (
            if g.is_campaign() { "PRESSURE" } else { "CIRCLING" },
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
    let show_stats = !compact || !ust.compact_overflow;
    let stats_w: f32 = if show_stats {
        chip_ws.iter().sum::<f32>() + pad * 2.0
    } else {
        0.0
    };

    // On a phone there is not room for every secondary toggle. Retain the
    // player-safety controls before the automatic deployment label: Pause,
    // overview reset, Menu, and the requested rapid tempo ladder all remain
    // reachable without a keyboard.
    let rank = |c: Cmd| match c {
        Cmd::Quality => 0,
        Cmd::Help => 1,
        Cmd::Sound => 2,
        Cmd::Send => 3,
        Cmd::Cancel => 8,
        Cmd::Pause => 5,
        Cmd::View => 6,
        Cmd::Menu => 7,
        Cmd::Speed => 9,
        Cmd::More => 10,
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
    for (i, (label, value, col)) in chips.iter().enumerate().filter(|_| show_stats) {
        let r = Rect::from_min_size(pos2(x, full.top()), vec2(chip_ws[i], h));
        stat_chip(ui, r, label, value, *col, value_size);
        let response = ui.interact(r, Id::new(("top_stat", i)), Sense::click().union(Sense::hover()));
        if i == 0 {
            ust.command_rects.push(("intel_open_compact", r));
            ust.command_rects.push(("intel_open", r));
            if response.clicked() {
                ust.show_threat_intel = !ust.show_threat_intel;
                if ust.show_threat_intel {
                    g.sound_cues.push(Cue::Select);
                }
            }
        }
        match i {
            0 => {
                response.on_hover_ui(|ui| {
                    if let Some(state) = g.campaign.as_ref() {
                        let id = crate::game::campaign::encounter_id(state.encounter);
                        let encounter = state.current();
                        ui.label(
                            RichText::new(format!(
                                "Campaign · Chapter {} · encounter {} of 60",
                                id.chapter, id.local
                            ))
                            .strong()
                            .color(pal::GOLD),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} · {}s active · {} packet{}",
                                encounter.formation.name(),
                                encounter.duration_seconds,
                                encounter.packets.len(),
                                if encounter.packets.len() == 1 { "" } else { "s" },
                            ))
                            .size(11.0)
                            .color(pal::DIM),
                        );
                    }
                    ui.label(
                        RichText::new(g.difficulty.label())
                            .strong()
                            .color(pal::GOLD),
                    );
                    ui.label(
                        RichText::new(g.difficulty.blurb())
                            .size(11.0)
                            .color(pal::DIM),
                    );
                    if g.doctrine_picks() > 0 {
                        ui.label(
                            RichText::new(format!(
                                "Command: Arsenal {}  |  Overdrive {}  |  High Ground {}",
                                g.doctrines[0], g.doctrines[1], g.doctrines[2]
                            ))
                            .size(11.0)
                            .color(pal::GOOD),
                        );
                    }
                });
            }
            1 => {
                response.on_hover_ui(|ui| {
                    let oldest = g.creeps.iter().map(|c| c.laps).max().unwrap_or(0);
                    ui.label(RichText::new("Ring pressure").strong());
                    ui.label(
                        RichText::new(format!(
                            "{}",
                            if g.is_campaign() {
                                format!(
                                    "Weighted bodies breach {:.0} pressure after {:.1}s grace. {} bodies are currently visible.",
                                    g.campaign_pressure_capacity(),
                                    g.campaign_pressure_grace,
                                    g.creeps.len(),
                                )
                            } else {
                                format!(
                                    "The run ends above {} circling enemies. Fullest so far: {}.",
                                    g.flood_limit(), g.stats.peak_circling
                                )
                            }
                        ))
                        .size(11.0)
                        .color(pal::DIM),
                    );
                    if oldest > 0 {
                        ui.label(
                            RichText::new(format!(
                                "Oldest survivor: {oldest} lap{}. Hard-mode survivors accelerate each lap.",
                                if oldest == 1 { "" } else { "s" }
                            ))
                            .size(11.0)
                            .color(pal::BAD),
                        );
                    }
                });
            }
            _ => {
                response.on_hover_ui(|ui| {
                    ui.label(RichText::new("Economy").strong());
                    if g.is_campaign() {
                        ui.label(
                            RichText::new(
                                "Each authored Campaign budget pays 40% as packets deploy; the remaining 60% arrives through cleared bodies.",
                            )
                            .size(11.0)
                            .color(pal::DIM),
                        );
                    } else {
                        ui.label(
                            RichText::new(
                                "Kills and wave stipends fund towers. Rush early for instant gold, or wait and clear the ring for a Clean Sweep bonus.",
                            )
                            .size(11.0)
                            .color(pal::DIM),
                        );
                        ui.label(
                            RichText::new(format!(
                                "This run: {} rush gold  |  {} clean sweeps",
                                gold_str(g.stats.rush_gold as i64),
                                g.stats.clean_sweeps
                            ))
                            .size(11.0)
                            .color(pal::GOOD),
                        );
                    }
                });
            }
        }
        ust.stat_rects.push(r);
        x += chip_ws[i] + pad;
    }
    if show_preview {
        let r = Rect::from_min_size(
            pos2(x, full.top() + 2.0),
            vec2(preview_w, (h - 4.0).min(42.0)),
        );
        wave_preview(g, ui, ust, r);
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
        let command_name = match cmd {
            Cmd::Send => "send",
            Cmd::Cancel => "cancel",
            Cmd::Pause => "pause",
            Cmd::View => "view",
            Cmd::Speed => "speed",
            Cmd::Sound => "sound",
            Cmd::Quality => "quality",
            Cmd::Menu => "menu",
            Cmd::Help => "help",
            Cmd::More => "more",
        };
        ust.command_rects.push((command_name, r));
        let fill = match cmd {
            Cmd::Send if deploying_stream => pal::PANEL_DEEP,
            Cmd::Send if g.is_campaign() && g.phase == Phase::Combat => {
                Color32::from_rgb(82, 68, 31)
            }
            Cmd::Send if g.is_campaign() => Color32::from_rgb(38, 106, 68),
            Cmd::Send if g.creeps.len() + g.spawn_left as usize > g.flood_limit() * 3 / 4 => {
                Color32::from_rgb(126, 49, 39)
            }
            Cmd::Send if g.wave > 0 => Color32::from_rgb(115, 77, 29),
            Cmd::Send => Color32::from_rgb(38, 106, 68),
            Cmd::Sound if !ust.sound_enabled => Color32::from_rgb(62, 40, 43),
            Cmd::Menu => pal::CARD_HOVER,
            Cmd::Cancel => pal::BAD.gamma_multiply(0.45),
            _ => pal::CARD,
        };
        let mut label = RichText::new(*text).size(13.0).color(pal::INK);
        if matches!(cmd, Cmd::Send | Cmd::Menu | Cmd::Cancel) {
            label = label.strong();
        }
        let resp = ui.put(r, egui::Button::new(label).fill(fill).corner_radius(6.0));
        if *cmd == Cmd::View {
            ust.view_rect = r;
        }
        if *cmd == Cmd::Speed {
            ust.speed_rect = r;
        }
        if *cmd == Cmd::Pause {
            paint_transport(ui, r, g.paused);
        }
        let resp = match cmd {
            Cmd::Send => resp.on_hover_text(if g.is_campaign() && deploying_stream {
                "This authored packet is entering now. Rush unlocks only after the final packet is fully on the ring."
            } else if g.is_campaign() && g.phase == Phase::Combat {
                if g.campaign.as_ref().is_some_and(|state| state.can_rush()) {
                    "Rush the legal recovery tail. It cannot remove packets, objectives, or living enemies."
                } else {
                    "The formation is active. Ordinary survivors stay on the ring while the next authored formation auto-streams at its schedule boundary."
                }
            } else if g.is_campaign() {
                "Campaign auto-deploys after its short build beat. Press Enter to start this authored encounter immediately."
            } else if deploying_stream {
                "The current stream is still entering the ring. Rush unlocks when every authored enemy has deployed."
            } else if g.prep {
                "Start Wave 1 (Enter) when you are ready. Opening setup has no Rush bonus."
            } else if g.wave > 0 {
                "Rush now (Enter): begin the next stream while survivors are still circling. The gold pays for that overlap."
            } else {
                "Call the next wave early (Enter). The bonus is the setup time you give up."
            }),
            Cmd::Cancel => resp.on_hover_text("Cancel placement"),
            Cmd::Pause => resp.on_hover_text("Pause (Space)"),
            Cmd::View => resp.on_hover_text("Reset overview (R). Wheel/pinch to zoom; middle-drag or touch-drag while zoomed to pan."),
            Cmd::Speed => resp.on_hover_text(
                "Tempo (F): 10x / 25x / 50x / 100x. Fast play advances the full simulation; it does not skip encounters or rewards."
            ),
            Cmd::Sound => resp.on_hover_text("Toggle sound effects."),
            Cmd::Quality => {
                resp.on_hover_text("Graphics quality (B). Lower it if the frame rate drags.")
            }
            Cmd::Menu => resp.on_hover_text("Back to the menu. This ends the run."),
            Cmd::Help => resp.on_hover_text("How to play (H)"),
            Cmd::More => resp.on_hover_text("More touch controls"),
        };
        if resp.clicked() {
            match cmd {
                Cmd::Send => g.send_wave(),
                Cmd::Cancel => { g.build_choice = None; g.selected = None; },
                Cmd::Pause => g.paused = !g.paused,
                Cmd::View => ust.reset_view = true,
                Cmd::Speed => g.cycle_speed(),
                Cmd::Sound => {
                    ust.sound_enabled = !ust.sound_enabled;
                    if ust.sound_enabled {
                        g.sound_cues.push(Cue::Select);
                    }
                }
                Cmd::Quality => {
                    ust.quality = ust.quality.lower().unwrap_or(crate::gfx::Quality::Ultra);
                    ust.quality_dirty = true;
                    ust.auto_quality = false;
                }
                Cmd::Menu => ust.want_menu = true,
                Cmd::Help => ust.show_help = true,
                Cmd::More => ust.compact_overflow = !ust.compact_overflow,
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
        // Keep the large resource value below the small-caps label. The
        // original baseline was only a pixel clear at the 42px RTS strip, so
        // "CAMPAIGN" and "C1 2/600" looked like one malformed word.
        pos2(r.center().x, r.bottom() - value_size * 0.62),
        Align2::CENTER_CENTER,
        value,
        FontId::monospace(value_size),
        color,
    );
}

fn wave_preview(g: &mut Game, ui: &mut Ui, ust: &mut UiState, rect: Rect) {
    let w = g.next_wave_def();
    let upcoming = if g.is_campaign() {
        g.upcoming_wave_number()
    } else if g.endless {
        g.wave + 1
    } else {
        (g.wave + 1).min(N_WAVES)
    };
    let elite_stride = g.difficulty.elite_stride(upcoming);
    let resp = ui.interact(rect, Id::new("wave_preview"), Sense::click().union(Sense::hover()));
    ust.command_rects.push(("intel_open", resp.rect));
    if resp.clicked() {
        ust.show_threat_intel = !ust.show_threat_intel;
        if ust.show_threat_intel {
            g.sound_cues.push(Cue::Select);
        }
    }
    // The map tags its own waves - Air, Immune, Hero, Boss - and those four
    // words are the whole of what a player has to react to.
    let flagged = !w.tag.is_empty() || elite_stride.is_some();
    carved(ui, rect, pal::PANEL_DEEP, flagged);
    let p = ui.painter();
    if flagged {
        p.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(
                2.0,
                if w.tag.is_empty() {
                    pal::GOLD
                } else {
                    c32(w.armour_type.color(), 0.95)
                },
            ),
            StrokeKind::Inside,
        );
    }

    let title = if g.is_campaign() {
        let id = crate::game::campaign::encounter_id(upcoming as u16);
        match g.phase {
            // This is a thin RTS strip, not a dashboard heading. The longer
            // wording overlapped the live timer at ordinary desktop widths.
            // The live strip shares a 40px high card with the timer. Keep the
            // complete encounter number but remove redundant words/spacing so
            // its title has a real rect at every desktop HUD width.
            Phase::Combat => format!("LIVE C{} E{}", id.chapter, upcoming),
            Phase::Victory => "CAMPAIGN COMPLETE".to_string(),
            Phase::Defeat => "OVERRUN".to_string(),
            _ => format!("NEXT C{} E{}", id.chapter, upcoming),
        }
    } else { match g.phase {
        // This card always describes `next_wave_def`, not the enemies already
        // on the ring. Calling it the current wave made the armour warning and
        // countdown contradict the main wave counter.
        Phase::Combat if g.endless => format!("NEXT ENDLESS WAVE {upcoming}"),
        Phase::Combat => format!("NEXT WAVE {upcoming} OF {N_WAVES}"),
        Phase::Victory => "ALL WAVES CLEARED".to_string(),
        Phase::Defeat => "OVERRUN".to_string(),
        _ => format!("OPENING WAVE {upcoming}"),
    }};
    // Reserve the top-right status before painting the title. At a wide
    // desktop strip the old fixed "ACTIVE C1 / ENCOUNTER 600" string ran
    // underneath the countdown, and at smaller widths it became an unreadable
    // pile rather than a thin RTS information strip.
    let top_status = if g.is_campaign() && g.phase == Phase::Build {
        format!("AUTO {:.0}s", g.wave_timer.ceil().max(0.0))
    } else if g.prep {
        "READY".to_owned()
    } else if g.wave < g.last_wave() {
        format!("{:.0}s", g.wave_timer.max(0.0))
    } else if g.phase == Phase::Combat {
        format!("{} left", g.creeps.len() as u32 + g.spawn_left)
    } else {
        String::new()
    };
    // This preview only owns a 36--40px strip. A 9px title plus a 12px
    // formation line painted into the same baseline clipped the lower line at
    // ordinary 1440px desktop height; reserve two explicit compact rows.
    let top_font = FontId::monospace(8.0);
    let status_w = ui
        .painter()
        .layout_no_wrap(top_status.clone(), FontId::monospace(10.0), pal::INK)
        .rect
        .width();
    let title_w = (rect.width() - 22.0 - status_w).max(36.0);
    p.text(
        rect.left_top() + vec2(8.0, 3.0),
        Align2::LEFT_TOP,
        elide(ui, &title, &top_font, title_w),
        top_font,
        pal::DIM,
    );
    if !top_status.is_empty() {
        p.text(
            rect.right_top() + vec2(-8.0, 3.0),
            Align2::RIGHT_TOP,
            top_status,
            FontId::monospace(10.0),
            if g.is_campaign() && g.phase == Phase::Build {
                pal::GOOD
            } else if g.prep {
                pal::GOOD
            } else if g.wave < g.last_wave() {
                pal::ACC
            } else {
                pal::INK
            },
        );
    }

    let formation = if w.tag == "Boss" || w.tag == "Commander" && w.count > 1 {
        format!("{} commander + {} escorts", w.name, w.count - 1)
    } else {
        format!("{} x{}", w.name, w.count)
    };
    let line = format!(
        "{}  ·  {} armour {}{}",
        formation,
        w.armour,
        w.armour_type.name(),
        elite_stride
            .map(|stride| format!("  ·  Vanguard 1/{stride}"))
            .unwrap_or_default()
    );
    let mut tx = rect.left() + 8.0;
    if !w.tag.is_empty() {
        // A filled badge, not a word in a sentence. Missing an Air wave with no
        // anti-air, or an Immune wave with no Chaos, decides the run.
        let tint = if w.flying {
            AIR_TINT
        } else {
            w.armour_type.color()
        };
        let label = w.tag.to_ascii_uppercase();
        let bw = 12.0 + label.len() as f32 * 7.0;
        let badge = Rect::from_min_size(pos2(tx, rect.top() + 15.0), vec2(bw, 12.0));
        p.rect_filled(badge, CornerRadius::same(4), c32(tint, 1.0));
        p.text(
            badge.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(8.0),
            Color32::from_rgb(10, 14, 24),
        );
        tx += bw + 6.0;
    }
    p.text(
        pos2(tx, rect.top() + 15.0),
        Align2::LEFT_TOP,
        elide(
            ui,
            &line,
            &FontId::proportional(10.0),
            rect.right() - tx - 8.0,
        ),
        FontId::proportional(10.0),
        c32(w.armour_type.color(), 1.0),
    );

    /* The old third instruction row was intentionally removed: this strip
       owns only title/status, threat/formation, and the separate progress bar.
        p.text(
            rect.left_bottom() + vec2(8.0, -5.0),
            Align2::LEFT_BOTTOM,
            "AUTO DEPLOY · ENTER NOW",
            FontId::monospace(8.5),
            pal::GOOD,
        );
    // removed legacy send instruction row
        p.text(
            rect.left_bottom() + vec2(8.0, -5.0),
            Align2::LEFT_BOTTOM,
            "BUILD, THEN SEND",
            FontId::monospace(8.5),
            pal::GOOD,
        );
    */
    if !g.pending_doctrine && !g.prep && g.wave < g.last_wave() {
        let frac = (g.wave_timer / w.lead_in.max(1.0)).clamp(0.0, 1.0);
        let bar = Rect::from_min_size(
            rect.left_bottom() + vec2(8.0, -6.0),
            vec2((rect.width() - 16.0).max(1.0), 3.0),
        );
        p.rect_filled(bar, CornerRadius::same(2), pal::LINE);
        p.rect_filled(
            Rect::from_min_size(bar.min, vec2(bar.width() * frac, 3.0)),
            CornerRadius::same(2),
            pal::ACC,
        );
    }

    resp.on_hover_ui(|ui| render_encounter_threat_tooltip(ui, g, upcoming, elite_stride));
}

fn render_encounter_threat_tooltip(ui: &mut Ui, g: &Game, upcoming: u32, elite_stride: Option<u32>) {
    let w = g.next_wave_def();
    if g.is_campaign() {
        let enc = crate::game::campaign::resolved_encounter(upcoming as u16);
        let id = enc.id;
        ui.label(
            RichText::new(format!("Campaign C{} E{:02} · {}", id.chapter, id.local, enc.title))
                .strong()
                .size(13.0)
                .color(pal::GOLD),
        );
        ui.label(
            RichText::new(format!(
                "Formation: {} · {} bodies across {} packet{} · {}s active",
                enc.formation.name(),
                w.count,
                enc.packets.len(),
                if enc.packets.len() == 1 { "" } else { "s" },
                enc.duration_seconds
            ))
            .size(10.5)
            .color(pal::DIM),
        );

        if let Some(cmd) = enc.commander {
            let (mech, adv) = crate::game::campaign::commander_threat_advice(cmd);
            let minimum_hp = crate::game::campaign_commander_hp_floor(g.difficulty, upcoming as u16);
            ui.separator();
            ui.label(RichText::new(format!("COMMANDER: {}", cmd.name())).strong().color(pal::BAD));
            ui.label(RichText::new(mech).size(11.0).color(pal::INK));
            ui.label(RichText::new(format!("Counter: {adv}")).size(11.0).color(pal::GOOD));
            if minimum_hp > 0.0 {
                ui.label(
                    RichText::new(format!(
                        "Commander minimum: {} health · target Strongest and commit focused damage.",
                        short(minimum_hp as f64),
                    ))
                    .size(10.5)
                    .color(pal::GOLD),
                );
            }
        }

        let traits = crate::game::campaign::encounter_threat_traits(&enc);
        if !traits.is_empty() {
            ui.separator();
            ui.label(RichText::new("PACKET THREATS & TACTICAL ADVICE:").strong().size(11.0).color(pal::GOLD_LINE));
            for t in traits {
                let (desc, counter) = crate::game::campaign::trait_threat_advice(t);
                ui.label(RichText::new(format!("• {}: {}", t.name(), desc)).size(11.0).color(pal::INK));
                ui.label(RichText::new(format!("   Advice: {counter}")).size(10.5).color(pal::GOOD));
            }
        }
        ui.separator();
        ui.label(
            RichText::new("REPRESENTATIVE ESCORT HEALTH")
                .size(9.0)
                .strong()
                .color(pal::DIM),
        );
        ui.label(
            RichText::new(format!(
                "{} health · {} armour ({:.0}% taken)",
                short(w.hp as f64),
                w.armour,
                armour_mult(w.armour) * 100.0
            ))
            .size(10.5)
            .color(pal::DIM),
        );
    } else {
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
        if w.tag == "Boss" || w.tag == "Commander" {
            ui.label(
                RichText::new(format!(
                    "Commander: {:.0}x health, 75% control resistance, and {:.1}%/sec repair for nearby escorts.",
                    BOSS_HP_MULT,
                    BOSS_MENDER_PER_SEC * 100.0
                ))
                .size(11.0)
                .color(pal::GOLD),
            );
            ui.label(RichText::new("Focus Strongest to break the commander; Corruption suppresses escort repair.").size(11.0).color(pal::GOOD));
        }
        if let Some(stride) = elite_stride {
            ui.label(
                RichText::new(format!(
                    "Every {stride}th enemy is a Vanguard: tougher, faster, and 50% resistant to control."
                ))
                .size(11.0)
                .color(pal::GOLD),
            );
        }
        let pay = g.bounty_for_wave(g.wave + 1);
        let payout = if w.tag == "Boss" {
            format!(
                "Pays {} per escort; {} for the commander",
                gold_str(pay as i64),
                gold_str(pay.saturating_mul(BOSS_REWARD_MULT) as i64)
            )
        } else {
            format!("Pays {} gold a kill", gold_str(pay as i64))
        };
        ui.label(RichText::new(payout).size(11.0).color(pal::GOLD));
    }
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
                gold_str(g.bounty_for_wave(g.wave.max(1)) as i64),
                pal::GOLD,
            ));
            rows.push((
                "Next wave pays",
                format!("+{}", gold_str(g.stipend_for_wave(g.wave + 1) as i64)),
                pal::GOLD,
            ));
            rows.push(("", String::new(), pal::DIM));
            rows.push((
                "Circling",
                format!("{} / {}", g.creeps.len(), g.flood_limit()),
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
                RichText::new(format!(
                    "{} · overflow above {}",
                    g.difficulty.label(),
                    g.flood_limit()
                ))
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
    // A horizontal child otherwise takes only the height requested by its
    // first sibling.  On short desktop windows that collapsed the palette's
    // measured well to a 44px row even though the surrounding command panel
    // had a full compact bar available.  Reserve the bar before children lay
    // themselves out so every visible tower command remains a real touch
    // target rather than a tiny dark icon.
    ui.set_min_height(bar_h(compact));
    console(ui, ui.available_rect_before_wrap().expand(8.0));
    ui.horizontal(|ui| {
        // On a phone the board is the scarce resource: drop the minimap first,
        // then the selection panel, before ever shrinking the build palette.
        if !compact {
            // Keep the console's instruments at a deliberate maximum width on
            // ultrawide displays. The side rails become breathing room rather
            // than stretching a command slot into a banner.
            let content_w = width.min(1500.0);
            ui.add_space(((width - content_w) * 0.5).max(0.0));
            minimap(g, ui, bar_h(compact));
            ui.add_space(8.0);
            let command_w = 296.0;
            let dossier_w = (content_w - bar_h(compact) - command_w - 16.0).max(320.0);
            selection_panel(g, ui, compact, dossier_w);
            ui.add_space(8.0);
            build_palette(g, ui, ust, command_w);
        } else {
            if width > TINY_WIDTH {
                selection_panel(g, ui, compact, 268.0);
                ui.add_space(6.0);
            }
            build_palette(g, ui, ust, ui.available_width());
        }
    });
}

/// The fixed desktop command dock.
///
/// This is an original RTS-style command console: a battle briefing, selected
/// defense dossier, tactile command grid and three immediate battle actions.
/// A wide, tall rail also earns a small tactical map; the minimum rail keeps
/// every build and touch target exposed instead of clipping or scrolling.
///
/// This is content for an already-created stage rail, rather than creating a
/// panel itself.  At [`COMMAND_DOCK_W`] it consumes
/// [`COMMAND_DOCK_CONTENT_H`] points before panel margins.
pub fn command_dock(g: &mut Game, ui: &mut Ui, ust: &mut UiState) {
    const GAP: f32 = 6.0;
    const BRIEFING_H: f32 = 78.0;
    // 50px outer height leaves a real 44px touch target after the carved
    // console inset, including at the narrowest desktop rail.
    const COMMAND_H: f32 = 50.0;

    let dock = ui.available_rect_before_wrap();
    let width = dock.width().max(1.0);
    console(ui, dock);

    // A local spacing scope makes the content-height promise exact and avoids
    // a nested layout quietly inventing a scroll bar in a short desktop stage.
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = GAP;
        ui.vertical(|ui| {
            let tall_wide_console = width >= 430.0 && dock.height() >= 620.0;
            dock_briefing(g, ui, width, if tall_wide_console { 94.0 } else { BRIEFING_H });

            // The dossier carries a portrait, selected unit readout and the
            // real upgrade / target / sell actions above the command grid.
            selection_panel(g, ui, true, width);
            build_palette_dock(g, ui, ust, width);
            battle_commands(g, ui, width, COMMAND_H);

            // At the minimum desktop size the rail is intentionally complete
            // with no spare pixels. On a taller monitor, leave neither a dead
            // dashboard void nor a redundant minimap: use the extra room for
            // the next few counter-relevant waves. This makes the fixed board
            // feel like one deliberate stage at every size.
            let spare = ui.available_height();
            if spare >= 172.0 {
                let intel_h = spare.min(176.0);
                ui.add_space((spare - intel_h).max(0.0));
                let (intel, _) = ui.allocate_exact_size(vec2(width, intel_h), Sense::hover());
                wave_intel(g, ui, intel);
            }
        });
    });
}

/// A briefing in the visual language of an RTS command console. It avoids
/// copying another game's assets or layout verbatim while retaining the useful
/// hierarchy: mission state on the left and a genuine tactical overview when
/// the player's viewport can afford one.
fn dock_briefing(g: &Game, ui: &mut Ui, width: f32, h: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let show_minimap = width >= 430.0 && h >= 90.0;
    if !show_minimap {
        next_wave_card(g, ui, rect);
        return;
    }

    let mini_side = (h - 8.0).max(44.0);
    let mini = Rect::from_min_size(
        pos2(rect.right() - mini_side - 2.0, rect.top() + 2.0),
        vec2(mini_side, mini_side),
    );
    let briefing = Rect::from_min_max(rect.left_top(), pos2(mini.left() - 6.0, rect.bottom()));
    next_wave_card(g, ui, briefing);
    let mut mini_ui = ui.new_child(egui::UiBuilder::new().max_rect(mini));
    mini_ui.set_min_size(mini.size());
    minimap(g, &mut mini_ui, mini_side);
}

/// Large, always-present commands finish the rail. They are deliberately
/// redundant with keyboard and top-bar shortcuts: a player should be able to
/// read and touch the three battle actions without hunting through chrome.
fn battle_commands(g: &mut Game, ui: &mut Ui, width: f32, h: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    carved(ui, rect, pal::PANEL_DEEP, false);
    let well = rect.shrink2(vec2(4.0, 3.0));
    let gap = 4.0;
    let button_w = ((well.width() - gap * 2.0) / 3.0).max(44.0);
    let can_send = if g.is_campaign() {
        (g.phase == Phase::Build || g.campaign.as_ref().is_some_and(|state| state.can_rush()))
            && !matches!(g.phase, Phase::Defeat | Phase::Victory)
            && !g.pending_doctrine
    } else {
        g.wave < g.last_wave()
            && !matches!(g.phase, Phase::Defeat | Phase::Victory)
            && !g.pending_doctrine
    };
    let call_label = if g.is_campaign() {
        if g.prep || g.phase == Phase::Build { "START NOW" }
        else if g.campaign.as_ref().is_some_and(|state| state.can_rush()) { "RUSH" }
        else { "ON CLOCK" }
    } else if g.prep { "START" } else { "RUSH" };
    let tempo_label = if g.is_campaign() {
        format!("TEMPO {:.0}X", g.speed)
    } else {
        format!("TEMPO {:.0}X", g.speed)
    };
    let cells = [
        (call_label, if can_send { pal::GOOD } else { pal::DIM }),
        (if g.paused { "RESUME" } else { "PAUSE" }, pal::INK),
        (tempo_label.as_str(), pal::ACC),
    ];
    for (i, (label, colour)) in cells.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(well.left() + i as f32 * (button_w + gap), well.top()),
            vec2(button_w, well.height()),
        );
        let response = ui.interact(r, ui.id().with(("battle_command", i)), Sense::click());
        let fill = if response.hovered() { pal::CARD_HOVER } else { pal::CARD };
        ui.painter().rect_filled(r, CornerRadius::same(3), fill);
        ui.painter().rect_stroke(
            r,
            CornerRadius::same(3),
            Stroke::new(1.0, if i == 0 { *colour } else { pal::GOLD_LINE }),
            StrokeKind::Inside,
        );
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::monospace(10.0),
            *colour,
        );
        if response.clicked() {
            match i {
                0 if can_send => g.send_wave(),
                1 => g.paused = !g.paused,
                2 => g.cycle_speed(),
                _ => {}
            }
        }
    }
}

/// Draw the desktop dock as part of the compact game stage instead of pinning
/// a full-height panel to the far edge of an ultrawide browser. The stage is a
/// map and its controls together; unused monitor space remains a quiet
/// backdrop on both sides instead of becoming a giant accidental gutter.
pub fn command_dock_area(ctx: &Context, rect: Rect, g: &mut Game, ust: &mut UiState) {
    egui::Area::new(Id::new("shop"))
        .fixed_pos(rect.left_top())
        .default_size(rect.size())
        .movable(false)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            // An `Area` otherwise shrinks to its content. Reserve the full
            // map height so the rail reads as one deliberate game edge.
            ui.set_min_size(rect.size());
            let outer = ui.max_rect();
            ui.painter().rect_filled(outer, 0.0, pal::PANEL);
            let inner = outer.shrink2(vec2(10.0, 8.0));
            let mut content = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            command_dock(g, &mut content, ust);
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
    // Empty build tiles are deliberately absent here. Painting all thousand of
    // them turned the compact overview into graph paper; placement feedback
    // already appears on the battlefield when a tower card is armed. The
    // minimap's job is route, threats and built defenses.
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

/// The selected tower is a dossier, not another shop card. A large portrait
/// anchors recognition, the performance readout answers whether it is earning
/// its pad, and the three commands sit on one predictable bottom row.
fn selection_panel_desktop(g: &mut Game, ui: &mut Ui, w: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, BAR_H), Sense::hover());
    carved(ui, rect, pal::PANEL, true);

    let show_intel = w >= 650.0;
    let intel = if show_intel {
        Some(Rect::from_min_size(
            pos2(rect.right() - 190.0, rect.top() + 7.0),
            vec2(182.0, rect.height() - 14.0),
        ))
    } else {
        None
    };
    if let Some(r) = intel {
        wave_intel(g, ui, r);
    }
    let main = Rect::from_min_max(
        rect.left_top(),
        pos2(
            intel.map_or(rect.right(), |r| r.left() - 7.0),
            rect.bottom(),
        ),
    );
    let p = ui.painter();

    let Some(ti) = g.selected.filter(|&i| i < g.towers.len()) else {
        let opening = g.wave == 0 && g.towers.is_empty();
        p.text(
            pos2(main.left() + 14.0, main.top() + 11.0),
            Align2::LEFT_TOP,
            "COMMAND DOSSIER",
            FontId::monospace(9.0),
            pal::GOLD_LINE,
        );
        let emblem = pos2(main.left() + 68.0, main.center().y + 2.0);
        p.circle_filled(emblem, 35.0, pal::PANEL_DEEP);
        p.circle_stroke(emblem, 35.0, Stroke::new(2.0, pal::GOLD_LINE));
        p.circle_stroke(emblem, 27.0, Stroke::new(1.0, pal::BEVEL));
        p.line_segment(
            [emblem + vec2(-17.0, 9.0), emblem + vec2(0.0, -17.0)],
            Stroke::new(3.0, pal::GOOD),
        );
        p.line_segment(
            [emblem + vec2(0.0, -17.0), emblem + vec2(18.0, 10.0)],
            Stroke::new(3.0, pal::GOOD),
        );
        p.circle_filled(emblem + vec2(0.0, -17.0), 4.0, pal::GOLD);

        let tx = main.left() + 120.0;
        p.text(
            pos2(tx, main.center().y - 30.0),
            Align2::LEFT_TOP,
            if opening {
                "Choose your opening tower"
            } else {
                "Select a tower"
            },
            FontId::proportional(18.0),
            if opening { pal::ACC } else { pal::INK },
        );
        p.text(
            pos2(tx, main.center().y - 2.0),
            Align2::LEFT_TOP,
            if opening {
                "Pick a command icon, then place it on a free plot beside the circuit."
            } else {
                "Its damage, contribution, upgrades and targeting controls appear here."
            },
            FontId::proportional(12.0),
            pal::DIM,
        );
        p.text(
            pos2(tx, main.center().y + 25.0),
            Align2::LEFT_TOP,
            "Tip: inspect NEXT THREATS before committing your gold.",
            FontId::monospace(10.0),
            pal::GOOD,
        );
        if g.doctrine_picks() > 0 {
            p.text(
                pos2(tx, main.bottom() - 25.0),
                Align2::LEFT_TOP,
                format!(
                    "COMMAND  ARS {}  |  SPD {}  |  RNG {}",
                    g.doctrines[0], g.doctrines[1], g.doctrines[2]
                ),
                FontId::monospace(9.5),
                pal::GOLD,
            );
        }
        return;
    };

    let tw = g.towers[ti].clone();
    let def = tw.def();
    let col = tower_color(def);
    let choices = tw.upgrades();

    p.text(
        pos2(main.left() + 12.0, main.top() + 7.0),
        Align2::LEFT_TOP,
        "SELECTED DEFENSE",
        FontId::monospace(8.5),
        pal::GOLD_LINE,
    );
    let port = Rect::from_min_size(main.left_top() + vec2(12.0, 24.0), vec2(104.0, 104.0));
    carved(ui, port, pal::PANEL_DEEP, true);
    if let Some(texture) = tower_icons(ui.ctx()) {
        p.image(
            texture.id(),
            port.shrink(4.0),
            tower_icon_uv(def),
            Color32::WHITE,
        );
        p.rect_filled(
            Rect::from_min_max(
                pos2(port.left() + 4.0, port.bottom() - 24.0),
                pos2(port.right() - 4.0, port.bottom() - 4.0),
            ),
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(4, 8, 8, 218),
        );
    } else {
        p.circle_filled(port.center(), 31.0, c32(col, 1.0));
    }
    p.text(
        port.center_bottom() + vec2(0.0, -13.0),
        Align2::CENTER_CENTER,
        format!("LEVEL {} / {}", tw.level(), tw.ladder_len()),
        FontId::monospace(9.5),
        pal::GOLD,
    );
    tower_rank_track(
        p,
        Rect::from_min_max(
            pos2(port.left() + 9.0, port.bottom() - 31.0),
            pos2(port.right() - 9.0, port.bottom() - 27.0),
        ),
        tw.level(),
        tw.ladder_len(),
        c32(col, 1.0),
    );

    let tx = port.right() + 13.0;
    let text_right = main.right() - 12.0;
    let name_font = FontId::proportional(18.0);
    p.text(
        pos2(tx, port.top()),
        Align2::LEFT_TOP,
        elide(ui, tw.full_name(), &name_font, (text_right - tx).max(30.0)),
        name_font,
        c32(col, 1.0),
    );
    p.text(
        pos2(tx, port.top() + 25.0),
        Align2::LEFT_TOP,
        format!(
            "{}  ·  {}  ·  {}",
            def.family.name(),
            def.attack.name(),
            def.targets.label()
        ),
        FontId::proportional(11.0),
        c32(col, 0.92),
    );
    let dps = tw.dmg() * tw.rate();
    p.text(
        pos2(tx, port.top() + 48.0),
        Align2::LEFT_TOP,
        if tw.is_support() {
            format!("AURA {:.1} RANGE", def.abil.aura_range.max(tw.range()))
        } else {
            format!("{} DPS     {:.1} RANGE", short(dps as f64), tw.range())
        },
        FontId::monospace(13.0),
        pal::INK,
    );
    p.text(
        pos2(tx, port.top() + 72.0),
        Align2::LEFT_TOP,
        if tw.is_support() {
            let n = buffed_count(g, ti);
            format!("BUFFING {n} TOWER{}", if n == 1 { "" } else { "S" })
        } else {
            format!(
                "{} KILLS  ·  {} DEALT  ·  {}g EARNED",
                tw.kills,
                short(tw.damage),
                gold_str(tw.gold_earned as i64)
            )
        },
        FontId::monospace(10.5),
        pal::DIM,
    );
    if tw.buff_dmg > 0.0 || tw.buff_rate > 0.0 || tw.buff_range > 0.0 {
        p.text(
            pos2(tx, port.top() + 91.0),
            Align2::LEFT_TOP,
            format!(
                "+{:.0}% DMG  |  +{:.0}% RATE  |  +{:.1} RNG",
                tw.buff_dmg * 100.0,
                tw.buff_rate * 100.0,
                tw.buff_range
            ),
            FontId::proportional(10.5),
            pal::GOOD,
        );
    }

    let action_rect = Rect::from_min_max(
        pos2(main.left() + 12.0, main.bottom() - 51.0),
        pos2(main.right() - 12.0, main.bottom() - 9.0),
    );
    let mut action = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(action_rect));
    child.horizontal(|ui| {
        let gap = ui.spacing().item_spacing.x;
        let bw = ((action_rect.width() - gap * 2.0) / 3.0).max(70.0);
        match choices.len() {
            0 => {
                ui.add_enabled(
                    false,
                    egui::Button::new(RichText::new("MAXIMUM\nTIER").size(10.5))
                        .fill(pal::CARD)
                        .min_size(vec2(bw, 40.0)),
                );
            }
            1 => {
                let (_, cost) = choices[0];
                let can = g.can_afford(cost);
                let button = egui::Button::new(
                    RichText::new(format!("UPGRADE [U]\n{}g", gold_str(cost as i64)))
                        .size(10.5)
                        .strong(),
                )
                .fill(if can {
                    Color32::from_rgb(40, 96, 74)
                } else {
                    pal::CARD
                })
                .min_size(vec2(bw, 40.0));
                if ui
                    .add_enabled(can, button)
                    .on_hover_ui(|ui| tower_tooltip(ui, choices[0].0, g))
                    .clicked()
                {
                    action = Some(Action::Upgrade);
                }
            }
            n => {
                let deepest = choices
                    .iter()
                    .map(|(into, _)| display_ladder_len(*into))
                    .max()
                    .unwrap_or(1);
                ui.add_enabled(
                    false,
                    egui::Button::new(
                        RichText::new(format!("{n} PATHS\nUP TO {deepest} RANKS")).size(10.0),
                    )
                    .fill(Color32::from_rgb(86, 68, 27))
                    .min_size(vec2(bw, 40.0)),
                );
            }
        }
        if ui
            .add(
                egui::Button::new(RichText::new(format!("TARGET\n{}", tw.mode.label())).size(10.5))
                    .fill(pal::CARD)
                    .min_size(vec2(bw, 40.0)),
            )
            .on_hover_text(format!(
                "{}\nClick to cycle priority.",
                tw.mode.description()
            ))
            .clicked()
        {
            action = Some(Action::Target);
        }
        if ui
            .add(
                egui::Button::new(RichText::new(format!(
                    "SELL [S]\n{}g",
                    gold_str(g.tower_sell_value(ti) as i64)
                )))
                .fill(Color32::from_rgb(66, 31, 39))
                .min_size(vec2(bw, 40.0)),
            )
            .on_hover_text("Sell this tower for the exact refund shown.")
            .clicked()
        {
            action = Some(Action::Sell);
        }
    });

    match action {
        Some(Action::Upgrade) => g.upgrade(ti),
        Some(Action::Target) => {
            g.towers[ti].mode = g.towers[ti].mode.next();
            g.wants_save = true;
            g.sound_cues.push(Cue::Select);
        }
        Some(Action::Sell) => g.sell(ti),
        None => {}
    }
}

/// One wave card is more useful than a miniature schedule on a fixed map: it
/// gives the next counter, its scale, and the one action available right now.
fn next_wave_card(g: &Game, ui: &mut Ui, rect: Rect) {
    let n = g.upcoming_wave_number();
    let resp = ui.interact(rect, Id::new("next_wave_card"), Sense::click().union(Sense::hover()));
    resp.on_hover_ui(|ui| render_encounter_threat_tooltip(ui, g, n, g.difficulty.elite_stride(n)));
    carved(ui, rect, pal::PANEL_DEEP, false);
    let p = ui.painter();
    let wave = g.next_wave_def();
    let trait_name = if !wave.tag.is_empty() {
        wave.tag
    } else {
        wave.armour_type.name()
    };
    let trait_line = if wave.flying {
        format!("{}  /  AIR", trait_name.to_ascii_uppercase())
    } else {
        trait_name.to_ascii_uppercase()
    };
    let trait_colour = if wave.flying {
        c32(AIR_TINT, 1.0)
    } else {
        c32(wave.armour_type.color(), 1.0)
    };
    let (clock, action, action_colour) = if g.pending_doctrine {
        (
            "PAUSED".to_owned(),
            if g.is_campaign() { "CHOOSE CAMPAIGN PERK" } else { "CHOOSE COMMAND UPGRADE" },
            pal::GOLD,
        )
    } else if g.is_campaign() && g.phase == Phase::Build {
        (
            format!("AUTO {:.0}s", g.wave_timer.ceil().max(0.0)),
            "AUTO DEPLOY · ENTER NOW",
            pal::GOOD,
        )
    } else if g.prep {
        ("READY".to_owned(), "ENTER TO START WAVE 1", pal::GOOD)
    } else if g.spawn_left > 0 {
        (
            format!("{} IN", g.spawn_left),
            if g.is_campaign() { "CURRENT PACKET IS DEPLOYING" } else { "CURRENT WAVE IS DEPLOYING" },
            pal::ACC,
        )
    } else if g.wave_timer > 0.5 {
        (
            format!("{:.0}s", g.wave_timer.ceil()),
            if g.is_campaign() { "NEXT FORMATION AUTO-STREAMS" } else { "ENTER TO RUSH THE NEXT WAVE" },
            pal::GOLD,
        )
    } else {
        ("READY".to_owned(), "ENTER TO CALL THE NEXT WAVE", pal::GOOD)
    };

    let heading = if g.is_campaign() {
        let id = crate::game::campaign::encounter_id(n as u16);
        format!("NEXT  /  C{} E{:02}  ·  {n}/600", id.chapter, id.local)
    } else {
        format!("NEXT  /  WAVE {n:02}")
    };
    p.text(
        rect.left_top() + vec2(9.0, 6.0),
        Align2::LEFT_TOP,
        heading,
        FontId::monospace(8.5),
        pal::GOLD_LINE,
    );
    p.text(
        rect.right_top() + vec2(-9.0, 6.0),
        Align2::RIGHT_TOP,
        clock,
        FontId::monospace(9.0),
        action_colour,
    );
    let name_font = FontId::proportional(12.0);
    p.text(
        rect.left_top() + vec2(9.0, 19.0),
        Align2::LEFT_TOP,
        elide(ui, wave.name, &name_font, rect.width() - 74.0),
        name_font,
        pal::INK,
    );
    p.text(
        rect.right_top() + vec2(-9.0, 21.0),
        Align2::RIGHT_TOP,
        format!("x{}", wave.count),
        FontId::monospace(12.0),
        pal::GOLD,
    );
    p.text(
        rect.left_top() + vec2(9.0, 32.0),
        Align2::LEFT_TOP,
        trait_line,
        FontId::monospace(8.5),
        trait_colour,
    );
    p.text(
        rect.left_bottom() + vec2(9.0, -4.0),
        Align2::LEFT_BOTTOM,
        action,
        FontId::monospace(7.5),
        action_colour,
    );
}

fn wave_intel(g: &Game, ui: &Ui, rect: Rect) {
    carved(ui, rect, pal::PANEL_DEEP, false);
    let p = ui.painter();
    p.text(
        rect.left_top() + vec2(9.0, 7.0),
        Align2::LEFT_TOP,
        "NEXT THREATS",
        FontId::monospace(8.5),
        pal::GOLD_LINE,
    );
    for k in 0..4u32 {
        let n = g.upcoming_wave_number() + k;
        if n > g.last_wave() {
            break;
        }
        let wave = g.wave_def(n);
        let row = Rect::from_min_size(
            pos2(rect.left() + 7.0, rect.top() + 25.0 + k as f32 * 37.0),
            vec2(rect.width() - 14.0, 32.0),
        );
        if k == 0 {
            p.rect_filled(row, CornerRadius::same(2), Color32::from_rgb(30, 45, 39));
            p.rect_stroke(
                row,
                CornerRadius::same(2),
                Stroke::new(1.0, pal::GOLD_LINE),
                StrokeKind::Inside,
            );
        }
        p.text(
            pos2(row.left() + 6.0, row.center().y),
            Align2::LEFT_CENTER,
            format!("{n:02}"),
            FontId::monospace(11.0),
            if k == 0 { pal::GOLD } else { pal::DIM },
        );
        let tag = if !wave.tag.is_empty() {
            wave.tag
        } else {
            wave.armour_type.name()
        };
        p.text(
            pos2(row.left() + 34.0, row.center().y - 6.0),
            Align2::LEFT_CENTER,
            tag,
            FontId::proportional(11.0),
            c32(wave.armour_type.color(), if k == 0 { 1.0 } else { 0.78 }),
        );
        p.text(
            pos2(row.left() + 34.0, row.center().y + 8.0),
            Align2::LEFT_CENTER,
            format!("x{}{}", wave.count, if wave.flying { "  AIR" } else { "" }),
            FontId::monospace(8.5),
            if wave.flying {
                c32(AIR_TINT, 1.0)
            } else {
                pal::DIM
            },
        );
    }
}

fn selection_panel(g: &mut Game, ui: &mut Ui, compact: bool, w: f32) {
    if !compact {
        selection_panel_desktop(g, ui, w);
        return;
    }
    let (rect, _) = ui.allocate_exact_size(vec2(w, bar_h(compact)), Sense::hover());
    carved(ui, rect, pal::PANEL, true);
    let p = ui.painter();

    let Some(ti) = g.selected.filter(|&i| i < g.towers.len()) else {
        let opening = g.wave == 0 && g.towers.is_empty();
        let (title, detail, accent) = if let Some((def_i, _)) = g.build_choice {
            let def_opt = TOWERS.get(def_i);
            let name = def_opt.map(|d| d.name).unwrap_or("TOWER");
            let counter_note = if g.is_campaign() {
                if let Some(d) = def_opt {
                    let (s_short, w_short) = tower_counterplay_short(d);
                    let volley = if matches!(d.family, Family::Multi | Family::SuperMulti) {
                        " · 100%/40% volley"
                    } else {
                        ""
                    };
                    format!("+{s_short} · -{w_short}{volley} · ")
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            (
                format!("PLACING {}", name.to_ascii_uppercase()),
                format!("{counter_note}Click clear grass"),
                pal::ACC,
            )
        } else if opening {
            (
                "START HERE: PICK A TOWER CARD".to_owned(),
                "Then click a free plot beside the road".to_owned(),
                pal::ACC,
            )
        } else {
            (
                "SELECT A TOWER".to_owned(),
                "Click a tower to inspect, upgrade, or sell".to_owned(),
                pal::DIM,
            )
        };
        p.text(
            pos2(rect.center().x, rect.center().y - 12.0),
            Align2::CENTER_CENTER,
            title,
            FontId::monospace(11.5),
            accent,
        );
        p.text(
            pos2(rect.center().x, rect.center().y + 12.0),
            Align2::CENTER_CENTER,
            detail,
            FontId::proportional(11.5),
            pal::DIM,
        );
        if g.doctrine_picks() > 0 {
            p.text(
                pos2(rect.center().x, rect.bottom() - 18.0),
                Align2::CENTER_CENTER,
                format!(
                    "COMMAND  ARS {}  |  SPD {}  |  RNG {}",
                    g.doctrines[0], g.doctrines[1], g.doctrines[2]
                ),
                FontId::monospace(9.5),
                pal::GOOD,
            );
        }
        return;
    };

    let tw = g.towers[ti].clone();
    let def = tw.def();
    let col = tower_color(def);
    let choices = tw.upgrades();

    // Portrait frame.
    // The portrait shares the same authored icon language as the shop, so the
    // thing selected on the field is instantly recognisable below.
    // A rail dossier has one short read band and one separate command band.
    // The former 64px portrait collided with the actions below it in a 108px
    // panel, making the selected state look like one cramped block.
    let port = Rect::from_min_size(rect.left_top() + vec2(9.0, 8.0), vec2(42.0, 42.0));
    carved(ui, port, pal::PANEL_DEEP, true);
    let p = ui.painter();
    if let Some(texture) = tower_icons(ui.ctx()) {
        p.image(
            texture.id(),
            port.shrink(3.0),
            tower_icon_uv(def),
            Color32::WHITE,
        );
        p.rect_filled(
            Rect::from_min_max(
                pos2(port.left() + 3.0, port.bottom() - 16.0),
                pos2(port.right() - 3.0, port.bottom() - 3.0),
            ),
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(4, 8, 8, 210),
        );
    } else {
        p.rect_filled(
            Rect::from_center_size(port.center(), vec2(21.0, 21.0)),
            CornerRadius::same(3),
            c32(col, 1.0),
        );
    }
    // A Siege Tower has twenty levels and a Poison Tower fifteen, so the step
    // is only meaningful next to the length of its own path.
    p.text(
        port.center_bottom() + vec2(0.0, -8.0),
        Align2::CENTER_CENTER,
        format!("{} / {}", tw.level(), tw.ladder_len()),
        FontId::monospace(8.0),
        pal::GOLD,
    );
    tower_rank_track(
        p,
        Rect::from_min_max(
            pos2(port.left() + 4.0, port.bottom() - 20.0),
            pos2(port.right() - 4.0, port.bottom() - 17.0),
        ),
        tw.level(),
        tw.ladder_len(),
        c32(col, 1.0),
    );

    let tx = port.right() + 8.0;
    let name_font = FontId::proportional(12.5);
    p.text(
        pos2(tx, port.top() + 1.0),
        Align2::LEFT_TOP,
        elide(ui, tw.full_name(), &name_font, rect.right() - tx - 10.0),
        name_font,
        c32(col, 1.0),
    );
    p.text(
        pos2(tx, port.top() + 16.0),
        Align2::LEFT_TOP,
        format!(
            "{} · {} · {}",
            def.family.name(),
            def.attack.name(),
            def.targets.label()
        ),
        FontId::proportional(8.5),
        c32(col, 0.9),
    );

    let dps = tw.dmg() * tw.rate();
    let stats_line = if tw.is_support() {
        format!("aura radius {:.1}", def.abil.aura_range.max(tw.range()))
    } else {
        format!("{} dps   ·   range {:.1}", short(dps as f64), tw.range())
    };
    p.text(
        pos2(tx, port.top() + 30.0),
        Align2::LEFT_TOP,
        stats_line,
        FontId::monospace(9.5),
        pal::INK,
    );
    if g.is_campaign() {
        let (s_short, w_short) = tower_counterplay_short(def);
        let volley_note = if matches!(def.family, Family::Multi | Family::SuperMulti) {
            "  ·  100%/40% volley"
        } else {
            ""
        };
        p.text(
            pos2(tx, port.top() + 41.5),
            Align2::LEFT_TOP,
            format!("+{s_short}  ·  -{w_short}{volley_note}"),
            FontId::monospace(8.5),
            pal::DIM,
        );
    }

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
    // A full-width desktop dossier can show contribution history. The short
    // rail keeps the read band clean and reserves its lower half for commands.
    if rect.height() > 140.0 {
        p.text(
            pos2(tx, port.top() + 52.0),
            Align2::LEFT_TOP,
            contribution,
            FontId::monospace(10.5),
            pal::DIM,
        );
        if tw.buff_dmg > 0.0 || tw.buff_rate > 0.0 || tw.buff_range > 0.0 {
            p.text(
                pos2(tx, port.top() + 68.0),
                Align2::LEFT_TOP,
                format!(
                    "+{:.0}% DMG  |  +{:.0}% RATE  |  +{:.1} RNG",
                    tw.buff_dmg * 100.0,
                    tw.buff_rate * 100.0,
                    tw.buff_range
                ),
                FontId::proportional(10.5),
                pal::GOOD,
            );
        }
    }

    // Command buttons along the bottom of the panel.
    let mut action: Option<Action> = None;
    // Share the available width evenly so every tower command remains an
    // honest target within the rail, separate from the read band above.
    let bar = Rect::from_min_size(
        rect.left_bottom() + vec2(9.0, -44.0),
        vec2((rect.width() - 18.0).max(1.0), 38.0),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar));
    child.horizontal(|ui| {
        let gap = ui.spacing().item_spacing.x;
        let button_w = ((bar.width() - gap * 2.0) / 3.0).max(1.0);
        match choices.len() {
            0 => {
                let b = egui::Button::new(RichText::new("Fully upgraded").size(9.5))
                    .fill(pal::CARD)
                    .min_size(vec2(button_w, 36.0));
                ui.add_enabled(false, b);
            }
            1 => {
                let (into, cost) = choices[0];
                let can = g.can_afford(cost);
                let current = def.effective_dps();
                let next = TOWERS[into].effective_dps();
                let gain = if current > 0.0 {
                    ((next / current - 1.0) * 100.0).round().max(0.0) as u32
                } else {
                    0
                };
                let label = if gain > 0 {
                    format!("Upgrade +{gain}%\n{}g", gold_str(cost as i64))
                } else {
                    format!("Upgrade\n{}g", gold_str(cost as i64))
                };
                let b = egui::Button::new(RichText::new(label).size(9.5).strong())
                    .fill(if can {
                        Color32::from_rgb(43, 110, 190)
                    } else {
                        pal::CARD
                    })
                    .min_size(vec2(button_w, 36.0));
                if ui
                    .add_enabled(can, b)
                    .on_hover_ui(|ui| tower_tooltip(ui, into, g))
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
                        .size(9.5)
                        .strong(),
                )
                .fill(Color32::from_rgb(96, 74, 26))
                .min_size(vec2(button_w, 36.0));
                ui.add_enabled(false, b);
            }
        }

        let tb = egui::Button::new(RichText::new(format!("Target\n{}", tw.mode.label())).size(9.5))
            .fill(pal::CARD)
            .min_size(vec2(button_w, 36.0));
        if ui
            .add(tb)
            .on_hover_text(format!(
                "{}\nClick to cycle targeting priority.",
                tw.mode.description()
            ))
            .clicked()
        {
            action = Some(Action::Target);
        }

        let sb = egui::Button::new(
            RichText::new(format!(
                "Sell\n{}g",
                gold_str(g.tower_sell_value(ti) as i64)
            ))
            .size(9.5),
        )
        .fill(Color32::from_rgb(64, 32, 42))
        .min_size(vec2(button_w, 36.0));
        if ui
            .add(sb)
            .on_hover_text("Sell this tower for the exact refund shown.")
            .clicked()
        {
            action = Some(Action::Sell);
        }
    });

    match action {
        Some(Action::Upgrade) => g.upgrade(ti),
        Some(Action::Target) => {
            g.towers[ti].mode = g.towers[ti].mode.next();
            g.wants_save = true;
        }
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
fn build_palette(g: &mut Game, ui: &mut Ui, ust: &mut UiState, width: f32) {
    if ust.compact {
        build_palette_strip(g, ui, ust);
    } else {
        build_palette_grid(g, ui, ust, width);
    }
}

/// The fixed rail ends in a proper RTS command grid: visible icon commands,
/// keycaps and costs below a selected-defense dossier. Names and rules remain
/// available in the tooltip/dossier rather than making the build menu read as
/// a modern settings list. There are twelve physical cells so all eleven shop
/// choices retain their number-row hotkeys, including `-`, with one quiet
/// spare cell rather than a paged or scrolling command list.
fn build_palette_dock(g: &mut Game, ui: &mut Ui, ust: &mut UiState, width: f32) {
    // Eleven shop commands fit in three compact rows at the fixed 304px rail;
    // six branch choices fit in two.  The former 298px well was sized for a
    // 600px rail and left an entire visibly empty command row once the stage
    // was corrected to a sensible desktop proportion.
    const H: f32 = 248.0;
    const GAP: f32 = 5.0;
    const PAD: f32 = 8.0;
    const HEADER_H: f32 = 24.0;

    let (rect, _) = ui.allocate_exact_size(vec2(width, H), Sense::hover());
    carved(ui, rect, pal::PANEL_DEEP, false);
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
        None => g.shop_entries(),
    };
    let p = ui.painter();
    p.text(
        rect.left_top() + vec2(PAD + 1.0, 6.0),
        Align2::LEFT_TOP,
        if branching.is_some() {
            "CHOOSE UPGRADE"
        } else {
            "BUILD TOWER"
        },
        FontId::monospace(8.5),
        if branching.is_some() {
            pal::GOLD
        } else {
            pal::GOLD_LINE
        },
    );
    p.text(
        rect.right_top() + vec2(-(PAD + 1.0), 6.0),
        Align2::RIGHT_TOP,
        "1-0 / -",
        FontId::monospace(8.0),
        pal::DIM,
    );

    let well = Rect::from_min_max(
        rect.left_top() + vec2(PAD, HEADER_H),
        rect.right_bottom() - vec2(PAD, PAD),
    );

    ust.hotkeys.clear();
    ust.card_rects.clear();
    ust.palette_rect = rect;
    ust.palette_page = 0;

    if entries.is_empty() {
        p.text(
            well.center(),
            Align2::CENTER_CENTER,
            "No command available",
            FontId::proportional(11.0),
            pal::DIM,
        );
        return;
    }

    // Keep a six-way tower fork as two intentional rows of three, rather
    // than six postage stamps across a wide dashboard.  The full shop remains
    // a four-column command grid, with a centred partial final row instead of
    // painted empty slots that make the rail look unfinished.
    let cols = match entries.len() {
        1..=3 => entries.len(),
        4 => 4,
        5..=6 => 3,
        _ => 4,
    };
    let rows = entries.len().div_ceil(cols);
    let side = ((well.width() - GAP * (cols.saturating_sub(1)) as f32) / cols as f32)
        .min((well.height() - GAP * (rows.saturating_sub(1)) as f32) / rows as f32)
        .floor()
        .max(44.0);
    let grid_h = side * rows as f32 + GAP * (rows.saturating_sub(1)) as f32;
    let top = (well.center().y - grid_h * 0.5).round();
    for row in 0..rows {
        let first = row * cols;
        let count = (entries.len() - first).min(cols);
        let row_w = side * count as f32 + GAP * (count.saturating_sub(1)) as f32;
        let left = (well.center().x - row_w * 0.5).round();
        for col in 0..count {
            let slot = first + col;
            let card = Rect::from_min_size(
                pos2(left + col as f32 * (side + GAP), top + row as f32 * (side + GAP)),
                vec2(side, side),
            );
            if let Some(&def_i) = entries.get(slot) {
                ust.hotkeys.push(def_i);
                ust.card_rects.push(card);
                tower_command_card(g, ui, card, def_i, branching, palette_hotkey(slot));
            }
        }
    }
}

/// A readable 54px tower target for the two-column dock palette.
fn tower_dock_card(
    g: &mut Game,
    ui: &mut Ui,
    rect: Rect,
    def_i: usize,
    upgrading: Option<usize>,
    hotkey: &str,
) {
    let def = &TOWERS[def_i];
    let affordable = g.can_afford(def.gold);
    let selected = upgrading.is_none() && g.build_choice.map(|(d, _)| d) == Some(def_i);
    let resp = ui.interact(
        rect,
        ui.id().with(("tower_dock_card", def_i)),
        Sense::click(),
    );
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.08);
    slot_frame_amount(ui, rect, hover, selected);
    let p = ui.painter_at(rect);

    let icon_side = (rect.height() - 12.0).min(40.0).max(1.0);
    let icon = Rect::from_center_size(
        pos2(rect.left() + 6.0 + icon_side * 0.5, rect.center().y),
        vec2(icon_side, icon_side),
    );
    p.rect_filled(icon, CornerRadius::same(2), Color32::from_rgb(8, 12, 11));
    if let Some(texture) = tower_icons(ui.ctx()) {
        p.image(
            texture.id(),
            icon,
            tower_icon_uv(def),
            if affordable {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(112, 112, 112, 190)
            },
        );
    } else {
        p.circle_filled(
            icon.center(),
            icon.width() * 0.28,
            c32(tower_color(def), if affordable { 1.0 } else { 0.4 }),
        );
    }
    let key = Rect::from_min_size(icon.left_top() + vec2(2.0, 2.0), vec2(13.0, 13.0));
    p.rect_filled(
        key,
        CornerRadius::same(2),
        Color32::from_rgba_unmultiplied(3, 7, 6, 226),
    );
    p.text(
        key.center(),
        Align2::CENTER_CENTER,
        hotkey,
        FontId::monospace(8.5),
        pal::GOLD,
    );

    let text_left = icon.right() + 6.0;
    let text_width = (rect.right() - text_left - 6.0).max(12.0);
    let name_font = FontId::proportional(10.5);
    p.text(
        pos2(text_left, rect.top() + 5.0),
        Align2::LEFT_TOP,
        // The authored unit name belongs in the hover detail. The rail needs a
        // recognisable family name that never becomes "Bouncing Tow..".
        def.family.short(),
        name_font,
        if affordable {
            c32(tower_color(def), 1.0)
        } else {
            pal::DIM
        },
    );
    let family_font = FontId::proportional(8.0);
    p.text(
        pos2(text_left, rect.top() + 19.0),
        Align2::LEFT_TOP,
        elide(ui, def.family.name(), &family_font, text_width),
        family_font,
        pal::DIM,
    );
    let role_font = FontId::monospace(7.5);
    let cost = gold_str(def.gold as i64);
    let cost_width = p
        .layout_no_wrap(cost.clone(), FontId::monospace(9.0), pal::GOLD)
        .rect
        .width();
    p.text(
        pos2(text_left, rect.bottom() - 6.0),
        Align2::LEFT_BOTTOM,
        elide(
            ui,
            def.family.role_tag(),
            &role_font,
            (text_width - cost_width - 9.0).max(8.0),
        ),
        role_font,
        c32(def.family.fx_color(), if affordable { 0.95 } else { 0.48 }),
    );
    p.circle_filled(
        pos2(rect.right() - cost_width - 8.0, rect.bottom() - 10.0),
        2.2,
        if affordable { pal::GOLD } else { pal::BAD },
    );
    p.text(
        rect.right_bottom() + vec2(-5.0, -6.0),
        Align2::RIGHT_BOTTOM,
        cost,
        FontId::monospace(9.0),
        if affordable { pal::GOLD } else { pal::BAD },
    );

    if resp.clicked() {
        match upgrading {
            Some(ti) => g.upgrade_into(ti, def_i),
            None if affordable => {
                g.build_choice = Some((def_i, 1));
                g.selected = None;
                g.sound_cues.push(Cue::Select);
            }
            None => {
                g.error(format!(
                    "Need {}g for {}",
                    gold_str(def.gold as i64),
                    def.name
                ));
                g.sound_cues.push(Cue::Error);
            }
        }
    }
    resp.on_hover_ui(|ui| tower_tooltip(ui, def_i, g));
}

/// Desktop command card: the familiar four-by-three muscle-memory grid. The
/// old eleven-wide carousel forced the eye across half the monitor and made
/// every tower compete with the selected unit. Here icons are commands; names,
/// rules and exact numbers belong to the hover dossier.
fn build_palette_grid(g: &mut Game, ui: &mut Ui, ust: &mut UiState, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, BAR_H), Sense::hover());
    carved(ui, rect, pal::PANEL, true);
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
        None => g.shop_entries(),
    };
    let p = ui.painter();
    p.text(
        rect.left_top() + vec2(10.0, 7.0),
        Align2::LEFT_TOP,
        if branching.is_some() {
            "CHOOSE UPGRADE"
        } else {
            "TOWER COMMANDS"
        },
        FontId::monospace(8.5),
        if branching.is_some() {
            pal::GOLD
        } else {
            pal::GOLD_LINE
        },
    );
    p.text(
        rect.right_top() + vec2(-10.0, 7.0),
        Align2::RIGHT_TOP,
        "1-0 / -",
        FontId::monospace(8.0),
        pal::DIM,
    );

    const COLS: usize = 4;
    const ROWS: usize = 3;
    const GAP: f32 = 5.0;
    let well = Rect::from_min_max(
        rect.left_top() + vec2(8.0, 24.0),
        rect.right_bottom() - vec2(8.0, 8.0),
    );
    let side = ((well.width() - GAP * (COLS - 1) as f32) / COLS as f32)
        .min((well.height() - GAP * (ROWS - 1) as f32) / ROWS as f32)
        .floor()
        .max(44.0);
    let grid_size = vec2(
        side * COLS as f32 + GAP * (COLS - 1) as f32,
        side * ROWS as f32 + GAP * (ROWS - 1) as f32,
    );
    let origin = pos2(
        (well.center().x - grid_size.x * 0.5).round(),
        (well.center().y - grid_size.y * 0.5).round(),
    );

    ust.hotkeys.clear();
    ust.card_rects.clear();
    ust.palette_rect = rect;
    ust.palette_page = 0;
    for slot in 0..COLS * ROWS {
        let col = slot % COLS;
        let row = slot / COLS;
        let card = Rect::from_min_size(
            origin + vec2(col as f32 * (side + GAP), row as f32 * (side + GAP)),
            vec2(side, side),
        );
        if let Some(&def_i) = entries.get(slot) {
            ust.hotkeys.push(def_i);
            ust.card_rects.push(card);
            tower_command_card(g, ui, card, def_i, branching, palette_hotkey(slot));
        } else {
            slot_frame(ui, card, false, false);
        }
    }
}

fn tower_command_card(
    g: &mut Game,
    ui: &mut Ui,
    rect: Rect,
    def_i: usize,
    upgrading: Option<usize>,
    hotkey: &str,
) {
    let def = &TOWERS[def_i];
    let affordable = g.can_afford(def.gold);
    let selected = upgrading.is_none() && g.build_choice.map(|(d, _)| d) == Some(def_i);
    let resp = ui.interact(rect, ui.id().with(("tower_command", def_i)), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.08);
    slot_frame_amount(ui, rect, hover, selected);
    let p = ui.painter_at(rect);
    let icon = rect.shrink(3.0);
    p.rect_filled(icon, CornerRadius::same(2), Color32::from_rgb(8, 12, 11));
    if let Some(texture) = tower_icons(ui.ctx()) {
        p.image(
            texture.id(),
            icon,
            tower_icon_uv(def),
            if affordable {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(112, 112, 112, 190)
            },
        );
    } else {
        p.circle_filled(
            icon.center(),
            icon.width() * 0.28,
            c32(tower_color(def), if affordable { 1.0 } else { 0.4 }),
        );
    }
    let cost_band =
        Rect::from_min_max(pos2(icon.left(), icon.bottom() - 16.0), icon.right_bottom());
    p.rect_filled(
        cost_band,
        CornerRadius::ZERO,
        Color32::from_rgba_unmultiplied(4, 8, 7, 220),
    );
    p.circle_filled(
        pos2(cost_band.left() + 8.0, cost_band.center().y),
        3.0,
        pal::GOLD,
    );
    p.text(
        pos2(cost_band.left() + 14.0, cost_band.center().y),
        Align2::LEFT_CENTER,
        gold_str(def.gold as i64),
        FontId::monospace(9.5),
        if affordable { pal::GOLD } else { pal::BAD },
    );
    let key = Rect::from_min_size(icon.left_top() + vec2(2.0, 2.0), vec2(14.0, 14.0));
    p.rect_filled(
        key,
        CornerRadius::same(2),
        Color32::from_rgba_unmultiplied(3, 7, 6, 226),
    );
    p.text(
        key.center(),
        Align2::CENTER_CENTER,
        hotkey,
        FontId::monospace(9.0),
        pal::GOLD,
    );
    if def.targets != Targets::GroundOnly && def.targets != Targets::Nothing {
        p.text(
            icon.right_top() + vec2(-3.0, 3.0),
            Align2::RIGHT_TOP,
            if def.targets == Targets::AirOnly { "AIR!" } else { "G+A" },
            FontId::monospace(7.5),
            c32(AIR_TINT, if affordable { 1.0 } else { 0.5 }),
        );
    }
    let rank = format!("L1/{}", display_ladder_len(def_i));
    let rank_box = Rect::from_min_size(icon.right_top() + vec2(-26.0, 16.0), vec2(24.0, 10.0));
    p.rect_filled(
        rank_box,
        CornerRadius::same(2),
        Color32::from_rgba_unmultiplied(3, 7, 6, 222),
    );
    p.text(
        rank_box.center(),
        Align2::CENTER_CENTER,
        rank,
        FontId::monospace(6.5),
        pal::GOLD,
    );

    if resp.clicked() {
        match upgrading {
            Some(ti) => g.upgrade_into(ti, def_i),
            None if affordable => {
                g.build_choice = Some((def_i, 1));
                g.selected = None;
                g.sound_cues.push(Cue::Select);
            }
            None => {
                g.error(format!(
                    "Need {}g for {}",
                    gold_str(def.gold as i64),
                    def.name
                ));
                g.sound_cues.push(Cue::Error);
            }
        }
    }
    resp.on_hover_ui(|ui| tower_tooltip(ui, def_i, g));
}

fn slot_frame_amount(ui: &Ui, r: Rect, hover: f32, chosen: bool) {
    let mix = |a: Color32, b: Color32, t: f32| {
        let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
        Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
    };
    let p = ui.painter();
    p.rect_filled(
        r,
        CornerRadius::same(3),
        mix(pal::PANEL_DEEP, pal::CARD_HOVER, hover),
    );
    p.rect_stroke(
        r,
        CornerRadius::same(3),
        Stroke::new(
            if chosen { 2.0 } else { 1.0 + hover },
            if chosen {
                pal::GOLD
            } else {
                mix(pal::GOLD_LINE, pal::GOLD, hover)
            },
        ),
        StrokeKind::Inside,
    );
}

fn build_palette_strip(g: &mut Game, ui: &mut Ui, ust: &mut UiState) {
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
        None => g.shop_entries(),
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
        tower_card(g, ui, card, i, branching, palette_hotkey(slot));
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
        let w = g.wave_def(n);
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
        } else if g.difficulty.elite_stride(n).is_some() {
            // A gold V says that Vanguards are mixed into the wave without
            // hiding the armour label that determines the player's counter.
            p.text(
                pos2(rect.right() - 10.0, y + row_h * 0.5),
                Align2::CENTER_CENTER,
                "V",
                FontId::monospace(9.0),
                pal::GOLD,
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
                let w = g.wave_def(n);
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
    hotkey: &str,
) {
    let def = &TOWERS[def_i];
    let cost = def.gold;
    let affordable = g.can_afford(cost);
    let selected = upgrading.is_none() && g.build_choice.map(|(d, _)| d) == Some(def_i);
    let col = tower_color(def);

    let resp = ui.interact(rect, ui.id().with(("build_card", def_i)), Sense::click());
    slot_frame(ui, rect, resp.hovered(), selected);
    let p = ui.painter_at(rect);

    // Lay out down the card as fractions of its height. Illustrated family
    // icons replace the old flat colour chips; target and attack labels remain
    // beside them because those are rules, not decoration.
    let h = rect.height();
    let compact_card = rect.width() <= 70.0;
    let icon_side = (h * 0.54).min(rect.width() * 0.80);
    let icon = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + h * 0.31),
        vec2(icon_side, icon_side),
    );
    // The source icons have dark geometry; a black swatch made red/navy
    // families indistinguishable at command-bar scale. A restrained family
    // backplate and brighter edge preserve the authored pixels while making
    // the silhouette readable before a player opens a tooltip.
    p.rect_filled(icon, CornerRadius::same(2), c32(col, if affordable { 0.46 } else { 0.22 }));
    if let Some(texture) = tower_icons(ui.ctx()) {
        p.image(
            texture.id(),
            icon,
            tower_icon_uv(def),
            if affordable {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(120, 120, 120, 190)
            },
        );
    } else {
        p.rect_filled(
            icon.shrink(icon_side * 0.16),
            CornerRadius::same(2),
            c32(col, if affordable { 1.0 } else { 0.42 }),
        );
    }
    p.rect_stroke(
        icon,
        CornerRadius::same(2),
        Stroke::new(1.35, c32(col, if affordable { 0.98 } else { 0.45 })),
        StrokeKind::Inside,
    );
    // Which layers it answers. Missing that a tower cannot reach the air is
    // what loses wave 7, and it belongs on the card rather than in a tooltip.
    let (tag, tint) = match def.targets {
        Targets::Both => ("G+A", AIR_TINT),
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
    if !compact_card {
        let rank = format!("L1/{}", display_ladder_len(def_i));
        let rank_box = Rect::from_min_size(rect.right_top() + vec2(-29.0, 15.0), vec2(26.0, 11.0));
        p.rect_filled(rank_box, CornerRadius::same(2), Color32::from_rgba_unmultiplied(3, 7, 6, 222));
        p.text(rank_box.center(), Align2::CENTER_CENTER, rank, FontId::monospace(6.8), pal::GOLD);
    }

    let name_font = FontId::proportional(11.5);
    p.text(
        pos2(rect.center().x, rect.top() + h * 0.61),
        Align2::CENTER_TOP,
        elide(ui, def.family.short(), &name_font, rect.width() - 6.0),
        name_font,
        pal::INK,
    );
    // Elided, not clipped. The palette shrinks its cards to fit, so a fixed
    // string will eventually be wider than the card - and a centred one then
    // loses characters off *both* ends.
    if !compact_card {
        let role_font = FontId::proportional(9.0);
        p.text(pos2(rect.center().x, rect.top() + h * 0.74), Align2::CENTER_TOP,
            elide(ui, def.family.role_tag(), &role_font, rect.width() - 6.0), role_font,
            c32(def.family.fx_color(), 0.95));
    }
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
        hotkey,
        FontId::monospace(9.5),
        pal::GOLD,
    );

    if resp.clicked() {
        match upgrading {
            Some(ti) => g.upgrade_into(ti, def_i),
            None => {
                g.build_choice = Some((def_i, 1));
                g.selected = None;
                g.sound_cues.push(Cue::Select);
            }
        }
    }
    resp.on_hover_ui(|ui| tower_tooltip(ui, def_i, g));
}

/// Eleven build cards fit the desktop command bar. Their shortcuts follow the
/// physical number row rather than printing impossible two-digit keys.
fn palette_hotkey(slot: usize) -> &'static str {
    const KEYS: [&str; 11] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-"];
    KEYS.get(slot).copied().unwrap_or("")
}

#[cfg(test)]
mod command_card_tests {
    use egui::{Context, RawInput, Rect, pos2, vec2};

    use super::{
        COMMAND_DOCK_CONTENT_H, COMMAND_DOCK_W, UiState, command_dock, install_style,
        palette_hotkey,
    };
    use crate::game::{Game, defs::shop_order};

    #[test]
    fn every_desktop_command_card_advertises_a_real_unique_key() {
        let keys: Vec<&str> = (0..11).map(palette_hotkey).collect();
        assert_eq!(
            keys,
            ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-"]
        );
    }

    #[test]
    fn desktop_dock_keeps_command_grid_inside_a_304px_rail() {
        let ctx = Context::default();
        install_style(&ctx);
        let mut game = Game::new();
        let mut ust = UiState::default();
        let dock = Rect::from_min_size(pos2(0.0, 0.0), vec2(COMMAND_DOCK_W, 650.0));

        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(dock),
                ..Default::default()
            },
            |ui| command_dock(&mut game, ui, &mut ust),
        );
        out.textures_delta.clear();

        assert!(
            COMMAND_DOCK_CONTENT_H <= dock.height(),
            "the standard desktop rail is shorter than the dock content"
        );
        assert_eq!(
            ust.hotkeys,
            shop_order(),
            "the dock must expose the complete keyboard command card"
        );
        assert_eq!(ust.card_rects.len(), shop_order().len());
        assert!(
            ust.card_rects
                .iter()
                .all(|card| card.width() >= 44.0 && card.height() >= 44.0
                    && (card.width() - card.height()).abs() < 0.1),
            "the rail must keep the square RTS command targets touchable: {:?}",
            ust.card_rects
        );
        assert!(
            dock.contains_rect(ust.palette_rect),
            "palette {:?} escapes dock {:?}",
            ust.palette_rect,
            dock
        );
        for (i, card) in ust.card_rects.iter().enumerate() {
            assert!(
                ust.palette_rect.contains_rect(*card) && dock.contains_rect(*card),
                "card {i} at {card:?} is outside the dock palette {:?}",
                ust.palette_rect
            );
        }
        for i in 0..ust.card_rects.len() {
            for j in i + 1..ust.card_rects.len() {
                assert!(
                    !ust.card_rects[i].intersects(ust.card_rects[j]),
                    "dock cards {i} and {j} overlap"
                );
            }
        }
    }
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

/// A tiny, legible ladder is more useful than hiding twenty actual upgrades
/// behind a tooltip. The notches make path depth visible on both the desktop
/// dossier and compact touch layout without turning the board into a text UI.
fn tower_rank_track(
    p: &egui::Painter,
    rect: Rect,
    level: u32,
    total: u32,
    accent: Color32,
) {
    let total = total.max(1);
    let fraction = (level.min(total) as f32 / total as f32).clamp(0.0, 1.0);
    p.rect_filled(rect, CornerRadius::same(1), Color32::from_rgb(5, 11, 9));
    let filled = Rect::from_min_max(
        rect.left_top(),
        pos2(rect.left() + rect.width() * fraction, rect.bottom()),
    );
    p.rect_filled(filled, CornerRadius::same(1), accent);
    p.rect_stroke(
        rect,
        CornerRadius::same(1),
        Stroke::new(0.7, pal::GOLD_LINE),
        StrokeKind::Inside,
    );
    let marks = total.min(20);
    for step in 1..marks {
        let x = rect.left() + rect.width() * step as f32 / marks as f32;
        p.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(0.45, Color32::from_rgba_unmultiplied(4, 8, 7, 185)),
        );
    }
}

pub fn tower_counterplay_summary(def: &TowerLevel) -> (&'static str, &'static str) {
    match def.family {
        Family::Multi => (
            "Swarms & light unarmoured crowds (+15%, primary 100% / fan 40%)",
            "Plated armour (0.60x), shields (0.65x; broken shields still resist if plated), flying (0.85x)",
        ),
        Family::SuperMulti => (
            "Mass swarms & broad waves (10 targets, primary 100% / fan 40%)",
            "Plated armour (0.75x), shields (0.78x; broken shields still resist if plated), flying (0.85x)",
        ),
        Family::Single => (
            "Plated armour (1.15x focused impact)",
            "Broad crowds without multi/splash support",
        ),
        Family::Siege => (
            "Dense groups & light swarms (+10% splash)",
            if def.targets == Targets::GroundOnly {
                "Flying enemies (ground targets only)"
            } else {
                "Single tough bosses"
            },
        ),
        Family::Air => (
            "Flying & airborne units (+25% damage)",
            if def.targets == Targets::AirOnly {
                "Ground enemies (air targets only)"
            } else {
                "Broad ground swarms (focused single-target)"
            },
        ),
        Family::Chaos | Family::SuperChaos => (
            "Active shields (+20%), unresisted chaos damage",
            "Broad swarms without splash support",
        ),
        Family::Destruction | Family::SuperDestruct => (
            "Dense armour & active shields (AoE chaos)",
            if def.targets == Targets::GroundOnly {
                "Flying enemies (ground targets only)"
            } else {
                "Slow attack rate against swarms"
            },
        ),
        Family::Corruption => (
            "Armour pen (per hit), suppresses healing 2s, active shields (+20%)",
            "High body counts (single target per shot, no permanent strip)",
        ),
        Family::Poison => (
            "Single tough targets & bosses (flat stacking DPS & slow)",
            "Fast runner swarms",
        ),
        Family::Slow | Family::Frost => (
            "Swift runners & dense packs (area slow / freeze)",
            "Fires no direct damage on its own",
        ),
        Family::Critical => (
            "Heavy plated bosses & high-HP units (massive crits)",
            "Single-target focus against swarms (no splash)",
        ),
        Family::OneStrike => (
            "Single heavy targets (massive hero damage)",
            "Wasted on low-HP swarms",
        ),
        Family::Demon => (
            "Shields (+20%), chance to execute non-bosses",
            "Expensive single-target investment",
        ),
        Family::Troll => (
            "Fast units (roots target), short-range chaos frenzy",
            "Short attack range",
        ),
        Family::Fire => (
            "Area burn every 0.25s & neighbour damage boost",
            "High purchase cost, short aura reach",
        ),
        Family::Aura | Family::Damage | Family::Speed => (
            "Amplifies surrounding tower damage and attack rate",
            "Fires no attacks directly",
        ),
        Family::Bouncing | Family::SuperBounce => (
            "Spread runners along lane (chain bounce)",
            "Single tough bosses",
        ),
        Family::King => (
            "Long range overview; unlocks endgame Super towers",
            "High initial cost",
        ),
    }
}

pub fn tower_counterplay_short(def: &TowerLevel) -> (&'static str, &'static str) {
    match def.family {
        Family::Multi => ("Swarms", "Armour, Shields, Air"),
        Family::SuperMulti => ("Mass Swarms", "Armour, Shields"),
        Family::Single => ("Armour", "Dense Crowds"),
        Family::Siege => (
            "AoE Splash",
            if def.targets == Targets::GroundOnly { "Flying" } else { "Single Bosses" },
        ),
        Family::Air => (
            "Anti-Air",
            if def.targets == Targets::AirOnly { "Ground" } else { "Swarms" },
        ),
        Family::Chaos | Family::SuperChaos => ("Shields, Chaos", "Broad Crowds"),
        Family::Destruction | Family::SuperDestruct => (
            "Chaos AoE",
            if def.targets == Targets::GroundOnly { "Flying" } else { "Slow Attack" },
        ),
        Family::Corruption => ("Armour-Pen, Anti-Heal", "Swarms"),
        Family::Poison => ("Bosses, DoT", "Fast Swarms"),
        Family::Slow | Family::Frost => ("Slow / Freeze", "No Direct Dmg"),
        Family::Critical => ("Heavy Crits", "Swarms"),
        Family::OneStrike => ("Hero Strike", "Swarms"),
        Family::Demon => ("Shields, Execute", "Cost"),
        Family::Troll => ("Root, Frenzy", "Short Range"),
        Family::Fire => ("Burn Aura (0.25s)", "Cost, Short Reach"),
        Family::Aura | Family::Damage | Family::Speed => ("Buff Auras", "No Attack"),
        Family::Bouncing | Family::SuperBounce => ("Chain Hits", "Single Bosses"),
        Family::King => ("Endgame", "Cost"),
    }
}

fn tower_tooltip(ui: &mut Ui, def_i: usize, g: &Game) {
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
            display_ladder_len(def_i)
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
    if g.is_campaign() {
        if matches!(def.family, Family::Multi | Family::SuperMulti) {
            ui.label(
                RichText::new("• Campaign Volley: Primary shot 100% damage, extra fan shots 40% damage")
                    .size(10.5)
                    .color(pal::ACC),
            );
        }
        let (strong, weak) = tower_counterplay_summary(def);
        ui.label(RichText::new(format!("• Campaign Strong vs: {strong}")).size(10.5).color(pal::GOOD));
        ui.label(RichText::new(format!("• Campaign Weak vs: {weak}")).size(10.5).color(pal::BAD));
    }
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

    for line in ability_lines(g, &def.abil) {
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

/// Everything an ability block does in the active ruleset. Legacy keeps the
/// imported map values; Campaign rider budgets are queried from combat so
/// ability text cannot drift from the damage actually applied on hit.
pub fn ability_lines(g: &Game, a: &Abil) -> Vec<String> {
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
            short(crate::game::combat::effective_poison_dps(g, a.poison_dps) as f64),
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
        v.push(format!("Pierces {} armour on hit & suppresses healing for 2s", a.armour_pen));
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
    } else if g.pending_doctrine {
        doctrine_draft(g, ctx, ust);
    }
    if ust.show_help && !g.pending_doctrine {
        help(ctx, ust);
    }
    if ust.show_threat_intel && !g.pending_doctrine {
        threat_intel_modal(g, ctx, ust);
    } else {
        ust.threat_intel_rect = None;
    }
}

/// Three clear, permanent choices at waves 10, 20 and 30. The simulation is
/// paused while this is open, so reading a build-defining decision never costs
/// the player half a wave.
fn doctrine_draft(g: &mut Game, ctx: &Context, ust: &mut UiState) {
    let mut picked = None;
    let viewport = ctx.content_rect();
    let outer_w = (viewport.width() - 24.0).clamp(220.0, 480.0);
    // The content width excludes the frame margins and stroke, so the outer
    // dialog remains within the promised viewport inset.
    let content_w = (outer_w - 28.0).max(192.0);
    let scroll_h = (viewport.height() - 74.0).max(160.0);
    let modal_resp = egui::Modal::new(Id::new("doctrine_draft"))
        .frame(
            egui::Frame::NONE
                .fill(pal::PANEL)
                .stroke(Stroke::new(1.5, pal::GOLD))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(12.0),
        )
        .show(ctx, |ui| {
            ui.set_width(content_w);
            egui::ScrollArea::vertical()
                .max_height(scroll_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_width(content_w);
                    // At a chapter boundary the Campaign cursor already
                    // points at the next encounter. The reward belongs to the
                    // commander that just finished (`g.wave`).
                    let chapter = crate::game::campaign::encounter_id(g.wave.max(1) as u16).chapter;
                    ui.label(RichText::new("Commander defeated").strong().size(20.0).color(pal::GOLD));
                    ui.label(
                        RichText::new(if g.is_campaign() {
                            format!("Campaign C{chapter} E{} reinforcement", g.wave)
                        } else {
                            format!("Wave {} command reinforcement", g.wave)
                        })
                        .size(10.5)
                        .color(pal::DIM),
                    );
                    ui.add_space(8.0);
                    for doctrine in Doctrine::ALL {
                        let rank = g.doctrine_rank(doctrine);
                        let accent = match doctrine {
                            Doctrine::Arsenal => Color32::from_rgb(239, 116, 72),
                            Doctrine::Overdrive => pal::ACC,
                            Doctrine::Reach => pal::GOOD,
                        };
                        let (increment, role) = match doctrine {
                            Doctrine::Arsenal => ("+12% damage", "More damage per hit"),
                            Doctrine::Overdrive => ("+10% attack speed", "More attacks and on-hit chances"),
                            Doctrine::Reach => ("+0.6 range", "Wider coverage and aura reach"),
                        };
                        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::click());
                        ust.command_rects.push(("doctrine_choice", rect));
                        let painter = ui.painter();
                        let active = response.hovered() || response.has_focus();
                        painter.rect_filled(
                            rect,
                            CornerRadius::same(7),
                            if active { Color32::from_rgb(36, 48, 56) } else { pal::CARD },
                        );
                        painter.rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.25, accent), StrokeKind::Inside);
                        painter.text(rect.left_top() + vec2(10.0, 7.0), Align2::LEFT_TOP, doctrine.label(), FontId::proportional(13.0), pal::INK);
                        painter.text(rect.left_top() + vec2(10.0, 24.0), Align2::LEFT_TOP, increment, FontId::monospace(10.0), accent);
                        let role_font = FontId::proportional(9.0);
                        painter.text(
                            rect.left_bottom() + vec2(10.0, -7.0),
                            Align2::LEFT_BOTTOM,
                            elide(ui, role, &role_font, (rect.width() - 20.0).max(20.0)),
                            role_font,
                            pal::DIM,
                        );
                        if rect.width() >= 270.0 {
                            painter.text(rect.right_top() + vec2(-10.0, 9.0), Align2::RIGHT_TOP, format!("R{rank} -> R{}", rank + 1), FontId::monospace(10.0), pal::GOLD);
                        }
                        if response.clicked() { picked = Some(doctrine); }
                        ui.add_space(5.0);
                    }
                    ui.label(RichText::new("Choose the reinforcement your defence needs.").size(10.5).color(pal::DIM));
                });
        });
    ust.command_rects.push(("doctrine_modal", modal_resp.response.rect));
    if let Some(doctrine) = picked {
        g.choose_doctrine(doctrine);
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
                    RichText::new(format!(
                        "{}  |  COMMAND RATING {}  |  SCORE {}",
                        g.difficulty.label().to_ascii_uppercase(),
                        g.command_rating(),
                        gold_str(g.command_score() as i64)
                    ))
                    .monospace()
                    .strong()
                    .size(12.0)
                    .color(pal::GOLD),
                );
                ui.label(
                    RichText::new(if won {
                        if g.is_campaign() {
                            "All 600 Campaign encounters cleared; the Circle Tyrant objective is gone."
                                .to_owned()
                        } else {
                            format!("All {N_WAVES} waves cleared, and the ring emptied.")
                        }
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
                            ("Rush gold", gold_str(g.stats.rush_gold as i64)),
                            ("Clean sweeps", g.stats.clean_sweeps.to_string()),
                            ("Net worth", gold_str(g.net_worth())),
                            ("Towers built", g.stats.towers_built.to_string()),
                            ("Command upgrades", g.doctrine_picks().to_string()),
                            // Not "leaked" - nothing can leak off a circuit,
                            // and a stat that is always zero is worse than no
                            // stat at all. How full the ring ever got is the
                            // number that describes the run.
                            (
                                "Fullest ring",
                                format!("{} / {}", g.stats.peak_circling, g.flood_limit()),
                            ),
                        ] {
                            ui.label(RichText::new(k).color(pal::DIM));
                            ui.label(RichText::new(v).monospace());
                            ui.end_row();
                        }
                    });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if won && !g.is_campaign() {
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
                            egui::Button::new(
                                RichText::new(format!("Play {} again", g.difficulty.label()))
                                    .strong()
                                    .size(14.0),
                            )
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
            ui.label(
                RichText::new("The essentials first; details appear in tower and wave tooltips.")
                    .size(12.0)
                    .color(pal::DIM),
            );
            ui.add_space(6.0);
            for (title, body) in [
                ("Win condition", "Enemies circle forever. Keep CIRCLING below the mode's limit, clear all 36 waves, then choose whether to continue into endless."),
                ("Read both lanes", "Each enemy chooses clockwise or counter-clockwise at the source. Build for coverage on both sides of the circuit instead of making one kill box."),
                ("Build and upgrade", "Choose a tower below and place it on a highlighted fortified pad. Pads protect each tower's footprint; shallow tower spam falls behind, so select towers to upgrade or sell. The 10-gold Single tower branches into six specialist families."),
                ("Read the next-wave warning", "Flying waves need anti-air. Immune waves need Chaos or Hero damage. The HUD warns you when your current board has no answer."),
                ("Choose your tempo", "After the current stream has fully deployed, call the next wave early for a Rush bonus and overlap its survivors. Or let the clock expire with an empty ring for a Clean Sweep bonus."),
                ("Use area damage", "Large waves are a throughput test. Splash, bounce and multishot clear streams; slow and poison buy them more time on the road."),
                ("Command upgrades", "Veteran and Nightmare pause before waves 10, 20 and 30. Choose permanent damage, attack-speed or range upgrades that reinforce your current build."),
                ("Veterans and Vanguards", "Survivors gain speed, but never extra bounty, on completed laps. Gold-ringed Vanguards are tougher, faster and control-resistant. A commander reaching lap 4 ends Veteran; lap 3 ends Nightmare."),
            ] {
                ui.label(RichText::new(title).strong().size(13.0));
                ui.label(RichText::new(body).size(12.0).color(pal::DIM));
                ui.add_space(6.0);
            }
            ui.separator();
            ui.label(
                RichText::new("1-9, 0, - pick tower · click clear grass to keep building · Esc/right-click cancel · Space pause · F speed · Enter start/rush wave · U upgrade · S sell · wheel/pinch zoom · middle-drag or touch-drag pan · R reset view")
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

fn threat_intel_modal(g: &Game, ctx: &Context, ust: &mut UiState) {
    let upcoming = if g.is_campaign() {
        g.upcoming_wave_number()
    } else if g.endless {
        g.wave + 1
    } else {
        (g.wave + 1).min(N_WAVES)
    };
    let elite_stride = g.difficulty.elite_stride(upcoming);
    let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));

    let screen = ctx.content_rect();
    // Outer bounds: at most viewport width minus 24px and height minus 32px
    let max_outer_w = (screen.width() - 24.0).min(420.0).max(240.0);
    let max_outer_h = (screen.height() - 32.0).min(640.0).max(240.0);

    // Subtract inner margin (14 * 2 = 28px) and stroke (1.5 * 2 = 3px)
    let margin_x = 14.0f32;
    let margin_y = 16.0f32;
    let stroke_w = 1.5f32;
    let content_w = (max_outer_w - (margin_x * 2.0 + stroke_w * 2.0)).max(180.0);
    let content_h = (max_outer_h - (margin_y * 2.0 + stroke_w * 2.0)).max(140.0);

    let modal_resp = egui::Modal::new(Id::new("threat_intel_modal"))
        .frame(
            egui::Frame::NONE
                .fill(pal::PANEL)
                .stroke(Stroke::new(stroke_w, pal::GOLD))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::symmetric(margin_x as i8, margin_y as i8)),
        )
        .show(ctx, |ui| {
            ui.set_width(content_w);
            ui.set_max_height(content_h);
            egui::ScrollArea::vertical()
                .max_height((content_h - 60.0).max(100.0))
                .show(ui, |ui| {
                    render_encounter_threat_tooltip(ui, g, upcoming, elite_stride);
                });
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                let btn = egui::Button::new(RichText::new("CLOSE [X]").strong().size(12.0))
                    .fill(Color32::from_rgb(43, 110, 190))
                    .min_size(vec2(130.0, 30.0));
                let resp = ui.add(btn);
                ust.command_rects.push(("intel_close", resp.rect));
                if resp.clicked() {
                    close = true;
                }
            });
        });

    let actual_rect = modal_resp.response.rect;
    ust.threat_intel_rect = Some(actual_rect);
    ust.command_rects.push(("intel_modal", actual_rect));

    if close || modal_resp.should_close() {
        ust.show_threat_intel = false;
        ust.threat_intel_rect = None;
    }
}

// ---------------------------------------------------------------- board overlay

/// Floating damage numbers and the transient toast, drawn over the 3D canvas.
pub fn board_text(g: &Game, ui: &Ui, cam: &Camera, rect: Rect, show_guidance: bool) {
    let p = ui.painter_at(rect);
    // Floating text is an information channel, not a log. A late-game horde
    // can create dozens of hits in one frame; rendering every queued label
    // made identical red numbers overlap into an opaque pile. Reserve visible
    // cells for decisive events first and allow ordinary damage only where it
    // has a free screen-space cell.
    const CELL: f32 = 34.0;
    let limit = if rect.width() < 620.0 { 7 } else { 12 };
    let mut occupied: Vec<(i32, i32)> = Vec::with_capacity(limit);
    let mut drawn = 0usize;
    for kind in [
        crate::game::TextKind::Crit,
        crate::game::TextKind::Gold,
        crate::game::TextKind::Damage,
    ] {
        for t in g.texts.iter().filter(|t| t.kind == kind) {
            if drawn >= limit {
                break;
            }
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
        let cell = (
            ((pos.x - rect.left()) / CELL).floor() as i32,
            ((pos.y - rect.top()) / CELL).floor() as i32,
        );
        if occupied.iter().any(|&(x, y)| (x - cell.0).abs() <= 1 && (y - cell.1).abs() <= 1) {
            continue;
        }
        occupied.push(cell);
        drawn += 1;
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
        p.text(pos, Align2::CENTER_CENTER, txt, font, col.gamma_multiply(a));
        }
    }

    if let Some((msg, t, tone)) = &g.toast {
        let a = (t / 2.2).clamp(0.0, 1.0);
        let color = match tone {
            ToastTone::Info => pal::ACC,
            ToastTone::Good => pal::GOOD,
            ToastTone::Bad => pal::BAD,
        };
        p.text(
            pos2(rect.center().x, rect.top() + 30.0),
            Align2::CENTER_CENTER,
            msg,
            FontId::proportional(15.0),
            color.gamma_multiply(a),
        );
    }

    // Desktop has a dedicated rail for placement and counter instructions.
    // Keeping this board overlay for compact screens only stops it hiding the
    // upper lane and the horde under a 430px instruction card.
    if !show_guidance {
        return;
    }

    // Progressive, contextual guidance. It disappears as soon as the action is
    // understood and returns only for a strategically dangerous counter. This
    // replaces a wall of instructions with the one decision that matters now.
    let guidance = if let Some((def_i, _)) = g.build_choice {
        let def = &TOWERS[def_i];
        let placement_help = if rect.width() < 620.0 {
            format!("Tap clear grass · CANCEL stops · {}", def.targets.label())
        } else {
            format!("Click clear grass. Keep placing until Esc/right-click · {}", def.targets.label())
        };
        Some((
            format!("PLACE {} — {}g", def.name, gold_str(def.gold as i64)),
            placement_help,
            pal::ACC,
        ))
    } else if g.wave == 0 && g.towers.is_empty() {
        Some((
            "STEP 1 OF 3  ·  CHOOSE A TOWER".to_owned(),
            "Pick a tower below. Single is flexible; Siege controls crowds.".to_owned(),
            pal::ACC,
        ))
    } else if g.wave == 0 && !g.towers.is_empty() {
        Some((
            "STEP 3 OF 3  ·  CALL WAVE 1".to_owned(),
            "Press Enter or START WAVE when your opening is ready. There is no countdown."
                .to_owned(),
            pal::GOOD,
        ))
    } else if g.flood() > 0.72 {
        let detail = if g.is_campaign() {
            format!(
                "Pressure {:.0}/{:.0} · {} bodies. Upgrade or add crowd control.",
                g.campaign_pressure(),
                g.campaign_pressure_capacity(),
                g.creeps.len(),
            )
        } else {
            format!("{} of {} enemies are circling. Upgrade or add crowd control.", g.creeps.len(), g.flood_limit())
        };
        Some((
            "DANGER  ·  THE RING IS FILLING".to_owned(),
            detail,
            pal::BAD,
        ))
    } else if g.wave < g.last_wave() {
        let next = g.next_wave_def();
        let has_air = g.towers.iter().any(|t| t.targets().can_hit(true));
        let has_chaos = g
            .towers
            .iter()
            .any(|t| matches!(t.attack(), Attack::Chaos | Attack::Hero));
        if next.flying && !has_air {
            Some((
                format!("COUNTER NEEDED  ·  AIR ON WAVE {}", g.wave + 1),
                "Build an Air Tower, or one whose target line says Ground + Air.".to_owned(),
                pal::BAD,
            ))
        } else if next.armour_type == ArmourType::Divine && !has_chaos {
            Some((
                format!("COUNTER NEEDED  ·  IMMUNE ON WAVE {}", g.wave + 1),
                "Build Chaos or Hero damage; other attacks deal almost nothing.".to_owned(),
                pal::BAD,
            ))
        } else if g.creeps.iter().map(|c| c.laps).max().unwrap_or(0) >= 2 {
            let veterans = g.creeps.iter().filter(|c| c.laps > 0).count();
            Some((
                "VETERANS ARE GAINING SPEED".to_owned(),
                format!(
                    "{veterans} survivors have completed a lap. Default First targeting prioritises them."
                ),
                pal::GOLD,
            ))
        } else {
            None
        }
    } else {
        None
    };

    if let Some((title, detail, accent)) = guidance {
        let width = (rect.width() - 32.0).min(430.0).max(220.0);
        let card = Rect::from_min_size(rect.left_top() + vec2(16.0, 16.0), vec2(width, 58.0));
        p.rect_filled(
            card,
            CornerRadius::same(7),
            Color32::from_rgba_unmultiplied(10, 17, 16, 226),
        );
        p.rect_stroke(
            card,
            CornerRadius::same(7),
            Stroke::new(1.5, accent.gamma_multiply(0.9)),
            StrokeKind::Inside,
        );
        p.text(
            card.left_top() + vec2(12.0, 9.0),
            Align2::LEFT_TOP,
            title,
            FontId::monospace(11.0),
            accent,
        );
        p.text(
            card.left_top() + vec2(12.0, 31.0),
            Align2::LEFT_TOP,
            elide(ui, &detail, &FontId::proportional(11.5), card.width() - 24.0),
            FontId::proportional(11.5),
            pal::INK,
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
    let Some(ti) = g.tower_at(w) else {
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
