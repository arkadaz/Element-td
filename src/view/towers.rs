//! Towers on their pads.
//!
//! The reference roster has a hundred and thirty-one statistical rungs. Drawing
//! one mesh for every rung made upgrades look duplicated; drawing 131 unrelated
//! meshes would destroy family recognition. Each command family now has four
//! staged CC0 weapon turrets (foundation, fortified, empowered, apex), with moving
//! branch-specific silhouettes layered here:
//!
//!   - the **plinth** it stands on, cut to its attack type,
//!   - the **level ring**: one notch of light per step up its family's path, so
//!     investment reads from across the board without any text, and
//!   - its own authored weapon construction, stone footings, timber, iron,
//!     bronze fittings and sparse amber insets, and
//!   - one restrained range ring when it is selected.
//!
//! Gameplay type remains available in the compact command card and threat
//! readout. The battlefield no longer recolours an entire physical assembly by
//! attack type: that made materially authored towers read like plastic toys.

use super::PLOT_TOP;
use super::models::{Pose, Skin};
use super::{models, theme};
use crate::game::{Tower, TOWER_RENDER_FOOTPRINT_SCALE, TOWER_RENDER_HEIGHT_SCALE};
use crate::game::defs::*;
use crate::gfx::draw::{DrawList, Material, Shape, mix, rgba};

/// Top of the stone plinth every tower stands on.
const DECK: f32 = PLOT_TOP + 0.20;

/// Quaternius' OBJ turrets point along source +Z. The bake converts that to
/// local -Y, while the simulation defines yaw zero as aiming along world +X.
/// Rotating the imported body a quarter turn makes its barrel agree with
/// `Tower::angle`, projectile launch effects, and procedural family dressing.
const DOWNLOADED_TURRET_YAW: f32 = std::f32::consts::FRAC_PI_2;

#[inline]
fn downloaded_turret_yaw(aim_yaw: f32) -> f32 {
    aim_yaw + DOWNLOADED_TURRET_YAW
}

pub fn draw(d: &mut DrawList, tw: &Tower, selected: bool, now: f32) {
    let def = tw.def();
    let base = def.color();
    // A build click must produce a complete, useful silhouette immediately.
    // `now` is simulation time, so the former construction lerp froze a newly
    // purchased tower at six percent scale whenever a player paused to place
    // or inspect it (and made the effect nearly invisible at the requested
    // 100x tempo).  The spawn burst and the short attack cooldown still give
    // construction tactile feedback; model visibility must not depend on the
    // simulation running.
    let grow = 1.0;
    let skin = Skin::wearing(def.model, base, tw.flash * 0.5);
    let stage = tower_visual_stage(def);

    plinth(d, tw);
    level_ring(d, tw);

    let pose = Pose {
        pos: tw.pos,
        z: DECK,
        yaw: tw.angle,
        // Bounded by the pad it stands on. Every model is built as a multiple
        // of this, and the widest of them - the ship, the turtle, the arcane
        // observatory - reach about two and a half times it, so a maxed tower
        // fills its pad and no more. It still visibly grows as it climbs.
        r: 0.46 * (0.72 + 0.28 * tw.progress()) * grow,
        t: now * 2.0 + tw.pos[0] * 1.7 + tw.pos[1] * 0.9,
        // A tower does not walk, but its parts still breathe.
        walk: false,
        lights: true,
    };
    let model_scale = tw.visual_model_scale() * grow;
    let model_envelope = [
        model_scale * TOWER_RENDER_FOOTPRINT_SCALE,
        model_scale * TOWER_RENDER_FOOTPRINT_SCALE,
        model_scale * TOWER_RENDER_HEIGHT_SCALE,
    ];
    // Source vertex colours and material ids carry the construction. A white
    // instance tint preserves limestone, oak, black iron and worn bronze
    // rather than painting the entire assembled mesh with an attack colour.
    let tint = rgba([1.0, 1.0, 1.0], 1.0);
    if !models::draw_downloaded_animated_scaled(
        d,
        family_asset(tw.family(), stage),
        [tw.pos[0], tw.pos[1], DECK],
        model_envelope,
        downloaded_turret_yaw(tw.angle),
        tint,
        Material::METAL,
        0.0,
        tw.flash.clamp(0.0, 1.0),
    ) {
        models::draw(d, def.model, &pose, &skin);
    }
    // The source meshes already provide eleven different readable silhouettes.
    // Do not cover them with generic coloured orbitals and cones: the old
    // dressing was the visual reason every command icon looked like the same
    // bright toy on a differently coloured base. At high construction stages,
    // one small amber sight is enough localized fantasy light.
    if stage >= 2 {
        d.sphere_lit(
            [
                tw.pos[0],
                tw.pos[1],
                DECK + model_scale * TOWER_RENDER_HEIGHT_SCALE * 0.96,
            ],
            0.040 + tw.progress() * 0.018,
            rgba([0.95, 0.30, 0.045], 0.88),
            0.10,
        );
    }

    // A tower in a frenzy is visibly working - the Troll Tower's own ability.
    // Keep this close to the plinth: a full board may have several frantic
    // towers, and broad additive pools erase the units fighting between them.
    if tw.ramp > 0.0 {
        d.glow(
            [tw.pos[0], tw.pos[1], DECK + 0.5],
            1.10,
            0.9,
            rgba([1.0, 0.55, 0.25], 0.18),
        );
    }

    // The board has one visual language for reach: select a tower and see its
    // attack radius.  Aura, slow, and fire radii are intentionally not drawn
    // as extra circles; stacked rings made a busy fight unreadable and did not
    // change where the player can click or what the tower does.
    if selected {
        d.ground_ring(
            tw.pos,
            tw.range(),
            // Selection feedback has to remain visible on a dark field, but
            // it cannot read as a cyan territory overlay while the player is
            // trying to inspect the actual terrain and moving army.
            0.006,
            rgba(family_accent(tw.family()), 0.030),
            64,
        );
    }
}

// ---------------------------------------------------------------- the plinth

/// A turned stone footing, dressed by attack type.
///
/// The dressing is the one place the roster's five attack types get a shape of
/// their own: a Chaos tower stands on thorns, a Siege tower on megaliths, a
/// Magic tower on fluted columns. It is small, but it is what makes a board of
/// mixed towers legible from directly overhead.
fn plinth(d: &mut DrawList, tw: &Tower) {
    let p = tw.pos;
    let dark = theme::STONE_DARK;
    // The former two smooth cylinders stayed from the primitive-only tower
    // renderer after the authored models gained cut-stone foundations.  They
    // read as pale hockey pucks beneath every otherwise constructed tower.
    // Keep a shared, deliberately octagonal lower footing that agrees with
    // the source mesh rather than competing with it.
    d.slab_mat(
        p,
        [0.96, 0.96],
        PLOT_TOP + 0.04,
        0.13,
        rgba(dark, 1.0),
        Material::STONE,
    );
    d.prism(
        [p[0], p[1], PLOT_TOP + 0.135],
        0.82,
        0.15,
        std::f32::consts::FRAC_PI_4,
        rgba([0.070, 0.080, 0.068], 1.0),
        Material::STONE,
    );
    d.prism(
        [p[0], p[1], PLOT_TOP + 0.225],
        0.64,
        0.075,
        std::f32::consts::FRAC_PI_4,
        rgba([0.105, 0.118, 0.098], 1.0),
        Material::STONE,
    );

    match tw.attack() {
        // Fluted columns, pale and even.
        Attack::Magic => {
            for k in 0..6 {
                let a = k as f32 * 1.047;
                d.cylinder(
                    [p[0] + a.cos() * 0.33, p[1] + a.sin() * 0.33, DECK - 0.03],
                    0.11,
                    0.24,
                    0.0,
                    rgba([0.30, 0.285, 0.235], 1.0),
                    Material::STONE,
                );
            }
        }
        // Three rough megaliths carrying the weight.
        Attack::Siege => {
            for k in 0..3 {
                let a = k as f32 * 2.094 + 0.6;
                d.prism(
                    [p[0] + a.cos() * 0.30, p[1] + a.sin() * 0.30, DECK - 0.05],
                    0.30,
                    0.22,
                    a,
                    rgba([0.29, 0.265, 0.215], 1.0),
                    Material::STONE,
                );
            }
        }
        // A collar of thorns leaning outward.
        Attack::Chaos => {
            for k in 0..8 {
                let a = k as f32 * 0.785;
                d.cone(
                    [p[0] + a.cos() * 0.32, p[1] + a.sin() * 0.32, DECK - 0.06],
                    0.13,
                    0.22,
                    0.0,
                    rgba([0.075, 0.070, 0.080], 1.0),
                    Material::DARK_METAL,
                );
            }
        }
        // A stepped hearth with vents glowing between the courses.
        Attack::Spells | Attack::Hero => {
            d.prism(
                [p[0], p[1], DECK - 0.06],
                0.70,
                0.18,
                0.4,
                rgba([0.095, 0.082, 0.072], 1.0),
                Material::STONE,
            );
            for k in 0..4 {
                let a = k as f32 * 1.571 + 0.4;
                d.cylinder(
                    [p[0] + a.cos() * 0.30, p[1] + a.sin() * 0.30, DECK - 0.06],
                    0.11,
                    0.10,
                    0.0,
                    rgba([0.34, 0.19, 0.055], 1.0),
                    Material::METAL,
                );
            }
        }
        // Plain courses of stone.
        _ => {
            d.cylinder(
                [p[0], p[1], DECK - 0.08],
                0.70,
                0.12,
                0.0,
                rgba(dark, 1.0),
                Material::STONE,
            );
        }
    }

    d.prism(
        [p[0], p[1], DECK - 0.02],
        0.58,
        0.08,
        std::f32::consts::FRAC_PI_4,
        rgba(dark, 1.0),
        Material::STONE,
    );
}

/// One lit notch per step up the family's path.
///
/// A Siege Tower has twenty levels and a Poison Tower fifteen, so a number
/// would be unreadable at board zoom. Notches around the deck are not: an
/// almost-complete ring says "nearly maxed" from anywhere on the screen, and a
/// single mark says "ten gold seed".
fn level_ring(d: &mut DrawList, tw: &Tower) {
    let total = tw.ladder_len().max(1);
    let done = tw.level().min(total);
    let p = tw.pos;
    let n = total.min(12);
    let lit = ((done as f32 / total as f32) * n as f32).round() as u32;
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let q = [p[0] + a.cos() * 0.56, p[1] + a.sin() * 0.56];
        let on = k < lit;
        d.shape(
            Shape::Quad,
            [q[0], q[1], DECK + 0.02],
            [0.046, 0.046, 1.0],
            a,
            0.0,
            if on {
                // Upgrade investment remains readable from the overhead view,
                // but as forged bronze marks rather than a saturated faction
                // halo around every tower.
                rgba([0.33, 0.17, 0.045], 1.0)
            } else {
                rgba(mix(theme::STONE_DARK, [0.0, 0.0, 0.0], 0.35), 1.0)
            },
            Material::METAL,
            if on { 0.08 } else { 0.0 },
        );
    }
}

fn boost3(c: [f32; 3], k: f32) -> [f32; 3] {
    [
        (c[0] * k).min(1.0),
        (c[1] * k).min(1.0),
        (c[2] * k).min(1.0),
    ]
}

/// Four construction milestones per command-card identity. Statistical rungs
/// inside a milestone retain continuity; crossing a milestone replaces the
/// whole downloaded assembly, so an upgrade is visible without selecting it.
fn family_asset(family: Family, stage: usize) -> &'static str {
    use Family::*;
    const SEED: [&str; 4] = ["TowerSeed0", "TowerSeed1", "TowerSeed2", "TowerSeed3"];
    const SIEGE: [&str; 4] = ["TowerSiege0", "TowerSiege1", "TowerSiege2", "TowerSiege3"];
    const BOUNCE: [&str; 4] = [
        "TowerBounce0",
        "TowerBounce1",
        "TowerBounce2",
        "TowerBounce3",
    ];
    const MULTI: [&str; 4] = ["TowerMulti0", "TowerMulti1", "TowerMulti2", "TowerMulti3"];
    const CORRUPT: [&str; 4] = [
        "TowerCorrupt0",
        "TowerCorrupt1",
        "TowerCorrupt2",
        "TowerCorrupt3",
    ];
    const AIR: [&str; 4] = ["TowerAir0", "TowerAir1", "TowerAir2", "TowerAir3"];
    const CHAOS: [&str; 4] = ["TowerChaos0", "TowerChaos1", "TowerChaos2", "TowerChaos3"];
    const DESTROY: [&str; 4] = [
        "TowerDestroy0",
        "TowerDestroy1",
        "TowerDestroy2",
        "TowerDestroy3",
    ];
    const AURA: [&str; 4] = ["TowerAura0", "TowerAura1", "TowerAura2", "TowerAura3"];
    const DEMON: [&str; 4] = ["TowerDemon0", "TowerDemon1", "TowerDemon2", "TowerDemon3"];
    const KING: [&str; 4] = ["TowerKing0", "TowerKing1", "TowerKing2", "TowerKing3"];

    let stages = match family {
        Single => &SEED,
        Siege => &SIEGE,
        Bouncing | SuperBounce => &BOUNCE,
        Multi | Critical | SuperMulti => &MULTI,
        Corruption | Poison => &CORRUPT,
        Air | Frost | Slow => &AIR,
        Chaos | SuperChaos => &CHAOS,
        Destruction | SuperDestruct | Fire => &DESTROY,
        Aura | Damage | Speed => &AURA,
        Demon | Troll => &DEMON,
        King | OneStrike => &KING,
    };
    stages[stage.min(3)]
}

fn local(p: [f32; 2], yaw: f32, x: f32, y: f32, z: f32) -> [f32; 3] {
    let (s, c) = yaw.sin_cos();
    [p[0] + x * c - y * s, p[1] + x * s + y * c, z]
}

/// Branch identity and moving parts layered onto the staged CC0 body. These
/// are silhouettes, not decoration: orbiting shot for Bounce, a fan of bolts
/// for Multi, antenna for Air, furnace for Destruction, crown for King.
fn family_dressing(d: &mut DrawList, tw: &Tower, stage: usize, scale: f32, now: f32, grow: f32) {
    use Family::*;
    let p = tw.pos;
    let yaw = tw.angle;
    let z = DECK + scale * 0.86;
    let col = family_accent(tw.family());
    let lit = rgba(col, grow);
    let dark = rgba(mix(col, [0.05, 0.06, 0.07], 0.58), grow);
    let n = (stage + 1) as u32;

    match tw.family() {
        Single => {
            for k in 0..n.min(3) {
                let a = now * 0.7 + k as f32 * std::f32::consts::TAU / n.min(3) as f32;
                d.sphere_lit(
                    [p[0] + a.cos() * 0.24, p[1] + a.sin() * 0.24, z],
                    0.09,
                    lit,
                    0.32,
                );
            }
        }
        Siege => {
            for side in [-1.0f32, 1.0] {
                let q = local(p, yaw, -0.26, side * 0.34, DECK + 0.30);
                d.sphere_lit(q, 0.13 + stage as f32 * 0.015, dark, 0.0);
            }
        }
        Bouncing | SuperBounce => {
            for k in 0..(n + 1).min(4) {
                let a = now * (1.3 + stage as f32 * 0.12)
                    + k as f32 * std::f32::consts::TAU / (n + 1).min(4) as f32;
                d.sphere_lit(
                    [
                        p[0] + a.cos() * 0.34,
                        p[1] + a.sin() * 0.34,
                        z + a.sin() * 0.05,
                    ],
                    0.10,
                    lit,
                    0.26,
                );
            }
        }
        Multi | SuperMulti | Critical => {
            let bolts = (2 + stage).min(5);
            for k in 0..bolts {
                let spread = (k as f32 - (bolts - 1) as f32 * 0.5) * 0.12;
                let a = local(p, yaw, 0.05, spread, z - 0.03);
                let b = local(p, yaw, 0.46, spread * 1.35, z + 0.05);
                d.link(Shape::Taper, a, b, 0.035, lit, Material::METAL, 0.12);
            }
        }
        Corruption | Poison => {
            let acid = if tw.family() == Poison {
                [0.35, 1.0, 0.12]
            } else {
                col
            };
            for k in 0..n.min(3) {
                let a = k as f32 * 2.094 + 0.5;
                d.sphere_lit(
                    [
                        p[0] + a.cos() * 0.29,
                        p[1] + a.sin() * 0.29,
                        z - 0.06 + k as f32 * 0.04,
                    ],
                    0.10 + 0.015 * stage as f32,
                    rgba(acid, grow),
                    0.28,
                );
            }
        }
        Air | Frost | Slow => {
            let turn = now * if tw.family() == Frost { 0.45 } else { 1.1 };
            for arm in 0..2 {
                let a = turn + arm as f32 * std::f32::consts::FRAC_PI_2;
                let from = [p[0] - a.cos() * 0.30, p[1] - a.sin() * 0.30, z];
                let to = [p[0] + a.cos() * 0.30, p[1] + a.sin() * 0.30, z];
                d.link(Shape::Taper, from, to, 0.035, lit, Material::METAL, 0.18);
            }
            d.sphere_lit([p[0], p[1], z + 0.08], 0.095, lit, 0.35);
        }
        Chaos | SuperChaos => {
            for k in 0..(3 + stage).min(6) {
                let a = k as f32 * std::f32::consts::TAU / (3 + stage).min(6) as f32;
                d.cone(
                    [p[0] + a.cos() * 0.35, p[1] + a.sin() * 0.35, DECK + 0.29],
                    0.09,
                    0.32 + stage as f32 * 0.04,
                    a,
                    dark,
                    Material::DARK_METAL,
                );
            }
        }
        Destruction | SuperDestruct | Fire => {
            let flame = if tw.family() == Fire {
                [1.0, 0.62, 0.10]
            } else {
                col
            };
            d.sphere_lit(
                [p[0], p[1], z - 0.08],
                0.16 + stage as f32 * 0.025,
                rgba(flame, grow),
                0.46,
            );
            d.cone(
                [p[0], p[1], z + 0.13 + (now * 4.0).sin() * 0.025],
                0.15,
                0.34 + stage as f32 * 0.05,
                now,
                rgba(flame, grow),
                Material::GEM,
            );
        }
        Aura | Damage | Speed => {
            let orbit = match tw.family() {
                Damage => [1.0, 0.36, 0.18],
                Speed => [0.22, 0.88, 1.0],
                _ => col,
            };
            for k in 0..(2 + stage).min(5) {
                let a = -now * 0.75 + k as f32 * std::f32::consts::TAU / (2 + stage).min(5) as f32;
                d.prism(
                    [
                        p[0] + a.cos() * 0.34,
                        p[1] + a.sin() * 0.34,
                        z + a.cos() * 0.05,
                    ],
                    0.10,
                    0.20,
                    a,
                    rgba(orbit, grow),
                    Material::GEM,
                );
            }
        }
        Demon | Troll => {
            for side in [-1.0f32, 1.0] {
                let base = local(p, yaw, 0.0, side * 0.25, z - 0.05);
                let tip = local(p, yaw, 0.15, side * (0.42 + stage as f32 * 0.04), z + 0.24);
                d.link(
                    Shape::Cone,
                    base,
                    tip,
                    0.08,
                    dark,
                    Material::DARK_METAL,
                    0.08,
                );
            }
        }
        King | OneStrike => {
            for k in 0..5 {
                let a = k as f32 * std::f32::consts::TAU / 5.0;
                let q = [p[0] + a.cos() * 0.25, p[1] + a.sin() * 0.25, z + 0.10];
                d.cone(
                    q,
                    0.07,
                    0.25 + stage as f32 * 0.035,
                    a,
                    lit,
                    Material::METAL,
                );
            }
            d.sphere_lit([p[0], p[1], z + 0.22], 0.10, lit, 0.42);
        }
    }
}

fn family_accent(family: Family) -> [f32; 3] {
    family.fx_color()
}

/// A translucent preview of what is about to be built, turning on the spot.
pub fn draw_ghost(d: &mut DrawList, def_i: usize, p: [f32; 2], now: f32) {
    let Some(def) = TOWERS.get(def_i) else { return };
    let skin = Skin::wearing(def.model, def.color(), 0.0);
    let pose = Pose {
        pos: p,
        z: DECK,
        yaw: now * 0.6,
        r: 0.31,
        t: now * 2.0,
        walk: false,
        lights: true,
    };
    if !models::draw_downloaded_animated_scaled(
        d,
        family_asset(def.family, 0),
        [p[0], p[1], DECK],
        // Match the opening source assembly's playable scale. A placement
        // ghost is a real model preview, not a tiny icon stamped on grass.
        [
            1.12 * TOWER_RENDER_FOOTPRINT_SCALE,
            1.12 * TOWER_RENDER_FOOTPRINT_SCALE,
            1.12 * TOWER_RENDER_HEIGHT_SCALE,
        ],
        downloaded_turret_yaw(now * 0.6),
        rgba(def.color(), 0.55),
        Material::STONE,
        0.0,
        0.0,
    ) {
        models::draw(d, def.model, &pose, &skin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::gfx::mesh::{model_bucket, model_slots};

    #[test]
    fn imported_turret_front_matches_the_simulation_aim_vector() {
        for aim in [
            0.0,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            -std::f32::consts::FRAC_PI_2,
            0.73,
        ] {
            // The imported mesh's authored front is local -Y.
            let yaw = downloaded_turret_yaw(aim);
            let transformed_front = [yaw.sin(), -yaw.cos()];
            let expected = [aim.cos(), aim.sin()];
            assert!((transformed_front[0] - expected[0]).abs() < 1e-6);
            assert!((transformed_front[1] - expected[1]).abs() < 1e-6);
        }
    }

    #[test]
    fn tower_draw_applies_the_imported_asset_axis_correction() {
        let mut game = Game::new();
        game.gold = 10_000;
        let def = family_start(Family::Single).expect("single tower root");
        game.build_choice = Some((def, 1));
        assert!(game.try_build(0));
        game.towers[0].angle = 0.73;
        game.towers[0].built_at = -10.0;

        let mut list = DrawList::default();
        draw(&mut list, &game.towers[0], false, 1.0);
        let slot = model_slots()
            .iter()
            .find(|(name, _)| name == "TowerSeed0")
            .map(|(_, slot)| *slot)
            .expect("TowerSeed0 model");
        let instances = &list.solid[model_bucket(slot)];
        assert_eq!(instances.len(), 1);
        assert!(
            (instances[0].rot[0] - downloaded_turret_yaw(0.73)).abs() < 1e-6,
            "draw bypassed the imported turret axis correction"
        );
        assert!(
            instances[0].scale[2] > instances[0].scale[0],
            "the authored tower must retain a tall 3D construction envelope at overview scale"
        );
        let envelope_ratio = instances[0].scale[2] / instances[0].scale[0];
        let expected_ratio = TOWER_RENDER_HEIGHT_SCALE / TOWER_RENDER_FOOTPRINT_SCALE;
        assert!(
            (envelope_ratio - expected_ratio).abs() < 1e-5,
            "tower rendering and its gameplay muzzle profile have diverged"
        );
    }

    #[test]
    fn a_paused_new_build_is_a_complete_3d_tower_not_a_tiny_plinth() {
        let mut game = Game::new();
        game.gold = 10_000;
        let def = family_start(Family::Single).expect("single tower root");
        game.build_choice = Some((def, 1));
        assert!(game.try_build(0));
        // A freshly placed tower has `built_at == game.time`; this mirrors a
        // player opening the pause menu immediately after a placement click.
        let mut list = DrawList::default();
        draw(&mut list, &game.towers[0], false, game.time);
        let slot = model_slots()
            .iter()
            .find(|(name, _)| name == "TowerSeed0")
            .map(|(_, slot)| *slot)
            .expect("TowerSeed0 model");
        let instance = &list.solid[model_bucket(slot)][0];
        assert!(
            instance.scale[0] > 1.0 && instance.scale[2] > 1.3,
            "a paused fresh placement regressed to the old 6% construction model"
        );
    }
}
