//! Monster models.
//!
//! Armour type is carried by colour, so **shape** has to carry the identity: the
//! Brute is a slab on stumpy legs, the Runner is a leaning dart, the Warden and
//! Mender float with no legs at all, the Bulwark hides behind a plate that
//! visibly breaks. You should know what is coming before you read the HUD.

use super::theme;
use crate::game::Creep;
use crate::game::defs::Kind;
use crate::gfx::draw::{Color, DrawList, mix, rgba};

/// Offset a point by a rotated local (forward, right) pair.
fn local(p: [f32; 2], yaw: f32, fwd: f32, right: f32) -> [f32; 2] {
    let (c, s) = (yaw.cos(), yaw.sin());
    [p[0] + c * fwd - s * right, p[1] + s * fwd + c * right]
}

pub fn draw(d: &mut DrawList, c: &Creep) {
    let base = c.armor.color();
    let body_rgb = mix(base, [1.0, 1.0, 1.0], c.flash * 0.75);
    let body_col = rgba(body_rgb, 1.0);
    let dark = rgba(mix(body_rgb, [0.05, 0.05, 0.08], 0.45), 1.0);
    let r = c.radius;

    // Contact shadow - the strongest single 3D cue there is.
    let floats = matches!(c.kind, Kind::Warden | Kind::Mender | Kind::Phaser);
    d.slab(
        c.pos,
        [r * 2.3, r * 1.9],
        0.21,
        0.03,
        [0.0, 0.0, 0.0, if floats { 0.20 } else { 0.36 }],
    );

    match c.kind {
        Kind::Grunt => grunt(d, c, body_col, dark),
        Kind::Runner => runner(d, c, body_col, dark),
        Kind::Brute => brute(d, c, body_col, dark),
        Kind::Swarm => swarm(d, c, body_col, dark),
        Kind::Warden => warden(d, c, body_col, dark),
        Kind::Mender => mender(d, c, body_col, dark),
        Kind::Bulwark => bulwark(d, c, body_col, dark),
        Kind::Phaser => phaser(d, c, body_col, dark),
        Kind::Boss => boss(d, c, body_col, dark),
    }

    status(d, c, base);
    health_bar(d, c);
}

// ---------------------------------------------------------------- shared parts

/// Four legs stepping in diagonal pairs.
fn legs(d: &mut DrawList, c: &Creep, col: Color, count: usize, spread: f32, len: f32) {
    let walking = c.stun <= 0.0;
    let r = c.radius;
    for i in 0..count {
        let fwd = if i < 2 { 1.0 } else { -1.0 };
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let phase = c.bob * 2.0 + (i as f32) * std::f32::consts::FRAC_PI_2;
        let lift = if walking { (phase.sin() * 0.5 + 0.5) * r * 0.30 } else { 0.0 };
        let q = local(c.pos, c.facing, fwd * r * spread, side * r * spread);
        d.cube(
            [q[0], q[1], len * 0.5 + lift * 0.5],
            [r * 0.26, r * 0.26, len + lift],
            c.facing,
            col,
        );
    }
}

fn eyes(d: &mut DrawList, c: &Creep, at: [f32; 2], z: f32, size: f32, spread: f32) {
    for s in [-1.0f32, 1.0] {
        let e = local(at, c.facing, size * 0.5, s * spread);
        d.cube_lit(
            [e[0], e[1], z],
            [size * 0.4, size * 0.4, size * 0.4],
            c.facing,
            rgba([1.0, 0.92, 0.72], 1.0),
            1.0,
        );
    }
}

// ---------------------------------------------------------------- the nine

/// The baseline: a boxy quadruped.
fn grunt(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.55;
    let bz = leg + r * 0.80;
    legs(d, c, dark, 4, 0.55, leg);
    d.cube(c.pos_z(bz), [r * 1.7, r * 1.5, r * 1.4], c.facing, col);
    let h = local(c.pos, c.facing, r * 0.90, 0.0);
    d.cube([h[0], h[1], bz + r * 0.30], [r * 0.9, r * 0.85, r * 0.8], c.facing, dark);
    eyes(d, c, h, bz + r * 0.42, r * 0.5, r * 0.26);
}

/// Long, low and leaning forward, with two legs and a streaming tail.
fn runner(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.60;
    let bz = leg + r * 0.60;
    legs(d, c, dark, 2, 0.40, leg);
    // Body pitched nose-down.
    let body = local(c.pos, c.facing, 0.0, 0.0);
    d.solid.push(crate::gfx::draw::Instance {
        pos: [body[0], body[1], bz],
        scale: [r * 2.3, r * 1.0, r * 0.95],
        rot: [c.facing, -0.22],
        params: [0.0, 0.0],
        color: col,
    });
    let h = local(c.pos, c.facing, r * 1.15, 0.0);
    d.cube([h[0], h[1], bz - r * 0.18], [r * 0.75, r * 0.7, r * 0.6], c.facing, dark);
    eyes(d, c, h, bz - r * 0.10, r * 0.42, r * 0.20);
    // Tail streaming behind, swinging with the stride.
    let sway = (c.bob * 2.0).sin() * 0.35;
    let t0 = local(c.pos, c.facing, -r * 0.9, 0.0);
    let t1 = local(c.pos, c.facing + sway, -r * 2.1, 0.0);
    d.bar([t0[0], t0[1], bz + r * 0.1], [t1[0], t1[1], bz + r * 0.5], r * 0.16, col, 0.35);
}

/// A slab of muscle: wide, low, heavy plates, stumpy legs.
fn brute(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.42;
    let bz = leg + r * 0.85;
    legs(d, c, dark, 4, 0.62, leg);
    d.cube(c.pos_z(bz), [r * 1.9, r * 1.9, r * 1.55], c.facing, col);
    // Shoulder plates: the identifying feature.
    for s in [-1.0f32, 1.0] {
        let q = local(c.pos, c.facing, r * 0.15, s * r * 1.0);
        d.cube(
            [q[0], q[1], bz + r * 0.55],
            [r * 1.3, r * 0.45, r * 0.55],
            c.facing,
            rgba(mix([dark[0], dark[1], dark[2]], [1.0, 1.0, 1.0], 0.18), 1.0),
        );
    }
    // Small sunken head.
    let h = local(c.pos, c.facing, r * 0.85, 0.0);
    d.cube([h[0], h[1], bz + r * 0.30], [r * 0.7, r * 0.7, r * 0.6], c.facing, dark);
    eyes(d, c, h, bz + r * 0.38, r * 0.36, r * 0.20);
}

/// Tiny and twitchy - it only has to read as "one of many".
fn swarm(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let jitter = (c.bob * 3.0).sin() * r * 0.10;
    let bz = r * 0.85 + jitter;
    legs(d, c, dark, 2, 0.45, r * 0.45);
    d.cube(c.pos_z(bz), [r * 1.5, r * 1.4, r * 1.3], c.facing + jitter, col);
    let h = local(c.pos, c.facing, r * 0.75, 0.0);
    eyes(d, c, h, bz + r * 0.2, r * 0.45, r * 0.22);
}

/// Floats, robed, with a rune orbiting it. No legs at all.
fn warden(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let hover = (c.bob * 1.4).sin() * r * 0.12;
    let bz = r * 1.3 + hover;
    // Tapering robe: three shrinking boxes.
    d.cube(c.pos_z(bz - r * 0.55), [r * 1.7, r * 1.6, r * 0.5], c.facing, dark);
    d.cube(c.pos_z(bz - r * 0.10), [r * 1.4, r * 1.3, r * 0.55], c.facing, col);
    d.cube(c.pos_z(bz + r * 0.40), [r * 1.0, r * 0.95, r * 0.5], c.facing, col);
    // Hood.
    let h = local(c.pos, c.facing, r * 0.20, 0.0);
    d.cube([h[0], h[1], bz + r * 0.80], [r * 0.85, r * 0.8, r * 0.55], c.facing, dark);
    eyes(d, c, h, bz + r * 0.80, r * 0.42, r * 0.20);
    // Orbiting ward rune - the tell that magic bounces off it.
    for i in 0..3 {
        let a = c.bob * 0.9 + i as f32 * 2.094;
        let q = [c.pos[0] + a.cos() * r * 1.5, c.pos[1] + a.sin() * r * 1.5];
        d.cube_lit([q[0], q[1], bz + r * 0.2], [r * 0.3, r * 0.14, r * 0.14], a, rgba(c.armor.color(), 1.0), 1.0);
    }
}

/// A floating orb inside a halo, with a heal pulse washing the ground.
fn mender(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let hover = (c.bob * 1.6).sin() * r * 0.14;
    let bz = r * 1.5 + hover;
    // Trailing robe tapering to nothing.
    d.cube(c.pos_z(bz - r * 0.85), [r * 0.6, r * 0.6, r * 0.6], c.facing, dark);
    d.cube(c.pos_z(bz - r * 0.30), [r * 1.1, r * 1.05, r * 0.6], c.facing, dark);
    d.cube(c.pos_z(bz + r * 0.25), [r * 1.3, r * 1.25, r * 0.8], c.facing, col);
    // Halo.
    let spin = c.bob * 1.2;
    for i in 0..6 {
        let a = spin + i as f32 * 1.047;
        let q = [c.pos[0] + a.cos() * r * 1.15, c.pos[1] + a.sin() * r * 1.15];
        d.cube_lit(
            [q[0], q[1], bz + r * 0.95],
            [r * 0.28, r * 0.12, r * 0.10],
            a + std::f32::consts::FRAC_PI_2,
            rgba([0.55, 1.0, 0.70], 1.0),
            1.0,
        );
    }
    // The heal aura it projects: impossible to miss, so it can be focused down.
    let pulse = (c.bob * 1.8).sin() * 0.5 + 0.5;
    d.ground_ring(c.pos, 2.6 * (0.85 + pulse * 0.15), 0.09, rgba([0.45, 1.0, 0.62], 0.45), 40);
    d.glow([c.pos[0], c.pos[1], bz], r * 3.2, 2.2, rgba([0.45, 1.0, 0.62], 0.25 + pulse * 0.12));
}

/// Hides behind a slab shield that visibly breaks as it soaks damage.
fn bulwark(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.45;
    let bz = leg + r * 0.85;
    legs(d, c, dark, 4, 0.55, leg);
    d.cube(c.pos_z(bz), [r * 1.6, r * 1.6, r * 1.5], c.facing, col);
    let h = local(c.pos, c.facing, r * 0.55, 0.0);
    d.cube([h[0], h[1], bz + r * 0.75], [r * 0.7, r * 0.7, r * 0.55], c.facing, dark);
    eyes(d, c, h, bz + r * 0.78, r * 0.34, r * 0.18);

    // The shield: a tall plate held out front. It shrinks and dims as it breaks.
    let frac = if c.max_shield > 0.0 { (c.shield / c.max_shield).clamp(0.0, 1.0) } else { 0.0 };
    if frac > 0.0 {
        let q = local(c.pos, c.facing, r * 1.25, 0.0);
        let hgt = r * (1.2 + 1.4 * frac);
        d.cube_lit(
            [q[0], q[1], bz + r * 0.1],
            [r * 0.22, r * 2.1, hgt],
            c.facing,
            rgba(mix([0.55, 0.75, 1.0], [1.0, 1.0, 1.0], 1.0 - frac), 0.35 + 0.5 * frac),
            0.7,
        );
        d.glow([q[0], q[1], bz + r * 0.2], r * 2.4, 2.0, rgba([0.5, 0.72, 1.0], 0.20 * frac));
    } else {
        // Broken: only the shattered handle remains.
        let q = local(c.pos, c.facing, r * 1.1, 0.0);
        d.cube([q[0], q[1], bz - r * 0.2], [r * 0.2, r * 0.9, r * 0.4], c.facing, dark);
    }
}

/// Half-there: a thin body with an after-image that separates while phasing.
fn phaser(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let hover = (c.bob * 2.2).sin() * r * 0.10;
    let bz = r * 1.15 + hover;
    let ghosting = c.slow_off;

    // Main body: narrow and tall.
    let a = if ghosting { 0.55 } else { 1.0 };
    d.cube(c.pos_z(bz), [r * 1.1, r * 1.1, r * 1.9], c.facing, [col[0], col[1], col[2], a]);
    let h = local(c.pos, c.facing, r * 0.35, 0.0);
    d.cube([h[0], h[1], bz + r * 1.05], [r * 0.8, r * 0.8, r * 0.6], c.facing, [dark[0], dark[1], dark[2], a]);
    eyes(d, c, h, bz + r * 1.05, r * 0.4, r * 0.20);

    // After-image trailing behind, strongest while it is ignoring slows.
    let lag = if ghosting { r * 1.1 } else { r * 0.45 };
    let t = local(c.pos, c.facing, -lag, 0.0);
    d.cube_lit(
        [t[0], t[1], bz],
        [r * 0.9, r * 0.9, r * 1.7],
        c.facing,
        [col[0], col[1], col[2], if ghosting { 0.30 } else { 0.12 }],
        0.8,
    );
    if ghosting {
        d.glow([c.pos[0], c.pos[1], bz], r * 3.0, 2.2, rgba(c.armor.color(), 0.30));
    }
}

/// Enormous, crowned, horned, spined. Should stop the player mid-sentence.
fn boss(d: &mut DrawList, c: &Creep, col: Color, dark: Color) {
    let r = c.radius;
    let leg = r * 0.50;
    let bz = leg + r * 1.05;
    legs(d, c, dark, 4, 0.62, leg);

    // Bulk.
    d.cube(c.pos_z(bz), [r * 2.0, r * 1.9, r * 1.8], c.facing, col);
    // Shoulder armour.
    for s in [-1.0f32, 1.0] {
        let q = local(c.pos, c.facing, -r * 0.1, s * r * 1.05);
        d.cube([q[0], q[1], bz + r * 0.75], [r * 1.2, r * 0.55, r * 0.6], c.facing, dark);
    }
    // Back spines.
    for k in 0..4 {
        let off = -0.55 + k as f32 * 0.32;
        let q = local(c.pos, c.facing, r * off, 0.0);
        d.cube_lit(
            [q[0], q[1], bz + r * 1.15],
            [r * 0.2, r * 0.2, r * (0.5 + 0.18 * (3 - k.min(3)) as f32)],
            c.facing,
            rgba(c.armor.color(), 1.0),
            0.6,
        );
    }
    // Head with a crown of horns.
    let h = local(c.pos, c.facing, r * 1.0, 0.0);
    d.cube([h[0], h[1], bz + r * 0.55], [r * 1.15, r * 1.1, r * 0.9], c.facing, dark);
    eyes(d, c, h, bz + r * 0.65, r * 0.55, r * 0.30);
    for s in [-1.0f32, 1.0] {
        let q = local(c.pos, c.facing, r * 0.75, s * r * 0.55);
        d.solid.push(crate::gfx::draw::Instance {
            pos: [q[0], q[1], bz + r * 1.35],
            scale: [r * 0.22, r * 0.22, r * 1.0],
            rot: [c.facing, s * 0.30],
            params: [0.0, 0.0],
            color: dark,
        });
    }
    d.glow([c.pos[0], c.pos[1], bz], r * 3.4, 2.0, rgba(c.armor.color(), 0.35));
}

// ---------------------------------------------------------------- overlays

fn status(d: &mut DrawList, c: &Creep, base: [f32; 3]) {
    let r = c.radius;
    let bz = r * 1.4;
    let _ = base;
    if c.slow.t > 0.0 && !c.slow_off {
        d.glow([c.pos[0], c.pos[1], bz], r * 2.6, 1.8, rgba([0.45, 0.80, 1.0], 0.32));
    }
    if c.stun > 0.0 {
        d.cube_lit(
            [c.pos[0], c.pos[1], bz + r * 2.0],
            [r * 0.8, r * 0.8, r * 0.14],
            c.bob,
            rgba([1.0, 1.0, 1.0], 0.9),
            1.0,
        );
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
    let bar_z = r * 3.1 + 0.25;
    d.cube_lit([c.pos[0], c.pos[1], bar_z], [w + 0.05, 0.12, 0.11], 0.0, theme::HP_BACK, 0.35);
    let fill = if hp > 0.35 { theme::HP_FILL } else { theme::HP_LOW };
    d.cube_lit(
        [c.pos[0] - w * 0.5 * (1.0 - hp), c.pos[1], bar_z + 0.015],
        [w * hp, 0.12, 0.12],
        0.0,
        fill,
        0.85,
    );
    // A second, blue bar above it while a shield is holding.
    if c.max_shield > 0.0 && c.shield > 0.0 {
        let sf = (c.shield / c.max_shield).clamp(0.0, 1.0);
        d.cube_lit(
            [c.pos[0] - w * 0.5 * (1.0 - sf), c.pos[1], bar_z + 0.14],
            [w * sf, 0.10, 0.10],
            0.0,
            [0.50, 0.74, 1.0, 1.0],
            0.85,
        );
    }
}

/// Small convenience so model code reads as "body at height z".
trait AtHeight {
    fn pos_z(&self, z: f32) -> [f32; 3];
}

impl AtHeight for Creep {
    fn pos_z(&self, z: f32) -> [f32; 3] {
        [self.pos[0], self.pos[1], z]
    }
}
