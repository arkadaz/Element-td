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

#[test]
fn the_roster_is_the_map_s_roster() {
    assert_eq!(
        LEVELS.len(),
        120,
        "the map has 120 tower levels across 17 families"
    );

    // The ladders, exactly as the map has them.
    for (family, len) in [
        (Family::Single, 1),
        (Family::Siege, 20),
        (Family::Poison, 15),
        (Family::Critical, 10),
        (Family::Multi, 10),
        (Family::Bouncing, 10),
        (Family::Air, 10),
        (Family::Troll, 7),
        (Family::Chaos, 5),
        (Family::Destruction, 5),
        (Family::Corruption, 5),
        (Family::Demon, 5),
        (Family::King, 4),
        (Family::Fire, 4),
        (Family::Slow, 4),
        (Family::Aura, 3),
        (Family::OneStrike, 2),
    ] {
        let n = LEVELS.iter().filter(|t| t.family == family).count();
        assert_eq!(n, len, "{:?} should have {len} levels, found {n}", family);
    }

    // Every ladder is a ladder: steps run 0..n with no gaps and no repeats.
    for family in LEVELS.iter().map(|t| t.family) {
        let mut steps: Vec<u32> = LEVELS
            .iter()
            .filter(|t| t.family == family)
            .map(|t| t.step)
            .collect();
        steps.sort_unstable();
        let want: Vec<u32> = (0..steps.len() as u32).collect();
        assert_eq!(steps, want, "{family:?} has a broken ladder");
    }

    // And it climbs: each rung costs more and hits harder than the one below.
    for family in [
        Family::Siege,
        Family::Poison,
        Family::Critical,
        Family::Multi,
    ] {
        let mut rungs: Vec<&TowerLevel> = LEVELS.iter().filter(|t| t.family == family).collect();
        rungs.sort_by_key(|t| t.step);
        for pair in rungs.windows(2) {
            // Damage always climbs. This is the property the ladder is for.
            assert!(
                pair[1].dps() > pair[0].dps(),
                "{family:?}: {} is no stronger than {}",
                pair[1].name,
                pair[0].name
            );
        }
        // Damage is asserted above. Price is not asserted per family, because
        // the map's own pricing is not monotonic and pinning each exception by
        // hand turns this test into a transcript. It is bounded below instead.
    }

    // Across the whole roster, a rung should cost more than the one under it.
    // A handful do not, and those are the map's, not the extraction's - Siege
    // 1 to 2 drops from 100 gold to 50. Copying exactly means copying that, so
    // the anomalies are counted rather than corrected: a bound catches a
    // systematic regression (a ladder sorted wrongly would produce dozens)
    // without pretending the source data is tidier than it is.
    let mut odd: Vec<String> = Vec::new();
    for family in LEVELS.iter().map(|t| t.family) {
        let mut rungs: Vec<&TowerLevel> = LEVELS.iter().filter(|t| t.family == family).collect();
        rungs.sort_by_key(|t| t.step);
        for p in rungs.windows(2) {
            if p[1].gold <= p[0].gold {
                odd.push(format!(
                    "{} ({}g) -> {} ({}g)",
                    p[0].name, p[0].gold, p[1].name, p[1].gold
                ));
            }
        }
    }
    odd.sort();
    odd.dedup();
    assert!(
        odd.len() <= 6,
        "{} rungs cost no more than the one below - the ladders are probably          ordered wrongly:
  {}",
        odd.len(),
        odd.join("
  ")
    );
}

#[test]
fn the_shop_is_eleven_towers_and_the_seed_is_ten_gold() {
    let roots: Vec<&TowerLevel> = LEVELS
        .iter()
        .filter(|t| t.step == 0 && t.family.buildable())
        .collect();
    assert_eq!(roots.len(), 11, "the map's builder offers eleven towers");

    let cheapest = roots.iter().min_by_key(|t| t.gold).unwrap();
    assert_eq!(cheapest.gold, 10, "the seed tower costs ten gold");
    assert_eq!(cheapest.family, Family::Single);

    // And the six specialisations cannot be bought any other way.
    for f in SPECIALISATIONS {
        assert!(
            !f.buildable(),
            "{f:?} should only be reachable through Single shot"
        );
        assert!(
            LEVELS.iter().any(|t| t.family == f),
            "{f:?} has no levels at all"
        );
    }
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

    // And a wave does fly.
    assert!(WAVES.iter().any(|w| w.flying), "no wave flies");
}

/// The counter system, and the reason four families exist.
#[test]
fn divine_armour_is_answered_only_by_chaos_and_spells() {
    for a in [Attack::Normal, Attack::Siege, Attack::Magic] {
        assert_eq!(
            type_mult(a, ArmourType::Divine),
            0.05,
            "{a:?} should do 5% to Divine"
        );
    }
    for a in [Attack::Chaos, Attack::Spells] {
        assert_eq!(
            type_mult(a, ArmourType::Divine),
            1.0,
            "{a:?} should be unaffected by Divine"
        );
    }

    // Chaos is flat against everything - that is the whole point of it.
    for d in [
        ArmourType::Unarmoured,
        ArmourType::Light,
        ArmourType::Medium,
        ArmourType::Heavy,
        ArmourType::Fortified,
        ArmourType::Hero,
        ArmourType::Divine,
    ] {
        assert_eq!(
            type_mult(Attack::Chaos, d),
            1.0,
            "Chaos is resisted by {d:?}"
        );
    }

    // Somebody must actually be able to deal it.
    let chaos: Vec<&str> = LEVELS
        .iter()
        .filter(|t| t.attack == Attack::Chaos)
        .map(|t| t.family.name())
        .collect();
    assert!(
        !chaos.is_empty(),
        "nothing in the roster deals Chaos damage"
    );

    // And Divine has to turn up often enough to matter.
    let divine = WAVES
        .iter()
        .filter(|w| w.armour_type == ArmourType::Divine)
        .count();
    assert!(
        divine >= 6,
        "only {divine} Divine waves in the whole campaign"
    );
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
        WAVES.iter().any(|w| w.armour >= 200),
        "no wave is heavily armoured"
    );
}

#[test]
fn the_waves_are_the_map_s_waves() {
    assert_eq!(WAVES.len(), 36, "the campaign is thirty-six waves");
    assert_eq!(WAVES[0].name, "Troll");
    assert_eq!(WAVES[0].hp, 250.0);
    assert_eq!(WAVES[0].armour, 0);
    assert_eq!(WAVES[0].count, 66);

    // Health climbs by four orders of magnitude across the campaign, which is
    // what the six-figure end of the tower ladder is priced against.
    let last = WAVES.last().unwrap();
    assert!(last.hp >= 500_000.0, "the last wave has {} health", last.hp);
    assert!(
        last.hp / WAVES[0].hp > 1000.0,
        "the campaign only scales {}x",
        last.hp / WAVES[0].hp
    );

    // Waves are streams: counts in the dozens and hundreds, not single bosses.
    let big = WAVES.iter().filter(|w| w.count >= 60).count();
    assert!(big >= 10, "only {big} waves arrive in real numbers");
}

/// Prints the roster. Run it to see what was extracted:
///     cargo test --release show_the_extracted_roster -- --ignored --nocapture
#[test]
#[ignore = "prints the whole roster"]
fn show_the_extracted_roster() {
    println!();
    println!("GREEN CIRCLE TD - extracted roster");
    println!("{:-<96}", "");
    let mut fams: Vec<Family> = Vec::new();
    for t in LEVELS {
        if !fams.contains(&t.family) {
            fams.push(t.family);
        }
    }
    for f in fams {
        let rungs: Vec<&TowerLevel> = LEVELS.iter().filter(|t| t.family == f).collect();
        println!(
            "{:<16} {:>2} levels  {:<12} {:<14} {}",
            f.name(),
            rungs.len(),
            format!("{:?}", rungs[0].attack),
            rungs[0].targets.label(),
            if f.buildable() {
                "shop"
            } else {
                "from Single shot"
            }
        );
        for t in rungs {
            println!(
                "    {:<38} {:>7}g  dmg {:>8.0}  cd {:.2}  rng {:>5.1}  aoe {:>4.1}",
                t.name, t.gold, t.damage, t.cooldown, t.range, t.splash
            );
        }
    }
    println!();
    println!("WAVES");
    println!("{:-<96}", "");
    for w in WAVES {
        println!(
            "  {:>2}  {:<16} x{:<5} hp {:>8.0}  armour {:>3} {:<11} {}",
            w.wave,
            w.name,
            w.count,
            w.hp,
            w.armour,
            w.armour_type.name(),
            if w.flying { "FLYING" } else { "" }
        );
    }
}
