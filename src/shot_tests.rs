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
use crate::game::{Difficulty, Game, Phase, WAVE_PERIOD};
use crate::gfx::Quality;
use crate::shot;

/// Waves to photograph. Chosen to show the arc: an opening board, the roster
/// opening up, the ring starting to fill, and the endgame under real pressure.
const AT: [u32; 4] = [4, 13, 24, 33];

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
    g.start_run_with_difficulty(0x5CA1_AB1E, Difficulty::Veteran);
    let decor = Decor::build(&g.board);
    let mut built = 0usize;
    let mut shots = 0;

    println!();
    for target in AT {
        // Play forward to the wave we want a picture of.
        while g.wave < target && !matches!(g.phase, Phase::Defeat | Phase::Victory) {
            if g.pending_doctrine {
                let pick = match g.doctrine_picks() {
                    0 | 2 => crate::game::Doctrine::Arsenal,
                    _ => crate::game::Doctrine::Overdrive,
                };
                g.choose_doctrine(pick);
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
        // Let the wave get going so the picture has monsters in it. Nothing is
        // selected: a selected tower paints its range ring across the board,
        // and a screenshot of that is a screenshot of the range ring.
        for _ in 0..(WAVE_PERIOD * 0.55 * 60.0) as u32 {
            g.update(1.0 / 60.0);
        }
        g.selected = None;

        // Land on an active firing frame instead of photographing the quiet
        // gap between two cooldowns. This makes the review capture exercise
        // projectile silhouettes and impact shockwaves as well as towers.
        for _ in 0..120 {
            g.fx.particles.clear();
            g.update(1.0 / 120.0);
            if !g.projs.is_empty() || !g.beams.is_empty() {
                break;
            }
        }

        let shot = shot::capture(&g, &decor, W, H, Quality::Ultra);
        let path = dir.join(format!("wave{:02}.png", g.wave));
        shot::write_png(&path, &shot).expect("could not write the PNG");
        shots += 1;
        println!(
            "  wave {:>2}  {:>3} towers  {:>3}/{} circling  ->  {}",
            g.wave,
            g.towers.len(),
            g.creeps.len(),
            g.flood_limit(),
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
    g.gold = 500_000;
    let mut n = 0;
    for slot in 0..g.board.slots.len() {
        g.build_choice = Some((n % TOWERS.len(), 1));
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
        "wave {}: {} creeps, {} beams, {} projectiles, {} towers",
        g.wave,
        g.creeps.len(),
        g.beams.len(),
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

// ---------------------------------------------------------------- the HUD

/// Captures the game as a player sees it: board and HUD together.
///
/// Run it with:
///     cargo test --release capture_the_interface -- --ignored --nocapture
#[test]
#[ignore = "renders PNGs; run it deliberately"]
fn capture_the_interface() {
    let dir = out_dir();
    println!();

    // 1. The very first thing a run shows: the opening essence draft.
    let mut g = Game::new();
    g.start_run_with_difficulty(0x5CA1_AB1E, Difficulty::Veteran);
    let decor = Decor::build(&g.board);
    save(&dir, "ui_draft", &mut g, &decor);

    // 1b. The premium hard-mode milestone choice. Keep this as an explicit
    // state so the capture never has to play ten waves merely to inspect a
    // modal's spacing and information hierarchy.
    let mut doctrine = Game::new();
    doctrine.start_run_with_difficulty(0x5CA1_AB1E, Difficulty::Veteran);
    doctrine.wave = 10;
    doctrine.pending_doctrine = true;
    doctrine.paused = true;
    let doctrine_decor = Decor::build(&doctrine.board);
    save(&dir, "ui_command_upgrade", &mut doctrine, &doctrine_decor);

    // 2. Mid-game: a full build palette, the essence strip, a live wave.
    let mut built = 0usize;
    while g.wave < 30 && !matches!(g.phase, Phase::Defeat | Phase::Victory) {
        if g.pending_doctrine {
            let pick = match g.doctrine_picks() {
                0 | 2 => crate::game::Doctrine::Arsenal,
                _ => crate::game::Doctrine::Overdrive,
            };
            g.choose_doctrine(pick);
        }
        super::game::tests::spend_for_shot(&mut g, &mut built);
        let was = g.wave;
        let mut t = 0.0;
        while g.wave == was && t < WAVE_PERIOD * 3.0 {
            g.update(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
    }
    if g.pending_doctrine {
        g.choose_doctrine(crate::game::Doctrine::Arsenal);
    }
    for _ in 0..(WAVE_PERIOD * 0.5 * 60.0) as u32 {
        g.update(1.0 / 60.0);
    }
    // Select a tower so the panel shows a real tower's stats and commands.
    if !g.towers.is_empty() {
        g.selected = Some(0);
    }
    save(&dir, "ui_midgame", &mut g, &decor);

    // 3. Under pressure, with the ring most of the way full.
    while g.wave < 66 && !matches!(g.phase, Phase::Defeat | Phase::Victory) {
        if g.pending_doctrine {
            g.choose_doctrine(crate::game::Doctrine::Arsenal);
        }
        super::game::tests::spend_for_shot(&mut g, &mut built);
        let was = g.wave;
        let mut t = 0.0;
        while g.wave == was && t < WAVE_PERIOD * 3.0 {
            g.update(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
    }
    for _ in 0..(WAVE_PERIOD * 0.6 * 60.0) as u32 {
        g.update(1.0 / 60.0);
    }
    g.selected = None;
    save(&dir, "ui_pressure", &mut g, &decor);
}

fn save(dir: &std::path::Path, name: &str, g: &mut Game, decor: &Decor) {
    let s = crate::shot_ui::capture(g, decor, W, H, Quality::Ultra);
    let path = dir.join(format!("{name}.png"));
    crate::shot_ui::write_png(&path, &s).expect("could not write the PNG");
    println!(
        "  {name:<12} wave {:>2}  {:>3} towers  {:>3}/{} circling  {:>7} gold  ->  {}",
        g.wave,
        g.towers.len(),
        g.creeps.len(),
        g.flood_limit(),
        g.gold,
        path.display()
    );
}

/// Prints what is actually in one frame's draw list, by shape. Used to find a
/// stray ring or a runaway glow:
///     cargo test --release what_is_in_the_frame -- --ignored --nocapture
#[test]
#[ignore = "diagnostic"]
fn what_is_in_the_frame() {
    let mut g = Game::new();
    g.start_run(0x5CA1_AB1E);
    let decor = Decor::build(&g.board);
    let mut built = 0usize;
    while g.wave < 13 && !matches!(g.phase, crate::game::Phase::Defeat) {
        super::game::tests::spend_for_shot(&mut g, &mut built);
        let was = g.wave;
        let mut t = 0.0;
        while g.wave == was && t < 150.0 {
            g.update(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
    }
    for _ in 0..(45.0 * 0.55 * 60.0) as u32 {
        g.update(1.0 / 60.0);
    }
    println!(
        "wave {}  creeps {}  towers {}  beams {}  projs {}  selected {:?}  build {:?}",
        g.wave,
        g.creeps.len(),
        g.towers.len(),
        g.beams.len(),
        g.projs.len(),
        g.selected,
        g.build_choice
    );
    let widths: Vec<f32> = g.beams.iter().map(|b| b.width).collect();
    println!("  beam widths: {widths:?}");
    let mut d = crate::gfx::draw::DrawList::default();
    crate::view::draw_scene(&g, &decor, &mut d, g.time);
    println!("  glows {}  solids {}", d.glow.len(), d.solid_count());
    for t in &g.towers {
        println!(
            "  tower {} range {:.1} model {:?} burn {:.0}/{:.1} aura {:.1}",
            t.full_name(),
            t.range(),
            t.def().model,
            t.abil().burn_dps,
            t.abil().burn_range,
            t.abil().aura_range
        );
    }
}

/// Prints the average colour of a patch of the frame, so the lighting can be
/// tuned against a number rather than against an impression.
///
///     cargo test --release what_colour_is_the_grass -- --ignored --nocapture
#[test]
#[ignore = "diagnostic"]
fn what_colour_is_the_grass() {
    let mut g = Game::new();
    g.start_run(7);
    g.prep = false;
    let decor = Decor::build(&g.board);
    let shot = shot::capture(&g, &decor, 640, 360, Quality::Ultra);
    let px = |x: usize, y: usize| {
        let i = (y * 640 + x) * 4;
        (
            shot.rgba[i] as u32,
            shot.rgba[i + 1] as u32,
            shot.rgba[i + 2] as u32,
        )
    };
    let patch = |name: &str, x0: usize, y0: usize, w: usize, h: usize| {
        let (mut r, mut gg, mut b) = (0u32, 0u32, 0u32);
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let p = px(x, y);
                r += p.0;
                gg += p.1;
                b += p.2;
            }
        }
        let n = (w * h) as u32;
        println!(
            "  {name:<10} rgb({:>3}, {:>3}, {:>3})",
            r / n,
            gg / n,
            b / n
        );
    };
    println!();
    println!("frame averages:");
    patch("sky", 40, 20, 60, 30);
    patch("field-mid", 300, 200, 60, 30);
    patch("field-low", 300, 300, 60, 30);
    patch("left", 90, 220, 40, 40);
}

/// How bright a *busy* frame actually is, as a distribution.
///
///     cargo test --release how_bright_is_a_busy_frame -- --ignored --nocapture
///
/// `what_colour_is_the_grass` measures an empty board, which is why the field
/// could be measured correct against a screenshot while the game still looked
/// bleached: everything that was too bright was a unit, and there were no units
/// in the frame it sampled. This plays to a wave with three hundred monsters on
/// the lane and reports percentiles.
///
/// What to aim at, read off the Warcraft III screenshot this port is matched
/// to: a median around 90-110, a 99th percentile below about 210, and under a
/// couple of percent of the frame above 240. A 99th percentile pinned at 255
/// means the units are blowing out, whatever the average says.
#[test]
#[ignore = "diagnostic"]
fn how_bright_is_a_busy_frame() {
    let mut g = Game::new();
    g.start_run(0x5CA1_AB1E);
    let decor = Decor::build(&g.board);
    let mut built = 0usize;
    while g.wave < 13 && !matches!(g.phase, Phase::Defeat | Phase::Victory) {
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
    g.selected = None;

    let (w, h) = (640usize, 360usize);
    let shot = shot::capture(&g, &decor, w as u32, h as u32, Quality::Ultra);
    let lum = |i: usize| {
        (shot.rgba[i] as u32 * 54 + shot.rgba[i + 1] as u32 * 183 + shot.rgba[i + 2] as u32 * 19)
            / 256
    };

    let mut hist = [0u32; 256];
    for i in (0..shot.rgba.len()).step_by(4) {
        hist[lum(i) as usize] += 1;
    }
    let total: u32 = hist.iter().sum();
    let pct = |want: f32| {
        let target = (total as f32 * want) as u32;
        let mut acc = 0u32;
        for (v, n) in hist.iter().enumerate() {
            acc += n;
            if acc >= target {
                return v;
            }
        }
        255
    };
    let hot: u32 = hist[240..].iter().sum();

    println!();
    println!("wave {}  {} creeps in frame", g.wave, g.creeps.len());
    println!(
        "  luminance  p50 {:>3}   p90 {:>3}   p99 {:>3}   max {:>3}",
        pct(0.50),
        pct(0.90),
        pct(0.99),
        hist.iter().rposition(|&n| n > 0).unwrap_or(0)
    );
    println!(
        "  above 240: {:.2}% of the frame   (aim under 2%)",
        hot as f32 * 100.0 / total as f32
    );

    // Band means: the lane runs across the upper third, the open field below.
    let band = |name: &str, y0: usize, y1: usize| {
        let (mut r, mut gg, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
        for y in y0..y1 {
            for x in 0..w {
                let i = (y * w + x) * 4;
                r += shot.rgba[i] as u64;
                gg += shot.rgba[i + 1] as u64;
                b += shot.rgba[i + 2] as u64;
                n += 1;
            }
        }
        println!(
            "  {name:<12} rgb({:>3}, {:>3}, {:>3})",
            r / n,
            gg / n,
            b / n
        );
    };
    band("lane band", h / 8, h * 2 / 5);
    band("open field", h * 3 / 5, h - 1);
}

/// Every model archetype, alone and close up.
///
///     cargo test --release capture_the_model_sheet -- --ignored --nocapture
///     TD_MODELS=Warrior,Mage cargo test --release capture_the_model_sheet -- --ignored
///
/// One PNG per archetype, because a grid of them is a fight with the camera and
/// a single centred figure is not. `TD_MODELS` narrows it to a comma-separated
/// list while you are working on one.
///
/// This is the only way to actually look at these: on the board a monster is
/// forty pixels tall in a crowd of three hundred, and at that size a figure
/// with the right proportions and one with the wrong proportions are the same
/// smudge. Every proportion in `view/models.rs` was set against these.
#[test]
#[ignore = "renders PNGs; run it deliberately"]
fn capture_the_model_sheet() {
    use crate::gfx::draw::DrawList;
    use crate::view::models::{self, Pose, Skin};

    let dir = out_dir();
    let want = std::env::var("TD_MODELS").unwrap_or_default();
    let wanted: Vec<&str> = want.split(',').filter(|s| !s.is_empty()).collect();
    // Asset authors need to inspect both contact poses. A model can look
    // coherent at rest yet explode half-way through a mismatched bake, which
    // only becomes a pale horde smear at gameplay scale. Keep the default
    // moving frame, with an explicit diagnostic override for rest pose.
    let pose_t = std::env::var("TD_MODEL_POSE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.35);

    println!();
    let mut n = 0;
    for &m in crate::game::greentd_types::Model::ALL {
        let name = format!("{m:?}");
        if !wanted.is_empty() && !wanted.iter().any(|w| w.eq_ignore_ascii_case(&name)) {
            continue;
        }
        let mut d = DrawList::default();
        let pose = Pose {
            pos: [0.0, 0.0],
            z: 0.1,
            // Big: the sheet is for seeing the figure, not for reproducing how
            // small it is in play.
            r: 0.42,
            yaw: 0.7,
            t: pose_t,
            walk: true,
            lights: true,
        };
        models::draw(&mut d, m, &pose, &Skin::wearing(m, [0.85, 0.72, 0.35], 0.0));
        // The two campaign bodies deliberately use a taller tactical transform
        // than older imported meshes.  Aim their inspection camera through the
        // actual torso centre; the old fixed 0.62 lift cropped off their head,
        // shield and horns and made a useful visual review impossible.
        let lift = if matches!(m, crate::game::greentd_types::Model::Warrior | crate::game::greentd_types::Model::Brute) {
            1.72
        } else {
            0.62
        };
        let shot = crate::shot::capture_list(
            &d,
            [0.0, 0.0],
            18.0,
            // Far shallower than the game's fifty-two degrees. From the play
            // camera a standing figure is a head and a pair of shoulders, which
            // is fine for playing and useless for judging leg length.
            24.0,
            // Aimed at the live body's chest rather than at the grass.
            lift,
            560,
            660,
        );
        let path = dir.join(format!("model_{name}.png"));
        crate::shot::write_png(&path, &shot).expect("could not write the sheet");
        // The biggest single piece, because one mis-scaled primitive is the
        // failure mode here: a figure can be forty correct pieces and one cone
        // larger than all of them, and on the board that reads as a smudge
        // rather than as an obvious bug.
        let mut big = (0.0f32, 0usize, [0.0f32; 3]);
        for (si, b) in d.solid.iter().enumerate() {
            for i in b {
                let m = i.scale[0].abs().max(i.scale[1].abs()).max(i.scale[2].abs());
                if m > big.0 {
                    big = (m, si, i.scale);
                }
            }
        }
        println!(
            "  {name:<14} {:>3} pieces   biggest: shape {} at [{:.2},{:.2},{:.2}]  ->  {}",
            d.solid_count(),
            big.1,
            big.2[0],
            big.2[1],
            big.2[2],
            path.display()
        );
        n += 1;
    }
    assert!(n > 0, "TD_MODELS matched nothing");
}

/// One close render per command-card tower family. These are downloaded CC0
/// assemblies, so a loader regression, bad source orientation, or mismatched
/// scale must be visible before it reaches the board screenshots.
#[test]
#[ignore = "renders PNGs; run it deliberately"]
fn capture_the_tower_roster() {
    use crate::gfx::draw::DrawList;
    use crate::view::towers;

    let dir = out_dir();
    // Asset review needs a clean look at a representative tower, rather than
    // inferring one assembly from four staged versions sharing a sheet.  This
    // is a diagnostic selector only; leaving it unset still captures the full
    // live roster used by the command cards.
    let wanted = std::env::var("TD_TOWER_FAMILIES").unwrap_or_default();
    let wanted: Vec<&str> = wanted.split(',').filter(|s| !s.is_empty()).collect();
    let families = [
        Family::Single,
        Family::Siege,
        Family::Bouncing,
        Family::Multi,
        Family::Corruption,
        Family::Air,
        Family::Chaos,
        Family::Destruction,
        Family::Aura,
        Family::Demon,
        Family::King,
    ];
    let mut captured = 0usize;
    for family in families {
        if !wanted.is_empty() && !wanted.iter().any(|name| *name == format!("{family:?}")) {
            continue;
        }
        let mut g = Game::new();
        g.start_run(17);
        g.gold = 1_000_000;
        g.build_choice = Some((family_start(family).expect("shop family"), 1));
        let slot = g
            .board
            .slots
            .iter()
            .position(|s| s.tower.is_none())
            .expect("free plot");
        assert!(g.try_build(slot));
        let tw = g.towers[0].clone();
        let levels: Vec<usize> = TOWERS
            .iter()
            .enumerate()
            .filter(|(_, def)| def.family == family)
            .map(|(i, _)| i)
            .collect();
        let picks = if levels.len() <= 1 {
            vec![0]
        } else {
            vec![0, levels.len() / 3, levels.len() * 2 / 3, levels.len() - 1]
        };
        let mut d = DrawList::default();
        let middle = (picks.len().saturating_sub(1)) as f32 * 0.5;
        for (column, pick) in picks.iter().copied().enumerate() {
            let mut stage = tw.clone();
            stage.def = levels[pick];
            // Across the camera's right axis, not world X: at the game's
            // 45-degree yaw, a row on X recedes into depth and the near tower
            // fills the frame while the far one becomes a dot.
            let across = (column as f32 - middle) * 1.65;
            stage.pos = [
                tw.pos[0] + across * std::f32::consts::FRAC_1_SQRT_2,
                tw.pos[1] - across * std::f32::consts::FRAC_1_SQRT_2,
            ];
            stage.angle = 0.55;
            towers::draw(&mut d, &stage, false, 2.0);
        }
        let centre = tw.pos;
        let span = if picks.len() == 1 { 3.6 } else { 7.4 };
        let image = crate::shot::capture_list(&d, centre, span, 42.0, 0.72, 960, 420);
        let path = dir.join(format!("tower_{family:?}.png"));
        crate::shot::write_png(&path, &image).expect("could not write the tower sheet");
        println!("  {family:?} -> {}", path.display());
        captured += 1;
    }
    assert!(captured > 0, "TD_TOWER_FAMILIES matched no live family");
}

/// Rebuild the command-card atlas from the renderer itself. Every family gets
/// its own column (so branch dressing and colour remain truthful) and each of
/// the four construction milestones gets a row. Stages a short ladder cannot
/// reach are filled with its nearest real stage but are never addressed by UI.
#[test]
#[ignore = "writes the production tower icon atlas; run it deliberately"]
fn bake_matching_tower_icon_atlas() {
    use crate::gfx::draw::DrawList;
    use crate::shot::Shot;
    use crate::view::towers;

    const CELL: u32 = 128;
    const STAGES: u32 = 4;
    let width = CELL * Family::ALL.len() as u32;
    let height = CELL * STAGES;
    let mut atlas = Shot {
        width,
        height,
        rgba: vec![0; (width * height * 4) as usize],
    };
    // Keep all 96 target/readbacks on one device. `capture_list` is ideal for
    // one diagnostic card, but creating a new native adapter for every atlas
    // cell faulted some drivers before the sheet could be written.
    let mut jobs: Vec<(u32, u32, DrawList, [f32; 2], f32, f32)> = Vec::new();

    for family in Family::ALL {
        let mut g = Game::new();
        g.start_run(17);
        g.gold = 1_000_000;
        g.build_choice = Some((family_start(Family::Single).expect("seed tower"), 1));
        assert!(g.try_build(0));
        let base = g.towers[0].clone();
        let levels: Vec<usize> = TOWERS
            .iter()
            .enumerate()
            .filter(|(_, def)| def.family == family)
            .map(|(i, _)| i)
            .collect();
        assert!(!levels.is_empty(), "{family:?} has no tower levels");

        let centre = base.pos;
        for stage in 0..STAGES as usize {
            let def_i = *levels
                .iter()
                .min_by_key(|&&i| tower_visual_stage(&TOWERS[i]).abs_diff(stage))
                .expect("family level");
            let mut tower = base.clone();
            tower.def = def_i;
            tower.angle = 0.55;
            tower.built_at = 0.0;
            let mut draw = DrawList::default();
            towers::draw(&mut draw, &tower, false, 10.0);
            // These authored cannon/obelisk families are genuinely taller than
            // the Seed/Multi silhouette. Their close card crop cut the barrel
            // crown on Siege and Chaos/Destruction as well as Air. Keep normal
            // families at the proven close frame, while all tall variants use
            // the same conservative full-body camera with a small top margin.
            let (span, lift) = if matches!(
                family,
                // These five were individually inspected at all four atlas
                // rows and already have complete silhouettes at the close
                // framing. Every other family uses the conservative full-body
                // camera so an advanced barrel, aura mast, or bounce crown can
                // never cross a command-card cell edge.
                Family::Single | Family::Multi | Family::King | Family::SuperMulti | Family::Troll
            ) {
                (2.9, 0.72)
            } else {
                (3.8, 1.1)
            };
            jobs.push((family.icon_slot() as u32, stage as u32, draw, centre, span, lift));
        }
    }
    let requests: Vec<_> = jobs
        .iter()
        .map(|(_, _, draw, centre, span, lift)| (draw, *centre, *span, 42.0, *lift, CELL, CELL))
        .collect();
    let icons = crate::shot::capture_lists(&requests);
    assert_eq!(icons.len(), jobs.len(), "all command-card cells captured");
    for ((dst_col, stage, _, _, _, _), icon) in jobs.iter().zip(icons.iter()) {
        for y in 0..CELL {
                let src = (y * CELL * 4) as usize;
                let dst = ((((*stage * CELL + y) * width) + *dst_col * CELL) * 4) as usize;
                atlas.rgba[dst..dst + (CELL * 4) as usize]
                    .copy_from_slice(&icon.rgba[src..src + (CELL * 4) as usize]);
        }
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("tower_icons.png");
    crate::shot::write_png(&path, &atlas).expect("could not write matching icon atlas");
    println!("  matching tower icon atlas -> {}", path.display());
}
