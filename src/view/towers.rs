//! Tower models.
//!
//! Every tower is assembled from the same unit cube, so identity has to come from
//! **silhouette**, not colour: a crossbow's wide horizontal limb, a cannon's fat
//! forward barrel, frost's jagged shard cluster, a beacon's bare obelisk. You
//! should be able to name any tower from its outline alone.

use super::theme;
use super::PLOT_TOP;
use crate::game::Tower;
use crate::game::defs::*;
use crate::gfx::draw::{Color, DrawList, boost, mix, rgba};

/// Top of the stone plinth every tower stands on.
const DECK: f32 = PLOT_TOP + 0.18;

/// Places a box sitting on `z` and returns the height of its top.
fn stack(d: &mut DrawList, p: [f32; 2], z: f32, size: [f32; 3], yaw: f32, col: Color, em: f32) -> f32 {
    d.solid.push(crate::gfx::draw::Instance {
        pos: [p[0], p[1], z + size[2] * 0.5],
        scale: size,
        rot: [yaw, 0.0],
        params: [em, 0.0],
        color: col,
    });
    z + size[2]
}

/// Offset a point by a rotated local (forward, right) pair.
fn local(p: [f32; 2], yaw: f32, fwd: f32, right: f32) -> [f32; 2] {
    let (c, s) = (yaw.cos(), yaw.sin());
    [p[0] + c * fwd - s * right, p[1] + s * fwd + c * right]
}

pub fn draw(d: &mut DrawList, tw: &Tower, selected: bool, now: f32) {
    let def = tw.def();
    let col = tower_color(def);
    let p = tw.pos;
    // Towers pop up when built.
    let grow = (((now - tw.built_at) * 4.0).min(1.0)).max(0.05);

    plinth(d, p, col, tw.tier);

    match def.id {
        "ballista" => ballista(d, tw, col, grow, now),
        "cannon" => cannon(d, tw, col, grow),
        "frost" => frost(d, tw, col, grow, now),
        "pyre" => pyre(d, tw, col, grow, now),
        "tesla" => tesla(d, tw, col, grow, now),
        "venom" => venom(d, tw, col, grow, now),
        "beacon" => beacon(d, tw, col, grow, now),
        "mint" => mint(d, tw, col, grow, now),
        _ => {
            stack(d, p, DECK, [0.5, 0.5, 0.6 * grow], tw.angle, rgba(col, 1.0), 0.3);
        }
    }

    if selected {
        d.ground_ring(p, tw.range(), 0.11, rgba(col, 0.85), 80);
        d.glow([p[0], p[1], 0.55], 1.2, 2.2, rgba(col, 0.16));
    }
}

/// Stone foundation, growing a course of blocks for every couple of levels.
fn plinth(d: &mut DrawList, p: [f32; 2], col: [f32; 3], tier: u32) {
    let dark = mix(theme::STONE_DARK, col, 0.12);
    // The socket is packed flush with cut stone, then the plinth rises out of
    // it: an occupied plot is visibly the finished version of an empty one.
    d.slab(p, [0.86, 0.86], PLOT_TOP + 0.02, 0.14, rgba(dark, 1.0));
    d.slab(p, [0.80, 0.80], DECK, 0.12, rgba(theme::STONE, 1.0));
    // Corner blocks appear as the tower is invested in, so level reads from afar.
    if tier >= 3 {
        for (dx, dy) in [(-0.34, -0.34), (0.34, -0.34), (-0.34, 0.34), (0.34, 0.34)] {
            d.cube([p[0] + dx, p[1] + dy, DECK + 0.06], [0.16, 0.16, 0.12], 0.0, rgba(dark, 1.0));
        }
    }
    if tier >= 5 {
        for (dx, dy) in [(-0.34, -0.34), (0.34, -0.34), (-0.34, 0.34), (0.34, 0.34)] {
            d.cube_lit(
                [p[0] + dx, p[1] + dy, DECK + 0.16],
                [0.10, 0.10, 0.10],
                0.0,
                rgba(col, 1.0),
                0.9,
            );
        }
    }
}

// ---------------------------------------------------------------- the eight

/// Wide horizontal bow limb across a narrow tower. Unmistakable crossbar.
fn ballista(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let yaw = tw.angle;
    let wood = mix([0.30, 0.21, 0.13], col, 0.25);
    let t = tw.tier as f32;

    // Two uprights and a cross-brace.
    let h = (0.34 + 0.05 * t) * grow;
    for r in [-0.19f32, 0.19] {
        let q = local(p, yaw, 0.0, r);
        stack(d, q, DECK, [0.13, 0.13, h], yaw, rgba(wood, 1.0), 0.0);
    }
    let top = DECK + h;
    stack(d, p, top, [0.44, 0.5, 0.10], yaw, rgba(wood, 1.0), 0.0);

    // The bow: one long limb across the aim direction. This is the silhouette.
    let limb = if tw.fork == Some(0) { 1.35 } else { 1.02 } + 0.04 * t;
    let recoil = tw.flash * 0.10;
    let bz = top + 0.13;
    d.cube([p[0], p[1], bz], [0.16, limb, 0.09], yaw, rgba(wood, 1.0));
    // Swept tips, so the limb reads as a bow rather than a plank.
    for s in [-1.0f32, 1.0] {
        let q = local(p, yaw, -0.10, s * limb * 0.5);
        d.cube([q[0], q[1], bz], [0.22, 0.12, 0.08], yaw, rgba(mix(wood, col, 0.5), 1.0));
    }
    // Bolt in the groove.
    let n = if tw.fork == Some(1) { 3 } else { 1 };
    for i in 0..n {
        let off = (i as f32 - (n as f32 - 1.0) * 0.5) * 0.16;
        let q = local(p, yaw, 0.20 - recoil, off);
        d.cube_lit([q[0], q[1], bz + 0.06], [0.52, 0.07, 0.07], yaw, rgba(col, 1.0), 0.85);
    }
    // Marksman gets a raised sight.
    if tw.fork == Some(0) {
        let q = local(p, yaw, -0.16, 0.0);
        d.cube_lit([q[0], q[1], bz + 0.20], [0.20, 0.09, 0.09], yaw, rgba(col, 1.0), 1.0);
    }
    if tw.flash > 0.0 {
        let m = local(p, yaw, 0.55, 0.0);
        d.glow([m[0], m[1], bz + 0.06], 0.44 * tw.flash, 1.6, boost(rgba(col, tw.flash), 2.2));
    }
    let _ = now;
}

/// Low, wide, with a fat barrel and wheels. Reads as artillery.
fn cannon(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32) {
    let p = tw.pos;
    let yaw = tw.angle;
    let iron = mix([0.20, 0.21, 0.24], col, 0.20);
    let t = tw.tier as f32;

    // Squat carriage.
    let h = (0.20 + 0.025 * t) * grow;
    let top = stack(d, p, DECK, [0.66, 0.58, h], yaw, rgba(iron, 1.0), 0.0);

    // Wheels either side - the giveaway detail.
    for s in [-1.0f32, 1.0] {
        let q = local(p, yaw, -0.04, s * 0.34);
        d.cube([q[0], q[1], DECK + h * 0.55], [0.34, 0.10, 0.34], yaw, rgba(theme::STONE_DARK, 1.0));
        d.cube_lit([q[0], q[1], DECK + h * 0.55], [0.12, 0.12, 0.12], yaw, rgba(col, 1.0), 0.6);
    }

    // Barrel, pitched up, recoiling when it fires.
    let recoil = tw.flash * 0.16;
    let len = if tw.fork == Some(0) { 0.92 } else { 0.52 } + 0.03 * t;
    let girth = if tw.fork == Some(1) { 0.34 } else { 0.26 };
    let pitch = if tw.fork == Some(0) { 0.42 } else { 0.16 };
    let base = local(p, yaw, -0.08 - recoil, 0.0);
    let tip = local(p, yaw, len - recoil, 0.0);
    d.bar(
        [base[0], base[1], top + 0.10],
        [tip[0], tip[1], top + 0.10 + len * pitch],
        girth,
        rgba(iron, 1.0),
        0.0,
    );
    // Muzzle ring.
    d.cube_lit(
        [tip[0], tip[1], top + 0.10 + len * pitch],
        [0.10, girth * 1.25, girth * 1.25],
        yaw,
        rgba(col, 1.0),
        0.55 + tw.flash * 0.45,
    );
    // Grapeshot sprouts extra muzzles.
    if tw.fork == Some(1) {
        for s in [-1.0f32, 1.0] {
            let q = local(p, yaw, len * 0.8 - recoil, s * 0.16);
            d.cube([q[0], q[1], top + 0.10], [0.34, 0.12, 0.12], yaw, rgba(iron, 1.0));
        }
    }
    if tw.flash > 0.0 {
        d.glow(
            [tip[0], tip[1], top + 0.14 + len * pitch],
            0.55 * tw.flash,
            1.5,
            boost(rgba(col, tw.flash), 2.4),
        );
    }
}

/// A cluster of leaning crystal shards. No barrel at all.
fn frost(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let ice = mix(col, [1.0, 1.0, 1.0], 0.15);
    let t = tw.tier as f32;
    let n = 3 + (tw.tier.min(6) / 2) as usize; // 3..6 shards

    // Frozen base ring.
    d.slab(p, [0.70, 0.70], DECK + 0.06, 0.08, rgba(mix(ice, [0.1, 0.1, 0.15], 0.6), 1.0));

    for i in 0..n {
        let a = i as f32 * 2.399 + 0.4; // golden-angle scatter, stable per index
        let r = if i == 0 { 0.0 } else { 0.16 + 0.05 * (i % 3) as f32 };
        let q = [p[0] + a.cos() * r, p[1] + a.sin() * r];
        let h = (if i == 0 { 0.62 } else { 0.30 + 0.11 * ((i * 7) % 4) as f32 }) * (0.8 + 0.06 * t)
            * grow;
        let w = 0.15 - 0.012 * i as f32;
        // Shards lean outwards; a slight sway makes them feel alive.
        let sway = (now * 0.9 + i as f32).sin() * 0.02;
        d.solid.push(crate::gfx::draw::Instance {
            pos: [q[0], q[1], DECK + 0.10 + h * 0.5],
            scale: [w, w, h],
            rot: [a + sway, 0.0],
            params: [0.10, 0.0],
            color: rgba(ice, 1.0),
        });
        // Glowing tip.
        d.cube_lit(
            [q[0], q[1], DECK + 0.10 + h],
            [w * 0.8, w * 0.8, w * 0.9],
            a,
            rgba(col, 1.0),
            0.75 + tw.flash * 0.25,
        );
    }

    // Glacier freezes hard: a cold mist pools at the base. Rime rings the tile.
    match tw.fork {
        Some(0) => d.glow([p[0], p[1], DECK + 0.25], 0.95, 2.2, rgba(col, 0.30 + tw.flash * 0.3)),
        Some(1) => d.ground_ring(p, 0.62, 0.07, rgba(col, 0.55), 20),
        _ => d.glow([p[0], p[1], DECK + 0.35], 0.70, 2.4, rgba(col, 0.18 + tw.flash * 0.35)),
    }
}

/// A brazier bowl with fire licking out of it.
fn pyre(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let iron = [0.17, 0.15, 0.16];
    let t = tw.tier as f32;

    // Column, then a bowl that flares outward - the distinctive profile.
    let h = (0.26 + 0.04 * t) * grow;
    let top = stack(d, p, DECK, [0.26, 0.26, h], 0.0, rgba(iron, 1.0), 0.0);
    let bowl = 0.50 + 0.02 * t;
    stack(d, p, top, [bowl * 0.72, bowl * 0.72, 0.08], 0.0, rgba(iron, 1.0), 0.0);
    let rim = stack(d, p, top + 0.08, [bowl, bowl, 0.12], 0.0, rgba(iron, 1.0), 0.0);
    // Coals.
    d.cube_lit([p[0], p[1], rim - 0.02], [bowl * 0.82, bowl * 0.82, 0.05], 0.0, rgba(col, 1.0), 1.0);

    // Flames: a few boxes bobbing on different phases.
    let flames = if tw.fork == Some(0) { 5 } else { 3 };
    for i in 0..flames {
        let a = i as f32 * 2.1;
        let r = 0.10 + 0.04 * (i % 2) as f32;
        let wob = (now * 6.0 + i as f32 * 1.7).sin();
        let fh = 0.20 + 0.10 * (wob * 0.5 + 0.5) + 0.02 * t;
        d.cube_lit(
            [p[0] + a.cos() * r, p[1] + a.sin() * r, rim + fh * 0.5],
            [0.11, 0.11, fh],
            a + wob * 0.2,
            rgba(mix(col, [1.0, 0.92, 0.5], 0.35), 1.0),
            1.0,
        );
    }
    d.glow([p[0], p[1], rim + 0.24], 0.95 + tw.flash * 0.3, 1.9, rgba(col, 0.42));
    // Furnace glows hotter the longer it has been firing.
    if tw.fork == Some(1) && tw.ramp > 0.0 {
        d.glow([p[0], p[1], rim + 0.2], 0.8 + tw.ramp * 0.5, 2.0, rgba([1.0, 0.85, 0.4], 0.35));
    }
}

/// Stacked coil rings around a pole, with an orb on top.
fn tesla(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let metal = [0.22, 0.24, 0.28];
    let t = tw.tier as f32;
    let h = (0.60 + 0.07 * t) * grow;

    stack(d, p, DECK, [0.15, 0.15, h], 0.0, rgba(metal, 1.0), 0.0);

    // Rings: wide flat plates that shrink going up, slowly counter-rotating.
    let rings = 2 + (tw.tier.min(6) / 2) as usize;
    for i in 0..rings {
        let f = 1.0 - i as f32 * 0.18;
        let z = DECK + h * (0.30 + 0.22 * i as f32).min(0.94);
        let spin = now * (0.5 + 0.2 * i as f32) * if i % 2 == 0 { 1.0 } else { -1.0 };
        d.cube([p[0], p[1], z], [0.56 * f, 0.56 * f, 0.05], spin, rgba(metal, 1.0));
        d.cube_lit([p[0], p[1], z], [0.60 * f, 0.10, 0.035], spin, rgba(col, 1.0), 0.8);
    }

    // Orb: the charge indicator.
    let charge = (1.0 - tw.cooldown.max(0.0) * tw.rate().max(0.1)).clamp(0.0, 1.0);
    let orb = 0.20 + 0.05 * charge;
    d.cube_lit(
        [p[0], p[1], DECK + h + orb * 0.5],
        [orb, orb, orb],
        now * 1.6,
        rgba(col, 1.0),
        0.6 + charge * 0.4,
    );
    d.glow(
        [p[0], p[1], DECK + h + orb * 0.5],
        0.55 + charge * 0.35 + tw.flash * 0.4,
        2.0,
        rgba(col, 0.25 + charge * 0.25),
    );
    // Storm sprouts extra emitters.
    if tw.fork == Some(0) {
        for s in [-1.0f32, 1.0] {
            let q = local(p, now * 0.8, 0.0, s * 0.30);
            d.cube_lit([q[0], q[1], DECK + h * 0.9], [0.08, 0.08, 0.22], 0.0, rgba(col, 1.0), 1.0);
        }
    }
}

/// A squat vat of bubbling venom with pipes out of the sides.
fn venom(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let brass = mix([0.26, 0.23, 0.17], col, 0.18);
    let t = tw.tier as f32;

    // Wide belly, narrower neck: a cauldron profile.
    let h = (0.30 + 0.035 * t) * grow;
    let belly = 0.62;
    let mid = stack(d, p, DECK, [belly * 0.8, belly * 0.8, 0.08], 0.0, rgba(brass, 1.0), 0.0);
    let top = stack(d, p, mid, [belly, belly, h], 0.0, rgba(brass, 1.0), 0.0);
    // Liquid surface.
    d.cube_lit([p[0], p[1], top - 0.02], [belly * 0.84, belly * 0.84, 0.05], 0.0, rgba(col, 1.0), 1.0);

    // Bubbles rising and popping.
    for i in 0..3 {
        let ph = (now * (0.8 + 0.2 * i as f32) + i as f32 * 2.0).rem_euclid(1.0);
        let a = i as f32 * 2.3;
        let r = 0.14;
        let s = 0.10 * (1.0 - ph);
        if s > 0.01 {
            d.cube_lit(
                [p[0] + a.cos() * r, p[1] + a.sin() * r, top + ph * 0.20],
                [s, s, s],
                0.0,
                rgba(mix(col, [1.0, 1.0, 1.0], 0.3), 1.0),
                1.0,
            );
        }
    }

    // Pipes: two angled spouts, aimed by the turret.
    let yaw = tw.angle;
    let n = if tw.fork == Some(0) { 3 } else { 2 };
    for i in 0..n {
        let off = (i as f32 - (n as f32 - 1.0) * 0.5) * 0.20;
        let a = local(p, yaw, 0.18, off);
        let b = local(p, yaw, 0.52, off * 1.4);
        d.bar([a[0], a[1], top - 0.02], [b[0], b[1], top + 0.14], 0.10, rgba(brass, 1.0), 0.0);
        d.cube_lit([b[0], b[1], top + 0.14], [0.09, 0.09, 0.09], yaw, rgba(col, 1.0), 0.9);
    }
    d.glow([p[0], p[1], top + 0.10], 0.70, 2.2, rgba(col, 0.22 + tw.flash * 0.3));
}

/// A bare obelisk with a ring orbiting it. Tallest and thinnest; never shoots.
fn beacon(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let stone = mix(theme::STONE, col, 0.25);
    let t = tw.tier as f32;
    let h = (0.95 + 0.10 * t) * grow;

    // Tapering shaft in three courses.
    let mut z = DECK;
    for i in 0..3 {
        let f = 1.0 - i as f32 * 0.18;
        z = stack(d, p, z, [0.30 * f, 0.30 * f, h / 3.0], 0.0, rgba(stone, 1.0), 0.0);
    }
    // Crown.
    d.cube_lit([p[0], p[1], z + 0.14], [0.22, 0.22, 0.28], now * 0.9, rgba(col, 1.0), 1.0);

    // Orbiting ring, drawn as four blocks on a circle.
    let orbit = 0.42 + 0.02 * t;
    let spin = now * 1.1;
    for i in 0..4 {
        let a = spin + i as f32 * std::f32::consts::FRAC_PI_2;
        let bob = (now * 1.6 + i as f32).sin() * 0.05;
        d.cube_lit(
            [p[0] + a.cos() * orbit, p[1] + a.sin() * orbit, z * 0.72 + bob],
            [0.16, 0.09, 0.09],
            a + std::f32::consts::FRAC_PI_2,
            rgba(col, 1.0),
            0.95,
        );
    }
    d.glow([p[0], p[1], z + 0.16], 0.85, 2.0, rgba(col, 0.35));
    // The aura reach is only drawn on hover or selection - a board full of
    // permanent rings buries everything else under white circles.
}

/// A strongbox with a stepped roof and stacks of coins.
fn mint(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let wood = [0.24, 0.18, 0.13];
    let t = tw.tier as f32;

    let h = (0.34 + 0.03 * t) * grow;
    let top = stack(d, p, DECK, [0.62, 0.62, h], 0.0, rgba(wood, 1.0), 0.0);
    // Stepped roof: two shrinking slabs. Reads as a vault.
    let r1 = stack(d, p, top, [0.68, 0.68, 0.08], 0.0, rgba(mix(wood, col, 0.3), 1.0), 0.0);
    let r2 = stack(d, p, r1, [0.46, 0.46, 0.08], 0.0, rgba(mix(wood, col, 0.5), 1.0), 0.0);
    d.cube_lit([p[0], p[1], r2 + 0.06], [0.22, 0.22, 0.10], now * 0.6, rgba(col, 1.0), 1.0);

    // Coin slot on the face.
    d.cube_lit([p[0], p[1] - 0.32, DECK + h * 0.6], [0.24, 0.03, 0.06], 0.0, rgba(col, 1.0), 0.9);

    // Coin stacks at the corners: more of them as it levels.
    let stacks = 2 + (tw.tier.min(6) / 2) as usize;
    for i in 0..stacks {
        let a = i as f32 * 1.9 + 0.6;
        let q = [p[0] + a.cos() * 0.36, p[1] + a.sin() * 0.36];
        let n = 2 + (i % 3);
        for k in 0..n {
            d.cube_lit(
                [q[0], q[1], DECK + 0.03 + k as f32 * 0.055],
                [0.17, 0.17, 0.05],
                a,
                rgba(col, 1.0),
                0.45,
            );
        }
    }
    // Treasury hovers a coin above the roof.
    if tw.fork == Some(0) {
        let bob = (now * 2.0).sin() * 0.06;
        d.cube_lit([p[0], p[1], r2 + 0.34 + bob], [0.20, 0.20, 0.05], now * 2.2, rgba(col, 1.0), 1.0);
    }
    d.glow([p[0], p[1], r2 + 0.10], 0.62, 2.2, rgba(col, 0.22));
}

// ---------------------------------------------------------------- ghost

/// Translucent preview of what you are about to build.
pub fn draw_ghost(d: &mut DrawList, def_i: usize, tier: u32, p: [f32; 2], now: f32) {
    let def = &TOWERS[def_i];
    let col = tower_color(def);
    let ghost = rgba(col, 0.42);

    d.slab(p, [0.90, 0.90], PLOT_TOP + 0.10, 0.12, rgba(col, 0.25));
    // A cheap stand-in with the right proportions, so the footprint reads.
    let h = match def.id {
        "beacon" => 1.05,
        "tesla" => 0.85,
        "ballista" => 0.62,
        "cannon" => 0.36,
        "venom" | "mint" => 0.48,
        _ => 0.60,
    };
    d.cube([p[0], p[1], DECK + h * 0.5], [0.44, 0.44, h], 0.0, ghost);
    d.cube_lit([p[0], p[1], DECK + h + 0.10], [0.36, 0.36, 0.18], now * 0.8, rgba(col, 0.55), 0.9);
    d.glow([p[0], p[1], DECK + h * 0.6], 0.95, 2.0, rgba(col, 0.22));
    let _ = tier;
}
