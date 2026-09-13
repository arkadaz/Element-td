//! Simulation soak tests.
//!
//! Two jobs. The first is index bookkeeping: creeps are removed with
//! `swap_remove` while projectiles, splash lists and pads hold indices into the
//! same vectors, and these tests hammer those paths. The second is that the
//! game the map describes is actually playable - that a sensible board clears
//! thirty-six waves, that a board with no answer to the air does not, and that
//! nothing can wedge a wave open forever.

use super::defs::*;
use super::*;

/// A game with more money than any board can spend.
fn rich_game() -> Game {
    let mut g = Game::new();
    g.gold = 500_000_000;
    g
}

/// Index of a tower by the map's own name. Panics loudly, because a typo here
/// silently turns a real assertion into a test of the wrong tower.
fn t(name: &str) -> usize {
    TOWERS
        .iter()
        .position(|d| d.name == name)
        .unwrap_or_else(|| panic!("no tower {name:?}"))
}

/// The first rung of a family.
fn root(f: Family) -> usize {
    family_start(f).unwrap_or_else(|| panic!("no {f:?} family"))
}

/// Walks a tower as far up its own path as it will go, taking the first branch
/// at every fork.
fn max_out(g: &mut Game, ti: usize) {
    for _ in 0..40 {
        let before = g.towers[ti].def;
        let Some(&(into, _)) = g.upgrade_choices(ti).first() else {
            break;
        };
        g.upgrade_into(ti, into);
        if g.towers[ti].def == before {
            break;
        }
    }
}

/// Builds one tower of `family` on the free pad nearest `along` tiles down the
/// lane, and returns its index.
///
/// By position rather than by pad number, because a pad index says nothing
/// about where on the compact multi-lane circuit a tower stands - twelve
/// towers on pads 0..12 all ended up at one end of the map, watching an empty
/// corridor while the test measured the other end.
fn build(g: &mut Game, family: Family, along: f32) -> usize {
    let at = g.board.sample(along);
    let slot = (0..g.board.slots.len())
        .filter(|&i| g.board.slots[i].tower.is_none())
        .min_by(|&a, &b| {
            let d = |i: usize| {
                let p = g.board.slots[i].pos;
                (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2)
            };
            d(a).total_cmp(&d(b))
        })
        .expect("a free pad");
    g.build_choice = Some((root(family), 1));
    assert!(g.try_build(slot), "could not build {family:?} at {along}");
    g.build_choice = None;
    g.towers.len() - 1
}

/// A handful of towers of one family, spread along the first stretch of lane.
fn line_of(g: &mut Game, family: Family, n: usize) -> Vec<usize> {
    (0..n)
        .map(|k| {
            let ti = build(g, family, k as f32 * 2.5);
            max_out(g, ti);
            ti
        })
        .collect()
}

/// Puts a tower on every free pad, cycling through the shop.
fn fill_pads(g: &mut Game) -> u32 {
    let shop = shop_order();
    let mut built = 0;
    for slot in 0..g.board.slots.len() {
        g.build_choice = Some((shop[built as usize % shop.len()], 1));
        if g.try_build(slot) {
            built += 1;
        }
    }
    g.build_choice = None;
    g.selected = None;
    built
}

/// Stops the wave clock and empties the ring, so a test can put exactly what it
/// wants on the circuit and measure only that.
///
/// The clock never stops on its own - there is no build phase to borrow.
/// Without this, half the combat tests were quietly measuring the real wave
/// arriving behind whatever they had spawned by hand, and one of them concluded
/// that a ground-only tower had shot down five flyers.
fn isolate(g: &mut Game) {
    g.phase = Phase::Combat;
    g.prep = false;
    g.creeps.clear();
    g.spawn_left = 0;
    // Far enough out that no test runs long enough to reach it.
    g.wave_timer = 1.0e6;
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

/// A wave definition built by hand, so a test can ask for exactly one kind of
/// monster without hunting the campaign for a wave that happens to match.
fn creep(hp: f32, armour: i32, kind: ArmourType, flying: bool) -> WaveDef {
    WaveDef {
        name: "test",
        tag: "",
        model: if flying {
            Model::Dragon
        } else {
            Model::Warrior
        },
        scale: 1.0,
        count: 1,
        hp,
        armour,
        armour_type: kind,
        speed: 1.6,
        flying,
        spawn_gap: 1.0,
        lead_in: 45.0,
    }
}

// ---------------------------------------------------------------- board

/// The road is a closed circuit, and that is load-bearing: every rule about
/// leaking, lives and losing follows from there being no exit.
#[test]
fn the_road_is_a_closed_circuit_with_no_exit() {
    let b = super::board::Board::new();
    assert!(b.total > 30.0, "circuit is only {} tiles round", b.total);

    // Walking the whole length returns you to where you started.
    let a = b.sample(0.0);
    let z = b.sample(b.total);
    let gap = ((a[0] - z[0]).powi(2) + (a[1] - z[1]).powi(2)).sqrt();
    assert!(
        gap < 0.05,
        "the circuit does not close: {gap:.3} tiles apart"
    );

    // And the heading is continuous across the seam, or monsters would spin on
    // the spot every lap.
    let h0 = b.heading(0.02);
    let h1 = b.heading(b.total - 0.02);
    let dot = h0[0] * h1[0] + h0[1] * h1[1];
    assert!(dot > 0.9, "the circuit kinks at the seam: dot {dot:.3}");

    // Distances wrap rather than clamping.
    assert!((b.wrap(b.total + 3.0) - 3.0).abs() < 0.01);
    assert!((b.wrap(-1.0) - (b.total - 1.0)).abs() < 0.01);
}

#[test]
fn winding_route_uses_the_middle_of_the_board() {
    let b = super::board::Board::new();
    assert!(
        (96.0..102.0).contains(&b.total),
        "winding lane is {:.1} tiles",
        b.total
    );

    // An outer rectangle leaves this point a dozen tiles from combat.  This
    // deliberately leaves a small, scenic central clearing while keeping the
    // middle comfortably inside ordinary tower range.
    let middle_distance = b.dist_to_road([12.0, 12.0]);
    assert!(
        middle_distance <= 3.2,
        "the map has become a hollow ring again: middle is {middle_distance:.2} tiles from the lane"
    );

    // This is a trail, not a scribble.  A self-crossing route creates a false
    // intersection where two reverse-moving hordes overlap visually and was
    // the root cause of the rejected tangled lower lane.
    for i in 0..super::greentd_map::LAP.len() {
        let a = super::greentd_map::LAP[i];
        let b = super::greentd_map::LAP[(i + 1) % super::greentd_map::LAP.len()];
        for j in i + 1..super::greentd_map::LAP.len() {
            if j == i + 1 || (i == 0 && j + 1 == super::greentd_map::LAP.len()) {
                continue;
            }
            let c = super::greentd_map::LAP[j];
            let d = super::greentd_map::LAP[(j + 1) % super::greentd_map::LAP.len()];
            assert!(
                !segments_cross(a, b, c, d),
                "route segments {i} and {j} cross"
            );
        }
    }
}

fn segments_cross(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    let turn = |p: [f32; 2], q: [f32; 2], r: [f32; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let ab_c = turn(a, b, c);
    let ab_d = turn(a, b, d);
    let cd_a = turn(c, d, a);
    let cd_b = turn(c, d, b);
    (ab_c > 0.0001 && ab_d < -0.0001 || ab_c < -0.0001 && ab_d > 0.0001)
        && (cd_a > 0.0001 && cd_b < -0.0001 || cd_a < -0.0001 && cd_b > 0.0001)
}

#[test]
fn red_creeps_split_both_ways_like_the_reference_trigger() {
    let b = super::board::Board::new();
    let start_a = b.sample_travel(0.0, 1.0);
    let start_b = b.sample_travel(0.0, -1.0);
    assert!((start_a[0] - start_b[0]).abs() < 0.01);
    assert!((start_a[1] - start_b[1]).abs() < 0.01);

    let cw = b.heading_travel(0.5, 1.0);
    let ccw = b.heading_travel(0.5, -1.0);
    let dot = cw[0] * ccw[0] + cw[1] * ccw[1];
    assert!(
        dot < 0.25,
        "the two Red streams do not take separate arms of the junction: {cw:?} / {ccw:?}"
    );
}

/// Prints the board the map produced. Diagnostic:
///     cargo test --release show_the_board -- --ignored --nocapture
#[test]
#[ignore = "prints the board"]
fn show_the_board() {
    let b = super::board::Board::new();
    println!(
        "lane {:.1} tiles, {} pads, arena {:?}",
        b.total,
        b.slots.len(),
        super::greentd_map::ARENA
    );
    let (x0, y0, x1, y1) = (
        super::greentd_map::ARENA[0] as i32,
        super::greentd_map::ARENA[1] as i32,
        super::greentd_map::ARENA[2] as i32,
        super::greentd_map::ARENA[3] as i32,
    );
    for ty in (y0..=y1).rev() {
        let mut row = String::new();
        for tx in x0..=x1 {
            row.push(if super::board::is_corridor(tx, ty) {
                '#'
            } else if b.tile_slot([tx as f32 + 0.5, ty as f32 + 0.5]).is_some() {
                'o'
            } else {
                '.'
            });
        }
        println!("{ty:3} {row}");
    }
}

#[test]
fn every_dense_build_tile_is_legal_and_touches_the_fight() {
    let b = super::board::Board::new();
    assert_eq!(b.slots.len(), 164, "dense shoulder drifted");
    for (i, s) in b.slots.iter().enumerate() {
        let tx = s.pos[0].floor() as i32;
        let ty = s.pos[1].floor() as i32;
        assert_eq!(
            b.tile_slot(s.pos),
            Some(i),
            "slot {i} is not on its own tile"
        );
        assert_eq!(
            b.slot_at(s.pos),
            Some(i),
            "slot {i} cannot be clicked directly"
        );
        assert!(
            !super::board::is_corridor(tx, ty),
            "a pad sits in the corridor at {:?}",
            s.pos
        );
        let road_dist = b.dist_to_road(s.pos);
        assert!(
            (super::board::PAD_ROAD_MIN..=super::board::PAD_ROAD_MAX).contains(&road_dist),
            "pad {:?} is {road_dist:.2} from the road, outside the tactical shoulder",
            s.pos
        );
    }

    // No silent holes: every clear tile in the marked shoulder builds, and
    // neither the lane nor decorative grass can steal an adjacent click.
    let a = super::greentd_map::ARENA;
    for ty in a[1] as i32..=a[3] as i32 {
        for tx in a[0] as i32..=a[2] as i32 {
            let p = [tx as f32 + 0.5, ty as f32 + 0.5];
            let inside = p[0] >= a[0] + 1.25
                && p[1] >= a[1] + 1.25
                && p[0] <= a[2] - 1.25
                && p[1] <= a[3] - 1.25;
            let wanted = inside
                && super::board::buildable_tile(tx, ty)
                && (super::board::PAD_ROAD_MIN..=super::board::PAD_ROAD_MAX)
                    .contains(&b.dist_to_road(p));
            assert_eq!(
                b.tile_slot(p).is_some(),
                wanted,
                "tile ({tx}, {ty}) does not agree with the visible build shoulder"
            );
        }
    }
}

#[test]
fn each_pad_holds_exactly_one_tower() {
    let mut g = rich_game();
    let built = fill_pads(&mut g);
    assert_eq!(built as usize, g.towers.len());
    for (i, tw) in g.towers.iter().enumerate() {
        assert_eq!(g.board.slots[tw.slot].tower, Some(i));
    }
    let occupied = g.board.slots.iter().filter(|s| s.tower.is_some()).count();
    assert_eq!(occupied, g.towers.len());
}

#[test]
fn selling_frees_the_pad_and_keeps_indices_straight() {
    let mut g = rich_game();
    fill_pads(&mut g);
    let mut rng = Rng::new(7);
    for _ in 0..40 {
        if g.towers.is_empty() {
            break;
        }
        let i = (rng.next_u64() % g.towers.len() as u64) as usize;
        let slot = g.towers[i].slot;
        g.sell(i);
        assert!(g.board.slots[slot].tower.is_none(), "pad still claimed");
        for (n, tw) in g.towers.iter().enumerate() {
            assert_eq!(
                g.board.slots[tw.slot].tower,
                Some(n),
                "pad {} points at the wrong tower after a sell",
                tw.slot
            );
        }
    }
}

/// Selling pays back the map's own point value, which is nearly always
/// everything sunk into the tower.
#[test]
fn selling_refunds_what_the_map_says() {
    let mut g = rich_game();
    let ti = build(&mut g, Family::Siege, 0.0);
    max_out(&mut g, ti);
    let refund = g.towers[ti].sell_value();
    let before = g.gold;
    g.sell(ti);
    assert_eq!(g.gold - before, refund as i64);
    assert!(
        refund > 30_000,
        "a maxed Siege Tower refunds only {refund}g"
    );
}

#[test]
fn a_source_refund_anomaly_cannot_mint_gold() {
    let mut g = rich_game();
    let ti = build(&mut g, Family::Chaos, 0.0);
    for _ in 0..2 {
        let (into, _) = g.upgrade_choices(ti)[0];
        g.upgrade_into(ti, into);
    }

    let invested = g.towers[ti].invested;
    let raw_source_refund = g.towers[ti].def().refund;
    assert!(
        raw_source_refund > invested,
        "the regression path no longer exercises a source refund anomaly"
    );
    assert_eq!(g.towers[ti].sell_value(), invested);

    let before_sale = g.gold;
    g.sell(ti);
    assert_eq!(g.gold - before_sale, invested as i64);
}

#[test]
fn hard_modes_charge_a_visible_respec_cost() {
    let mut veteran = rich_game();
    veteran.difficulty = Difficulty::Veteran;
    let ti = build(&mut veteran, Family::Siege, 0.0);
    let invested = veteran.towers[ti].invested;
    assert_eq!(veteran.tower_sell_value(ti), invested * 80 / 100);
    veteran.spawn_left = 1;
    assert_eq!(veteran.tower_sell_value(ti), invested * 65 / 100);
    let before = veteran.gold;
    veteran.sell(ti);
    assert_eq!(veteran.gold - before, (invested * 65 / 100) as i64);

    let mut nightmare = rich_game();
    nightmare.difficulty = Difficulty::Nightmare;
    let ti = build(&mut nightmare, Family::Siege, 0.0);
    let invested = nightmare.towers[ti].invested;
    assert_eq!(nightmare.tower_sell_value(ti), invested * 70 / 100);
    nightmare.spawn_left = 1;
    assert_eq!(nightmare.tower_sell_value(ti), invested * 50 / 100);
}

#[test]
fn commander_hunters_default_to_strongest_without_overriding_player_intent() {
    let mut g = rich_game();
    let king = build(&mut g, Family::King, 0.0);
    assert_eq!(g.towers[king].mode, TargetMode::Strongest);

    let seed = build(&mut g, Family::Single, 2.0);
    g.upgrade_into(seed, root(Family::OneStrike));
    assert_eq!(g.towers[seed].mode, TargetMode::Strongest);

    let chosen = build(&mut g, Family::Single, 4.0);
    g.towers[chosen].mode = TargetMode::Closest;
    g.upgrade_into(chosen, root(Family::OneStrike));
    assert_eq!(g.towers[chosen].mode, TargetMode::Closest);
}

// ---------------------------------------------------------------- the loss

#[test]
fn an_undefended_ring_floods_and_the_run_is_lost() {
    let mut g = Game::new();
    g.start_run(1);
    g.send_wave();
    run_for(&mut g, 60.0 * 25.0);
    assert_eq!(
        g.phase,
        Phase::Defeat,
        "an empty board survived; ring holds {}/{FLOOD_LIMIT}",
        g.creeps.len()
    );
}

#[test]
fn a_hard_mode_commander_cannot_circle_forever_below_the_crowd_cap() {
    for (difficulty, limit) in [(Difficulty::Veteran, 4), (Difficulty::Nightmare, 3)] {
        let mut hard = Game::new();
        hard.start_run_with_difficulty(99, difficulty);
        hard.wave = 31;
        let boss_wave = hard.wave_def(31);
        hard.spawn_creep_ranked(&boss_wave, boss_wave.hp, 1.0, 0.0, false, true);
        hard.creeps[0].laps = limit - 1;
        hard.check_end();
        assert_ne!(
            hard.phase,
            Phase::Defeat,
            "{difficulty:?} ended one lap early"
        );

        hard.creeps[0].laps = limit;
        hard.check_end();
        assert_eq!(
            hard.phase,
            Phase::Defeat,
            "{difficulty:?} ignored its commander lap limit"
        );
    }

    let mut classic = Game::new();
    classic.wave = 31;
    let boss_wave = classic.wave_def(31);
    classic.spawn_creep_ranked(&boss_wave, boss_wave.hp, 1.0, 0.0, false, true);
    classic.creeps[0].laps = 3;
    classic.check_end();
    assert_ne!(
        classic.phase,
        Phase::Defeat,
        "Legacy rules must stay faithful"
    );
}

#[test]
fn monsters_go_round_and_round() {
    let mut g = rich_game();
    isolate(&mut g);
    let w = creep(1.0e9, 0, ArmourType::Unarmoured, false);
    g.spawn_creep(&w, w.hp, 1.0, 0.0);
    let total = g.board.total;
    run_for(&mut g, total / w.speed * 1.6);
    let c = &g.creeps[0];
    assert!(c.laps >= 1, "nothing completed a lap in {total:.0} tiles");
    assert!(c.dist >= 0.0 && c.dist < total, "dist {} escaped", c.dist);
}

#[test]
fn long_run_does_not_panic() {
    let mut g = rich_game();
    fill_pads(&mut g);
    g.prep = false;
    g.phase = Phase::Combat;
    run_for(&mut g, 60.0 * 30.0);
}

/// A splash hit into a dense pack removes many creeps in one call, which is
/// exactly where a stale index turns into a panic or a silent wrong kill.
#[test]
fn splash_into_a_dense_pack_is_safe() {
    let mut g = rich_game();
    isolate(&mut g);
    let ti = build(&mut g, Family::Siege, 6.0);
    max_out(&mut g, ti);
    let at = g.towers[ti].pos;
    // Put the pack right under the tower so everything is inside the splash.
    let mut best = 0.0f32;
    let mut bestd = f32::MAX;
    let mut d = 0.0;
    while d < g.board.total {
        let p = g.board.sample(d);
        let dd = (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2);
        if dd < bestd {
            bestd = dd;
            best = d;
        }
        d += 0.25;
    }
    let w = creep(50.0, 0, ArmourType::Unarmoured, false);
    for _ in 0..120 {
        g.spawn_creep(&w, w.hp, 1.0, best);
    }
    let before = g.creeps.len();
    run_for(&mut g, 12.0);
    assert!(g.creeps.len() < before, "the pack was never touched");
    for c in &g.creeps {
        assert!(c.hp > 0.0, "a dead creep is still on the ring");
    }
}

// ---------------------------------------------------------------- the roster

/// Every tower in the roster has to be reachable by clicking, or it is data
/// nobody will ever see.
#[test]
fn every_tower_is_reachable_from_the_shop() {
    let mut seen = vec![false; TOWERS.len()];
    let mut queue: Vec<usize> = shop_order();
    for &i in &queue {
        seen[i] = true;
    }
    while let Some(i) = queue.pop() {
        for (u, _) in upgrades_of(i) {
            if !seen[u] {
                seen[u] = true;
                queue.push(u);
            }
        }
    }
    let missing: Vec<&str> = TOWERS
        .iter()
        .enumerate()
        .filter(|(i, _)| !seen[*i])
        .map(|(_, t)| t.name)
        .collect();
    assert!(missing.is_empty(), "unreachable towers: {missing:?}");
}

/// And every one of them can actually be built and paid for.
#[test]
fn every_tower_can_be_built_and_upgraded_into() {
    let mut g = rich_game();
    // There are more towers in the roster than pads on the board, so each is
    // built, checked and sold again.
    for i in 0..TOWERS.len() {
        g.build_choice = Some((i, 1));
        assert!(g.try_build(0), "could not build {}", TOWERS[i].name);
        assert_eq!(g.towers[0].def, i);
        g.sell(0);
    }
    g.build_choice = None;
    assert!(g.towers.is_empty());
}

#[test]
fn the_seed_becomes_one_of_six_and_costs_ten_gold() {
    let mut g = Game::new();
    g.gold = 10;
    let ti = build(&mut g, Family::Single, 0.0);
    assert_eq!(g.gold, 0, "the seed should cost exactly ten gold");
    assert!(g.towers[ti].has_choice());
    let choices = g.upgrade_choices(ti);
    assert_eq!(choices.len(), 6);

    // With no money, the choice is refused rather than half-applied.
    let before = g.towers[ti].def;
    g.upgrade_into(ti, choices[0].0);
    assert_eq!(g.towers[ti].def, before, "a tower was upgraded for free");

    g.gold = 10_000;
    g.upgrade_into(ti, choices[0].0);
    assert_eq!(g.towers[ti].def, choices[0].0);
    assert!(!g.towers[ti].has_choice() || g.towers[ti].family() == Family::Aura);
}

/// The Aura Tower is the one family that branches at every rung, and the two
/// branches are free side-grades of each other.
#[test]
fn the_aura_tower_branches_at_every_rung() {
    let mut g = rich_game();
    let ti = build(&mut g, Family::Aura, 0.0);
    assert_eq!(g.upgrade_choices(ti).len(), 3);

    let dmg = t("Damage Tower");
    g.upgrade_into(ti, dmg);
    assert_eq!(g.towers[ti].family(), Family::Damage);
    assert!(g.towers[ti].abil().dmg_aura > 0.0);

    // And swapping sideways to Speed costs nothing.
    let speed = t("Speed Tower");
    let before = g.gold;
    g.upgrade_into(ti, speed);
    assert_eq!(g.towers[ti].family(), Family::Speed);
    assert_eq!(g.gold, before, "the side-grade should be free");
    assert!(g.towers[ti].abil().speed_aura > 0.0);
}

#[test]
fn a_tower_cannot_be_upgraded_into_something_it_does_not_lead_to() {
    let mut g = rich_game();
    let ti = build(&mut g, Family::Siege, 0.0);
    let before = g.towers[ti].def;
    g.upgrade_into(ti, t("Chaos tower "));
    assert_eq!(g.towers[ti].def, before, "the upgrade graph was ignored");
}

// ---------------------------------------------------------------- damage

/// The whole counter system, in one test: Immune waves take five percent from
/// everything except Chaos, and a hundred times from Hero damage.
#[test]
fn immune_waves_only_fall_to_chaos_and_hero() {
    let hp = 40_000.0;
    let base = 10_000.0;
    let immune = damage_taken(base, Attack::Siege, 0, ArmourType::Divine);
    let chaos = damage_taken(base, Attack::Chaos, 0, ArmourType::Divine);
    let hero = damage_taken(base, Attack::Hero, 0, ArmourType::Divine);
    assert!(
        immune * 20.0 <= chaos + 1.0,
        "Immune is not resisting Siege"
    );
    assert!(chaos > hp / 8.0);
    assert!(hero > chaos * 50.0, "Hero damage is not the map's x100");
}

/// The same thing, played out: a board of Siege towers cannot kill an Immune
/// wave, and the same board with Chaos towers can.
#[test]
fn chaos_answers_an_immune_wave_and_siege_does_not() {
    let kill_time = |family: Family| -> f32 {
        let mut g = rich_game();
        isolate(&mut g);
        line_of(&mut g, family, 10);
        let w = creep(50_000.0, 40, ArmourType::Divine, false);
        g.spawn_creep(&w, w.hp, 1.0, 0.0);
        let mut t = 0.0;
        while !g.creeps.is_empty() && t < 120.0 {
            g.update(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
        t
    };
    let siege = kill_time(Family::Siege);
    let chaos = kill_time(Family::Chaos);
    assert!(
        chaos < siege * 0.5,
        "Chaos took {chaos:.1}s and Siege {siege:.1}s against an Immune wave"
    );
    assert!(chaos < 60.0, "even Chaos could not kill it: {chaos:.1}s");
}

/// Armour is the difficulty curve, so it has to actually bite.
#[test]
fn armour_reduces_what_lands() {
    let bare = damage_taken(1000.0, Attack::Normal, 0, ArmourType::Unarmoured);
    let heavy = damage_taken(1000.0, Attack::Normal, 200, ArmourType::Unarmoured);
    assert!((bare - 1000.0).abs() < 0.1);
    assert!(heavy < 90.0, "200 armour let {heavy:.0} through");
}

/// The Corruption Tower strips armour, which is most of what it is for.
#[test]
fn corruption_strips_armour_off_what_it_hits() {
    let dealt = |pen: i32| damage_taken(1000.0, Attack::Normal, 60 - pen, ArmourType::Unarmoured);
    assert!(
        dealt(45) > dealt(0) * 2.0,
        "stripping 45 armour barely helped"
    );
    let corr = &TOWERS[t("Corruption Tower 5: Perfect")];
    assert_eq!(corr.abil.armour_pen, 75);
}

// ---------------------------------------------------------------- the air

#[test]
fn ground_towers_cannot_touch_the_air() {
    let mut g = rich_game();
    isolate(&mut g);
    line_of(&mut g, Family::Siege, 12);
    let w = creep(2_000.0, 0, ArmourType::Unarmoured, true);
    for _ in 0..8 {
        g.spawn_creep(&w, w.hp, 1.0, 0.0);
    }
    run_for(&mut g, 45.0);
    assert_eq!(
        g.creeps.len(),
        8,
        "a ground-only board shot down {} flyers",
        8 - g.creeps.len()
    );
}

#[test]
fn the_air_tower_answers_the_air_and_nothing_else() {
    let air1 = &TOWERS[root(Family::Air)];
    assert_eq!(air1.targets, Targets::AirOnly);

    let mut g = rich_game();
    isolate(&mut g);
    line_of(&mut g, Family::Air, 8);
    let flyer = creep(20_000.0, 0, ArmourType::Unarmoured, true);
    for _ in 0..6 {
        g.spawn_creep(&flyer, flyer.hp, 1.0, 0.0);
    }
    run_for(&mut g, 40.0);
    assert!(g.creeps.is_empty(), "the Air Towers missed the air");

    // And the top of the ladder covers both, exactly as the map has it.
    let air10 = &TOWERS[t("Air Tower 10: Perfect")];
    assert_eq!(air10.targets, Targets::Both);
}

#[test]
fn ground_towers_are_lethal_on_the_ground() {
    let mut g = rich_game();
    isolate(&mut g);
    line_of(&mut g, Family::Siege, 8);
    let w = creep(50_000.0, 20, ArmourType::Unarmoured, false);
    for _ in 0..10 {
        g.spawn_creep(&w, w.hp, 1.0, 0.0);
    }
    run_for(&mut g, 45.0);
    assert!(
        g.creeps.is_empty(),
        "{} walkers survived a maxed Siege board",
        g.creeps.len()
    );
}

#[test]
fn an_in_flight_shot_keeps_its_target_rules_after_its_tower_is_sold() {
    let mut g = rich_game();
    isolate(&mut g);
    let ti = build(&mut g, Family::Siege, 3.0);
    let def = g.towers[ti].def;
    assert_eq!(TOWERS[def].targets, Targets::GroundOnly);

    let ground = creep(1.0e9, 0, ArmourType::Unarmoured, false);
    let air = creep(1.0e9, 0, ArmourType::Unarmoured, true);
    g.spawn_creep(&ground, ground.hp, 1.0, 3.0);
    g.spawn_creep(&air, air.hp, 1.0, 3.0);
    let at = g.creeps[0].pos;
    g.creeps[1].pos = at;
    let target_uid = g.creeps[0].uid;
    let ground_before = g.creeps[0].hp;
    let air_before = g.creeps[1].hp;

    g.projs.push(Proj {
        pos: at,
        z: g.creeps[0].height(),
        vel: [0.0, 0.0],
        kind: ProjKind::Shell,
        tower: ti,
        def,
        dmg: 10_000.0,
        splash: 3.0,
        bounces: 0,
        crit: false,
        target_idx: 0,
        target_uid,
        life: 1.0,
        trail: 1.0,
    });
    g.sell(ti);
    g.spatial.rebuild(&g.creeps);
    combat::step_projectiles(&mut g, 1.0 / 120.0);

    assert!(
        g.creeps[0].hp < ground_before,
        "the primary ground target was missed"
    );
    assert_eq!(
        g.creeps[1].hp, air_before,
        "selling the Siege tower turned its ground-only splash into an air hit"
    );
}

// ---------------------------------------------------------------- abilities

#[test]
fn poison_keeps_working_after_the_shot_lands() {
    let mut g = rich_game();
    isolate(&mut g);
    let ti = build(&mut g, Family::Single, 3.0);
    g.upgrade_into(ti, root(Family::Poison));
    max_out(&mut g, ti);
    assert!(g.towers[ti].abil().poison_dps > 0.0);

    let w = creep(1.0e9, 0, ArmourType::Unarmoured, false);
    g.spawn_creep(&w, w.hp, 1.0, 0.0);
    run_for(&mut g, 6.0);
    assert!(g.creeps[0].poison.t > 0.0, "nothing was poisoned");
    let hp = g.creeps[0].hp;

    // Take the tower away; the poison must carry on doing its work.
    g.sell(ti);
    run_for(&mut g, 2.0);
    assert!(
        g.creeps[0].hp < hp,
        "poison stopped the moment the tower did"
    );
}

#[test]
fn a_weaker_poison_never_truncates_a_stronger_stack() {
    let mut g = rich_game();
    isolate(&mut g);

    let strong = build(&mut g, Family::Single, 3.0);
    g.upgrade_into(strong, root(Family::Poison));
    max_out(&mut g, strong);
    let weak = build(&mut g, Family::Single, 5.0);
    g.upgrade_into(weak, root(Family::Poison));
    assert!(g.towers[strong].abil().poison_dps > g.towers[weak].abil().poison_dps);

    let w = creep(1.0e9, 0, ArmourType::Unarmoured, false);
    g.spawn_creep(&w, w.hp, 1.0, 0.0);
    combat::on_hit_riders(&mut g, strong, 0);
    let amount = g.creeps[0].poison.amt;
    let duration = g.creeps[0].poison.t;
    combat::on_hit_riders(&mut g, weak, 0);

    assert!(g.creeps[0].poison.amt >= amount);
    assert!(g.creeps[0].poison.t >= duration);
}

#[test]
fn the_slow_tower_slows_without_firing_a_shot() {
    let mut g = rich_game();
    isolate(&mut g);
    let ti = build(&mut g, Family::Single, 3.0);
    g.upgrade_into(ti, root(Family::Slow));
    max_out(&mut g, ti);
    assert!(
        g.towers[ti].is_support(),
        "the Slow Tower should not attack"
    );
    assert!(g.towers[ti].abil().slow_amt > 0.0);

    // A creep walking through the cloud loses speed.
    let at = g.towers[ti].pos;
    let mut best = 0.0f32;
    let mut bestd = f32::MAX;
    let mut d = 0.0;
    while d < g.board.total {
        let p = g.board.sample(d);
        let dd = (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2);
        if dd < bestd {
            bestd = dd;
            best = d;
        }
        d += 0.25;
    }
    let w = creep(1.0e9, 0, ArmourType::Unarmoured, false);
    g.spawn_creep(&w, w.hp, 1.0, best);
    run_for(&mut g, 1.0);
    assert!(
        g.creeps[0].speed() < g.creeps[0].base_speed,
        "the cloud did not slow anything"
    );
}

#[test]
fn an_aura_tower_buffs_its_neighbours_and_stops_when_sold() {
    let mut g = rich_game();
    isolate(&mut g);
    // A cluster: one aura in the middle of a handful of attackers.
    let mut attackers = Vec::new();
    for k in 0..9 {
        attackers.push(build(&mut g, Family::Siege, 4.0 + k as f32 * 0.6));
    }
    let base: Vec<f32> = attackers.iter().map(|&i| g.towers[i].dmg()).collect();

    let aura = build(&mut g, Family::Aura, 6.0);
    g.upgrade_into(aura, t("Damage Tower"));
    g.rebuild_auras();

    let buffed = attackers
        .iter()
        .enumerate()
        .filter(|(n, i)| g.towers[**i].dmg() > base[*n] + 1e-3)
        .count();
    assert!(buffed > 0, "the Damage Tower buffed nothing");

    g.sell(aura);
    g.rebuild_auras();
    for (n, &i) in attackers.iter().enumerate() {
        assert!(
            (g.towers[i].dmg() - base[n]).abs() < 1e-3,
            "a tower kept its aura after the Damage Tower was sold"
        );
    }
}

#[test]
fn identical_local_auras_use_the_strongest_source_instead_of_multiplying() {
    let mut g = rich_game();
    isolate(&mut g);
    let attacker = build(&mut g, Family::Siege, 6.0);
    let base = g.towers[attacker].buff_dmg;

    let make_damage_aura = |game: &mut Game, along: f32| {
        let aura = build(game, Family::Aura, along);
        for name in ["Damage Tower", "Damage Tower 2", "Damage Tower 3"] {
            game.upgrade_into(aura, t(name));
        }
        aura
    };
    make_damage_aura(&mut g, 5.5);
    let one = g.towers[attacker].buff_dmg;
    assert!(one > base);
    make_damage_aura(&mut g, 6.5);
    let two = g.towers[attacker].buff_dmg;
    assert!(
        (two - one).abs() < 1e-5,
        "two identical auras stacked from {one:.2} to {two:.2}"
    );
}

#[test]
fn the_fire_tower_burns_what_stands_near_it() {
    let mut g = rich_game();
    isolate(&mut g);
    let ti = build(&mut g, Family::Single, 0.0);
    g.upgrade_into(ti, root(Family::Fire));
    let a = g.towers[ti].abil();
    assert!(a.burn_dps > 0.0 && a.burn_range > 0.0);
    assert!(a.dmg_aura > 0.0, "the Fire Tower also buffs its neighbours");
}

/// One-Strike is the map's Hero-damage tower, and Hero damage is a hundred
/// times the number on the card.
#[test]
fn the_one_strike_tower_deletes_what_it_hits() {
    let def = &TOWERS[root(Family::OneStrike)];
    assert_eq!(def.attack, Attack::Hero);
    assert!(def.abil.crit_chance >= 1.0, "it should always crit");
    let dealt = damage_taken(def.damage, def.attack, 200, ArmourType::Divine);
    assert!(
        dealt > 500_000.0,
        "a One-Strike hit only does {dealt:.0} to the toughest wave"
    );
}

// ---------------------------------------------------------------- control

/// Roots must never be able to hold a wave still forever. Nothing dies, nothing
/// leaks, and the wave never ends - a run stalled exactly this way before the
/// immunity window existed.
#[test]
fn a_wall_of_rooting_towers_cannot_freeze_a_wave_forever() {
    let mut g = rich_game();
    isolate(&mut g);
    for k in 0..24 {
        let ti = build(&mut g, Family::Single, k as f32 * 1.5);
        g.upgrade_into(ti, root(Family::Troll));
    }
    let w = creep(1.0e12, 0, ArmourType::Unarmoured, false);
    g.spawn_creep(&w, w.hp, 1.0, 0.0);
    let start = g.creeps[0].dist;
    run_for(&mut g, 90.0);
    let moved = g.creeps[0].laps as f32 * g.board.total + g.creeps[0].dist - start;
    assert!(
        moved > 10.0,
        "a rooted monster moved {moved:.2} tiles in ninety seconds"
    );
}

#[test]
fn repeated_roots_diminish_and_then_recover() {
    let mut c = creep(1.0e9, 0, ArmourType::Unarmoured, false);
    c.speed = 1.0;
    let mut g = rich_game();
    isolate(&mut g);
    g.spawn_creep(&c, c.hp, 1.0, 0.0);

    // Hammer it: resistance climbs to its ceiling.
    for _ in 0..12 {
        g.creeps[0].stun_immune = 0.0;
        g.creeps[0].stun = 0.0;
        g.creeps[0].stun_dr = (g.creeps[0].stun_dr + STUN_DR_STEP).min(STUN_DR_MAX);
    }
    assert!((g.creeps[0].stun_dr - STUN_DR_MAX).abs() < 1e-4);

    // Leave it alone and the resistance bleeds off.
    run_for(&mut g, 6.0);
    assert!(
        g.creeps[0].stun_dr < STUN_DR_MAX * 0.5,
        "resistance never recovered: {}",
        g.creeps[0].stun_dr
    );
}

// ---------------------------------------------------------------- waves

#[test]
fn wave_table_is_well_formed() {
    for n in 1..=CAMPAIGN_WAVES {
        let w = wave_at(n);
        assert!(w.count > 0, "wave {n} sends nothing");
        assert!(w.hp > 0.0, "wave {n} has no health");
        assert!(
            w.speed > 0.2 && w.speed < 6.0,
            "wave {n} moves at {}",
            w.speed
        );
        assert!(w.spawn_gap > 0.0 && w.spawn_gap < 10.0);
        assert!(bounty_of(&w) >= 1, "wave {n} pays nothing");
    }
    // Health only ever climbs across the campaign in the broad sense: the map
    // dips on Immune waves on purpose, so compare the ends rather than pairs.
    assert!(wave_at(CAMPAIGN_WAVES).hp > wave_at(1).hp * 1000.0);
}

#[test]
fn a_wave_finishes_arriving_within_its_window() {
    let lane = super::board::Board::new().total;
    for difficulty in Difficulty::ALL {
        let mut g = Game::new();
        g.start_run_with_difficulty(7, difficulty);
        for n in [1u32, 12, 24, 36] {
            let w = g.wave_def(n);
            let window = w.spawn_gap * w.count.saturating_sub(1) as f32;
            assert!(
                window <= w.lead_in,
                "{difficulty:?} wave {n} needs {window:.1}s to arrive but advances in {:.1}s",
                w.lead_in
            );
            assert!(
                window <= WAVE_SPAWN_WINDOW + 0.5,
                "{difficulty:?} wave {n} takes {window:.0}s to arrive"
            );
            assert!(
                w.lead_in + 1e-4 >= window + WAVE_RECOVERY,
                "{difficulty:?} wave {n} has no recovery after its horde"
            );
            let horde_span = w.speed * window;
            assert!(
                horde_span <= lane * 0.60,
                "{difficulty:?} wave {n} is a thin stream spanning {horde_span:.1} of {lane:.1} tiles"
            );
        }
    }
}

#[test]
fn boss_waves_have_one_real_commander_and_keep_every_authored_enemy() {
    let mut g = Game::new();
    g.start_run_with_difficulty(0xB055, Difficulty::Veteran);
    g.wave = 34;
    g.prep = false;
    g.begin_wave(false);
    let w = g.wave_def(35);
    assert_eq!(w.tag, "Boss");

    while g.spawn_left > 0 {
        g.spawn_timer = 0.0;
        g.spawn_step(0.0);
    }

    assert_eq!(g.creeps.len(), w.count as usize);
    let leaders: Vec<&Creep> = g.creeps.iter().filter(|c| c.is_boss()).collect();
    assert_eq!(
        leaders.len(),
        1,
        "a boss banner became a mass of boss units"
    );
    assert!((leaders[0].max_hp / w.hp - BOSS_HP_MULT).abs() < 0.01);
    assert_eq!(leaders[0].bounty, g.bounty_for_wave(35) * BOSS_REWARD_MULT);
    assert!(
        g.creeps
            .iter()
            .filter(|c| !c.is_boss() && !c.elite)
            .all(|c| (c.max_hp - w.hp).abs() < 0.01)
    );
}

#[test]
fn boss_repair_has_visible_corruption_counterplay() {
    let mut g = rich_game();
    isolate(&mut g);
    g.wave = 35;
    let boss_wave = g.wave_def(35);
    g.spawn_creep_ranked(&boss_wave, boss_wave.hp, 1.0, 4.0, false, true);
    g.spawn_creep_ranked(&boss_wave, boss_wave.hp, 1.0, 4.2, false, false);
    g.creeps[1].hp *= 0.5;
    let hurt = g.creeps[1].hp;
    g.step_creeps(1.0);
    assert!(g.creeps[1].hp > hurt, "the commander repaired no escort");

    let ti = build(&mut g, Family::Corruption, 4.2);
    combat::on_hit_riders(&mut g, ti, 1);
    assert!(g.creeps[1].suppress > 0.0);
    let suppressed = g.creeps[1].hp;
    g.step_creeps(0.5);
    assert_eq!(
        g.creeps[1].hp, suppressed,
        "Corruption did not suppress commander repair"
    );
}

#[test]
fn wave_35_cannot_overflow_live_combat_or_visual_queues() {
    let mut g = rich_game();
    isolate(&mut g);
    g.wave = 35;
    let super_multi = t("Super Multi Tower : Perfect");
    for slot in 0..g.board.slots.len() {
        g.build_choice = Some((super_multi, 1));
        let _ = g.try_build(slot);
    }
    g.build_choice = None;
    g.selected = None;
    for tower in &mut g.towers {
        tower.cooldown = 0.0;
    }

    let w = g.wave_def(35);
    for i in 0..w.count {
        // Deliberately enormous health keeps every target alive while the
        // fully packed board fires its first volley.
        g.spawn_creep_ranked(
            &w,
            1.0e12,
            1.0,
            i as f32 / w.count as f32 * g.board.total,
            false,
            i == 0,
        );
    }
    g.spatial.rebuild(&g.creeps);
    combat::step_towers(&mut g, 1.0 / 120.0);
    combat::step_projectiles(&mut g, 1.0 / 120.0);

    assert!(g.projs.len() <= MAX_PROJECTILES);
    assert!(g.beams.len() <= MAX_BEAMS);
    assert!(g.texts.len() <= MAX_FLOAT_TEXTS);
    assert!(g.fx.particles.len() <= 8192);
    assert!(
        g.creeps
            .iter()
            .all(|c| c.hp.is_finite() && c.hp > 0.0 && c.pos.into_iter().all(f32::is_finite))
    );
}

#[test]
fn same_frame_kills_keep_creep_indices_stable_until_damage_is_resolved() {
    let mut g = rich_game();
    isolate(&mut g);
    let ti = build(&mut g, Family::Single, 2.0);
    let w = g.wave_def(1);
    g.spawn_creep(&w, w.hp, 1.0, 2.0);
    g.spawn_creep(&w, w.hp, 1.0, 3.0);
    let uids = [g.creeps[0].uid, g.creeps[1].uid];

    assert!(combat::damage_creep(&mut g, 0, 1.0e20, ti, false));
    assert_eq!(
        g.creeps.len(),
        2,
        "the first kill invalidated later hit indices"
    );
    assert_eq!(g.creeps[1].uid, uids[1]);
    assert!(combat::damage_creep(&mut g, 1, 1.0e20, ti, false));
    assert_eq!(g.creeps.iter().map(|c| c.uid).collect::<Vec<_>>(), uids);

    combat::step_projectiles(&mut g, 0.0);
    assert!(
        g.creeps.is_empty(),
        "dead targets survived the resolution barrier"
    );
}

#[test]
fn waves_keep_escalating_past_the_campaign() {
    let last = wave_at(CAMPAIGN_WAVES);
    let past = wave_at(CAMPAIGN_WAVES + 5);
    assert!(past.hp > last.hp, "endless does not escalate");
    assert!(past.armour > last.armour);
    assert!(bounty_of(&past) > bounty_of(&last), "endless does not pay");

    // And it stays finite twenty waves out, or the HUD prints infinity.
    let deep = wave_at(CAMPAIGN_WAVES + 20);
    assert!(deep.hp.is_finite() && wave_clear_bonus(CAMPAIGN_WAVES + 20) > 0);
}

#[test]
fn clearing_the_campaign_wins_but_can_be_continued() {
    let mut g = rich_game();
    fill_pads(&mut g);
    g.wave = CAMPAIGN_WAVES;
    g.spawn_left = 0;
    g.creeps.clear();
    g.phase = Phase::Combat;
    g.prep = false;
    g.update(1.0 / 60.0);
    assert_eq!(g.phase, Phase::Victory);

    g.continue_endless();
    assert!(g.endless);
    assert_ne!(g.phase, Phase::Victory);
    assert!(g.last_wave() > CAMPAIGN_WAVES);
}

// ---------------------------------------------------------------- economy

#[test]
fn the_purse_starts_where_the_map_starts_it() {
    let g = Game::new();
    assert_eq!(g.gold, 1_000, "the map hands out a thousand gold");
    assert_eq!(FLOOD_LIMIT, 700, "the map loses at seven hundred");
}

#[test]
fn difficulty_modes_preserve_classic_and_add_real_pressure() {
    let mut classic = Game::new();
    classic.start_run_with_difficulty(7, Difficulty::Classic);
    let base = wave_at(CAMPAIGN_WAVES);
    let faithful = classic.wave_def(CAMPAIGN_WAVES);
    assert_eq!(faithful.hp, base.hp);
    assert_eq!(faithful.speed, base.speed);
    assert_eq!(faithful.spawn_gap, base.spawn_gap);
    assert_eq!(faithful.lead_in, base.lead_in);
    assert_eq!(classic.flood_limit(), FLOOD_LIMIT);
    assert_eq!(Difficulty::Classic.elite_stride(36), None);

    let mut veteran = Game::new();
    veteran.start_run_with_difficulty(7, Difficulty::Veteran);
    let hard = veteran.wave_def(CAMPAIGN_WAVES);
    assert!(hard.hp >= base.hp * 1.54);
    assert!(hard.speed > base.speed);
    assert!(hard.spawn_gap < base.spawn_gap);
    assert!(hard.lead_in < base.lead_in);
    assert_eq!(veteran.flood_limit(), 450);
    veteran.wave = CAMPAIGN_WAVES;
    assert_eq!(veteran.flood_limit(), 280);
    assert_eq!(Difficulty::Veteran.elite_stride(4), Some(8));

    let mut nightmare = Game::new();
    nightmare.start_run_with_difficulty(7, Difficulty::Nightmare);
    let brutal = nightmare.wave_def(CAMPAIGN_WAVES);
    assert!(brutal.hp > hard.hp);
    assert!(brutal.speed > hard.speed);
    assert!(brutal.spawn_gap < hard.spawn_gap);
    assert!(nightmare.bounty_for_wave(20) < veteran.bounty_for_wave(20));
    assert_eq!(nightmare.flood_limit(), 400);
    nightmare.wave = CAMPAIGN_WAVES;
    assert_eq!(nightmare.flood_limit(), 200);
    assert_eq!(Difficulty::Nightmare.elite_stride(2), Some(6));

    nightmare.restart();
    assert_eq!(nightmare.difficulty, Difficulty::Nightmare);
}

#[test]
fn tempo_rewards_patience_or_risk_but_not_both() {
    let mut opening = Game::new();
    let opening_gold = opening.gold;
    opening.send_wave();
    assert!(!opening.prep, "Wave 1 did not start on command");
    assert_eq!(opening.stats.rush_gold, 0, "opening paid fake Rush gold");
    assert!(
        opening.gold > opening_gold,
        "Wave 1 missed its normal stipend"
    );

    let mut patient = Game::new();
    patient.wave = 1;
    let before = patient.gold;
    patient.begin_wave(false);
    assert_eq!(patient.stats.clean_sweeps, 1);
    assert!(patient.gold >= before + 20);
    assert_eq!(patient.stats.rush_gold, 0);

    let mut rushing = Game::new();
    rushing.wave = 1;
    rushing.prep = false;
    rushing.wave_timer = 15.0;
    rushing.send_wave();
    assert_eq!(rushing.stats.clean_sweeps, 0);
    assert!(rushing.stats.rush_gold > 0);

    let mut full_stream = Game::new();
    full_stream.send_wave();
    let first_wave = full_stream.spawn_left;
    let first_gold = full_stream.gold;
    assert!(first_wave > 0);
    full_stream.send_wave();
    assert_eq!(full_stream.wave, 1, "Enter skipped a deploying wave");
    assert_eq!(full_stream.spawn_left, first_wave);
    assert_eq!(full_stream.gold, first_gold, "blocked Rush still paid gold");
    assert!(full_stream.creeps.is_empty());

    full_stream.spawn_left = 0;
    full_stream.send_wave();
    assert_eq!(full_stream.wave, 2, "Rush did not unlock after deployment");
}

#[test]
fn campaign_uses_a_rapid_only_ladder_through_the_hundred_x_max() {
    let mut g = Game::new();
    g.cycle_speed();
    assert_eq!(g.speed, 25.0);
    g.cycle_speed();
    assert_eq!(g.speed, 50.0);
    g.cycle_speed();
    assert_eq!(g.speed, MAX_ENDLESS_SPEED);
    g.cycle_speed();
    assert_eq!(g.speed, CAMPAIGN_DEFAULT_SPEED);

    // A fresh expedition opens at the player-facing rapid pace and never
    // falls through the old 1x/2x/5x values while reaching 100x.
    g.start_campaign(77, Difficulty::Veteran);
    assert_eq!(g.speed, CAMPAIGN_DEFAULT_SPEED);
    g.cycle_speed();
    assert_eq!(g.speed, 25.0);
    g.cycle_speed();
    assert_eq!(g.speed, 50.0);
    g.cycle_speed();
    assert_eq!(g.speed, MAX_CAMPAIGN_SPEED);
    g.cycle_speed();
    assert_eq!(g.speed, CAMPAIGN_DEFAULT_SPEED);

    // Legacy/Endless retain their rules and save behavior, but no longer
    // force a player through a historic 1x/2x/5x speed ladder.
    g.mode = RunMode::Legacy;
    g.endless = true;
    g.cycle_speed();
    assert_eq!(g.speed, 25.0);
    g.cycle_speed();
    assert_eq!(g.speed, 50.0);
    g.cycle_speed();
    assert_eq!(g.speed, MAX_ENDLESS_SPEED);
    g.cycle_speed();
    assert_eq!(g.speed, CAMPAIGN_DEFAULT_SPEED);
}

#[test]
fn tower_progression_reports_the_real_deepest_upgrade_path() {
    let seed = family_start(Family::Single).expect("starter seed");
    assert_eq!(ladder_len(Family::Single), 1, "source seed family changed");
    assert_eq!(display_ladder_len(seed), 16, "seed hid its deepest attached route");
    assert_eq!(ladder_len(Family::Siege), 20);
    assert_eq!(display_ladder_len(family_start(Family::Siege).unwrap()), 20);
    assert!(display_ladder_len(seed) > 10);
}

#[test]
fn campaign_auto_deploys_after_a_real_time_build_beat_and_legacy_stays_manual() {
    let mut campaign = Game::new();
    campaign.start_campaign(0xA070, Difficulty::Classic);
    assert_eq!(campaign.phase, Phase::Build);
    assert!(!campaign.prep, "Campaign inherited Legacy's manual opening flag");
    assert_eq!(campaign.wave_timer, CAMPAIGN_AUTOSTART_SECONDS);

    // The build beat is wall-clock time, not accelerated simulation time.
    campaign.update(CAMPAIGN_AUTOSTART_SECONDS - 0.01);
    assert_eq!(campaign.phase, Phase::Build);
    campaign.update(0.02);
    assert_eq!(campaign.phase, Phase::Combat, "Campaign did not auto-deploy");
    assert_eq!(campaign.wave, 1);
    assert!(
        campaign.campaign.as_ref().is_some_and(|state| state.elapsed_seconds > 0.0),
        "auto deployment did not enter the production encounter adapter"
    );

    let mut legacy = Game::new();
    legacy.update(5.0);
    assert_eq!(legacy.phase, Phase::Build);
    assert!(legacy.prep, "Legacy opening changed while adding Campaign auto-start");
    assert_eq!(legacy.wave, 0);
}

#[test]
fn legacy_recovery_is_still_held_to_the_standard_two_x_baseline() {
    for difficulty in Difficulty::ALL {
        let mut g = Game::new();
        g.start_run_with_difficulty(7, difficulty);
        // Wave one is player-started. Every later boundary still leaves a
        // recovery beat, even at 2x, so the campaign does not become a blur.
        let boundaries: f32 = (2..=CAMPAIGN_WAVES)
            .map(|wave| g.wave_def(wave).lead_in)
            .sum();
        let fastest_wall_clock = boundaries / CAMPAIGN_STANDARD_SPEED;
        assert!(
            fastest_wall_clock >= 9.0 * 60.0,
            "{difficulty:?} can be compressed to only {:.1} minutes",
            fastest_wall_clock / 60.0
        );
    }
}

#[test]
fn hundred_x_uses_the_full_fixed_step_budget_at_thirty_hz() {
    let mut g = Game::new();
    g.speed = MAX_CAMPAIGN_SPEED;
    g.update(1.0 / 30.0);
    assert!(
        (g.time - 100.0 / 30.0).abs() < 0.003,
        "100x dropped simulation time at a 30 Hz browser frame: {}",
        g.time
    );
}

#[test]
fn command_drafts_pause_hard_modes_and_buff_the_whole_board() {
    let mut g = Game::new();
    g.start_run_with_difficulty(11, Difficulty::Veteran);
    g.gold = 10_000;
    let ti = build(&mut g, Family::Siege, 2.0);
    let before = g.towers[ti].dmg();

    g.wave = 9;
    g.begin_wave(false);
    assert!(g.pending_doctrine && g.paused);
    g.choose_doctrine(Doctrine::Arsenal);
    assert!(!g.pending_doctrine && !g.paused);
    assert_eq!(g.doctrine_rank(Doctrine::Arsenal), 1);
    assert!((g.towers[ti].dmg() / before - 1.12).abs() < 0.001);

    let mut classic = Game::new();
    classic.wave = 9;
    classic.begin_wave(false);
    assert!(!classic.pending_doctrine && !classic.paused);
}

#[test]
fn hard_mode_survivors_accelerate_without_minting_gold_each_lap() {
    let mut g = Game::new();
    g.start_run_with_difficulty(13, Difficulty::Veteran);
    g.wave = 1;
    let w = g.wave_def(1);
    g.spawn_creep(&w, w.hp, 1.0, g.board.total - 0.01);
    g.creeps[0].dist = g.board.total - 0.01;
    let speed = g.creeps[0].base_speed;
    let bounty = g.creeps[0].bounty;
    g.step_creeps(0.1);
    assert_eq!(g.creeps[0].laps, 1);
    assert!((g.creeps[0].base_speed / speed - 1.06).abs() < 0.001);
    assert_eq!(g.creeps[0].bounty, bounty);
}

#[test]
fn nightmare_streams_visible_vanguards_with_double_bounty() {
    let mut g = Game::new();
    g.start_run_with_difficulty(9, Difficulty::Nightmare);
    g.wave = 2;
    g.send_wave();
    let w = g.wave_def(3);
    let normal_bounty = g.bounty_for_wave(3);

    for _ in 0..6 {
        g.spawn_timer = 0.0;
        g.spawn_step(0.0);
    }

    let elites: Vec<&Creep> = g.creeps.iter().filter(|c| c.elite).collect();
    assert_eq!(elites.len(), 1, "one in six should be a Vanguard");
    let elite = elites[0];
    assert!((elite.max_hp - w.hp * 3.2).abs() < 0.1);
    assert!((elite.base_speed - w.speed * 1.14).abs() < 0.001);
    assert_eq!(elite.bounty, normal_bounty * 2);
    assert_eq!(elite.control_scale(), 0.5);
    assert!(
        g.creeps
            .iter()
            .filter(|c| !c.elite)
            .all(|c| c.max_hp == w.hp)
    );
}

#[test]
fn a_tower_cannot_be_built_without_the_gold() {
    let mut g = Game::new();
    g.gold = 9;
    g.build_choice = Some((root(Family::Single), 1));
    assert!(!g.try_build(0), "a ten gold tower was built with nine gold");
    assert!(g.towers.is_empty());
}

#[test]
fn free_grass_placement_uses_real_positions_and_one_authoritative_footprint_rule() {
    use super::{FREE_TOWER_SLOT, PlacementIssue};
    use super::board::{BUILD_WORLD, TOWER_FOOTPRINT_RADIUS};

    let mut g = Game::new();
    g.gold = 100_000;
    let root = root(Family::Single);
    let mut points = Vec::new();
    let mut y = BUILD_WORLD[1] + TOWER_FOOTPRINT_RADIUS;
    while y <= BUILD_WORLD[3] - TOWER_FOOTPRINT_RADIUS && points.len() < 3 {
        let mut x = BUILD_WORLD[0] + TOWER_FOOTPRINT_RADIUS;
        while x <= BUILD_WORLD[2] - TOWER_FOOTPRINT_RADIUS && points.len() < 3 {
            if let Ok(p) = g.buildability_at([x, y], false)
                && points.iter().all(|q: &[f32; 2]| {
                    let dx = q[0] - p[0];
                    let dy = q[1] - p[1];
                    dx * dx + dy * dy > 5.0
                })
            {
                points.push(p);
            }
            x += 0.50;
        }
        y += 0.50;
    }
    assert_eq!(points.len(), 3, "the meadow lost clear grass build positions");
    assert!(
        points.iter().any(|p| p[0] < 0.0 || p[1] < 0.0),
        "expanded rendered grass was silently excluded from building"
    );

    for p in points {
        let gold = g.gold;
        g.build_choice = Some((root, 1));
        assert!(g.try_build_at(p), "clear grass at {p:?} was rejected");
        let tower = g.towers.last().expect("tower was built");
        assert_eq!(tower.pos, p, "tower was moved to a legacy pad");
        assert_eq!(tower.slot, FREE_TOWER_SLOT, "free tower claimed a hidden pad");
        assert_eq!(g.gold, gold - TOWERS[root].gold as i64, "purchase did not charge once");
    }

    let gold = g.gold;
    g.build_choice = Some((root, 1));
    assert_eq!(
        g.buildability_at(g.board.start(), true),
        Err(PlacementIssue::Road),
        "route geometry stopped rejecting a full tower footprint"
    );
    assert!(!g.try_build_at(g.board.start()));
    assert_eq!(g.gold, gold, "road click spent gold");

    // Retired landmark coordinates and visible grass cover may not turn into
    // invisible blockers.  A player can see and build on these positions;
    // only the actual road, world boundary and tower footprint may reject.
    for p in [[12.0, 11.6], [10.3, 6.2], [15.7, 6.1], [-4.0, 4.0]] {
        assert_ne!(
            g.buildability_at(p, true),
            Err(PlacementIssue::SolidScenery),
            "visual meadow cover became an invisible blocker at {p:?}",
        );
    }

    assert_eq!(
        g.buildability_at([BUILD_WORLD[2] + 2.0, BUILD_WORLD[3] + 2.0], true),
        Err(PlacementIssue::OutsideWorld)
    );
    assert!(!g.try_build_at([BUILD_WORLD[2] + 2.0, BUILD_WORLD[3] + 2.0]));
    assert_eq!(g.gold, gold, "outside click spent gold");

    let occupied = g.towers[0].pos;
    assert_eq!(g.buildability_at(occupied, true), Err(PlacementIssue::TowerOverlap));
    assert!(!g.try_build_at(occupied));
    assert_eq!(g.gold, gold, "overlap click spent gold");

    g.gold = 0;
    assert_eq!(g.buildability_at(g.first_clear_grass().unwrap(), true), Err(PlacementIssue::NotEnoughGold));
}

/// Kill money has to keep up with the roster, or the run is decided by
/// arithmetic rather than by play. This is the one number the map does not
/// contain, so it is checked against the roster it has to buy.
#[test]
fn kill_money_keeps_pace_with_the_roster() {
    // Everything one board could earn by killing every creep in the campaign.
    let purse: i64 = (1..=CAMPAIGN_WAVES)
        .map(|n| {
            let w = wave_at(n);
            bounty_of(&w) as i64 * w.count as i64 + wave_clear_bonus(n) as i64
        })
        .sum();
    // What a full board of maxed towers costs. Nobody buys the whole roster, so
    // this is thirty towers at the price of a maxed Siege ladder.
    let ladder: u32 = TOWERS
        .iter()
        .filter(|t| t.family == Family::Siege)
        .map(|t| t.gold)
        .sum();
    let board = ladder as i64 * 30;
    assert!(
        purse > board,
        "a whole campaign pays {purse} and a board costs {board}"
    );
    assert!(
        purse < board * 40,
        "a whole campaign pays {purse}, which is {}x a full board",
        purse / board.max(1)
    );

    // And the early game has to be playable on a thousand gold.
    let first_five: i64 = (1..=5)
        .map(|n| {
            let w = wave_at(n);
            bounty_of(&w) as i64 * w.count as i64 + wave_clear_bonus(n) as i64
        })
        .sum();
    assert!(
        (400..40_000).contains(&first_five),
        "the first five waves pay {first_five}"
    );
}

// ---------------------------------------------------------------- playing it

/// A board with a real answer to everything clears the campaign.
///
/// This is the test that says the port is a *game*. It plays the whole thing:
/// buying towers as gold allows, spreading across the roster the way a player
/// would, and it has to survive thirty-six waves.
#[test]
fn a_sensible_build_clears_the_campaign() {
    let mut g = Game::new();
    g.start_run(0x5CA1_AB1E);
    let mut plan = Planner::default();

    for _ in 0..(CAMPAIGN_WAVES + 6) {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        plan.spend(&mut g);
        if g.prep {
            g.send_wave();
        }
        let target = g.wave;
        let mut elapsed = 0.0;
        let mut next_spend = 1.0;
        while g.wave == target && elapsed < WAVE_PERIOD * 3.0 {
            g.update(1.0 / 60.0);
            elapsed += 1.0 / 60.0;
            // Players can place or upgrade as kill gold arrives. Waiting until
            // a wave boundary turns the intended live-build game into an
            // artificial cash bank, which is especially misleading for the
            // winding horde route.
            if elapsed >= next_spend {
                plan.spend(&mut g);
                next_spend += 1.0;
            }
            if matches!(g.phase, Phase::Defeat | Phase::Victory) {
                break;
            }
        }
    }
    // Finish off whatever is left of the last stream.
    let mut elapsed = 0.0;
    while !matches!(g.phase, Phase::Defeat | Phase::Victory) && elapsed < 400.0 {
        plan.spend(&mut g);
        for _ in 0..60 {
            g.update(1.0 / 60.0);
        }
        elapsed += 1.0;
    }

    assert_eq!(
        g.phase,
        Phase::Victory,
        "a played board reached wave {} with {}/{FLOOD_LIMIT} circling and {} towers",
        g.wave,
        g.creeps.len(),
        g.towers.len()
    );
}

/// The recommended mode is harder, but its milestone choices must make a
/// thoughtful counter-build viable. A default mode that only the test-friendly
/// Classic curve can finish would be a trap on the title screen.
#[test]
fn a_sensible_build_can_master_veteran() {
    let mut g = Game::new();
    g.start_run_with_difficulty(0x5CA1_AB1E, Difficulty::Veteran);
    let mut plan = Planner::default();

    for _ in 0..(CAMPAIGN_WAVES + 8) {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        if g.pending_doctrine {
            let pick = match g.doctrine_picks() {
                0 | 2 => Doctrine::Arsenal,
                _ => Doctrine::Overdrive,
            };
            g.choose_doctrine(pick);
        }
        plan.spend(&mut g);
        let target = g.wave;
        let mut elapsed = 0.0;
        while g.wave == target && elapsed < WAVE_PERIOD * 3.0 {
            g.update(1.0 / 60.0);
            elapsed += 1.0 / 60.0;
            if matches!(g.phase, Phase::Defeat | Phase::Victory) {
                break;
            }
        }
    }
    let mut elapsed = 0.0;
    while !matches!(g.phase, Phase::Defeat | Phase::Victory) && elapsed < 500.0 {
        if g.pending_doctrine {
            g.choose_doctrine(Doctrine::Arsenal);
        }
        plan.spend(&mut g);
        for _ in 0..60 {
            g.update(1.0 / 60.0);
        }
        elapsed += 1.0;
    }

    let surviving_commanders: Vec<(u32, u32, u32, bool)> = g
        .creeps
        .iter()
        .filter(|c| c.is_boss())
        .map(|c| (c.uid, c.laps, c.hp.max(0.0).round() as u32, c.flying))
        .collect();
    assert_eq!(
        g.phase,
        Phase::Victory,
        "Veteran ended on wave {} with {}/{} circling, {} towers ({}), {}g cash, {}g earned, {}g spent, peak {}, commanders {:?}, and doctrines {:?}",
        g.wave,
        g.creeps.len(),
        g.flood_limit(),
        g.towers.len(),
        board_summary(&g),
        g.gold,
        g.stats.gold_earned,
        g.stats.gold_spent,
        g.stats.peak_circling,
        surviving_commanders,
        g.doctrines
    );
    assert_eq!(g.doctrine_picks(), 3);
    assert!(
        g.stats.peak_circling >= 350,
        "Veteran never created meaningful ring pressure: peak {}",
        g.stats.peak_circling
    );
    assert!(
        g.stats.gold_spent * 10 >= g.stats.gold_earned * 7,
        "Veteran still handed the winning plan a large idle surplus: earned {}, spent {}",
        g.stats.gold_earned,
        g.stats.gold_spent
    );
}

/// Spending every coin on a carpet of unupgraded starter towers is not a
/// strategy. This board deliberately includes all eleven shop families and
/// spreads them across random free pads, so it cannot fail merely because it
/// forgot Air or Chaos. It must fail because late armour and Vanguards demand
/// positioning, upgrades and a coherent damage plan.
#[test]
fn shallow_mixed_spam_cannot_clear_veteran() {
    let mut g = Game::new();
    g.start_run_with_difficulty(0xBAD5_EED, Difficulty::Veteran);
    let shop = shop_order();
    let mut next_family = 0usize;
    let mut shuffle = crate::rng::Rng::new(0x5A11_0BAD);

    for _ in 0..(CAMPAIGN_WAVES + 8) {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        if g.pending_doctrine {
            // Even giving spam the straightforward damage doctrine must not
            // turn it into an accidental winning plan.
            g.choose_doctrine(Doctrine::Arsenal);
        }

        for _ in 0..g.board.slots.len() {
            let def = shop[next_family % shop.len()];
            if !g.can_afford(TOWERS[def].gold) {
                break;
            }
            let free: Vec<usize> = g
                .board
                .slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.tower.is_none().then_some(i))
                .collect();
            if free.is_empty() {
                break;
            }
            let slot = free[shuffle.next_u32() as usize % free.len()];
            g.build_choice = Some((def, 1));
            assert!(
                g.try_build(slot),
                "spam build should fit a free protected pad"
            );
            g.build_choice = None;
            next_family += 1;
        }

        if g.prep {
            g.send_wave();
        }
        let target = g.wave;
        let mut elapsed = 0.0;
        while g.wave == target && elapsed < WAVE_PERIOD * 3.0 {
            g.update(1.0 / 60.0);
            elapsed += 1.0 / 60.0;
            if matches!(g.phase, Phase::Defeat | Phase::Victory) {
                break;
            }
        }
    }

    assert_eq!(
        g.phase,
        Phase::Defeat,
        "{} randomly placed, unupgraded towers cleared Veteran at wave {}",
        g.towers.len(),
        g.wave
    );
    assert!(
        g.wave >= 8,
        "mixed spam failed on wave {} before all core counters were readable",
        g.wave
    );
}

/// Nightmare is the optimisation mode. The broad, forgiving plan that clears
/// Veteran should make real progress but eventually drown here; otherwise the
/// third button is only a different label on the same solved campaign.
#[test]
fn a_generalist_plan_does_not_trivialize_nightmare() {
    let mut g = Game::new();
    g.start_run_with_difficulty(0x5CA1_AB1E, Difficulty::Nightmare);
    let mut plan = Planner::default();

    for _ in 0..(CAMPAIGN_WAVES + 8) {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        if g.pending_doctrine {
            g.choose_doctrine(Doctrine::Arsenal);
        }
        plan.spend(&mut g);
        if g.prep {
            g.send_wave();
        }
        let target = g.wave;
        let mut elapsed = 0.0;
        while g.wave == target && elapsed < WAVE_PERIOD * 3.0 {
            g.update(1.0 / 60.0);
            elapsed += 1.0 / 60.0;
            if matches!(g.phase, Phase::Defeat | Phase::Victory) {
                break;
            }
        }
    }

    assert_eq!(
        g.phase,
        Phase::Defeat,
        "the generalist Veteran plan trivialised Nightmare at wave {} with {}/{} circling",
        g.wave,
        g.creeps.len(),
        g.flood_limit()
    );
    assert!(
        g.wave >= 7,
        "Nightmare collapsed on wave {} before the first warned Air counter-check",
        g.wave
    );
}

/// And a board with no answer to the air does not.
#[test]
fn a_ground_only_board_drowns_in_the_air() {
    let mut g = Game::new();
    g.start_run(0x5CA1_AB1E);
    g.gold = 500_000_000;
    for k in 0..g.board.slots.len().min(60) {
        let ti = build(&mut g, Family::Siege, k as f32 * 1.4);
        max_out(&mut g, ti);
    }
    g.prep = false;
    g.phase = Phase::Combat;

    // Run through the first air wave. Nothing on this board can reach it, so
    // every flyer that spawns has to still be circling at the end.
    let mut spawned = 0u32;
    let mut last = 0usize;
    while g.wave < 9 && !matches!(g.phase, Phase::Defeat) {
        g.update(1.0 / 60.0);
        let n = g.creeps.iter().filter(|c| c.flying).count();
        if n > last {
            spawned += (n - last) as u32;
        }
        last = n;
    }
    assert!(spawned > 0, "no air wave arrived by wave 9");
    assert_eq!(
        last as u32,
        spawned,
        "a ground-only board killed {} of {spawned} flyers",
        spawned - last as u32
    );
}

/// Buys towers the way a player would.
///
/// Three rules form the benchmark's basic strategy:
///
///   1. **Cover what is coming.** If an air wave is due and nothing on the
///      board can reach the air, or an Immune wave is due and nothing deals
///      Chaos, everything is put on hold until that gap is filled. This is the
///      rule the roster exists for: throughput alone loses on wave five.
///   2. **Otherwise, buy whatever is cheapest** - the next tower on the plan or
///      the cheapest upgrade on the board. A player with spare gold spends it.
///   3. **Answer commanders with single-target burst.** Crowd splash cannot be
///      allowed to masquerade as a complete build once marked bosses arrive.
#[derive(Default)]
struct Planner {
    built: usize,
}

/// The order a board gets built in when nothing is urgent, as a repeating
/// pattern rather than a fixed list.
///
/// Twenty-six entries are enough variety to keep the compact circuit covered,
/// while repeating the pattern lets the plan add a third late-game layer when
/// the final command waves justify it.
const PLAN: [Family; 26] = [
    Family::Siege,
    Family::Siege,
    Family::Siege,
    Family::Siege,
    Family::Siege,
    Family::Siege,
    Family::Multi,
    Family::Siege,
    Family::Siege,
    Family::Multi,
    Family::Siege,
    Family::Siege,
    Family::Destruction,
    Family::Siege,
    Family::Siege,
    Family::Troll,
    Family::Siege,
    Family::Siege,
    Family::Destruction,
    Family::Siege,
    Family::Multi,
    Family::Siege,
    Family::Siege,
    Family::Troll,
    Family::Siege,
    Family::Siege,
];

/// How many waves ahead a gap in the board counts as urgent.
const LOOKAHEAD: u32 = 4;

/// How much lane one tower is responsible for.
///
/// A Siege Tower reaches seven tiles, so it covers about fourteen of corridor if
/// the corridor runs past it. Halved, because a lane you cover exactly once is a
/// lane where every monster is under fire for one tower's worth of time and no
/// more, and that is not enough throughput to hold a circuit.
const LANE_PER_TOWER: f32 = 7.0;

/// How many towers it takes to cover this board's lane.
fn coverage_target(g: &Game) -> usize {
    (g.board.total / LANE_PER_TOWER).ceil() as usize
}

/// The family to build `n`th, cycling the plan once it runs out.
///
/// The tail of the pattern is mostly Siege with Multi, Destruction and Troll
/// mixed through it, which is the shape a real board keeps as it grows.
fn plan_at(n: usize) -> Family {
    PLAN[n % PLAN.len()]
}

impl Planner {
    fn spend(&mut self, g: &mut Game) {
        for _ in 0..500 {
            // 1. A hole in the board's coverage outranks everything else -
            //    but only once it is nearly paid for. Saving from nothing, for
            //    four waves, with four towers on the board, is how a player
            //    loses on wave seven with an Air Tower they never bought.
            if let Some(family) = urgent(g) {
                let cost = seed_cost(family);
                if g.gold >= cost {
                    if self.place(g, family) {
                        continue;
                    }
                } else if g.gold * 2 >= cost {
                    return;
                }
            }

            // 2. Cover the lane before deepening anything on it, at whatever
            //    length this map's lane happens to be - see `coverage_target`.
            let target = coverage_target(g);
            // A third layer is late-game reinforcement, not cheap-tower spam:
            // establish two layers, finish their upgrade paths, then add and
            // finish one reserve tower at a time.
            let mature = g
                .towers
                .iter()
                .enumerate()
                .all(|(ti, _)| g.upgrade_choices(ti).is_empty());
            let can_expand = self.built < target * 2 || (self.built < target * 3 && mature);
            let build = if can_expand {
                let f = plan_at(self.built);
                Some((f, seed_cost(f)))
            } else {
                None
            };
            let up = cheapest_upgrade(g);
            // Cover the whole lane before deepening any of it. A few very good
            // towers still watch only a fraction of the compact circuit, while
            // the rest of the wave walks past them.
            let spread = g.towers.len() < target;
            let take_build = match (build, up) {
                (Some((_, bc)), Some((_, _, uc))) => spread || bc <= uc as i64,
                (Some(_), None) => true,
                _ => false,
            };
            if take_build {
                let (family, cost) = build.expect("checked");
                if g.gold < cost || !self.place(g, family) {
                    return;
                }
                continue;
            }
            let Some((ti, into, cost)) = up else { return };
            if !g.can_afford(cost) {
                return;
            }
            g.upgrade_into(ti, into);
        }
    }

    /// Puts one tower of `family` down, through the seed if that is the only
    /// way to reach it. False if there is no pad or no money.
    ///
    /// Beside the lane, spread along it. The compact arena offers a deliberate
    /// set of protected pads and only the ones within a tower's reach of the
    /// corridor are worth anything.
    fn place(&mut self, g: &mut Game, family: Family) -> bool {
        // Spread along the whole lane, wrapping so a second pass fills the gaps
        // between the first rather than piling up past the end of it.
        let target = coverage_target(g).max(1);
        let along = (self.built as f32 + 0.5) / target as f32 * g.board.total;
        let along = along % g.board.total;
        let at = g.board.sample(along);
        let Some(slot) = (0..g.board.slots.len())
            .filter(|&i| g.board.slots[i].tower.is_none())
            .min_by(|&a, &b| {
                let d = |i: usize| {
                    let p = g.board.slots[i].pos;
                    (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2)
                };
                d(a).total_cmp(&d(b))
            })
        else {
            return false;
        };
        let seed = if family_is_shop(family) {
            root(family)
        } else {
            root(Family::Single)
        };
        g.build_choice = Some((seed, 1));
        let ok = g.try_build(slot);
        g.build_choice = None;
        if !ok {
            return false;
        }
        let ti = g.towers.len() - 1;
        if TOWERS[seed].family == Family::Single {
            g.upgrade_into(ti, root(family));
        }
        self.built += 1;
        true
    }
}

/// A capability the board is missing and is about to need.
fn urgent(g: &Game) -> Option<Family> {
    let soon = |f: fn(&WaveDef) -> bool| {
        (1..=LOOKAHEAD).any(|k| {
            let n = g.wave + k;
            n <= g.last_wave() && f(&wave_at(n))
        })
    };
    let has_air = g.towers.iter().any(|t| t.targets().can_hit(true));
    if !has_air && soon(|w| w.flying) {
        return Some(Family::Air);
    }
    let has_chaos = g
        .towers
        .iter()
        .any(|t| matches!(t.attack(), Attack::Chaos | Attack::Hero));
    if !has_chaos && soon(|w| w.armour_type == ArmourType::Divine) {
        return Some(Family::Chaos);
    }
    let has_commander_hunter = g
        .towers
        .iter()
        .any(|t| matches!(t.family(), Family::OneStrike | Family::King));
    if !has_commander_hunter && soon(|w| w.tag == "Boss") {
        return Some(Family::OneStrike);
    }
    None
}

/// What one tower of this family costs to get to, seed included.
fn seed_cost(f: Family) -> i64 {
    if family_is_shop(f) {
        TOWERS[root(f)].gold as i64
    } else {
        (TOWERS[root(Family::Single)].gold + TOWERS[root(f)].gold) as i64
    }
}

fn cheapest_upgrade(g: &Game) -> Option<(usize, usize, u32)> {
    let mut best: Option<(usize, usize, u32)> = None;
    for ti in 0..g.towers.len() {
        for (into, cost) in g.upgrade_choices(ti) {
            if best.is_none_or(|(_, _, c)| cost < c) {
                best = Some((ti, into, cost));
            }
        }
    }
    best
}

fn family_is_shop(f: Family) -> bool {
    family_start(f).is_some_and(|i| TOWERS[i].shop)
}

// ---------------------------------------------------------------- reporting

/// Plays a whole campaign and prints what happened, wave by wave. Not an
/// assertion - it is how balance is actually looked at:
///     cargo test --release a_narrated_playthrough -- --ignored --nocapture
#[test]
#[ignore = "prints a whole run"]
fn a_narrated_playthrough() {
    let mut g = Game::new();
    g.start_run(0x5CA1_AB1E);
    let mut plan = Planner::default();

    println!();
    println!("======================= GREEN CIRCLE TD =======================");
    println!(
        "  seed {:#x}   ring holds {FLOOD_LIMIT}   {} gold",
        g.seed, g.gold
    );

    for _ in 0..(CAMPAIGN_WAVES + 4) {
        if matches!(g.phase, Phase::Defeat | Phase::Victory) {
            break;
        }
        plan.spend(&mut g);
        let wave = g.wave + 1;
        let w = g.wave_def(wave);

        if g.prep {
            g.send_wave();
        }
        let target = g.wave;
        let mut elapsed = 0.0;
        let mut peak = g.creeps.len();
        while g.wave == target && elapsed < WAVE_PERIOD * 3.0 {
            g.update(1.0 / 60.0);
            elapsed += 1.0 / 60.0;
            peak = peak.max(g.creeps.len());
            if matches!(g.phase, Phase::Defeat | Phase::Victory) {
                break;
            }
        }
        println!(
            "  wave {:>2}  {:<16} x{:<4} {:<10} {:<6}  ring {:>3}/{FLOOD_LIMIT}  gold {:>12}  towers {:>3}",
            wave,
            w.name,
            w.count,
            w.armour_type.name(),
            w.tag,
            peak,
            g.gold,
            g.towers.len()
        );
    }

    println!();
    println!("  finished at wave {} - {:?}", g.wave, g.phase);
    println!("  {}", board_summary(&g));
    println!(
        "  kills {}  damage {:.3e}  earned {}",
        g.stats.kills, g.stats.damage, g.stats.gold_earned
    );
}

fn board_summary(g: &Game) -> String {
    let mut counts: Vec<(Family, usize, u32)> = Vec::new();
    for t in &g.towers {
        match counts.iter_mut().find(|(f, _, _)| *f == t.family()) {
            Some(e) => {
                e.1 += 1;
                e.2 = e.2.max(t.level());
            }
            None => counts.push((t.family(), 1, t.level())),
        }
    }
    counts.sort_by_key(|(_, n, _)| std::cmp::Reverse(*n));
    counts
        .iter()
        .map(|(f, n, top)| format!("{}x{} (to {})", n, f.name(), top))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Plays a board the way [`Planner`] does, for the screenshot harness.
///
/// `built` is carried between calls so a caller stepping through a whole run
/// keeps one planner's worth of state without owning the type.
pub(crate) fn spend_for_shot(g: &mut Game, built: &mut usize) {
    let mut plan = Planner { built: *built };
    plan.spend(g);
    *built = plan.built;
}

fn find_in_range_legal_spot(g: &Game, def: usize) -> Option<[f32; 2]> {
    let range = TOWERS[def].range;
    let b = g.board.build_world();
    let mut best: Option<([f32; 2], f32)> = None;
    let mut y = b[1] + TOWER_FOOTPRINT_RADIUS;
    while y <= b[3] - TOWER_FOOTPRINT_RADIUS {
        let mut x = b[0] + TOWER_FOOTPRINT_RADIUS;
        while x <= b[2] - TOWER_FOOTPRINT_RADIUS {
            if let Ok(pos) = g.buildability_at([x, y], true) {
                let d = g.board.dist_to_road(pos);
                if d <= range {
                    match best {
                        None => best = Some((pos, d)),
                        Some((_, best_d)) if d < best_d => best = Some((pos, d)),
                        _ => {}
                    }
                }
            }
            x += 0.50;
        }
        y += 0.50;
    }
    best.map(|(p, _)| p)
}

fn find_best_coverage_legal_spot(g: &Game, def: usize) -> Option<[f32; 2]> {
    let range = TOWERS[def].range;
    let r2 = range * range;
    let b = g.board.build_world();
    let mut best: Option<([f32; 2], f32)> = None;
    let mut y = b[1] + TOWER_FOOTPRINT_RADIUS;
    while y <= b[3] - TOWER_FOOTPRINT_RADIUS {
        let mut x = b[0] + TOWER_FOOTPRINT_RADIUS;
        while x <= b[2] - TOWER_FOOTPRINT_RADIUS {
            if let Ok(pos) = g.buildability_at([x, y], true) {
                let d = g.board.dist_to_road(pos);
                if d <= range {
                    let mut covered = 0.0_f32;
                    let mut dist = 0.0_f32;
                    while dist < g.board.total {
                        let pt = g.board.sample(dist);
                        let dx = pt[0] - pos[0];
                        let dy = pt[1] - pos[1];
                        if dx * dx + dy * dy <= r2 {
                            covered += 1.0;
                        }
                        dist += 1.0;
                    }
                    let score = covered * 10.0 - d;
                    match best {
                        None => best = Some((pos, score)),
                        Some((_, best_score)) if score > best_score => best = Some((pos, score)),
                        _ => {}
                    }
                }
            }
            x += 0.50;
        }
        y += 0.50;
    }
    best.map(|(p, _)| p)
}

fn find_entrance_aware_legal_spot(g: &Game, def: usize, initial: bool) -> Option<[f32; 2]> {
    let range = TOWERS[def].range;
    let r2 = range * range;
    let b = g.board.build_world();
    let mut best: Option<([f32; 2], f32)> = None;
    let mut y = b[1] + TOWER_FOOTPRINT_RADIUS;
    while y <= b[3] - TOWER_FOOTPRINT_RADIUS {
        let mut x = b[0] + TOWER_FOOTPRINT_RADIUS;
        while x <= b[2] - TOWER_FOOTPRINT_RADIUS {
            if let Ok(pos) = g.buildability_at([x, y], true) {
                let d = g.board.dist_to_road(pos);
                if d <= range {
                    // 1. Entrance route scoring: sample first 12 route units in BOTH directions from spawn:
                    let mut entrance_covered = 0.0_f32;
                    let mut covers_cw = false;
                    let mut covers_ccw = false;
                    for step in 0..=12 {
                        let prog = step as f32;
                        let p_cw = g.board.sample_travel(prog, 1.0);
                        let p_ccw = g.board.sample_travel(prog, -1.0);
                        let d_cw2 = (p_cw[0] - pos[0]).powi(2) + (p_cw[1] - pos[1]).powi(2);
                        let d_ccw2 = (p_ccw[0] - pos[0]).powi(2) + (p_ccw[1] - pos[1]).powi(2);
                        if d_cw2 <= r2 {
                            entrance_covered += 1.0;
                            if step > 0 { covers_cw = true; }
                        }
                        if d_ccw2 <= r2 {
                            entrance_covered += 1.0;
                            if step > 0 { covers_ccw = true; }
                        }
                    }

                    // Direct spawn point coverage (dist = 0):
                    let spawn_p = g.board.sample(0.0);
                    let covers_spawn = (spawn_p[0] - pos[0]).powi(2) + (spawn_p[1] - pos[1]).powi(2) <= r2;

                    // 2. General route coverage:
                    let mut general_covered = 0.0_f32;
                    let mut dist = 0.0_f32;
                    while dist < g.board.total {
                        let pt = g.board.sample(dist);
                        let dx = pt[0] - pos[0];
                        let dy = pt[1] - pos[1];
                        if dx * dx + dy * dy <= r2 {
                            general_covered += 1.0;
                        }
                        dist += 1.0;
                    }

                    let dual_flank_bonus = if covers_cw && covers_ccw { 100.0 } else { 0.0 };
                    let spawn_bonus = if covers_spawn { 60.0 } else { 0.0 };

                    let score = if initial {
                        entrance_covered * 40.0 + dual_flank_bonus + spawn_bonus + general_covered * 5.0 - d
                    } else {
                        entrance_covered * 15.0 + general_covered * 15.0 - d
                    };

                    match best {
                        None => best = Some((pos, score)),
                        Some((_, best_score)) if score > best_score => best = Some((pos, score)),
                        _ => {}
                    }
                }
            }
            x += 0.50;
        }
        y += 0.50;
    }
    best.map(|(p, _)| p)
}

fn def_by_family(family: Family) -> usize {
    TOWERS
        .iter()
        .position(|t| t.family == family && t.shop)
        .or_else(|| family_start(family))
        .unwrap_or_else(|| panic!("No legal start tower for family {family:?}"))
}

#[test]
fn campaign_difficulty_baseline_simulations() {
    // 1. Unattended run on Veteran fails early due to pressure overflow
    let mut g_unattended = Game::new();
    g_unattended.start_campaign(42, Difficulty::Veteran);
    g_unattended.speed = 4.0;
    g_unattended.update(CAMPAIGN_AUTOSTART_SECONDS + 0.1);
    let mut ticks = 0;
    while g_unattended.phase != Phase::Defeat && ticks < 20_000 {
        g_unattended.update(0.1);
        ticks += 1;
    }
    assert_eq!(
        g_unattended.phase,
        Phase::Defeat,
        "Unattended Veteran board must suffer defeat"
    );
    assert!(
        g_unattended.wave <= 10,
        "Unattended run must fail in early encounters (failed at encounter {})",
        g_unattended.wave
    );

    // 2. Shallow single-type un-upgraded spam fails against mixed waves
    let mut g_spam = Game::new();
    g_spam.start_campaign(42, Difficulty::Veteran);
    g_spam.speed = 2.0;
    let single_def = def_by_family(Family::Single);
    for _ in 0..8 {
        if let Some(pos) = find_in_range_legal_spot(&g_spam, single_def) {
            g_spam.build_choice = Some((single_def, 1));
            let _ = g_spam.try_build_at(pos);
        }
    }
    g_spam.send_wave();
    let mut spam_ticks = 0;
    while g_spam.phase != Phase::Defeat && spam_ticks < 20_000 && g_spam.wave < 15 {
        if g_spam.pending_doctrine {
            g_spam.choose_doctrine(Doctrine::Arsenal);
        }
        if g_spam.phase == Phase::Build {
            if let Some(pos) = find_in_range_legal_spot(&g_spam, single_def) {
                g_spam.build_choice = Some((single_def, 1));
                let _ = g_spam.try_build_at(pos);
            }
            g_spam.send_wave();
        }
        g_spam.update(0.1);
        spam_ticks += 1;
    }
    assert_eq!(
        g_spam.phase,
        Phase::Defeat,
        "Shallow un-upgraded spam must fail to hold mixed campaign encounters"
    );

    // 3. Competent mixed strategy with legal in-range purchases and upgrades
    let mut g_mixed = Game::new();
    g_mixed.start_campaign(42, Difficulty::Veteran);
    g_mixed.speed = 2.0;

    let siege_def = def_by_family(Family::Siege);
    let single_def = def_by_family(Family::Single);

    // Build Siege Tower (splash) on legal in-range grass
    let p_siege = find_in_range_legal_spot(&g_mixed, siege_def).expect("legal spot for siege");
    g_mixed.build_choice = Some((siege_def, 1));
    assert!(g_mixed.try_build_at(p_siege));
    let siege_idx = 0;

    // Upgrade Siege to Tier 2 (Demon Hunter)
    g_mixed.upgrade(siege_idx);

    // Build Single-target Tower for focused DPS
    let p_single = find_in_range_legal_spot(&g_mixed, single_def).expect("legal spot for single");
    g_mixed.build_choice = Some((single_def, 1));
    assert!(g_mixed.try_build_at(p_single));

    // Send wave and run through multiple encounters
    g_mixed.send_wave();
    let mut ticks = 0;
    while g_mixed.phase != Phase::Defeat && ticks < 25_000 && g_mixed.wave < 7 {
        if g_mixed.pending_doctrine {
            let pick = Doctrine::ALL
                .into_iter()
                .find(|&d| g_mixed.doctrine_rank(d) < 3)
                .unwrap_or(Doctrine::Arsenal);
            g_mixed.choose_doctrine(pick);
        }
        if g_mixed.phase == Phase::Build {
            // Reinvest: upgrade existing towers or add new towers
            for ti in 0..g_mixed.towers.len() {
                g_mixed.upgrade(ti);
            }
            if let Some(pos) = find_in_range_legal_spot(&g_mixed, siege_def) {
                g_mixed.build_choice = Some((siege_def, 1));
                let _ = g_mixed.try_build_at(pos);
            }
            g_mixed.send_wave();
        }
        g_mixed.update(0.1);
        ticks += 1;
    }

    assert_ne!(
        g_mixed.phase,
        Phase::Defeat,
        "Competent mixed build should survive early encounters"
    );
    assert!(
        g_mixed.wave >= 5,
        "Competent mixed build should advance across multiple encounters (reached {})",
        g_mixed.wave
    );
}

fn boundary_fixture(encounter: u16) -> Game {
    let mut g = Game::new();
    assert!(g.start_campaign_diagnostic_fixture(
        0xCA11_6000 + encounter as u64,
        Difficulty::Classic,
        encounter,
    ));
    g.speed = 2.0;
    g
}

fn pilot_tick(g: &mut Game) {
    if g.pending_doctrine {
        let pick = Doctrine::ALL
            .into_iter()
            .find(|&d| g.doctrine_rank(d) < 3)
            .expect("campaign perk cap left no legal choice");
        g.choose_doctrine(pick);
        return;
    }
    if g.phase == Phase::Build {
        g.send_wave();
    }
    g.update(0.125);
    let alive = g.creeps.len();
    for ci in 0..alive {
        if g.creeps[ci].hp > 0.0 {
            combat::damage_creep(g, ci, 1_000_000.0, usize::MAX, false);
        }
    }
    if g
        .campaign
        .as_ref()
        .is_some_and(campaign::CampaignState::can_rush)
    {
        g.send_wave();
    }
}

fn run_until(g: &mut Game, limit: usize, predicate: impl Fn(&Game) -> bool) {
    for _ in 0..limit {
        if predicate(g) {
            return;
        }
        pilot_tick(g);
    }
    panic!(
        "campaign diagnostic fixture exceeded {limit} production ticks: phase {:?}, wave {}, creeps {}, expected reward {:?}, campaign {:?}",
        g.phase,
        g.wave,
        g.creeps.len(),
        g.campaign.as_ref().map(|s| s.current().reward),
        g.campaign,
    );
}

#[test]
fn campaign_boundary_fixtures_use_live_transition_code() {
    let mut thirty_five = boundary_fixture(35);
    run_until(&mut thirty_five, 2_500, |g| {
        g.phase == Phase::Combat
            && g.campaign_encounter() == Some(37)
            && g.wave == 37
    });
    assert_ne!(thirty_five.phase, Phase::Victory);
    assert_eq!(thirty_five.campaign_chapter(), Some(1));

    let mut sixty = boundary_fixture(60);
    run_until(&mut sixty, 2_500, |g| {
        g.phase == Phase::Combat
            && g.campaign_encounter() == Some(61)
            && g.wave == 61
    });
    assert_ne!(sixty.phase, Phase::Victory);
    assert_eq!(sixty.campaign_chapter(), Some(2));
    assert!(sixty.gold > 0, "chapter purse did not enter the live economy");
}

#[test]
fn campaign_economy_is_tuned_by_difficulty() {
    // 1. Starting gold distinction
    assert_eq!(Difficulty::Classic.campaign_starting_gold(), 600);
    assert_eq!(Difficulty::Veteran.campaign_starting_gold(), 500);
    assert_eq!(Difficulty::Nightmare.campaign_starting_gold(), 420);

    let enc1 = campaign::resolved_encounter(1);
    let raw_reward = enc1.reward;
    let total_bodies: usize = enc1.packets.iter().map(|p| p.bodies as usize).sum();

    let classic_budget = Difficulty::Classic.campaign_encounter_budget(raw_reward, 1, 1);
    let vet_budget = Difficulty::Veteran.campaign_encounter_budget(raw_reward, 1, 1);
    let nm_budget = Difficulty::Nightmare.campaign_encounter_budget(raw_reward, 1, 1);

    assert_eq!(classic_budget, raw_reward);
    assert_eq!(vet_budget, (raw_reward as f32 * 0.82).round() as u32);
    assert_eq!(nm_budget, (raw_reward as f32 * 0.68).round() as u32);
    assert!(classic_budget > vet_budget);
    assert!(vet_budget > nm_budget);

    // 2. Compare live deployment payout on Encounter 1 across Classic, Veteran, Nightmare
    let mut g_classic = Game::new();
    g_classic.start_campaign(42, Difficulty::Classic);
    assert_eq!(g_classic.gold, 600);
    g_classic.send_wave();
    let classic_payout = (g_classic.gold - 600) as u32;
    assert_eq!(classic_payout, classic_budget * 40 / 100);

    let mut g_vet = Game::new();
    g_vet.start_campaign(42, Difficulty::Veteran);
    assert_eq!(g_vet.gold, 500);
    g_vet.send_wave();
    let vet_payout = (g_vet.gold - 500) as u32;
    assert_eq!(vet_payout, vet_budget * 40 / 100);

    let mut g_nm = Game::new();
    g_nm.start_campaign(42, Difficulty::Nightmare);
    assert_eq!(g_nm.gold, 420);
    g_nm.send_wave();
    let nm_payout = (g_nm.gold - 420) as u32;
    assert_eq!(nm_payout, nm_budget * 40 / 100);

    assert!(classic_payout > vet_payout);
    assert!(vet_payout > nm_payout);

    // 3. Spawning all bodies on Veteran:
    // Scaled kill budget = vet_budget - vet_payout.
    // Quotients and remainders distribute exactly with zero payouts allowed.
    let vet_kill_budget = vet_budget - vet_payout;
    let expected_one_bounties = (vet_kill_budget % total_bodies as u32) as usize;
    let expected_zero_bounties = total_bodies - expected_one_bounties;

    g_vet.campaign_pressure_grace = 1_000_000.0;
    while g_vet.campaign.as_ref().unwrap().queued_bodies_left > 0 || g_vet.creeps.len() < total_bodies {
        g_vet.update(0.1);
    }
    assert_eq!(g_vet.creeps.len(), total_bodies);
    let zero_bounties = g_vet.creeps.iter().filter(|c| c.bounty == 0).count();
    let one_bounties = g_vet.creeps.iter().filter(|c| c.bounty == 1).count();
    assert_eq!(one_bounties, expected_one_bounties);
    assert_eq!(zero_bounties, expected_zero_bounties, "Zero payouts must be allowed for excess bodies");

    let total_bounties: u32 = g_vet.creeps.iter().map(|c| c.bounty).sum();
    assert_eq!(total_bounties, vet_kill_budget);

    // 4. Defeat all creeps and collect all bounties
    for ci in 0..g_vet.creeps.len() {
        combat::damage_creep(&mut g_vet, ci, 1_000_000.0, usize::MAX, false);
    }
    g_vet.update(0.1);
    assert_eq!(g_vet.creeps.len(), 0, "All creeps must be cleared");
    assert_eq!(
        g_vet.gold,
        500 + vet_budget as i64,
        "Total gold earned is starting gold + exact scaled encounter budget"
    );

    // 5. Advance until encounter 2 transitions
    for _ in 0..150 {
        if g_vet.campaign_encounter() == Some(2) {
            break;
        }
        g_vet.update(0.2);
    }
    assert_eq!(g_vet.campaign_encounter(), Some(2));
    assert_eq!(g_vet.gold, 500 + vet_budget as i64);

    // 6. Save and load verification: assert exact total across full kill and save-load
    g_vet.campaign_pressure_grace = crate::game::campaign::BREACH_SECONDS;
    let saved = crate::save::Save::capture(&g_vet);
    let mut g_restored = Game::new();
    assert!(
        saved.restore(&mut g_restored),
        "Campaign state with zero bounties and scaled ledger restores cleanly"
    );
    assert_eq!(g_restored.gold, 500 + vet_budget as i64);
    assert_eq!(g_restored.campaign_encounter(), Some(2));
}

#[test]
fn campaign_commander_lap_limit_triggers_defeat_on_veteran() {
    assert_eq!(Difficulty::Veteran.commander_lap_limit(), Some(4));
    assert_eq!(Difficulty::Nightmare.commander_lap_limit(), Some(3));
    assert_eq!(Difficulty::Classic.commander_lap_limit(), None);

    let mut g_vet = Game::new();
    g_vet.start_campaign(42, Difficulty::Veteran);
    g_vet.send_wave();

    let wave_def = WaveDef {
        name: "Test Commander",
        tag: "Commander",
        model: Model::Warrior,
        scale: 1.5,
        count: 1,
        hp: 5000.0,
        armour: 5,
        armour_type: ArmourType::Hero,
        speed: 1.0,
        flying: false,
        spawn_gap: 0.0,
        lead_in: 0.0,
    };
    g_vet.spawn_creep_ranked(&wave_def, wave_def.hp, 1.0, 0.0, false, true);
    let boss_idx = g_vet.creeps.len() - 1;
    g_vet.creeps[boss_idx].boss = true;
    g_vet.creeps[boss_idx].laps = 3;

    g_vet.check_end();
    assert_eq!(g_vet.phase, Phase::Combat, "3 laps on Veteran should not defeat yet");

    g_vet.creeps[boss_idx].laps = 4;
    g_vet.check_end();
    assert_eq!(g_vet.phase, Phase::Defeat, "4 laps on Veteran must trigger Phase::Defeat");

    let mut g_classic = Game::new();
    g_classic.start_campaign(42, Difficulty::Classic);
    g_classic.send_wave();
    g_classic.spawn_creep_ranked(&wave_def, wave_def.hp, 1.0, 0.0, false, true);
    let classic_boss_idx = g_classic.creeps.len() - 1;
    g_classic.creeps[classic_boss_idx].boss = true;
    g_classic.creeps[classic_boss_idx].laps = 5;
    g_classic.check_end();
    assert_ne!(g_classic.phase, Phase::Defeat, "Classic mode has no commander lap limit");
}

#[test]
fn campaign_commander_classes_change_rules_and_save_their_one_shots() {
    let commander = WaveDef {
        name: "Commander", tag: "Commander", model: Model::Warrior, scale: 1.2,
        count: 1, hp: 900.0, armour: 8, armour_type: ArmourType::Hero,
        speed: 0.8, flying: false, spawn_gap: 0.0, lead_in: 0.0,
    };

    // C1E10 is a Bulwark: breaking its physical shield opens a finite,
    // visibly different armour window rather than changing an HP multiplier.
    let mut bulwark = Game::new();
    bulwark.start_campaign(77, Difficulty::Veteran);
    bulwark.campaign.as_mut().unwrap().encounter = 10;
    bulwark.spawn_creep_ranked(&commander, commander.hp, 1.0, 0.0, false, true);
    let b = bulwark.creeps.len() - 1;
    bulwark.creeps[b].campaign_encounter = 10;
    bulwark.creeps[b].shield = 100.0;
    bulwark.creeps[b].max_shield = 100.0;
    bulwark.creeps[b].shield = 0.0;
    bulwark.step_campaign_commander_mechanics(0.1);
    assert_eq!(bulwark.creeps[b].armour, 0, "Bulwark shield break exposes its core");
    assert_eq!(bulwark.campaign.as_ref().unwrap().commander_triggers & 1, 1);

    // C1E30 is a Brood Keeper: thresholds add a fixed twelve zero-bounty
    // bodies and the trigger mask survives an exact campaign save.
    let mut brood = Game::new();
    brood.start_campaign(78, Difficulty::Veteran);
    brood.campaign.as_mut().unwrap().encounter = 30;
    brood.wave = 29;
    brood.prep = false;
    brood.begin_campaign_encounter();
    brood.spawn_creep_ranked(&commander, commander.hp, 1.0, 0.0, false, true);
    let boss = brood.creeps.len() - 1;
    brood.creeps[boss].campaign_encounter = 30;
    brood.creeps[boss].hp = brood.creeps[boss].max_hp * 0.30;
    brood.step_campaign_commander_mechanics(0.1);
    assert_eq!(brood.creeps.len(), 13, "two thresholds create two bounded broods");
    assert!(brood.creeps.iter().skip(1).all(|c| c.bounty == 0));
    let saved = crate::save::Save::capture(&brood);
    let mut restored = Game::new();
    assert!(saved.restore(&mut restored));
    assert_eq!(
        restored.campaign.as_ref().unwrap().commander_triggers,
        brood.campaign.as_ref().unwrap().commander_triggers,
        "reload preserves which brood thresholds have already fired"
    );

    // C1E20's Hunt Captain is born at the final packet, not at encounter
    // zero. Its first separated pulse fires after birth and a living boss can
    // be saved well past the 120-second packet schedule.
    let mut hunt = Game::new();
    hunt.start_campaign(79, Difficulty::Veteran);
    hunt.campaign.as_mut().unwrap().encounter = 20;
    hunt.wave = 19;
    hunt.prep = false;
    hunt.begin_campaign_encounter();
    {
        let state = hunt.campaign.as_mut().unwrap();
        state.elapsed_seconds = 146.0;
        state.commander_spawned_at = Some(122.0);
    }
    hunt.spawn_creep_ranked(&commander, commander.hp, 1.0, 2.0, false, true);
    hunt.spawn_creep_ranked(&commander, commander.hp, 1.0, 2.2, false, false);
    let hunt_boss = hunt.creeps.len() - 2;
    hunt.creeps[hunt_boss].campaign_encounter = 20;
    hunt.creeps[hunt_boss + 1].campaign_encounter = 20;
    let escort_speed = hunt.creeps[hunt_boss + 1].base_speed;
    hunt.step_campaign_commander_mechanics(0.1);
    assert_eq!(hunt.campaign.as_ref().unwrap().commander_triggers & 0x0f, 1);
    assert!(hunt.creeps[hunt_boss + 1].base_speed > escort_speed, "first Hunt pulse hastes a live escort");
    let saved_hunt = crate::save::Save::capture(&hunt);
    let mut resumed_hunt = Game::new();
    assert!(saved_hunt.restore(&mut resumed_hunt), "living commander cleanup resumes after 120 seconds");
    resumed_hunt.campaign.as_mut().unwrap().elapsed_seconds = 168.0;
    resumed_hunt.step_campaign_commander_mechanics(0.1);
    assert_eq!(resumed_hunt.campaign.as_ref().unwrap().commander_triggers & 0x0f, 2, "second Hunt pulse remains separately timed after resume");
}

#[test]
fn campaign_commander_state_resets_between_different_commander_classes() {
    let mut state = campaign::CampaignState::default();
    // E10 Bulwark has fired a mechanic; completing it must not pre-arm the
    // distinct E20 Hunt Captain. Use the production `finish` transition for
    // every intervening encounter, rather than resetting fields in a fixture.
    state.encounter = 10;
    state.commander_triggers = 0b111;
    state.commander_window_at = 123.0;
    state.commander_spawned_at = Some(120.0);
    while state.encounter < 20 {
        let current = state.current();
        state.elapsed_seconds = current.duration_seconds as f32;
        state.deployed_packets = current.packets.len() as u8;
        assert!(state.finish(0).is_some(), "E{} did not finish", current.id.global);
        assert_eq!(state.commander_triggers, 0, "E{} inherited a prior commander trigger", state.encounter);
        assert_eq!(state.commander_window_at, -1.0, "E{} inherited a prior commander window", state.encounter);
        assert_eq!(state.commander_spawned_at, None, "E{} inherited a prior commander birth", state.encounter);
    }
    assert_eq!(state.current().commander, Some(campaign::CommanderClass::HuntCaptain));
}

#[test]
fn campaign_poison_uses_tenth_legacy_rider_budget_without_touching_legacy() {
    let legacy = TOWERS[root(Family::Poison)].abil.poison_dps;
    assert_eq!(legacy, 100.0, "source Poison root arithmetic changed");
    let legacy_game = Game::new();
    assert_eq!(combat::effective_poison_dps(&legacy_game, legacy), 100.0);
    let mut campaign_game = Game::new();
    campaign_game.start_campaign(0xC015_0A_u64, Difficulty::Veteran);
    assert_eq!(combat::effective_poison_dps(&campaign_game, legacy), 10.0);
    assert_eq!(10.0 * 12.0, 120.0, "Campaign Poison root twelve-stack cap must be explicit");
}

#[test]
fn hard_campaign_commanders_use_authored_floor_once_and_classic_isolated() {
    let encounter = campaign::resolved_encounter(10);
    let packet = *encounter.packets.last().expect("commander packet");
    for difficulty in [Difficulty::Veteran, Difficulty::Nightmare] {
        let mut game = Game::new();
        game.start_campaign(0xC0AA_0100_u64, difficulty);
        game.spawn_campaign_body(&encounter, packet, 0, 0, true);
        let boss = game.creeps.last().expect("spawned commander");
        let floor = campaign_commander_hp_floor(difficulty, 10);
        assert!(boss.hp >= floor && boss.max_hp >= floor, "{difficulty:?} commander floor absent");
        assert_eq!(boss.hp, boss.max_hp, "floor did not reset current HP at birth");
    }
    let mut classic = Game::new();
    classic.start_campaign(0xC0AA_100_u64, Difficulty::Classic);
    classic.spawn_campaign_body(&encounter, packet, 0, 0, true);
    assert!(classic.creeps.last().unwrap().max_hp < campaign_commander_hp_floor(Difficulty::Veteran, 10));
}

#[test]
fn campaign_ordinary_action_script_veteran_challenge() {
    let run_simulation = |label: &str, target: u32, freeze_after: Option<u32>, init_fn: &dyn Fn(&mut Game)| -> (Phase, u32, i64, usize, f32, u32, f32, bool) {
        let mut g = Game::new();
        g.start_campaign(101, Difficulty::Veteran);
        g.speed = 4.0;
        init_fn(&mut g);
        g.send_wave();
        let mut ticks = 0;
        let mut e121_roster_logged = false;
        let mut weighted_peak_pressure = 0.0_f32;
        let mut chapter_peak_pressure = [0.0_f32; 10];
        let mut commander_seconds = Vec::new();
        // This is ordinary player automation, not the instant-kill diagnostic:
        // it must survive the first commander (E10) and first air formation
        // (E21) with legal purchases and reinvested encounter income.
        while !matches!(g.phase, Phase::Defeat | Phase::Victory) && ticks < 180_000 && g.wave < target {
            if g.pending_doctrine {
                let pick = Doctrine::ALL
                    .into_iter()
                    .find(|&d| g.doctrine_rank(d) < 3)
                    .unwrap_or(Doctrine::Arsenal);
                g.choose_doctrine(pick);
            }
            if g.phase == Phase::Build {
                let investing = freeze_after.is_none_or(|checkpoint| g.wave < checkpoint);
                // The long trace buys from earned gold only. Rotate coverage
                // so later armoured/air encounters cannot be passed by an
                // opening board that merely compounds old tiers forever.
                if investing && g.gold >= 240 {
                    let family = match g.wave % 4 {
                        0 => Family::Siege,
                        1 => Family::Single,
                        2 => Family::Air,
                        _ => Family::Slow,
                    };
                    let def = def_by_family(family);
                    if let Some(pos) = find_in_range_legal_spot(&g, def) {
                        g.build_choice = Some((def, 1));
                        let _ = g.try_build_at(pos);
                    }
                }
                if investing {
                    for ti in 0..g.towers.len() {
                        if g.towers[ti].has_choice() {
                            let poison_def = def_by_family(Family::Poison);
                            g.upgrade_into(ti, poison_def);
                        } else {
                            g.upgrade(ti);
                        }
                    }
                }
                g.send_wave();
            }
            // A commander can be born and die inside one simulation update;
            // preserve the old encounter state so its physical lifetime is
            // still measurable when the encounter cursor advances.
            let before = g.campaign.clone();
            g.update(0.1);
            let weighted_pressure = g.campaign_pressure();
            weighted_peak_pressure = weighted_peak_pressure.max(weighted_pressure);
            if let Some(chapter) = g.campaign_chapter() {
                let slot = chapter.saturating_sub(1) as usize;
                if let Some(peak) = chapter_peak_pressure.get_mut(slot) {
                    *peak = peak.max(weighted_pressure);
                }
            }
            if let Some(previous) = before {
                let previous_encounter = previous.encounter;
                if previous.current().commander.is_some()
                    && g.campaign.as_ref().is_none_or(|state| state.encounter != previous_encounter)
                    && let Some(spawned) = previous.commander_spawned_at
                {
                    commander_seconds.push((previous_encounter, (previous.elapsed_seconds - spawned).max(0.0)));
                }
            }
            if !e121_roster_logged && g.wave >= 121 {
                e121_roster_logged = true;
                let mut roster: std::collections::BTreeMap<String, (usize, f32)> = std::collections::BTreeMap::new();
                for tower in &g.towers {
                    let entry = roster
                        .entry(format!("{} L{}", tower.full_name(), tower.level()))
                        .or_insert((0, 0.0));
                    entry.0 += 1;
                    entry.1 += tower.dmg() * tower.rate();
                }
                let total_dps: f32 = roster.values().map(|(_, dps)| dps).sum();
                eprintln!(
                    "{label} E121 roster: {} towers, {:.0} raw direct DPS, {}",
                    g.towers.len(), total_dps,
                    roster.iter().map(|(name, (count, dps))| format!("{count}x {name}={dps:.0}")).collect::<Vec<_>>().join("; ")
                );
            }
            ticks += 1;
        }
        let final_dps: f32 = g.towers.iter().map(|tower| tower.dmg() * tower.rate()).sum();
        let mut final_roster: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for tower in &g.towers {
            *final_roster.entry(format!("{} L{}", tower.full_name(), tower.level())).or_default() += 1;
        }
        eprintln!(
            "{label} final roster: {} towers, {final_dps:.0} raw direct DPS, {}",
            g.towers.len(),
            final_roster.iter().map(|(name, count)| format!("{count}x {name}")).collect::<Vec<_>>().join("; ")
        );
        eprintln!(
            "{label} pressure: weighted peak {weighted_peak_pressure:.1}; per-chapter weighted peaks {:?}; commander live seconds {:?}",
            chapter_peak_pressure, commander_seconds
        );
        (g.phase, g.wave, g.gold, g.towers.len(), g.campaign_pressure(), g.stats.peak_circling,
            weighted_peak_pressure, g.campaign.as_ref().is_some_and(|state| state.complete))
    };

    let siege_def = def_by_family(Family::Siege);
    let single_def = def_by_family(Family::Single);
    let poison_def = def_by_family(Family::Poison);

    // Baseline failures:
    // 1. Unattended board on Veteran fails in early encounters (wave <= 2)
    let (phase_unattended, wave_unattended) = {
        let mut g = Game::new();
        g.start_campaign(101, Difficulty::Veteran);
        g.speed = 4.0;
        g.update(CAMPAIGN_AUTOSTART_SECONDS + 0.1);
        let mut ticks = 0;
        while g.phase != Phase::Defeat && ticks < 15_000 {
            g.update(0.1);
            ticks += 1;
        }
        (g.phase, g.wave)
    };
    assert_eq!(phase_unattended, Phase::Defeat, "Unattended Veteran board must fail");
    assert!(wave_unattended <= 2, "Unattended Veteran board must fail at encounter 1 or 2");

    // 2. Shallow un-upgraded single spam fails early
    let (phase_spam, wave_spam) = {
        let mut g = Game::new();
        g.start_campaign(101, Difficulty::Veteran);
        g.speed = 4.0;
        for _ in 0..6 {
            if let Some(pos) = find_in_range_legal_spot(&g, single_def) {
                g.build_choice = Some((single_def, 1));
                let _ = g.try_build_at(pos);
            }
        }
        g.send_wave();
        let mut ticks = 0;
        while g.phase != Phase::Defeat && ticks < 20_000 && g.wave < 10 {
            if g.pending_doctrine {
                g.choose_doctrine(Doctrine::Arsenal);
            }
            if g.phase == Phase::Build {
                g.send_wave();
            }
            g.update(0.1);
            ticks += 1;
        }
        (g.phase, g.wave)
    };
    assert_eq!(phase_spam, Phase::Defeat, "Shallow spam must fail on Veteran");
    assert!(wave_spam <= 3, "Shallow un-upgraded spam must fail by encounter 3");

    // Strategy 1: AoE Siege splash focus
    let (phase_strat1, wave_strat1, ..) = run_simulation("siege", 22, None, &|g| {
        let p1 = find_in_range_legal_spot(g, siege_def).expect("siege spot 1");
        g.build_choice = Some((siege_def, 1));
        assert!(g.try_build_at(p1));
        g.upgrade(0);

        let p2 = find_in_range_legal_spot(g, siege_def).expect("siege spot 2");
        g.build_choice = Some((siege_def, 1));
        assert!(g.try_build_at(p2));
        g.upgrade(1);
    });
    assert_ne!(phase_strat1, Phase::Defeat, "AoE Siege strategy survives through deep encounters");
    assert!(wave_strat1 >= 22, "AoE Siege strategy must clear boss E10 and air E21 (reached {})", wave_strat1);

    // Strategy 2: Single-target & Poison branching DPS control
    let (phase_strat2, wave_strat2, ..) = run_simulation("single-poison", 22, None, &|g| {
        for _ in 0..3 {
            if let Some(pos) = find_in_range_legal_spot(g, single_def) {
                g.build_choice = Some((single_def, 1));
                let _ = g.try_build_at(pos);
            }
        }
        for ti in 0..g.towers.len() {
            g.upgrade_into(ti, poison_def);
            g.upgrade(ti);
        }
    });
    assert_ne!(phase_strat2, Phase::Defeat, "Single-target/Poison strategy survives through deep encounters");
    assert!(wave_strat2 >= 22, "Single-target/Poison strategy must clear boss E10 and air E21 (reached {})", wave_strat2);

    // Strategy 3: Mixed synergy (AoE Siege + Single DPS + Upgrades)
    let (phase_strat3, wave_strat3, ..) = run_simulation("mixed", 22, None, &|g| {
        let p1 = find_in_range_legal_spot(g, siege_def).expect("siege spot");
        g.build_choice = Some((siege_def, 1));
        assert!(g.try_build_at(p1));
        g.upgrade(0);

        let p2 = find_in_range_legal_spot(g, single_def).expect("single spot");
        g.build_choice = Some((single_def, 1));
        assert!(g.try_build_at(p2));
        g.upgrade_into(1, poison_def);

        if let Some(p3) = find_in_range_legal_spot(g, siege_def) {
            g.build_choice = Some((siege_def, 1));
            let _ = g.try_build_at(p3);
        }
    });
    assert_ne!(phase_strat3, Phase::Defeat, "Mixed synergy strategy survives through deep encounters");
    assert!(wave_strat3 >= 22, "Mixed synergy strategy must clear boss E10 and air E21 (reached {})", wave_strat3);

    // A longer, ordinary-action reinvestment trace spans the full campaign. It
    // does not use the instant-kill fixture, grant gold, reset towers, or
    // clear enemies; each build beat spends only its accumulated economy.
    let (phase_long, wave_long, spare_gold, defenses, pressure, raw_peak_count, weighted_peak_pressure, complete) = run_simulation("active", 601, None, &|g| {
        for family in [Family::Siege, Family::Single, Family::Air, Family::Slow] {
            let def = def_by_family(family);
            if let Some(pos) = find_in_range_legal_spot(g, def) {
                g.build_choice = Some((def, 1));
                let _ = g.try_build_at(pos);
            }
        }
    });
    eprintln!("ordinary reinvestment terminal: E{wave_long}, {spare_gold}g spare, {defenses} towers, {pressure:.1} current weighted pressure, raw peak count {raw_peak_count}, weighted peak pressure {weighted_peak_pressure:.1}");
    assert_eq!(phase_long, Phase::Victory, "ordinary reinvestment must resolve the final commander");
    assert!(complete, "Victory must carry CampaignState.complete, not a timed-out living boss");
    assert!(wave_long >= 600, "long reinvestment trace ended at E{wave_long}; gold {spare_gold}, towers {defenses}, pressure {pressure:.1}");

    let (phase_frozen, wave_frozen, frozen_gold, frozen_towers, frozen_pressure, frozen_raw_peak_count, frozen_weighted_peak_pressure, frozen_complete) = run_simulation("frozen-after-E121", 601, Some(121), &|g| {
        for family in [Family::Siege, Family::Single, Family::Air, Family::Slow] {
            let def = def_by_family(family);
            if let Some(pos) = find_in_range_legal_spot(g, def) {
                g.build_choice = Some((def, 1));
                let _ = g.try_build_at(pos);
            }
        }
    });
    eprintln!("frozen-after-E121 terminal: {phase_frozen:?} E{wave_frozen}, {frozen_gold}g spare, {frozen_towers} towers, {frozen_pressure:.1} current weighted pressure, raw peak count {frozen_raw_peak_count}, weighted peak pressure {frozen_weighted_peak_pressure:.1}, complete {frozen_complete}");
    assert_eq!(
        phase_frozen,
        Phase::Defeat,
        "a board frozen after E121 must actually lose; an incomplete timeout is not balance evidence"
    );
}

/// Bounded early-campaign evidence, intentionally separate from the long
/// regression trace above. Run with:
/// `cargo test terra_challenge100_first60_baseline -- --ignored --nocapture`
///
/// This is a deterministic ordinary-play probe, not an optimal-play claim:
/// every purchase uses the live campaign purse, every placement goes through
/// `try_build_at`, and combat advances through production cooldown/projectile
/// updates. It exists to give tuning a small, auditable first-60 baseline.
#[derive(Clone, Copy)]
enum EarlyPolicy {
    OrdinaryMixed,
    EntranceAwareMixed,
    EntranceAwareCorruption,
    EntranceAwareFrozenAfterE10,
    CheapPoisonFrozenAfterE10,
    EntranceAwareMissesFirstAir,
}

impl EarlyPolicy {
    fn label(self) -> &'static str {
        match self {
            Self::OrdinaryMixed => "ordinary-script-adapted mixed",
            Self::EntranceAwareMixed => "entrance-aware economical mixed",
            Self::EntranceAwareCorruption => "entrance-aware mixed with saved Corruption",
            Self::EntranceAwareFrozenAfterE10 => "entrance-aware mixed, frozen after E10",
            Self::CheapPoisonFrozenAfterE10 => "cheap-Poison mixed baseline, frozen after E10",
            Self::EntranceAwareMissesFirstAir => "entrance-aware mixed, ignores first air warning",
        }
    }

    fn may_invest(self, encounter: u16) -> bool {
        !matches!(self, Self::EntranceAwareFrozenAfterE10 | Self::CheapPoisonFrozenAfterE10) || encounter <= 10
    }

    fn ignores_air(self, encounter: u16) -> bool {
        matches!(self, Self::EntranceAwareMissesFirstAir) && encounter <= 21
    }
}

fn early_has_family(g: &Game, family: Family) -> bool {
    g.towers.iter().any(|tower| tower.family() == family)
}

fn early_buy(g: &mut Game, family: Family, entrance_aware: bool) -> bool {
    let def = def_by_family(family);
    let pos = if entrance_aware {
        find_entrance_aware_legal_spot(g, def, g.towers.is_empty())
    } else {
        find_best_coverage_legal_spot(g, def)
    };
    let Some(pos) = pos else { return false };
    g.build_choice = Some((def, 1));
    let bought = g.try_build_at(pos);
    g.build_choice = None;
    bought
}

fn early_has_coming_trait(g: &Game, trait_: campaign::ResolvedTrait) -> bool {
    let Some(state) = g.campaign.as_ref() else { return false };
    let start = state.encounter;
    (start..=(start + 3).min(60)).any(|encounter| {
        campaign::resolved_encounter(encounter)
            .packets
            .iter()
            .any(|packet| packet.traits.contains(&Some(trait_)))
    })
}

fn early_upgrade_one(g: &mut Game) -> bool {
    let Some((tower, into, _)) = (0..g.towers.len())
        .filter(|&tower| !g.towers[tower].has_choice())
        .flat_map(|tower| g.upgrade_choices(tower).into_iter().map(move |(into, cost)| (tower, into, cost)))
        .min_by_key(|(_, _, cost)| *cost)
    else { return false };
    let before = g.stats.gold_spent;
    g.upgrade_into(tower, into);
    g.stats.gold_spent != before
}

fn early_upgrade_one_including_choice(g: &mut Game) -> bool {
    let Some((tower, into, _)) = cheapest_upgrade(g) else { return false };
    let before = g.stats.gold_spent;
    g.upgrade_into(tower, into);
    g.stats.gold_spent != before
}

fn early_invest(g: &mut Game, policy: EarlyPolicy) {
    let encounter = g.campaign.as_ref().map_or(1, |state| state.encounter);
    if !policy.may_invest(encounter) {
        return;
    }
    let entrance_aware = !matches!(policy, EarlyPolicy::OrdinaryMixed);
    // The ordinary-script variant begins with the existing test's mixed
    // roles. The other variants deliberately put their first Siege/Single at
    // the two-direction entrance, then reserve for warned counters.
    let wanted_air = early_has_coming_trait(g, campaign::ResolvedTrait::Flying)
        && !policy.ignores_air(encounter);
    let wanted_focus = early_has_coming_trait(g, campaign::ResolvedTrait::Armoured)
        || early_has_coming_trait(g, campaign::ResolvedTrait::Shielded);
    let next = if !early_has_family(g, Family::Siege) {
        Some(Family::Siege)
    } else if !early_has_family(g, Family::Single) {
        Some(Family::Single)
    } else if wanted_air && !early_has_family(g, Family::Air) {
        Some(Family::Air)
    } else if !early_has_family(g, Family::Slow) {
        Some(Family::Slow)
    } else {
        None
    };
    if let Some(family) = next {
        if early_buy(g, family, entrance_aware) {
            return;
        }
    }
    // The focused alternative banks through the opening. Its one legal
    // Corruption purchase is intentionally made from live combat below, when
    // earned gold reaches the threshold during the first commander.
    if matches!(policy, EarlyPolicy::EntranceAwareCorruption)
        && encounter >= 6 && !early_has_family(g, Family::Corruption)
    {
        return;
    }
    // The Single seed is the deliberately retained focused-damage answer to
    // armour. Before the first air warning, reserve rather than consuming the
    // Air purchase in a greedy Siege upgrade; the miss-warning variant makes
    // the opposite, explicitly labelled choice.
    if wanted_air && !early_has_family(g, Family::Air) {
        let _ = early_buy(g, Family::Air, entrance_aware);
        return;
    }
    let _ = wanted_focus; // Single is already present before an armour warning.
    // A deliberately conservative reinvestment beat: one real upgrade only,
    // never fixture money or an all-at-once roster.
    if matches!(policy, EarlyPolicy::CheapPoisonFrozenAfterE10) {
        let _ = early_upgrade_one_including_choice(g);
    } else {
        let _ = early_upgrade_one(g);
    }
}

#[test]
#[ignore = "prints bounded first-60 Campaign challenge evidence"]
fn terra_challenge100_first60_baseline() {
    #[derive(Default)]
    struct Interval {
        sampled_peak_pressure: f32,
        seconds_over_70: f32,
        min_grace: f32,
        final_deployment_survivors: Vec<String>,
        purchases: u32,
        upgrades: u32,
        spent_start: u64,
    }

    fn run(policy: EarlyPolicy, difficulty: Difficulty) {
        const SEED: u64 = 0xC100_0060;
        let mut g = Game::new();
        g.start_campaign(SEED, difficulty);
        // Compare simulated rather than wall-clock time. `update` applies
        // speed internally; active_seconds is the production campaign clock.
        g.speed = 2.0;
        let mut interval = Interval { min_grace: campaign::BREACH_SECONDS, spent_start: g.stats.gold_spent, ..Interval::default() };
        let mut interval_start = 1_u16;
        let mut commander_rows: Vec<String> = Vec::new();
        let mut commander_recorded = std::collections::BTreeSet::new();
        let mut deployment_recorded = std::collections::BTreeSet::new();
        let mut focused_purchase: Option<(u16, i64)> = None;
        let mut ticks = 0_u32;

        while !matches!(g.phase, Phase::Defeat | Phase::Victory)
            && g.campaign.as_ref().is_some_and(|state| state.encounter <= 60)
            && ticks < 160_000
        {
            if g.pending_doctrine {
                let doctrine = Doctrine::ALL.into_iter()
                    .find(|&choice| g.doctrine_rank(choice) < 3)
                    .unwrap_or(Doctrine::Arsenal);
                g.choose_doctrine(doctrine);
            }
            if g.phase == Phase::Build {
                let spent = g.stats.gold_spent;
                let towers = g.towers.len();
                early_invest(&mut g, policy);
                if g.stats.gold_spent != spent {
                    if g.towers.len() > towers { interval.purchases += 1; }
                    else { interval.upgrades += 1; }
                }
                g.send_wave();
            }

            // A player may build during combat. The focused plan banks its
            // ordinary income and takes exactly one direct Corruption purchase
            // during E10 or later once it can legally pay 500g.
            if matches!(policy, EarlyPolicy::EntranceAwareCorruption)
                && focused_purchase.is_none()
                && g.phase == Phase::Combat
                && g.campaign.as_ref().is_some_and(|state| state.encounter >= 10)
                && g.gold >= 500
            {
                let encounter = g.campaign.as_ref().unwrap().encounter;
                let before_gold = g.gold;
                if early_buy(&mut g, Family::Corruption, true) {
                    focused_purchase = Some((encounter, before_gold - g.gold));
                }
            }

            let before = g.campaign.clone();
            let before_active = before.as_ref().map_or(0.0, |state| state.active_seconds);
            g.update(0.1);
            let simulated_dt = g.campaign.as_ref()
                .map_or(0.0, |state| state.active_seconds - before_active) as f32;
            let pressure = g.campaign_pressure();
            interval.sampled_peak_pressure = interval.sampled_peak_pressure.max(pressure);
            let capacity = g.flood_limit() as f32;
            if pressure > capacity * 0.70 { interval.seconds_over_70 += simulated_dt; }
            interval.min_grace = interval.min_grace.min(g.campaign_pressure_grace);

            if let Some(state) = g.campaign.as_ref() {
                if state.elapsed_seconds >= state.current().duration_seconds as f32
                    && deployment_recorded.insert(state.encounter)
                {
                    let survivors = g.creeps.iter()
                        .filter(|creep| creep.campaign_encounter == state.encounter).count();
                    interval.final_deployment_survivors.push(format!("E{}={survivors}", state.encounter));
                }
            }

            if let Some(old) = before {
                let changed = g.campaign.as_ref().is_none_or(|state| state.encounter != old.encounter);
                let same_commander = old.current().commander.is_some()
                    && g.campaign.as_ref().is_some_and(|state| state.encounter == old.encounter);
                let commander_alive = g.creeps.iter().any(|creep|
                    creep.is_boss() && creep.campaign_encounter == old.encounter && creep.hp > 0.0);
                if old.current().commander.is_some()
                    && !commander_recorded.contains(&old.encounter)
                    && ((same_commander && old.commander_spawned_at.is_some() && !commander_alive) || changed)
                {
                    if let Some(spawned) = old.commander_spawned_at {
                        let death_at = if same_commander {
                            g.campaign.as_ref().map_or(old.elapsed_seconds, |state| state.elapsed_seconds)
                        } else {
                            old.elapsed_seconds
                        };
                        commander_rows.push(format!(
                            "E{} live {:.1}s, mechanics {}",
                            old.encounter,
                            (death_at - spawned).max(0.0),
                            old.commander_triggers.count_ones(),
                        ));
                    } else {
                        commander_rows.push(format!("E{} no physical commander spawn", old.encounter));
                    }
                    commander_recorded.insert(old.encounter);
                }
                if changed && (old.encounter % 10 == 0 || old.encounter == 60) {
                    let spent = g.stats.gold_spent - interval.spent_start;
                    eprintln!(
                        "{} / {} / E{}-{}: sampled-frame peak pressure {:.1}; >70% {:.1} simulated s at 2x; min breach grace {:.1}s; purchases {}; upgrades {}; spend {}g; final-deployment survivors [{}]; cash {}g; towers {}",
                        difficulty.label(), policy.label(), interval_start, old.encounter,
                        interval.sampled_peak_pressure, interval.seconds_over_70, interval.min_grace,
                        interval.purchases, interval.upgrades, spent, interval.final_deployment_survivors.join(", "),
                        g.gold, g.towers.len(),
                    );
                    interval_start = old.encounter + 1;
                    interval = Interval { min_grace: campaign::BREACH_SECONDS, spent_start: g.stats.gold_spent, ..Interval::default() };
                }
            }
            ticks += 1;
        }
        let roster = board_summary(&g);
        if matches!(policy, EarlyPolicy::EntranceAwareCorruption) {
            assert!(
                early_has_family(&g, Family::Corruption),
                "focused policy reached {:?} at E{} without its required legal direct Corruption purchase",
                g.phase, g.wave,
            );
            assert!(focused_purchase.is_some(), "focused policy owned Corruption without the recorded live purchase");
        }
        eprintln!(
            "{} / {} terminal: {:?} at E{} after {} ticks; cash {}g, spent {}g, focused live purchase {:?}, roster [{}]; commanders [{}]",
            difficulty.label(), policy.label(), g.phase, g.wave, ticks, g.gold, g.stats.gold_spent,
            focused_purchase, roster, commander_rows.join("; "),
        );
    }

    eprintln!("TERRA_CHALLENGE100 baseline: fixed seed 0xC1000060; legal purse/placement; live production combat; first 60 only.");
    for policy in [
        EarlyPolicy::OrdinaryMixed,
        EarlyPolicy::EntranceAwareMixed,
        EarlyPolicy::EntranceAwareCorruption,
        EarlyPolicy::EntranceAwareFrozenAfterE10,
        EarlyPolicy::CheapPoisonFrozenAfterE10,
        EarlyPolicy::EntranceAwareMissesFirstAir,
    ] {
        for difficulty in [Difficulty::Veteran, Difficulty::Nightmare] {
            run(policy, difficulty);
        }
    }
}

/// Legal Veteran probes for the historical "just buy Multi" answer.  These
/// deliberately spend only the campaign purse: no fixture gold, damage, or
/// board reset is involved.  Keep the breadth and King-to-SuperMulti policies
/// separate so a result cannot be dismissed as one poor upgrade script.
/// Comprehensive proof of campaign counterplay:
/// 1. Equal legal budgets produce differentiated combat against Swarm, Armoured, and Flying.
/// 2. Shield break semantics: active shields absorb pellets (0.65x); once shield <= 0, penalty ends.
/// 3. In-flight captured projectile resolves by launching tower definition even after sale/upgrade.
/// 4. Corruption on-hit applies 2s suppression (halting regen) and ignores armour per-hit without mutating base creep armour.
#[test]
fn campaign_counterplay_combat_effects_at_equal_budget() {
    let multi_def = def_by_family(Family::Multi);
    let siege_def = def_by_family(Family::Siege);
    let corrupt_def = def_by_family(Family::Corruption);

    // 1. Equal 500g Budget Combat Proof: Multi 1 (400g) + Siege 1 (100g) = 500g vs Corruption 1 (500g)
    let cost_roster_a = TOWERS[multi_def].gold + TOWERS[siege_def].gold;
    let cost_roster_b = TOWERS[corrupt_def].gold;
    assert_eq!(cost_roster_a, 500, "Roster A (Multi 400g + Siege 100g) must cost exactly 500g");
    assert_eq!(cost_roster_b, 500, "Roster B (Corruption 500g) must cost exactly 500g");
    assert_eq!(cost_roster_a, cost_roster_b, "Both rosters must have equal 500g budget");

    // Micro-simulation runner for isolated combat without ambient campaign packet dispatch:
    let step_isolated_combat = |g: &mut Game, seconds: f32| {
        let dt = 1.0 / 60.0;
        let steps = (seconds / dt) as usize;
        for _ in 0..steps {
            g.time += dt;
            g.spatial.rebuild(&g.creeps);
            g.step_creeps(dt);
            combat::step_towers(g, dt);
            combat::step_projectiles(g, dt);
        }
    };

    // Threat Group 1: Many light unarmoured swarm bodies (12 Gnolls, 80 HP, 0 armour, Unarmoured)
    let swarm_def = creep(80.0, 0, ArmourType::Unarmoured, false);

    // Roster A (Multi + Siege) vs Swarm Pack:
    let mut g_swarm_a = Game::new();
    g_swarm_a.start_campaign(42, Difficulty::Classic);
    assert_eq!(g_swarm_a.gold, 600, "Classic starting gold is 600g");
    isolate(&mut g_swarm_a);
    let p_build = 4.0;
    build(&mut g_swarm_a, Family::Multi, p_build);
    build(&mut g_swarm_a, Family::Siege, p_build);
    assert_eq!(600 - g_swarm_a.gold, 500, "Roster A spends exactly 500g");
    assert_eq!(g_swarm_a.towers.len(), 2);

    let at = g_swarm_a.towers[0].pos;
    let mut best_track = 0.0f32;
    let mut best_dist = f32::MAX;
    let mut d = 0.0;
    while d < g_swarm_a.board.total {
        let p = g_swarm_a.board.sample(d);
        let dd = (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2);
        if dd < best_dist {
            best_dist = dd;
            best_track = d;
        }
        d += 0.25;
    }

    let start_uid_swarm_a = g_swarm_a.next_uid;
    for i in 0..12 {
        let track_d = best_track - 1.5 + (i as f32) * 0.25;
        g_swarm_a.spawn_creep(&swarm_def, swarm_def.hp, 1.0, track_d);
    }
    let end_uid_swarm_a = g_swarm_a.next_uid;
    assert_eq!(end_uid_swarm_a - start_uid_swarm_a, 12);
    for (i, c) in g_swarm_a.creeps.iter_mut().enumerate() {
        c.dist = best_track - 1.5 + (i as f32) * 0.25;
        c.route_dir = 1.0;
        c.lane = 0.0;
        c.base_speed = 1.0;
        place(&g_swarm_a.board, c);
    }

    // Roster B (Corruption) vs Swarm Pack:
    let mut g_swarm_b = Game::new();
    g_swarm_b.start_campaign(42, Difficulty::Classic);
    assert_eq!(g_swarm_b.gold, 600, "Classic starting gold is 600g");
    isolate(&mut g_swarm_b);
    build(&mut g_swarm_b, Family::Corruption, p_build);
    assert_eq!(600 - g_swarm_b.gold, 500, "Roster B spends exactly 500g");
    assert_eq!(g_swarm_b.towers.len(), 1);
    assert_eq!(g_swarm_b.towers[0].pos, g_swarm_a.towers[0].pos, "Both primary towers occupy identical pad");

    let start_uid_swarm_b = g_swarm_b.next_uid;
    for i in 0..12 {
        let track_d = best_track - 1.5 + (i as f32) * 0.25;
        g_swarm_b.spawn_creep(&swarm_def, swarm_def.hp, 1.0, track_d);
    }
    let end_uid_swarm_b = g_swarm_b.next_uid;
    assert_eq!(end_uid_swarm_b - start_uid_swarm_b, 12);
    for (i, c) in g_swarm_b.creeps.iter_mut().enumerate() {
        c.dist = best_track - 1.5 + (i as f32) * 0.25;
        c.route_dir = 1.0;
        c.lane = 0.0;
        c.base_speed = 1.0;
        place(&g_swarm_b.board, c);
    }

    // Step 4.0s of isolated combat at equal budget:
    step_isolated_combat(&mut g_swarm_a, 4.0);
    step_isolated_combat(&mut g_swarm_b, 4.0);

    assert!(g_swarm_a.creeps.iter().all(|c| (start_uid_swarm_a..end_uid_swarm_a).contains(&c.uid)));
    assert!(g_swarm_b.creeps.iter().all(|c| (start_uid_swarm_b..end_uid_swarm_b).contains(&c.uid)));
    assert!(g_swarm_a.creeps.len() <= 12);
    assert!(g_swarm_b.creeps.len() <= 12);

    let kills_swarm_a = 12 - g_swarm_a.creeps.len();
    let kills_swarm_b = 12 - g_swarm_b.creeps.len();
    let rem_hp_swarm_a: f32 = g_swarm_a.creeps.iter().map(|c| c.hp.max(0.0)).sum();
    let rem_hp_swarm_b: f32 = g_swarm_b.creeps.iter().map(|c| c.hp.max(0.0)).sum();

    eprintln!(
        "EQUAL-BUDGET SWARM COMBAT: Multi+Siege kills {kills_swarm_a}/12, rem HP {rem_hp_swarm_a:.1} | Corruption kills {kills_swarm_b}/12, rem HP {rem_hp_swarm_b:.1}"
    );

    // Multi (3 simultaneous targets) + Siege (splash) shreds swarm pack faster than single-target Corruption:
    assert!(
        kills_swarm_a > kills_swarm_b || rem_hp_swarm_a < rem_hp_swarm_b,
        "Multi+Siege multishot and splash clears swarms better than single-target Corruption: kills {kills_swarm_a} vs {kills_swarm_b}, remaining HP {rem_hp_swarm_a:.1} vs {rem_hp_swarm_b:.1}"
    );

    // Threat Group 2: Fewer tougher armoured bodies (3 Brutes, 2000 HP, 20 armour, Medium)
    let heavy_def = creep(2000.0, 20, ArmourType::Medium, false);

    let mut g_heavy_a = Game::new();
    g_heavy_a.start_campaign(42, Difficulty::Classic);
    isolate(&mut g_heavy_a);
    build(&mut g_heavy_a, Family::Multi, p_build);
    build(&mut g_heavy_a, Family::Siege, p_build);
    assert_eq!(600 - g_heavy_a.gold, 500);

    let start_uid_heavy_a = g_heavy_a.next_uid;
    for i in 0..3 {
        let track_d = best_track - 1.0 + (i as f32) * 0.8;
        g_heavy_a.spawn_creep(&heavy_def, heavy_def.hp, 1.0, track_d);
    }
    let end_uid_heavy_a = g_heavy_a.next_uid;
    assert_eq!(end_uid_heavy_a - start_uid_heavy_a, 3);
    for (i, c) in g_heavy_a.creeps.iter_mut().enumerate() {
        c.dist = best_track - 1.0 + (i as f32) * 0.8;
        c.route_dir = 1.0;
        c.lane = 0.0;
        c.base_speed = 1.0;
        place(&g_heavy_a.board, c);
    }

    let mut g_heavy_b = Game::new();
    g_heavy_b.start_campaign(42, Difficulty::Classic);
    isolate(&mut g_heavy_b);
    build(&mut g_heavy_b, Family::Corruption, p_build);
    assert_eq!(600 - g_heavy_b.gold, 500);

    let start_uid_heavy_b = g_heavy_b.next_uid;
    for i in 0..3 {
        let track_d = best_track - 1.0 + (i as f32) * 0.8;
        g_heavy_b.spawn_creep(&heavy_def, heavy_def.hp, 1.0, track_d);
    }
    let end_uid_heavy_b = g_heavy_b.next_uid;
    assert_eq!(end_uid_heavy_b - start_uid_heavy_b, 3);
    for (i, c) in g_heavy_b.creeps.iter_mut().enumerate() {
        c.dist = best_track - 1.0 + (i as f32) * 0.8;
        c.route_dir = 1.0;
        c.lane = 0.0;
        c.base_speed = 1.0;
        place(&g_heavy_b.board, c);
    }

    // Step 4.0s of isolated combat against Plated Heavy:
    step_isolated_combat(&mut g_heavy_a, 4.0);
    step_isolated_combat(&mut g_heavy_b, 4.0);

    assert!(g_heavy_a.creeps.iter().all(|c| (start_uid_heavy_a..end_uid_heavy_a).contains(&c.uid)));
    assert!(g_heavy_b.creeps.iter().all(|c| (start_uid_heavy_b..end_uid_heavy_b).contains(&c.uid)));
    assert!(g_heavy_a.creeps.len() <= 3);
    assert!(g_heavy_b.creeps.len() <= 3);

    let kills_heavy_a = 3 - g_heavy_a.creeps.len();
    let kills_heavy_b = 3 - g_heavy_b.creeps.len();
    let rem_hp_heavy_a: f32 = g_heavy_a.creeps.iter().map(|c| c.hp.max(0.0)).sum();
    let rem_hp_heavy_b: f32 = g_heavy_b.creeps.iter().map(|c| c.hp.max(0.0)).sum();

    eprintln!(
        "EQUAL-BUDGET PLATED COMBAT: Multi+Siege kills {kills_heavy_a}/3, rem HP {rem_hp_heavy_a:.1} | Corruption kills {kills_heavy_b}/3, rem HP {rem_hp_heavy_b:.1}"
    );

    // Corruption pierces 15 armour with 15 armour pen, decimating plated heavies,
    // whereas Multi (0.60x) and Siege (0.50x) struggle against heavy plated targets:
    assert!(
        kills_heavy_b > kills_heavy_a || rem_hp_heavy_b < rem_hp_heavy_a,
        "Corruption with armour-pen shreds Plated Heavy far better than Multi+Siege: kills {kills_heavy_b} vs {kills_heavy_a}, remaining HP {rem_hp_heavy_b:.1} vs {rem_hp_heavy_a:.1}"
    );

    // 2. Late E600 Swarm Hit: Swarms retain 1.15x swarm multiplier even with late-game numeric armour
    let mut g_late = Game::new();
    g_late.start_campaign(42, Difficulty::Classic);
    g_late.creeps.push(Creep {
        uid: 100, dist: 5.0, route_dir: 1.0, lane: 0.0, pos: [10.0, 10.0],
        facing: 0.0, hp: 500.0, max_hp: 500.0, base_speed: 1.0, armour: 25,
        armour_type: ArmourType::Unarmoured, model: Model::Gnoll, flying: false,
        radius: 0.5, bounty: 1, boss: false, elite: false,
        slow: Timed::default(), burn: Timed::default(), poison: Timed::default(),
        shred: Timed::default(), stun: 0.0, stun_dr: 0.0, kb_cd: 0.0,
        suppress: 0.0, stun_immune: 0.0, push_left: PUSHBACK_BUDGET, laps: 0,
        flash: 0.0, bob: 0.0, shield: 0.0, max_shield: 0.0,
        regen_per_second: 0.0, resistant: false, campaign_encounter: 600,
        pressure: 0.20, death_killer: None,
    });
    let late_mult = combat::campaign_core_role_multiplier(&g_late.creeps[0], Family::Multi);
    assert!((late_mult - 1.15).abs() < 0.001, "Late E600 swarm with 25 armour must retain 1.15x swarm multiplier");
    combat::damage_creep_from_def(&mut g_late, 0, TOWERS[multi_def].damage, 0, multi_def, false);
    let expected_late_dmg = damage_taken(TOWERS[multi_def].damage * 1.15, Attack::Normal, 25, ArmourType::Unarmoured);
    let actual_late_dmg = 500.0 - g_late.creeps[0].hp;
    assert!(
        (actual_late_dmg - expected_late_dmg).abs() < 0.01,
        "Late swarm damage must apply 1.15x multiplier: actual {actual_late_dmg:.2} vs expected {expected_late_dmg:.2}"
    );

    // 3. Shield Break Overflow: breaking a shield does not penalize remaining damage to exposed health
    let mut g_shield = Game::new();
    g_shield.start_campaign(42, Difficulty::Classic);
    g_shield.creeps.push(Creep {
        uid: 200, dist: 5.0, route_dir: 1.0, lane: 0.0, pos: [10.0, 10.0],
        facing: 0.0, hp: 100.0, max_hp: 100.0, base_speed: 1.0, armour: 0,
        armour_type: ArmourType::Unarmoured, model: Model::Gnoll, flying: false,
        radius: 0.5, bounty: 1, boss: false, elite: false,
        slow: Timed::default(), burn: Timed::default(), poison: Timed::default(),
        shred: Timed::default(), stun: 0.0, stun_dr: 0.0, kb_cd: 0.0,
        suppress: 0.0, stun_immune: 0.0, push_left: PUSHBACK_BUDGET, laps: 0,
        flash: 0.0, bob: 0.0, shield: 10.0, max_shield: 10.0,
        regen_per_second: 0.0, resistant: false, campaign_encounter: 11,
        pressure: 0.20, death_killer: None,
    });
    // Multi fires a 50.0 base damage hit against 10.0 shield.
    // Shield takes 0.65x multiplier: pot_shield_dmg = 50 * 0.65 = 32.5 > 10.0 curr_shield.
    // Absorbed = 10.0. Frac = 10 / 32.5 = 0.30769. Rem base = 50 * (1 - 0.30769) = 34.615.
    // Overflow applies core role multiplier 1.15 (unarmoured swarm): 34.615 * 1.15 = 39.81 damage to HP.
    combat::damage_creep_from_def(&mut g_shield, 0, 50.0, 0, multi_def, false);
    assert_eq!(g_shield.creeps[0].shield, 0.0, "Shield must be completely shattered");
    let hp_damage = 100.0 - g_shield.creeps[0].hp;
    // With proper split, HP took ~39.81 damage. Under old bug with whole-hit 0.65x, it would have taken only 22.5 damage.
    assert!(
        hp_damage > 35.0,
        "Overflow damage must apply core unshielded multiplier to exposed health (took {hp_damage:.2} hp damage)"
    );

    // 4. Captured projectile definition after sale:
    let mut g_proj = Game::new();
    g_proj.start_campaign(42, Difficulty::Classic);
    let target_creep = creep(100.0, 0, ArmourType::Unarmoured, false);
    g_proj.spawn_creep(&target_creep, target_creep.hp, 1.0, 5.0);
    let p1 = find_in_range_legal_spot(&g_proj, multi_def).expect("legal spot for multi");
    g_proj.build_choice = Some((multi_def, 1));
    assert!(g_proj.try_build_at(p1));
    let target_uid = g_proj.creeps[0].uid;
    let target_pos = g_proj.creeps[0].pos;
    g_proj.projs.push(Proj {
        pos: target_pos,
        z: g_proj.creeps[0].height(),
        vel: [10.0, 0.0],
        kind: ProjKind::Dart,
        tower: 0,
        def: multi_def,
        dmg: 50.0,
        splash: 0.0,
        bounces: 0,
        crit: false,
        target_idx: 0,
        target_uid,
        life: 1.0,
        trail: 1.0,
    });
    g_proj.sell(0);
    assert_eq!(g_proj.towers.len(), 0, "Tower was sold");
    let prev_hp = g_proj.creeps[0].hp;
    g_proj.spatial.rebuild(&g_proj.creeps);
    combat::step_projectiles(&mut g_proj, 1.0 / 60.0);
    assert!(g_proj.creeps[0].hp < prev_hp, "Captured projectile must still resolve damage by captured def after sale");

    // 5. Corruption on-hit suppression:
    let mut g_corr = Game::new();
    g_corr.start_campaign(42, Difficulty::Classic);
    g_corr.creeps.push(Creep {
        uid: 10, dist: 5.0, route_dir: 1.0, lane: 0.0, pos: [10.0, 10.0],
        facing: 0.0, hp: 50.0, max_hp: 100.0, base_speed: 1.0, armour: 15,
        armour_type: ArmourType::Medium, model: Model::Troll, flying: false,
        radius: 0.5, bounty: 1, boss: false, elite: false,
        slow: Timed::default(), burn: Timed::default(), poison: Timed::default(),
        shred: Timed::default(), stun: 0.0, stun_dr: 0.0, kb_cd: 0.0,
        suppress: 0.0, stun_immune: 0.0, push_left: PUSHBACK_BUDGET, laps: 0,
        flash: 0.0, bob: 0.0, shield: 0.0, max_shield: 0.0,
        regen_per_second: 10.0, resistant: false, campaign_encounter: 15,
        pressure: 1.0, death_killer: None,
    });
    let cp = find_in_range_legal_spot(&g_corr, corrupt_def).expect("legal spot for corrupt");
    g_corr.build_choice = Some((corrupt_def, 1));
    assert!(g_corr.try_build_at(cp));
    combat::on_hit_riders(&mut g_corr, 0, 0);
    assert!(g_corr.creeps[0].suppress >= 2.0, "Corruption applies 2s healing suppression");
    assert_eq!(g_corr.creeps[0].armour, 15, "Corruption must NOT permanently strip base armour from creep");

    let before_step = g_corr.creeps[0].hp;
    g_corr.step_creeps(0.5);
    assert_eq!(g_corr.creeps[0].hp, before_step, "Suppression must prevent creep regeneration");
}

/// Legal Campaign strategy probes testing Multi-only opening and alternative mono policy.
#[derive(Clone, Debug)]
struct LegalPolicyResult {
    phase: Phase,
    wave: u32,
    gold: i64,
    gold_spent: u64,
    towers_len: usize,
    peak_pressure: f32,
    max_level: u32,
    complete: bool,
    first_tower_pos: [f32; 2],
    first_tower_dmg: f64,
    total_damage: f64,
    kills: u64,
    defeat_cause: String,
    roster_str: String,
}

/// Legal Campaign strategy probes testing Multi-only opening and alternative mono policy.
/// Spends only earned purse; verifies that neither Multi nor Siege forms a universal exploit.
fn run_legal_mono_policy(
    label: &str,
    diff: Difficulty,
    policy: &str,
    family: Family,
    max_ticks: usize,
) -> LegalPolicyResult {
    let mut g = Game::new();
    let seed = match family {
        Family::Multi => 0x4D55_4C54,
        Family::Corruption => 0x434F_5252,
        Family::Siege => 0x5349_4547,
        _ => 42,
    };
    g.start_campaign(seed, diff);
    g.speed = 4.0;
    let def = def_by_family(family);

    let use_entrance = policy == "banking_entrance";
    let find_spot = |g: &Game, def: usize, initial: bool| {
        if use_entrance {
            find_entrance_aware_legal_spot(g, def, initial)
        } else {
            find_best_coverage_legal_spot(g, def)
        }
    };

    // Initial build with earned starting purse (600g):
    if let Some(pos) = find_spot(&g, def, true) {
        if g.can_afford(TOWERS[def].gold) {
            g.build_choice = Some((def, 1));
            let _ = g.try_build_at(pos);
        }
    }
    g.send_wave();

    let mut ticks = 0usize;
    let mut peak_pressure = 0.0_f32;
    let mut first_tower_fired = false;
    while !matches!(g.phase, Phase::Victory | Phase::Defeat) && ticks < max_ticks && g.wave < 601 {
        if !first_tower_fired && !g.towers.is_empty() && g.towers[0].damage > 0.0 {
            first_tower_fired = true;
        }
        if g.pending_doctrine {
            let pick = Doctrine::ALL
                .into_iter()
                .find(|&d| g.doctrine_rank(d) < 3)
                .unwrap_or(Doctrine::Arsenal);
            g.choose_doctrine(pick);
        }
        let is_build = g.phase == Phase::Build;
        let is_combat_beat = g.phase == Phase::Combat && ticks % 10 == 0;
        if is_build || is_combat_beat {
            if policy == "upgrade_first" {
                for ti in 0..g.towers.len() {
                    if g.towers[ti].family() == family {
                        g.upgrade(ti);
                    }
                }
                if g.can_afford(TOWERS[def].gold) {
                    if let Some(pos) = find_spot(&g, def, false) {
                        g.build_choice = Some((def, 1));
                        let _ = g.try_build_at(pos);
                    }
                }
            } else if policy == "breadth_first" {
                let mut built = false;
                if g.can_afford(TOWERS[def].gold) {
                    if let Some(pos) = find_spot(&g, def, false) {
                        g.build_choice = Some((def, 1));
                        if g.try_build_at(pos) {
                            built = true;
                        }
                    }
                }
                if !built {
                    for ti in 0..g.towers.len() {
                        if g.towers[ti].family() == family {
                            g.upgrade(ti);
                        }
                    }
                }
            } else if policy == "banking" || policy == "banking_entrance" {
                // Phase 1: Establish two Multi towers at Level 2:
                if g.towers.is_empty() {
                    if g.can_afford(TOWERS[def].gold) {
                        if let Some(pos) = find_spot(&g, def, true) {
                            g.build_choice = Some((def, 1));
                            let _ = g.try_build_at(pos);
                        }
                    }
                } else if g.towers.len() == 1 {
                    if g.towers[0].level() == 1 {
                        if let Some(&next_u) = TOWERS[g.towers[0].def].upgrades.first() {
                            if g.can_afford(TOWERS[next_u as usize].gold) {
                                g.upgrade(0);
                            }
                        }
                    } else if g.can_afford(TOWERS[def].gold) {
                        if let Some(pos) = find_spot(&g, def, true) {
                            g.build_choice = Some((def, 1));
                            let _ = g.try_build_at(pos);
                        }
                    }
                } else if g.towers.len() == 2 && g.towers[1].level() == 1 {
                    if let Some(&next_u) = TOWERS[g.towers[1].def].upgrades.first() {
                        if g.can_afford(TOWERS[next_u as usize].gold) {
                            g.upgrade(1);
                        }
                    }
                } else {
                    // Phase 2: Two Multi towers established at L2+.
                    // Reserve gold for next upgrade on the lead tower; do not buy fresh towers or cheaper upgrades:
                    let lead_idx = g.towers.iter().position(|t| !TOWERS[t.def].upgrades.is_empty());
                    if let Some(lead_ti) = lead_idx {
                        let next_u = TOWERS[g.towers[lead_ti].def].upgrades[0] as usize;
                        let upgrade_cost = TOWERS[next_u].gold;
                        if g.can_afford(upgrade_cost) {
                            g.upgrade(lead_ti);
                        }
                    } else {
                        // All existing towers fully maxed out: build and raise another tower
                        if g.can_afford(TOWERS[def].gold) {
                            if let Some(pos) = find_spot(&g, def, false) {
                                g.build_choice = Some((def, 1));
                                let _ = g.try_build_at(pos);
                            }
                        }
                    }
                }
            }
            if is_build {
                g.send_wave();
            }
        }
        g.update(0.1);
        peak_pressure = peak_pressure.max(g.campaign_pressure());
        ticks += 1;
    }

    if !first_tower_fired && !g.towers.is_empty() && g.towers[0].damage > 0.0 {
        first_tower_fired = true;
    }

    let mut roster_map: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for tower in &g.towers {
        *roster_map.entry(format!("{} L{}", tower.full_name(), tower.level())).or_default() += 1;
    }
    let roster_str = roster_map.iter().map(|(n, c)| format!("{c}x {n}")).collect::<Vec<_>>().join(", ");
    let max_level = g.towers.iter().map(|t| t.level()).max().unwrap_or(0);
    let complete = g.campaign.as_ref().is_some_and(|state| state.complete);
    let first_tower_pos = g.towers.first().map(|t| t.pos).unwrap_or([0.0, 0.0]);
    let first_tower_dmg = g.towers.first().map(|t| t.damage).unwrap_or(0.0);
    let total_damage = g.stats.damage;
    let kills = g.stats.kills;
    let defeat_cause = g.toast.as_ref().map(|(t, ..)| t.clone()).unwrap_or_else(|| {
        if g.phase == Phase::Defeat {
            "Breach: campaign pressure exceeded capacity".to_string()
        } else {
            "Victory achieved".to_string()
        }
    });

    eprintln!(
        "{label}: {:?} at E{}; gold: {}g spare, {}g spent; towers: {}; max level: L{}; peak pressure: {:.1}; first pos: [{:.2}, {:.2}] (dmg {:.0}, fired: {}); total dmg: {:.0}; kills: {}; complete: {}; cause: {}; roster: [{}]",
        g.phase, g.wave, g.gold, g.stats.gold_spent, g.towers.len(), max_level, peak_pressure,
        first_tower_pos[0], first_tower_pos[1], first_tower_dmg, first_tower_fired, total_damage, kills, complete, defeat_cause, roster_str
    );

    LegalPolicyResult {
        phase: g.phase,
        wave: g.wave,
        gold: g.gold,
        gold_spent: g.stats.gold_spent,
        towers_len: g.towers.len(),
        peak_pressure,
        max_level,
        complete,
        first_tower_pos,
        first_tower_dmg,
        total_damage,
        kills,
        defeat_cause,
        roster_str,
    }
}

/// Legal Campaign strategy probes testing Multi-only opening, banking policy, entrance-aware placement, and alternative mono policies.
/// Spends only earned purse; verifies that neither Multi nor Siege forms a universal exploit.
#[test]
fn campaign_multi_legal_baseline_probe() {
    let mut log_lines = Vec::new();
    log_lines.push("=== LEGAL CAMPAIGN STRATEGY BASELINE PROBES ===".to_string());

    // 1. Multi Upgrade-First on Classic
    let r1 = run_legal_mono_policy(
        "Multi Upgrade-First (Classic)",
        Difficulty::Classic,
        "upgrade_first",
        Family::Multi,
        180_000,
    );
    log_lines.push(format!("Multi Upgrade-First (Classic): {:?} E{}, {}g spare, {}g spent, {} towers, peak pressure {:.1}, roster: [{}]", r1.phase, r1.wave, r1.gold, r1.gold_spent, r1.towers_len, r1.peak_pressure, r1.roster_str));

    // 2. Multi Breadth-First on Classic
    let r2 = run_legal_mono_policy(
        "Multi Breadth-First (Classic)",
        Difficulty::Classic,
        "breadth_first",
        Family::Multi,
        180_000,
    );
    log_lines.push(format!("Multi Breadth-First (Classic): {:?} E{}, {}g spare, {}g spent, {} towers, peak pressure {:.1}, roster: [{}]", r2.phase, r2.wave, r2.gold, r2.gold_spent, r2.towers_len, r2.peak_pressure, r2.roster_str));

    // 3. Multi Upgrade-First on Veteran
    let r3 = run_legal_mono_policy(
        "Multi Upgrade-First (Veteran)",
        Difficulty::Veteran,
        "upgrade_first",
        Family::Multi,
        180_000,
    );
    log_lines.push(format!("Multi Upgrade-First (Veteran): {:?} E{}, {}g spare, {}g spent, {} towers, peak pressure {:.1}, roster: [{}]", r3.phase, r3.wave, r3.gold, r3.gold_spent, r3.towers_len, r3.peak_pressure, r3.roster_str));

    // 4. Multi Breadth-First on Veteran
    let r4 = run_legal_mono_policy(
        "Multi Breadth-First (Veteran)",
        Difficulty::Veteran,
        "breadth_first",
        Family::Multi,
        180_000,
    );
    log_lines.push(format!("Multi Breadth-First (Veteran): {:?} E{}, {}g spare, {}g spent, {} towers, peak pressure {:.1}, roster: [{}]", r4.phase, r4.wave, r4.gold, r4.gold_spent, r4.towers_len, r4.peak_pressure, r4.roster_str));

    // 5. Corruption-Only Sanity Case on Veteran (checks replacement exploit)
    let r5 = run_legal_mono_policy(
        "Corruption-Only (Veteran)",
        Difficulty::Veteran,
        "upgrade_first",
        Family::Corruption,
        180_000,
    );
    log_lines.push(format!("Corruption-Only (Veteran): {:?} E{}, {}g spare, {}g spent, {} towers, peak pressure {:.1}, roster: [{}]", r5.phase, r5.wave, r5.gold, r5.gold_spent, r5.towers_len, r5.peak_pressure, r5.roster_str));

    // 6. Mono-Siege Sanity Case on Classic (must fail against Air E21)
    let r6 = run_legal_mono_policy(
        "Mono-Siege (Classic)",
        Difficulty::Classic,
        "upgrade_first",
        Family::Siege,
        180_000,
    );
    log_lines.push(format!("Mono-Siege (Classic): {:?} E{}, {}g spare, {}g spent, {} towers, peak pressure {:.1}, roster: [{}]", r6.phase, r6.wave, r6.gold, r6.gold_spent, r6.towers_len, r6.peak_pressure, r6.roster_str));

    // 7. Multi Banking Policy on Classic (Whole-Route Coverage)
    let r_bc = run_legal_mono_policy(
        "Multi Banking (Classic)",
        Difficulty::Classic,
        "banking",
        Family::Multi,
        180_000,
    );
    log_lines.push(format!("Multi Banking (Classic): {:?} E{}, {}g spare, {}g spent, {} towers, max level L{}, peak pressure {:.1}, complete: {}, roster: [{}]", r_bc.phase, r_bc.wave, r_bc.gold, r_bc.gold_spent, r_bc.towers_len, r_bc.max_level, r_bc.peak_pressure, r_bc.complete, r_bc.roster_str));

    // 8. Multi Banking Policy on Veteran (Whole-Route Coverage)
    let r_bv = run_legal_mono_policy(
        "Multi Banking (Veteran)",
        Difficulty::Veteran,
        "banking",
        Family::Multi,
        180_000,
    );
    log_lines.push(format!("Multi Banking (Veteran): {:?} E{}, {}g spare, {}g spent, {} towers, max level L{}, peak pressure {:.1}, complete: {}, roster: [{}]", r_bv.phase, r_bv.wave, r_bv.gold, r_bv.gold_spent, r_bv.towers_len, r_bv.max_level, r_bv.peak_pressure, r_bv.complete, r_bv.roster_str));

    // 9. Multi Entrance-Aware Banking Policy on Classic
    let r_ec = run_legal_mono_policy(
        "Multi Entrance-Aware Banking (Classic)",
        Difficulty::Classic,
        "banking_entrance",
        Family::Multi,
        180_000,
    );
    log_lines.push(format!("Multi Entrance-Aware Banking (Classic): {:?} E{}, {}g spare, {}g spent, {} towers, max level L{}, peak pressure {:.1}, first pos: [{:.2}, {:.2}], total dmg: {:.0}, kills: {}, complete: {}, cause: {}, roster: [{}]", r_ec.phase, r_ec.wave, r_ec.gold, r_ec.gold_spent, r_ec.towers_len, r_ec.max_level, r_ec.peak_pressure, r_ec.first_tower_pos[0], r_ec.first_tower_pos[1], r_ec.total_damage, r_ec.kills, r_ec.complete, r_ec.defeat_cause, r_ec.roster_str));

    // 10. Multi Entrance-Aware Banking Policy on Veteran
    let r_ev = run_legal_mono_policy(
        "Multi Entrance-Aware Banking (Veteran)",
        Difficulty::Veteran,
        "banking_entrance",
        Family::Multi,
        180_000,
    );
    log_lines.push(format!("Multi Entrance-Aware Banking (Veteran): {:?} E{}, {}g spare, {}g spent, {} towers, max level L{}, peak pressure {:.1}, first pos: [{:.2}, {:.2}], total dmg: {:.0}, kills: {}, complete: {}, cause: {}, roster: [{}]", r_ev.phase, r_ev.wave, r_ev.gold, r_ev.gold_spent, r_ev.towers_len, r_ev.max_level, r_ev.peak_pressure, r_ev.first_tower_pos[0], r_ev.first_tower_pos[1], r_ev.total_damage, r_ev.kills, r_ev.complete, r_ev.defeat_cause, r_ev.roster_str));

    let _ = std::fs::write("gemini_strategy_verified.log", log_lines.join("\n"));

    let banking_log = format!(
        "=== MULTI BANKING POLICY RESULTS ===\nMulti Banking (Classic): {:?} E{}, {}g spare, {}g spent, {} towers, max level L{}, peak pressure {:.1}, complete: {}, roster: [{}]\nMulti Banking (Veteran): {:?} E{}, {}g spare, {}g spent, {} towers, max level L{}, peak pressure {:.1}, complete: {}, roster: [{}]\n",
        r_bc.phase, r_bc.wave, r_bc.gold, r_bc.gold_spent, r_bc.towers_len, r_bc.max_level, r_bc.peak_pressure, r_bc.complete, r_bc.roster_str,
        r_bv.phase, r_bv.wave, r_bv.gold, r_bv.gold_spent, r_bv.towers_len, r_bv.max_level, r_bv.peak_pressure, r_bv.complete, r_bv.roster_str
    );
    let _ = std::fs::write("gemini_multi_banking.log", banking_log);

    let entrance_log = format!(
        "=== MULTI ENTRANCE-AWARE BANKING POLICY RESULTS ===\n\
        Multi Entrance-Aware Banking (Classic):\n\
          Terminal State: {:?} at E{}\n\
          Economy: {}g spare, {}g spent\n\
          Defenses: {} towers, max tier: L{}\n\
          First Tower Position: [{:.2}, {:.2}] (damage dealt: {:.0}, fired: {})\n\
          Combat Output: {:.0} total damage, {} kills\n\
          Peak Pressure: {:.1}\n\
          Defeat Cause: {}\n\
          Campaign Complete: {}\n\
          Roster: [{}]\n\n\
        Multi Entrance-Aware Banking (Veteran):\n\
          Terminal State: {:?} at E{}\n\
          Economy: {}g spare, {}g spent\n\
          Defenses: {} towers, max tier: L{}\n\
          First Tower Position: [{:.2}, {:.2}] (damage dealt: {:.0}, fired: {})\n\
          Combat Output: {:.0} total damage, {} kills\n\
          Peak Pressure: {:.1}\n\
          Defeat Cause: {}\n\
          Campaign Complete: {}\n\
          Roster: [{}]\n",
        r_ec.phase, r_ec.wave, r_ec.gold, r_ec.gold_spent, r_ec.towers_len, r_ec.max_level,
        r_ec.first_tower_pos[0], r_ec.first_tower_pos[1], r_ec.first_tower_dmg, r_ec.first_tower_dmg > 0.0,
        r_ec.total_damage, r_ec.kills, r_ec.peak_pressure, r_ec.defeat_cause, r_ec.complete, r_ec.roster_str,
        r_ev.phase, r_ev.wave, r_ev.gold, r_ev.gold_spent, r_ev.towers_len, r_ev.max_level,
        r_ev.first_tower_pos[0], r_ev.first_tower_pos[1], r_ev.first_tower_dmg, r_ev.first_tower_dmg > 0.0,
        r_ev.total_damage, r_ev.kills, r_ev.peak_pressure, r_ev.defeat_cause, r_ev.complete, r_ev.roster_str,
    );
    let _ = std::fs::write("gemini_multi_entrance.log", entrance_log);

    let volley_balance_log = format!(
        "=== MULTI 40% FAN VOLLEY BALANCE VERIFICATION ===\n\n        1. PRE-FIX REPRODUCTION BASELINE (100% Fan Damage; source of truth: astra_multi_entrance_pre_fan40.log):\n           Classic Entrance-Aware Banking: Victory E600, 177300g spent, 4800g spare, 3 towers (2xMultiL10+1xMultiL5), peak 162.0, complete: true\n           Veteran Entrance-Aware Banking: Defeat E4, 700g spent, 53g spare, 1xMultiL2, peak 156.5, complete: false\n\n        2. POST-FIX OUTCOME (40% Fan Damage on Campaign Multi / SuperMulti):\n           Classic Entrance-Aware Banking:\n             Terminal State: {:?} at E{}\n             Defenses: {} towers, max tier: L{}\n             Economy: {}g spare, {}g spent\n             Peak Pressure: {:.1}\n             Defeat Cause: {}\n             Campaign Complete: {}\n             Combat Output: {:.0} damage, {} kills\n             Roster: [{}]\n\n           Veteran Entrance-Aware Banking:\n             Terminal State: {:?} at E{}\n             Defenses: {} towers, max tier: L{}\n             Economy: {}g spare, {}g spent\n             Peak Pressure: {:.1}\n             Defeat Cause: {}\n             Campaign Complete: {}\n             Combat Output: {:.0} damage, {} kills\n             Roster: [{}]\n\n        3. MULTI STRATEGY COMPARISONS (40% Fan):\n           Multi Upgrade-First (Classic): {:?} E{}, peak pressure {:.1}\n           Multi Breadth-First (Classic): {:?} E{}, peak pressure {:.1}\n           Multi Upgrade-First (Veteran): {:?} E{}, peak pressure {:.1}\n           Multi Breadth-First (Veteran): {:?} E{}, peak pressure {:.1}\n           Corruption-Only (Veteran): {:?} E{}, peak pressure {:.1}\n           Mono-Siege (Classic): {:?} E{}, peak pressure {:.1}\n",
        r_ec.phase, r_ec.wave, r_ec.towers_len, r_ec.max_level, r_ec.gold, r_ec.gold_spent,
        r_ec.peak_pressure, r_ec.defeat_cause, r_ec.complete, r_ec.total_damage, r_ec.kills, r_ec.roster_str,
        r_ev.phase, r_ev.wave, r_ev.towers_len, r_ev.max_level, r_ev.gold, r_ev.gold_spent,
        r_ev.peak_pressure, r_ev.defeat_cause, r_ev.complete, r_ev.total_damage, r_ev.kills, r_ev.roster_str,
        r1.phase, r1.wave, r1.peak_pressure,
        r2.phase, r2.wave, r2.peak_pressure,
        r3.phase, r3.wave, r3.peak_pressure,
        r4.phase, r4.wave, r4.peak_pressure,
        r5.phase, r5.wave, r5.peak_pressure,
        r6.phase, r6.wave, r6.peak_pressure,
    );
    let _ = std::fs::write("gemini_multi_volley_balance.log", volley_balance_log);

    // Assertions:
    // Mono-Siege must fail against Air (E21) because it is GroundOnly:
    assert_eq!(r6.phase, Phase::Defeat, "Mono-Siege cannot survive Air encounters (E21) without anti-air");
    assert!(r6.wave >= 10, "Mono-Siege clears early ground encounters");

    // First tower must actually fire against opening packet:
    assert!(r_ec.first_tower_dmg > 0.0, "First tower on Classic must fire against opening packet");
    assert!(r_ev.first_tower_dmg > 0.0, "First tower on Veteran must fire against opening packet");

    // Multi Entrance-Aware Banking must suffer actual Defeat before E600 under the agreed 40% fan volley budget:
    assert_eq!(r_ec.phase, Phase::Defeat, "Multi Entrance-Aware Banking on Classic must suffer Defeat under 40% fan volley budget");
    assert!(r_ec.wave < 600, "Multi Entrance-Aware Banking on Classic must be defeated before E600");
    assert_eq!(r_ev.phase, Phase::Defeat, "Multi Entrance-Aware Banking on Veteran must suffer Defeat");
    assert!(r_ev.wave < 600, "Multi Entrance-Aware Banking on Veteran must be defeated before E600");

    // All runs must terminate in Victory or Defeat (no timeout/hang):
    assert!(matches!(r1.phase, Phase::Victory | Phase::Defeat));
    assert!(matches!(r2.phase, Phase::Victory | Phase::Defeat));
    assert!(matches!(r3.phase, Phase::Victory | Phase::Defeat));
    assert!(matches!(r4.phase, Phase::Victory | Phase::Defeat));
    assert!(matches!(r5.phase, Phase::Victory | Phase::Defeat));
    assert!(matches!(r_bc.phase, Phase::Victory | Phase::Defeat));
    assert!(matches!(r_bv.phase, Phase::Victory | Phase::Defeat));
}

#[test]
fn test_campaign_multi_volley_fan_scaling_actual_shots() {
    let multi_def = def_by_family(Family::Multi);
    let base_damage = TOWERS[multi_def].damage; // 139.0
    let multishot_count = TOWERS[multi_def].abil.multishot as usize; // 3
    assert_eq!(multishot_count, 3, "Multi 1 fires at 3 targets (1 primary + 2 extras)");

    // -------------------------------------------------------------
    // 1. Campaign Mode: Primary shot 100% (139.0), Extra fan shots 40% (55.6)
    // -------------------------------------------------------------
    let mut g_camp = Game::new();
    g_camp.start_campaign(42, Difficulty::Classic);
    isolate(&mut g_camp);

    let p0 = find_in_range_legal_spot(&g_camp, multi_def).expect("spot for multi");
    g_camp.build_choice = Some((multi_def, 1));
    assert!(g_camp.try_build_at(p0));
    assert_eq!(g_camp.towers.len(), 1);
    let tower_pos = g_camp.towers[0].pos;

    // Spawn 3 creeps in range
    let creep_def = creep(200.0, 0, ArmourType::Unarmoured, false);
    for i in 0..3 {
        g_camp.spawn_creep(&creep_def, creep_def.hp, 1.0, 5.0 + (i as f32) * 0.3);
        g_camp.creeps[i].pos = [tower_pos[0] + 1.0 + (i as f32) * 0.2, tower_pos[1]];
    }
    g_camp.spatial.rebuild(&g_camp.creeps);
    g_camp.towers[0].cooldown = 0.0;

    combat::step_towers(&mut g_camp, 1.0 / 60.0);
    assert_eq!(g_camp.projs.len(), 3, "Multi 1 must launch exactly 3 projectiles for 3 in-range targets");

    // Proj 0 is primary target: 100% base damage
    let p_prim = &g_camp.projs[0];
    let prim_ci = p_prim.target_idx;
    assert!((p_prim.dmg - base_damage).abs() < 1e-3, "Primary shot in Campaign must have 100% base damage ({base_damage}), got {}", p_prim.dmg);

    // Proj 1 and 2 are extra fan shots: 40% damage (0.40 * base_damage)
    let mut fan_targets = Vec::new();
    for i in 1..3 {
        let p_fan = &g_camp.projs[i];
        let expected_fan = base_damage * 0.40;
        assert!(
            (p_fan.dmg - expected_fan).abs() < 1e-3,
            "Extra fan shot #{i} in Campaign must have 40% damage ({expected_fan}), got {}",
            p_fan.dmg
        );
        assert_ne!(p_fan.target_idx, prim_ci, "Extra shot must target a different creep than primary");
        fan_targets.push(p_fan.target_idx);
    }
    assert_ne!(fan_targets[0], fan_targets[1], "Extra shots must target distinct creeps");

    // Verify definition and damage captured across tower sale:
    g_camp.sell(0);
    assert_eq!(g_camp.towers.len(), 0, "Multi tower sold while projectiles in flight");
    let initial_hps: Vec<f32> = g_camp.creeps.iter().map(|c| c.hp).collect();

    // Step projectiles until impact:
    for _ in 0..120 {
        combat::step_projectiles(&mut g_camp, 1.0 / 60.0);
    }
    assert_eq!(g_camp.projs.len(), 0, "All projectiles reached targets");

    let expected_prim_dealt = damage_taken(base_damage * 1.15, Attack::Normal, 0, ArmourType::Unarmoured);
    let expected_fan_dealt = damage_taken(base_damage * 0.40 * 1.15, Attack::Normal, 0, ArmourType::Unarmoured);

    let camp_primary_dealt = initial_hps[prim_ci] - g_camp.creeps[prim_ci].hp;
    assert!(
        (camp_primary_dealt - expected_prim_dealt).abs() < 0.1,
        "Primary hit must deal full 100% damage: got {camp_primary_dealt}, expected {expected_prim_dealt}"
    );
    for &ci in &fan_targets {
        let fan_dealt = initial_hps[ci] - g_camp.creeps[ci].hp;
        assert!(
            (fan_dealt - expected_fan_dealt).abs() < 0.1,
            "Extra hit must deal scaled 40% damage: got {fan_dealt}, expected {expected_fan_dealt}"
        );
    }

    // -------------------------------------------------------------
    // 2. Legacy Mode: Primary 100%, Extra fan shots 100% (UNSCALED)
    // -------------------------------------------------------------
    let mut g_leg = Game::new();
    assert!(!g_leg.is_campaign());

    let p0_leg = find_in_range_legal_spot(&g_leg, multi_def).expect("spot for multi");
    g_leg.build_choice = Some((multi_def, 1));
    assert!(g_leg.try_build_at(p0_leg));
    let tower_pos_leg = g_leg.towers[0].pos;

    for i in 0..3 {
        g_leg.spawn_creep(&creep_def, creep_def.hp, 1.0, 5.0 + (i as f32) * 0.3);
        g_leg.creeps[i].pos = [tower_pos_leg[0] + 1.0 + (i as f32) * 0.2, tower_pos_leg[1]];
    }
    g_leg.spatial.rebuild(&g_leg.creeps);
    g_leg.towers[0].cooldown = 0.0;

    combat::step_towers(&mut g_leg, 1.0 / 60.0);
    assert_eq!(g_leg.projs.len(), 3, "Multi 1 must launch 3 projectiles in Legacy");

    // In Legacy, ALL 3 projectiles retain full 100% base damage:
    for (i, p) in g_leg.projs.iter().enumerate() {
        assert!(
            (p.dmg - base_damage).abs() < 1e-3,
            "Legacy shot #{i} must remain strictly unscaled at 100% base damage ({base_damage}), got {}",
            p.dmg
        );
    }

    // Step projectiles to impact in Legacy:
    let initial_hps_leg: Vec<f32> = g_leg.creeps.iter().map(|c| c.hp).collect();
    for _ in 0..120 {
        combat::step_projectiles(&mut g_leg, 1.0 / 60.0);
    }
    for i in 0..3 {
        let leg_dealt = initial_hps_leg[i] - g_leg.creeps[i].hp;
        let expected_leg_dealt = damage_taken(base_damage, Attack::Normal, 0, ArmourType::Unarmoured);
        assert!(
            (leg_dealt - expected_leg_dealt).abs() < 0.1,
            "Legacy creep #{i} must take full unscaled damage: got {leg_dealt}, expected {expected_leg_dealt}"
        );
    }

    // -------------------------------------------------------------
    // 3. Campaign Instant Saturation Path: Covers Detonation fallback
    // -------------------------------------------------------------
    let mut g_sat = Game::new();
    g_sat.start_campaign(42, Difficulty::Classic);
    isolate(&mut g_sat);

    let p0_sat = find_in_range_legal_spot(&g_sat, multi_def).expect("spot for multi");
    g_sat.build_choice = Some((multi_def, 1));
    assert!(g_sat.try_build_at(p0_sat));
    let tower_pos_sat = g_sat.towers[0].pos;

    // Pre-fill projectiles to MAX_PROJECTILES to force immediate Detonation path
    for _ in 0..MAX_PROJECTILES {
        g_sat.projs.push(Proj {
            pos: [0.0, 0.0],
            z: 0.0,
            vel: [0.0, 0.0],
            kind: ProjKind::Dart,
            tower: 0,
            def: multi_def,
            dmg: 0.0,
            splash: 0.0,
            bounces: 0,
            crit: false,
            target_idx: 0,
            target_uid: 0,
            life: 0.0,
            trail: 0.0,
        });
    }
    assert_eq!(g_sat.projs.len(), MAX_PROJECTILES);

    for i in 0..3 {
        g_sat.spawn_creep(&creep_def, creep_def.hp, 1.0, 5.0 + (i as f32) * 0.3);
        g_sat.creeps[i].pos = [tower_pos_sat[0] + 1.0 + (i as f32) * 0.2, tower_pos_sat[1]];
    }
    g_sat.spatial.rebuild(&g_sat.creeps);
    g_sat.towers[0].cooldown = 0.0;

    let hps_before_sat: Vec<f32> = g_sat.creeps.iter().map(|c| c.hp).collect();
    combat::step_towers(&mut g_sat, 1.0 / 60.0);

    let sat_prim_uid = g_sat.towers[0].target_uid;
    let sat_prim_ci = g_sat.creeps.iter().position(|c| c.uid == sat_prim_uid).expect("primary creep");

    // Direct detonation applied on firing frame:
    let sat_prim_dealt = hps_before_sat[sat_prim_ci] - g_sat.creeps[sat_prim_ci].hp;
    assert!(
        (sat_prim_dealt - expected_prim_dealt).abs() < 0.1,
        "Saturation primary hit must deal full 100% damage: got {sat_prim_dealt}, expected {expected_prim_dealt}"
    );

    for i in 0..3 {
        if i != sat_prim_ci {
            let sat_fan_dealt = hps_before_sat[i] - g_sat.creeps[i].hp;
            assert!(
                (sat_fan_dealt - expected_fan_dealt).abs() < 0.1,
                "Saturation extra hit must deal scaled 40% damage: got {sat_fan_dealt}, expected {expected_fan_dealt}"
            );
        }
    }

    // -------------------------------------------------------------
    // 4. SuperMulti Campaign Scaling: 100% primary + 40% fan on 10 targets
    // -------------------------------------------------------------
    let smulti_def = def_by_family(Family::SuperMulti);
    let smulti_base_dmg = TOWERS[smulti_def].damage; // 39999.0
    let mut g_smulti = Game::new();
    g_smulti.start_campaign(42, Difficulty::Classic);
    isolate(&mut g_smulti);
    g_smulti.gold = 200_000;
    let p_sm = find_in_range_legal_spot(&g_smulti, smulti_def).expect("spot for smulti");
    g_smulti.build_choice = Some((smulti_def, 1));
    assert!(g_smulti.try_build_at(p_sm));
    let sm_pos = g_smulti.towers[0].pos;

    // Spawn 4 creeps in range
    for i in 0..4 {
        g_smulti.spawn_creep(&creep_def, 200_000.0, 1.0, 5.0 + (i as f32) * 0.3);
        g_smulti.creeps[i].pos = [sm_pos[0] + 1.0 + (i as f32) * 0.2, sm_pos[1]];
    }
    g_smulti.spatial.rebuild(&g_smulti.creeps);
    g_smulti.towers[0].cooldown = 0.0;
    combat::step_towers(&mut g_smulti, 1.0 / 60.0);

    assert_eq!(g_smulti.projs.len(), 4, "SuperMulti fires at all 4 available targets");
    assert!((g_smulti.projs[0].dmg - smulti_base_dmg).abs() < 1e-2, "SuperMulti primary must be 100%");
    for i in 1..4 {
        let expected_sm_fan = smulti_base_dmg * 0.40;
        assert!(
            (g_smulti.projs[i].dmg - expected_sm_fan).abs() < 1e-2,
            "SuperMulti fan shot #{i} must be 40% ({expected_sm_fan}), got {}",
            g_smulti.projs[i].dmg
        );
    }
}
