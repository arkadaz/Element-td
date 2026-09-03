//! Where a frame's CPU time actually goes.
//!
//! This exists because the answer was repeatedly assumed rather than measured.
//! Every report of the game running slowly was met with culling, batching and
//! instance-count work on the CPU side, and none of it helped, because on a
//! packed board the simulation and the draw-list build together cost well under
//! a tenth of a millisecond. The frame was never waiting on them - it was
//! waiting on render passes, and that is where the fix eventually came from.
//!
//! Keeping it as a test rather than a one-off script means the claim in
//! `docs/DESIGN.md` stays true, and that a change which quietly makes the CPU
//! side twenty times more expensive is caught rather than discovered later by
//! someone on a phone.
#![cfg(test)]

use std::time::Instant;

use crate::decor::Decor;
use crate::game::defs::{FREE_TIERS, MAX_TIER, TOWERS};
use crate::game::{Game, Phase};
use crate::gfx::draw::DrawList;
use crate::view;

/// Budget for one frame's CPU work, in milliseconds. Generous by an order of
/// magnitude - the point is to catch a regression of *kind*, not of degree, and
/// this runs on whatever machine CI happens to give it.
const BUDGET_MS: f64 = 2.0;

/// A board with a tower on every pad and a wave walking the road.
fn busy_board(wave: u32) -> Game {
    let mut g = Game::new();
    g.start_run(7);
    g.gold = i64::MAX / 4;
    g.essence = [(MAX_TIER - FREE_TIERS) as u8; 6];
    g.pending_draft = None;

    for slot in 0..g.board.slots.len() {
        let def = slot % TOWERS.len();
        g.build_choice = Some((def, 1));
        if g.try_build(slot) {
            let ti = g.towers.len() - 1;
            while g.towers[ti].tier < 6 {
                let before = g.towers[ti].tier;
                g.upgrade(ti);
                if g.towers[ti].tier == before {
                    break;
                }
            }
        }
    }
    g.build_choice = None;
    g.selected = None;
    g.wave = wave;

    // Fill the ring by hand, right up to the flood limit - the worst frame the
    // game can ever be asked to draw, and the one it has to survive. Letting
    // the wave arrive on its own measures an empty board, because a hundred
    // level-six towers delete a stream faster than it can spawn.
    g.phase = Phase::Combat;
    let w = crate::game::defs::wave_at(wave);
    for i in 0..crate::game::FLOOD_LIMIT {
        g.spawn_creep(&w, 1.0e9, 1.0, i as f32 * 0.45);
    }
    g.update(1.0 / 60.0);
    g
}

#[test]
fn the_ring_and_its_pads_are_the_size_the_design_says() {
    let b = crate::game::board::Board::new();
    println!(
        "circuit {:.1} tiles round, {} build pads, ring holds {}",
        b.total,
        b.slots.len(),
        crate::game::FLOOD_LIMIT
    );
    // Long enough that a lap takes real time, short enough that a tower on one
    // side is not irrelevant to the other.
    assert!(
        (40.0..80.0).contains(&b.total),
        "circuit is {:.1} tiles",
        b.total
    );
    assert!((60..140).contains(&b.slots.len()), "{} pads", b.slots.len());
}

#[test]
fn a_packed_board_costs_almost_nothing_on_the_cpu() {
    for wave in [20u32, 50, 79] {
        let g = busy_board(wave);
        let decor = Decor::build(&g.board);
        let mut d = DrawList::default();

        view::draw_scene(&g, &decor, &mut d, 0.0); // warm
        let solids = d.solid_count();

        const N: u32 = 200;
        let t0 = Instant::now();
        for i in 0..N {
            d.clear();
            view::draw_scene(&g, &decor, &mut d, i as f32 * 0.016);
        }
        let draw_ms = t0.elapsed().as_secs_f64() * 1000.0 / N as f64;

        let mut sim = g;
        let t1 = Instant::now();
        for _ in 0..N {
            sim.update(1.0 / 60.0);
        }
        let sim_ms = t1.elapsed().as_secs_f64() * 1000.0 / N as f64;

        println!(
            "wave {wave:>3}: {:>3} towers {:>3} creeps {solids:>6} instances | \
             draw {draw_ms:>6.3} ms  sim {sim_ms:>6.3} ms",
            sim.towers.len(),
            sim.creeps.len(),
        );
        assert!(
            draw_ms + sim_ms < BUDGET_MS,
            "wave {wave}: {draw_ms:.3} ms to build the draw list and {sim_ms:.3} ms to simulate - \
             the CPU side has become something a frame has to wait for"
        );
    }
}
