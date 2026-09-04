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
use crate::game::Creep;
use crate::gfx::draw::{DrawList, Material, Shape, rgba};

/// Draws one monster.
///
/// `detail` says whether this instance may afford the fine parts of its model -
/// boots, buckles, wing membranes, an additive glow. A wave of a hundred and
/// fifty cannot: a hundred and fifty additive sprites on one stretch of road is
/// a white sheet, and a hundred and fifty belt buckles is instances spent on
/// something four pixels wide. A wave of fifteen can, and should.
pub fn draw(d: &mut DrawList, c: &Creep, detail: bool) {
    let skin = Skin::wearing(c.model, c.armour_type.color(), c.flash);
    let r = c.radius;
    let ground = if c.flying { c.height() - r * 1.2 } else { 0.21 };

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

    let pose = Pose {
        pos: c.pos,
        z: ground,
        yaw: c.facing,
        // Drawn a little larger than it collides, so a silhouette survives
        // being forty pixels tall.
        r: r * 1.2,
        t: c.bob,
        walk: c.stun <= 0.0,
        lights: detail || c.is_boss(),
    };
    models::draw(d, c.model, &pose, &skin);

    status(d, c);
    health_bar(d, c);
}

// ---------------------------------------------------------------- overlays

fn status(d: &mut DrawList, c: &Creep) {
    let r = c.radius;
    let bz = c.height();
    // At most one glow per monster, and a faint one. A wave is a hundred and
    // fifty creeps, and a hundred and fifty additive sprites on top of each
    // other is a white sheet rather than a status effect.
    let tint = if c.burn.t > 0.0 {
        Some([1.0, 0.45, 0.12])
    } else if c.poison.t > 0.0 {
        Some([0.45, 1.0, 0.35])
    } else if c.slow.t > 0.0 {
        Some([0.45, 0.80, 1.0])
    } else {
        None
    };
    if let Some(col) = tint {
        d.glow([c.pos[0], c.pos[1], bz], r * 1.9, 0.7, rgba(col, 0.13));
    }
    if c.stun > 0.0 {
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

/// A health bar, on the few monsters it tells you something about.
///
/// Warcraft III does not float a bar over every unit on the field, and the
/// reason is visible the moment you try it: at wave thirteen this lane carries
/// three hundred and thirty-eight monsters, nearly all of them damaged, and
/// three hundred bars is a green picket fence with the game behind it. Removing
/// them entirely was the single largest improvement to the picture in this
/// whole pass.
///
/// So the bar earns its place. A boss always has one, because a boss is the one
/// monster a player tracks individually. Everything else gets one only once it
/// is under half, which is the point the number changes a decision - whether to
/// let it round again or spend on another tower - and by then only a handful of
/// the wave qualifies at any instant.
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
