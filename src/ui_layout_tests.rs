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
    /// Which tower each laid-out card is for, so a paged palette can be checked
    /// for covering the whole roster rather than only for fitting on screen.
    hotkeys: Vec<usize>,
    stats: Vec<Rect>,
    controls_left: f32,
}

/// Runs one full HUD frame at `size` and reports where everything landed.
fn lay_out(size: [f32; 2]) -> Layout {
    lay_out_page(size, 0)
}

/// Runs one full HUD frame at `size`, showing page `page` of the build palette.
fn lay_out_page(size: [f32; 2], page: usize) -> Layout {
    let ctx = Context::default();
    ui::install_style(&ctx);

    let mut game = Game::new();
    let mut ust = UiState::default();
    ust.compact = ui::compact_for_view(size[0], size[1]);
    ust.palette_page = page;

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
        hotkeys: ust.hotkeys.clone(),
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
        [390.0, 844.0],
        [500.0, 400.0],
        [667.0, 375.0],
        [844.0, 390.0],
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
            assert!(
                r.width() > 20.0,
                "at {size:?} readout {i} collapsed to {}",
                r.width()
            );
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
        [390.0, 844.0],
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

        let compact = ui::compact_for_view(size[0], size[1]);
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
        [390.0, 844.0],
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

        // Twenty-one towers do not fit across a phone at a size worth tapping,
        // so the palette pages. What must hold is that every tower is reachable
        // on *some* page - a card that is merely dropped is a tower that cannot
        // be built at all.
        let total = crate::game::defs::shop_order().len();
        let mut reached: Vec<usize> = Vec::new();
        for page in 0..total {
            let l = lay_out_page(size, page);
            if page > 0 && l.hotkeys.first() == reached.first() {
                break; // wrapped around
            }
            for &i in &l.hotkeys {
                if !reached.contains(&i) {
                    reached.push(i);
                }
            }
            if reached.len() == total {
                break;
            }
        }
        reached.sort_unstable();
        assert_eq!(
            reached.len(),
            total,
            "{size:?}: only {} of {total} towers are reachable across every page",
            reached.len()
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
            assert!(
                c.height() >= 50.0,
                "{size:?}: card {i} squashed to {}",
                c.height()
            );
            assert!(
                c.width() >= 44.0,
                "{size:?}: card {i} too narrow to tap: {}",
                c.width()
            );
        }
        // Cards must not overlap each other. Desktop uses a 4x3 command grid,
        // while compact screens keep the paged strip, so check geometry rather
        // than assuming every later card is to the right of the previous one.
        for i in 0..l.cards.len() {
            for j in i + 1..l.cards.len() {
                assert!(
                    !l.cards[i].intersects(l.cards[j]),
                    "{size:?}: build cards {i} and {j} overlap"
                );
            }
        }
    }
}

#[test]
fn desktop_commands_form_a_four_by_three_grid() {
    let l = lay_out([1280.0, 720.0]);
    assert_eq!(
        l.cards.len(),
        11,
        "every base tower should be visible at once"
    );
    let near = |a: f32, b: f32| (a - b).abs() < 0.5;
    for row in 0..3 {
        let first = row * 4;
        let end = (first + 4).min(l.cards.len());
        for card in &l.cards[first..end] {
            assert!(near(card.top(), l.cards[first].top()));
        }
    }
    assert!(near(l.cards[0].left(), l.cards[4].left()));
    assert!(near(l.cards[4].left(), l.cards[8].left()));
    assert!(l.cards[4].top() > l.cards[0].bottom());
    assert!(l.cards[8].top() > l.cards[4].bottom());
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

    let mut fonts = epaint::text::Fonts::new(Default::default(), egui::FontDefinitions::default());
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

/// Text that does not fit its box is shortened, never clipped.
///
/// A centred string wider than its container loses characters off *both* ends,
/// so "Stacking poison" rendered as "tacking poiso" - which reads as a word
/// rather than as truncation, and is therefore worse than useless. The build
/// palette shrinks its cards to fit however many towers the draft has unlocked,
/// so this is not a hypothetical.
#[test]
fn labels_too_wide_for_their_box_are_shortened_not_cut() {
    let ctx = Context::default();
    ui::install_style(&ctx);
    let mut out = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
            ..Default::default()
        },
        |ui| {
            let font = egui::FontId::proportional(9.0);
            let width = |s: &str| {
                ui.painter()
                    .layout_no_wrap(s.to_owned(), font.clone(), egui::Color32::WHITE)
                    .rect
                    .width()
            };

            // Every name on every build card, at the narrowest a card can get.
            const MIN_CARD: f32 = 46.0 - 6.0;
            for d in crate::game::defs::TOWERS {
                for label in [d.name, d.family.short(), d.family.role()] {
                    let short = ui::elide(ui, label, &font, MIN_CARD);
                    assert!(
                        width(&short) <= MIN_CARD,
                        "{:?} still {:.1}px wide in a {MIN_CARD}px card",
                        short,
                        width(&short)
                    );
                    assert!(!short.is_empty(), "{label} elided away to nothing");
                }
            }

            // A string that already fits is left exactly alone.
            assert_eq!(ui::elide(ui, "Roots", &font, 400.0), "Roots");
            // And one that does not is marked as shortened.
            let cut = ui::elide(ui, "Stacking poison", &font, 40.0);
            assert!(cut.ends_with(".."), "{cut:?} is not marked as shortened");
            assert!(!cut.starts_with("tacking"), "elide cut from the front");
        },
    );
    out.textures_delta.clear();
}

/// The live Build HUD and the commander reinforcement modal share a frame in
/// production. Check their published hit rectangles rather than repeating the
/// layout constants here: a modal can look centred while its real buttons or
/// the threat card still escape a short viewport.
#[test]
fn build_wave_preview_and_doctrine_choices_fit_real_viewports() {
    for size in [[1440.0, 900.0], [390.0, 844.0], [844.0, 390.0]] {
        let ctx = Context::default();
        ui::install_style(&ctx);
        let mut game = Game::new();
        game.start_campaign(0xD0C7_1000, crate::game::Difficulty::Veteran);
        game.wave = 10;
        game.pending_doctrine = true;
        let mut ust = UiState::default();
        ust.compact = ui::compact_for_view(size[0], size[1]);
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]))),
            ..Default::default()
        };
        for _ in 0..3 {
            let mut out = ctx.run_ui(input.clone(), |ui| {
                egui::Panel::top("hud")
                    .exact_size(ui::top_h(ust.compact))
                    .show(ui, |ui| ui::top_bar(&mut game, ui, &mut ust, "60 fps"));
                ui::modals(&mut game, &ctx, &mut ust);
            });
            out.textures_delta.clear();
        }

        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]));
        let modal = ust.command_rects.iter().find_map(|(name, rect)| (*name == "doctrine_modal").then_some(*rect))
            .unwrap_or_else(|| panic!("{size:?}: doctrine modal did not publish an actual rect"));
        assert!(screen.contains_rect(modal), "{size:?}: modal {modal:?} escapes screen {screen:?}");
        let choices: Vec<_> = ust.command_rects.iter().filter_map(|(name, rect)| (*name == "doctrine_choice").then_some(*rect)).collect();
        assert_eq!(choices.len(), 3, "{size:?}: expected exactly three doctrine choices");
        for (i, choice) in choices.iter().enumerate() {
            assert!(modal.contains_rect(*choice), "{size:?}: choice {i} {choice:?} leaves modal {modal:?}");
            assert!(choice.height() >= 52.0 && choice.height() <= 60.0, "{size:?}: choice {i} is not a compact card: {choice:?}");
        }
        for pair in choices.windows(2) {
            assert!(!pair[0].intersects(pair[1]), "{size:?}: doctrine cards overlap");
        }
        assert!(
            ust.command_rects.iter().any(|(name, rect)| *name == "intel_open" && screen.contains_rect(*rect)),
            "{size:?}: Build threat preview has no reachable on-screen input rect"
        );
    }
}

/// The tooltip consumes combat's rider query, rather than maintaining a
/// second Campaign factor beside its presentation string.
#[test]
fn poison_ability_text_uses_the_active_mode_rider_budget() {
    let poison = crate::game::defs::TOWERS
        .iter()
        .find(|tower| tower.family == crate::game::defs::Family::Poison)
        .expect("Poison tower exists")
        .abil;
    let legacy = Game::new();
    let mut campaign = Game::new();
    campaign.start_campaign(0xA811_1E57, crate::game::Difficulty::Veteran);

    for game in [&legacy, &campaign] {
        let effective = crate::game::combat::effective_poison_dps(game, poison.poison_dps);
        let line = ui::ability_lines(game, &poison)
            .into_iter()
            .find(|line| line.starts_with("Poison:"))
            .expect("Poison ability line");
        assert!(
            line.contains(&format!("Poison: {}/s", effective.round() as u32)),
            "{line:?} did not present combat's {effective}/s rider"
        );
    }
    assert_ne!(
        crate::game::combat::effective_poison_dps(&legacy, poison.poison_dps),
        crate::game::combat::effective_poison_dps(&campaign, poison.poison_dps),
        "the mode-specific check needs distinct Legacy and Campaign budgets"
    );
}
