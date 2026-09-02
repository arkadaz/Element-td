//! Simulation soak tests.
//!
//! The risky part of this code base is index bookkeeping: creeps are removed with
//! `swap_remove` while projectiles, splash lists and pads hold indices into the
//! same vectors. These tests hammer those paths.

use super::board::{BUILD_FAR, Board, ROAD_HALF};
use super::defs::*;
use super::*;

/// A game with money and every element drafted to the hilt.
///
/// Most tests are about the simulation, not about the draft, and gating every
/// one of them behind an essence economy would only obscure what they check.
/// The draft itself is tested directly, further down.
fn rich_game() -> Game {
    let mut g = Game::new();
    g.gold = 5_000_000;
    unlock_all(&mut g);
    g
}

/// Enough essences of every element that every tower reaches [`MAX_TIER`].
fn unlock_all(g: &mut Game) {
    g.essence = [(MAX_TIER - FREE_TIERS) as u8; 6];
    g.pending_draft = None;
    g.drafts_taken = ESSENCE_WAVES.len();
}

/// Index of a tower by its id. Panics loudly, because a typo here silently
/// turns a real assertion into a test of the wrong tower.
fn t(id: &str) -> usize {
    TOWERS
        .iter()
        .position(|d| d.id == id)
        .unwrap_or_else(|| panic!("no tower {id:?}"))
}

/// Puts a tower on every free pad, cycling through the whole roster.
fn fill_pads(g: &mut Game, tier: u32) -> u32 {
    unlock_all(g);
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
        assert!(
            d <= BUILD_FAR + 0.01,
            "pad at {:?} is stranded (d={d})",
            s.pos
        );
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
    assert!(
        g.lives < before,
        "no tower on the board, yet nothing leaked"
    );
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
        assert!(
            c.hp.is_finite() && c.hp > 0.0,
            "dead monster left on the road"
        );
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
    // Silt has the widest blast in the game; put it next to the road.
    let cannon = t("silt");
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
    // Thornwall shoves; Abyss drags. Both move a monster the wrong way down the
    // road, and neither may ever push one off the front of it.
    for id in ["thornwall", "abyss"] {
        let def = t(id);
        g.build_choice = Some((def, MAX_TIER));
        assert!(g.try_build(nearest_slot_to_dist(&g, 6.0)));
        g.build_choice = None;
        assert!(
            g.towers[0]
                .specials()
                .iter()
                .any(|s| matches!(s, Special::Knockback { .. } | Special::Pull { .. })),
            "{id} should move its target backwards"
        );
        g.sell(0);
    }
    let thorn = t("thornwall");
    g.build_choice = Some((thorn, MAX_TIER));
    assert!(g.try_build(nearest_slot_to_dist(&g, 6.0)));
    g.build_choice = None;

    let w = g.wave_def(1);
    g.phase = Phase::Combat;
    g.spawn_creep(&w, 1.0e9, 1.0, 0.0);
    g.creeps[0].dist = 0.5;
    run_for(&mut g, 20.0);
    for c in &g.creeps {
        assert!(
            c.dist >= 0.0,
            "knockback pushed a monster off the start of the road"
        );
    }
}

// ---------------------------------------------------------------- data

#[test]
fn the_counter_triangle_holds() {
    // Physical bounces off plate and shreds wards; magic is the mirror image.
    assert!(armor_mult(Damage::Physical, Armor::Plated) < 1.0);
    assert!(armor_mult(Damage::Physical, Armor::Warded) > 1.0);
    assert!(armor_mult(Damage::Magic, Armor::Plated) > 1.0);
    assert!(armor_mult(Damage::Magic, Armor::Warded) < 1.0);
    // Ethereal is the new wall: it shrugs off everything physical and burns
    // badly, and magic is the only thing that answers it.
    assert!(armor_mult(Damage::Physical, Armor::Ethereal) < 1.0);
    assert!(armor_mult(Damage::Fire, Armor::Ethereal) < 1.0);
    assert!(armor_mult(Damage::Magic, Armor::Ethereal) > 1.0);
    // Fire is the swarm answer.
    assert!(armor_mult(Damage::Fire, Armor::Unarmoured) > 1.0);
    // Toxic is the dependable floor: never resisted, never bonus, anywhere.
    for a in [
        Armor::Unarmoured,
        Armor::Plated,
        Armor::Warded,
        Armor::Ethereal,
        Armor::Boss,
    ] {
        assert_eq!(
            armor_mult(Damage::Toxic, a),
            1.0,
            "toxic is not flat against {a:?}"
        );
    }
    // Bosses tax everything else.
    assert!(armor_mult(Damage::Physical, Armor::Boss) < 1.0);
    assert!(armor_mult(Damage::Magic, Armor::Boss) < 1.0);
    assert!(armor_mult(Damage::Fire, Armor::Boss) < 1.0);
}

#[test]
fn the_roster_is_exactly_six_pures_and_every_pair() {
    assert_eq!(
        TOWERS.len(),
        27 - 6,
        "twenty-one towers: six pure, fifteen dual"
    );

    // Every element has exactly one pure tower, at the index its enum says.
    for e in ELEMENTS {
        let d = &TOWERS[pure_index(e)];
        assert_eq!(
            d.elem,
            (e, None),
            "pure_index({e:?}) does not point at a pure tower"
        );
    }

    // Every unordered pair has exactly one dual tower, findable from either
    // side. This is what stops a pair being silently missing - a player who
    // drafted those two elements would simply never see a reward for it.
    let mut seen = std::collections::HashSet::new();
    for a in ELEMENTS {
        for b in ELEMENTS {
            if a == b {
                assert_eq!(dual_index(a, b), None, "an element paired with itself");
                continue;
            }
            let i = dual_index(a, b).unwrap_or_else(|| panic!("no dual for {a:?}+{b:?}"));
            assert_eq!(dual_index(b, a), Some(i), "dual lookup is order-dependent");
            seen.insert(i);
        }
    }
    assert_eq!(
        seen.len(),
        15,
        "the fifteen pairs do not map to fifteen distinct towers"
    );

    // Roles stay unique, and duals stay meaningfully more expensive than pures -
    // if a dual were not worth the second element, breadth would be free.
    let mut roles = std::collections::HashSet::new();
    let dearest_pure = TOWERS
        .iter()
        .filter(|d| !d.is_dual())
        .map(|d| d.cost)
        .max()
        .unwrap();
    let cheapest_dual = TOWERS
        .iter()
        .filter(|d| d.is_dual())
        .map(|d| d.cost)
        .min()
        .unwrap();
    for d in TOWERS.iter() {
        assert!(roles.insert(d.role), "two towers share the role {}", d.role);
    }
    assert!(
        cheapest_dual > dearest_pure * 2,
        "duals at {cheapest_dual}g are not a step up from pures at {dearest_pure}g"
    );
}

/// The essence ceiling is the whole strategic spine, so it gets its own test.
#[test]
fn essences_gate_what_can_be_built_and_how_far() {
    let mut g = Game::new();
    g.gold = 5_000_000;
    g.essence = [0; 6];
    g.pending_draft = None;

    let bramble = t("bramble");
    let wildfire = t("wildfire");

    // Nothing at all is buildable before the first draft.
    assert!(
        TOWERS.iter().enumerate().all(|(i, _)| !g.unlocked(i)),
        "a tower unlocked with no essences"
    );
    g.build_choice = Some((bramble, 1));
    assert!(!g.try_build(0), "built a tower with no essences");

    // One Nature opens the Nature pure at level 3, and nothing else.
    g.essence[Element::Nature.idx()] = 1;
    assert_eq!(g.tier_cap_of(bramble), FREE_TIERS + 1);
    assert_eq!(
        g.tier_cap_of(wildfire),
        0,
        "a dual unlocked from one element"
    );
    assert_eq!(g.missing_elements(wildfire), vec![Element::Fire]);

    // The pair opens the dual, capped by whichever element is scarcer.
    g.essence[Element::Fire.idx()] = 1;
    assert_eq!(g.tier_cap_of(wildfire), FREE_TIERS + 1);
    g.essence[Element::Nature.idx()] = 5;
    assert_eq!(g.tier_cap_of(bramble), FREE_TIERS + 5);
    assert_eq!(
        g.tier_cap_of(wildfire),
        FREE_TIERS + 1,
        "a dual read its larger element"
    );

    // Six of an element reaches the ceiling, and no further.
    g.essence = [6; 6];
    for (i, d) in TOWERS.iter().enumerate() {
        assert_eq!(
            g.tier_cap_of(i),
            MAX_TIER,
            "{} is not maxed by six of each",
            d.id
        );
    }
    g.essence = [20; 6];
    assert_eq!(
        g.tier_cap_of(bramble),
        MAX_TIER,
        "the ceiling is not a ceiling"
    );

    // Building above the ceiling is refused rather than clamped: a card that
    // offers level 5 and delivers level 3 is worse than one that refuses.
    g.essence = [1; 6];
    g.build_choice = Some((bramble, FREE_TIERS + 2));
    assert!(!g.try_build(0), "built above the essence ceiling");
    g.build_choice = Some((bramble, FREE_TIERS + 1));
    assert!(g.try_build(0), "could not build at the ceiling");

    // And upgrading stops there too, without taking the gold.
    while g.towers[0].tier < FREE_TIERS + 1 {
        g.upgrade(0);
    }
    let (tier, gold) = (g.towers[0].tier, g.gold);
    g.upgrade(0);
    assert_eq!(g.towers[0].tier, tier, "upgraded past the essence ceiling");
    assert_eq!(g.gold, gold, "a refused upgrade still charged for itself");
}

/// The offer must always be a real decision. An offer of three elements the
/// player already has plenty of, or three they have none of, is a coin toss
/// dressed as a choice.
#[test]
fn every_draft_offer_can_both_deepen_and_broaden() {
    for seed in 0..400u64 {
        let mut rng = crate::rng::Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
        // Walk a whole campaign of drafts, taking a different element each time
        // so the essence pool takes many different shapes.
        let mut essence = [0u8; 6];
        for step in 0..ESSENCE_WAVES.len() {
            let offer = draft_offer(&mut rng, &essence);

            let mut sorted = offer;
            sorted.sort();
            let mut dedup = sorted.to_vec();
            dedup.dedup();
            assert_eq!(
                dedup.len(),
                DRAFT_SIZE,
                "an offer repeated an element: {offer:?}"
            );

            let held = |e: Element| essence[e.idx()] > 0;
            let any_held = ELEMENTS.iter().any(|&e| held(e));
            let any_new = ELEMENTS.iter().any(|&e| !held(e));
            if any_held {
                assert!(
                    offer.iter().any(|&e| held(e)),
                    "seed {seed} step {step}: no way to deepen from {offer:?} holding {essence:?}"
                );
            }
            if any_new {
                assert!(
                    offer.iter().any(|&e| !held(e)),
                    "seed {seed} step {step}: no way to broaden from {offer:?} holding {essence:?}"
                );
            }
            let pick = offer[step % DRAFT_SIZE];
            essence[pick.idx()] += 1;
        }
    }
}

/// A run is reproducible from its seed alone - which is what lets multiplayer
/// hand every client the same number and nothing else.
#[test]
fn the_same_seed_offers_the_same_drafts() {
    let mut a = Game::new();
    let mut b = Game::new();
    a.start_run(0xA11CE);
    b.start_run(0xA11CE);
    assert_eq!(a.pending_draft, b.pending_draft);
    for _ in 0..ESSENCE_WAVES.len() {
        assert_eq!(
            a.pending_draft, b.pending_draft,
            "drafts diverged from one seed"
        );
        assert!(a.take_essence(0));
        assert!(b.take_essence(0));
        a.wave += 4;
        b.wave += 4;
        a.offer_draft_if_due();
        b.offer_draft_if_due();
    }
    assert_eq!(a.essence, b.essence);
}

/// Combat must not start while an essence is owed. A wave that arrives during
/// the decision turns the decision into a penalty.
#[test]
fn a_wave_cannot_start_while_an_essence_is_owed() {
    let mut g = Game::new();
    g.start_run(7);
    assert!(g.pending_draft.is_some(), "the run should open on a draft");

    let wave = g.wave;
    g.send_wave();
    assert_eq!(g.wave, wave, "a wave started with a draft pending");
    assert_eq!(g.phase, Phase::Build);

    // Nor may the build timer running out start one.
    g.build_timer = 0.01;
    run_for(&mut g, 3.0);
    assert_eq!(
        g.wave, wave,
        "the build timer started a wave with a draft pending"
    );

    assert!(g.take_essence(0));
    assert!(g.pending_draft.is_none());
    g.send_wave();
    assert_eq!(
        g.wave,
        wave + 1,
        "the wave still would not start after the draft"
    );
}

#[test]
fn shields_stop_everything_except_toxic() {
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

    let prism = t("prism");
    g.build_choice = Some((prism, 1));
    assert!(g.try_build(nearest_slot_to_dist(&g, 5.0)));
    g.build_choice = None;
    combat::damage_creep(&mut g, 0, 100.0, 0, false);
    assert!(g.creeps[0].shield < 200.0, "shield did not absorb");
    assert_eq!(
        g.creeps[0].hp, 500.0,
        "health should be untouched behind a shield"
    );

    let bramble = t("bramble");
    g.build_choice = Some((bramble, 1));
    assert!(g.try_build(nearest_slot_to_dist(&g, 9.0)));
    g.build_choice = None;
    let vi = g.towers.len() - 1;
    combat::damage_creep(&mut g, 0, 50.0, vi, false);
    assert!(g.creeps[0].hp < 500.0, "toxic should bypass the shield");
}

#[test]
fn groves_buff_the_cluster_and_stop_when_sold() {
    let mut g = rich_game();
    let ballista = t("prism");
    let beacon = t("grove");

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
        let reach = TOWERS[beacon].stats(1).range;
        let d = (g.board.slots[slot].pos[0] - g.towers[0].pos[0]).powi(2)
            + (g.board.slots[slot].pos[1] - g.towers[0].pos[1]).powi(2);
        if d < reach * reach && g.try_build(slot) {
            placed = true;
            break;
        }
    }
    g.build_choice = None;
    assert!(placed, "no pad near enough to test the aura");
    assert!(
        g.towers[0].dmg() > plain,
        "the grove did not buff its neighbour"
    );

    let bi = g.towers.iter().position(|t| t.is_support()).unwrap();
    g.sell(bi);
    let after = g.towers.iter().find(|t| !t.is_support()).unwrap().dmg();
    assert!((after - plain).abs() < 0.01, "the aura outlived the grove");
}

#[test]
fn every_tower_is_buildable_and_priced_sanely() {
    let mut g = Game::new();
    unlock_all(&mut g);
    for (i, d) in TOWERS.iter().enumerate() {
        assert!(d.range > 0.0, "{}", d.id);
        assert_eq!(
            g.max_tier_of(i),
            MAX_TIER,
            "{} is not maxed by a full essence pool",
            d.id
        );
        for tier in 1..=MAX_TIER {
            assert!(d.cost_at(tier) > 0);
            assert!(d.dps_at(tier).is_finite());
        }
        // The levels have to climb steeply enough to be worth taking, and a
        // maxed tower has to stay inside what a campaign actually pays out.
        assert!(
            d.cost_at(MAX_TIER) > d.cost_at(1) * 15,
            "{} barely gets more expensive",
            d.id
        );
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
    // The shop lists every tower exactly once: pures first, then duals,
    // cheapest first inside each group.
    let order = shop_order();
    assert_eq!(order.len(), TOWERS.len());
    let mut seen = order.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), TOWERS.len());
    for w in order.windows(2) {
        let (a, b) = (&TOWERS[w[0]], &TOWERS[w[1]]);
        assert!(
            (a.is_dual(), a.cost) <= (b.is_dual(), b.cost),
            "shop order breaks at {} -> {}",
            a.id,
            b.id
        );
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
            assert!(
                w.kind.is_boss(),
                "wave {} should be a boss, got {:?}",
                i + 1,
                w.kind
            );
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
        //
        // `WaveDef::speed` is already in tiles per second - it is the type's
        // walking multiplier *times* WALK_SPEED. Dividing by WALK_SPEED again
        // here understated every wave by a factor of 1.7 and reported an
        // hour-long campaign as fifty-three minutes.
        let walk = road / d.speed;
        let spawn = d.gap * (d.count.saturating_sub(1)) as f32;
        seconds += BUILD_TIME + walk + spawn;
    }
    let minutes = seconds / 60.0;
    println!("CAMPAIGN LENGTH: {minutes:.0} minutes");
    assert!(
        (60.0..110.0).contains(&minutes),
        "a full campaign takes {minutes:.0} minutes; the target is well over an hour"
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
    assert_eq!(
        g.phase,
        Phase::Build,
        "endless should roll straight into the next wave"
    );
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
                Special::Pull { .. } => "pull",
                Special::Execute { .. } => "execute",
                Special::Suppress => "suppress",
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

    // And every attacker must have a reason to exist beyond its damage number.
    // With twenty-one towers, several share a delivery and several share a
    // damage type - what none of them may share is all three of delivery,
    // damage type and effect. That is the line between a roster and a list.
    for (i, a) in TOWERS.iter().enumerate() {
        for b in TOWERS.iter().skip(i + 1) {
            if a.targets == Targets::Nothing || b.targets == Targets::Nothing {
                continue;
            }
            let same_delivery =
                std::mem::discriminant(&a.delivery) == std::mem::discriminant(&b.delivery);
            let same_effect = verb(a) == verb(b);
            assert!(
                !(a.dtype == b.dtype && same_delivery && same_effect),
                "{} and {} overlap: same damage type, delivery and effect",
                a.id,
                b.id
            );
        }
    }

    // Every element must appear on a tower that answers air and on one that
    // does not, or drafting it would be a coin toss on whether the run can
    // shoot upwards.
    for e in ELEMENTS {
        let mut air = false;
        for d in TOWERS.iter().filter(|d| d.elements().any(|x| x == e)) {
            air |= d.targets == Targets::Both;
        }
        assert!(air, "{e:?} unlocks nothing that can shoot at the air");
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
        vec!["boulder", "mire", "thornwall", "magma", "silt"],
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
        .map(|d| d.effective_dps_at(MAX_TIER))
        .fold(0.0, f32::max);
    let best_both: f32 = TOWERS
        .iter()
        .filter(|d| d.targets == Targets::Both && d.dtype != Damage::None)
        .map(|d| d.effective_dps_at(MAX_TIER))
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
    let cannon = t("boulder");
    let tesla = t("mirror");

    // One of each, on the two pads nearest the road.
    for (def, tier) in [(cannon, MAX_TIER), (tesla, MAX_TIER)] {
        let slot = g
            .board
            .slots
            .iter()
            .position(|s| s.tower.is_none())
            .unwrap();
        g.build_choice = Some((def, tier));
        assert!(g.try_build(slot));
    }
    g.build_choice = None;

    // Send a flying wave and let it walk the whole road.
    let w = WaveDef {
        kind: Kind::Wisp,
        ..wave_at(7)
    };
    g.phase = Phase::Combat;
    for _ in 0..10 {
        g.spawn_creep(&w, 4_000.0, 1.0, 2.0);
    }
    run_for(&mut g, 30.0);

    let cannon_kills: u32 = g
        .towers
        .iter()
        .filter(|t| t.def == cannon)
        .map(|t| t.kills)
        .sum();
    let cannon_dmg: f64 = g
        .towers
        .iter()
        .filter(|t| t.def == cannon)
        .map(|t| t.damage)
        .sum();
    let tesla_dmg: f64 = g
        .towers
        .iter()
        .filter(|t| t.def == tesla)
        .map(|t| t.damage)
        .sum();

    assert_eq!(
        cannon_kills, 0,
        "a ground-only tower killed something airborne"
    );
    assert!(
        cannon_dmg < 0.001,
        "a ground-only tower dealt {cannon_dmg} damage to the air"
    );
    assert!(
        tesla_dmg > 0.0,
        "the Mirror should have been shooting the whole time"
    );
}

/// And it must still be lethal on the ground, or the drawback has no upside.
#[test]
fn ground_towers_are_lethal_on_the_ground() {
    let mut g = rich_game();
    let cannon = t("boulder");
    let slot = g
        .board
        .slots
        .iter()
        .position(|s| s.tower.is_none())
        .unwrap();
    g.build_choice = Some((cannon, MAX_TIER));
    assert!(g.try_build(slot));
    g.build_choice = None;

    let w = WaveDef {
        kind: Kind::Grunt,
        ..wave_at(7)
    };
    g.phase = Phase::Combat;
    for _ in 0..10 {
        g.spawn_creep(&w, 400.0, 1.0, 2.0);
    }
    run_for(&mut g, 30.0);
    assert!(
        g.towers[0].damage > 0.0,
        "the Boulder never fired at a ground wave"
    );
}

/// Magma holds ground rather than tracking a target: it must leave fire behind
/// that hurts whatever walks into it, and must never light up the sky.
#[test]
fn magma_burns_the_road_and_only_the_road() {
    let mut g = rich_game();
    let pyre = t("magma");
    let slot = g
        .board
        .slots
        .iter()
        .position(|s| s.tower.is_none())
        .unwrap();
    g.build_choice = Some((pyre, MAX_TIER));
    assert!(g.try_build(slot));
    g.build_choice = None;

    let ground = WaveDef {
        kind: Kind::Grunt,
        ..wave_at(9)
    };
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
    assert!(ever_burning, "the Magma never lit the road");
    assert!(g.towers[0].damage > 0.0, "fire on the road did no damage");
    // The shred is the real payload - the damage is the smaller half.
    assert!(
        ever_shredded,
        "nothing standing in the fire was ever shredded"
    );

    // Now the same wave, airborne.
    let mut g2 = rich_game();
    let slot = g2
        .board
        .slots
        .iter()
        .position(|s| s.tower.is_none())
        .unwrap();
    g2.build_choice = Some((pyre, MAX_TIER));
    assert!(g2.try_build(slot));
    g2.build_choice = None;
    let air = WaveDef {
        kind: Kind::Wisp,
        ..wave_at(7)
    };
    g2.phase = Phase::Combat;
    for _ in 0..8 {
        g2.spawn_creep(&air, 20_000.0, 1.0, 2.0);
    }
    run_for(&mut g2, 12.0);
    assert!(
        g2.towers[0].damage < 0.001,
        "the Magma burned something airborne"
    );
}

/// Zones are the only thing in the game that leaks memory if nothing retires
/// them, and they are created several times a second per Pyre.
#[test]
fn burning_ground_expires() {
    let mut g = rich_game();
    let pyre = t("magma");
    for _ in 0..6 {
        let slot = g
            .board
            .slots
            .iter()
            .position(|s| s.tower.is_none())
            .unwrap();
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
    assert!(
        g.zones.len() < 400,
        "burning ground is piling up: {} zones",
        g.zones.len()
    );

    // With nothing left to burn they must all drain away.
    g.creeps.clear();
    run_for(&mut g, 12.0);
    assert!(
        g.zones.is_empty(),
        "{} zones outlived the wave",
        g.zones.len()
    );
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

/// A whole run, played end to end by a deliberately unsophisticated bot.
///
/// This is the balance harness. It drafts, builds, upgrades and sends waves the
/// way an inattentive player would - it never reads the wave preview, never
/// sells a mistake, and never places a tower thoughtfully beyond huddling near
/// the road. If *this* wins comfortably the campaign is not asking enough; if it
/// cannot win at all the campaign is asking too much.
///
/// `prefs` is the draft plan, most wanted first. `ground_only` is the trap
/// build: a bot that answers the road and nothing else.
fn autoplay(prefs: &[Element], ground_only: bool) -> Game {
    autoplay_traced(prefs, ground_only).0
}

/// One wave's outcome: its number, whether anything in it flew, and how many
/// lives it cost.
type WaveCost = (u32, bool, i32);

fn autoplay_traced(prefs: &[Element], ground_only: bool) -> (Game, Vec<WaveCost>) {
    let mut g = Game::new();
    g.start_run(0xE1E_7D0);
    let mut costs: Vec<WaveCost> = Vec::new();

    let mut built = 0usize;
    for _ in 0..(CAMPAIGN_WAVES + 8) {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        if g.phase == Phase::Build {
            // Take every essence owed before spending a coin - the draft can
            // unlock the tower this wave's gold is about to be poured into.
            while g.pending_draft.is_some() {
                take_preferred(&mut g, prefs);
            }
            spend(&mut g, ground_only, &mut built);
            if std::env::var("TD_TRACE").is_ok() && (g.wave + 1) % 10 == 0 {
                let w = g.next_wave_def();
                let dps: f32 = g.towers.iter().map(|t| t.dmg() * t.rate()).sum();
                println!(
                    "  w{:>3} | {:>2} towers {:>9.0} dps | next {:?} x{} = {:>10.0} ehp | lives {:>2} gold {:>10} | ess {:?}",
                    g.wave + 1,
                    g.towers.len(),
                    dps,
                    w.kind,
                    w.count,
                    w.hp * w.count as f32,
                    g.lives,
                    g.gold,
                    g.essence
                );
            }
            g.send_wave();
            assert_ne!(
                g.phase,
                Phase::Build,
                "the bot failed to start wave {}",
                g.wave + 1
            );
        }
        let before = g.lives;
        let flying = g.wave_def(g.wave).has_air();
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
        if g.phase == Phase::Combat {
            let mut lines = String::new();
            for c in g.creeps.iter().take(6) {
                lines.push_str(&format!(
                    "\n    {:?} dist {:.1}/{:.1} spd {:.2}/{:.2} hp {:.0}/{:.0} stun {:.2} slow {:.2} heal {:.3}",
                    c.kind, c.dist, g.board.total, c.speed(), c.base_speed,
                    c.hp, c.max_hp, c.stun, c.slow.amt, c.heal
                ));
            }
            panic!(
                "wave {} never ended: {} alive after 200s, {} lives, {} leaked, spawn_left {}{}",
                g.wave,
                g.creeps.len(),
                g.lives,
                g.stats.leaked,
                g.spawn_left,
                lines
            );
        }
        costs.push((g.wave, flying, before - g.lives));
    }
    (g, costs)
}

/// Drafts towards an even spread across the planned elements.
///
/// Prefers any planned element, and among those the one it holds *fewest* of -
/// which converges on 5/5/5/5 across four elements rather than piling into
/// whichever happened to be offered first. Taking the plan's first choice every
/// time produced a three-element board, which is a different strategy being
/// measured under the wrong name.
fn take_preferred(g: &mut Game, prefs: &[Element]) {
    let Some(offer) = g.pending_draft else { return };
    let planned = |e: Element| prefs.contains(&e);
    let rank = |e: Element| prefs.iter().position(|&x| x == e).unwrap_or(prefs.len());
    // Planned first, then whichever is held least, then plan order. The last
    // term only breaks ties, but it decides the opening - and the opening is
    // where a build that needs one specific element to have anything at all to
    // put down either works or starves.
    let key = |e: Element| (!planned(e), g.essence[e.idx()], rank(e));
    let mut best = 0usize;
    for i in 1..offer.len() {
        if key(offer[i]) < key(offer[best]) {
            best = i;
        }
    }
    assert!(
        g.take_essence(best),
        "the bot could not take an offered essence"
    );
}

/// Empties the purse: fill the board out to a working size, then pour
/// everything into levels, then spread again once there is nothing left to
/// upgrade.
/// Below this many towers the bot widens greedily; above it, only with money to
/// spare, so that levels and pads compete for the same gold as they do for a
/// player.
const BOT_CORE: usize = 10;

fn spend(g: &mut Game, ground_only: bool, built: &mut usize) {
    loop {
        // What the draft has actually unlocked, best value first. Recomputed
        // every pass, because a draft changes both the list and its order.
        //
        // Ordered by damage *per gold at its own ceiling*, not by raw damage.
        // Sorting by raw damage makes the bot buy the most expensive dual in
        // the roster at level one and then have nothing left to level it with,
        // which is not an unsophisticated strategy so much as a broken one.
        let mut want: Vec<usize> = (0..TOWERS.len())
            .filter(|&i| g.unlocked(i))
            .filter(|&i| !ground_only || TOWERS[i].targets != Targets::Both)
            .collect();
        if want.is_empty() {
            break;
        }
        let value = |i: usize| {
            let cap = g.tier_cap_of(i).max(1);
            TOWERS[i].effective_dps_at(cap) / TOWERS[i].cost_at(cap) as f32
        };
        want.sort_by(|&a, &b| value(b).total_cmp(&value(a)));

        let cheapest = (0..g.towers.len())
            .filter_map(|i| g.upgrade_cost_of(i).map(|c| (i, c)))
            .min_by_key(|(_, c)| *c);
        let free_pad = spread_pad(g);
        let def = want[*built % want.len()];
        let cost = TOWERS[def].cost_at(1) as i64;
        // Widen freely up to a core board, then whenever there is gold to spare
        // on top of what levels are eating.
        //
        // There used to be a hard board size that unlocked entirely once every
        // tower hit its ceiling, and that step made the whole harness
        // non-monotonic: a run that maxed out a wave earlier sprawled to the
        // edge of the board and cruised, and one that maxed out a wave later
        // stalled at half the towers and died. Measuring a curve with an
        // instrument that has a cliff in it measures the cliff.
        // Board size tracks the wave, and everything else goes into levels.
        //
        // Two rules were tried before this one. "Widen only once every tower is
        // maxed" put a cliff in the harness and made it non-monotonic. "Widen
        // whenever there is spare gold" is worse: a tier-one tower is always
        // affordable, so the bot papered the board with ninety-nine level-one
        // towers and never levelled anything. A player does neither - they add
        // a pad every wave or two and feed the rest to what they already own.
        let target = (BOT_CORE + g.wave as usize).min(g.board.slots.len());
        let want_new = free_pad.is_some() && *built < target;

        if want_new && g.can_afford(cost as u32) {
            g.build_choice = Some((def, 1));
            if g.try_build(free_pad.unwrap()) {
                *built += 1;
                continue;
            }
        }
        match cheapest {
            Some((i, c)) if g.can_afford(c) => {
                let before = g.towers[i].tier;
                g.upgrade(i);
                // A refused upgrade would spin this loop forever.
                if g.towers[i].tier == before {
                    break;
                }
            }
            _ => break,
        }
    }
    g.build_choice = None;
    g.selected = None;
}

/// A four-element spread: broad enough to unlock six duals, deep enough to take
/// them past level five. This is the shape a thoughtful player converges on.
const SENSIBLE_DRAFT: [Element; 4] = [
    Element::Light,
    Element::Water,
    Element::Fire,
    Element::Nature,
];

/// The trap: a broad draft spent entirely on towers that cannot elevate.
///
/// Deliberately four elements, not two. Two would starve the board of damage
/// long before the first flying wave, and a run that dies on wave seven proves
/// nothing about the air layer. This build is genuinely strong on the road.
/// Earth first, and then the three elements that pair with it into the rest of
/// the ground-only towers.
///
/// Four of the five towers that cannot elevate are Earth towers, so a board
/// built to hold the road is an Earth board. Earth leads because Boulder is the
/// only ground-only *pure*: without it this build has nothing at all to put
/// down in the opening waves, and a run that dies on wave seven for lack of any
/// tower proves nothing about the air.
const GROUND_DRAFT: [Element; 4] = [
    Element::Earth,
    Element::Nature,
    Element::Water,
    Element::Fire,
];

/// The other trap: deep in two elements and blind everywhere else. It reaches
/// level eight on three towers and has an answer to nothing it did not draft.
const NARROW_DRAFT: [Element; 2] = [Element::Water, Element::Light];

// ---------------------------------------------------------------- playthrough

/// A narrated run, played the way a person who reads the screen would play it.
///
/// The balance tests above use a deliberately unsophisticated bot, because the
/// question they ask is "does the campaign beat someone who is not paying
/// attention". This one asks the opposite question - what does the game feel
/// like when it is played properly - and prints the whole run so it can be read
/// rather than only asserted on.
///
/// Run it with:
///     cargo test --release a_narrated_playthrough -- --ignored --nocapture
#[test]
#[ignore = "prints a whole campaign; run it deliberately"]
fn a_narrated_playthrough() {
    let mut g = Game::new();
    g.start_run(0x5CA1_AB1E);
    let mut built = 0usize;
    let mut worst: Vec<(u32, String, i32)> = Vec::new();

    println!();
    println!("=========================== ELEMENTAL TD ===========================");
    println!("  seed {:#x}   {} lives   {} gold", g.seed, g.lives, g.gold);

    for _ in 0..(CAMPAIGN_WAVES + 4) {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        while g.pending_draft.is_some() {
            draft_thoughtfully(&mut g);
        }
        spend_thoughtfully(&mut g, &mut built);

        let w = g.next_wave_def();
        let lives_before = g.lives;
        let label = format!(
            "{:?} x{}{}",
            w.kind,
            w.count,
            w.escort
                .map_or(String::new(), |e| format!(" + {:?} x{}", e.kind, e.count))
        );

        if (g.wave + 1) % 10 == 0 || g.wave < 3 {
            println!();
            println!(
                "-- wave {:>2} --  {label}   [{}{}]",
                g.wave + 1,
                w.armor().name(),
                if w.has_air() { ", FLYING" } else { "" }
            );
            println!("   board: {}", board_summary(&g));
            println!(
                "   essences: {}   gold {}   lives {}",
                essence_summary(&g),
                g.gold,
                g.lives
            );
        }

        g.send_wave();
        let dt = 1.0 / 60.0;
        let mut elapsed = 0.0;
        while g.phase == Phase::Combat && elapsed < 200.0 {
            g.update(dt);
            elapsed += dt;
        }
        assert_ne!(g.phase, Phase::Combat, "wave {} never ended", g.wave);

        let lost = lives_before - g.lives;
        if lost > 0 {
            println!(
                "   !! wave {:>2} {label} cost {lost} lives ({} left)",
                g.wave, g.lives
            );
            worst.push((g.wave, label, lost));
        }
    }

    println!();
    println!("=========================== RESULT ===========================");
    println!(
        "  {:?} on wave {}, {} of {START_LIVES} lives, {} kills, {} leaked",
        g.phase, g.wave, g.lives, g.stats.kills, g.stats.leaked
    );
    println!("  board:    {}", board_summary(&g));
    println!("  essences: {}", essence_summary(&g));
    worst.sort_by_key(|(_, _, n)| -n);
    println!("  hardest waves:");
    for (wave, label, lost) in worst.iter().take(6) {
        println!("     wave {wave:>2}  {label}  -{lost} lives");
    }
    // Deliberately not asserting a win. This bot is a hand-written model of a
    // player, and tuning the game until *it* wins would be fitting the game to
    // the model rather than the other way round - the campaign's difficulty is
    // claimed by `a_sensible_build_clears_the_campaign`, which uses a fixed,
    // deliberately unsophisticated strategy that does not move.
    //
    // What this run does assert is that a plausibly-played campaign is a game
    // rather than a wall: it must get past the point where the roster is fully
    // open and every mechanic has been introduced.
    assert!(
        g.wave >= 30,
        "a board built by reading the screen died on wave {} - something is unanswerable",
        g.wave
    );
    assert!(
        worst.iter().all(|(_, _, lost)| *lost <= START_LIVES / 2),
        "one wave took half the run: {:?}",
        worst.iter().max_by_key(|(_, _, n)| *n)
    );
}

fn essence_summary(g: &Game) -> String {
    ELEMENTS
        .iter()
        .filter(|e| g.essence[e.idx()] > 0)
        .map(|e| format!("{}{}", e.glyph(), g.essence[e.idx()]))
        .collect::<Vec<_>>()
        .join(" ")
}

fn board_summary(g: &Game) -> String {
    let mut kinds: Vec<(&str, usize, u32)> = Vec::new();
    for t in &g.towers {
        let name = t.def().name;
        match kinds.iter_mut().find(|(n, _, _)| *n == name) {
            Some((_, n, top)) => {
                *n += 1;
                *top = (*top).max(t.tier);
            }
            None => kinds.push((name, 1, t.tier)),
        }
    }
    kinds.sort_by_key(|(_, n, _)| std::cmp::Reverse(*n));
    let dps: f32 = g.towers.iter().map(|t| t.dmg() * t.rate()).sum();
    let list = kinds
        .iter()
        .take(6)
        .map(|(n, c, top)| format!("{c}x{n}(L{top})"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{} towers, {:.0} dps  {list}", g.towers.len(), dps)
}

/// Which damage types the board can actually field.
fn types_covered(g: &Game) -> Vec<Damage> {
    let mut out = Vec::new();
    for t in &g.towers {
        let d = t.dtype();
        if d != Damage::None && !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// What fraction of the board's damage a given damage type accounts for.
fn share_of(g: &Game, d: Damage) -> f32 {
    let total: f32 = g.towers.iter().map(|t| t.dmg() * t.rate()).sum();
    if total <= 0.0 {
        return 0.0;
    }
    let mine: f32 = g
        .towers
        .iter()
        .filter(|t| t.dtype() == d)
        .map(|t| t.dmg() * t.rate())
        .sum();
    mine / total
}

/// What fraction of the board's damage can reach a flying target.
fn air_share(g: &Game) -> f32 {
    let total: f32 = g.towers.iter().map(|t| t.dmg() * t.rate()).sum();
    if total <= 0.0 {
        return 0.0;
    }
    let air: f32 = g
        .towers
        .iter()
        .filter(|t| t.def().targets == Targets::Both)
        .map(|t| t.dmg() * t.rate())
        .sum();
    air / total
}

/// Drafts to cover a hole first, then to deepen what is already carrying the
/// board - which is roughly how a person who reads the tooltips would play.
fn draft_thoughtfully(g: &mut Game) {
    let Some(offer) = g.pending_draft else { return };
    let have = types_covered(g);
    // A player who has read the help text commits to about four elements and
    // then deepens them. Both extremes are traps the design is built to punish:
    // six elements caps everything at level five, and two leaves an armour
    // class with no answer at all.
    const COMMIT: usize = 4;
    let distinct = g.essence.iter().filter(|&&n| n > 0).count();
    let mut best = 0usize;
    let mut best_score = f32::MIN;

    for (i, &e) in offer.iter().enumerate() {
        let mut after = g.essence;
        after[e.idx()] += 1;
        let mut score = 0.0f32;
        for (ti, d) in TOWERS.iter().enumerate() {
            let before = g.tier_cap_of(ti);
            let now = tier_cap(&after, d);
            if now > before {
                // Unlocking something outright is worth much more than one more
                // level of something already owned.
                // Raising the ceiling of something already standing on the
                // board is worth far more than unlocking something that would
                // still have to be paid for - and spreading into a sixth
                // element caps everything at level five, which is how a board
                // ends up with an answer to everything and the numbers to kill
                // nothing.
                let built = g.towers.iter().filter(|t| t.def == ti).count();
                score += built as f32 * 1.5;
                score += if before == 0 { 1.5 } else { 0.5 };
                // And a damage type the board cannot currently field is the
                // single most valuable thing a draft can buy.
                if before == 0 && d.dtype != Damage::None && !have.contains(&d.dtype) {
                    score += 10.0;
                }
            }
        }
        // And depth is worth pursuing for its own sake once the answers exist.
        score += g.essence[e.idx()] as f32 * 1.2;
        // Refuse to open a fifth or sixth front. Spreading is only correct
        // while there are still armour classes with nothing pointed at them.
        if g.essence[e.idx()] == 0 && distinct >= COMMIT {
            score -= 40.0;
        }

        if score > best_score {
            best_score = score;
            best = i;
        }
    }

    let taken = offer[best];
    let before_unlocked: Vec<&str> = TOWERS
        .iter()
        .enumerate()
        .filter(|(i, _)| g.unlocked(*i))
        .map(|(_, d)| d.name)
        .collect();
    assert!(g.take_essence(best));
    let now_unlocked: Vec<&str> = TOWERS
        .iter()
        .enumerate()
        .filter(|(i, _)| g.unlocked(*i))
        .map(|(_, d)| d.name)
        .collect();
    let fresh: Vec<&&str> = now_unlocked
        .iter()
        .filter(|n| !before_unlocked.contains(n))
        .collect();

    println!(
        "   draft {}: [{}] -> {} {}",
        g.drafts_taken,
        offer
            .iter()
            .map(|e| e.name())
            .collect::<Vec<_>>()
            .join(", "),
        taken.name(),
        if fresh.is_empty() {
            "(deepens)".to_string()
        } else {
            format!(
                "(unlocks {})",
                fresh.iter().map(|s| **s).collect::<Vec<_>>().join(", ")
            )
        }
    );
}

/// Buys the missing answer first, then levels whatever is cheapest.
fn spend_thoughtfully(g: &mut Game, built: &mut usize) {
    loop {
        let have = types_covered(g);
        let mut want: Vec<usize> = (0..TOWERS.len()).filter(|&i| g.unlocked(i)).collect();
        if want.is_empty() {
            break;
        }
        // Value per gold, then corrected for what the board is short of.
        //
        // Raw value alone produces a monoculture: whichever tower happens to
        // win on paper gets built fifty times, which is neither how anyone
        // plays nor a fair test of the roster. A player spreads because the
        // next wave might be the one their single answer cannot hurt.
        let air = air_share(g);
        let value = |i: usize| {
            let cap = g.tier_cap_of(i).max(1);
            let d = &TOWERS[i];
            let mut v = d.effective_dps_at(cap) / d.cost_at(cap) as f32;
            if d.dtype != Damage::None {
                if !have.contains(&d.dtype) {
                    // A damage type nothing covers is worth overpaying for.
                    v *= 4.0;
                } else {
                    // And one the board already leans on is worth less than
                    // its number says.
                    v *= 1.0 - share_of(g, d.dtype) * 0.8;
                }
            }
            // Anything that answers the air, while the board mostly cannot.
            if d.targets == Targets::Both && air < 0.55 {
                v *= 1.0 + (0.55 - air) * 3.0;
            }
            // Diminishing returns on a tower already owned. Nothing else in
            // this model captures overkill - two towers covering one bend do
            // not kill twice as much - and without it the bot buys thirty-four
            // copies of whatever wins on paper, which is neither how anyone
            // plays nor a fair test of the roster.
            let owned = g.towers.iter().filter(|t| t.def == i).count();
            v /= 1.0 + owned as f32 * 0.25;
            v
        };
        want.sort_by(|&a, &b| value(b).total_cmp(&value(a)));

        let cheapest = (0..g.towers.len())
            .filter_map(|i| g.upgrade_cost_of(i).map(|c| (i, c)))
            .min_by_key(|(_, c)| *c);
        let free_pad = spread_pad(g);
        let def = want[0];
        let target = (BOT_CORE + g.wave as usize).min(g.board.slots.len());

        if free_pad.is_some() && *built < target && g.can_afford(TOWERS[def].cost_at(1)) {
            g.build_choice = Some((def, 1));
            if g.try_build(free_pad.unwrap()) {
                *built += 1;
                continue;
            }
        }
        match cheapest {
            Some((i, c)) if g.can_afford(c) => {
                let before = g.towers[i].tier;
                g.upgrade(i);
                if g.towers[i].tier == before {
                    break;
                }
            }
            _ => break,
        }
    }
    g.build_choice = None;
    g.selected = None;
}

/// Breadth has to be worth something. Two elements reach level eight on three
/// towers, which is the most damage per gold available - if that also wins, the
/// whole draft is decoration and the counter table means nothing.
#[test]
fn two_elements_are_not_enough_however_deep_they_go() {
    let g = autoplay(&NARROW_DRAFT, false);
    println!(
        "NARROW -> phase {:?} wave {} lives {} leaked {} towers {} maxed {}",
        g.phase,
        g.wave,
        g.lives,
        g.stats.leaked,
        g.towers.len(),
        g.towers.iter().filter(|t| t.tier >= MAX_TIER).count()
    );
    assert_ne!(
        g.phase,
        Phase::Victory,
        "a two-element board cleared all {CAMPAIGN_WAVES} waves with {} lives - breadth in the \
         draft buys nothing",
        g.lives
    );
    // It should get a long way, though. A narrow build is meant to be a real
    // strategy that runs out of answers, not an obvious mistake.
    assert!(
        g.wave >= 25,
        "the two-element board died on wave {} - that is not a trap, it is a wall",
        g.wave
    );
}

#[test]
fn a_sensible_build_clears_the_campaign() {
    let g = autoplay(&SENSIBLE_DRAFT, false);
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
    assert!(
        g.lives > 0 && g.stats.leaked > 0,
        "and it should have cost something"
    );
    // Nearly everything earned should be on the board by the end. A large idle
    // pile means the last stretch had nothing left to decide.
    // Some late surplus is now *intended*, which it was not before the draft
    // existed. A board whose towers have all reached their essence ceiling has
    // nothing left to buy but more pads, and that is the price of having
    // drafted wide - the gold sitting there is the player being shown, in the
    // most concrete way available, what a fifth element cost them. A fifth of
    // net worth is where that stops being a lesson and starts being a hole.
    assert!(
        (g.gold as f32) < g.net_worth() as f32 * 0.20,
        "{} gold left idle against a {} net worth - the endgame has no sink at all",
        g.gold,
        g.net_worth()
    );
}

/// The other half of the same claim: a board that cannot shoot upwards must
/// lose, and must lose **because** of that.
///
/// Two things are needed to show causation rather than correlation, because
/// each alone can be explained away:
///
///   - a **control** - the same draft, the same gold, the same everything, with
///     the one restriction lifted. If the control does not get far further,
///     what killed the run was weakness, not the sky.
///   - a **breakdown** of which waves actually took the lives. A board short of
///     damage bleeds on the ground waves too.
#[test]
fn ignoring_the_air_loses_the_run() {
    let (g, costs) = autoplay_traced(&GROUND_DRAFT, true);
    let control = autoplay(&GROUND_DRAFT, false);
    let lost_to_air: i32 = costs
        .iter()
        .filter(|(_, air, _)| *air)
        .map(|(_, _, n)| n)
        .sum();
    let lost_to_ground: i32 = costs
        .iter()
        .filter(|(_, air, _)| !*air)
        .map(|(_, _, n)| n)
        .sum();
    println!(
        "GROUND -> phase {:?} wave {} lives {} | lost {lost_to_air} to air, {lost_to_ground} to \
         ground | control reached wave {}",
        g.phase, g.wave, g.lives, control.wave
    );

    assert_ne!(
        g.phase,
        Phase::Victory,
        "a board with no anti-air cleared all {CAMPAIGN_WAVES} waves - the air layer means nothing"
    );
    // It has to hold the road until something flies. The first flying wave is
    // wave 7, and it arrives softened.
    assert!(
        g.wave >= 7,
        "the ground-only board died on wave {} - before air even arrives, so this proves nothing \
         except that it had no damage",
        g.wave
    );
    // The same build with the air answered must get far further.
    assert!(
        control.wave > g.wave * 2,
        "the same draft reached wave {} with anti-air and wave {} without - not much of a lesson",
        control.wave,
        g.wave
    );
    // And the flying waves must be what actually took the lives.
    assert!(
        lost_to_air > lost_to_ground,
        "the ground-only board lost {lost_to_air} lives to flying waves and {lost_to_ground} to \
         walking ones - it died of weakness, not of having no answer to the sky"
    );
}

#[test]
fn a_board_built_entirely_of_treasuries_cannot_run_away() {
    let mut g = rich_game();
    let mint = t("tombstone");
    let mut n = 0;
    for slot in 0..g.board.slots.len() {
        g.build_choice = Some((mint, 1));
        if g.try_build(slot) {
            let ti = g.towers.len() - 1;
            while g.towers[ti].tier < MAX_TIER {
                g.upgrade(ti);
            }
            n += 1;
        }
    }
    g.build_choice = None;
    g.selected = None;
    assert!(
        n > 10,
        "only managed {n} Tombstones; this test needs a full board"
    );

    assert!(
        g.interest_rate() <= INTEREST_MAX + 1e-6,
        "{n} Tombstones push interest to {:.0}%, over the {:.0}% cap",
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

    assert!(
        half > 0 && full > half,
        "interest does not scale with a sensible balance"
    );
    assert_eq!(
        full, absurd,
        "an enormous pile earns more than the ceiling allows"
    );

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
    assert!(
        late > early * 20,
        "the interest ceiling does not grow with the waves"
    );
}

/// Deep endless has to stay arithmetic. Payouts used to saturate `u32` around
/// wave 300 and every wave from then on paid exactly 4,294,967,295 gold.
#[test]
fn deep_endless_payouts_stay_finite() {
    for w in [100u32, 200, 300, 500, 900] {
        let d = wave_at(w);
        let clear = wave_clear_bonus(w);
        assert!(
            d.hp.is_finite() && d.hp > 0.0,
            "wave {w} health is {}",
            d.hp
        );
        assert!(d.bounty < u32::MAX, "wave {w} bounty saturated");
        assert!(clear < u32::MAX, "wave {w} clear bonus saturated");
        // And the wave still has to be getting harder faster than it pays.
        let pay = d.bounty as f64 * d.count as f64 + clear as f64;
        let hp = d.hp as f64 * d.count as f64;
        assert!(
            hp > pay,
            "wave {w} pays {pay:.0} for only {hp:.0} health - endless never ends"
        );
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
    // Eclipse is the only stun in the roster; Boulder is the knockback.
    let frost = t("eclipse");
    let cannon = t("boulder");

    let mut n = 0;
    for slot in 0..g.board.slots.len() {
        // Eclipse stuns, Boulder shoves. Alternating them puts both kinds of
        // hard control on the same stretch of road, which is the arrangement
        // that used to freeze a wave forever.
        let def = if n % 2 == 0 { frost } else { cannon };
        g.build_choice = Some((def, 1));
        if g.try_build(slot) {
            let ti = g.towers.len() - 1;
            while g.towers[ti].tier < MAX_TIER {
                g.upgrade(ti);
            }
            n += 1;
        }
    }
    g.build_choice = None;
    g.selected = None;
    assert!(
        n > 20,
        "only built {n} control towers; this test needs a full board"
    );

    // Monsters far too tough for this board to actually kill.
    let w = WaveDef {
        kind: Kind::Grunt,
        ..wave_at(40)
    };
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
    assert!(
        *lengths.last().unwrap() > 0.0,
        "stun became a total immunity"
    );
    assert!(dr <= STUN_DR_MAX + 1e-6);

    // Left alone, it recovers.
    let before = dr;
    for _ in 0..120 {
        dr = (dr - STUN_DR_DECAY / 60.0).max(0.0);
    }
    assert!(dr < before, "stun resistance never wears off");
}
