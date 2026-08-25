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
use crate::gfx::draw::{Color, DrawList, Material, Shape, boost, mix, rgba};

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

/// Everything that never moves, split by whether it casts a shadow.
///
/// The ground, the road and the build pads are flat and sit *on* the ground, so
/// their shadows land exactly where they already are - drawing sixteen hundred
/// of them into the shadow map every frame changes nothing on screen. Only
/// things that stand up cast: walls, gates, trees, fences, lamps.
pub struct Statics {
    pub casters: DrawList,
    pub flat: DrawList,
}

pub fn build_static(g: &Game, decor: &Decor) -> Statics {
    let mut casters = DrawList::default();
    let mut flat = DrawList::default();
    terrain(g, &mut flat, &mut casters);
    road(g, &mut flat);
    gates_static(g, &mut casters);
    casters.append_solids(&decor.statics);
    Statics { casters, flat }
}

fn terrain(g: &Game, d: &mut DrawList, tall: &mut DrawList) {
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
                d.slab_mat(
                    p,
                    [1.0, 1.0],
                    PLOT_TOP,
                    PLOT_TOP + 0.44,
                    rgba(theme::PAD_EARTH, 1.0),
                    Material::EARTH,
                );
                for (dx, dy, sx, sy) in [
                    (0.0, 0.455, 1.0, 0.09),
                    (0.0, -0.455, 1.0, 0.09),
                    (0.455, 0.0, 0.09, 1.0),
                    (-0.455, 0.0, 0.09, 1.0),
                ] {
                    d.cube_mat(
                        [p[0] + dx, p[1] + dy, PLOT_TOP + 0.03],
                        [sx, sy, 0.12],
                        0.0,
                        rgba(theme::PAD_KERB, 1.0),
                        Material::STONE,
                    );
                }
                d.slab_mat(
                    p,
                    [0.86, 0.86],
                    PLOT_TOP - 0.05,
                    0.10,
                    rgba(theme::PAD_SOIL, 1.0),
                    Material::EARTH,
                );
            } else {
                // Wild ground: gentle height steps so the field has relief, and
                // a few tufts so the turf is not a flat plane of one colour.
                let top = 0.09 + (h * 3.0).floor() * 0.024;
                let base = mix(theme::GRASS_A, theme::GRASS_B, h);
                d.slab_mat(p, [1.0, 1.0], top, top + 0.45, rgba(base, 1.0), Material::EARTH);
                if h > 0.80 {
                    let j = hash2(tx * 7 + 3, ty * 11 + 5);
                    tall.shape(
                        Shape::Cone,
                        [p[0] + (j - 0.5) * 0.5, p[1] + (h - 0.5) * 0.5, top + 0.09],
                        [0.30, 0.30, 0.22],
                        j * 6.0,
                        0.12,
                        rgba(mix(theme::GRASS_A, theme::GRASS_EDGE, j), 1.0),
                        Material::FOLIAGE,
                        0.0,
                    );
                }
            }
        }
    }

    // A low wall around the plot, so the board reads as a solid object: a
    // rusticated base, a chamfered course, then a rounded coping.
    let (w, h) = (BW, BH);
    let wall = rgba(theme::WALL, 1.0);
    let cap = rgba(theme::STONE, 1.0);
    for (cx, cy, sx, sy) in [
        (w * 0.5, -0.4, w + 1.6, 0.8),
        (w * 0.5, h + 0.4, w + 1.6, 0.8),
        (-0.4, h * 0.5, 0.8, h + 1.6),
        (w + 0.4, h * 0.5, 0.8, h + 1.6),
    ] {
        tall.cube_mat([cx, cy, 0.20], [sx, sy, 0.56], 0.0, wall, Material::STONE);
        tall.cube_mat([cx, cy, 0.50], [sx * 0.99, sy * 0.99, 0.10], 0.0, cap, Material::STONE);
        // Coping: a capsule laid along the wall gives it a rounded top edge.
        let along = sx > sy;
        let (ax, ay, bx, by) = if along {
            (cx - sx * 0.5, cy, cx + sx * 0.5, cy)
        } else {
            (cx, cy - sy * 0.5, cx, cy + sy * 0.5)
        };
        tall.link(
            Shape::Capsule,
            [ax, ay, 0.57],
            [bx, by, 0.57],
            if along { sy * 0.72 } else { sx * 0.72 },
            cap,
            Material::STONE,
            0.0,
        );
    }
    // Corner towers: a stone drum with a conical roof, so the board has corners
    // you can actually see rather than four more cubes.
    for (cx, cy) in [(-0.4, -0.4), (w + 0.4, -0.4), (-0.4, h + 0.4), (w + 0.4, h + 0.4)] {
        tall.cylinder([cx, cy, 0.45], 1.10, 1.10, 0.0, rgba(theme::STONE_DARK, 1.0), Material::STONE);
        tall.cylinder([cx, cy, 1.03], 1.24, 0.14, 0.0, cap, Material::STONE);
        tall.cone([cx, cy, 1.38], 1.30, 0.62, 0.0, rgba(theme::STONE_DARK, 1.0), Material::STONE);
        tall.sphere([cx, cy, 1.74], 0.24, cap, Material::METAL);
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
        d.cube_mat(
            [mid[0], mid[1], 0.055],
            [len + 0.26, ROAD_HALF * 2.0 + 0.30, 0.30],
            yaw,
            rgba(theme::ROAD_EDGE, 1.0),
            Material::EARTH,
        );
        d.cube_mat(
            [mid[0], mid[1], 0.145],
            [len + 0.22, ROAD_HALF * 2.0, 0.12],
            yaw,
            rgba(theme::ROAD, 1.0),
            Material::EARTH,
        );
    }
    // Kerb stones: rounded cobbles down both verges, so the road has an edge
    // instead of ending at a hard rectangle.
    let n = (g.board.total / 0.9) as i32;
    for i in 0..=n {
        let dist = (i as f32 * 0.9).min(g.board.total);
        let p = g.board.sample(dist);
        let hd = g.board.heading(dist);
        let side = [-hd[1], hd[0]];
        let jitter = hash2(i, 17);
        for s in [-1.0f32, 1.0] {
            let r = ROAD_HALF + 0.20 + jitter * 0.05;
            d.shape(
                Shape::Sphere,
                [p[0] + side[0] * s * r, p[1] + side[1] * s * r, 0.19],
                [0.34 + jitter * 0.1, 0.30, 0.20],
                hd[1].atan2(hd[0]) + jitter,
                0.0,
                rgba(mix(theme::ROAD_EDGE, theme::STONE, 0.20 + jitter * 0.3), 1.0),
                Material::STONE,
                0.0,
            );
        }
    }
}

/// Spawn and exit portals: paired obelisks under a stone lintel, with a lit
/// runestone in each. They mark the two ends of the run from any camera angle.
fn gates_static(g: &Game, d: &mut DrawList) {
    for (dist, col) in [(0.9f32, theme::SPAWN), (g.board.total - 0.9, theme::EXIT)] {
        let p = g.board.sample(dist);
        let dir = g.board.heading(dist);
        let side = [-dir[1], dir[0]];
        let yaw = dir[1].atan2(dir[0]);
        for s in [-1.0f32, 1.0] {
            let px = p[0] + side[0] * s * (ROAD_HALF + 0.34);
            let py = p[1] + side[1] * s * (ROAD_HALF + 0.34);
            // Stepped plinth, tapered shaft, capital, finial.
            d.cylinder([px, py, 0.22], 0.92, 0.30, yaw, rgba(theme::STONE_DARK, 1.0), Material::STONE);
            d.shape(
                Shape::Cone,
                [px, py, 0.90],
                [0.64, 0.64, 1.90],
                yaw,
                0.0,
                rgba(theme::STONE, 1.0),
                Material::STONE,
                0.0,
            );
            d.cylinder([px, py, 1.42], 0.60, 0.16, yaw, rgba(theme::STONE_DARK, 1.0), Material::STONE);
            d.shape(
                Shape::Prism,
                [px, py, 1.62],
                [0.34, 0.34, 0.30],
                yaw,
                0.0,
                rgba(col, 1.0),
                Material::GEM,
                1.0,
            );
            d.cone([px, py, 1.86], 0.40, 0.26, yaw, rgba(theme::STONE, 1.0), Material::STONE);
        }
        // Lintel spanning the road, with a runeband cut into its underside.
        let a = [
            p[0] + side[0] * (ROAD_HALF + 0.34),
            p[1] + side[1] * (ROAD_HALF + 0.34),
        ];
        let b = [
            p[0] - side[0] * (ROAD_HALF + 0.34),
            p[1] - side[1] * (ROAD_HALF + 0.34),
        ];
        d.link(
            Shape::Box,
            [a[0], a[1], 1.52],
            [b[0], b[1], 1.52],
            0.34,
            rgba(theme::STONE, 1.0),
            Material::STONE,
            0.0,
        );
        d.link(
            Shape::Cylinder,
            [a[0], a[1], 1.34],
            [b[0], b[1], 1.34],
            0.12,
            rgba(col, 0.95),
            Material::GEM,
            1.0,
        );
        let _ = yaw;
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
    zones(g, d, t);
    shots(g, d);
    beams(g, d);
    build_ghost(g, d, t);
}

fn torches(decor: &Decor, d: &mut DrawList, t: f32) {
    for tor in &decor.torches {
        let flicker =
            0.72 + 0.28 * ((t * 9.0 + tor.phase).sin() * 0.5 + (t * 5.3 + tor.phase).sin() * 0.5);
        // A real flame, not just a light: a cone that leans with the flicker.
        d.shape(
            Shape::Cone,
            [tor.pos[0], tor.pos[1], tor.pos[2] + 0.10 * flicker],
            [0.20, 0.20, 0.30 * flicker + 0.14],
            t * 2.0 + tor.phase,
            ((t * 7.0 + tor.phase).sin()) * 0.14,
            rgba([1.0, 0.66, 0.26], 1.0),
            Material::GEM,
            1.0,
        );
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
        let yaw = hd[1].atan2(hd[0]);
        // Two strokes meeting at a point: an actual chevron, not a dash.
        for s in [-1.0f32, 1.0] {
            d.shape(
                Shape::Box,
                [
                    p[0] - hd[0] * 0.16 - hd[1] * s * 0.16,
                    p[1] - hd[1] * 0.16 + hd[0] * s * 0.16,
                    0.215,
                ],
                [0.46, 0.11, 0.03],
                yaw + s * 0.72,
                0.0,
                rgba([0.62, 0.76, 0.98], a * 1.8),
                Material::GEM,
                0.9,
            );
        }
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

/// Burning road left by a Pyre.
///
/// It has to read as *ground you do not want to walk on* from across the board,
/// because its whole value is positional - so it gets a hard-edged scorch ring,
/// a bed of embers and flames that lean, rather than a soft glow that could be
/// mistaken for a range indicator.
fn zones(g: &Game, d: &mut DrawList, t: f32) {
    for z in &g.zones {
        let fade = (z.life / z.max_life.max(0.01)).clamp(0.0, 1.0);
        // Dies down rather than blinking out.
        let heat = (fade * 1.6).min(1.0);
        let col = mix([1.0, 0.35, 0.10], [1.0, 0.86, 0.42], heat * 0.5);

        d.ground_ring(z.pos, z.radius, 0.10, rgba(col, 0.45 + 0.35 * heat), 28);
        // Embers: a scatter of small hot discs on the road itself.
        let n = (z.radius * 9.0) as i32;
        for i in 0..n {
            let a = i as f32 * 2.399 + z.pos[0];
            let rr = z.radius * (0.15 + 0.75 * ((i * 7 % 11) as f32 / 11.0));
            let p = [z.pos[0] + a.cos() * rr, z.pos[1] + a.sin() * rr];
            let flick = ((t * 5.0 + i as f32 * 1.3).sin() * 0.5 + 0.5) * heat;
            d.shape(
                Shape::Quad,
                [p[0], p[1], 0.21],
                [0.30, 0.30, 1.0],
                a,
                0.0,
                rgba(mix([0.35, 0.10, 0.04], col, flick), 0.55 + 0.4 * flick),
                Material::EARTH,
                0.7 * flick,
            );
        }
        // Flames, leaning as they rise.
        let flames = (z.radius * 5.0) as i32;
        for i in 0..flames {
            let a = i as f32 * 1.9 + t * 0.6;
            let rr = z.radius * 0.62 * ((i % 3) as f32 / 3.0 + 0.35);
            let wob = (t * 6.0 + i as f32 * 2.1).sin();
            let h = (0.35 + 0.28 * (wob * 0.5 + 0.5)) * heat;
            if h < 0.05 {
                continue;
            }
            d.shape(
                Shape::Cone,
                [z.pos[0] + a.cos() * rr, z.pos[1] + a.sin() * rr, 0.22 + h * 0.5],
                [0.26, 0.26, h],
                a,
                wob * 0.16,
                rgba(col, 1.0),
                Material::GEM,
                1.0,
            );
        }
        d.glow([z.pos[0], z.pos[1], 0.45], z.radius * 2.2, 1.9, rgba(col, 0.30 * heat));
    }
}

// ---------------------------------------------------------------- shots

fn shots(g: &Game, d: &mut DrawList) {
    for p in &g.projs {
        let col = tower_color(&TOWERS[p.def]);
        let yaw = p.vel[1].atan2(p.vel[0]);
        let long = p.kind == crate::game::ProjKind::Lance;
        if long {
            // A bolt: shaft plus a conical head, flying nose-first.
            let (c, s) = (yaw.cos(), yaw.sin());
            d.link(
                Shape::Cylinder,
                [p.pos[0] - c * 0.40, p.pos[1] - s * 0.40, p.z],
                [p.pos[0] + c * 0.24, p.pos[1] + s * 0.24, p.z],
                0.09,
                boost(rgba(col, 1.0), 1.2),
                Material::METAL,
                0.55,
            );
            d.shape(
                Shape::Cone,
                [p.pos[0] + c * 0.34, p.pos[1] + s * 0.34, p.z],
                [0.16, 0.16, 0.24],
                yaw,
                std::f32::consts::FRAC_PI_2,
                boost(rgba(col, 1.0), 1.4),
                Material::METAL,
                0.8,
            );
        } else {
            // A shell: a round ball, stretched slightly along its flight.
            d.shape(
                Shape::Sphere,
                [p.pos[0], p.pos[1], p.z],
                [0.34, 0.22, 0.22],
                yaw,
                0.0,
                boost(rgba(col, 1.0), 1.3),
                Material::GEM,
                0.9,
            );
        }
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

// ================================================================ tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Creep, Timed};
    use crate::gfx::STATIC_CAP;
    use crate::gfx::draw::SHAPE_COUNT;

    fn fresh() -> (Game, Decor) {
        let g = Game::new();
        let decor = Decor::build(&g.board);
        (g, decor)
    }

    /// The whole board is uploaded once into a fixed buffer. If a scenery pass
    /// ever overflows it, the far half of the map silently disappears - exactly
    /// the kind of bug nobody reports and everybody sees.
    #[test]
    fn the_static_scene_fits_in_its_buffer() {
        let (g, decor) = fresh();
        let list = build_static(&g, &decor);
        let n = list.casters.solid_count() + list.flat.solid_count();
        assert!(n > 500, "board is suspiciously empty: {n} solids");
        assert!(n <= STATIC_CAP, "static scene overflows: {n} > {STATIC_CAP}");
    }

    /// Every tower must be built from several kinds of primitive, and must not
    /// be mostly boxes. A tower that is a stack of cubes is the exact failure
    /// this whole model pass exists to prevent.
    #[test]
    fn no_tower_is_just_a_pile_of_boxes() {
        let (mut g, _) = fresh();
        g.gold = 5_000_000;
        for i in 0..TOWERS.len() {
            let slot = g
                .board
                .slots
                .iter()
                .position(|s| s.tower.is_none())
                .expect("a free pad");
            // Tiers are 1-based; walk the whole ladder so every level is drawn.
            let tier = (i as u32 % MAX_TIER) + 1;
            g.build_choice = Some((i, tier));
            assert!(g.try_build(slot), "could not build {}", TOWERS[i].id);
            let tw = g.towers.last().unwrap().clone();

            let mut d = DrawList::default();
            towers::draw(&mut d, &tw, false, 3.0);

            let used = (0..SHAPE_COUNT).filter(|&k| !d.solid[k].is_empty()).count();
            assert!(
                used >= 3,
                "{} uses only {used} primitive(s) - that is a box, not a model",
                TOWERS[i].id
            );
            let boxes = d.solid[Shape::Box as usize].len();
            let total = d.solid_count();
            assert!(boxes * 2 <= total, "{} is {boxes}/{total} boxes", TOWERS[i].id);
        }
        g.build_choice = None;
    }

    fn dummy(kind: Kind) -> Creep {
        Creep {
            uid: 1,
            dist: 6.0,
            lane: 0.0,
            pos: [8.0, 6.0],
            facing: 0.4,
            hp: 60.0,
            max_hp: 100.0,
            base_speed: 1.0,
            armor: Armor::Unarmoured,
            kind,
            radius: kind.radius(),
            bounty: 5,
            slow: Timed::default(),
            burn: Timed::default(),
            poison: Timed::default(),
            shred: Timed::default(),
            stun: 0.0,
            stun_dr: 0.0,
            kb_cd: 0.0,
            regen: 0.0,
            splits: 0,
            shield: 40.0,
            max_shield: 80.0,
            heal: 0.0,
            phasing: false,
            slow_off: false,
            flash: 0.0,
            bob: 1.3,
        }
    }

    /// Silhouette is what tells two monsters apart at gameplay zoom, so no two
    /// kinds may be assembled from the same primitives in the same amounts.
    #[test]
    fn every_monster_has_its_own_silhouette() {
        let kinds = [
            Kind::Grunt,
            Kind::Runner,
            Kind::Brute,
            Kind::Swarm,
            Kind::Warden,
            Kind::Mender,
            Kind::Bulwark,
            Kind::Phaser,
            Kind::Boss,
        ];
        let mut prints: Vec<(Kind, Vec<usize>)> = Vec::new();
        for k in kinds {
            let c = dummy(k);
            let mut d = DrawList::default();
            monsters::draw(&mut d, &c);
            let counts: Vec<usize> = (0..SHAPE_COUNT).map(|i| d.solid[i].len()).collect();
            assert!(
                counts.iter().sum::<usize>() >= 5,
                "{k:?} is too simple to read as a creature"
            );
            prints.push((k, counts));
        }
        for a in 0..prints.len() {
            for b in a + 1..prints.len() {
                assert_ne!(
                    prints[a].1, prints[b].1,
                    "{:?} and {:?} are built identically",
                    prints[a].0, prints[b].0
                );
            }
        }
    }
}

#[cfg(test)]
mod budget {
    use super::*;
    use crate::gfx::draw::SHAPE_COUNT;
    use crate::gfx::mesh;

    /// Prints where the per-frame instance and triangle budget actually goes.
    /// Run with `cargo test -- --nocapture budget`.
    #[test]
    fn report() {
        let lib = mesh::build();
        let tris: Vec<usize> = (0..SHAPE_COUNT)
            .map(|i| lib.spans[i].count as usize / 3)
            .collect();
        let cost = |d: &DrawList| -> (usize, usize) {
            let n: usize = d.solid.iter().map(|b| b.len()).sum();
            let t: usize = (0..SHAPE_COUNT).map(|i| d.solid[i].len() * tris[i]).sum();
            (n, t)
        };

        let mut g = Game::new();
        let decor = Decor::build(&g.board);
        let statics = build_static(&g, &decor);
        let mut stat = DrawList::default();
        stat.append_solids(&statics.casters);
        stat.append_solids(&statics.flat);
        let (cn, ct) = cost(&statics.casters);
        let (sn, st) = cost(&stat);
        println!("STATIC   {sn:>6} inst  {st:>8} tris  (shadow casters: {cn} inst, {ct} tris)");

        // A late-game board: every pad filled and maxed, a full wave on the road.
        g.gold = 50_000_000;
        for slot in 0..g.board.slots.len() {
            g.build_choice = Some((slot % TOWERS.len(), MAX_TIER));
            g.try_build(slot);
        }
        g.build_choice = None;
        g.selected = Some(0);
        g.send_wave();
        for _ in 0..600 {
            g.update(1.0 / 60.0);
        }
        println!("towers {}  creeps {}", g.towers.len(), g.creeps.len());

        let mut d = DrawList::default();
        towers_only(&g, &mut d);
        let (tn, tt) = cost(&d);
        println!("TOWERS   {tn:>6} inst  {tt:>8} tris");

        d.clear();
        for c in &g.creeps {
            monsters::draw(&mut d, c);
        }
        let (mn, mt) = cost(&d);
        println!("MONSTERS {mn:>6} inst  {mt:>8} tris");

        d.clear();
        draw_scene(&g, &decor, &mut d, 3.0);
        let (an, at) = cost(&d);
        println!("FRAME    {an:>6} inst  {at:>8} tris  + {} glows", d.glow.len());
        println!("TOTAL    {:>6} inst  {:>8} tris", sn + an, st + at);
        for i in 0..SHAPE_COUNT {
            println!("  shape {i}: {} tris/mesh, {} static, {} dyn", tris[i], stat.solid[i].len(), d.solid[i].len());
        }
    }

    fn towers_only(g: &Game, d: &mut DrawList) {
        for (i, tw) in g.towers.iter().enumerate() {
            towers::draw(d, tw, g.selected == Some(i), g.time);
        }
    }
}
