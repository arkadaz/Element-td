//! Builds the 3D scene from game state.
//!
//! Split in two on purpose:
//!   - [`build_static`] runs **once** and produces the terrain, road, scenery and
//!     build grid. None of it changes, so none of it is rebuilt or re-uploaded
//!     per frame.
//!   - [`draw_scene`] runs each frame and emits only what actually moves.
//!
//! This is the only place that decides what the board looks like; colours live in
//! [`theme`] and the models live in the `towers` and `monsters` submodules.

pub mod monsters;
pub mod towers;

use crate::decor::Decor;
use crate::game::board::{BH, BW, ROAD_HALF};
use crate::game::defs::*;
use crate::game::{Game, Phase};
use crate::gfx::draw::{Color, DrawList, Instance, boost, mix, rgba};

pub mod theme {
    use super::Color;
    pub const GRASS_A: [f32; 3] = [0.118, 0.168, 0.140];
    pub const GRASS_B: [f32; 3] = [0.098, 0.145, 0.124];
    pub const GRASS_EDGE: [f32; 3] = [0.074, 0.110, 0.096];
    // A build plot is a SOCKET CUT INTO the ground, not a platform sitting on it.
    // Every empty plot must be darker than the grass around it, or the board
    // reads as a field of litter. Nothing empty is ever emissive.
    /// Apron around the socket - warm, road-family, darker than turf.
    pub const PAD_EARTH: [f32; 3] = [0.098, 0.094, 0.082];
    /// Socket floor - the darkest thing on the field.
    pub const PAD_SOIL: [f32; 3] = [0.072, 0.070, 0.062];
    /// The stone kerb ring. Adjacent plots share a kerb, so the grid reads as
    /// one deliberate lattice rather than scattered squares.
    pub const PAD_KERB: [f32; 3] = [0.148, 0.150, 0.152];
    /// Corner markers, lit only while you are holding a tower you can afford.
    pub const PAD_ARM: [f32; 3] = [0.30, 0.62, 0.78];
    /// "Your wallet is the problem", not "this plot is the problem".
    pub const PAD_BROKE: [f32; 3] = [0.55, 0.44, 0.22];
    pub const ROAD: [f32; 3] = [0.300, 0.252, 0.205];
    pub const ROAD_EDGE: [f32; 3] = [0.180, 0.150, 0.122];
    pub const STONE: [f32; 3] = [0.165, 0.175, 0.215];
    pub const STONE_DARK: [f32; 3] = [0.105, 0.115, 0.150];
    pub const WALL: [f32; 3] = [0.090, 0.100, 0.130];
    pub const HP_BACK: Color = [0.05, 0.06, 0.09, 0.95];
    pub const HP_FILL: Color = [0.38, 0.92, 0.44, 1.0];
    pub const HP_LOW: Color = [0.98, 0.38, 0.32, 1.0];
    pub const GHOST_OK: [f32; 3] = [0.42, 0.85, 1.00];
    pub const GHOST_BAD: [f32; 3] = [1.00, 0.32, 0.38];
    pub const SPAWN: [f32; 3] = [1.00, 0.30, 0.36];
    pub const EXIT: [f32; 3] = [0.32, 1.00, 0.60];
}

/// Height of the buildable ground, so towers and highlights sit flush on it.
pub const PLOT_TOP: f32 = 0.10;

/// Cheap deterministic hash, for per-tile variation.
fn hash2(x: i32, y: i32) -> f32 {
    let n = (x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263)) as u32;
    let n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
}

// ================================================================ static

/// Everything that never moves. Built once at startup.
pub fn build_static(g: &Game, decor: &Decor) -> Vec<Instance> {
    let mut d = DrawList::default();
    terrain(g, &mut d);
    road(g, &mut d);
    gates_static(g, &mut d);
    d.solid.extend_from_slice(&decor.statics);
    d.solid
}

fn terrain(g: &Game, d: &mut DrawList) {
    // Which tiles are build plots - they get a flat, obviously regular surface.
    let mut plot = vec![false; (BW * BH) as usize];
    for s in &g.board.slots {
        let tx = s.pos[0].floor() as usize;
        let ty = s.pos[1].floor() as usize;
        plot[ty * BW as usize + tx] = true;
    }

    for ty in 0..BH as i32 {
        for tx in 0..BW as i32 {
            let p = [tx as f32 + 0.5, ty as f32 + 0.5];
            if g.board.dist_to_road(p) < ROAD_HALF + 0.30 {
                continue; // the road covers this ground
            }
            let is_plot = plot[(ty * BW as i32 + tx) as usize];
            let h = hash2(tx, ty);

            if is_plot {
                // Apron, then a kerb ring, then a recessed socket floor. The
                // socket is the darkest thing on the board so an empty plot
                // reads as "unfilled", never as clutter.
                d.slab(p, [1.0, 1.0], PLOT_TOP, PLOT_TOP + 0.44, rgba(theme::PAD_EARTH, 1.0));
                for (dx, dy, sx, sy) in [
                    (0.0, 0.455, 1.0, 0.09),
                    (0.0, -0.455, 1.0, 0.09),
                    (0.455, 0.0, 0.09, 1.0),
                    (-0.455, 0.0, 0.09, 1.0),
                ] {
                    d.cube(
                        [p[0] + dx, p[1] + dy, PLOT_TOP + 0.03],
                        [sx, sy, 0.12],
                        0.0,
                        rgba(theme::PAD_KERB, 1.0),
                    );
                }
                d.slab(p, [0.86, 0.86], PLOT_TOP - 0.05, 0.10, rgba(theme::PAD_SOIL, 1.0));
            } else {
                // Wild ground: gentle height steps so the field has relief.
                let top = 0.09 + (h * 3.0).floor() * 0.024;
                let base = mix(theme::GRASS_A, theme::GRASS_B, h);
                d.slab(p, [1.0, 1.0], top, top + 0.45, rgba(base, 1.0));
            }
        }
    }

    // A low wall around the plot, so the board reads as a solid object.
    let (w, h) = (BW, BH);
    let wall = rgba(theme::WALL, 1.0);
    let cap = rgba(theme::STONE, 1.0);
    for (cx, cy, sx, sy) in [
        (w * 0.5, -0.4, w + 1.6, 0.8),
        (w * 0.5, h + 0.4, w + 1.6, 0.8),
        (-0.4, h * 0.5, 0.8, h + 1.6),
        (w + 0.4, h * 0.5, 0.8, h + 1.6),
    ] {
        d.cube([cx, cy, 0.20], [sx, sy, 0.56], 0.0, wall);
        d.cube([cx, cy, 0.50], [sx * 0.99, sy * 0.99, 0.10], 0.0, cap);
    }
    for (cx, cy) in [(-0.4, -0.4), (w + 0.4, -0.4), (-0.4, h + 0.4), (w + 0.4, h + 0.4)] {
        d.cube([cx, cy, 0.34], [1.0, 1.0, 0.86], 0.0, rgba(theme::STONE_DARK, 1.0));
        d.cube([cx, cy, 0.80], [0.86, 0.86, 0.12], 0.0, cap);
    }
    let _ = theme::GRASS_EDGE;
}

fn road(g: &Game, d: &mut DrawList) {
    for w in g.board.path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-4 {
            continue;
        }
        let yaw = dy.atan2(dx);
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        d.cube(
            [mid[0], mid[1], 0.055],
            [len + 0.26, ROAD_HALF * 2.0 + 0.30, 0.30],
            yaw,
            rgba(theme::ROAD_EDGE, 1.0),
        );
        d.cube(
            [mid[0], mid[1], 0.145],
            [len + 0.22, ROAD_HALF * 2.0, 0.12],
            yaw,
            rgba(theme::ROAD, 1.0),
        );
    }
}

fn gates_static(g: &Game, d: &mut DrawList) {
    for (dist, col) in [(0.9f32, theme::SPAWN), (g.board.total - 0.9, theme::EXIT)] {
        let p = g.board.sample(dist);
        let dir = g.board.heading(dist);
        let side = [-dir[1], dir[0]];
        let yaw = dir[1].atan2(dir[0]);
        for s in [-1.0f32, 1.0] {
            let px = p[0] + side[0] * s * (ROAD_HALF + 0.34);
            let py = p[1] + side[1] * s * (ROAD_HALF + 0.34);
            d.prop([px, py], [0.42, 0.42, 1.35], yaw, rgba(theme::STONE_DARK, 1.0));
            d.cube([px, py, 1.40], [0.52, 0.52, 0.14], yaw, rgba(theme::STONE, 1.0));
            d.cube_lit([px, py, 1.54], [0.30, 0.30, 0.20], yaw, rgba(col, 1.0), 1.0);
        }
        d.cube(
            [p[0], p[1], 1.50],
            [0.34, ROAD_HALF * 2.0 + 1.0, 0.20],
            yaw,
            rgba(theme::STONE, 1.0),
        );
        d.cube_lit(
            [p[0], p[1], 1.38],
            [0.16, ROAD_HALF * 2.0 + 0.4, 0.08],
            yaw,
            rgba(col, 0.95),
            1.0,
        );
    }
}

// ================================================================ dynamic

pub fn draw_scene(g: &Game, decor: &Decor, d: &mut DrawList, t: f32) {
    torches(decor, d, t);
    gate_glow(g, d, t);
    chevrons(g, d, t);
    plots(g, d, t);
    for (i, tw) in g.towers.iter().enumerate() {
        towers::draw(d, tw, g.selected == Some(i), g.time);
    }
    for c in &g.creeps {
        monsters::draw(d, c);
    }
    shots(g, d);
    beams(g, d);
    build_ghost(g, d, t);
}

fn torches(decor: &Decor, d: &mut DrawList, t: f32) {
    for tor in &decor.torches {
        let flicker =
            0.72 + 0.28 * ((t * 9.0 + tor.phase).sin() * 0.5 + (t * 5.3 + tor.phase).sin() * 0.5);
        d.glow(tor.pos, 1.3 * flicker, 2.2, rgba([1.0, 0.58, 0.20], 0.42 * flicker));
    }
}

fn gate_glow(g: &Game, d: &mut DrawList, t: f32) {
    let pulse = 0.55 + 0.45 * (t * 2.0).sin();
    for (dist, col) in [(0.9f32, theme::SPAWN), (g.board.total - 0.9, theme::EXIT)] {
        let p = g.board.sample(dist);
        d.glow([p[0], p[1], 1.45], 1.5 * pulse.max(0.6), 2.0, rgba(col, 0.45));
        d.glow([p[0], p[1], 0.45], 2.0, 2.4, rgba(col, 0.18 * pulse));
    }
}

/// Chevrons drifting along the road show which way the monsters travel.
fn chevrons(g: &Game, d: &mut DrawList, t: f32) {
    let n = (g.board.total / 2.6) as i32;
    for i in 0..n {
        let phase = (t * 0.85 + i as f32 * 0.5).rem_euclid(1.0);
        let dist = (i as f32 * 2.6 + phase * 2.6).min(g.board.total);
        let a = 0.30 * (1.0 - (phase - 0.5).abs() * 2.0).max(0.0);
        if a <= 0.01 {
            continue;
        }
        let p = g.board.sample(dist);
        let hd = g.board.heading(dist);
        d.cube_lit(
            [p[0], p[1], 0.215],
            [0.55, 0.14, 0.03],
            hd[1].atan2(hd[0]),
            rgba([0.62, 0.76, 0.98], a * 1.8),
            0.9,
        );
    }
}

/// Build plots are part of the static terrain, so all that is drawn here is the
/// state: which are free while you hold a tower, and which one you are pointing at.
fn plots(g: &Game, d: &mut DrawList, t: f32) {
    let Some((def_i, tier)) = g.build_choice else {
        // Idle: the grid stays dark. Only the plot under the cursor answers.
        if let Some(i) = g.hover_slot {
            if let Some(s) = g.board.slots.get(i) {
                if s.tower.is_none() {
                    outline(d, s.pos, rgba(theme::GHOST_OK, 0.30), 0.06);
                }
            }
        }
        return;
    };

    // Only plots you can actually afford light up. As gold drains, fewer plots
    // are lit - the board tells you what you can do without saying a word.
    let affordable = g.can_afford(TOWERS[def_i].cost_at(tier));
    let pulse = 0.5 + 0.5 * (t * 2.2).sin();

    for (i, s) in g.board.slots.iter().enumerate() {
        let hovered = g.hover_slot == Some(i);
        if s.tower.is_some() {
            if hovered {
                // Occupied: flat and matte. Bad news should never bloom.
                outline(d, s.pos, rgba(theme::GHOST_BAD, 0.75), 0.07);
            }
            continue;
        }
        if !affordable {
            if hovered {
                outline(d, s.pos, rgba(theme::PAD_BROKE, 0.70), 0.06);
            }
            continue;
        }

        // Four corner markers on the kerb, not a wash over the whole tile.
        let (col, em, size) = if hovered {
            (theme::GHOST_OK, 1.0, 0.20)
        } else {
            (theme::PAD_ARM, 0.55 + 0.25 * pulse, 0.15)
        };
        for (dx, dy) in [(-0.42, -0.42), (0.42, -0.42), (-0.42, 0.42), (0.42, 0.42)] {
            d.cube_lit(
                [s.pos[0] + dx, s.pos[1] + dy, PLOT_TOP + 0.09],
                [size, size, 0.05],
                0.0,
                rgba(col, 1.0),
                em,
            );
        }
        if hovered {
            // Wash the socket floor, so the target is unmistakable.
            d.cube_lit(
                [s.pos[0], s.pos[1], PLOT_TOP - 0.02],
                [0.84, 0.84, 0.02],
                0.0,
                rgba(theme::GHOST_OK, 0.40),
                1.0,
            );
            d.glow([s.pos[0], s.pos[1], PLOT_TOP + 0.35], 0.85, 2.2, rgba(theme::GHOST_OK, 0.28));
        }
    }
}

/// Four thin bars framing a tile.
fn outline(d: &mut DrawList, p: [f32; 2], col: Color, w: f32) {
    for (dx, dy, sx, sy) in [
        (0.0, 0.46, 0.96, w),
        (0.0, -0.46, 0.96, w),
        (0.46, 0.0, w, 0.96),
        (-0.46, 0.0, w, 0.96),
    ] {
        d.cube_lit(
            [p[0] + dx, p[1] + dy, PLOT_TOP + 0.10],
            [sx, sy, 0.04],
            0.0,
            col,
            1.0,
        );
    }
}

fn build_ghost(g: &Game, d: &mut DrawList, t: f32) {
    let (Some((def_i, tier)), Some(slot)) = (g.build_choice, g.hover_slot) else {
        return;
    };
    let Some(s) = g.board.slots.get(slot) else { return };
    let def = &TOWERS[def_i];
    let ok = s.tower.is_none() && g.can_afford(def.cost_at(tier));
    let p = s.pos;
    let pulse = 0.55 + 0.25 * (t * 5.0).sin();

    if ok {
        towers::draw_ghost(d, def_i, tier, p, t);
        d.ground_ring(p, def.stats(tier, None).range, 0.10, rgba(tower_color(def), 0.55), 80);
    } else {
        for yaw in [0.7f32, -0.7] {
            d.cube_lit(
                [p[0], p[1], PLOT_TOP + 0.3],
                [0.82, 0.16, 0.12],
                yaw,
                rgba(theme::GHOST_BAD, pulse),
                0.9,
            );
        }
    }
}

// ---------------------------------------------------------------- shots

fn shots(g: &Game, d: &mut DrawList) {
    for p in &g.projs {
        let col = tower_color(&TOWERS[p.def]);
        let yaw = p.vel[1].atan2(p.vel[0]);
        let long = p.kind == crate::game::ProjKind::Lance;
        let (l, w) = if long { (0.85, 0.14) } else { (0.32, 0.16) };
        d.cube_lit([p.pos[0], p.pos[1], p.z], [l, w, w], yaw, boost(rgba(col, 1.0), 1.3), 1.0);
        d.glow([p.pos[0], p.pos[1], p.z], 0.36, 1.7, boost(rgba(col, 0.9), 1.5));
    }
}

fn beams(g: &Game, d: &mut DrawList) {
    for b in &g.beams {
        let a = b.t.clamp(0.0, 1.0);
        if b.width <= 0.0 {
            // Nova: an expanding ground shockwave.
            let r = (b.to[0] - b.from[0]) * (1.15 - a);
            d.ground_ring(
                [b.from[0], b.from[1]],
                r.max(0.08),
                0.16,
                boost(rgba(b.color, a * 0.9), 1.6),
                40,
            );
            d.glow([b.from[0], b.from[1], 0.3], r.max(0.1) * 1.2, 2.0, rgba(b.color, a * 0.28));
        } else {
            let w = b.width * (0.5 + a * 0.5);
            d.bar(b.from, b.to, w, boost(rgba(b.color, a), 2.0), 1.0);
            d.glow(b.to, w * 3.5, 1.6, boost(rgba(b.color, a), 1.6));
        }
    }
}

/// Whether the path hint should be emphasised (between waves).
pub fn show_hint(g: &Game) -> bool {
    g.phase == Phase::Build || g.build_choice.is_some()
}
