//! Monster models.
//!
//! Armour type is carried by colour, so **shape** has to carry the identity: the
//! Brute is a boulder on stumpy legs, the Runner a leaning dart, the Warden and
//! Mender float with no legs at all, the Bulwark hides behind a plate that
//! visibly breaks. You should know what is coming before you read the HUD.
//!
//! Bodies are spheres, limbs are capsules, horns and spines are cones - nothing
//! here is a box that could have been a creature.

use super::theme;
use crate::game::Creep;
use crate::game::defs::Kind;
use crate::gfx::draw::{Color, DrawList, Material, Shape, mix, rgba};

/// Offset a point by a rotated local (forward, right) pair.
fn local(p: [f32; 2], yaw: f32, fwd: f32, right: f32) -> [f32; 2] {
    let (c, s) = (yaw.cos(), yaw.sin());
    [p[0] + c * fwd - s * right, p[1] + s * fwd + c * right]
}

pub fn draw(d: &mut DrawList, c: &Creep) {
    let base = c.armor.color();
    let body_rgb = mix(base, [1.0, 1.0, 1.0], c.flash * 0.75);
    let body = rgba(body_rgb, 1.0);
    let dark = rgba(mix(body_rgb, [0.05, 0.05, 0.08], 0.45), 1.0);
    let r = c.radius;

    // Contact shadow - a flat disc, so it reads as a shadow not a plate. A
    // flyer's is smaller and fainter, which is most of what tells you at a
    // glance that it is out of a mortar's reach.
    let hovers = matches!(c.kind, Kind::Warden | Kind::Mender | Kind::Phaser);
    let (spread, alpha) = if c.flying() {
        (1.5, 0.16)
    } else if hovers {
        (2.2, 0.20)
    } else {
        (2.4, 0.36)
    };
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

    match c.kind {
        Kind::Grunt => grunt(d, c, body, dark),
        Kind::Runner => runner(d, c, body, dark),
        Kind::Brute => brute(d, c, body, dark),
        Kind::Swarm => swarm(d, c, body, dark),
        Kind::Warden => warden(d, c, body, dark),
        Kind::Mender => mender(d, c, body, dark),
        Kind::Bulwark => bulwark(d, c, body, dark),
        Kind::Phaser => phaser(d, c, body, dark),
        Kind::Boss => boss(d, c, body, dark),
        Kind::Wisp => wisp(d, c, body, dark),
        Kind::Drake => drake(d, c, body, dark),
        Kind::Skylord => skylord(d, c, body, dark),
    }

    status(d, c);
    health_bar(d, c);
}

// ---------------------------------------------------------------- shared parts

/// Capsule legs stepping in diagonal pairs.
fn legs(d: &mut DrawList, c: &Creep, col: Color, count: usize, spread: f32, len: f32) {
    let walking = c.stun <= 0.0;
    let r = c.radius;
    for i in 0..count {
        let fwd = if i < 2 { 1.0 } else { -1.0 };
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let phase = c.bob * 2.0 + (i as f32) * std::f32::consts::FRAC_PI_2;
        let lift = if walking { (phase.sin() * 0.5 + 0.5) * r * 0.30 } else { 0.0 };
        // Legs swing forward as they lift, so the gait reads.
        let swing = if walking { phase.cos() * r * 0.18 } else { 0.0 };
        let hip = local(c.pos, c.facing, fwd * r * spread, side * r * spread);
        let foot = local(
            c.pos,
            c.facing,
            fwd * r * spread + swing,
            side * r * spread,
        );
        d.link(
            Shape::Capsule,
            [hip[0], hip[1], len + lift],
            [foot[0], foot[1], lift * 0.5],
            r * 0.26,
            col,
            Material::CHITIN,
            0.0,
        );
    }
}

fn eyes(d: &mut DrawList, c: &Creep, at: [f32; 2], z: f32, size: f32, spread: f32) {
    for s in [-1.0f32, 1.0] {
        let e = local(at, c.facing, size * 0.55, s * spread);
        d.sphere_lit([e[0], e[1], z], size * 0.42, rgba([1.0, 0.90, 0.68], 1.0), 1.0);
    }
}

/// A rounded body: one squashed sphere, which is the whole difference between
/// "creature" and "crate".
#[allow(clippy::too_many_arguments)]
fn body_blob(d: &mut DrawList, c: &Creep, z: f32, l: f32, w: f32, h: f32, col: Color, mat: Material) {
    d.shape(
        Shape::Sphere,
        [c.pos[0], c.pos[1], z],
        [l, w, h],
        c.facing,
        0.0,
        col,
        mat,
        0.0,
    );
}

// ---------------------------------------------------------------- the nine

/// The baseline: a rounded body on four legs with a blunt snout.
fn grunt(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.60;
    let bz = leg + r * 0.75;
    legs(d, c, dark, 4, 0.55, leg);
    body_blob(d, c, bz, r * 1.9, r * 1.6, r * 1.5, col, Material::CHITIN);
    let h = local(c.pos, c.facing, r * 0.95, 0.0);
    d.shape(
        Shape::Sphere,
        [h[0], h[1], bz + r * 0.28],
        [r * 1.0, r * 0.9, r * 0.85],
        c.facing,
        0.0,
        dark,
        Material::CHITIN,
        0.0,
    );
    eyes(d, c, h, bz + r * 0.40, r * 0.46, r * 0.26);
}

/// Long, low and pitched nose-down, with two legs and a streaming tail.
fn runner(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.62;
    let bz = leg + r * 0.58;
    legs(d, c, dark, 2, 0.40, leg);
    d.shape(
        Shape::Sphere,
        [c.pos[0], c.pos[1], bz],
        [r * 2.5, r * 1.05, r * 1.0],
        c.facing,
        -0.22,
        col,
        Material::CHITIN,
        0.0,
    );
    let h = local(c.pos, c.facing, r * 1.2, 0.0);
    d.shape(
        Shape::Cone,
        [h[0], h[1], bz - r * 0.16],
        [r * 0.75, r * 0.7, r * 0.9],
        c.facing,
        std::f32::consts::FRAC_PI_2,
        dark,
        Material::CHITIN,
        0.0,
    );
    eyes(d, c, h, bz - r * 0.05, r * 0.4, r * 0.20);
    // Tail swinging with the stride.
    let sway = (c.bob * 2.0).sin() * 0.4;
    let t0 = local(c.pos, c.facing, -r * 0.9, 0.0);
    let t1 = local(c.pos, c.facing + sway, -r * 2.2, 0.0);
    d.link(
        Shape::Capsule,
        [t0[0], t0[1], bz + r * 0.1],
        [t1[0], t1[1], bz + r * 0.55],
        r * 0.20,
        col,
        Material::CHITIN,
        0.25,
    );
}

/// A boulder of muscle: wide, low, plated, on stumpy legs.
fn brute(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.40;
    let bz = leg + r * 0.90;
    legs(d, c, dark, 4, 0.64, leg);
    body_blob(d, c, bz, r * 2.0, r * 2.0, r * 1.7, col, Material::CHITIN);
    // Shoulder plates: capsules laid across the back.
    for s in [-1.0f32, 1.0] {
        let a = local(c.pos, c.facing, r * 0.5, s * r * 0.95);
        let b = local(c.pos, c.facing, -r * 0.5, s * r * 1.05);
        d.link(
            Shape::Capsule,
            [a[0], a[1], bz + r * 0.55],
            [b[0], b[1], bz + r * 0.55],
            r * 0.42,
            rgba(
                [
                    dark[0] + (1.0 - dark[0]) * 0.18,
                    dark[1] + (1.0 - dark[1]) * 0.18,
                    dark[2] + (1.0 - dark[2]) * 0.18,
                ],
                1.0,
            ),
            Material::METAL,
            0.0,
        );
    }
    let h = local(c.pos, c.facing, r * 0.9, 0.0);
    d.sphere([h[0], h[1], bz + r * 0.25], r * 0.85, dark, Material::CHITIN);
    eyes(d, c, h, bz + r * 0.33, r * 0.36, r * 0.20);
}

/// A skittering insect: segmented body, four legs, twitching antennae. Tiny,
/// but unmistakably alive - these arrive forty at a time.
fn swarm(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let jitter = (c.bob * 3.0).sin() * r * 0.12;
    let bz = r * 0.90 + jitter;
    legs(d, c, dark, 4, 0.50, r * 0.45);

    // Abdomen behind, thorax in front - two segments, not one blob.
    let ab = local(c.pos, c.facing, -r * 0.55, 0.0);
    d.shape(
        Shape::Sphere,
        [ab[0], ab[1], bz],
        [r * 1.5, r * 1.3, r * 1.2],
        c.facing,
        0.0,
        col,
        Material::CHITIN,
        0.0,
    );
    let th = local(c.pos, c.facing, r * 0.35, 0.0);
    d.sphere([th[0], th[1], bz + r * 0.10], r * 1.15, dark, Material::CHITIN);
    eyes(d, c, th, bz + r * 0.22, r * 0.44, r * 0.22);

    // Antennae, flicking with the gait.
    for sd in [-1.0f32, 1.0] {
        let base = local(c.pos, c.facing, r * 0.75, sd * r * 0.20);
        let tip = local(c.pos, c.facing + sd * (c.bob * 3.0).sin() * 0.25, r * 1.5, sd * r * 0.55);
        d.link(
            Shape::Cone,
            [base[0], base[1], bz + r * 0.30],
            [tip[0], tip[1], bz + r * 0.70],
            r * 0.16,
            dark,
            Material::CHITIN,
            0.0,
        );
    }
}

/// Floats, robed, with runes orbiting it. No legs at all.
fn warden(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let hover = (c.bob * 1.4).sin() * r * 0.12;
    let bz = r * 1.35 + hover;
    // A robe is a cone; a hood is a sphere. Neither is a box.
    d.shape(
        Shape::Cone,
        [c.pos[0], c.pos[1], bz - r * 0.35],
        [r * 2.0, r * 1.9, r * 1.7],
        c.facing,
        0.0,
        col,
        Material::CHITIN,
        0.0,
    );
    let h = local(c.pos, c.facing, r * 0.12, 0.0);
    d.sphere([h[0], h[1], bz + r * 0.72], r * 0.95, dark, Material::CHITIN);
    eyes(d, c, h, bz + r * 0.72, r * 0.42, r * 0.20);
    // Orbiting wards: the tell that magic bounces off it.
    for i in 0..3 {
        let a = c.bob * 0.9 + i as f32 * 2.094;
        let q = [c.pos[0] + a.cos() * r * 1.6, c.pos[1] + a.sin() * r * 1.6];
        d.shape(
            Shape::Prism,
            [q[0], q[1], bz + r * 0.25],
            [r * 0.34, r * 0.34, r * 0.14],
            a,
            0.0,
            rgba(c.armor.color(), 1.0),
            Material::GEM,
            1.0,
        );
    }
}

/// A floating orb inside a halo, with a heal pulse washing the ground.
fn mender(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let hover = (c.bob * 1.6).sin() * r * 0.14;
    let bz = r * 1.55 + hover;
    // Robe tapering to nothing below.
    d.shape(
        Shape::Cone,
        [c.pos[0], c.pos[1], bz - r * 0.55],
        [r * 1.5, r * 1.5, r * 1.9],
        0.0,
        std::f32::consts::PI,
        dark,
        Material::CHITIN,
        0.0,
    );
    d.sphere([c.pos[0], c.pos[1], bz + r * 0.30], r * 1.35, col, Material::GEM);
    // Halo ring.
    let spin = c.bob * 1.2;
    for i in 0..6 {
        let a = spin + i as f32 * 1.047;
        let q = [c.pos[0] + a.cos() * r * 1.2, c.pos[1] + a.sin() * r * 1.2];
        d.shape(
            Shape::Capsule,
            [q[0], q[1], bz + r * 1.0],
            [r * 0.30, r * 0.13, r * 0.13],
            a + std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
            rgba([0.55, 1.0, 0.70], 1.0),
            Material::GEM,
            1.0,
        );
    }
    // The heal aura: impossible to miss, so it can be focused down.
    let pulse = (c.bob * 1.8).sin() * 0.5 + 0.5;
    d.ground_ring(c.pos, 2.6 * (0.85 + pulse * 0.15), 0.09, rgba([0.45, 1.0, 0.62], 0.45), 40);
    d.glow(
        [c.pos[0], c.pos[1], bz],
        r * 3.2,
        2.2,
        rgba([0.45, 1.0, 0.62], 0.25 + pulse * 0.12),
    );
}

/// Hides behind a curved shield that visibly breaks as it soaks damage.
fn bulwark(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.45;
    let bz = leg + r * 0.85;
    legs(d, c, dark, 4, 0.55, leg);
    body_blob(d, c, bz, r * 1.7, r * 1.7, r * 1.6, col, Material::CHITIN);
    let h = local(c.pos, c.facing, r * 0.55, 0.0);
    d.sphere([h[0], h[1], bz + r * 0.72], r * 0.8, dark, Material::CHITIN);
    eyes(d, c, h, bz + r * 0.75, r * 0.34, r * 0.18);

    let frac = if c.max_shield > 0.0 { (c.shield / c.max_shield).clamp(0.0, 1.0) } else { 0.0 };
    if frac > 0.0 {
        let q = local(c.pos, c.facing, r * 1.3, 0.0);
        let hgt = r * (1.2 + 1.5 * frac);
        // A curved pavise, not a slab: a squashed sphere makes it bow outwards.
        d.shape(
            Shape::Sphere,
            [q[0], q[1], bz + r * 0.1],
            [r * 0.45, r * 2.2, hgt],
            c.facing,
            0.0,
            rgba(mix([0.55, 0.75, 1.0], [1.0, 1.0, 1.0], 1.0 - frac), 0.42 + 0.45 * frac),
            Material::GEM,
            0.35,
        );
        d.glow([q[0], q[1], bz + r * 0.2], r * 2.4, 2.0, rgba([0.5, 0.72, 1.0], 0.20 * frac));
    } else {
        // Broken: only the boss of the shield remains, hanging off the arm.
        let q = local(c.pos, c.facing, r * 1.05, 0.0);
        d.sphere([q[0], q[1], bz - r * 0.25], r * 0.55, dark, Material::METAL);
    }
}

/// Half-there: a narrow body with an after-image that separates while phasing.
fn phaser(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let hover = (c.bob * 2.2).sin() * r * 0.10;
    let bz = r * 1.2 + hover;
    let ghosting = c.slow_off;
    let a = if ghosting { 0.55 } else { 1.0 };

    d.shape(
        Shape::Capsule,
        [c.pos[0], c.pos[1], bz],
        [r * 1.25, r * 1.25, r * 2.2],
        c.facing,
        0.0,
        [col[0], col[1], col[2], a],
        Material::GEM,
        0.15,
    );
    let h = local(c.pos, c.facing, r * 0.3, 0.0);
    d.sphere([h[0], h[1], bz + r * 1.0], r * 0.85, [dark[0], dark[1], dark[2], a], Material::GEM);
    eyes(d, c, h, bz + r * 1.0, r * 0.4, r * 0.20);

    // After-image, strongest while it is ignoring slows.
    let lag = if ghosting { r * 1.2 } else { r * 0.5 };
    let t = local(c.pos, c.facing, -lag, 0.0);
    d.shape(
        Shape::Capsule,
        [t[0], t[1], bz],
        [r * 1.05, r * 1.05, r * 2.0],
        c.facing,
        0.0,
        [col[0], col[1], col[2], if ghosting { 0.30 } else { 0.12 }],
        Material::GEM,
        0.8,
    );
    if ghosting {
        d.glow([c.pos[0], c.pos[1], bz], r * 3.0, 2.2, rgba(c.armor.color(), 0.30));
    }
}

/// Enormous, crowned, horned, spined. Should stop the player mid-sentence.
fn boss(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.52;
    let bz = leg + r * 1.05;
    legs(d, c, dark, 4, 0.62, leg);

    body_blob(d, c, bz, r * 2.2, r * 2.0, r * 1.9, col, Material::CHITIN);
    // Shoulder armour.
    for s in [-1.0f32, 1.0] {
        let a = local(c.pos, c.facing, r * 0.5, s * r * 1.0);
        let b = local(c.pos, c.facing, -r * 0.6, s * r * 1.1);
        d.link(
            Shape::Capsule,
            [a[0], a[1], bz + r * 0.7],
            [b[0], b[1], bz + r * 0.7],
            r * 0.5,
            dark,
            Material::METAL,
            0.0,
        );
    }
    // Back spines: real cones, tallest at the shoulders.
    for k in 0..4 {
        let off = -0.55 + k as f32 * 0.32;
        let q = local(c.pos, c.facing, r * off, 0.0);
        let hgt = r * (0.55 + 0.2 * (3 - k.min(3)) as f32);
        d.cone(
            [q[0], q[1], bz + r * 1.05 + hgt * 0.5],
            r * 0.34,
            hgt,
            c.facing,
            rgba(c.armor.color(), 1.0),
            Material::GEM,
        );
    }
    let h = local(c.pos, c.facing, r * 1.05, 0.0);
    d.sphere([h[0], h[1], bz + r * 0.5], r * 1.15, dark, Material::CHITIN);
    eyes(d, c, h, bz + r * 0.6, r * 0.55, r * 0.30);
    // Horns sweeping back.
    for s in [-1.0f32, 1.0] {
        let a = local(c.pos, c.facing, r * 0.85, s * r * 0.55);
        let b = local(c.pos, c.facing, r * 0.35, s * r * 0.95);
        d.link(
            Shape::Cone,
            [a[0], a[1], bz + r * 1.05],
            [b[0], b[1], bz + r * 1.85],
            r * 0.30,
            dark,
            Material::METAL,
            0.0,
        );
    }
    d.glow([c.pos[0], c.pos[1], bz], r * 3.4, 2.0, rgba(c.armor.color(), 0.35));
}

// ---------------------------------------------------------------- overlays

fn status(d: &mut DrawList, c: &Creep) {
    let r = c.radius;
    let bz = c.height();
    if c.slow.t > 0.0 && !c.slow_off {
        d.glow([c.pos[0], c.pos[1], bz], r * 2.6, 1.8, rgba([0.45, 0.80, 1.0], 0.32));
    }
    if c.stun > 0.0 {
        // A ring of sparks spinning overhead.
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
    if c.burn.t > 0.0 {
        d.glow([c.pos[0], c.pos[1], bz + r * 0.5], r * 2.8, 1.7, rgba([1.0, 0.45, 0.12], 0.42));
    }
    if c.poison.t > 0.0 {
        d.glow([c.pos[0], c.pos[1], bz + r * 0.5], r * 2.6, 1.7, rgba([0.45, 1.0, 0.35], 0.36));
    }
    if c.shred.t > 0.0 {
        d.ground_ring(c.pos, r * 1.9, 0.06, rgba([0.85, 0.45, 1.0], 0.65), 14);
    }
}

fn health_bar(d: &mut DrawList, c: &Creep) {
    let hp = c.hp_frac();
    if hp >= 0.999 && c.shield <= 0.0 {
        return;
    }
    let r = c.radius;
    let w = (r * 2.6).max(0.44);
    let bar_z = c.height() + r * 1.8 + 0.25;
    // Flat quads, so bars never catch a specular highlight and shimmer.
    d.shape(
        Shape::Quad,
        [c.pos[0], c.pos[1], bar_z],
        [w + 0.05, 0.13, 1.0],
        0.0,
        0.0,
        theme::HP_BACK,
        Material::EARTH,
        0.35,
    );
    let fill = if hp > 0.35 { theme::HP_FILL } else { theme::HP_LOW };
    d.shape(
        Shape::Quad,
        [c.pos[0] - w * 0.5 * (1.0 - hp), c.pos[1], bar_z + 0.015],
        [w * hp, 0.12, 1.0],
        0.0,
        0.0,
        fill,
        Material::EARTH,
        0.9,
    );
    if c.max_shield > 0.0 && c.shield > 0.0 {
        let sf = (c.shield / c.max_shield).clamp(0.0, 1.0);
        d.shape(
            Shape::Quad,
            [c.pos[0] - w * 0.5 * (1.0 - sf), c.pos[1], bar_z + 0.14],
            [w * sf, 0.10, 1.0],
            0.0,
            0.0,
            [0.50, 0.74, 1.0, 1.0],
            Material::EARTH,
            0.9,
        );
    }
}

// ---------------------------------------------------------------- the air

/// Beating wings, drawn as a pair of swept capsule spars with a membrane
/// between them. The flap angle is what sells a flyer at gameplay zoom - a
/// static silhouette in the air reads as a bug, not a bird.
#[allow(clippy::too_many_arguments)]
fn wings(
    d: &mut DrawList,
    c: &Creep,
    at: [f32; 3],
    span: f32,
    chord: f32,
    beat: f32,
    col: Color,
    membrane: Color,
) {
    let flap = (c.bob * beat).sin();
    for s in [-1.0f32, 1.0] {
        // Shoulder, elbow, tip: a real wing has a bend in it.
        let elbow = local([at[0], at[1]], c.facing, chord * 0.25, s * span * 0.45);
        let tip = local([at[0], at[1]], c.facing, -chord * 0.35, s * span);
        let ez = at[2] + flap * span * 0.30;
        let tz = at[2] + flap * span * 0.55;
        d.link(Shape::Capsule, at, [elbow[0], elbow[1], ez], chord * 0.16, col, Material::CHITIN, 0.0);
        d.link(
            Shape::Capsule,
            [elbow[0], elbow[1], ez],
            [tip[0], tip[1], tz],
            chord * 0.12,
            col,
            Material::CHITIN,
            0.0,
        );
        // Membrane: a thin squashed sphere spanning shoulder to tip.
        let mid = [(at[0] + tip[0]) * 0.5, (at[1] + tip[1]) * 0.5];
        d.shape(
            Shape::Sphere,
            [mid[0], mid[1], (at[2] + tz) * 0.5],
            [chord * 1.15, span * 0.95, chord * 0.10],
            c.facing + s * flap * 0.18,
            flap * 0.35 * s,
            membrane,
            Material::GEM,
            0.0,
        );
    }
}

/// A mote of light with a comet tail. Fragile, fast, and arrives in a cloud -
/// forty of these is the wave that teaches you to build anti-air.
fn wisp(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let z = c.height();
    let pulse = (c.bob * 4.0).sin() * 0.5 + 0.5;

    d.sphere_lit([c.pos[0], c.pos[1], z], r * 1.7 + pulse * r * 0.25, col, 0.85);
    d.sphere([c.pos[0], c.pos[1], z], r * 2.5, [col[0], col[1], col[2], 0.22], Material::GEM);
    // Three motes orbiting the core, so it shimmers rather than sitting still.
    for i in 0..3 {
        let a = c.bob * 2.4 + i as f32 * 2.094;
        d.sphere_lit(
            [c.pos[0] + a.cos() * r * 1.5, c.pos[1] + a.sin() * r * 1.5, z + (a * 2.0).sin() * r * 0.5],
            r * 0.42,
            dark,
            1.0,
        );
    }
    // Tail, fading behind it along the road.
    for k in 1..4 {
        let t = k as f32;
        let p = local(c.pos, c.facing, -r * 1.1 * t, 0.0);
        d.sphere(
            [p[0], p[1], z - t * r * 0.1],
            r * (1.3 - t * 0.3),
            [col[0], col[1], col[2], 0.32 / t],
            Material::GEM,
        );
    }
    d.glow([c.pos[0], c.pos[1], z], r * 4.0, 2.0, [col[0], col[1], col[2], 0.34]);
}

/// A plated flying serpent: long body, real wings, horned head. Heavy armour,
/// so bringing only physical anti-air is not enough.
fn drake(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let z = c.height();

    // Body: three segments along the heading, so it flexes rather than gliding
    // as one rigid lozenge.
    let sway = (c.bob * 1.8).sin();
    for (i, (fwd, w, hgt)) in [(0.55, 1.05, 0.95), (-0.15, 1.25, 1.10), (-0.85, 0.85, 0.80)]
        .iter()
        .enumerate()
    {
        let bend = sway * 0.16 * i as f32;
        let p = local(c.pos, c.facing + bend, r * fwd, 0.0);
        d.shape(
            Shape::Sphere,
            [p[0], p[1], z + bend * r * 0.3],
            [r * 1.5, r * w, r * hgt],
            c.facing + bend,
            0.0,
            col,
            Material::CHITIN,
            0.0,
        );
    }

    wings(
        d,
        c,
        [c.pos[0], c.pos[1], z + r * 0.35],
        r * 3.0,
        r * 1.1,
        3.4,
        dark,
        [col[0] * 0.8, col[1] * 0.85, col[2] * 0.95, 0.72],
    );

    // Neck and head.
    let neck = local(c.pos, c.facing, r * 1.25, 0.0);
    let head = local(c.pos, c.facing, r * 2.0, 0.0);
    d.link(
        Shape::Capsule,
        [neck[0], neck[1], z + r * 0.1],
        [head[0], head[1], z + r * 0.45],
        r * 0.42,
        col,
        Material::CHITIN,
        0.0,
    );
    d.shape(
        Shape::Sphere,
        [head[0], head[1], z + r * 0.45],
        [r * 1.1, r * 0.75, r * 0.7],
        c.facing,
        0.0,
        dark,
        Material::CHITIN,
        0.0,
    );
    eyes(d, c, head, z + r * 0.55, r * 0.36, r * 0.20);
    for s in [-1.0f32, 1.0] {
        let a = local(c.pos, c.facing, r * 1.9, s * r * 0.3);
        let b = local(c.pos, c.facing, r * 1.2, s * r * 0.6);
        d.link(
            Shape::Cone,
            [a[0], a[1], z + r * 0.7],
            [b[0], b[1], z + r * 1.2],
            r * 0.22,
            dark,
            Material::METAL,
            0.0,
        );
    }
    // Tail, tapering to a barb.
    let t0 = local(c.pos, c.facing, -r * 1.3, 0.0);
    let t1 = local(c.pos, c.facing + sway * 0.5, -r * 2.6, 0.0);
    d.link(Shape::Capsule, [t0[0], t0[1], z], [t1[0], t1[1], z - r * 0.2], r * 0.26, col, Material::CHITIN, 0.0);
    d.cone([t1[0], t1[1], z - r * 0.3], r * 0.4, r * 0.7, c.facing, dark, Material::METAL);
}

/// The air boss: enormous wings, a crown, and a mantle of orbiting shards.
fn skylord(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let z = c.height();

    d.shape(
        Shape::Sphere,
        [c.pos[0], c.pos[1], z],
        [r * 1.5, r * 1.6, r * 2.0],
        c.facing,
        0.0,
        col,
        Material::CHITIN,
        0.0,
    );
    // Robed lower body tapering into nothing, so it reads as hanging in the air.
    d.shape(
        Shape::Cone,
        [c.pos[0], c.pos[1], z - r * 1.3],
        [r * 1.7, r * 1.7, r * 2.2],
        c.facing,
        std::f32::consts::PI,
        dark,
        Material::CHITIN,
        0.0,
    );

    wings(
        d,
        c,
        [c.pos[0], c.pos[1], z + r * 0.6],
        r * 4.2,
        r * 1.5,
        2.4,
        dark,
        [col[0], col[1], col[2], 0.62],
    );

    let head = local(c.pos, c.facing, r * 0.25, 0.0);
    d.sphere([head[0], head[1], z + r * 1.5], r * 0.95, dark, Material::CHITIN);
    eyes(d, c, head, z + r * 1.55, r * 0.5, r * 0.26);
    // Crown of cones.
    for k in 0..5 {
        let a = k as f32 * 1.257 - 0.63 + c.facing;
        let hgt = r * (0.7 + 0.35 * (2 - (k as i32 - 2).abs()) as f32);
        d.cone(
            [head[0] + a.cos() * r * 0.5, head[1] + a.sin() * r * 0.5, z + r * 2.1 + hgt * 0.5],
            r * 0.26,
            hgt,
            a,
            [col[0], col[1], col[2], 1.0],
            Material::GEM,
        );
    }
    // Mantle: shards circling at the waist.
    for i in 0..6 {
        let a = c.bob * 1.1 + i as f32 * 1.047;
        d.shape(
            Shape::Prism,
            [c.pos[0] + a.cos() * r * 2.2, c.pos[1] + a.sin() * r * 2.2, z - r * 0.4 + (a * 2.0).sin() * r * 0.3],
            [r * 0.42, r * 0.42, r * 0.30],
            a,
            0.0,
            rgba(c.armor.color(), 1.0),
            Material::GEM,
            1.0,
        );
    }
    d.glow([c.pos[0], c.pos[1], z], r * 4.5, 2.0, rgba(c.armor.color(), 0.38));
}
