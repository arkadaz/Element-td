//! Towers on their pads.
//!
//! A hundred and thirty-one towers is far too many to hand-sculpt, and the map
//! does not ask anyone to: it dresses each one in a stock Warcraft III model,
//! and reuses those freely. So the body comes from [`super::models`] - the same
//! library the monsters draw through - and what lives here is everything that
//! is about the *tower* rather than the model:
//!
//!   - the **plinth** it stands on, cut to its attack type,
//!   - the **level ring**: one notch of light per step up its family's path, so
//!     investment reads from across the board without any text, and
//!   - the range ring and glow when it is selected.
//!
//! Colour is the tower's attack type throughout, which is the one thing a
//! player has to read at a glance: on an Immune wave, everything that is not
//! Chaos red or Hero gold is doing five percent.

use super::PLOT_TOP;
use super::models::{Pose, Skin};
use super::{models, theme};
use crate::game::Tower;
use crate::game::defs::*;
use crate::gfx::draw::{DrawList, Material, Shape, mix, rgba};

/// Top of the stone plinth every tower stands on.
const DECK: f32 = PLOT_TOP + 0.20;

pub fn draw(d: &mut DrawList, tw: &Tower, selected: bool, now: f32) {
    let def = tw.def();
    let base = def.color();
    let grow = (((now - tw.built_at) * 4.0).min(1.0)).max(0.06);
    let skin = Skin::wearing(def.model, base, tw.flash * 0.5);

    plinth(d, tw, base);
    level_ring(d, tw, base);

    let pose = Pose {
        pos: tw.pos,
        z: DECK,
        yaw: tw.angle,
        // Bounded by the pad it stands on. Every model is built as a multiple
        // of this, and the widest of them - the ship, the turtle, the arcane
        // observatory - reach about two and a half times it, so a maxed tower
        // fills its pad and no more. It still visibly grows as it climbs.
        r: 0.40 * (0.72 + 0.28 * tw.progress()) * grow,
        t: now * 2.0 + tw.pos[0] * 1.7 + tw.pos[1] * 0.9,
        // A tower does not walk, but its parts still breathe.
        walk: false,
        lights: true,
    };
    models::draw(d, def.model, &pose, &skin);

    // A tower in a frenzy is visibly working - the Troll Tower's own ability.
    if tw.ramp > 0.0 {
        d.glow(
            [tw.pos[0], tw.pos[1], DECK + 0.5],
            1.5,
            1.1,
            rgba([1.0, 0.55, 0.25], 0.35),
        );
    }
    // A Fire Tower's immolation is the one aura with a permanent mark on the
    // ground, because it is the only one that is *damaging* everything inside
    // it - a slow cloud and a damage aura are shown when the tower is selected.
    let a = def.abil;
    if a.burn_dps > 0.0 && a.burn_range > 0.0 {
        d.ground_ring(
            tw.pos,
            a.burn_range,
            0.05,
            rgba([1.0, 0.48, 0.16], 0.12),
            40,
        );
    }

    if selected {
        d.ground_ring(tw.pos, tw.range(), 0.11, rgba(base, 0.85), 80);
        d.glow([tw.pos[0], tw.pos[1], 0.55], 1.2, 2.2, rgba(base, 0.16));
        // What its aura actually covers, in the colour of what it does.
        if a.is_aura() {
            d.ground_ring(
                tw.pos,
                a.aura_range,
                0.07,
                rgba([0.95, 0.86, 0.45], 0.5),
                64,
            );
        }
        if a.slow_amt > 0.0 && a.slow_range > 0.0 {
            d.ground_ring(tw.pos, a.slow_range, 0.07, rgba([0.45, 0.80, 1.0], 0.5), 56);
        }
    }
}

// ---------------------------------------------------------------- the plinth

/// A turned stone footing, dressed by attack type.
///
/// The dressing is the one place the roster's five attack types get a shape of
/// their own: a Chaos tower stands on thorns, a Siege tower on megaliths, a
/// Magic tower on fluted columns. It is small, but it is what makes a board of
/// mixed towers legible from directly overhead.
fn plinth(d: &mut DrawList, tw: &Tower, base: [f32; 3]) {
    let p = tw.pos;
    let dark = mix(theme::STONE_DARK, base, 0.12);
    d.slab_mat(
        p,
        [0.86, 0.86],
        PLOT_TOP + 0.04,
        0.16,
        rgba(dark, 1.0),
        Material::STONE,
    );
    d.cylinder(
        [p[0], p[1], PLOT_TOP + 0.12],
        0.76,
        0.16,
        0.0,
        rgba(theme::STONE, 1.0),
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
                    rgba(mix([0.72, 0.70, 0.64], base, 0.30), 1.0),
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
                    rgba(mix(theme::STONE, base, 0.30), 1.0),
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
                    rgba(mix([0.10, 0.09, 0.14], base, 0.40), 1.0),
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
                rgba(mix([0.10, 0.08, 0.09], base, 0.12), 1.0),
                Material::STONE,
            );
            for k in 0..4 {
                let a = k as f32 * 1.571 + 0.4;
                d.cylinder(
                    [p[0] + a.cos() * 0.30, p[1] + a.sin() * 0.30, DECK - 0.06],
                    0.11,
                    0.10,
                    0.0,
                    rgba(boost3(base, 1.5), 1.0),
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

    d.cylinder(
        [p[0], p[1], DECK - 0.02],
        0.62,
        0.10,
        0.0,
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
fn level_ring(d: &mut DrawList, tw: &Tower, base: [f32; 3]) {
    let total = tw.ladder_len().max(1);
    let done = tw.level().min(total);
    let p = tw.pos;
    let n = total.min(20);
    let lit = ((done as f32 / total as f32) * n as f32).round() as u32;
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let q = [p[0] + a.cos() * 0.56, p[1] + a.sin() * 0.56];
        let on = k < lit;
        d.shape(
            Shape::Quad,
            [q[0], q[1], DECK + 0.02],
            [0.062, 0.062, 1.0],
            a,
            0.0,
            if on {
                rgba(base, 1.0)
            } else {
                rgba(mix(theme::STONE_DARK, [0.0, 0.0, 0.0], 0.35), 1.0)
            },
            Material::METAL,
            if on { 0.25 } else { 0.0 },
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
    models::draw(d, def.model, &pose, &skin);
}
