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

pub mod models;
pub mod monsters;
pub mod towers;

use crate::decor::Decor;
use crate::game::board::{self, ROAD_HALF};
use crate::game::defs::*;
use crate::game::greentd_map::ARENA;
use crate::game::{Game, Phase};
use crate::gfx::draw::{Color, DrawList, GroundTex, Material, Shape, boost, mix, rgba};

/// The map's own palette: Warcraft III's Lordaeron Summer tileset, in
/// daylight.
///
/// The board used to be lit like a night level - a blue-grey field under a
/// dark sky - and it looked nothing like the map it is a port of. Green Circle
/// TD is a bright green field with tan dirt corridors cut through it, and these
/// are those colours: `Agrs` and `Agrd` for the turf, `Adrt` for the corridors,
/// `Arck` for the stone. Albedo values, so the lighting can do its work.
pub mod theme {
    use super::Color;
    /// `Agrs`, the lit grass. Mossy, not lime.
    pub const GRASS_A: [f32; 3] = [0.050, 0.084, 0.043];
    /// `Agrd`, the darker patches the tileset mixes through it.
    pub const GRASS_B: [f32; 3] = [0.034, 0.060, 0.036];
    pub const GRASS_EDGE: [f32; 3] = [0.024, 0.045, 0.030];
    /// Corner markers, lit only while you are holding a tower you can afford.
    pub const PAD_ARM: [f32; 3] = [0.40, 0.78, 0.95];
    /// "Your wallet is the problem", not "this plot is the problem".
    pub const PAD_BROKE: [f32; 3] = [0.85, 0.62, 0.24];
    /// `Arck`, the corridors: cool worn rock like the reference lane, not the
    /// tan plank-like surface the first standalone pass produced.
    pub const ROAD: [f32; 3] = [0.118, 0.122, 0.112];
    pub const ROAD_EDGE: [f32; 3] = [0.052, 0.058, 0.054];
    /// `Arck`.
    pub const STONE: [f32; 3] = [0.255, 0.230, 0.184];
    pub const STONE_DARK: [f32; 3] = [0.125, 0.111, 0.091];
    pub const WALL: [f32; 3] = [0.094, 0.088, 0.074];
    pub const HP_BACK: Color = [0.02, 0.02, 0.02, 0.95];
    pub const HP_FILL: Color = [0.30, 0.86, 0.26, 1.0];
    pub const HP_LOW: Color = [0.95, 0.28, 0.20, 1.0];
    pub const GHOST_OK: [f32; 3] = [0.42, 0.85, 1.00];
    pub const GHOST_BAD: [f32; 3] = [1.00, 0.32, 0.38];
    pub const SPAWN: [f32; 3] = [1.00, 0.30, 0.36];
}

/// How many monsters may be on the ring before their models drop to the coarse
/// build. Above this a wave is a crowd rather than a cast, and the detail is
/// both invisible and expensive.
const CROWD: usize = 40;

/// Height of the ground plane. Everything on the board sits flush on it - the
/// terrain is flat, as it is in the map, and relief comes from what stands on
/// it rather than from the floor itself.
pub const GROUND_Z: f32 = 0.10;

/// Kept under its old name: where a tower's plinth starts.
pub const PLOT_TOP: f32 = GROUND_Z;

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

/// The arena: min x, min y, max x, max y in tiles, with a tile of margin so the
/// wall has something to stand on.
fn field() -> (i32, i32, i32, i32) {
    (
        ARENA[0] as i32,
        ARENA[1] as i32,
        ARENA[2] as i32,
        ARENA[3] as i32,
    )
}

fn terrain(g: &Game, d: &mut DrawList, tall: &mut DrawList) {
    let (x0, y0, x1, y1) = field();

    // One flat plane under everything, so no gap between tiles can ever show
    // the sky through the floor.
    let (l, b0, r, t) = (x0 as f32, y0 as f32, x1 as f32 + 1.0, y1 as f32 + 1.0);
    d.slab_mat(
        [(l + r) * 0.5, (b0 + t) * 0.5],
        [r - l, t - b0],
        GROUND_Z - 0.02,
        0.5,
        rgba(theme::GRASS_EDGE, 1.0),
        Material::EARTH,
    );

    for ty in y0..=y1 {
        for tx in x0..=x1 {
            let p = [tx as f32 + 0.5, ty as f32 + 0.5];
            // Follow the compact, rounded runtime path rather than the
            // eight-player corridor pixels still present in the source map.
            let corridor = g.board.dist_to_road(p) <= ROAD_HALF + 0.48;

            // Terrain is drawn as **flat quads**, not as boxes.
            //
            // The box mesh is chamfered by twelve percent on every edge - which
            // is right for a crate and wrong for a floor. Fifteen hundred of
            // them side by side gave the field a quilted, corduroy surface with
            // a bright bevel around every single tile, and that, more than any
            // colour, is what stopped the board reading as ground.
            let (z, base) = if corridor {
                (GROUND_Z - 0.012, theme::ROAD)
            } else {
                // Turf in patches rather than per-tile noise. White noise on a
                // grid reads as graph paper; a low-frequency blend reads as a
                // field, which is what Warcraft III's tilesets do with four
                // variants of one texture.
                // Barely any. This used to swing the whole way from GRASS_A to
                // GRASS_EDGE in three-tile blocks, which was the right call
                // when a tile was one flat colour and is the wrong one now
                // there is a grass texture underneath: the blocks read as a
                // chequerboard laid over the grain. The texture is the
                // variation; this is only enough to stop it tiling visibly.
                // Keep the base continuous across tiles. Large per-tile colour
                // blocks were visible as rectangles from the tactical camera;
                // the shader supplies broad, world-space variation instead.
                let c = mix(theme::GRASS_A, theme::GRASS_B, 0.18);
                (GROUND_Z, c)
            };
            // The tile's colour still comes from the palette measured against
            // a Warcraft III screenshot; the texture multiplies into it. Doing
            // it that way keeps the field the right green - a photographic
            // grass albedo on its own is far yellower than Lordaeron Summer -
            // while giving it the grain a flat fill was missing.
            d.ground(
                if corridor {
                    GroundTex::Stone
                } else {
                    GroundTex::Grass
                },
                [p[0], p[1], z],
                [1.0, 1.0],
                rgba(base, 1.0),
                Material::EARTH,
            );

            // The edge where turf meets corridor, and only there: a couple of
            // hundred strips rather than a bevel on every tile in the field.
            if corridor {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let neighbour = [(tx + dx) as f32 + 0.5, (ty + dy) as f32 + 0.5];
                    if g.board.dist_to_road(neighbour) <= ROAD_HALF + 0.48 {
                        continue;
                    }
                    d.shape(
                        Shape::Quad,
                        [p[0] + dx as f32 * 0.44, p[1] + dy as f32 * 0.44, z + 0.004],
                        [
                            if dx == 0 { 1.0 } else { 0.14 },
                            if dy == 0 { 1.0 } else { 0.14 },
                            1.0,
                        ],
                        0.0,
                        0.0,
                        rgba(theme::ROAD_EDGE, 1.0),
                        Material::EARTH,
                        0.0,
                    );
                }
                continue;
            }
        }
    }

    // A low wall around the plot, so the board reads as a solid object: a
    // rusticated base, a chamfered course, then a rounded coping.
    let (fx0, fy0, fx1, fy1) = field();
    let (l, b0, r, t) = (fx0 as f32, fy0 as f32, fx1 as f32 + 1.0, fy1 as f32 + 1.0);
    let (w, h) = (r - l, t - b0);
    let (mx, my) = ((l + r) * 0.5, (b0 + t) * 0.5);
    let wall = rgba(theme::WALL, 1.0);
    let cap = rgba(theme::STONE, 1.0);
    for (cx, cy, sx, sy) in [
        (mx, b0 - 0.4, w + 1.6, 0.8),
        (mx, t + 0.4, w + 1.6, 0.8),
        (l - 0.4, my, 0.8, h + 1.6),
        (r + 0.4, my, 0.8, h + 1.6),
    ] {
        tall.cube_mat([cx, cy, 0.20], [sx, sy, 0.56], 0.0, wall, Material::STONE);
        tall.cube_mat(
            [cx, cy, 0.50],
            [sx * 0.99, sy * 0.99, 0.10],
            0.0,
            cap,
            Material::STONE,
        );
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
    for (cx, cy) in [
        (l - 0.4, b0 - 0.4),
        (r + 0.4, b0 - 0.4),
        (l - 0.4, t + 0.4),
        (r + 0.4, t + 0.4),
    ] {
        tall.cylinder(
            [cx, cy, 0.45],
            1.10,
            1.10,
            0.0,
            rgba(theme::STONE_DARK, 1.0),
            Material::STONE,
        );
        tall.cylinder([cx, cy, 1.03], 1.24, 0.14, 0.0, cap, Material::STONE);
        tall.cone(
            [cx, cy, 1.38],
            1.30,
            0.62,
            0.0,
            rgba(theme::STONE_DARK, 1.0),
            Material::STONE,
        );
        tall.sphere([cx, cy, 1.74], 0.24, cap, Material::METAL);
    }
    let _ = theme::GRASS_EDGE;
}

fn road(g: &Game, d: &mut DrawList) {
    // The textured corridor tiles are the road surface. Continuous low kerbs
    // read cleanly from the tactical camera. The former
    // chain of hundreds of spheres looked like beads and added noise precisely
    // where units need a crisp silhouette.
    for w in g.board.path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-4 {
            continue;
        }
        let side = [-dy / len, dx / len];
        for s in [-1.0f32, 1.0] {
            let off = ROAD_HALF + 0.18;
            d.link(
                Shape::Capsule,
                [a[0] + side[0] * s * off, a[1] + side[1] * s * off, 0.20],
                [b[0] + side[0] * s * off, b[1] + side[1] * s * off, 0.20],
                0.13,
                rgba(mix(theme::ROAD_EDGE, theme::STONE, 0.34), 1.0),
                Material::STONE,
                0.0,
            );
        }
    }
}

/// Spawn portal: a downloaded stone arch with a restrained magical halo.
/// The old asset was a sci-fi landing pad, which did not match either its icon
/// or this fantasy battlefield. The arch gives the source junction a vertical,
/// readable silhouette without occupying a build pad.
fn gates_static(g: &Game, d: &mut DrawList) {
    for (dist, col) in [(board::SPAWN_DIST + 0.45, theme::SPAWN)] {
        let p = g.board.sample(dist);
        let dir = g.board.heading(dist);
        let yaw = dir[1].atan2(dir[0]);
        models::draw_downloaded(
            d,
            "DemonGate",
            [p[0], p[1], 0.035],
            1.22,
            yaw,
            rgba([0.78, 0.73, 0.68], 1.0),
            Material::STONE,
            0.04,
        );
        d.ground_ring([p[0], p[1]], 0.78, 0.055, rgba(col, 0.42), 40);
        d.sphere_lit([p[0], p[1], 0.64], 0.105, rgba(col, 0.92), 0.55);
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
    // Fine detail while the ring is thin enough for it to be seen, and dropped
    // once a wave fills the corridor - see `monsters::draw`.
    let detail = g.creeps.len() <= CROWD;
    for c in &g.creeps {
        monsters::draw(d, c, detail);
    }
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
        d.glow(
            tor.pos,
            1.3 * flicker,
            2.2,
            rgba([1.0, 0.58, 0.20], 0.42 * flicker),
        );
    }
}

fn gate_glow(g: &Game, d: &mut DrawList, t: f32) {
    let pulse = 0.55 + 0.45 * (t * 2.0).sin();
    for (dist, col) in [(board::SPAWN_DIST + 0.9, theme::SPAWN)] {
        let p = g.board.sample(dist);
        d.glow(
            [p[0], p[1], 0.34],
            0.64 * pulse.max(0.6),
            2.0,
            rgba(col, 0.24),
        );
        d.ground_ring([p[0], p[1]], 0.72, 0.055, rgba(col, 0.38 * pulse), 32);
    }
}

/// Two separated chevron streams show the source map's split route.
fn chevrons(g: &Game, d: &mut DrawList, t: f32) {
    let n = (g.board.total / 4.2) as i32;
    for direction in [-1.0f32, 1.0] {
        for i in 0..n {
            let phase = (t * 0.85 + i as f32 * 0.5).rem_euclid(1.0);
            let dist = (i as f32 * 4.2 + phase * 4.2).min(g.board.total);
            let a = 0.13 * (1.0 - (phase - 0.5).abs() * 2.0).max(0.0);
            if a <= 0.01 {
                continue;
            }
            let centre = g.board.sample_travel(dist, direction);
            let hd = g.board.heading_travel(dist, direction);
            let p = [centre[0] - hd[1] * 0.22, centre[1] + hd[0] * 0.22];
            let yaw = hd[1].atan2(hd[0]);
            // Two strokes meeting at a point: an actual chevron, not a dash.
            for s in [-1.0f32, 1.0] {
                d.shape(
                    Shape::Box,
                    [
                        p[0] - hd[0] * 0.13 - hd[1] * s * 0.11,
                        p[1] - hd[1] * 0.13 + hd[0] * s * 0.11,
                        0.215,
                    ],
                    [0.23, 0.045, 0.014],
                    yaw + s * 0.72,
                    0.0,
                    rgba([0.54, 0.68, 0.78], a),
                    Material::GEM,
                    0.16,
                );
            }
        }
    }
}

/// Build plots are part of the static terrain, so all that is drawn here is the
/// state: which are free while you hold a tower, and which one you are pointing at.
fn plots(g: &Game, d: &mut DrawList, t: f32) {
    let Some((def_i, _)) = g.build_choice else {
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

    // Only spaced, useful plots light up. Their three-tile lattice is deliberate:
    // tower silhouettes stay separate and a location is a commitment.
    let affordable = g.can_afford(TOWERS[def_i].gold);
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
            d.glow(
                [s.pos[0], s.pos[1], PLOT_TOP + 0.35],
                0.85,
                2.2,
                rgba(theme::GHOST_OK, 0.28),
            );
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
    let (Some((def_i, _)), Some(slot)) = (g.build_choice, g.hover_slot) else {
        return;
    };
    let Some(s) = g.board.slots.get(slot) else {
        return;
    };
    let def = &TOWERS[def_i];
    let ok = s.tower.is_none() && g.can_afford(def.gold);
    let p = s.pos;
    let pulse = 0.55 + 0.25 * (t * 5.0).sin();

    if ok {
        towers::draw_ghost(d, def_i, p, t);
        d.ground_ring(
            p,
            TOWERS[def_i].range,
            0.10,
            rgba(tower_color(def), 0.55),
            80,
        );
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
        let col = TOWERS[p.def].family.fx_color();
        let yaw = p.vel[1].atan2(p.vel[0]);
        let (c, s) = (yaw.cos(), yaw.sin());
        let at = [p.pos[0], p.pos[1], p.z];
        use crate::game::ProjKind;
        match p.kind {
            ProjKind::Dart => {
                d.sphere_lit(at, 0.12, boost(rgba(col, 1.0), 1.25), 0.62);
                d.link(
                    Shape::Cone,
                    [p.pos[0] - c * 0.22, p.pos[1] - s * 0.22, p.z],
                    at,
                    0.075,
                    boost(rgba(col, 0.9), 1.15),
                    Material::GEM,
                    0.42,
                );
            }
            ProjKind::Shell => {
                d.sphere_lit(at, 0.22, rgba([0.18, 0.16, 0.14], 1.0), 0.0);
                d.sphere_lit(
                    [p.pos[0] - c * 0.08, p.pos[1] - s * 0.08, p.z + 0.06],
                    0.075,
                    rgba(col, 1.0),
                    0.36,
                );
            }
            ProjKind::Glaive => {
                d.shape(
                    Shape::Cylinder,
                    at,
                    [0.28, 0.28, 0.065],
                    p.life * 11.0,
                    0.0,
                    boost(rgba(col, 1.0), 1.16),
                    Material::METAL,
                    0.38,
                );
                d.sphere_lit(at, 0.075, rgba([0.88, 0.90, 0.96], 1.0), 0.12);
            }
            ProjKind::Bolt => {
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
                    -std::f32::consts::FRAC_PI_2,
                    boost(rgba(col, 1.0), 1.4),
                    Material::METAL,
                    0.8,
                );
            }
            ProjKind::Acid => {
                d.shape(
                    Shape::Sphere,
                    at,
                    [0.22, 0.16, 0.16],
                    yaw,
                    0.0,
                    boost(rgba(col, 0.94), 1.12),
                    Material::GEM,
                    0.48,
                );
                d.sphere_lit(
                    [p.pos[0] - c * 0.14, p.pos[1] - s * 0.14, p.z + 0.07],
                    0.09,
                    rgba([0.76, 1.0, 0.18], 0.90),
                    0.35,
                );
            }
            ProjKind::Missile => {
                d.link(
                    Shape::Capsule,
                    [p.pos[0] - c * 0.30, p.pos[1] - s * 0.30, p.z],
                    [p.pos[0] + c * 0.20, p.pos[1] + s * 0.20, p.z],
                    0.11,
                    rgba([0.58, 0.66, 0.74], 1.0),
                    Material::METAL,
                    0.05,
                );
                d.link(
                    Shape::Cone,
                    [p.pos[0] + c * 0.12, p.pos[1] + s * 0.12, p.z],
                    [p.pos[0] + c * 0.34, p.pos[1] + s * 0.34, p.z],
                    0.12,
                    rgba(col, 1.0),
                    Material::METAL,
                    0.24,
                );
                d.sphere_lit(
                    [p.pos[0] - c * 0.34, p.pos[1] - s * 0.34, p.z],
                    0.085,
                    rgba([1.0, 0.50, 0.12], 0.9),
                    0.52,
                );
            }
            ProjKind::Chaos => {
                d.shape(
                    Shape::Prism,
                    at,
                    [0.26, 0.18, 0.38],
                    yaw + p.life * 5.0,
                    std::f32::consts::FRAC_PI_2,
                    boost(rgba(col, 1.0), 1.2),
                    Material::GEM,
                    0.72,
                );
            }
            ProjKind::Flame => {
                d.sphere_lit(at, 0.24, rgba([1.0, 0.25, 0.04], 1.0), 0.72);
                d.link(
                    Shape::Cone,
                    [p.pos[0] - c * 0.42, p.pos[1] - s * 0.42, p.z],
                    [p.pos[0] - c * 0.06, p.pos[1] - s * 0.06, p.z],
                    0.15,
                    rgba([1.0, 0.72, 0.10], 0.92),
                    Material::GEM,
                    0.58,
                );
            }
            ProjKind::Orb => {
                d.sphere_lit(at, 0.20, boost(rgba(col, 1.0), 1.20), 0.72);
                let a = p.life * 12.0;
                d.sphere_lit(
                    [
                        p.pos[0] + a.cos() * 0.16,
                        p.pos[1] + a.sin() * 0.16,
                        p.z + 0.06,
                    ],
                    0.065,
                    rgba([0.92, 0.80, 1.0], 0.9),
                    0.38,
                );
            }
            ProjKind::Royal => {
                d.link(
                    Shape::Taper,
                    [p.pos[0] - c * 0.52, p.pos[1] - s * 0.52, p.z],
                    [p.pos[0] + c * 0.30, p.pos[1] + s * 0.30, p.z],
                    0.10,
                    boost(rgba(col, 1.0), 1.35),
                    Material::METAL,
                    0.82,
                );
                d.sphere_lit(at, 0.12, rgba([1.0, 0.96, 0.72], 1.0), 0.72);
            }
        }
        d.glow(
            at,
            if matches!(p.kind, ProjKind::Shell | ProjKind::Flame | ProjKind::Royal) {
                0.28
            } else {
                0.21
            },
            1.5,
            boost(rgba(col, 0.52), 1.15),
        );
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
                0.085,
                boost(rgba(b.color, a * 0.38), 1.14),
                40,
            );
            d.glow(
                [b.from[0], b.from[1], 0.3],
                r.max(0.1) * 1.2,
                1.2,
                rgba(b.color, a * 0.06),
            );
        } else {
            let w = b.width * (0.5 + a * 0.5);
            d.bar(b.from, b.to, w, boost(rgba(b.color, a), 1.35), 0.65);
            d.glow(b.to, w * 2.2, 1.8, boost(rgba(b.color, a * 0.55), 1.18));
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
    use crate::game::{Creep, Proj, ProjKind, Timed};
    use crate::gfx::STATIC_CAP;
    use crate::gfx::draw::SHAPE_COUNT;

    fn fresh() -> (Game, Decor) {
        let g = Game::new();
        let decor = Decor::build(&g.board);
        (g, decor)
    }

    #[test]
    fn bolt_tip_points_along_projectile_velocity() {
        let mut game = Game::new();
        let def = family_start(Family::Multi).expect("multi tower root");
        let velocity = [17.0, 11.0];
        game.projs.push(Proj {
            pos: [5.0, 7.0],
            z: 1.0,
            vel: velocity,
            kind: ProjKind::Bolt,
            tower: 0,
            def,
            dmg: 1.0,
            splash: 0.0,
            bounces: 0,
            crit: false,
            target_idx: 0,
            target_uid: 1,
            life: 1.0,
            trail: 0.0,
        });
        let mut list = DrawList::default();
        shots(&game, &mut list);
        let cone = list.solid[Shape::Cone as usize]
            .last()
            .expect("bolt tip cone");
        let yaw = cone.rot[0];
        let pitch = cone.rot[1];
        // Local +Z transformed by the same yaw/pitch convention as solid.wgsl.
        let forward = [
            -pitch.sin() * yaw.cos(),
            -pitch.sin() * yaw.sin(),
            pitch.cos(),
        ];
        let length = (velocity[0] * velocity[0] + velocity[1] * velocity[1]).sqrt();
        let expected = [velocity[0] / length, velocity[1] / length, 0.0];
        let dot = forward[0] * expected[0] + forward[1] * expected[1];
        assert!(dot > 0.999, "bolt tip points backward: dot={dot}");
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
        assert!(
            n <= STATIC_CAP,
            "static scene overflows: {n} > {STATIC_CAP}"
        );
    }

    /// Every tower must be built from several kinds of primitive, and must not
    /// be mostly boxes. A tower that is a stack of cubes is the exact failure
    /// this whole model pass exists to prevent.
    #[test]
    fn no_tower_is_just_a_pile_of_boxes() {
        let (mut g, _) = fresh();
        g.gold = 500_000_000;
        // There are more towers in the roster than pads on the board, so each
        // is built on the same pad, drawn, and sold again.
        for i in 0..TOWERS.len() {
            g.build_choice = Some((i, 1));
            assert!(g.try_build(0), "could not build {}", TOWERS[i].name);
            let tw = g.towers[0].clone();

            let mut d = DrawList::default();
            towers::draw(&mut d, &tw, false, 3.0);

            let used = (0..SHAPE_COUNT).filter(|&k| !d.solid[k].is_empty()).count();
            assert!(
                used >= 3,
                "{} uses only {used} primitive(s) - that is a box, not a model",
                TOWERS[i].name
            );
            let boxes = d.solid[Shape::Box as usize].len();
            let total = d.solid_count();
            assert!(
                boxes * 2 <= total,
                "{} is {boxes}/{total} boxes",
                TOWERS[i].name
            );
            let baked: usize = (crate::gfx::mesh::PRIM_COUNT..SHAPE_COUNT)
                .map(|bucket| d.solid[bucket].len())
                .sum();
            assert_eq!(
                baked, 1,
                "{} missed its staged downloaded tower assembly",
                TOWERS[i].name
            );
            g.sell(0);
        }
        g.build_choice = None;
    }

    fn dummy(model: Model, flying: bool) -> Creep {
        Creep {
            laps: 0,
            suppress: 0.0,
            stun_immune: 0.0,
            push_left: 0.0,
            uid: 1,
            dist: 6.0,
            route_dir: 1.0,
            lane: 0.0,
            pos: [8.0, 6.0],
            facing: 0.4,
            hp: 60.0,
            max_hp: 100.0,
            base_speed: 1.0,
            armour: 10,
            armour_type: ArmourType::Unarmoured,
            model,
            flying,
            radius: model.radius(),
            bounty: 5,
            boss: false,
            elite: false,
            slow: Timed::default(),
            burn: Timed::default(),
            poison: Timed::default(),
            shred: Timed::default(),
            stun: 0.0,
            stun_dr: 0.0,
            kb_cd: 0.0,
            flash: 0.0,
            bob: 1.3,
        }
    }

    /// Every model the map actually uses has to be a *model*.
    ///
    /// There are two ways to be one now, and they look completely different in
    /// a draw list. A baked mesh from `assets/models.bin` is a single instance
    /// carrying thousands of triangles; a generated build is dozens of
    /// instances of a handful of primitives. Both are fine. One sphere is not,
    /// and that is what this catches.
    #[test]
    fn every_model_is_actually_built() {
        for m in models_in_use() {
            let c = dummy(m, m.airborne());
            let mut d = DrawList::default();
            monsters::draw(&mut d, &c, true);
            let baked: usize = (crate::gfx::mesh::PRIM_COUNT..SHAPE_COUNT)
                .map(|i| d.solid[i].len())
                .sum();
            if baked > 0 {
                continue;
            }
            let n: usize = (0..SHAPE_COUNT).map(|i| d.solid[i].len()).sum();
            assert!(n >= 5, "{m:?} is too simple to read as anything");
        }
    }

    /// The shipped model pack is not optional in a release-quality build. The
    /// primitive constructions remain a corruption fallback, but an archetype
    /// present in the blob must actually select its one-mesh bucket.
    #[test]
    fn every_shipped_archetype_uses_its_baked_model() {
        for &m in Model::ALL {
            let c = dummy(m, m.airborne());
            let mut d = DrawList::default();
            monsters::draw(&mut d, &c, true);
            let baked: usize = (crate::gfx::mesh::PRIM_COUNT..SHAPE_COUNT)
                .map(|i| d.solid[i].len())
                .sum();
            assert_eq!(baked, 1, "{m:?} did not use its shipped baked mesh");
        }
    }

    /// Silhouette is what tells two things apart at gameplay zoom, so no two
    /// models may be assembled from the same primitives in the same amounts.
    ///
    /// Baked meshes each land in their own bucket, so this separates them for
    /// free; the rule bites on the generated builds, where it is easy to write
    /// two archetypes that differ only in colour.
    #[test]
    fn every_model_has_its_own_silhouette() {
        let mut prints: Vec<(Model, Vec<usize>)> = Vec::new();
        for m in models_in_use() {
            let c = dummy(m, m.airborne());
            let mut d = DrawList::default();
            monsters::draw(&mut d, &c, true);
            prints.push((m, (0..SHAPE_COUNT).map(|i| d.solid[i].len()).collect()));
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

    /// Every model referenced by the map's own data, towers and creeps alike.
    fn models_in_use() -> Vec<Model> {
        let mut v: Vec<Model> = Vec::new();
        for t in TOWERS {
            if !v.contains(&t.model) {
                v.push(t.model);
            }
        }
        for w in WAVES {
            if !v.contains(&w.model) {
                v.push(w.model);
            }
        }
        v
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
            g.build_choice = Some((slot % TOWERS.len(), 1));
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
            monsters::draw(&mut d, c, true);
        }
        let (mn, mt) = cost(&d);
        println!("MONSTERS {mn:>6} inst  {mt:>8} tris");

        d.clear();
        draw_scene(&g, &decor, &mut d, 3.0);
        let (an, at) = cost(&d);
        println!(
            "FRAME    {an:>6} inst  {at:>8} tris  + {} glows",
            d.glow.len()
        );
        println!("TOTAL    {:>6} inst  {:>8} tris", sn + an, st + at);
        for i in 0..SHAPE_COUNT {
            println!(
                "  shape {i}: {} tris/mesh, {} static, {} dyn",
                tris[i],
                stat.solid[i].len(),
                d.solid[i].len()
            );
        }
    }

    fn towers_only(g: &Game, d: &mut DrawList) {
        for (i, tw) in g.towers.iter().enumerate() {
            towers::draw(d, tw, g.selected == Some(i), g.time);
        }
    }
}
