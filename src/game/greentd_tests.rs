//! Checks that the extracted Green Circle TD tables say what the map says.
//!
//! These are not balance tests. They are assertions that the *extraction* is
//! faithful - that the numbers in `greentd.rs` are the map's numbers and that
//! the mechanics they depend on behave the way Warcraft III behaves. Everything
//! here was read out of `GREEN TD 9.3c PEIN.w3x` with an MPQ reader and an
//! object-data parser, so it is exactly the kind of thing that can be subtly
//! wrong in a way nobody notices for months.

use super::greentd::{LEVELS, WAVES};
use super::greentd_types::*;

/// Every family, and how many towers the map puts in it.
const FAMILIES: [(Family, usize); 24] = [
    (Family::Siege, 20),
    (Family::Poison, 15),
    (Family::Bouncing, 10),
    (Family::Multi, 10),
    (Family::Air, 10),
    (Family::Critical, 10),
    (Family::Troll, 7),
    (Family::Corruption, 5),
    (Family::Chaos, 5),
    (Family::Destruction, 5),
    (Family::Demon, 5),
    (Family::King, 4),
    (Family::Slow, 4),
    (Family::Fire, 4),
    (Family::Aura, 3),
    (Family::Damage, 3),
    (Family::Speed, 3),
    (Family::OneStrike, 2),
    (Family::Single, 1),
    (Family::SuperChaos, 1),
    (Family::SuperDestruct, 1),
    (Family::SuperMulti, 1),
    (Family::SuperBounce, 1),
    (Family::Frost, 1),
];

#[test]
fn the_roster_is_the_map_s_roster() {
    assert_eq!(
        LEVELS.len(),
        131,
        "the map has 131 towers across 24 families"
    );
    assert_eq!(
        FAMILIES.iter().map(|(_, n)| n).sum::<usize>(),
        LEVELS.len(),
        "the family table does not add up to the roster"
    );

    for (family, len) in FAMILIES {
        let n = LEVELS.iter().filter(|t| t.family == family).count();
        assert_eq!(n, len, "{family:?} should have {len} towers, found {n}");
    }

    // Every family is numbered 0..n with no gaps and no repeats, so "step 4 of
    // 20" in the HUD means what it says.
    for (family, _) in FAMILIES {
        let mut steps: Vec<u32> = LEVELS
            .iter()
            .filter(|t| t.family == family)
            .map(|t| t.step)
            .collect();
        steps.sort_unstable();
        let want: Vec<u32> = (0..steps.len() as u32).collect();
        assert_eq!(steps, want, "{family:?} is numbered wrongly");
    }

    // And the long ladders climb: each rung hits harder than the one below.
    for family in [
        Family::Siege,
        Family::Poison,
        Family::Critical,
        Family::Multi,
        Family::Bouncing,
        Family::Air,
    ] {
        let mut rungs: Vec<&TowerLevel> = LEVELS.iter().filter(|t| t.family == family).collect();
        rungs.sort_by_key(|t| t.step);
        for pair in rungs.windows(2) {
            assert!(
                pair[1].dps() > pair[0].dps(),
                "{family:?}: {} is no stronger than {}",
                pair[1].name,
                pair[0].name
            );
        }
    }

    // A rung should cost more than the one under it. A handful do not, and
    // those are the map's, not the extraction's - Siege 1 to 2 drops from 100
    // gold to 50. Copying exactly means copying that, so the anomalies are
    // counted rather than corrected: a bound catches a systematic regression (a
    // family sorted wrongly would produce dozens) without pretending the source
    // data is tidier than it is.
    let mut odd = 0;
    for (family, _) in FAMILIES {
        let mut rungs: Vec<&TowerLevel> = LEVELS.iter().filter(|t| t.family == family).collect();
        rungs.sort_by_key(|t| t.step);
        for p in rungs.windows(2) {
            if p[1].gold <= p[0].gold && p[1].gold > 0 {
                odd += 1;
            }
        }
    }
    assert!(odd <= 6, "{odd} rungs cost no more than the one below");
}

/// The upgrade graph, which is the part a straight ladder model got wrong.
#[test]
fn the_upgrade_graph_is_the_map_s_graph() {
    let index = |name: &str| {
        LEVELS
            .iter()
            .position(|t| t.name == name)
            .unwrap_or_else(|| panic!("no tower called {name}"))
    };
    let names = |i: usize| -> Vec<&str> {
        LEVELS[i]
            .upgrades
            .iter()
            .map(|&u| LEVELS[u as usize].name)
            .collect()
    };

    // Every edge points at a tower that exists.
    for t in LEVELS {
        for &u in t.upgrades {
            assert!(
                (u as usize) < LEVELS.len(),
                "{} upgrades into nothing at {u}",
                t.name
            );
        }
    }

    // The ten gold seed becomes one of six families, and none of those six can
    // be bought any other way.
    let seed = index("Single shot Tower");
    assert_eq!(LEVELS[seed].gold, 10);
    assert_eq!(
        LEVELS[seed].upgrades.len(),
        6,
        "the seed should offer six ways up, offers {:?}",
        names(seed)
    );
    for &u in LEVELS[seed].upgrades {
        let t = &LEVELS[u as usize];
        assert!(!t.shop, "{} should not be buyable from the shop", t.name);
    }

    // The Aura Tower branches into Damage or Speed at every rung, and those two
    // are free side-grades of each other.
    let aura = index("Aura Tower");
    assert_eq!(
        LEVELS[aura].upgrades.len(),
        3,
        "Aura Tower should offer Aura 2, Damage and Speed, offers {:?}",
        names(aura)
    );
    for name in ["Damage Tower", "Speed Tower"] {
        assert_eq!(
            LEVELS[index(name)].gold,
            0,
            "{name} is a free side-grade in the map"
        );
    }

    // The King Tower opens the four Super towers.
    let king = index("King Tower");
    assert_eq!(
        LEVELS[king].upgrades.len(),
        5,
        "King Tower should offer its own level 2 and four Supers, offers {:?}",
        names(king)
    );
    let supers = LEVELS
        .iter()
        .filter(|t| t.gold == 100_000)
        .collect::<Vec<_>>();
    assert_eq!(supers.len(), 4, "there are four hundred-thousand towers");
    for t in supers {
        assert_eq!(t.attack, Attack::Chaos, "{} should deal Chaos", t.name);
    }

    // And the Slow Tower's last rung becomes the Snowman.
    let slow4 = index("Slow Tower 4: Perfect");
    assert_eq!(names(slow4), vec!["Frost Tower Perfect: Snowman"]);
}

#[test]
fn the_shop_is_eleven_towers_and_the_seed_is_ten_gold() {
    let roots: Vec<&TowerLevel> = LEVELS.iter().filter(|t| t.shop).collect();
    assert_eq!(roots.len(), 11, "the map's builder offers eleven towers");

    // A shop tower is exactly one nothing upgrades into.
    for (i, t) in LEVELS.iter().enumerate() {
        let reachable = LEVELS.iter().any(|o| o.upgrades.contains(&(i as u16)));
        assert_eq!(
            t.shop, !reachable,
            "{} is marked shop={} but reachable={reachable}",
            t.name, t.shop
        );
    }

    let cheapest = roots.iter().min_by_key(|t| t.gold).unwrap();
    assert_eq!(cheapest.gold, 10, "the seed tower costs ten gold");
    assert_eq!(cheapest.family, Family::Single);
}

/// Selling refunds the map's own point value, which `UpgradeRefundRate=1.0`
/// makes very nearly everything the tower cost to get to.
#[test]
fn selling_pays_back_what_the_map_says() {
    // Almost every tower refunds at least what it cost - `UpgradeRefundRate=1.0`
    // in the map's own constants. The exceptions are the "Perfect" towers at the
    // top of each path, which the map deliberately makes a one-way purchase, and
    // there should only be a handful of those.
    let short: Vec<&str> = LEVELS
        .iter()
        .filter(|t| t.refund < t.gold)
        .map(|t| t.name)
        .collect();
    assert!(
        short.len() <= 15,
        "{} towers refund less than they cost: {short:?}",
        short.len()
    );
    for name in &short {
        assert!(
            name.contains("Perfect") || name.contains("Snowman"),
            "{name} refunds less than it cost but is not a top-of-path tower"
        );
    }
    let siege2 = LEVELS
        .iter()
        .find(|t| t.name.starts_with("Siege Tower 2:"))
        .unwrap();
    assert_eq!(siege2.gold, 50, "Siege 2 costs 50 in the map");
    assert_eq!(
        siege2.refund, 150,
        "and refunds everything sunk into it so far"
    );
}

#[test]
fn only_the_air_family_can_answer_what_flies() {
    // Air Tower 1 hits *nothing but* air, which is what makes anti-air a real
    // purchase rather than something you get for free with damage.
    let air1 = LEVELS
        .iter()
        .find(|t| t.family == Family::Air && t.step == 0)
        .expect("no Air Tower");
    assert_eq!(air1.targets, Targets::AirOnly);

    // Siege, Chaos and Destruction are stuck on the ground, all the way up.
    for f in [Family::Siege, Family::Chaos, Family::Destruction] {
        for t in LEVELS.iter().filter(|t| t.family == f) {
            assert_eq!(
                t.targets,
                Targets::GroundOnly,
                "{} should be ground only",
                t.name
            );
        }
    }

    // Five waves fly, and every one of them is something with wings or a rotor.
    let flying: Vec<&WaveRow> = WAVES.iter().filter(|w| w.flying).collect();
    assert_eq!(flying.len(), 5, "the map sends five air waves");
    for w in flying {
        assert!(
            w.model.airborne(),
            "wave {} flies but is drawn as {:?}",
            w.wave,
            w.model
        );
    }
}

/// The counter system, and the reason four families exist.
///
/// This map rewrites Warcraft III's whole attack table in `war3mapMisc.txt`.
/// Everything does full damage to everything except Immune, which takes five
/// percent from all of it - and Chaos, which the engine hard-codes at 1.0, and
/// Hero, which the map sets to a hundred times.
#[test]
fn immune_waves_are_answered_only_by_chaos_and_hero() {
    for a in [
        Attack::Normal,
        Attack::Pierce,
        Attack::Siege,
        Attack::Magic,
        Attack::Spells,
    ] {
        assert_eq!(
            type_mult(a, ArmourType::Divine),
            0.05,
            "{a:?} should do 5% to Immune"
        );
        // ...and full damage to everything else. There is no rock-paper-scissors
        // left in this map, and pretending otherwise misprices the whole roster.
        for d in [
            ArmourType::Unarmoured,
            ArmourType::Light,
            ArmourType::Medium,
            ArmourType::Heavy,
            ArmourType::Fortified,
            ArmourType::Hero,
        ] {
            assert_eq!(type_mult(a, d), 1.0, "{a:?} should be unresisted by {d:?}");
        }
    }
    assert_eq!(type_mult(Attack::Chaos, ArmourType::Divine), 1.0);
    assert_eq!(
        type_mult(Attack::Hero, ArmourType::Divine),
        100.0,
        "Hero damage is multiplied by a hundred - that is the One-Strike Kill Tower"
    );

    // Somebody must actually be able to deal both.
    assert!(
        LEVELS.iter().any(|t| t.attack == Attack::Chaos),
        "nothing in the roster deals Chaos damage"
    );
    assert!(
        LEVELS.iter().any(|t| t.attack == Attack::Hero),
        "nothing in the roster deals Hero damage"
    );

    // And Immune has to turn up often enough to matter: every fifth wave.
    let divine: Vec<u32> = WAVES
        .iter()
        .filter(|w| w.armour_type == ArmourType::Divine)
        .map(|w| w.wave)
        .collect();
    assert_eq!(
        divine,
        vec![5, 10, 15, 20, 25, 30, 35, 36],
        "Immune arrives every fifth wave, and the last wave is Immune too"
    );
}

/// The generated table is the map's table.
///
/// `type_mult` reads `greentd::DAMAGE`, which `tools/emit.py` writes out of
/// `war3mapMisc.txt`. This pins the six rows that file actually contains, so a
/// regenerated table that disagrees with the map fails here rather than
/// quietly changing what every tower in the game is worth.
#[test]
fn the_damage_table_is_generated_from_the_map() {
    use super::greentd::DAMAGE;
    assert_eq!(DAMAGE.len(), 7);
    for row in DAMAGE {
        assert_eq!(row.len(), 7);
    }
    // Every ordinary attack: full damage to the six ordinary armours, five
    // percent to Immune.
    for a in [
        Attack::Normal,
        Attack::Pierce,
        Attack::Siege,
        Attack::Magic,
        Attack::Spells,
    ] {
        for d in [
            ArmourType::Unarmoured,
            ArmourType::Light,
            ArmourType::Medium,
            ArmourType::Heavy,
            ArmourType::Fortified,
            ArmourType::Hero,
        ] {
            assert_eq!(DAMAGE[a.idx()][d.idx()], 1.0, "{a:?} vs {d:?}");
        }
        assert_eq!(DAMAGE[a.idx()][ArmourType::Divine.idx()], 0.05, "{a:?} vs Immune");
    }
    // Chaos is the engine's, not the file's, and Hero is the file's hundred.
    for d in [ArmourType::Unarmoured, ArmourType::Hero, ArmourType::Divine] {
        assert_eq!(DAMAGE[Attack::Chaos.idx()][d.idx()], 1.0);
        assert_eq!(DAMAGE[Attack::Hero.idx()][d.idx()], 100.0);
    }
}

#[test]
fn armour_values_reduce_the_way_warcraft_does() {
    // No armour, no reduction.
    assert!((armour_mult(0) - 1.0).abs() < 1e-6);
    // Each point is worth six percent of a point, stacking with falloff.
    assert!(
        (armour_mult(10) - 1.0 / 1.6).abs() < 1e-4,
        "{}",
        armour_mult(10)
    );
    // It approaches immunity without reaching it. The last wave carries 200.
    let two_hundred = armour_mult(200);
    assert!(
        (0.07..0.09).contains(&two_hundred),
        "200 armour should take about 8%, takes {two_hundred:.3}"
    );
    // And the map's worst is 700.
    assert!(armour_mult(700) > 0.0 && armour_mult(700) < 0.03);
    assert!(
        WAVES.iter().any(|w| w.armour >= 700),
        "no wave carries the map's worst armour"
    );
}

/// The abilities, which come out of a separate file to the units and are the
/// easiest thing in the whole extraction to lose silently.
#[test]
fn the_abilities_carry_the_map_s_numbers() {
    let by_name = |n: &str| LEVELS.iter().find(|t| t.name == n).unwrap();

    // Critical Tower 1: "Gives a 30% chance to do 4 times normal damage."
    let crit = by_name("Critical Tower 1");
    assert!((crit.abil.crit_chance - 0.30).abs() < 1e-4);
    assert!((crit.abil.crit_mult - 4.0).abs() < 1e-4);

    // Poison Tower 1: 100 a second and 30% slower for seven seconds.
    let poison = by_name("Poison Tower 1: Kel'Thuzad Ghost");
    assert!((poison.abil.poison_dps - 100.0).abs() < 1e-3);
    assert!((poison.abil.poison_slow - 0.30).abs() < 1e-4);
    assert!((poison.abil.poison_dur - 7.0).abs() < 1e-4);

    // Demon Tower 1: a one-in-ten chance of killing outright.
    assert!((by_name("Demon Tower 1").abil.kill_chance - 0.10).abs() < 1e-4);

    // Damage Tower 3: every tower within fifteen tiles hits 80% harder.
    let dmg = by_name("Damage Tower 3");
    assert!((dmg.abil.dmg_aura - 0.80).abs() < 1e-4);
    assert!(dmg.abil.aura_range > 14.0);

    // Speed Tower 3: the same, for attack rate.
    assert!((by_name("Speed Tower 3").abil.speed_aura - 0.80).abs() < 1e-4);

    // Multi Tower 1 fires on three at once; the Perfect one on ten.
    assert_eq!(by_name("Multi Tower 1").abil.multishot, 3);
    assert_eq!(by_name("Super Multi Tower : Perfect").abil.multishot, 10);

    // Corruption strips armour, fifteen points at a time up to seventy-five.
    assert_eq!(by_name("Corruption Tower 1").abil.armour_pen, 15);
    assert_eq!(by_name("Corruption Tower 5: Perfect").abil.armour_pen, 75);

    // The Troll Tower roots, and works itself up.
    let troll = by_name("Troll Tower 1");
    assert!(troll.abil.root_chance > 0.0);
    assert!(troll.abil.frenzy > 0.0 && troll.abil.frenzy_cd > troll.abil.frenzy_dur);

    // Every ability the roster carries must actually be reachable.
    assert!(LEVELS.iter().any(|t| t.abil.bounce > 0), "nothing bounces");
    assert!(
        LEVELS.iter().any(|t| t.abil.burn_dps > 0.0),
        "nothing burns"
    );
    assert!(
        LEVELS.iter().any(|t| t.abil.slow_amt > 0.0),
        "nothing slows"
    );
}

#[test]
fn the_waves_are_the_map_s_waves() {
    assert_eq!(WAVES.len(), 36, "the campaign is thirty-six waves");
    assert_eq!(WAVES[0].name, "Troll");
    assert_eq!(WAVES[0].hp, 250.0);
    assert_eq!(WAVES[0].armour, 0);
    assert_eq!(WAVES[0].count, 66);

    // Eight of the waves write their count as a JASS character literal rather
    // than a number - `set udg_integer14='}'` is a hundred and twenty-five.
    // Read literally those waves send nothing at all, which is exactly how they
    // came out the first time.
    for w in WAVES {
        assert!(w.count > 0, "wave {} sends nothing", w.wave);
    }
    assert_eq!(WAVES[6].count, 125, "wave 7 sends '}}' creeps");
    assert_eq!(WAVES[35].count, 120, "wave 36 sends 'x' creeps");

    // Health climbs by four orders of magnitude across the campaign, which is
    // what the six-figure end of the tower roster is priced against.
    let last = WAVES.last().unwrap();
    assert!(last.hp >= 500_000.0, "the last wave has {} health", last.hp);
    assert!(
        last.hp / WAVES[0].hp > 1000.0,
        "the campaign only scales {}x",
        last.hp / WAVES[0].hp
    );

    // Waves are streams: counts in the dozens and hundreds, not single bosses.
    let big = WAVES.iter().filter(|w| w.count >= 60).count();
    assert!(big >= 25, "only {big} waves arrive in real numbers");

    // The map banners four kinds of wave, and every one of them is used.
    for tag in ["Air", "Immune", "Hero", "Boss"] {
        assert!(
            WAVES.iter().any(|w| w.tag == tag),
            "no wave is tagged {tag}"
        );
    }
}

/// Prints the roster. Run it to see what was extracted:
///     cargo test --release show_the_extracted_roster -- --ignored --nocapture
#[test]
#[ignore = "prints the whole roster"]
fn show_the_extracted_roster() {
    println!();
    println!("GREEN CIRCLE TD - extracted roster");
    println!("{:-<110}", "");
    let mut fams: Vec<Family> = Vec::new();
    for t in LEVELS {
        if !fams.contains(&t.family) {
            fams.push(t.family);
        }
    }
    for f in fams {
        let rungs: Vec<&TowerLevel> = LEVELS.iter().filter(|t| t.family == f).collect();
        println!(
            "{:<18} {:>2} levels  {:<10} {:<16} {}",
            f.name(),
            rungs.len(),
            format!("{:?}", rungs[0].attack),
            rungs[0].targets.label(),
            if rungs[0].shop {
                "shop"
            } else {
                "upgrade only"
            }
        );
        for t in rungs {
            let next: Vec<&str> = t
                .upgrades
                .iter()
                .map(|&u| LEVELS[u as usize].name)
                .collect();
            println!(
                "    {:<40} {:>7}g  dmg {:>9.0}  cd {:.2}  rng {:>5.1}  aoe {:>4.1}  -> {}",
                t.name,
                t.gold,
                t.damage,
                t.cooldown,
                t.range,
                t.splash,
                next.join(", ")
            );
        }
    }
    println!();
    println!("WAVES");
    println!("{:-<110}", "");
    for w in WAVES {
        println!(
            "  {:>2}  {:<16} x{:<5} hp {:>9.0}  armour {:>3} {:<11} {:<7} {:?}",
            w.wave,
            w.name,
            w.count,
            w.hp,
            w.armour,
            w.armour_type.name(),
            w.tag,
            w.model
        );
    }
}
