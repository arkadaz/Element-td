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

/// The map's own palette: a sun-warmed moss field with packed tan earth where
/// the creeps have worn a route through it.
///
/// The board used to be lit like a night level - a blue-grey field under a
/// dark sky - and it looked nothing like the map it is a port of. Green Circle
/// TD wants a readable green field with a warm dirt route, rather than cool
/// stone lanes competing with the tower silhouettes. These are albedo values,
/// so the lighting can still do its work.
pub mod theme {
    use super::Color;
    /// Lit moss: warm enough to feel like a field, dark enough that units and
    /// cards remain the high-contrast information.
    // The field stays dark enough to frame the battle, but its midtone must
    // survive the browser's filmic output and a whole-board camera. The
    // former near-black albedo swallowed roots, road shoulders and contact
    // shadows into one muddy green plane on real mobile/ultrawide captures.
    pub const GRASS_A: [f32; 3] = [0.038, 0.052, 0.016];
    /// The gentle low-frequency turf variation under the grass texture.
    pub const GRASS_B: [f32; 3] = [0.015, 0.027, 0.007];
    pub const GRASS_EDGE: [f32; 3] = [0.007, 0.014, 0.004];
    /// Full-tile outline, lit while you are holding a tower you can afford.
    pub const PAD_ARM: [f32; 3] = [0.33, 0.40, 0.16];
    /// "Your wallet is the problem", not "this plot is the problem".
    pub const PAD_BROKE: [f32; 3] = [0.85, 0.62, 0.24];
    /// Damp travelled soil. The authored earth material supplies stone, root
    /// and moss aggregate; this measured neutral tint keeps it distinct from
    /// both dark grass and hostile formations without the former orange-ribbon
    /// look at browser exposure.
    pub const ROAD: [f32; 3] = [0.052, 0.038, 0.020];
    /// Flattened grass and damp soil just outside the travelled surface.
    pub const ROAD_SHOULDER: [f32; 3] = [0.014, 0.024, 0.006];
    /// `Arck`.
    pub const STONE: [f32; 3] = [0.105, 0.112, 0.092];
    pub const STONE_DARK: [f32; 3] = [0.035, 0.041, 0.031];
    pub const HP_BACK: Color = [0.02, 0.02, 0.02, 0.95];
    pub const HP_FILL: Color = [0.30, 0.86, 0.26, 1.0];
    pub const HP_LOW: Color = [0.95, 0.28, 0.20, 1.0];
    pub const GHOST_OK: [f32; 3] = [0.60, 0.78, 0.34];
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
    road(g, &mut flat, &mut casters);
    gates_static(g, &mut casters);
    casters.append_solids(&decor.statics);
    Statics { casters, flat }
}

/// A flat, clipped corner treatment used only for local, contextual placement
/// hints.  Persistent marks on every legal tile turn a living field into graph
/// paper, so idle terrain deliberately carries no socket lattice at all.
fn corner_marks(d: &mut DrawList, p: [f32; 2], col: Color) {
    const EDGE: f32 = 0.445;
    const LEG: f32 = 0.13;
    const WIDTH: f32 = 0.016;
    for (sx, sy) in [(-1.0f32, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        // Horizontal leg, then vertical leg: together they read as a corner
        // rather than a tiny tile or a permanent UI button.
        d.shape(
            Shape::Quad,
            [
                p[0] + sx * (EDGE - LEG * 0.5),
                p[1] + sy * EDGE,
                GROUND_Z + 0.010,
            ],
            [LEG, WIDTH, 1.0],
            0.0,
            0.0,
            col,
            Material::EARTH,
            0.0,
        );
        d.shape(
            Shape::Quad,
            [
                p[0] + sx * EDGE,
                p[1] + sy * (EDGE - LEG * 0.5),
                GROUND_Z + 0.011,
            ],
            [WIDTH, LEG, 1.0],
            0.0,
            0.0,
            col,
            Material::EARTH,
            0.0,
        );
    }
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

fn terrain(_g: &Game, d: &mut DrawList, _tall: &mut DrawList) {
    let (x0, y0, x1, y1) = field();

    // A continuous moss field, not one quad per logical tile.  The gameplay
    // grid remains fixed in the simulation, but rendering it tile-by-tile made
    // the lawn read as a bright spreadsheet before a single tower was built.
    // This broad physical underlay also continues well beyond the tactical
    // square. A tilted full-board camera sees farther ground at its top edge
    // than the fitted square's logical boundary; the old sixteen-tile apron
    // ended inside that frustum and exposed a blue "missing world" strip.
    // The larger slab is still one real terrain mesh, not a backdrop or a
    // cropped camera, and makes the field read as a meadow continuing beyond
    // the playable arena.
    let (l, b0, r, t) = (x0 as f32, y0 as f32, x1 as f32 + 1.0, y1 as f32 + 1.0);
    let (w, h) = (r - l, t - b0);
    d.slab_mat(
        [(l + r) * 0.5, (b0 + t) * 0.5],
        // A very wide camera needs more than the former thirty-two-tile
        // overhang at its perspective corners.  This is continuous terrain
        // geometry, not a painted backdrop: it removes the exposed blue
        // clear colour while the peripheral woodland gives that outer land a
        // tangible, non-buildable boundary.
        [w + 224.0, h + 224.0],
        GROUND_Z - 0.07,
        0.20,
        rgba(theme::GRASS_EDGE, 1.0),
        Material::EARTH,
    );
    d.ground(
        // The meadow uses the original moss/soil material at a deliberately
        // low world repeat.  It supplies a real broad turf response under the
        // separately modelled roots, shrubs and tower shadows rather than a
        // uniform dark-green plane or a texture-only substitute for geometry.
        GroundTex::Mosswatch,
        [(l + r) * 0.5, (b0 + t) * 0.5, GROUND_Z],
        [w + 224.0, h + 224.0],
        rgba(mix(theme::GRASS_A, theme::GRASS_B, 0.22), 1.0),
        Material::EARTH,
    );
}

/// A continuous, rounded soil route constructed from the same rounded
/// polyline the simulation uses.  The old terrain pass inferred a road from
/// tile centres; its stair-step edge and every-tile seams are why the lane
/// read as a mustard maze.  These overlapping oriented surfaces retain the
/// fixed game lane while giving it shoulders and actual grass geometry at the
/// edge. It intentionally has no parallel rut decals: those read as a race
/// track at overview scale rather than a worn woodland route.
fn road(g: &Game, d: &mut DrawList, tall: &mut DrawList) {
    // The collision corridor remains the authoritative simulation shape.  The
    // visible packed-earth strip is fractionally wider so a mass horde belongs
    // *in* the road rather than marching down a narrow dark slot, while the
    // moss shoulder stops well before the first legal build plots.
    // The collision lane is 1.24 tiles wide.  The former 1.48-tile visible
    // dirt plus a broad feather made the complete board read as a brown ribbon
    // diagram.  Keep the travelled centre fully grounded, then let irregular
    // physical verge growth do the transition instead of an ever-wider tint.
    const SURFACE_HALF: f32 = ROAD_HALF - 0.075;
    const SHOULDER_HALF: f32 = ROAD_HALF + 0.20;
    const FEATHER_HALF: f32 = ROAD_HALF + 0.48;
    for segment in g.board.path.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.015 {
            continue;
        }
        let yaw = dy.atan2(dx);
        let centre = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];

        // Three restrained alpha layers create a moss-to-packed-earth
        // transition. They are still actual route geometry and stay fully
        // deterministic for picking, but no longer form a hard green/brown
        // cutout around every lane.
        d.ground_oriented(
            // Match the broad meadow's real Mosswatch turf at the road edge.
            // The generic bright grass layer made the feather read as a
            // separate painted stripe once the field's material response was
            // restored.
            GroundTex::Mosswatch,
            [centre[0], centre[1], GROUND_Z + 0.002],
            [len + 0.22, FEATHER_HALF * 2.0],
            yaw,
            rgba(theme::ROAD_SHOULDER, 0.12),
            Material::EARTH,
        );
        d.ground_oriented(
            GroundTex::Mosswatch,
            [centre[0], centre[1], GROUND_Z + 0.003],
            [len + 0.16, SHOULDER_HALF * 2.0],
            yaw,
            rgba(theme::ROAD_SHOULDER, 0.28),
            Material::EARTH,
        );
        d.ground_oriented(
            GroundTex::Dirt,
            [centre[0], centre[1], GROUND_Z + 0.006],
            [len + 0.12, SURFACE_HALF * 2.0],
            yaw,
            rgba(theme::ROAD, 0.94),
            Material::EARTH,
        );
    }
    // `round_ring` already emits short tangent segments at every bend. The
    // former oversized circular join stamps made those bends look like a chain
    // of brown cookies, which was no more natural than the original ribbon.
    // Let the continuous rounded polyline carry the travelled soil, then break
    // its edge with rooted geometry below.
    road_worn_edge(g, tall, SURFACE_HALF);
    road_verge_clusters(g, d, SHOULDER_HALF);
    road_edge_detail(g, tall, SHOULDER_HALF);
}

/// Let real rooted grass bite into an otherwise exact travel corridor.
///
/// The route collision query deliberately stays smooth and deterministic, but
/// its visible dirt should not look like an orange vinyl ribbon laid over the
/// meadow.  These low, non-blocking clumps overlap only the painted edge. They
/// are actual mesh plants with their own normals and cast shadows; no UI grid,
/// decal or hidden build exclusion is involved.  Their one-sided, jittered
/// rhythm avoids a hedge while making the road look worn back into grass.
fn road_worn_edge(g: &Game, tall: &mut DrawList, surface_half: f32) {
    let mut dist = 1.15;
    let mut stamp = 0usize;
    while dist < g.board.total - 1.35 {
        let seed = stamp as f32 * 1.913;
        // Deliberate breathing gaps stop the edge from becoming a perfectly
        // spaced grass fence at whole-board scale.
        if stamp % 7 != 3 {
            let centre = g.board.sample(dist);
            let heading = g.board.heading(dist);
            let normal = [-heading[1], heading[0]];
            let side = if stamp % 4 == 1 || stamp % 9 == 0 { -1.0 } else { 1.0 };
            let inset = 0.020 + (seed.sin() * 0.050 + 0.045).abs();
            let p = [
                centre[0] + heading[0] * (seed.cos() * 0.22)
                    + normal[0] * side * (surface_half - inset),
                centre[1] + heading[1] * (seed.cos() * 0.22)
                    + normal[1] * side * (surface_half - inset),
            ];
            let name = if stamp % 23 == 0 { "NatureFern" } else { "NatureGrass" };
            let scale = if name == "NatureFern" {
                0.25 + (seed * 1.7).sin().abs() * 0.07
            } else {
                // The authored grass is low and leaning now; keep the dirt
                // intrusion subtle so a long route does not become a repeated
                // hedge of individual plant clumps.
                0.28 + (seed * 1.7).sin().abs() * 0.07
            };
            let _ = models::draw_downloaded(
                tall,
                name,
                [p[0], p[1], GROUND_Z + 0.020],
                scale,
                seed,
                rgba([0.45, 0.44, 0.30], 1.0),
                Material::FOLIAGE,
                0.0,
            );
        }
        dist += 2.18 + ((stamp as f32 * 1.29).sin() + 1.0) * 0.62;
        stamp += 1;
    }
}

/// Small, asymmetric groups of real grass where a route meets the meadow.
/// These are deliberately sampled by distance along the full path rather than
/// placed one per segment or on both sides: a regular row looks like fence
/// posts even when each object has actual blades.  Each one checks actual live
/// sockets rather than using a coarse distance band, so low edge growth can
/// break the road silhouette without concealing a valid build target.
fn road_verge_clusters(g: &Game, d: &mut DrawList, shoulder_half: f32) {
    let mut dist = 2.2;
    let mut cluster = 0usize;
    while dist < g.board.total - 2.0 {
        let centre = g.board.sample(dist);
        let heading = g.board.heading(dist);
        let normal = [-heading[1], heading[0]];
        let side = if cluster % 3 == 1 { -1.0 } else { 1.0 };
        // A route shoulder needs gaps.  Even actual plant meshes become a
        // decorative fence when they recur in paired stamps on every bend.
        let copies = 1;
        for copy in 0..copies {
            let seed = cluster as f32 * 2.37 + copy as f32 * 1.91;
            let along = (seed.sin() * 0.42) + (copy as f32 - 1.0) * 0.12;
            let offset = shoulder_half + 0.035 + (seed.cos() * 0.15 + 0.08).abs();
            let p = [
                centre[0] + heading[0] * along + normal[0] * side * offset,
                centre[1] + heading[1] * along + normal[1] * side * offset,
            ];
            let clear_of_socket = g.board.slots.iter().all(|slot| {
                let dx = slot.pos[0] - p[0];
                let dy = slot.pos[1] - p[1];
                dx * dx + dy * dy > 0.62 * 0.62
            });
            if clear_of_socket && g.board.dist_to_road(p) > ROAD_HALF + 0.16
            {
                let scale = 0.28 + (seed * 1.71).sin().abs() * 0.08;
                models::draw_downloaded(
                    d,
                    // Grass blades establish a low rooted verge.  A rare
                    // asymmetric fern reads as a local plant, not a repeating
                    // bright star along the entire road.
                    if cluster % 21 == 0 { "NatureFern" } else { "NatureGrass" },
                    [p[0], p[1], GROUND_Z + 0.018],
                    scale,
                    seed,
                    rgba([0.57, 0.55, 0.36], 1.0),
                    Material::FOLIAGE,
                    0.0,
                );
            }
        }
        // A deterministic non-grid spacing makes cluster gaps as visible as
        // the clusters themselves, rather than producing a dotted road edge.
        dist += 5.20 + ((cluster as f32 * 1.41).sin() + 1.0) * 0.86;
        cluster += 1;
    }
}

/// A road earns its age from physical objects casting into it, not from darker
/// texture noise.  These sparse rooted stones, cut stumps and moss clumps sit
/// beyond live tower sockets, so they break the route's engineered ribbon edge
/// without hiding a legal placement or altering the simulation corridor.
fn road_edge_detail(g: &Game, tall: &mut DrawList, shoulder_half: f32) {
    let mut dist = 4.0;
    let mut cluster = 0usize;
    while dist < g.board.total - 3.0 {
        let centre = g.board.sample(dist);
        let heading = g.board.heading(dist);
        let normal = [-heading[1], heading[0]];
        let side = if cluster % 4 == 1 || cluster % 7 == 0 { -1.0 } else { 1.0 };
        let seed = cluster as f32 * 1.719;
        let offset = shoulder_half + 0.30 + (seed.sin() * 0.16).abs();
        let p = [
            centre[0] + heading[0] * (seed.cos() * 0.28) + normal[0] * side * offset,
            centre[1] + heading[1] * (seed.cos() * 0.28) + normal[1] * side * offset,
        ];
        let inside_field = p[0] > 0.25 && p[0] < 23.75 && p[1] > 0.25 && p[1] < 23.75;
        let clear_socket = g.board.slots.iter().all(|slot| {
            let dx = slot.pos[0] - p[0];
            let dy = slot.pos[1] - p[1];
            dx * dx + dy * dy > 0.74 * 0.74
        });
        if inside_field && clear_socket && g.board.dist_to_road(p) > ROAD_HALF + 0.18 {
            // Road edges need rooted, low plant silhouette rather than a row
            // of bright boulder coins. A rare stump or stone tells a story;
            // most clusters are broad actual grass/fern meshes that soften
            // the soil edge without becoming a decorative obstacle course.
            let (name, scale, mat, tint) = match cluster % 11 {
                0 => (
                    "NatureRockA",
                    0.38 + (seed * 1.31).sin().abs() * 0.12,
                    Material::STONE,
                    [0.42, 0.47, 0.39],
                ),
                6 => (
                    "NatureStump",
                    0.40 + (seed * 0.77).cos().abs() * 0.12,
                    Material::WOOD,
                    [0.34, 0.20, 0.060],
                ),
                _ => (
                    if cluster % 25 == 0 { "NatureFern" } else { "NatureGrass" },
                    0.30 + (seed * 0.61).sin().abs() * 0.09,
                    Material::FOLIAGE,
                    [0.55, 0.53, 0.35],
                ),
            };
            let _ = models::draw_downloaded(
                tall,
                name,
                [p[0], p[1], GROUND_Z + 0.020],
                scale,
                seed,
                rgba(tint, 1.0),
                mat,
                0.0,
            );
        }
        // Intentional gaps matter: a constant hedge is merely another road
        // border. This is a loose record of use and erosion around bends.
        dist += 6.10 + ((cluster as f32 * 0.93).sin() + 1.0) * 1.20;
        cluster += 1;
    }
}

/// Spawn portal: a small landmark at the source junction. It must read at a
/// glance without becoming a building-sized obstruction or a permanent target
/// ring over the first lane.
fn gates_static(g: &Game, d: &mut DrawList) {
    for dist in [board::SPAWN_DIST + 0.45] {
        let p = g.board.sample(dist);
        let dir = g.board.heading(dist);
        let yaw = dir[1].atan2(dir[0]);
        models::draw_downloaded(
            d,
            "DemonGate",
            [p[0], p[1], 0.035],
            0.72,
            yaw,
            rgba([0.78, 0.73, 0.68], 1.0),
            Material::STONE,
            0.04,
        );
        d.sphere_lit([p[0], p[1], 0.42], 0.075, rgba(theme::SPAWN, 0.92), 0.55);
    }
}

// ================================================================ dynamic

pub fn draw_scene(g: &Game, _decor: &Decor, d: &mut DrawList, t: f32) {
    gate_glow(g, d, t);
    chevrons(g, d, t);
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

fn gate_glow(g: &Game, d: &mut DrawList, t: f32) {
    let pulse = 0.55 + 0.45 * (t * 2.0).sin();
    for dist in [board::SPAWN_DIST + 0.9] {
        let p = g.board.sample(dist);
        d.glow(
            [p[0], p[1], 0.34],
            0.64 * pulse.max(0.6),
            2.0,
            rgba(theme::SPAWN, 0.18),
        );
    }
}

/// Two separated chevron streams show the source map's split route.
fn chevrons(g: &Game, d: &mut DrawList, t: f32) {
    // Route direction is useful before a send, but in combat it is just a
    // second animated HUD painted underneath the horde.  The board itself and
    // moving enemies already establish direction once a fight is live.
    if g.phase != Phase::Build {
        return;
    }
    let n = (g.board.total / 4.2) as i32;
    for direction in [-1.0f32, 1.0] {
        for i in 0..n {
            let phase = (t * 0.85 + i as f32 * 0.5).rem_euclid(1.0);
            let dist = (i as f32 * 4.2 + phase * 4.2).min(g.board.total);
            let a = 0.055 * (1.0 - (phase - 0.5).abs() * 2.0).max(0.0);
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
                    rgba([0.37, 0.31, 0.17], a),
                    Material::EARTH,
                    0.0,
                );
            }
        }
    }
}

fn build_ghost(g: &Game, d: &mut DrawList, t: f32) {
    let (Some((def_i, _)), Some(raw_pos)) = (g.build_choice, g.hover_pos) else {
        return;
    };
    let def = &TOWERS[def_i];
    let p = g.board.quantize_build_pos(raw_pos);
    let ok = g.buildability_at(p, true).is_ok();
    let pulse = 0.55 + 0.25 * (t * 5.0).sin();

    if ok {
        towers::draw_ghost(d, def_i, p, t);
        d.ground_ring(
            p,
            TOWERS[def_i].range,
            // A preview answers one question ("will it reach?") and should
            // not turn a dense horde into a white spotlight.
            0.035,
            rgba(tower_color(def), 0.30),
            48,
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
        // Count visual work as well as instance records. The old floor used
        // hundreds of one-primitive grass ticks; the real baked grass clumps
        // replace those with fewer instances but far richer geometry. A check
        // that only rewarded instance count would actively push the scene back
        // toward sparse, toy-like markers.
        let library = crate::gfx::mesh::build();
        let triangles: usize = (0..SHAPE_COUNT)
            .map(|i| {
                let count = list.casters.solid[i].len() + list.flat.solid[i].len();
                count * library.spans[i].count as usize / 3
            })
            .sum();
        // One continuous physical field and grouped high-detail cover use far
        // fewer records than the former per-tile marker carpet.  These bounds
        // catch an accidental loss of the terrain/decor upload without
        // incentivising repeated visual noise merely to inflate an instance
        // counter.
        assert!(n > 260, "board is suspiciously empty: {n} solids");
        assert!(
            triangles > 65_000,
            "board lacks enough real terrain/cover geometry: {triangles} triangles"
        );
        assert!(
            n <= STATIC_CAP,
            "static scene overflows: {n} > {STATIC_CAP}"
        );
    }

    #[test]
    fn idle_scene_has_no_permanent_socket_lattice() {
        let g = Game::new();
        let mut d = DrawList::default();
        build_ghost(&g, &mut d, 0.0);
        assert!(
            d.solid_count() == 0,
            "idle board drew placement decorations instead of terrain"
        );
    }

    #[test]
    fn armed_free_grass_draws_one_ghost_not_a_socket_grid() {
        let mut g = Game::new();
        g.gold = 1_000_000;
        g.build_choice = Some((0, 1));
        g.hover_pos = g.first_clear_grass();
        let mut d = DrawList::default();
        build_ghost(&g, &mut d, 0.0);

        // The live feedback is one physical tower/range preview. The former
        // pad implementation could emit hundreds of coloured tile markers.
        let count = d.solid_count();
        assert!(count > 0, "armed grass position drew no ghost");
        // An imported model is a real multi-material assembly, so it may use
        // far more records than the old four bars. It must still remain a
        // single local preview rather than one record per historical pad.
        assert!(
            count < g.board.slots.len(),
            "armed state rebuilt a socket lattice: {count} records"
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
            shield: 0.0,
            max_shield: 0.0,
            regen_per_second: 0.0,
            resistant: false,
            campaign_encounter: 0,
            pressure: 1.0,
            death_killer: None,
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
