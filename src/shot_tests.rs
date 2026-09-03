//! Plays a run and captures the board at chosen waves.
//!
//! Run it deliberately:
//!     cargo test --release capture_a_playthrough -- --ignored --nocapture
//!
//! It writes PNGs beside the repo and prints where. Nothing here touches the
//! screen: the frames come out of the game's own renderer on a headless device.
#![cfg(all(test, not(target_arch = "wasm32")))]

use std::path::PathBuf;

use crate::decor::Decor;
use crate::game::defs::*;
use crate::game::{FLOOD_LIMIT, Game, Phase, WAVE_PERIOD};
use crate::gfx::Quality;
use crate::shot;

/// Waves to photograph. Chosen to show the arc: an opening board, the roster
/// opening up, the ring starting to fill, and the endgame under real pressure.
const AT: [u32; 4] = [3, 25, 55, 68];

const W: u32 = 1280;
const H: u32 = 720;

fn out_dir() -> PathBuf {
    let dir = std::env::var("TD_SHOT_DIR").unwrap_or_else(|_| "shots".to_string());
    let p = PathBuf::from(dir);
    std::fs::create_dir_all(&p).expect("could not create the shot directory");
    p
}

#[test]
#[ignore = "renders PNGs; run it deliberately"]
fn capture_a_playthrough() {
    let dir = out_dir();
    let mut g = Game::new();
    g.start_run(0x5CA1_AB1E);
    let decor = Decor::build(&g.board);
    let mut built = 0usize;
    let mut shots = 0;

    println!();
    for target in AT {
        // Play forward to the wave we want a picture of.
        while g.wave < target && !matches!(g.phase, Phase::Defeat | Phase::Victory) {
            while g.pending_draft.is_some() {
                super::game::tests::draft_for_shot(&mut g);
            }
            super::game::tests::spend_for_shot(&mut g, &mut built);
            let dt = 1.0 / 60.0;
            let mut t = 0.0;
            let was = g.wave;
            while g.wave == was && t < WAVE_PERIOD * 3.0 {
                g.update(dt);
                t += dt;
                if matches!(g.phase, Phase::Defeat | Phase::Victory) {
                    break;
                }
            }
        }
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            println!("  run ended on wave {} before wave {target}", g.wave);
            break;
        }
        // Let the wave get going so the picture has monsters in it.
        for _ in 0..(WAVE_PERIOD * 0.55 * 60.0) as u32 {
            g.update(1.0 / 60.0);
        }

        let shot = shot::capture(&g, &decor, W, H, Quality::Ultra);
        let path = dir.join(format!("wave{:02}.png", g.wave));
        shot::write_png(&path, &shot).expect("could not write the PNG");
        shots += 1;
        println!(
            "  wave {:>2}  {:>3} towers  {:>3}/{FLOOD_LIMIT} circling  ->  {}",
            g.wave,
            g.towers.len(),
            g.creeps.len(),
            path.display()
        );
    }
    assert!(shots > 0, "captured nothing");
}

/// The frame is not black, and it is not one flat colour.
///
/// A headless capture that silently produces a black rectangle is the classic
/// failure of this whole approach - it is what window-grabbing a GPU swapchain
/// usually gives you - so it gets an assertion rather than an eyeball.
#[test]
fn a_captured_frame_actually_has_a_board_in_it() {
    let mut g = Game::new();
    g.start_run(7);
    g.pending_draft = None;
    g.essence = [(MAX_TIER - FREE_TIERS) as u8; 6];
    g.gold = 500_000;
    let mut n = 0;
    for slot in 0..g.board.slots.len() {
        g.build_choice = Some((n % TOWERS.len(), 4));
        if g.try_build(slot) {
            n += 1;
        }
        if n >= 24 {
            break;
        }
    }
    g.build_choice = None;
    g.selected = None;
    let decor = Decor::build(&g.board);

    let s = shot::capture(&g, &decor, 480, 270, Quality::Balanced);
    assert_eq!(s.rgba.len(), 480 * 270 * 4);

    let lit = s
        .rgba
        .chunks(4)
        .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
        .count();
    let total = 480 * 270;
    assert!(
        lit * 20 > total,
        "only {lit} of {total} pixels have any light in them - the capture came back black"
    );

    // And it is a scene, not a fill: several distinct greys at least.
    let mut buckets = [0u32; 16];
    for p in s.rgba.chunks(4) {
        let lum = (p[0] as u32 * 2 + p[1] as u32 * 5 + p[2] as u32) / 8;
        buckets[(lum / 16).min(15) as usize] += 1;
    }
    let used = buckets.iter().filter(|&&c| c * 500 > total as u32).count();
    assert!(
        used >= 3,
        "the frame only uses {used} brightness bands - that is a flat fill"
    );
}

/// Which static geometry is brightest, and where.
///
/// The frame around the board renders as blinding white while every stone
/// colour in `theme` is dark, so the albedo and the thing on screen disagree.
/// Guessing which of eight hundred lines of scenery it is wastes more time than
/// printing it.
#[test]
#[ignore = "diagnostic"]
fn what_is_the_brightest_thing_on_the_board() {
    let g = Game::new();
    let decor = Decor::build(&g.board);
    let statics = crate::view::build_static(&g, &decor);

    let mut rows: Vec<(f32, String)> = Vec::new();
    for (name, list) in [("flat", &statics.flat), ("casters", &statics.casters)] {
        for (shape, bucket) in list.solid.iter().enumerate() {
            for inst in bucket {
                let lum = inst.color[0] * 0.3 + inst.color[1] * 0.6 + inst.color[2] * 0.1;
                rows.push((
                    lum,
                    format!(
                        "{name:>7} shape {shape} at [{:>6.1},{:>6.1},{:>5.2}] size [{:>5.2},{:>5.2},{:>5.2}] \
                         col [{:.2},{:.2},{:.2}] em {:.2}",
                        inst.pos[0], inst.pos[1], inst.pos[2],
                        inst.scale[0], inst.scale[1], inst.scale[2],
                        inst.color[0], inst.color[1], inst.color[2],
                        inst.params[0]
                    ),
                ));
            }
        }
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!();
    println!("brightest static geometry ({} instances):", rows.len());
    for (lum, line) in rows.iter().take(14) {
        println!("  lum {lum:.3}  {line}");
    }
}

/// What is actually in the frame at a busy moment.
///
/// The wave-55 capture is criss-crossed with big pale rings that obscure the
/// circuit, and there are several things in the renderer that draw a ring.
/// Counting them is quicker than reading all of them.
#[test]
#[ignore = "diagnostic"]
fn what_is_drawing_all_those_rings() {
    let mut g = Game::new();
    g.start_run(0x5CA1_AB1E);
    let decor = Decor::build(&g.board);
    let mut built = 0usize;
    while g.wave < 55 && !matches!(g.phase, Phase::Defeat | Phase::Victory) {
        while g.pending_draft.is_some() {
            super::game::tests::draft_for_shot(&mut g);
        }
        super::game::tests::spend_for_shot(&mut g, &mut built);
        let was = g.wave;
        let mut t = 0.0;
        while g.wave == was && t < WAVE_PERIOD * 3.0 {
            g.update(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
    }
    for _ in 0..(WAVE_PERIOD * 0.55 * 60.0) as u32 {
        g.update(1.0 / 60.0);
    }

    println!();
    println!(
        "wave {}: {} creeps, {} beams, {} zones, {} projectiles, {} towers",
        g.wave,
        g.creeps.len(),
        g.beams.len(),
        g.zones.len(),
        g.projs.len(),
        g.towers.len()
    );
    let ground_beams = g.beams.iter().filter(|b| b.width <= 0.0).count();
    println!("  of the beams, {ground_beams} are ground shockwaves (width 0)");
    let mut by_kind: Vec<(&str, usize)> = Vec::new();
    for t in &g.towers {
        let n = by_kind.iter().position(|(k, _)| *k == t.def().name);
        match n {
            Some(i) => by_kind[i].1 += 1,
            None => by_kind.push((t.def().name, 1)),
        }
    }
    by_kind.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("  board: {by_kind:?}");

    let mut list = crate::gfx::draw::DrawList::default();
    crate::view::draw_scene(&g, &decor, &mut list, g.time);
    println!(
        "  draw list: {} solids, {} glows",
        list.solid_count(),
        list.glow.len()
    );
    for (shape, bucket) in list.solid.iter().enumerate() {
        if !bucket.is_empty() {
            println!("    shape {shape}: {} instances", bucket.len());
        }
    }
}
