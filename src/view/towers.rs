//! Tower models.
//!
//! Twenty-one towers is too many to hand-sculpt without them drifting apart, so
//! every one is assembled from three interchangeable pieces:
//!
//!   - a **base**, chosen by the tower's primary element - the stance,
//!   - a **mast**, chosen by its delivery - the role, and therefore the
//!     silhouette,
//!   - a **crown**, chosen by its secondary element - the emitter.
//!
//! A pure tower takes its own element for both base and crown, so it reads as a
//! matched set; a dual tower is visibly the marriage of two, which is exactly
//! what it is. The mast carries the role, so every zone tower is a squat
//! cauldron and every chain tower is a lattice pylon whatever elements built it.
//! You should be able to name any tower from its outline alone, at any level.

use super::PLOT_TOP;
use super::theme;
use crate::game::Tower;
use crate::game::defs::*;
use crate::gfx::draw::{Color, DrawList, Material, Shape, boost, mix, rgba};

/// Top of the stone plinth every tower stands on.
const DECK: f32 = PLOT_TOP + 0.20;

/// Offset a point by a rotated local (forward, right) pair.
fn local(p: [f32; 2], yaw: f32, fwd: f32, right: f32) -> [f32; 2] {
    let (c, s) = (yaw.cos(), yaw.sin());
    [p[0] + c * fwd - s * right, p[1] + s * fwd + c * right]
}

/// Everything the three builders need, gathered once so they do not each have
/// to re-derive it from the Tower.
struct Build {
    p: [f32; 2],
    yaw: f32,
    tier: u32,
    /// 0 at the moment of placing, 1 once the tower has finished rising.
    grow: f32,
    /// Animation clock, shared so a tower's parts pulse together.
    now: f32,
    flash: f32,
    /// Base colour of the primary element, and of the secondary.
    a: [f32; 3],
    b: [f32; 3],
    /// Height of the mast top above [`DECK`]. The crown sits here.
    top: f32,
}

impl Build {
    fn attuned(&self) -> bool {
        self.tier >= ATTUNE_TIER
    }
    fn ascended(&self) -> bool {
        self.tier >= ASCEND_TIER
    }
    /// World position of the crown.
    fn head(&self) -> [f32; 3] {
        [self.p[0], self.p[1], DECK + self.top]
    }
}

pub fn draw(d: &mut DrawList, tw: &Tower, selected: bool, now: f32) {
    let def = tw.def();
    let (ea, eb) = (def.elem.0, def.elem.1.unwrap_or(def.elem.0));
    let grow = (((now - tw.built_at) * 4.0).min(1.0)).max(0.05);
    // Support towers stand tall and bare; everything else grows a mast that
    // scales with its level, so investment reads from across the board.
    let base_h = if matches!(def.delivery, Delivery::Aura) {
        0.62
    } else {
        0.34
    };
    let b = Build {
        p: tw.pos,
        yaw: tw.angle,
        tier: tw.tier,
        grow,
        now,
        flash: tw.flash,
        a: ea.color(),
        b: eb.color(),
        top: (base_h + 0.055 * tw.tier as f32) * grow,
    };

    plinth(d, &b, ea, def.is_dual());
    mast(d, &b, def.delivery);
    crown(d, &b, eb, def.delivery);
    if b.ascended() {
        halo(d, &b);
    }

    if selected {
        let col = tower_color(def);
        d.ground_ring(tw.pos, tw.range(), 0.11, rgba(col, 0.85), 80);
        d.glow([tw.pos[0], tw.pos[1], 0.55], 1.2, 2.2, rgba(col, 0.16));
    }
}

// ---------------------------------------------------------------- the base

/// A turned stone footing, dressed in the primary element's manner. A dual
/// tower gets a second ring of markers in its secondary colour, so pure and
/// dual are distinguishable at a glance without reading anything.
fn plinth(d: &mut DrawList, b: &Build, e: Element, dual: bool) {
    let p = b.p;
    let dark = mix(theme::STONE_DARK, b.a, 0.12);
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

    match e {
        // Roots that have grown around and through the stone.
        Element::Nature => {
            for k in 0..7 {
                let a = k as f32 * 0.897;
                let q = [p[0] + a.cos() * 0.36, p[1] + a.sin() * 0.36];
                d.link(
                    Shape::Capsule,
                    [q[0], q[1], PLOT_TOP + 0.14],
                    [p[0] + a.cos() * 0.14, p[1] + a.sin() * 0.14, DECK + 0.08],
                    0.075,
                    rgba(mix([0.24, 0.18, 0.12], b.a, 0.35), 1.0),
                    Material::WOOD,
                    0.0,
                );
            }
        }
        // A stepped hexagonal hearth with vents glowing between the courses.
        Element::Fire => {
            d.prism(
                [p[0], p[1], DECK - 0.06],
                0.70,
                0.18,
                0.4,
                rgba(mix([0.10, 0.08, 0.09], b.a, 0.12), 1.0),
                Material::STONE,
            );
            for k in 0..4 {
                let a = k as f32 * 1.571 + 0.4;
                d.cylinder(
                    [p[0] + a.cos() * 0.30, p[1] + a.sin() * 0.30, DECK - 0.06],
                    0.11,
                    0.10,
                    0.0,
                    rgba(boost3(b.a, 1.5), 1.0),
                    Material::METAL,
                );
            }
        }
        // A shallow basin with a still surface in it.
        Element::Water => {
            d.cylinder(
                [p[0], p[1], DECK - 0.08],
                0.70,
                0.12,
                0.0,
                rgba(dark, 1.0),
                Material::STONE,
            );
            d.cylinder(
                [p[0], p[1], DECK - 0.015],
                0.54,
                0.03,
                0.0,
                rgba(b.a, 0.75),
                Material::WATER,
            );
        }
        // Three rough megaliths carrying the weight.
        Element::Earth => {
            for k in 0..3 {
                let a = k as f32 * 2.094 + 0.6;
                d.prism(
                    [p[0] + a.cos() * 0.30, p[1] + a.sin() * 0.30, DECK - 0.05],
                    0.30,
                    0.22,
                    a,
                    rgba(mix(theme::STONE, b.a, 0.30), 1.0),
                    Material::STONE,
                );
            }
        }
        // Fluted columns, pale and even.
        Element::Light => {
            for k in 0..6 {
                let a = k as f32 * 1.047;
                d.cylinder(
                    [p[0] + a.cos() * 0.33, p[1] + a.sin() * 0.33, DECK - 0.03],
                    0.11,
                    0.24,
                    0.0,
                    rgba(mix([0.72, 0.70, 0.64], b.a, 0.30), 1.0),
                    Material::STONE,
                );
            }
        }
        // A collar of thorns leaning outward.
        Element::Dark => {
            for k in 0..8 {
                let a = k as f32 * 0.785;
                d.cone(
                    [p[0] + a.cos() * 0.32, p[1] + a.sin() * 0.32, DECK - 0.06],
                    0.13,
                    0.22,
                    0.0,
                    rgba(mix([0.10, 0.09, 0.14], b.a, 0.40), 1.0),
                    Material::DARK_METAL,
                );
            }
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

    // A dual tower carries three beads of its second element around the deck.
    if dual {
        for k in 0..3 {
            let a = k as f32 * 2.094 + b.yaw;
            d.sphere_lit(
                [p[0] + a.cos() * 0.34, p[1] + a.sin() * 0.34, DECK + 0.07],
                0.11,
                rgba(b.b, 1.0),
                0.8,
            );
        }
    }
}

// ---------------------------------------------------------------- the mast

/// The role, in one shape. This is what the eye actually reads at a distance,
/// so each delivery gets an outline nothing else uses.
fn mast(d: &mut DrawList, b: &Build, delivery: Delivery) {
    let p = b.p;
    let yaw = b.yaw;
    let h = b.top;
    let steel = mix([0.22, 0.23, 0.28], b.a, 0.25);
    let mid = DECK + h * 0.5;

    match delivery {
        // A tapered pillar under a yoke that swings to face the target.
        Delivery::Shot { .. } => {
            d.cylinder(
                [p[0], p[1], mid],
                0.30,
                h,
                0.0,
                rgba(steel, 1.0),
                Material::STONE,
            );
            let recoil = b.flash * 0.09;
            for s in [-1.0f32, 1.0] {
                let q = local(p, yaw, -recoil, s * 0.20);
                let f = local(p, yaw, 0.22 - recoil, s * 0.20);
                d.link(
                    Shape::Capsule,
                    [q[0], q[1], DECK + h - 0.06],
                    [f[0], f[1], DECK + h - 0.06],
                    0.09,
                    rgba(mix(steel, b.b, 0.35), 1.0),
                    Material::METAL,
                    0.0,
                );
            }
        }
        // A narrow spire with a focusing collar: everything about it points up.
        Delivery::Beam { .. } => {
            d.cylinder(
                [p[0], p[1], mid],
                0.22,
                h,
                0.0,
                rgba(steel, 1.0),
                Material::DARK_METAL,
            );
            for k in 0..3 {
                let z = DECK + h * (0.35 + 0.22 * k as f32);
                d.cylinder(
                    [p[0], p[1], z],
                    0.36 - 0.05 * k as f32,
                    0.05,
                    0.0,
                    rgba(mix(steel, b.b, 0.5), 1.0),
                    Material::METAL,
                );
            }
        }
        // A long barrel on a pivot, laid along the firing line.
        Delivery::Lance { .. } => {
            d.cylinder(
                [p[0], p[1], mid],
                0.26,
                h * 0.8,
                0.0,
                rgba(steel, 1.0),
                Material::METAL,
            );
            let recoil = b.flash * 0.16;
            let back = local(p, yaw, -0.30 - recoil, 0.0);
            let fore = local(p, yaw, 0.52 - recoil, 0.0);
            let z = DECK + h;
            d.link(
                Shape::Capsule,
                [back[0], back[1], z],
                [fore[0], fore[1], z],
                0.15,
                rgba(mix(steel, b.b, 0.30), 1.0),
                Material::METAL,
                0.0,
            );
        }
        // A lattice pylon with cross-arms - it looks like it conducts.
        Delivery::Chain { .. } => {
            for s in [-1.0f32, 1.0] {
                for f in [-1.0f32, 1.0] {
                    let foot = local(p, yaw, f * 0.20, s * 0.20);
                    d.link(
                        Shape::Capsule,
                        [foot[0], foot[1], DECK],
                        [p[0], p[1], DECK + h],
                        0.055,
                        rgba(steel, 1.0),
                        Material::DARK_METAL,
                        0.0,
                    );
                }
            }
            for k in 0..2 {
                let z = DECK + h * (0.42 + 0.30 * k as f32);
                let w = 0.30 - 0.09 * k as f32;
                let l = local(p, yaw, 0.0, -w);
                let r = local(p, yaw, 0.0, w);
                d.link(
                    Shape::Cylinder,
                    [l[0], l[1], z],
                    [r[0], r[1], z],
                    0.04,
                    rgba(mix(steel, b.b, 0.4), 1.0),
                    Material::METAL,
                    0.0,
                );
            }
        }
        // A squat wide drum. It aims at nothing, so nothing about it points.
        Delivery::Nova => {
            d.cylinder(
                [p[0], p[1], DECK + h * 0.42],
                0.62,
                h * 0.84,
                0.0,
                rgba(steel, 1.0),
                Material::METAL,
            );
            // Pressure beads that pulse outward as it charges. Beads rather
            // than a solid ring: a ring is forty flat segments, and this tower
            // already has very little else in it.
            let phase = (b.now * 1.4).fract();
            let rad = 0.5 + phase * 1.6;
            for k in 0..12 {
                let a = k as f32 * std::f32::consts::FRAC_PI_6 + b.now * 0.3;
                d.sphere_lit(
                    [p[0] + a.cos() * rad, p[1] + a.sin() * rad, DECK + 0.06],
                    0.09,
                    rgba(b.b, (1.0 - phase) * 0.8),
                    0.9,
                );
            }
            // Vents around the drum, so the silhouette reads as pressure.
            for k in 0..6 {
                let a = k as f32 * 1.047;
                d.cone(
                    [
                        p[0] + a.cos() * 0.32,
                        p[1] + a.sin() * 0.32,
                        DECK + h * 0.86,
                    ],
                    0.14,
                    0.16,
                    a,
                    rgba(mix(steel, b.b, 0.5), 1.0),
                    Material::METAL,
                );
            }
        }
        // A low tripod holding a cauldron over the road.
        Delivery::Zone { .. } => {
            for k in 0..3 {
                let a = k as f32 * 2.094 + yaw;
                let foot = [p[0] + a.cos() * 0.34, p[1] + a.sin() * 0.34];
                d.link(
                    Shape::Capsule,
                    [foot[0], foot[1], DECK],
                    [p[0], p[1], DECK + h * 0.9],
                    0.07,
                    rgba(steel, 1.0),
                    Material::DARK_METAL,
                    0.0,
                );
            }
            d.cylinder(
                [p[0], p[1], DECK + h],
                0.56,
                0.20,
                0.0,
                rgba(mix(steel, b.b, 0.25), 1.0),
                Material::METAL,
            );
        }
        // A bare obelisk. Nothing on it moves, because it never fires.
        Delivery::Aura => {
            d.prism(
                [p[0], p[1], mid],
                0.40,
                h,
                b.now * 0.25,
                rgba(mix(steel, b.a, 0.35), 1.0),
                Material::STONE,
            );
        }
    }
}

// ---------------------------------------------------------------- the crown

/// The emitter, in the secondary element's manner. This is the part that moves
/// and the part that glows, so it is where the tower's element is legible even
/// in a crowd.
fn crown(d: &mut DrawList, b: &Build, e: Element, delivery: Delivery) {
    let c = b.head();
    let col = b.b;
    let hot = boost3(col, 1.6);
    let pulse = 0.85 + 0.15 * (b.now * 2.2 + c[0]).sin();
    // Attuned towers carry a visibly bigger emitter; that is the whole point of
    // the milestone being at a level rather than at a purchase.
    let k = if b.attuned() { 1.28 } else { 1.0 };

    match e {
        // A bloom of leaves around a seed pod.
        Element::Nature => {
            d.sphere_lit(
                [c[0], c[1], c[2] + 0.10],
                0.26 * k,
                rgba(hot, 1.0),
                0.55 * pulse,
            );
            for i in 0..5 {
                let a = i as f32 * 1.257 + b.now * 0.4;
                let tip = [
                    c[0] + a.cos() * 0.30 * k,
                    c[1] + a.sin() * 0.30 * k,
                    c[2] + 0.06,
                ];
                d.link(
                    Shape::Capsule,
                    [c[0], c[1], c[2] + 0.08],
                    tip,
                    0.055,
                    rgba(mix(col, [0.20, 0.36, 0.18], 0.35), 1.0),
                    Material::FOLIAGE,
                    0.0,
                );
            }
        }
        // A brazier bowl with a flame standing in it.
        Element::Fire => {
            d.cylinder(
                [c[0], c[1], c[2] + 0.04],
                0.34 * k,
                0.12,
                0.0,
                rgba(mix([0.16, 0.13, 0.12], col, 0.35), 1.0),
                Material::DARK_METAL,
            );
            for i in 0..3 {
                let t = b.now * 3.0 + i as f32 * 2.1;
                let lean = 0.05 * t.sin();
                d.cone(
                    [c[0] + lean, c[1] + 0.04 * (t * 0.7).cos(), c[2] + 0.16],
                    0.22 * k,
                    (0.26 + 0.06 * (t * 1.3).sin()) * k,
                    0.0,
                    rgba(hot, 0.9),
                    Material::STONE,
                );
            }
            d.glow(
                [c[0], c[1], c[2] + 0.22],
                0.85 * k,
                2.4,
                rgba(hot, 0.30 * pulse),
            );
        }
        // Droplets orbiting a still core.
        Element::Water => {
            d.sphere_lit(
                [c[0], c[1], c[2] + 0.10],
                0.24 * k,
                rgba(hot, 0.92),
                0.6 * pulse,
            );
            for i in 0..3 {
                let a = b.now * 1.6 + i as f32 * 2.094;
                d.sphere_lit(
                    [
                        c[0] + a.cos() * 0.28 * k,
                        c[1] + a.sin() * 0.28 * k,
                        c[2] + 0.12 + 0.05 * (a * 2.0).sin(),
                    ],
                    0.11 * k,
                    rgba(col, 0.95),
                    0.7,
                );
            }
        }
        // An anvil-headed weight over a hexagonal collar. Heavy, blunt, unlit.
        Element::Earth => {
            d.cube_mat(
                [c[0], c[1], c[2] + 0.12],
                [0.46 * k, 0.34 * k, 0.22 * k],
                b.yaw,
                rgba(mix(theme::STONE, col, 0.45), 1.0),
                Material::STONE,
            );
            d.prism(
                [c[0], c[1], c[2] + 0.27 * k],
                0.30 * k,
                0.12,
                b.yaw,
                rgba(col, 1.0),
                Material::METAL,
            );
            for s in [-1.0f32, 1.0] {
                let q = local([c[0], c[1]], b.yaw, 0.0, s * 0.26 * k);
                d.cone(
                    [q[0], q[1], c[2] + 0.08],
                    0.16 * k,
                    0.18 * k,
                    b.yaw,
                    rgba(mix(theme::STONE, col, 0.6), 1.0),
                    Material::STONE,
                );
            }
        }
        // A faceted gem that throws light down onto the deck.
        Element::Light => {
            d.prism(
                [c[0], c[1], c[2] + 0.16],
                0.30 * k,
                0.30 * k,
                b.now * 0.9,
                rgba(hot, 0.95),
                Material::GEM,
            );
            d.glow(
                [c[0], c[1], c[2] + 0.16],
                0.95 * k,
                2.6,
                rgba(hot, 0.26 * pulse),
            );
        }
        // A void sphere inside a slowly turning ring.
        Element::Dark => {
            d.sphere(
                [c[0], c[1], c[2] + 0.14],
                0.26 * k,
                rgba([0.04, 0.03, 0.07], 1.0),
                Material::GEM,
            );
            for i in 0..10 {
                let a = i as f32 * 0.628 + b.now * 0.7;
                d.sphere_lit(
                    [
                        c[0] + a.cos() * 0.28 * k,
                        c[1] + a.sin() * 0.28 * k,
                        c[2] + 0.14,
                    ],
                    0.065,
                    rgba(hot, 1.0),
                    0.9,
                );
            }
            d.glow(
                [c[0], c[1], c[2] + 0.14],
                0.80 * k,
                2.2,
                rgba(col, 0.22 * pulse),
            );
        }
    }

    // A zone tower spills its element over the lip of the cauldron, so it is
    // obvious the thing pours onto the road rather than shooting at anything.
    if let Delivery::Zone { .. } = delivery {
        for i in 0..4 {
            let a = i as f32 * 1.571 + b.now * 0.5;
            d.sphere_lit(
                [
                    c[0] + a.cos() * 0.26,
                    c[1] + a.sin() * 0.26,
                    c[2] - 0.10 - 0.06 * ((b.now * 2.0 + i as f32).sin() * 0.5 + 0.5),
                ],
                0.09,
                rgba(hot, 0.9),
                0.8,
            );
        }
    }
}

/// The ascendant milestone: a ring of shards orbiting the crown, in the primary
/// element's colour so the two halves of a dual tower both show at level seven.
fn halo(d: &mut DrawList, b: &Build) {
    let c = b.head();
    for i in 0..6 {
        let a = i as f32 * 1.047 + b.now * 0.8;
        let z = c[2] + 0.34 + 0.05 * (a * 2.0 + b.now).sin();
        d.prism(
            [c[0] + a.cos() * 0.42, c[1] + a.sin() * 0.42, z],
            0.10,
            0.16,
            a,
            rgba(boost3(b.a, 1.4), 0.95),
            Material::GEM,
        );
    }
    d.glow([c[0], c[1], c[2] + 0.34], 1.1, 2.0, rgba(b.a, 0.18));
}

/// Brightens a bare rgb triple. [`boost`] works on an rgba Color.
fn boost3(c: [f32; 3], k: f32) -> [f32; 3] {
    let out: Color = boost(rgba(c, 1.0), k);
    [out[0], out[1], out[2]]
}

// ---------------------------------------------------------------- ghost

/// The translucent preview shown while a tower is held over a pad.
pub fn draw_ghost(d: &mut DrawList, def_i: usize, tier: u32, p: [f32; 2], now: f32) {
    let Some(def) = TOWERS.get(def_i) else { return };
    let (ea, eb) = (def.elem.0, def.elem.1.unwrap_or(def.elem.0));
    let base_h = if matches!(def.delivery, Delivery::Aura) {
        0.62
    } else {
        0.34
    };
    let b = Build {
        p,
        yaw: now * 0.6,
        tier: tier.max(1),
        grow: 1.0,
        now,
        flash: 0.0,
        a: ea.color(),
        b: eb.color(),
        top: base_h + 0.055 * tier.max(1) as f32,
    };
    mast(d, &b, def.delivery);
    crown(d, &b, eb, def.delivery);
}
