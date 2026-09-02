//! Static set dressing: trees, rocks, grass, fences, cliffs and torches.
//!
//! Generated once from the board layout with a fixed seed, then handed to the
//! renderer as a pre-built bucket of instances every frame. Nothing here affects
//! gameplay - it exists so the board reads as a place rather than a grid.
//!
//! Everything is modelled from the shape library: a tree is a tapered trunk with
//! stacked cone foliage, a rock is a crushed sphere, a fence is turned posts with
//! rails between them.

use crate::game::board::{BH, BW, Board, ROAD_HALF};
use crate::gfx::draw::{DrawList, Material, Shape, rgba};
use crate::rng::Rng;
use crate::view::theme;

/// A torch head: the renderer adds a flickering glow at each of these.
#[derive(Clone, Copy)]
pub struct Torch {
    pub pos: [f32; 3],
    pub phase: f32,
}

pub struct Decor {
    /// Everything static, already bucketed by shape.
    pub statics: DrawList,
    pub torches: Vec<Torch>,
}

impl Decor {
    pub fn build(board: &Board) -> Self {
        let mut rng = Rng::new(0xD3C0_1234_5678_9ABC);
        let mut d = DrawList::default();
        let mut torches = Vec::new();

        cliffs(&mut d, &mut rng);
        water(&mut d, &mut rng, board);
        fences(&mut d, board, &mut rng);
        scatter(&mut d, board, &mut rng);
        lamps(&mut d, &mut torches, board);

        Self {
            statics: d,
            torches,
        }
    }
}

/// Is this spot clear of the road and every build plot?
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
fn cliffs(d: &mut DrawList, rng: &mut Rng) {
    let steps = [
        (1.5f32, 0.10f32, theme::STONE_DARK),
        (2.9, -0.22, [0.085, 0.092, 0.120]),
        (4.4, -0.62, [0.062, 0.068, 0.092]),
    ];
    for (out, z, col) in steps {
        let (w, h) = (BW + out * 2.0, BH + out * 2.0);
        let t = 1.6;
        for (cx, cy, sx, sy) in [
            (BW * 0.5, -out, w, t),
            (BW * 0.5, BH + out, w, t),
            (-out, BH * 0.5, t, h),
            (BW + out, BH * 0.5, t, h),
        ] {
            d.cube_mat(
                [cx, cy, z],
                [sx, sy, 1.4],
                0.0,
                rgba(col, 1.0),
                Material::STONE,
            );
        }
    }
    // Boulders perched on the shelves: squashed spheres, never boxes.
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
        let r = rng.range(0.5, 1.2);
        d.shape(
            Shape::Sphere,
            [x, y, rng.range(0.0, 0.4)],
            [r, r * rng.range(0.7, 1.2), r * rng.range(0.55, 0.9)],
            rng.range(0.0, std::f32::consts::PI),
            rng.range(-0.3, 0.3),
            rgba([0.20, 0.21, 0.25], 1.0),
            Material::STONE,
            0.0,
        );
    }
}

/// A still pool tucked into a corner.
fn water(d: &mut DrawList, rng: &mut Rng, board: &Board) {
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
            d.slab_mat(
                p,
                [1.0, 1.0],
                -0.10,
                0.5,
                rgba([0.055, 0.075, 0.090], 1.0),
                Material::STONE,
            );
            // A near-mirror surface: this is where PBR earns its keep.
            d.slab_mat(
                p,
                [0.99, 0.99],
                0.055,
                0.06,
                rgba([0.09, 0.30, 0.42], 0.94),
                Material::WATER,
            );
            if edge > 0.6 && rng.chance(0.4) {
                for _ in 0..3 {
                    let h = rng.range(0.30, 0.55);
                    d.cylinder(
                        [
                            p[0] + rng.range(-0.4, 0.4),
                            p[1] + rng.range(-0.4, 0.4),
                            0.05 + h * 0.5,
                        ],
                        0.05,
                        h,
                        0.0,
                        rgba([0.16, 0.30, 0.18], 1.0),
                        Material::FOLIAGE,
                    );
                }
            }
        }
    }
}

/// Turned posts with rails between them, hugging both sides of the road.
fn fences(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    let wood = [0.240, 0.170, 0.110];
    let step = 1.6;
    let mut dist = 1.0;
    while dist < board.total - 1.0 {
        let p = board.sample(dist);
        let hd = board.heading(dist);
        let side = [-hd[1], hd[0]];
        for sgn in [-1.0f32, 1.0] {
            let off = ROAD_HALF + 0.34;
            let px = p[0] + side[0] * sgn * off;
            let py = p[1] + side[1] * sgn * off;
            if !is_free(board, [px, py], 0.0) {
                continue;
            }
            let ph = rng.range(0.38, 0.48);
            d.cylinder(
                [px, py, ph * 0.5],
                0.10,
                ph,
                0.0,
                rgba(wood, 1.0),
                Material::WOOD,
            );
            // Cap the post so it reads as turned timber.
            d.sphere([px, py, ph], 0.13, rgba(wood, 1.0), Material::WOOD);
            let nx = p[0] + hd[0] * step + side[0] * sgn * off;
            let ny = p[1] + hd[1] * step + side[1] * sgn * off;
            d.link(
                Shape::Cylinder,
                [px, py, ph * 0.72],
                [nx, ny, ph * 0.72],
                0.055,
                rgba(wood, 1.0),
                Material::WOOD,
                0.0,
            );
        }
        dist += step;
    }
}

/// Trees, rocks, bushes and grass over the open ground.
fn scatter(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    for ty in 0..BH as i32 {
        for tx in 0..BW as i32 {
            let p = [
                tx as f32 + 0.5 + rng.range(-0.30, 0.30),
                ty as f32 + 0.5 + rng.range(-0.30, 0.30),
            ];
            if !is_free(board, p, 0.55) {
                continue;
            }
            let roll = rng.unit();
            if roll < 0.10 {
                tree(d, rng, p);
            } else if roll < 0.17 {
                rocks(d, rng, p);
            } else if roll < 0.26 {
                bush(d, rng, p);
            } else if roll < 0.50 {
                tuft(d, rng, p);
            }
        }
    }
}

/// Tapered trunk, stacked cone canopy. The classic low-poly conifer.
fn tree(d: &mut DrawList, rng: &mut Rng, p: [f32; 2]) {
    let scale = rng.range(0.85, 1.4);
    let trunk_h = 0.55 * scale;
    d.cylinder(
        [p[0], p[1], trunk_h * 0.5],
        0.17 * scale,
        trunk_h,
        0.0,
        rgba([0.185, 0.130, 0.085], 1.0),
        Material::WOOD,
    );

    let tint = rng.unit();
    let leaf = [
        0.105 + tint * 0.055,
        0.255 + tint * 0.095,
        0.135 + tint * 0.045,
    ];
    let broadleaf = rng.chance(0.35);
    if broadleaf {
        // Round canopy: two overlapping squashed spheres.
        let r = 0.95 * scale;
        d.shape(
            Shape::Sphere,
            [p[0], p[1], trunk_h + r * 0.34],
            [r, r, r * 0.85],
            0.0,
            0.0,
            rgba(leaf, 1.0),
            Material::FOLIAGE,
            0.0,
        );
        d.shape(
            Shape::Sphere,
            [p[0] + r * 0.16, p[1] - r * 0.12, trunk_h + r * 0.66],
            [r * 0.72, r * 0.72, r * 0.62],
            0.0,
            0.0,
            rgba([leaf[0] * 1.12, leaf[1] * 1.12, leaf[2] * 1.12], 1.0),
            Material::FOLIAGE,
            0.0,
        );
    } else {
        // Conifer: three cones, narrowing upwards.
        let tiers = 3;
        let mut z = trunk_h - 0.08 * scale;
        for k in 0..tiers {
            let f = 1.0 - k as f32 * 0.22;
            let hgt = 0.52 * scale * f;
            let shade = 1.0 - k as f32 * 0.06;
            d.cone(
                [p[0], p[1], z + hgt * 0.5],
                1.05 * scale * f,
                hgt,
                0.0,
                rgba([leaf[0] * shade, leaf[1] * shade, leaf[2] * shade], 1.0),
                Material::FOLIAGE,
            );
            z += hgt * 0.62;
        }
    }
}

fn rocks(d: &mut DrawList, rng: &mut Rng, p: [f32; 2]) {
    let n = 1 + rng.next_u32() % 3;
    for _ in 0..n {
        let r = rng.range(0.22, 0.46);
        d.shape(
            Shape::Sphere,
            [
                p[0] + rng.range(-0.3, 0.3),
                p[1] + rng.range(-0.3, 0.3),
                r * 0.34,
            ],
            [r, r * rng.range(0.7, 1.2), r * rng.range(0.5, 0.85)],
            rng.range(0.0, std::f32::consts::PI),
            rng.range(-0.25, 0.25),
            rgba([0.215, 0.225, 0.265], 1.0),
            Material::STONE,
            0.0,
        );
    }
}

fn bush(d: &mut DrawList, rng: &mut Rng, p: [f32; 2]) {
    let r = rng.range(0.30, 0.48);
    for k in 0..3 {
        let a = k as f32 * 2.1 + rng.range(0.0, 1.0);
        let rr = r * rng.range(0.6, 1.0);
        d.shape(
            Shape::Sphere,
            [
                p[0] + a.cos() * r * 0.24,
                p[1] + a.sin() * r * 0.24,
                rr * 0.42,
            ],
            [rr, rr, rr * 0.8],
            0.0,
            0.0,
            rgba([0.100, 0.225, 0.128], 1.0),
            Material::FOLIAGE,
            0.0,
        );
    }
}

fn tuft(d: &mut DrawList, rng: &mut Rng, p: [f32; 2]) {
    for _ in 0..3 {
        let h = rng.range(0.16, 0.32);
        let a = rng.range(0.0, std::f32::consts::PI);
        // Blades lean, so grass does not look like a bed of nails.
        d.shape(
            Shape::Cone,
            [
                p[0] + rng.range(-0.25, 0.25),
                p[1] + rng.range(-0.25, 0.25),
                h * 0.5 + 0.05,
            ],
            [0.075, 0.075, h],
            a,
            rng.range(-0.25, 0.25),
            rgba([0.150, 0.270, 0.150], 1.0),
            Material::FOLIAGE,
            0.0,
        );
    }
}

/// Lamp posts along the road; the renderer adds the flame glow.
fn lamps(d: &mut DrawList, torches: &mut Vec<Torch>, board: &Board) {
    let mut dist = 4.0;
    let mut i = 0;
    while dist < board.total - 2.0 {
        let p = board.sample(dist);
        let hd = board.heading(dist);
        let side = [-hd[1], hd[0]];
        let sgn = if i % 2 == 0 { 1.0 } else { -1.0 };
        let off = ROAD_HALF + 0.85;
        let px = p[0] + side[0] * sgn * off;
        let py = p[1] + side[1] * sgn * off;
        if is_free(board, [px, py], 0.0) {
            let iron = rgba([0.12, 0.13, 0.16], 1.0);
            d.cylinder([px, py, 0.06], 0.30, 0.12, 0.0, iron, Material::DARK_METAL);
            d.cylinder([px, py, 0.58], 0.11, 1.05, 0.0, iron, Material::DARK_METAL);
            // Lantern: a small cage with a flame inside.
            d.prism([px, py, 1.20], 0.30, 0.26, 0.0, iron, Material::DARK_METAL);
            d.sphere_lit([px, py, 1.20], 0.20, rgba([1.0, 0.62, 0.24], 1.0), 1.0);
            d.cone([px, py, 1.40], 0.34, 0.16, 0.0, iron, Material::DARK_METAL);
            torches.push(Torch {
                pos: [px, py, 1.22],
                phase: i as f32 * 1.7,
            });
        }
        dist += 7.5;
        i += 1;
    }
}
