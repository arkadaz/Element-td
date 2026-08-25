//! Tower models.
//!
//! Each one is built from the shape library, and identity comes from
//! **silhouette**: a ballista's wide bow limb, a cannon's round barrel between
//! two wheels, frost's cluster of leaning shards, a beacon's bare obelisk. You
//! should be able to name any tower from its outline alone, at any level.

use super::PLOT_TOP;
use super::theme;
use crate::game::Tower;
use crate::game::defs::*;
use crate::gfx::draw::{DrawList, Material, Shape, boost, mix, rgba};

/// Top of the stone plinth every tower stands on.
const DECK: f32 = PLOT_TOP + 0.20;

/// Offset a point by a rotated local (forward, right) pair.
fn local(p: [f32; 2], yaw: f32, fwd: f32, right: f32) -> [f32; 2] {
    let (c, s) = (yaw.cos(), yaw.sin());
    [p[0] + c * fwd - s * right, p[1] + s * fwd + c * right]
}

pub fn draw(d: &mut DrawList, tw: &Tower, selected: bool, now: f32) {
    let def = tw.def();
    let col = tower_color(def);
    let p = tw.pos;
    let grow = (((now - tw.built_at) * 4.0).min(1.0)).max(0.05);

    plinth(d, p, col, tw.tier);

    match def.id {
        "ballista" => ballista(d, tw, col, grow),
        "cannon" => cannon(d, tw, col, grow),
        "frost" => frost(d, tw, col, grow, now),
        "pyre" => pyre(d, tw, col, grow, now),
        "tesla" => tesla(d, tw, col, grow, now),
        "venom" => venom(d, tw, col, grow, now),
        "beacon" => beacon(d, tw, col, grow, now),
        "mint" => mint(d, tw, col, grow, now),
        _ => {
            d.cylinder([p[0], p[1], DECK + 0.3], 0.5, 0.6 * grow, 0.0, rgba(col, 1.0), Material::STONE);
        }
    }

    if selected {
        d.ground_ring(p, tw.range(), 0.11, rgba(col, 0.85), 80);
        d.glow([p[0], p[1], 0.55], 1.2, 2.2, rgba(col, 0.16));
    }
}

/// A turned stone drum on a square footing. Courses are added as it levels, so
/// investment reads from across the board.
fn plinth(d: &mut DrawList, p: [f32; 2], col: [f32; 3], tier: u32) {
    let dark = mix(theme::STONE_DARK, col, 0.10);
    d.slab_mat(p, [0.86, 0.86], PLOT_TOP + 0.04, 0.16, rgba(dark, 1.0), Material::STONE);
    d.cylinder(
        [p[0], p[1], PLOT_TOP + 0.12],
        0.76,
        0.16,
        0.0,
        rgba(theme::STONE, 1.0),
        Material::STONE,
    );
    d.cylinder([p[0], p[1], DECK - 0.04], 0.68, 0.10, 0.0, rgba(dark, 1.0), Material::STONE);

    if tier >= 3 {
        for k in 0..6 {
            let a = k as f32 * 1.047;
            d.cylinder(
                [p[0] + a.cos() * 0.34, p[1] + a.sin() * 0.34, DECK + 0.05],
                0.13,
                0.14,
                0.0,
                rgba(dark, 1.0),
                Material::STONE,
            );
        }
    }
    if tier >= 5 {
        for k in 0..3 {
            let a = k as f32 * 2.094 + 0.5;
            d.sphere_lit(
                [p[0] + a.cos() * 0.40, p[1] + a.sin() * 0.40, DECK + 0.16],
                0.13,
                rgba(col, 1.0),
                0.9,
            );
        }
    }
}

// ---------------------------------------------------------------- the eight

/// Timber frame carrying a wide bow limb. The crossbar is the silhouette.
fn ballista(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32) {
    let p = tw.pos;
    let yaw = tw.angle;
    let wood = mix([0.30, 0.21, 0.13], col, 0.22);
    let t = tw.tier as f32;

    let h = (0.36 + 0.05 * t) * grow;
    for r in [-0.19f32, 0.19] {
        let q = local(p, yaw, 0.0, r);
        d.cylinder([q[0], q[1], DECK + h * 0.5], 0.13, h, 0.0, rgba(wood, 1.0), Material::WOOD);
    }
    let top = DECK + h;
    let a = local(p, yaw, 0.0, -0.19);
    let b = local(p, yaw, 0.0, 0.19);
    d.link(Shape::Cylinder, [a[0], a[1], top], [b[0], b[1], top], 0.11, rgba(wood, 1.0), Material::WOOD, 0.0);

    // The bow: two swept limbs from a central hub, with a string across them.
    let limb = (if tw.fork == Some(0) { 1.35 } else { 1.05 }) + 0.04 * t;
    let recoil = tw.flash * 0.10;
    let bz = top + 0.14;
    let hub = local(p, yaw, 0.0, 0.0);
    for s in [-1.0f32, 1.0] {
        let tip = local(p, yaw, -0.16, s * limb * 0.5);
        d.link(
            Shape::Capsule,
            [hub[0], hub[1], bz],
            [tip[0], tip[1], bz],
            0.13,
            rgba(wood, 1.0),
            Material::WOOD,
            0.0,
        );
        d.sphere([tip[0], tip[1], bz], 0.14, rgba(mix(wood, col, 0.55), 1.0), Material::METAL);
    }
    let l = local(p, yaw, -0.16, -limb * 0.5);
    let r = local(p, yaw, -0.16, limb * 0.5);
    d.link(
        Shape::Cylinder,
        [l[0], l[1], bz],
        [r[0], r[1], bz],
        0.022,
        rgba([0.75, 0.72, 0.62], 1.0),
        Material::WOOD,
        0.0,
    );

    // Bolts: a shaft with a real conical head.
    let n = if tw.fork == Some(1) { 3 } else { 1 };
    for i in 0..n {
        let off = (i as f32 - (n as f32 - 1.0) * 0.5) * 0.17;
        let back = local(p, yaw, -0.10 - recoil, off);
        let fwd = local(p, yaw, 0.34 - recoil, off);
        d.link(
            Shape::Cylinder,
            [back[0], back[1], bz + 0.07],
            [fwd[0], fwd[1], bz + 0.07],
            0.055,
            rgba([0.42, 0.34, 0.24], 1.0),
            Material::WOOD,
            0.0,
        );
        d.shape(
            Shape::Cone,
            [fwd[0], fwd[1], bz + 0.07],
            [0.13, 0.13, 0.22],
            yaw,
            std::f32::consts::FRAC_PI_2,
            rgba(col, 1.0),
            Material::METAL,
            0.35,
        );
    }
    if tw.fork == Some(0) {
        let q = local(p, yaw, -0.20, 0.0);
        d.cylinder([q[0], q[1], bz + 0.24], 0.09, 0.22, 0.0, rgba(mix(wood, col, 0.5), 1.0), Material::METAL);
        d.sphere_lit([q[0], q[1], bz + 0.36], 0.10, rgba(col, 1.0), 1.0);
    }
    if tw.flash > 0.0 {
        let m = local(p, yaw, 0.6, 0.0);
        d.glow([m[0], m[1], bz + 0.07], 0.44 * tw.flash, 1.6, boost(rgba(col, tw.flash), 2.2));
    }
}

/// Round barrel between two wheels on a low carriage. Reads as artillery.
fn cannon(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32) {
    let p = tw.pos;
    let yaw = tw.angle;
    let iron = mix([0.19, 0.20, 0.23], col, 0.16);
    let t = tw.tier as f32;

    let h = (0.22 + 0.025 * t) * grow;
    d.cube_mat(
        [p[0], p[1], DECK + h * 0.5],
        [0.62, 0.5, h],
        yaw,
        rgba([0.26, 0.19, 0.13], 1.0),
        Material::WOOD,
    );
    let top = DECK + h;

    // Wheels: cylinders lying on their sides, with iron hubs.
    for s in [-1.0f32, 1.0] {
        let q = local(p, yaw, -0.04, s * 0.34);
        d.shape(
            Shape::Cylinder,
            [q[0], q[1], DECK + 0.20],
            [0.40, 0.40, 0.10],
            yaw,
            std::f32::consts::FRAC_PI_2,
            rgba([0.24, 0.18, 0.12], 1.0),
            Material::WOOD,
            0.0,
        );
        d.shape(
            Shape::Cylinder,
            [q[0], q[1], DECK + 0.20],
            [0.15, 0.15, 0.16],
            yaw,
            std::f32::consts::FRAC_PI_2,
            rgba(iron, 1.0),
            Material::METAL,
            0.0,
        );
    }

    let recoil = tw.flash * 0.16;
    let len = (if tw.fork == Some(0) { 0.98 } else { 0.58 }) + 0.03 * t;
    let girth = if tw.fork == Some(1) { 0.34 } else { 0.26 };
    let pitch = if tw.fork == Some(0) { 0.44 } else { 0.16 };
    let base = local(p, yaw, -0.14 - recoil, 0.0);
    let tip = local(p, yaw, len - recoil, 0.0);
    d.link(
        Shape::Cylinder,
        [base[0], base[1], top + 0.12],
        [tip[0], tip[1], top + 0.12 + len * pitch],
        girth,
        rgba(iron, 1.0),
        Material::DARK_METAL,
        0.0,
    );
    d.sphere([base[0], base[1], top + 0.12], girth * 1.25, rgba(iron, 1.0), Material::DARK_METAL);
    d.shape(
        Shape::Cylinder,
        [tip[0], tip[1], top + 0.12 + len * pitch],
        [girth * 1.3, girth * 1.3, 0.10],
        yaw,
        pitch.atan() + std::f32::consts::FRAC_PI_2,
        rgba(col, 1.0),
        Material::METAL,
        0.35 + tw.flash * 0.5,
    );
    if tw.fork == Some(1) {
        for s in [-1.0f32, 1.0] {
            let q = local(p, yaw, len * 0.72 - recoil, s * 0.17);
            d.link(
                Shape::Cylinder,
                [q[0], q[1], top + 0.12],
                [q[0] + yaw.cos() * 0.26, q[1] + yaw.sin() * 0.26, top + 0.16],
                0.10,
                rgba(iron, 1.0),
                Material::DARK_METAL,
                0.0,
            );
        }
    }
    if tw.flash > 0.0 {
        d.glow(
            [tip[0], tip[1], top + 0.16 + len * pitch],
            0.55 * tw.flash,
            1.5,
            boost(rgba(col, tw.flash), 2.4),
        );
    }
}

/// A cluster of leaning crystal shards growing out of a frozen base.
fn frost(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let ice = mix(col, [1.0, 1.0, 1.0], 0.18);
    let t = tw.tier as f32;
    let n = 3 + (tw.tier.min(6) / 2) as usize;

    d.cylinder(
        [p[0], p[1], DECK + 0.05],
        0.70,
        0.10,
        0.0,
        rgba(mix(ice, [0.08, 0.10, 0.16], 0.62), 1.0),
        Material::GEM,
    );

    for i in 0..n {
        let a = i as f32 * 2.399 + 0.4;
        let r = if i == 0 { 0.0 } else { 0.17 + 0.05 * (i % 3) as f32 };
        let q = [p[0] + a.cos() * r, p[1] + a.sin() * r];
        let h = (if i == 0 { 0.70 } else { 0.34 + 0.12 * ((i * 7) % 4) as f32 })
            * (0.8 + 0.06 * t)
            * grow;
        let w = 0.24 - 0.02 * i as f32;
        let lean = if i == 0 { 0.0 } else { 0.20 + 0.05 * (i % 3) as f32 };
        let sway = (now * 0.8 + i as f32).sin() * 0.02;
        d.shape(
            Shape::Cone,
            [q[0], q[1], DECK + 0.10 + h * 0.5],
            [w, w, h],
            a + sway,
            lean,
            rgba(ice, 0.95),
            Material::GEM,
            0.12,
        );
        d.sphere_lit(
            [
                q[0] + a.cos() * lean * h * 0.5,
                q[1] + a.sin() * lean * h * 0.5,
                DECK + 0.10 + h,
            ],
            w * 0.5,
            rgba(col, 1.0),
            0.8 + tw.flash * 0.2,
        );
    }

    match tw.fork {
        Some(0) => d.glow([p[0], p[1], DECK + 0.3], 1.0, 2.2, rgba(col, 0.30 + tw.flash * 0.3)),
        Some(1) => d.ground_ring(p, 0.66, 0.07, rgba(col, 0.55), 20),
        _ => d.glow([p[0], p[1], DECK + 0.4], 0.72, 2.4, rgba(col, 0.18 + tw.flash * 0.35)),
    }
}

/// An iron brazier on a turned pedestal, with fire licking out of the bowl.
fn pyre(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let iron = rgba([0.16, 0.14, 0.15], 1.0);
    let t = tw.tier as f32;

    let h = (0.28 + 0.04 * t) * grow;
    d.cylinder([p[0], p[1], DECK + h * 0.5], 0.24, h, 0.0, iron, Material::DARK_METAL);
    let top = DECK + h;
    // An inverted cone makes a vessel; a rim finishes it.
    let bowl = 0.62 + 0.02 * t;
    d.shape(
        Shape::Cone,
        [p[0], p[1], top + 0.13],
        [bowl, bowl, 0.30],
        0.0,
        std::f32::consts::PI,
        iron,
        Material::DARK_METAL,
        0.0,
    );
    d.cylinder([p[0], p[1], top + 0.27], bowl, 0.07, 0.0, iron, Material::METAL);
    d.cylinder([p[0], p[1], top + 0.25], bowl * 0.82, 0.05, 0.0, rgba(col, 1.0), Material::GEM);

    let flames = if tw.fork == Some(0) { 5 } else { 3 };
    for i in 0..flames {
        let a = i as f32 * 2.1;
        let r = 0.11 + 0.05 * (i % 2) as f32;
        let wob = (now * 6.0 + i as f32 * 1.7).sin();
        let fh = 0.24 + 0.12 * (wob * 0.5 + 0.5) + 0.02 * t;
        d.shape(
            Shape::Cone,
            [p[0] + a.cos() * r, p[1] + a.sin() * r, top + 0.28 + fh * 0.5],
            [0.17, 0.17, fh],
            a,
            wob * 0.12,
            rgba(mix(col, [1.0, 0.94, 0.55], 0.35), 1.0),
            Material::GEM,
            1.0,
        );
    }
    d.glow([p[0], p[1], top + 0.5], 1.0 + tw.flash * 0.3, 1.9, rgba(col, 0.42));
    if tw.fork == Some(1) && tw.ramp > 0.0 {
        d.glow([p[0], p[1], top + 0.45], 0.85 + tw.ramp * 0.5, 2.0, rgba([1.0, 0.85, 0.4], 0.35));
    }
}

/// A coil: stacked ring plates up a column, with a charged orb on top.
fn tesla(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let metal = rgba([0.21, 0.23, 0.27], 1.0);
    let t = tw.tier as f32;
    let h = (0.64 + 0.07 * t) * grow;

    d.cylinder([p[0], p[1], DECK + h * 0.5], 0.17, h, 0.0, metal, Material::DARK_METAL);

    let rings = 2 + (tw.tier.min(6) / 2) as usize;
    for i in 0..rings {
        let f = 1.0 - i as f32 * 0.17;
        let z = DECK + h * (0.28 + 0.22 * i as f32).min(0.93);
        let spin = now * (0.5 + 0.2 * i as f32) * if i % 2 == 0 { 1.0 } else { -1.0 };
        d.cylinder([p[0], p[1], z], 0.60 * f, 0.055, spin, metal, Material::METAL);
        for k in 0..6 {
            let a = spin + k as f32 * 1.047;
            d.sphere_lit(
                [p[0] + a.cos() * 0.30 * f, p[1] + a.sin() * 0.30 * f, z + 0.03],
                0.09,
                rgba(col, 1.0),
                0.7,
            );
        }
    }

    let charge = (1.0 - tw.cooldown.max(0.0) * tw.rate().max(0.1)).clamp(0.0, 1.0);
    let orb = 0.30 + 0.08 * charge;
    d.sphere_lit([p[0], p[1], DECK + h + orb * 0.5], orb, rgba(col, 1.0), 0.6 + charge * 0.4);
    d.glow(
        [p[0], p[1], DECK + h + orb * 0.5],
        0.6 + charge * 0.35 + tw.flash * 0.4,
        2.0,
        rgba(col, 0.25 + charge * 0.25),
    );
    if tw.fork == Some(0) {
        for s in [-1.0f32, 1.0] {
            let q = local(p, now * 0.8, 0.0, s * 0.32);
            d.link(
                Shape::Cylinder,
                [q[0], q[1], DECK + h * 0.72],
                [q[0], q[1], DECK + h * 1.02],
                0.06,
                metal,
                Material::METAL,
                0.0,
            );
            d.sphere_lit([q[0], q[1], DECK + h * 1.02], 0.13, rgba(col, 1.0), 1.0);
        }
    }
}

/// A bellied cauldron of venom with spouts out of the front.
fn venom(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let brass = rgba(mix([0.28, 0.24, 0.16], col, 0.16), 1.0);
    let t = tw.tier as f32;

    let h = (0.44 + 0.035 * t) * grow;
    d.shape(
        Shape::Sphere,
        [p[0], p[1], DECK + h * 0.55],
        [0.76, 0.76, h * 1.25],
        0.0,
        0.0,
        brass,
        Material::METAL,
        0.0,
    );
    let top = DECK + h * 1.05;
    d.cylinder([p[0], p[1], top], 0.62, 0.08, 0.0, brass, Material::METAL);
    d.cylinder([p[0], p[1], top + 0.03], 0.54, 0.04, 0.0, rgba(col, 1.0), Material::WATER);

    for i in 0..3 {
        let ph = (now * (0.8 + 0.2 * i as f32) + i as f32 * 2.0).rem_euclid(1.0);
        let a = i as f32 * 2.3;
        let s = 0.16 * (1.0 - ph);
        if s > 0.02 {
            d.sphere_lit(
                [p[0] + a.cos() * 0.16, p[1] + a.sin() * 0.16, top + 0.04 + ph * 0.22],
                s,
                rgba(mix(col, [1.0, 1.0, 1.0], 0.3), 1.0),
                0.9,
            );
        }
    }

    let yaw = tw.angle;
    let n = if tw.fork == Some(0) { 3 } else { 2 };
    for i in 0..n {
        let off = (i as f32 - (n as f32 - 1.0) * 0.5) * 0.22;
        let a = local(p, yaw, 0.20, off);
        let b = local(p, yaw, 0.58, off * 1.35);
        d.link(
            Shape::Cylinder,
            [a[0], a[1], top - 0.04],
            [b[0], b[1], top + 0.16],
            0.10,
            brass,
            Material::METAL,
            0.0,
        );
        d.sphere_lit([b[0], b[1], top + 0.16], 0.11, rgba(col, 1.0), 0.9);
    }
    d.glow([p[0], p[1], top + 0.12], 0.72, 2.2, rgba(col, 0.22 + tw.flash * 0.3));
}

/// A bare tapered obelisk with a ring of stones orbiting it. Never shoots.
fn beacon(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let stone = rgba(mix(theme::STONE, col, 0.22), 1.0);
    let t = tw.tier as f32;
    let h = (1.00 + 0.10 * t) * grow;

    d.shape(
        Shape::Cone,
        [p[0], p[1], DECK + h * 0.5],
        [0.44, 0.44, h * 1.35],
        0.0,
        0.0,
        stone,
        Material::STONE,
        0.0,
    );
    let z = DECK + h;
    d.shape(
        Shape::Prism,
        [p[0], p[1], z + 0.16],
        [0.30, 0.30, 0.34],
        now * 0.9,
        0.0,
        rgba(col, 1.0),
        Material::GEM,
        1.0,
    );

    let orbit = 0.46 + 0.02 * t;
    for i in 0..4 {
        let a = now * 1.1 + i as f32 * std::f32::consts::FRAC_PI_2;
        let bob = (now * 1.6 + i as f32).sin() * 0.06;
        d.shape(
            Shape::Prism,
            [p[0] + a.cos() * orbit, p[1] + a.sin() * orbit, z * 0.70 + bob],
            [0.17, 0.17, 0.14],
            a,
            0.0,
            rgba(col, 1.0),
            Material::GEM,
            0.9,
        );
    }
    d.glow([p[0], p[1], z + 0.18], 0.9, 2.0, rgba(col, 0.35));
}

/// A round vault with a conical roof and coin stacks around the base.
fn mint(d: &mut DrawList, tw: &Tower, col: [f32; 3], grow: f32, now: f32) {
    let p = tw.pos;
    let wall = rgba([0.26, 0.21, 0.15], 1.0);
    let t = tw.tier as f32;

    let h = (0.40 + 0.03 * t) * grow;
    d.cylinder([p[0], p[1], DECK + h * 0.5], 0.66, h, 0.0, wall, Material::WOOD);
    let top = DECK + h;
    d.cylinder(
        [p[0], p[1], top + 0.04],
        0.74,
        0.08,
        0.0,
        rgba(mix([0.26, 0.21, 0.15], col, 0.35), 1.0),
        Material::METAL,
    );
    d.cone(
        [p[0], p[1], top + 0.28],
        0.78,
        0.40,
        0.0,
        rgba(mix([0.22, 0.18, 0.13], col, 0.25), 1.0),
        Material::WOOD,
    );
    d.sphere_lit([p[0], p[1], top + 0.52], 0.16, rgba(col, 1.0), 1.0);

    // Vault door on the near face.
    d.shape(
        Shape::Cylinder,
        [p[0], p[1] - 0.60, DECK + h * 0.55],
        [0.30, 0.30, 0.10],
        0.0,
        std::f32::consts::FRAC_PI_2,
        rgba(col, 1.0),
        Material::METAL,
        0.25,
    );

    let stacks = 2 + (tw.tier.min(6) / 2) as usize;
    for i in 0..stacks {
        let a = i as f32 * 1.9 + 0.6;
        let c = [p[0] + a.cos() * 0.44, p[1] + a.sin() * 0.44];
        let n = 2 + (i % 3);
        for k in 0..n {
            d.cylinder(
                [c[0], c[1], DECK + 0.04 + k as f32 * 0.055],
                0.20,
                0.05,
                a,
                rgba(col, 1.0),
                Material::METAL,
            );
        }
    }
    if tw.fork == Some(0) {
        let bob = (now * 2.0).sin() * 0.06;
        d.shape(
            Shape::Cylinder,
            [p[0], p[1], top + 0.78 + bob],
            [0.26, 0.26, 0.05],
            now * 2.2,
            0.24,
            rgba(col, 1.0),
            Material::METAL,
            0.8,
        );
    }
    d.glow([p[0], p[1], top + 0.3], 0.64, 2.2, rgba(col, 0.22));
}

// ---------------------------------------------------------------- ghost

/// Translucent preview of what is about to be built.
pub fn draw_ghost(d: &mut DrawList, def_i: usize, tier: u32, p: [f32; 2], now: f32) {
    let def = &TOWERS[def_i];
    let col = tower_color(def);

    d.cylinder([p[0], p[1], PLOT_TOP + 0.08], 0.78, 0.14, 0.0, rgba(col, 0.28), Material::STONE);
    let (h, shape) = match def.id {
        "beacon" => (1.10, Shape::Cone),
        "tesla" => (0.90, Shape::Cylinder),
        "ballista" => (0.66, Shape::Box),
        "cannon" => (0.40, Shape::Cylinder),
        "mint" => (0.55, Shape::Cylinder),
        "venom" => (0.52, Shape::Sphere),
        "pyre" => (0.60, Shape::Cylinder),
        _ => (0.62, Shape::Cylinder),
    };
    d.shape(
        shape,
        [p[0], p[1], DECK + h * 0.5],
        [0.56, 0.56, h],
        0.0,
        0.0,
        rgba(col, 0.40),
        Material::GEM,
        0.25,
    );
    d.sphere_lit([p[0], p[1], DECK + h + 0.14], 0.28, rgba(col, 0.55), 0.9);
    d.glow([p[0], p[1], DECK + h * 0.6], 1.0, 2.0, rgba(col, 0.22));
    let _ = (tier, now);
}
