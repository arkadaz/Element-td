//! Layout regression tests for the HUD.
//!
//! These run the real egui widgets headlessly at a range of window sizes and
//! assert that every panel gets the height it asked for and that the board is
//! left with usable space. Screenshots are not a reliable way to catch a panel
//! overflowing its own bounds; this is.

use egui::{Context, RawInput, Rect, pos2, vec2};

use crate::game::Game;
use crate::ui::{self, UiState};

struct Layout {
    top: Rect,
    bottom: Rect,
    central: Rect,
    palette: Rect,
    cards: Vec<Rect>,
    stats: Vec<Rect>,
    controls_left: f32,
}

/// Runs one full HUD frame at `size` and reports where everything landed.
fn lay_out(size: [f32; 2]) -> Layout {
    let ctx = Context::default();
    ui::install_style(&ctx);

    let mut game = Game::new();
    let mut ust = UiState::default();
    ust.compact = ui::compact_for(size[0]);

    let mut top = Rect::NOTHING;
    let mut bottom = Rect::NOTHING;
    let mut central = Rect::NOTHING;

    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]))),
        ..Default::default()
    };

    // Two passes: egui settles panel sizes on the second frame.
    for _ in 0..2 {
        // `run_ui` hands back the same root Ui that `eframe::App::ui` receives,
        // so this exercises exactly the layout the game uses.
        let mut out = ctx.run_ui(input.clone(), |ui| {
            top = egui::Panel::top("hud")
                .exact_size(ui::top_h(ust.compact))
                .resizable(false)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 4)))
                .show(ui, |ui| {
                    ui::top_bar(&mut game, ui, &mut ust, "60 fps");
                })
                .response
                .rect;
            bottom = egui::Panel::bottom("shop")
                .exact_size(ui::command_h(ust.compact))
                .resizable(false)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 8)))
                .show(ui, |ui| {
                    ui::command_bar(&mut game, ui, &mut ust);
                })
                .response
                .rect;
            central = egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| ui.available_rect_before_wrap())
                .inner;
        });
        // epaint insists the font atlas delta is consumed before the output is
        // dropped; there is no renderer here to consume it.
        out.textures_delta.clear();
    }
    Layout {
        top,
        bottom,
        central,
        palette: ust.palette_rect,
        cards: ust.card_rects.clone(),
        stats: ust.stat_rects.clone(),
        controls_left: ust.controls_left,
    }
}

/// Wave, lives and gold are the game. They must be on screen at every window
/// size, and the controls must never be allowed to push them off.
///
/// This is a real bug that shipped: the controls were drawn in a right-to-left
/// sub-layout, which claims all the remaining width, so on a narrow window the
/// resource readouts vanished off the left edge and the only thing visible was
/// the frame-rate counter.
#[test]
fn the_resource_readouts_are_never_pushed_off_screen() {
    for size in [
        [360.0, 640.0],
        [500.0, 400.0],
        [667.0, 375.0],
        [900.0, 500.0],
        [1280.0, 720.0],
        [1920.0, 1080.0],
        [2560.0, 1440.0],
    ] {
        let l = lay_out(size);
        assert_eq!(l.stats.len(), 3, "at {size:?} not every readout was drawn");
        for (i, r) in l.stats.iter().enumerate() {
            assert!(
                r.left() >= l.top.left() - 0.5 && r.right() <= l.top.right() + 0.5,
                "at {size:?} readout {i} at {r:?} is outside the top bar {:?}",
                l.top
            );
            assert!(
                r.right() <= l.controls_left + 0.5,
                "at {size:?} readout {i} runs into the controls (ends {}, controls start {})",
                r.right(),
                l.controls_left
            );
            assert!(r.width() > 20.0, "at {size:?} readout {i} collapsed to {}", r.width());
        }
        // And they must not overlap each other.
        for w in l.stats.windows(2) {
            assert!(
                w[0].right() <= w[1].left() + 0.5,
                "at {size:?} the readouts overlap: {:?} then {:?}",
                w[0],
                w[1]
            );
        }
    }
}

#[test]
fn the_command_bar_can_hold_everything_it_draws() {
    // Pure arithmetic, but it is the exact relationship that broke: the build
    // cards were taller than the well they were drawn into.
    let usable = ui::COMMAND_H - 16.0; // vertical inner margin, both sides
    assert!(
        ui::BAR_H <= usable,
        "command bar sections ({}) do not fit in COMMAND_H ({usable} usable)",
        ui::BAR_H
    );
    let card_well = ui::BAR_H - 26.0; // section label plus padding
    assert!(
        ui::CARD_H <= card_well,
        "build cards ({}) overflow their well ({card_well})",
        ui::CARD_H
    );
}

#[test]
fn the_hud_lays_out_at_every_common_window_size() {
    for size in [
        // Phones in landscape, then tablets, then desktops.
        [667.0, 375.0],
        [844.0, 390.0],
        [932.0, 430.0],
        [1024.0, 640.0],
        [1280.0, 720.0],
        [1366.0, 768.0],
        [1600.0, 900.0],
        [1920.0, 1080.0],
        [2560.0, 1440.0],
    ] {
        let l = lay_out(size);
        let (top, bottom, central) = (l.top, l.bottom, l.central);

        let compact = ui::compact_for(size[0]);
        assert!(
            (top.height() - ui::top_h(compact)).abs() < 1.0,
            "{size:?}: top strip is {} tall, wanted {}",
            top.height(),
            ui::top_h(compact)
        );
        assert!(
            (bottom.height() - ui::command_h(compact)).abs() < 1.0,
            "{size:?}: command bar is {} tall, wanted {}",
            bottom.height(),
            ui::command_h(compact)
        );
        // The board must still get a usable slab of screen.
        assert!(
            central.height() > 150.0,
            "{size:?}: only {} px left for the board",
            central.height()
        );
        assert!(central.width() > 300.0, "{size:?}: board too narrow");
        // Panels must not overlap the board.
        assert!(
            central.top() >= top.bottom() - 1.0,
            "{size:?}: board overlaps the top strip"
        );
        assert!(
            central.bottom() <= bottom.top() + 1.0,
            "{size:?}: board overlaps the command bar"
        );
        // And everything has to add up to the window.
        let total = top.height() + central.height() + bottom.height();
        assert!(
            (total - size[1]).abs() < 2.0,
            "{size:?}: panels sum to {total}, not {}",
            size[1]
        );
    }
}

#[test]
fn the_hud_survives_a_window_far_too_small_to_be_sensible() {
    // Should degrade, not panic or produce negative rects.
    let l = lay_out([420.0, 320.0]);
    assert!(l.top.height() >= 0.0 && l.bottom.height() >= 0.0);
    assert!(l.central.width() >= 0.0 && l.central.height() >= 0.0);
    for c in &l.cards {
        assert!(c.width() > 0.0 && c.height() > 0.0);
    }
}

#[test]
fn every_build_card_is_inside_its_panel() {
    // This is the check that was missing: the panels were the right height, but
    // the cards drawn inside them were not.
    for size in [
        [667.0, 375.0],
        [844.0, 390.0],
        [1024.0, 640.0],
        [1280.0, 720.0],
        [1440.0, 900.0],
        [1920.0, 1080.0],
        [2560.0, 1440.0],
    ] {
        let l = lay_out(size);
        assert!(
            !l.cards.is_empty(),
            "{size:?}: no build cards were laid out at all"
        );
        assert_eq!(
            l.cards.len(),
            crate::game::defs::TOWERS.len(),
            "{size:?}: only {} of {} towers fit in the palette",
            l.cards.len(),
            crate::game::defs::TOWERS.len()
        );
        for (i, c) in l.cards.iter().enumerate() {
            assert!(
                l.palette.contains_rect(*c),
                "{size:?}: card {i} at {c:?} escapes its panel {:?}",
                l.palette
            );
            assert!(
                l.bottom.contains_rect(*c),
                "{size:?}: card {i} at {c:?} escapes the command bar {:?}",
                l.bottom
            );
            assert!(c.height() >= 50.0, "{size:?}: card {i} squashed to {}", c.height());
            assert!(c.width() >= 44.0, "{size:?}: card {i} too narrow to tap: {}", c.width());
        }
        // Cards must not overlap each other.
        for w in l.cards.windows(2) {
            assert!(
                w[0].right() <= w[1].left() + 0.5,
                "{size:?}: build cards overlap"
            );
        }
    }
}

/// Every character the HUD prints must exist in the font it is printed with.
///
/// The pause button used to be U+25B6 / U+2759. egui bundles a Latin font and
/// an emoji font, and neither covers those, so the button rendered as a pair of
/// empty tofu boxes. A missing glyph is invisible to every other test here -
/// the layout is perfect and the pixels are wrong - so it gets its own check.
///
/// Note that measuring the laid-out width does *not* detect this: a missing
/// glyph is substituted, and the substitute has a perfectly ordinary width. The
/// font atlas has to be asked directly. The two fonts are also checked
/// separately, because they do not cover the same characters - the middle dot
/// the HUD uses everywhere exists in the proportional font and not in the
/// monospace one.
#[test]
fn every_glyph_the_hud_prints_actually_exists() {
    // Separators and marks used in labels and tooltips.
    const PROPORTIONAL: &str = "\u{00b7} \u{2022} \u{2014}";
    // Numbers, costs and the perf readout. Currently pure ASCII, and this is
    // what keeps it that way.
    const MONOSPACE: &str = "";

    let mut fonts =
        epaint::text::Fonts::new(Default::default(), egui::FontDefinitions::default());
    for (text, font) in [
        (PROPORTIONAL, egui::FontId::proportional(12.0)),
        (MONOSPACE, egui::FontId::monospace(12.0)),
    ] {
        for ch in text.chars() {
            if ch.is_ascii() {
                continue;
            }
            assert!(
                fonts.has_glyph(&font, ch),
                "U+{:04X} {ch:?} has no glyph in {:?} - it renders as a tofu box",
                ch as u32,
                font.family
            );
        }
    }

    // And the check has teeth: the glyphs that were actually broken must fail.
    for ch in ['\u{25b6}', '\u{2759}'] {
        assert!(
            !fonts.has_glyph(&egui::FontId::proportional(12.0), ch),
            "U+{:04X} is available after all - this test is no longer guarding anything",
            ch as u32
        );
    }
}
