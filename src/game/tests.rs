//! Simulation soak tests.
//!
//! The risky part of this code base is index bookkeeping: creeps are removed with
//! `swap_remove` while projectiles, splash lists and pads hold indices into the
//! same vectors. These tests hammer those paths.

use super::board::{BUILD_FAR, Board, ROAD_HALF};
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
        escort: None,
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
        escort: None,
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
        // Ten levels have to climb steeply enough to be worth taking, and a
        // maxed tower has to stay inside what a campaign actually pays out.
        assert!(d.cost_at(MAX_TIER) > d.cost_at(1) * 30, "{} barely gets more expensive", d.id);
        // Everything a full campaign pays: kill bounties and survival bonuses.
        let purse: f32 = (1..=CAMPAIGN_WAVES)
            .map(|w| {
                let d = wave_at(w);
                d.bounty as f32 * d.count as f32 + wave_clear_bonus(w) as f32
            })
            .sum();
        assert!(
            (d.cost_at(MAX_TIER) as f32) < purse / 12.0,
            "{} maxed costs {} against a {:.0} campaign purse - a board of these is unreachable",
            d.id,
            d.cost_at(MAX_TIER),
            purse
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
            assert!(w.kind.is_boss(), "wave {} should be a boss, got {:?}", i + 1, w.kind);
        }
    }
    assert!(
        waves[N_WAVES as usize - 1].hp > waves[0].hp * 200.0,
        "the curve barely climbs across the campaign"
    );
}

// ---------------------------------------------------------------- the curve

/// Health must outrun gold, so the run tightens as it goes.
///
/// It has to outrun it by a *lot*, which is not obvious: the player's board
/// grows by tower count and by level as well as by gold, and gold-per-damage
/// improves every level too. Those compound on top of the raw income curve.
///
/// This test only pins the direction and the monotonicity. Whether the result
/// is actually *fair* is not something arithmetic can answer, so it is not
/// asserted here - `a_sensible_build_clears_the_campaign` plays the whole
/// campaign and answers it properly.
#[test]
fn the_run_tightens_from_start_to_finish() {
    let block = |from: u32, to: u32| -> (f32, f32) {
        let mut hp = 0.0;
        let mut gold = 0.0;
        for w in from..=to {
            let d = wave_at(w);
            hp += d.hp * d.count as f32;
            gold += d.bounty as f32 * d.count as f32 + wave_clear_bonus(w) as f32;
        }
        (hp, gold)
    };
    let (hp_a, gold_a) = block(1, 20);
    let (hp_b, gold_b) = block(CAMPAIGN_WAVES - 19, CAMPAIGN_WAVES);
    assert!(
        hp_b / hp_a > gold_b / gold_a,
        "the run never tightens: health x{:.0} against gold x{:.0}",
        hp_b / hp_a,
        gold_b / gold_a
    );

    // And it must tighten smoothly - no wave may suddenly be far harder than
    // the one before it, which is how a curve becomes a wall.
    //
    // A type's debut wave is deliberately softened, so the step *up* from a
    // debut to that type's second appearance is expected and is not a wall.
    let debut_of = |kind: Kind| (1..=CAMPAIGN_WAVES).find(|&w| wave_at(w).kind == kind);
    for w in 2..=CAMPAIGN_WAVES {
        let a = wave_at(w - 1);
        let b = wave_at(w);
        // Compare like with like: boss waves are one enormous monster by design.
        if a.kind != b.kind || debut_of(a.kind) == Some(w - 1) {
            continue;
        }
        let jump = (b.hp * b.count as f32) / (a.hp * a.count as f32);
        assert!(
            jump < 1.6,
            "wave {w} is {jump:.1}x the wave before it for the same monster - that is a wall"
        );
    }
}

/// A run has to be long enough to be a run. Eighty waves at roughly 45 seconds
/// each is the hour the design asks for.
#[test]
fn a_full_run_is_about_an_hour() {
    assert_eq!(CAMPAIGN_WAVES, 80);
    let road = Board::new().total;
    let mut seconds = BUILD_TIME_FIRST;
    for w in 1..=CAMPAIGN_WAVES {
        let d = wave_at(w);
        // Build phase, then however long the wave takes to walk the road.
        let walk = road / (WALK_SPEED * d.speed);
        let spawn = d.gap * (d.count.saturating_sub(1)) as f32;
        seconds += BUILD_TIME + walk + spawn;
    }
    let minutes = seconds / 60.0;
    println!("CAMPAIGN LENGTH: {minutes:.0} minutes");
    assert!(
        (45.0..90.0).contains(&minutes),
        "a full campaign takes {minutes:.0} minutes; the target is about 60"
    );
}

#[test]
fn waves_keep_escalating_past_the_campaign() {
    let last = wave_at(CAMPAIGN_WAVES);
    let mut prev_peak = last.hp;
    for w in CAMPAIGN_WAVES + 1..=CAMPAIGN_WAVES + 60 {
        let d = wave_at(w);
        assert!(d.hp.is_finite() && d.hp > 0.0, "wave {w} has broken health");
        assert!(d.count > 0 && d.speed > 0.0, "wave {w} is malformed");
        assert!(d.bounty >= 1, "wave {w} pays nothing");
        // Boss waves dip in count but the curve as a whole must climb.
        if w % 10 != 0 {
            prev_peak = prev_peak.max(d.hp);
        }
    }
    // Fifty waves of endless should be a genuine escalation, not a plateau.
    let far = wave_at(CAMPAIGN_WAVES + 50);
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

// ---------------------------------------------------------------- the roster

/// No two towers may own the same verb.
///
/// This test exists because they did: Pyre and Venom were both poison-type
/// damage-over-time, which meant one of them was always strictly the worse
/// choice and the roster was padding. A tower's identity is its delivery plus
/// what its specials actually *do*, so that pair is what gets compared.
#[test]
fn every_tower_owns_a_verb_nothing_else_has() {
    /// The coarse shape of a tower, ignoring numbers.
    fn verb(d: &TowerDef) -> String {
        let delivery = match d.delivery {
            Delivery::Shot { .. } => "shot",
            Delivery::Beam { .. } => "beam",
            Delivery::Lance { .. } => "lance",
            Delivery::Chain { .. } => "chain",
            Delivery::Nova => "nova",
            Delivery::Zone { .. } => "zone",
            Delivery::Aura => "aura",
        };
        let mut tags: Vec<&str> = d
            .specials
            .iter()
            .map(|s| match s {
                Special::Burn { .. } => "burn",
                Special::Slow { .. } => "slow",
                Special::Poison { .. } => "poison",
                Special::Crit { .. } => "crit",
                Special::Stun { .. } => "stun",
                Special::Shred { .. } => "shred",
                Special::Ramp { .. } => "ramp",
                Special::Knockback { .. } => "knockback",
                Special::Buff { .. } => "buff",
                Special::Income { .. } => "income",
                Special::Bounty { .. } => "bounty",
                Special::Interest { .. } => "interest",
                Special::Contagion { .. } => "contagion",
            })
            .collect();
        tags.sort_unstable();
        let splash = if d.splash > 0.0 { "+splash" } else { "" };
        format!("{delivery}{splash}[{}]", tags.join(","))
    }

    let mut seen: Vec<(String, &str)> = Vec::new();
    for d in TOWERS {
        let v = verb(d);
        if let Some((_, other)) = seen.iter().find(|(s, _)| *s == v) {
            panic!("{} and {} are the same tower: both are {v}", other, d.id);
        }
        seen.push((v, d.id));
    }

    // And every attacker must have a reason to exist beyond its damage number:
    // no two share both a damage type and a delivery.
    for (i, a) in TOWERS.iter().enumerate() {
        for b in TOWERS.iter().skip(i + 1) {
            if a.targets == Targets::Nothing || b.targets == Targets::Nothing {
                continue;
            }
            let same_delivery =
                std::mem::discriminant(&a.delivery) == std::mem::discriminant(&b.delivery);
            assert!(
                !(a.dtype == b.dtype && same_delivery && (a.splash > 0.0) == (b.splash > 0.0)),
                "{} and {} overlap: same damage type and same delivery",
                a.id,
                b.id
            );
        }
    }
}

// ---------------------------------------------------------------- air

/// The core build tension: the two biggest area dealers cannot shoot up, and
/// everything else can. If that ever stops being true the game loses its shape -
/// either air is unanswerable, or it is free.
#[test]
fn the_air_layer_splits_the_roster() {
    let ground_only: Vec<&str> = TOWERS
        .iter()
        .filter(|d| d.targets == Targets::GroundOnly)
        .map(|d| d.id)
        .collect();
    assert_eq!(
        ground_only,
        vec!["cannon", "pyre"],
        "the ground-only set is the whole design; changing it needs a design decision"
    );

    // Every other attacker answers both layers, so the fix for a flying wave is
    // never "own the one anti-air tower".
    let both: Vec<&str> = TOWERS
        .iter()
        .filter(|d| d.targets == Targets::Both)
        .map(|d| d.id)
        .collect();
    assert!(both.len() >= 4, "too few towers answer air: {both:?}");

    // And the ground-only pair has to be worth the drawback.
    let best_ground: f32 = TOWERS
        .iter()
        .filter(|d| d.targets == Targets::GroundOnly)
        .map(|d| d.effective_dps_at(MAX_TIER, Some(0)))
        .fold(0.0, f32::max);
    let best_both: f32 = TOWERS
        .iter()
        .filter(|d| d.targets == Targets::Both && d.dtype != Damage::None)
        .map(|d| d.effective_dps_at(MAX_TIER, Some(0)))
        .fold(0.0, f32::max);
    assert!(
        best_ground > best_both * 0.85,
        "ground-only towers give up air and get nothing for it: {best_ground:.0} vs {best_both:.0}"
    );
}

#[test]
fn flying_waves_arrive_early_and_bosses_alternate_layers() {
    let first_air = (1..=CAMPAIGN_WAVES)
        .find(|&w| wave_at(w).kind.flying())
        .expect("no flying wave in the whole campaign");
    assert!(
        (5..=9).contains(&first_air),
        "first flying wave is {first_air}; it should arrive early enough to be a lesson"
    );

    let mut ground_bosses = 0;
    let mut air_bosses = 0;
    for w in (10..=CAMPAIGN_WAVES).step_by(10) {
        let d = wave_at(w);
        assert!(d.kind.is_boss(), "wave {w} is not a boss");
        if d.kind.flying() {
            air_bosses += 1;
        } else {
            ground_bosses += 1;
        }
    }
    assert!(
        ground_bosses >= 3 && air_bosses >= 3,
        "bosses do not alternate layers: {ground_bosses} ground, {air_bosses} air"
    );
}

/// A mortar must never hit something flying over it - not once, not at any
/// range, not even the target it was already holding when the thing took off.
#[test]
fn ground_towers_cannot_touch_the_air() {
    let mut g = rich_game();
    let cannon = TOWERS.iter().position(|d| d.id == "cannon").unwrap();
    let tesla = TOWERS.iter().position(|d| d.id == "tesla").unwrap();

    // One of each, on the two pads nearest the road.
    for (def, tier) in [(cannon, MAX_TIER), (tesla, MAX_TIER)] {
        let slot = g.board.slots.iter().position(|s| s.tower.is_none()).unwrap();
        g.build_choice = Some((def, tier));
        assert!(g.try_build(slot));
    }
    g.build_choice = None;

    // Send a flying wave and let it walk the whole road.
    let w = WaveDef { kind: Kind::Wisp, ..wave_at(7) };
    g.phase = Phase::Combat;
    for _ in 0..10 {
        g.spawn_creep(&w, 4_000.0, 1.0, 2.0);
    }
    run_for(&mut g, 30.0);

    let cannon_kills: u32 = g.towers.iter().filter(|t| t.def == cannon).map(|t| t.kills).sum();
    let cannon_dmg: f64 = g.towers.iter().filter(|t| t.def == cannon).map(|t| t.damage).sum();
    let tesla_dmg: f64 = g.towers.iter().filter(|t| t.def == tesla).map(|t| t.damage).sum();

    assert_eq!(cannon_kills, 0, "a cannon killed something airborne");
    assert!(cannon_dmg < 0.001, "a cannon dealt {cannon_dmg} damage to the air");
    assert!(tesla_dmg > 0.0, "the tesla should have been shooting the whole time");
}

/// And it must still be lethal on the ground, or the drawback has no upside.
#[test]
fn ground_towers_are_lethal_on_the_ground() {
    let mut g = rich_game();
    let cannon = TOWERS.iter().position(|d| d.id == "cannon").unwrap();
    let slot = g.board.slots.iter().position(|s| s.tower.is_none()).unwrap();
    g.build_choice = Some((cannon, MAX_TIER));
    assert!(g.try_build(slot));
    g.build_choice = None;

    let w = WaveDef { kind: Kind::Grunt, ..wave_at(7) };
    g.phase = Phase::Combat;
    for _ in 0..10 {
        g.spawn_creep(&w, 400.0, 1.0, 2.0);
    }
    run_for(&mut g, 30.0);
    assert!(g.towers[0].damage > 0.0, "the cannon never fired at a ground wave");
}

/// Pyre holds ground rather than tracking a target: it must leave fire behind
/// that hurts whatever walks into it, and must never light up the sky.
#[test]
fn pyre_burns_the_road_and_only_the_road() {
    let mut g = rich_game();
    let pyre = TOWERS.iter().position(|d| d.id == "pyre").unwrap();
    let slot = g.board.slots.iter().position(|s| s.tower.is_none()).unwrap();
    g.build_choice = Some((pyre, MAX_TIER));
    assert!(g.try_build(slot));
    g.build_choice = None;

    let ground = WaveDef { kind: Kind::Grunt, ..wave_at(9) };
    g.phase = Phase::Combat;
    for _ in 0..8 {
        g.spawn_creep(&ground, 20_000.0, 1.0, 2.0);
    }
    let mut ever_shredded = false;
    let mut ever_burning = false;
    for _ in 0..(12.0 * 60.0) as u32 {
        g.update(1.0 / 60.0);
        ever_burning |= !g.zones.is_empty();
        ever_shredded |= g.creeps.iter().any(|c| c.shred.t > 0.0);
    }
    assert!(ever_burning, "the pyre never lit the road");
    assert!(g.towers[0].damage > 0.0, "fire on the road did no damage");
    // The shred is the real payload - the damage is the smaller half.
    assert!(ever_shredded, "nothing standing in the fire was ever shredded");

    // Now the same wave, airborne.
    let mut g2 = rich_game();
    let slot = g2.board.slots.iter().position(|s| s.tower.is_none()).unwrap();
    g2.build_choice = Some((pyre, MAX_TIER));
    assert!(g2.try_build(slot));
    g2.build_choice = None;
    let air = WaveDef { kind: Kind::Wisp, ..wave_at(7) };
    g2.phase = Phase::Combat;
    for _ in 0..8 {
        g2.spawn_creep(&air, 20_000.0, 1.0, 2.0);
    }
    run_for(&mut g2, 12.0);
    assert!(g2.towers[0].damage < 0.001, "the pyre burned something airborne");
}

/// Zones are the only thing in the game that leaks memory if nothing retires
/// them, and they are created several times a second per Pyre.
#[test]
fn burning_ground_expires() {
    let mut g = rich_game();
    let pyre = TOWERS.iter().position(|d| d.id == "pyre").unwrap();
    for _ in 0..6 {
        let slot = g.board.slots.iter().position(|s| s.tower.is_none()).unwrap();
        g.build_choice = Some((pyre, MAX_TIER));
        g.try_build(slot);
    }
    g.build_choice = None;

    let w = wave_at(9);
    g.phase = Phase::Combat;
    for _ in 0..20 {
        g.spawn_creep(&w, 500_000.0, 1.0, 2.0);
    }
    run_for(&mut g, 60.0);
    assert!(g.zones.len() < 400, "burning ground is piling up: {} zones", g.zones.len());

    // With nothing left to burn they must all drain away.
    g.creeps.clear();
    run_for(&mut g, 12.0);
    assert!(g.zones.is_empty(), "{} zones outlived the wave", g.zones.len());
}

// ---------------------------------------------------------------- winnable

/// Plays the campaign the way a competent player would, and checks it can be
/// won. Balance arguments on paper are worth very little - the previous curve
/// looked reasonable written down and was arithmetically impossible past wave
/// forty. This actually runs the simulation.
///
/// The bot is deliberately unsophisticated: spend everything, prefer upgrading
/// what it already owns, keep a mix of layers. If a *simple* strategy clears it
/// with something in hand, a thinking player has room to be clever.
/// Where along the road a pad sits, in path distance.
///
/// Coverage of *the road* is what a board is short of, not coverage of the map.
/// Picking the pad furthest from other towers in plain 2D spreads them into the
/// corners, where they watch empty grass.
fn road_position(b: &Board, p: [f32; 2]) -> f32 {
    let mut best = (f32::MAX, 0.0);
    let steps = (b.total * 2.0) as i32;
    for i in 0..=steps {
        let d = i as f32 * 0.5;
        let q = b.sample(d);
        let dd = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2);
        if dd < best.0 {
            best = (dd, d);
        }
    }
    best.1
}

/// The free pad a competent player would take next.
///
/// Not the one furthest from the others: spreading evenly along the road means
/// a monster meets one tower at a time and survives each of them in turn.
/// Concentrated fire kills, and overlapping ranges stack Beacon auras - so the
/// model is a killbox. Hug the road first, and among equally good pads pick the
/// one nearest what is already built. Tried the even-spread version; it loses
/// the campaign on wave 76, which is the whole reason this comment exists.
fn spread_pad(g: &Game) -> Option<usize> {
    let centre = if g.towers.is_empty() {
        None
    } else {
        let n = g.towers.len() as f32;
        Some([
            g.towers.iter().map(|t| t.pos[0]).sum::<f32>() / n,
            g.towers.iter().map(|t| t.pos[1]).sum::<f32>() / n,
        ])
    };
    let mut best: Option<(usize, f32)> = None;
    for (i, s) in g.board.slots.iter().enumerate() {
        if s.tower.is_some() {
            continue;
        }
        // Closeness to the road dominates; closeness to the cluster breaks ties.
        let road = g.board.dist_to_road(s.pos);
        let huddle = centre.map_or(0.0, |c| {
            ((c[0] - s.pos[0]).powi(2) + (c[1] - s.pos[1]).powi(2)).sqrt()
        });
        let score = road * 3.0 + huddle * 0.35;
        if best.is_none_or(|(_, v)| score < v) {
            best = Some((i, score));
        }
    }
    best.map(|(i, _)| i)
}

fn autoplay(mixed_layers: bool) -> Game {
    let mut g = Game::new();
    // A reasonable spread. Ballista and Tesla answer air, Cannon and Pyre hold
    // the road, Frost buys time, Venom kills bosses, Beacon and Mint compound.
    let want: Vec<usize> = if mixed_layers {
        ["ballista", "cannon", "tesla", "frost", "pyre", "venom", "beacon", "mint"]
            .iter()
            .map(|id| TOWERS.iter().position(|d| d.id == *id).unwrap())
            .collect()
    } else {
        // The trap build: everything on the ground.
        ["cannon", "pyre", "cannon", "pyre", "mint", "beacon"]
            .iter()
            .map(|id| TOWERS.iter().position(|d| d.id == *id).unwrap())
            .collect()
    };

    let mut built = 0usize;
    for _ in 0..(CAMPAIGN_WAVES + 8) {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        // Spend during the build phase, the way a player does. This has to
        // happen once per wave: running a fixed number of seconds instead lets
        // the build timer elapse inside the run, the next wave auto-starts, and
        // the "player" silently never builds again.
        if g.phase == Phase::Build {
            spend(&mut g, &want, &mut built);
            if std::env::var("TD_TRACE").is_ok() && (g.wave + 1) % 10 == 0 {
                let w = g.next_wave_def();
                let dps: f32 = g.towers.iter().map(|t| t.dmg() * t.rate()).sum();
                println!(
                    "  w{:>3} | {:>2} towers {:>9.0} dps | next {:?} x{} = {:>10.0} ehp | lives {:>2} gold {}",
                    g.wave + 1,
                    g.towers.len(),
                    dps,
                    w.kind,
                    w.count,
                    w.hp * w.count as f32,
                    g.lives,
                    g.gold
                );
            }
            g.send_wave();
        }
        // Now run exactly one wave, until the game hands control back.
        let dt = 1.0 / 60.0;
        let mut elapsed = 0.0;
        while g.phase == Phase::Combat && elapsed < 200.0 {
            g.update(dt);
            elapsed += dt;
        }
        // A wave that outlasts this is not a hard wave, it is a stuck one -
        // permanently stun-locked monsters neither die nor leak, and the run
        // hangs forever. Silently moving on would mismeasure the whole test.
        assert!(
            g.phase != Phase::Combat,
            "wave {} never ended: {} monsters still alive after 200s, {} lives, {} leaked",
            g.wave,
            g.creeps.len(),
            g.lives,
            g.stats.leaked
        );
    }
    g
}

/// Empties the purse: fill the board out to a working size, then pour
/// everything into levels, then spread again once there is nothing left to
/// upgrade.
fn spend(g: &mut Game, want: &[usize], built: &mut usize) {
    loop {
        let cheapest = g
            .towers
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.upgrade_cost().map(|c| (i, c)))
            .min_by_key(|(_, c)| *c);
        let free_pad = spread_pad(g);
        let all_maxed = cheapest.is_none();
        let want_new = free_pad.is_some() && (*built < 26 || all_maxed);
        let def = want[*built % want.len()];

        if want_new && g.can_afford(TOWERS[def].cost_at(1)) {
            g.build_choice = Some((def, 1));
            if g.try_build(free_pad.unwrap()) {
                *built += 1;
                continue;
            }
        }
        match cheapest {
            // Alternate forks so both branches get exercised.
            Some((i, c)) if g.can_afford(c) => g.upgrade(i, Some(i % 2)),
            _ => break,
        }
    }
    g.build_choice = None;
    g.selected = None;
}

#[test]
fn a_sensible_build_clears_the_campaign() {
    let g = autoplay(true);
    println!(
        "MIXED  -> phase {:?} wave {} lives {} leaked {} towers {} maxed {} gold {} networth {}",
        g.phase,
        g.wave,
        g.lives,
        g.stats.leaked,
        g.towers.len(),
        g.towers.iter().filter(|t| t.tier >= MAX_TIER).count(),
        g.gold,
        g.net_worth(),
    );
    assert_eq!(
        g.phase,
        Phase::Victory,
        "a mixed board died on wave {} with {} lives and {} towers - the curve is too steep",
        g.wave,
        g.lives,
        g.towers.len()
    );
    // Won, but it must not have been a stroll. This bot does not read the wave
    // preview, does not place towers thoughtfully, and never sells a mistake -
    // if *it* finishes with most of its lives, a real player is asleep.
    assert!(
        g.lives < 15,
        "a naive board finished with {} of 20 lives - the campaign is not asking enough",
        g.lives
    );
    assert!(g.lives > 0 && g.stats.leaked > 0, "and it should have cost something");
    // Nearly everything earned should be on the board by the end. A large idle
    // pile means the last stretch had nothing left to decide.
    assert!(
        (g.gold as f32) < g.net_worth() as f32 * 0.15,
        "{} gold left idle against a {} net worth - the endgame has no sink",
        g.gold,
        g.net_worth()
    );
}

/// The other half of the same claim: a board that ignores the air must lose.
/// Without this, the ground/air split is decoration rather than a decision.
#[test]
fn ignoring_the_air_loses_the_run() {
    let g = autoplay(false);
    println!(
        "GROUND -> phase {:?} wave {} lives {} leaked {}",
        g.phase, g.wave, g.lives, g.stats.leaked
    );
    assert_ne!(
        g.phase,
        Phase::Victory,
        "a board with no anti-air cleared all {CAMPAIGN_WAVES} waves - the air layer means nothing"
    );
    // And it should die to the air specifically, not fall over immediately.
    assert!(
        g.wave >= 7,
        "the ground-only board died on wave {} - before air even arrives, so this proves nothing",
        g.wave
    );
}

// ---------------------------------------------------------------- economy

/// The economy must not run away.
///
/// This is a bug that shipped. Each Treasury added `0.04 * utility_scale(tier)`
/// to the interest rate, which at level 10 is +23.8% *each*, with no ceiling.
/// Four of them put compound interest over 100% a wave and gold doubled every
/// wave forever. A real game reached 813 billion gold on wave 89, maxed
/// everything permanently, and coasted to wave 136. Infinite money is the same
/// thing as no game.
///
/// So: build the greediest economy the game allows and check it stays finite.
#[test]
fn a_board_built_entirely_of_treasuries_cannot_run_away() {
    let mut g = rich_game();
    let mint = TOWERS.iter().position(|d| d.id == "mint").unwrap();
    let mut n = 0;
    for slot in 0..g.board.slots.len() {
        g.build_choice = Some((mint, 1));
        if g.try_build(slot) {
            let ti = g.towers.len() - 1;
            // Fork 0 is Treasury - the interest one.
            while g.towers[ti].tier < MAX_TIER {
                g.upgrade(ti, Some(0));
            }
            n += 1;
        }
    }
    g.build_choice = None;
    g.selected = None;
    assert!(n > 10, "only managed {n} Treasuries; this test needs a full board");

    assert!(
        g.interest_rate() <= INTEREST_MAX + 1e-6,
        "{n} Treasuries push interest to {:.0}%, over the {:.0}% cap",
        g.interest_rate() * 100.0,
        INTEREST_MAX * 100.0
    );

    // Hand it a fortune and run the economy for fifty waves with nothing being
    // spent. Interest is capped *and* only paid up to a ceiling, so the pile
    // must grow roughly linearly, not exponentially.
    g.gold = 1_000_000;
    let start = g.gold;
    for _ in 0..50 {
        g.wave += 1;
        g.end_wave_for_test();
    }
    let growth = g.gold as f64 / start as f64;
    assert!(
        growth < 20.0,
        "fifty idle waves multiplied the purse by {growth:.0}x - the economy compounds away"
    );
    assert!(g.gold > start, "an idle purse should still earn something");
}

/// Interest should reward banking a wave or two, and stop rewarding hoarding.
#[test]
fn interest_pays_on_a_bounded_pile() {
    let mut g = Game::new();
    g.wave = 30;

    let ceiling = g.interest_ceiling();
    assert!(ceiling > 0, "nothing earns interest at all");

    g.gold = ceiling / 2;
    let half = g.projected_interest();
    g.gold = ceiling;
    let full = g.projected_interest();
    g.gold = ceiling * 1_000;
    let absurd = g.projected_interest();

    assert!(half > 0 && full > half, "interest does not scale with a sensible balance");
    assert_eq!(full, absurd, "an enormous pile earns more than the ceiling allows");

    // And the ceiling has to keep up with the run, or banking stops mattering.
    let early = {
        let mut e = Game::new();
        e.wave = 5;
        e.interest_ceiling()
    };
    let late = {
        let mut l = Game::new();
        l.wave = CAMPAIGN_WAVES;
        l.interest_ceiling()
    };
    assert!(late > early * 20, "the interest ceiling does not grow with the waves");
}

/// Deep endless has to stay arithmetic. Payouts used to saturate `u32` around
/// wave 300 and every wave from then on paid exactly 4,294,967,295 gold.
#[test]
fn deep_endless_payouts_stay_finite() {
    for w in [100u32, 200, 300, 500, 900] {
        let d = wave_at(w);
        let clear = wave_clear_bonus(w);
        assert!(d.hp.is_finite() && d.hp > 0.0, "wave {w} health is {}", d.hp);
        assert!(d.bounty < u32::MAX, "wave {w} bounty saturated");
        assert!(clear < u32::MAX, "wave {w} clear bonus saturated");
        // And the wave still has to be getting harder faster than it pays.
        let pay = d.bounty as f64 * d.count as f64 + clear as f64;
        let hp = d.hp as f64 * d.count as f64;
        assert!(hp > pay, "wave {w} pays {pay:.0} for only {hp:.0} health - endless never ends");
    }
}

/// Hard control must be strong and finite.
///
/// A board of nothing but Frost and Grapeshot used to pin a wave in place
/// forever: every monster permanently stunned or shoved backwards, so nothing
/// died, nothing leaked, and the wave never ended. A full campaign hung on wave
/// 76. Stuns now diminish on repeat and knockback has a per-target cooldown, so
/// a control board slows a wave down instead of stopping time.
#[test]
fn a_wall_of_control_towers_cannot_freeze_a_wave_forever() {
    let mut g = rich_game();
    let frost = TOWERS.iter().position(|d| d.id == "frost").unwrap();
    let cannon = TOWERS.iter().position(|d| d.id == "cannon").unwrap();

    let mut n = 0;
    for slot in 0..g.board.slots.len() {
        // Glacier is the stun fork; Grapeshot is the knockback fork.
        let (def, fork) = if n % 2 == 0 { (frost, 0) } else { (cannon, 1) };
        g.build_choice = Some((def, 1));
        if g.try_build(slot) {
            let ti = g.towers.len() - 1;
            while g.towers[ti].tier < MAX_TIER {
                g.upgrade(ti, Some(fork));
            }
            n += 1;
        }
    }
    g.build_choice = None;
    g.selected = None;
    assert!(n > 20, "only built {n} control towers; this test needs a full board");

    // Monsters far too tough for this board to actually kill.
    let w = WaveDef { kind: Kind::Grunt, ..wave_at(40) };
    g.phase = Phase::Combat;
    let start_hp = 1.0e12;
    for _ in 0..10 {
        g.spawn_creep(&w, start_hp, 1.0, 0.0);
    }
    let before: Vec<f32> = g.creeps.iter().map(|c| c.dist).collect();
    run_for(&mut g, 90.0);

    // They cannot be killed, so they have to have made progress down the road.
    let moved = g
        .creeps
        .iter()
        .zip(before.iter())
        .filter(|(c, b)| c.dist > **b + 1.0)
        .count();
    assert!(
        moved > 0 || g.stats.leaked > 0 || g.creeps.is_empty(),
        "after 90 seconds nothing had advanced: the board has frozen time"
    );
    let furthest = g.creeps.iter().map(|c| c.dist).fold(0.0f32, f32::max);
    assert!(
        furthest > 8.0 || g.stats.leaked > 0,
        "the furthest monster is only {furthest:.1} tiles along after 90 seconds"
    );
}

/// Stuns on one target must get shorter, and recover when it is left alone.
#[test]
fn repeated_stuns_diminish_and_then_recover() {
    // Four stuns in a row, each landing on a more resistant target.
    let mut dr = 0.0f32;
    let mut lengths = Vec::new();
    for _ in 0..4 {
        lengths.push(1.0 * (1.0 - dr));
        dr = (dr + STUN_DR_STEP).min(STUN_DR_MAX);
    }
    for w in lengths.windows(2) {
        assert!(w[1] < w[0], "stuns are not diminishing: {lengths:?}");
    }
    assert!(*lengths.last().unwrap() > 0.0, "stun became a total immunity");
    assert!(dr <= STUN_DR_MAX + 1e-6);

    // Left alone, it recovers.
    let before = dr;
    for _ in 0..120 {
        dr = (dr - STUN_DR_DECAY / 60.0).max(0.0);
    }
    assert!(dr < before, "stun resistance never wears off");
}
