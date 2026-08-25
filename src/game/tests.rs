//! Simulation soak tests.
//!
//! The risky part of this code base is index bookkeeping: creeps are removed with
//! `swap_remove` while projectiles, splash lists and pads hold indices into the
//! same vectors. These tests hammer those paths.

use super::board::{BUILD_FAR, ROAD_HALF};
use super::defs::*;
use super::*;

fn rich_game() -> Game {
    let mut g = Game::new();
    g.gold = 5_000_000;
    g
}

/// Puts a tower on every free pad, cycling through the whole roster.
fn fill_pads(g: &mut Game, tier: u32) -> u32 {
    let mut built = 0;
    let slots = g.board.slots.len();
    for slot in 0..slots {
        g.build_choice = Some((built as usize % TOWERS.len(), tier));
        if g.try_build(slot) {
            built += 1;
        }
    }
    g.build_choice = None;
    g.selected = None;
    built
}

fn run_for(g: &mut Game, seconds: f32) {
    let dt = 1.0 / 60.0;
    for _ in 0..(seconds / dt) as u32 {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        g.update(dt);
    }
}

// ---------------------------------------------------------------- board

#[test]
fn the_road_runs_from_one_side_to_the_other() {
    let b = super::board::Board::new();
    assert!(b.total > 30.0, "road is only {} tiles long", b.total);
    let start = b.start();
    let end = b.end();
    assert!(start[0] < 0.0, "road should start off the left edge");
    assert!(end[0] > board::BW, "road should leave the right edge");

    // Sampling is continuous and stays on the polyline.
    let mut prev = b.sample(0.0);
    let mut d = 0.0;
    while d < b.total {
        let p = b.sample(d);
        let step = ((p[0] - prev[0]).powi(2) + (p[1] - prev[1]).powi(2)).sqrt();
        assert!(step < 0.35, "road sampling jumped {step} tiles at {d}");
        assert!(b.dist_to_road(p) < 0.05, "sample fell off the road at {d}");
        prev = p;
        d += 0.25;
    }
}

#[test]
fn pads_sit_beside_the_road_never_on_it() {
    let b = super::board::Board::new();
    assert!(b.slots.len() >= 20, "only {} build pads", b.slots.len());
    for s in &b.slots {
        let d = b.dist_to_road(s.pos);
        assert!(d > ROAD_HALF, "pad at {:?} is on the road (d={d})", s.pos);
        assert!(d <= BUILD_FAR + 0.01, "pad at {:?} is stranded (d={d})", s.pos);
    }
}

#[test]
fn each_pad_holds_exactly_one_tower() {
    let mut g = rich_game();
    g.build_choice = Some((0, 1));
    assert!(g.try_build(0), "first build should succeed");
    assert_eq!(g.board.slots[0].tower, Some(0));

    g.build_choice = Some((1, 1));
    assert!(!g.try_build(0), "a taken pad must refuse a second tower");
    assert_eq!(g.towers.len(), 1);
}

#[test]
fn selling_frees_the_pad_and_keeps_indices_straight() {
    let mut g = rich_game();
    let built = fill_pads(&mut g, 1);
    assert!(built > 10, "expected a full board, got {built}");

    // Sell from the front repeatedly: this is where swap_remove bites.
    while !g.towers.is_empty() {
        g.sell(0);
        for (i, t) in g.towers.iter().enumerate() {
            assert_eq!(
                g.board.slots[t.slot].tower,
                Some(i),
                "pad {} points at the wrong tower after a sell",
                t.slot
            );
        }
    }
    assert!(g.board.slots.iter().all(|s| s.tower.is_none()));
}

// ---------------------------------------------------------------- combat

#[test]
fn monsters_that_are_not_stopped_reach_the_end() {
    let mut g = rich_game();
    let before = g.lives;
    run_for(&mut g, 120.0);
    assert!(g.lives < before, "no tower on the board, yet nothing leaked");
    assert!(g.stats.leaked > 0);
}

/// A long run with every tower type firing into dense waves. Exercises splash
/// lists, deaths during iteration, splits, leaks and beams.
#[test]
fn long_run_does_not_panic() {
    let mut g = rich_game();
    let built = fill_pads(&mut g, 2);
    assert!(built > 15, "expected a dense board, got {built}");

    run_for(&mut g, 60.0 * 12.0);

    assert!(g.stats.kills > 0, "nothing died in twelve minutes");
    assert!(g.wave >= 3, "wave loop stalled at wave {}", g.wave);
    assert!(g.creeps.len() <= MAX_CREEPS);
    for c in &g.creeps {
        assert!(c.hp.is_finite() && c.hp > 0.0, "dead monster left on the road");
        assert!(c.pos[0].is_finite() && c.pos[1].is_finite());
        assert!(c.dist >= -1.0 && c.dist <= g.board.total + 1.0);
    }
    for p in &g.projs {
        assert!(p.pos[0].is_finite() && p.pos[1].is_finite() && p.z.is_finite());
    }
}

#[test]
fn splash_into_a_dense_pack_is_safe() {
    let mut g = rich_game();
    // The Cannon has the widest blast in the game; put it next to the road.
    let cannon = TOWERS.iter().position(|t| t.id == "cannon").unwrap();
    let slot = nearest_slot_to_dist(&g, 6.0);
    g.build_choice = Some((cannon, 3));
    assert!(g.try_build(slot));
    g.build_choice = None;

    let w = WaveDef {
        kind: Kind::Swarm,
        count: 300,
        hp: 40.0,
        speed: 0.6,
        bounty: 1,
        gap: 0.0,
        shield: 0.0,
        heal: 0.0,
        phasing: false,
        regen: false,
        split: true,
    };
    g.phase = Phase::Combat;
    let at = g.towers[0].pos;
    for i in 0..300 {
        g.spawn_creep(&w, w.hp, 1.0, 0.0);
        if let Some(c) = g.creeps.last_mut() {
            // Bunch them right where the tower is aiming.
            c.dist = 4.0 + (i % 9) as f32 * 0.12;
        }
    }
    let start = g.creeps.len();
    let _ = at;
    run_for(&mut g, 25.0);
    assert!(g.stats.kills > 0, "splash killed nothing out of {start}");
}

fn nearest_slot_to_dist(g: &Game, dist: f32) -> usize {
    let p = g.board.sample(dist);
    let mut best = 0;
    let mut best_d = f32::MAX;
    for (i, s) in g.board.slots.iter().enumerate() {
        let d = (s.pos[0] - p[0]).powi(2) + (s.pos[1] - p[1]).powi(2);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

#[test]
fn knockback_pushes_monsters_backwards_never_past_the_start() {
    let mut g = rich_game();
    // Grapeshot is the Cannon fork that shoves things back down the road.
    let cannon = TOWERS.iter().position(|t| t.id == "cannon").unwrap();
    let grapeshot = TOWERS[cannon]
        .forks
        .iter()
        .position(|f| f.name == "Grapeshot")
        .unwrap();
    g.build_choice = Some((cannon, 1));
    assert!(g.try_build(nearest_slot_to_dist(&g, 6.0)));
    g.build_choice = None;
    // Climb to the fork level and take the Grapeshot branch.
    while g.towers[0].tier + 1 < FORK_TIER {
        g.upgrade(0, None);
    }
    g.upgrade(0, Some(grapeshot));
    assert_eq!(g.towers[0].tier, FORK_TIER);
    assert_eq!(g.towers[0].fork, Some(grapeshot));
    assert!(
        g.towers[0]
            .specials()
            .iter()
            .any(|s| matches!(s, Special::Knockback { .. })),
        "the Grapeshot fork should carry knockback"
    );

    let w = g.wave_def(1);
    g.phase = Phase::Combat;
    g.spawn_creep(&w, 1.0e9, 1.0, 0.0);
    g.creeps[0].dist = 0.5;
    run_for(&mut g, 20.0);
    for c in &g.creeps {
        assert!(c.dist >= 0.0, "knockback pushed a monster off the start of the road");
    }
}

// ---------------------------------------------------------------- data

#[test]
fn the_counter_triangle_holds() {
    // Physical bounces off plate and shreds wards; magic is the mirror image.
    assert!(armor_mult(Damage::Physical, Armor::Heavy) < 1.0);
    assert!(armor_mult(Damage::Physical, Armor::Warded) > 1.0);
    assert!(armor_mult(Damage::Magic, Armor::Heavy) > 1.0);
    assert!(armor_mult(Damage::Magic, Armor::Warded) < 1.0);
    // Poison is the dependable floor: never resisted, never bonus.
    for a in [Armor::Unarmoured, Armor::Heavy, Armor::Warded, Armor::Boss] {
        assert_eq!(armor_mult(Damage::Poison, a), 1.0);
    }
    // Bosses tax everything else.
    assert!(armor_mult(Damage::Physical, Armor::Boss) < 1.0);
}

#[test]
fn every_tower_has_a_distinct_role_and_two_real_forks() {
    let mut roles = std::collections::HashSet::new();
    for d in TOWERS.iter() {
        assert!(roles.insert(d.role), "two towers share the role {}", d.role);
        let base = d.stats(MAX_TIER, None);
        let base_n = d.specials_for(None).iter().count();
        for (i, f) in d.forks.iter().enumerate() {
            // A fork must actually change something, or it is not a choice.
            let forked = d.stats(MAX_TIER, Some(i));
            let stats_differ = (base.dmg - forked.dmg).abs() > 0.01
                || (base.rate - forked.rate).abs() > 0.001
                || (base.range - forked.range).abs() > 0.001
                || (base.splash - forked.splash).abs() > 0.001;
            let specials_differ = d.specials_for(Some(i)).iter().count() != base_n;
            assert!(
                stats_differ || specials_differ,
                "{} fork {} changes nothing",
                d.id,
                f.name
            );
        }
        // The two forks must differ from each other, too.
        let a = d.stats(MAX_TIER, Some(0));
        let b = d.stats(MAX_TIER, Some(1));
        let differ = (a.dmg - b.dmg).abs() > 0.01
            || (a.rate - b.rate).abs() > 0.001
            || (a.range - b.range).abs() > 0.001
            || d.forks[0].specials.len() != d.forks[1].specials.len();
        assert!(differ, "{} forks are interchangeable", d.id);
    }
}

#[test]
fn shields_stop_everything_except_poison() {
    let mut g = rich_game();
    g.phase = Phase::Combat;
    let w = WaveDef {
        kind: Kind::Bulwark,
        count: 1,
        hp: 500.0,
        speed: 0.0,
        bounty: 1,
        gap: 0.0,
        shield: 200.0,
        heal: 0.0,
        phasing: false,
        regen: false,
        split: false,
    };
    g.spawn_creep(&w, w.hp, 1.0, 5.0);

    let ballista = TOWERS.iter().position(|t| t.id == "ballista").unwrap();
    g.build_choice = Some((ballista, 1));
    assert!(g.try_build(nearest_slot_to_dist(&g, 5.0)));
    g.build_choice = None;
    combat::damage_creep(&mut g, 0, 100.0, 0, false);
    assert!(g.creeps[0].shield < 200.0, "shield did not absorb");
    assert_eq!(g.creeps[0].hp, 500.0, "health should be untouched behind a shield");

    let venom = TOWERS.iter().position(|t| t.id == "venom").unwrap();
    g.build_choice = Some((venom, 1));
    assert!(g.try_build(nearest_slot_to_dist(&g, 9.0)));
    g.build_choice = None;
    let vi = g.towers.len() - 1;
    combat::damage_creep(&mut g, 0, 50.0, vi, false);
    assert!(g.creeps[0].hp < 500.0, "poison should bypass the shield");
}

#[test]
fn beacons_buff_the_cluster_and_stop_when_sold() {
    let mut g = rich_game();
    let ballista = TOWERS.iter().position(|t| t.id == "ballista").unwrap();
    let beacon = TOWERS.iter().position(|t| t.id == "beacon").unwrap();

    g.build_choice = Some((ballista, 1));
    let slot_a = nearest_slot_to_dist(&g, 6.0);
    assert!(g.try_build(slot_a));
    let plain = g.towers[0].dmg();

    g.build_choice = Some((beacon, 1));
    let mut placed = false;
    for slot in 0..g.board.slots.len() {
        if g.board.slots[slot].tower.is_some() {
            continue;
        }
        let reach = TOWERS[beacon].stats(1, None).range;
        let d = (g.board.slots[slot].pos[0] - g.towers[0].pos[0]).powi(2)
            + (g.board.slots[slot].pos[1] - g.towers[0].pos[1]).powi(2);
        if d < reach * reach && g.try_build(slot) {
            placed = true;
            break;
        }
    }
    g.build_choice = None;
    assert!(placed, "no pad near enough to test the aura");
    assert!(g.towers[0].dmg() > plain, "beacon did not buff the neighbour");

    let bi = g.towers.iter().position(|t| t.is_support()).unwrap();
    g.sell(bi);
    let after = g.towers.iter().find(|t| !t.is_support()).unwrap().dmg();
    assert!((after - plain).abs() < 0.01, "aura outlived the beacon");
}

#[test]
fn every_tower_is_buildable_and_priced_sanely() {
    let g = Game::new();
    for (i, d) in TOWERS.iter().enumerate() {
        assert!(d.range > 0.0, "{}", d.id);
        assert_eq!(g.max_tier_of(i), MAX_TIER, "{} should be ungated", d.id);
        for tier in 1..=MAX_TIER {
            assert!(d.cost_at(tier) > 0);
            assert!(d.dps_at(tier, None).is_finite());
        }
        // Six levels must climb steeply enough to matter and stay affordable.
        assert!(d.cost_at(MAX_TIER) > d.cost_at(1) * 20, "{}", d.id);
        assert!(
            d.cost_at(MAX_TIER) < 3_500,
            "{} maxed costs {} - too expensive to ever reach",
            d.id,
            d.cost_at(MAX_TIER)
        );
    }
    // The shop lists every tower exactly once, cheapest first.
    let order = shop_order();
    assert_eq!(order.len(), TOWERS.len());
    let mut seen = order.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), TOWERS.len());
    for w in order.windows(2) {
        assert!(TOWERS[w[0]].cost <= TOWERS[w[1]].cost, "shop is not sorted by cost");
    }
}

#[test]
fn wave_table_is_well_formed() {
    let waves = build_waves();
    assert_eq!(waves.len() as u32, N_WAVES);
    for (i, w) in waves.iter().enumerate() {
        assert!(w.count > 0, "wave {} has no monsters", i + 1);
        assert!(w.hp > 0.0 && w.hp.is_finite());
        assert!(w.speed > 0.0);
        if (i + 1) % 10 == 0 {
            assert_eq!(w.kind, Kind::Boss, "wave {} should be a boss", i + 1);
        }
    }
    assert!(waves[49].hp > waves[0].hp * 50.0, "difficulty barely climbs");
}

// ---------------------------------------------------------------- difficulty

#[test]
fn difficulty_actually_changes_the_run() {
    let normal = wave_at(20, Difficulty::Normal);
    let hard = wave_at(20, Difficulty::Hard);
    let nightmare = wave_at(20, Difficulty::Nightmare);

    assert!(hard.hp > normal.hp * 1.3, "Hard barely differs from Normal");
    assert!(nightmare.hp > hard.hp * 1.3, "Nightmare barely differs from Hard");
    // Harder runs pay better, or they are a wall rather than a challenge.
    assert!(hard.bounty >= normal.bounty);
    assert!(nightmare.bounty >= hard.bounty);
    // And they give you less room to fail.
    assert!(Difficulty::Hard.lives() < Difficulty::Normal.lives());
    assert!(Difficulty::Nightmare.lives() < Difficulty::Hard.lives());

    let mut g = Game::new();
    g.restart(Difficulty::Nightmare);
    assert_eq!(g.difficulty, Difficulty::Nightmare);
    assert_eq!(g.lives, Difficulty::Nightmare.lives());
    assert_eq!(g.wave, 0);
}

#[test]
fn waves_keep_escalating_past_the_campaign() {
    let last = wave_at(CAMPAIGN_WAVES, Difficulty::Normal);
    let mut prev_peak = last.hp;
    for w in CAMPAIGN_WAVES + 1..=CAMPAIGN_WAVES + 60 {
        let d = wave_at(w, Difficulty::Normal);
        assert!(d.hp.is_finite() && d.hp > 0.0, "wave {w} has broken health");
        assert!(d.count > 0 && d.speed > 0.0, "wave {w} is malformed");
        assert!(d.bounty >= 1, "wave {w} pays nothing");
        // Boss waves dip in count but the curve as a whole must climb.
        if w % 10 != 0 {
            prev_peak = prev_peak.max(d.hp);
        }
    }
    // Fifty waves of endless should be a genuine escalation, not a plateau.
    let far = wave_at(CAMPAIGN_WAVES + 50, Difficulty::Normal);
    assert!(
        far.hp > last.hp * 8.0,
        "endless barely ramps: wave {} is {:.0} vs {:.0} at the campaign end",
        CAMPAIGN_WAVES + 50,
        far.hp,
        last.hp
    );
    assert!(prev_peak > last.hp);
}

#[test]
fn clearing_the_campaign_wins_but_can_be_continued() {
    let mut g = rich_game();
    g.wave = CAMPAIGN_WAVES;
    g.phase = Phase::Combat;
    g.spawn_left = 0;
    // Finish the last wave with an empty field.
    g.update(1.0 / 60.0);
    assert_eq!(g.phase, Phase::Victory, "clearing the campaign should win");

    g.continue_endless();
    assert!(g.endless);
    assert_eq!(g.phase, Phase::Build);

    // And now the run no longer stops at the campaign boundary.
    g.wave = CAMPAIGN_WAVES + 3;
    g.phase = Phase::Combat;
    g.spawn_left = 0;
    g.update(1.0 / 60.0);
    assert_eq!(g.phase, Phase::Build, "endless should roll straight into the next wave");
}
