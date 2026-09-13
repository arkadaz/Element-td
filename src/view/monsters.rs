//! Monsters on the ring.
//!
//! The body itself comes from [`super::models`], which rebuilds the Warcraft
//! III unit the map dressed each wave in. What lives here is everything that is
//! about the *creep* rather than the model: the contact shadow, the status
//! glows, and the health bar.
//!
//! Colour carries armour type - the gold of an Immune wave, the violet of Hero
//! armour - so it can never carry identity as well. That is what the model is
//! for, and why a Bronze Dragon has to be recognisable as one at any zoom with
//! the colour taken out.

use super::models::{Pose, Skin};
use super::{models, theme};
use crate::game::{BOSS_MENDER_RANGE, Creep};
use crate::gfx::draw::{DrawList, Material, Shape, rgba};

/// Campaign's fixed whole-board camera needs a little more silhouette than the
/// extracted Legacy collision envelope supplied. This is render-only: route
/// spacing, targeting and saved physical radii remain untouched, while a live
/// C1 infantryman moves from a near-indistinguishable dash toward the brief's
/// 14--22px torso/head target. Legacy keeps its historical render scale.
const CAMPAIGN_RENDER_SCALE: f32 = 1.08;

/// Draws one monster.
///
/// `detail` says whether this instance may afford the fine parts of its model -
/// boots, buckles, wing membranes, an additive glow. A wave of a hundred and
/// fifty cannot: a hundred and fifty additive sprites on one stretch of road is
/// a white sheet, and a hundred and fifty belt buckles is instances spent on
/// something four pixels wide. A wave of fifteen can, and should.
pub fn draw(d: &mut DrawList, c: &Creep, detail: bool) {
    let skin = Skin::wearing(c.model, c.armour_type.color(), c.flash);
    let collision_r = c.radius;
    // A source body has one authored silhouette, but an army of 200 exact
    // clones reads like a spreadsheet even after its formation gains gaps.
    // The uid is stable through a save/resume, so this is a small individual
    // stature difference rather than a frame-to-frame pulse or an RNG-only
    // cosmetic that would make a restored battle visibly rearrange itself.
    let stature = 0.89 + (c.uid as f32 * 12.9898).sin().abs() * 0.19;
    let r = collision_r
        * if c.campaign_encounter > 0 {
            CAMPAIGN_RENDER_SCALE
        } else {
            0.92
        }
        * stature;
    let ground = if c.flying { c.height() - collision_r * 1.2 } else { 0.21 };

    // Contact shadow - a flat disc, so it reads as a shadow not a plate. A
    // flyer's is smaller and fainter, which is most of what tells you at a
    // glance that it is out of a Siege Tower's reach.
    let (spread, alpha) = if c.flying { (1.5, 0.16) } else { (2.4, 0.36) };
    d.shape(
        Shape::Quad,
        [c.pos[0], c.pos[1], 0.205],
        [r * spread, r * spread * 0.85, 1.0],
        0.0,
        0.0,
        [0.0, 0.0, 0.0, alpha],
        Material::EARTH,
        0.0,
    );

    // Vanguards are a gameplay rule, not a hidden multiplier. A single gold
    // crown above the silhouette stays readable in a horde; the former 24-piece
    // ground ring under every Vanguard became visual static on hard waves.
    if c.elite {
        elite_crown(d, c);
    }

    // A commander's repair field is a rule the player must be able to see.
    // It is deliberately a thin ground ring rather than another glow over the
    // model: escorts inside the radius are the thing the player is deciding
    // whether to suppress or separate from their leader.
    if c.is_boss() {
        d.ground_ring(
            c.pos,
            BOSS_MENDER_RANGE,
            0.042,
            rgba([1.0, 0.66, 0.20], 0.26),
            32,
        );
    }

    let pose = Pose {
        pos: c.pos,
        z: ground,
        // A tiny deterministic gait turn breaks the perfectly parallel source
        // mesh fence without lying about where a creature is travelling.  It
        // is especially important for shield-and-spear silhouettes in a dense
        // column, where exact yaw alignment otherwise becomes one blue stripe.
        yaw: c.facing
            + (c.bob * 2.17).sin() * 0.055
            + (c.uid as f32 * 5.177).sin() * 0.030,
        // Collision remains generous for targeting; the render footprint is
        // smaller so the authored high-count waves do not become one mesh.
        r,
        t: c.bob,
        walk: c.stun <= 0.0,
        lights: detail || c.is_boss() || c.elite,
    };
    // Preserve a tiny stable difference in dusty cloth/armour value across a
    // mass formation.  It is based only on the saved uid, never on frame time,
    // so a restored battle keeps the same individual horde character instead
    // of shimmer-recolouring while it walks.  Material ids still decide what
    // is iron, hide, cloth or chitin; this only stops exact clones reading as
    // a single blue/orange regiment at the full-board camera.
    let weather = (c.uid as f32 * 17.371).sin();
    let hue = (c.uid as f32 * 5.913).cos();
    let tint = match c.model {
        // The opening infantry keeps cold forged iron as its dominant part,
        // but dusty dark cloth and leather temper the former blue-primary
        // parade stripe. Bronze remains a tiny, readable local glint.
        crate::game::defs::Model::Warrior => [
            0.90 + weather * 0.055,
            0.79 + hue * 0.045,
            0.66 - weather * 0.045,
            1.0,
        ],
        // The shell runner is a dark earth-and-iron counterpoint, not a pale
        // grey clone. Its amber eyes remain the only bright local accent.
        crate::game::defs::Model::Brute => [
            0.74 + weather * 0.045,
            0.72 + hue * 0.040,
            0.57 - weather * 0.035,
            1.0,
        ],
        _ => [
            0.82 + weather * 0.055,
            0.84 + hue * 0.045,
            0.80 - weather * 0.040,
            1.0,
        ],
    };
    models::draw_with_tint(d, c.model, &pose, &skin, tint);

    status(d, c, detail);
    health_bar(d, c);
}

// ---------------------------------------------------------------- overlays

fn status(d: &mut DrawList, c: &Creep, detail: bool) {
    let r = c.radius;
    let bz = c.height();
    // A crowd is already communicating its pressure through movement and
    // density. Reserve per-creep flashes, particles, and glows for the few
    // units the player can act on individually, plus bosses and Vanguards.
    let featured = detail || c.is_boss() || c.elite;
    // The model already flashes under damage. This short, tight energy bloom
    // adds contact at the exact body position so even a fast bolt feels like it
    // struck something rather than merely disappearing. Crowds suppress the
    // extra facets; commanders and Vanguards always keep the reaction.
    if c.flash > 0.03 && featured {
        let hit = c.flash.clamp(0.0, 1.0);
        let col = c.armour_type.color();
        d.glow(
            [c.pos[0], c.pos[1], bz],
            r * (1.20 + hit * 0.95),
            2.4,
            rgba(
                [1.0, 0.88 + col[1] * 0.12, 0.72 + col[2] * 0.20],
                hit * 0.22,
            ),
        );
        let facets = if c.is_boss() { 2 } else { 1 };
        for i in 0..facets {
            let a = c.bob * 1.7 + i as f32 * std::f32::consts::TAU / facets as f32;
            d.sphere_lit(
                [
                    c.pos[0] + a.cos() * r * 0.82,
                    c.pos[1] + a.sin() * r * 0.82,
                    bz + (a * 1.3).sin() * r * 0.42,
                ],
                r * 0.10 * hit,
                rgba([1.0, 0.93, 0.76], hit),
                1.0,
            );
        }
    }
    // At most one glow per featured monster. A wave is a hundred and fifty
    // creeps, and a hundred and fifty additive sprites on top of each other is
    // a white sheet rather than a status effect.
    let tint = if c.burn.t > 0.0 {
        Some([1.0, 0.45, 0.12])
    } else if c.poison.t > 0.0 {
        Some([0.45, 1.0, 0.35])
    } else if c.slow.t > 0.0 {
        Some([0.45, 0.80, 1.0])
    } else {
        None
    };
    if let Some(col) = tint.filter(|_| featured) {
        d.glow([c.pos[0], c.pos[1], bz], r * 1.55, 0.6, rgba(col, 0.10));
    }
    if featured {
        if c.burn.t > 0.0 {
            // Two little tongues crawl up the silhouette instead of painting
            // the entire creature orange.
            for i in 0..2 {
                let a = c.bob * (2.1 + i as f32 * 0.3) + i as f32 * 2.7;
                let h = r * (0.42 + 0.20 * (a * 1.7).sin().abs());
                d.shape(
                    Shape::Cone,
                    [
                        c.pos[0] + a.cos() * r * 0.55,
                        c.pos[1] + a.sin() * r * 0.55,
                        bz + a.sin() * r * 0.28,
                    ],
                    [r * 0.16, r * 0.16, h],
                    a,
                    0.0,
                    rgba([1.0, 0.38 + i as f32 * 0.16, 0.06], 0.92),
                    Material::GEM,
                    0.92,
                );
            }
        } else if c.poison.t > 0.0 {
            for i in 0..2 {
                let a = c.bob * 1.3 + i as f32 * 3.1;
                d.sphere_lit(
                    [
                        c.pos[0] + a.cos() * r * 0.62,
                        c.pos[1] + a.sin() * r * 0.62,
                        bz - r * (0.15 + 0.32 * a.sin().abs()),
                    ],
                    r * 0.12,
                    rgba([0.48, 1.0, 0.24], 0.86),
                    0.78,
                );
            }
        } else if c.slow.t > 0.0 {
            for i in 0..3 {
                let a = c.bob * 0.55 + i as f32 * 2.094;
                d.shape(
                    Shape::Prism,
                    [
                        c.pos[0] + a.cos() * r * 0.76,
                        c.pos[1] + a.sin() * r * 0.76,
                        0.25 + r * 0.18,
                    ],
                    [r * 0.13, r * 0.13, r * 0.48],
                    a,
                    -0.28,
                    rgba([0.44, 0.82, 1.0], 0.78),
                    Material::GEM,
                    0.58,
                );
            }
        }
    }
    if c.stun > 0.0 && featured {
        // Roots: a ring of sparks spinning overhead.
        for i in 0..3 {
            let a = c.bob * 3.0 + i as f32 * 2.094;
            d.sphere_lit(
                [
                    c.pos[0] + a.cos() * r * 0.7,
                    c.pos[1] + a.sin() * r * 0.7,
                    bz + r * 2.0,
                ],
                r * 0.22,
                rgba([1.0, 1.0, 0.85], 1.0),
                1.0,
            );
        }
    }
}

/// One compact marker is enough to identify a Vanguard in a crowded lane.
/// It is deliberately above the body rather than on the road, so it neither
/// competes with a selected tower's range indicator nor adds another circle to
/// the combat floor.
fn elite_crown(d: &mut DrawList, c: &Creep) {
    let r = c.radius;
    d.pyramid(
        [c.pos[0], c.pos[1], c.height() + r * 0.92],
        (r * 0.46).clamp(0.16, 0.30),
        (r * 0.30).clamp(0.10, 0.20),
        c.bob * 0.35,
        rgba([1.0, 0.69, 0.22], 0.94),
        Material::GEM,
    );
}

/// A health bar, on the few monsters it tells you something about.
///
/// Warcraft III does not float a bar over every unit on the field, and the
/// reason is visible the moment you try it: at wave thirteen this lane carries
/// three hundred and thirty-eight monsters, nearly all of them damaged, and
/// three hundred bars is a green picket fence with the game behind it. Removing
/// them entirely was the single largest improvement to the picture in this
/// whole pass.
///
/// So the bar earns its place. A damaged boss has one, because a boss is the
/// one monster a player tracks individually. Everything else gets one only
/// once it is under half, which is the point the number changes a decision -
/// whether to let it round again or spend on another tower - and by then only a
/// handful of the wave qualifies at any instant.
fn health_bar(d: &mut DrawList, c: &Creep) {
    let hp = c.hp_frac();
    if hp >= 0.999 || (hp > 0.5 && !c.is_boss()) {
        return;
    }
    let r = c.radius;
    let w = (r * 1.7).max(0.34);
    let bar_z = c.height() + r * 1.8 + 0.22;
    // Flat quads, so bars never catch a specular highlight and shimmer.
    d.shape(
        Shape::Quad,
        [c.pos[0], c.pos[1], bar_z],
        [w + 0.04, 0.085, 1.0],
        0.0,
        0.0,
        theme::HP_BACK,
        Material::EARTH,
        0.0,
    );
    let fill = if hp > 0.35 {
        theme::HP_FILL
    } else {
        theme::HP_LOW
    };
    d.shape(
        Shape::Quad,
        [c.pos[0] - w * 0.5 * (1.0 - hp), c.pos[1], bar_z + 0.015],
        [w * hp, 0.075, 1.0],
        0.0,
        0.0,
        fill,
        Material::EARTH,
        0.30,
    );
}
