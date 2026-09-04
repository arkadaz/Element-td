//! Simulation soak tests.
//!
//! Two jobs. The first is index bookkeeping: creeps are removed with
//! `swap_remove` while projectiles, splash lists and pads hold indices into the
//! same vectors, and these tests hammer those paths. The second is that the
//! game the map describes is actually playable - that a sensible board clears
//! thirty-six waves, that a board with no answer to the air does not, and that
//! nothing can wedge a wave open forever.

use super::board::ROAD_HALF;
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
/// By position rather than by pad number, because the lane is eighty-five tiles
/// round and a pad index says nothing about where on it a tower stands - twelve
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
fn pads_sit_beside_the_road_never_on_it() {
    let b = super::board::Board::new();
    assert!(b.slots.len() > 40, "only {} pads", b.slots.len());
    for s in &b.slots {
        let tx = s.pos[0].floor() as i32;
        let ty = s.pos[1].floor() as i32;
        assert!(
            !super::board::is_corridor(tx, ty),
            "a pad sits in the corridor at {:?}",
            s.pos
        );
        assert!(
            b.dist_to_road(s.pos) > ROAD_HALF,
            "a pad overlaps the lane at {:?}",
            s.pos
        );
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

// ---------------------------------------------------------------- the loss

#[test]
fn an_undefended_ring_floods_and_the_run_is_lost() {
    let mut g = Game::new();
    g.start_run(1);
    run_for(&mut g, 60.0 * 25.0);
    assert_eq!(
        g.phase,
        Phase::Defeat,
        "an empty board survived; ring holds {}/{FLOOD_LIMIT}",
        g.creeps.len()
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
    for n in [1u32, 12, 24, 36] {
        let w = wave_at(n);
        let window = w.spawn_gap * w.count as f32;
        assert!(
            window <= WAVE_SPAWN_WINDOW + 0.5,
            "wave {n} takes {window:.0}s to arrive"
        );
    }
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
fn a_tower_cannot_be_built_without_the_gold() {
    let mut g = Game::new();
    g.gold = 9;
    g.build_choice = Some((root(Family::Single), 1));
    assert!(!g.try_build(0), "a ten gold tower was built with nine gold");
    assert!(g.towers.is_empty());
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
/// Two rules, and between them they are the whole of the game's strategy:
///
///   1. **Cover what is coming.** If an air wave is due and nothing on the
///      board can reach the air, or an Immune wave is due and nothing deals
///      Chaos, everything is put on hold until that gap is filled. This is the
///      rule the roster exists for: throughput alone loses on wave five.
///   2. **Otherwise, buy whatever is cheapest** - the next tower on the plan or
///      the cheapest upgrade on the board. A player with spare gold spends it.
#[derive(Default)]
struct Planner {
    built: usize,
}

/// The order a board gets built in when nothing is urgent, as a repeating
/// pattern rather than a fixed list.
///
/// It used to be a list of exactly twenty-six, which was a sensible number when
/// the lane was eighty-five tiles. The map's real circuit is two hundred and
/// thirty-six, and a fixed twenty-six towers on it is a board with holes you
/// could walk an army through - which is exactly what happened: the campaign
/// test died on wave fifteen with twelve towers built and seven hundred and one
/// monsters circling.
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
            let build = if self.built < target * 2 {
                let f = plan_at(self.built);
                Some((f, seed_cost(f)))
            } else {
                None
            };
            let up = cheapest_upgrade(g);
            // Cover the whole lane before deepening any of it. Four very good
            // towers watch a fraction of a two-hundred-tile circuit and the rest
            // walks past them.
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
    /// Beside the lane, spread along it. The arena is a thousand plots and only
    /// the ones within a tower's reach of the corridor are worth anything - the
    /// first free pad in index order is in the far corner of the field, and a
    /// board built there fires at nothing at all.
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
