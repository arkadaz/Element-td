//! Static set dressing: trees, rocks, grass, fences, cliffs and torches.
//!
//! Generated once from the board layout with a fixed seed, then handed to the
//! renderer as a pre-built slab of instances every frame. Nothing here affects
//! gameplay - it exists so the board reads as a place rather than a grid.

use crate::game::board::{BH, BW, Board, ROAD_HALF};
use crate::gfx::draw::{Color, Instance, rgba};
use crate::rng::Rng;
use crate::view::theme;

/// A torch head: the renderer adds a flickering glow at each of these.
#[derive(Clone, Copy)]
pub struct Torch {
    pub pos: [f32; 3],
    pub phase: f32,
}

pub struct Decor {
    /// Everything static, ready to be appended to the solid list.
    pub statics: Vec<Instance>,
    pub torches: Vec<Torch>,
}

fn push(v: &mut Vec<Instance>, pos: [f32; 3], scale: [f32; 3], yaw: f32, color: Color, em: f32) {
    v.push(Instance { pos, scale, rot: [yaw, 0.0], params: [em, 0.0], color });
}

impl Decor {
    pub fn build(board: &Board) -> Self {
        let mut rng = Rng::new(0xD3C0_1234_5678_9ABC);
        let mut s: Vec<Instance> = Vec::with_capacity(4096);
        let mut torches = Vec::new();

        cliffs(&mut s, &mut rng);
        water(&mut s, &mut rng, board);
        fences(&mut s, board, &mut rng);
        scatter(&mut s, board, &mut rng);
        lamps(&mut s, &mut torches, board);

        Self { statics: s, torches }
    }
}

/// Is this spot clear of the road and every build pad?
fn is_free(board: &Board, p: [f32; 2], clearance: f32) -> bool {
    if board.dist_to_road(p) < ROAD_HALF + clearance {
        return false;
    }
    board
        .slots
        .iter()
        .all(|s| (s.pos[0] - p[0]).abs() > 0.85 || (s.pos[1] - p[1]).abs() > 0.85)
}

// ---------------------------------------------------------------- terrain features

/// Stepped stone shelves outside the wall, so the plot sits on a plateau.
fn cliffs(s: &mut Vec<Instance>, rng: &mut Rng) {
    let steps = [
        (1.5f32, 0.10f32, theme::STONE_DARK),
        (2.9, -0.22, [0.085, 0.092, 0.120]),
        (4.4, -0.62, [0.062, 0.068, 0.092]),
    ];
    for (out, z, col) in steps {
        let (w, h) = (BW + out * 2.0, BH + out * 2.0);
        let thickness = 1.6;
        for (cx, cy, sx, sy) in [
            (BW * 0.5, -out, w, thickness),
            (BW * 0.5, BH + out, w, thickness),
            (-out, BH * 0.5, thickness, h),
            (BW + out, BH * 0.5, thickness, h),
        ] {
            push(s, [cx, cy, z], [sx, sy, 1.4], 0.0, rgba(col, 1.0), 0.0);
        }
    }
    // Boulders perched on the shelves.
    for _ in 0..64 {
        let edge = rng.next_u32() % 4;
        let t = rng.range(-0.1, 1.1);
        let out = rng.range(1.8, 4.2);
        let (x, y) = match edge {
            0 => (t * BW, -out),
            1 => (t * BW, BH + out),
            2 => (-out, t * BH),
            _ => (BW + out, t * BH),
        };
        let r = rng.range(0.35, 0.9);
        push(
            s,
            [x, y, rng.range(0.1, 0.5)],
            [r, r * rng.range(0.7, 1.2), r * rng.range(0.6, 1.1)],
            rng.range(0.0, 3.14),
            rgba([0.20, 0.21, 0.25], 1.0),
            0.0,
        );
    }
}

/// A still pool tucked into whichever corner has the most free ground.
fn water(s: &mut Vec<Instance>, rng: &mut Rng, board: &Board) {
    let centre = [BW - 4.5, 3.0];
    if !is_free(board, centre, 1.2) {
        return;
    }
    for dy in -2..=2i32 {
        for dx in -3..=3i32 {
            let p = [centre[0] + dx as f32, centre[1] + dy as f32];
            let edge = (dx.abs() as f32 / 3.0).max(dy.abs() as f32 / 2.0);
            if edge > 0.95 || !is_free(board, p, 1.0) {
                continue;
            }
            // Sunken basin, then the surface just below the grass line.
            push(s, [p[0], p[1], -0.10], [1.0, 1.0, 0.5], 0.0, rgba([0.055, 0.075, 0.090], 1.0), 0.0);
            push(
                s,
                [p[0], p[1], 0.055],
                [0.98, 0.98, 0.06],
                0.0,
                rgba([0.10, 0.34, 0.46], 0.92),
                0.30,
            );
            // Reeds around the rim.
            if edge > 0.6 && rng.chance(0.4) {
                for _ in 0..3 {
                    let rx = p[0] + rng.range(-0.4, 0.4);
                    let ry = p[1] + rng.range(-0.4, 0.4);
                    push(
                        s,
                        [rx, ry, 0.22],
                        [0.05, 0.05, rng.range(0.30, 0.55)],
                        rng.range(0.0, 3.14),
                        rgba([0.16, 0.30, 0.18], 1.0),
                        0.0,
                    );
                }
            }
        }
    }
}

/// Post-and-rail fence hugging both sides of the road.
fn fences(s: &mut Vec<Instance>, board: &Board, rng: &mut Rng) {
    let wood = [0.240, 0.170, 0.110];
    let step = 1.6;
    let mut d = 1.0;
    while d < board.total - 1.0 {
        let p = board.sample(d);
        let h = board.heading(d);
        let side = [-h[1], h[0]];
        let yaw = h[1].atan2(h[0]);
        for sgn in [-1.0f32, 1.0] {
            let off = ROAD_HALF + 0.34;
            let px = p[0] + side[0] * sgn * off;
            let py = p[1] + side[1] * sgn * off;
            if !is_free(board, [px, py], 0.0) {
                continue;
            }
            let ph = rng.range(0.36, 0.46);
            push(s, [px, py, ph * 0.5], [0.10, 0.10, ph], yaw, rgba(wood, 1.0), 0.0);
            // Rail reaching towards the next post.
            let nx = p[0] + h[0] * step * 0.5 + side[0] * sgn * off;
            let ny = p[1] + h[1] * step * 0.5 + side[1] * sgn * off;
            push(
                s,
                [(px + nx) * 0.5, (py + ny) * 0.5, ph * 0.75],
                [step * 0.55, 0.05, 0.06],
                yaw,
                rgba(wood, 1.0),
                0.0,
            );
        }
        d += step;
    }
}

/// Trees, rocks, bushes and grass tufts over the open ground.
fn scatter(s: &mut Vec<Instance>, board: &Board, rng: &mut Rng) {
    for ty in 0..BH as i32 {
        for tx in 0..BW as i32 {
            let jitter = [rng.range(-0.30, 0.30), rng.range(-0.30, 0.30)];
            let p = [tx as f32 + 0.5 + jitter[0], ty as f32 + 0.5 + jitter[1]];
            if !is_free(board, p, 0.55) {
                continue;
            }
            let roll = rng.unit();
            if roll < 0.10 {
                tree(s, rng, p);
            } else if roll < 0.17 {
                rocks(s, rng, p);
            } else if roll < 0.26 {
                bush(s, rng, p);
            } else if roll < 0.55 {
                tuft(s, rng, p);
            }
        }
    }
}

fn tree(s: &mut Vec<Instance>, rng: &mut Rng, p: [f32; 2]) {
    let scale = rng.range(0.8, 1.35);
    let trunk_h = 0.55 * scale;
    let yaw = rng.range(0.0, 3.14);
    push(
        s,
        [p[0], p[1], trunk_h * 0.5],
        [0.17 * scale, 0.17 * scale, trunk_h],
        yaw,
        rgba([0.185, 0.130, 0.085], 1.0),
        0.0,
    );
    // Two or three stacked canopies, narrowing upwards.
    let tint = rng.range(0.0, 1.0);
    let leaf = [
        0.105 + tint * 0.055,
        0.255 + tint * 0.095,
        0.135 + tint * 0.045,
    ];
    let tiers = 2 + (rng.unit() < 0.5) as u32;
    let mut z = trunk_h;
    for k in 0..tiers {
        let f = 1.0 - k as f32 * 0.24;
        let hgt = 0.42 * scale * f;
        push(
            s,
            [p[0], p[1], z + hgt * 0.5],
            [0.86 * scale * f, 0.86 * scale * f, hgt],
            yaw + k as f32 * 0.4,
            rgba(
                [
                    leaf[0] * (1.0 - k as f32 * 0.08),
                    leaf[1] * (1.0 - k as f32 * 0.08),
                    leaf[2] * (1.0 - k as f32 * 0.08),
                ],
                1.0,
            ),
            0.0,
        );
        z += hgt * 0.72;
    }
}

fn rocks(s: &mut Vec<Instance>, rng: &mut Rng, p: [f32; 2]) {
    let n = 1 + rng.next_u32() % 3;
    for _ in 0..n {
        let r = rng.range(0.18, 0.40);
        push(
            s,
            [p[0] + rng.range(-0.3, 0.3), p[1] + rng.range(-0.3, 0.3), r * 0.45],
            [r, r * rng.range(0.7, 1.2), r * rng.range(0.6, 1.0)],
            rng.range(0.0, 3.14),
            rgba([0.215, 0.225, 0.265], 1.0),
            0.0,
        );
    }
}

fn bush(s: &mut Vec<Instance>, rng: &mut Rng, p: [f32; 2]) {
    let r = rng.range(0.26, 0.42);
    push(
        s,
        [p[0], p[1], r * 0.55],
        [r * 1.4, r * 1.2, r * 1.0],
        rng.range(0.0, 3.14),
        rgba([0.100, 0.225, 0.128], 1.0),
        0.0,
    );
}

fn tuft(s: &mut Vec<Instance>, rng: &mut Rng, p: [f32; 2]) {
    for _ in 0..3 {
        let h = rng.range(0.14, 0.30);
        push(
            s,
            [p[0] + rng.range(-0.25, 0.25), p[1] + rng.range(-0.25, 0.25), h * 0.5 + 0.06],
            [0.045, 0.045, h],
            rng.range(0.0, 3.14),
            rgba([0.150, 0.270, 0.150], 1.0),
            0.0,
        );
    }
}

/// Lamp posts along the road; the renderer adds the flame glow.
fn lamps(s: &mut Vec<Instance>, torches: &mut Vec<Torch>, board: &Board) {
    let mut d = 4.0;
    let mut i = 0;
    while d < board.total - 2.0 {
        let p = board.sample(d);
        let h = board.heading(d);
        let side = [-h[1], h[0]];
        let sgn = if i % 2 == 0 { 1.0 } else { -1.0 };
        let off = ROAD_HALF + 0.85;
        let px = p[0] + side[0] * sgn * off;
        let py = p[1] + side[1] * sgn * off;
        if is_free(board, [px, py], 0.0) {
            push(s, [px, py, 0.55], [0.13, 0.13, 1.10], 0.0, rgba([0.12, 0.13, 0.16], 1.0), 0.0);
            push(s, [px, py, 1.16], [0.26, 0.26, 0.14], 0.0, rgba(theme::STONE, 1.0), 0.0);
            push(
                s,
                [px, py, 1.30],
                [0.16, 0.16, 0.20],
                0.0,
                rgba([1.0, 0.62, 0.24], 1.0),
                1.0,
            );
            torches.push(Torch { pos: [px, py, 1.32], phase: i as f32 * 1.7 });
        }
        d += 7.5;
        i += 1;
    }
}
