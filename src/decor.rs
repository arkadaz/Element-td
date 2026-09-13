//! Static set dressing: trees, rocks, grass, shrubs, cliffs and water.
//!
//! Generated once from the board layout with a fixed seed, then handed to the
//! renderer as a pre-built bucket of instances every frame. Nothing here affects
//! gameplay - it exists so the board reads as a place rather than a grid.
//!
//! Everything is modelled from the shape library: a tree is a tapered trunk with
//! stacked cone foliage, a rock is a crushed sphere, a fence is turned posts with
//! rails between them.

use crate::game::board::{BW, Board, ROAD_HALF};
use crate::game::greentd_map::ARENA;
use crate::gfx::draw::{DrawList, Material, Shape, rgba};
use crate::rng::Rng;

pub struct Decor {
    /// Everything static, already bucketed by shape.
    pub statics: DrawList,
}

impl Decor {
    pub fn build(board: &Board) -> Self {
        let mut rng = Rng::new(0xD3C0_1234_5678_9ABC);
        let mut d = DrawList::default();
        cliffs(&mut d, board, &mut rng);
        outer_woodland(&mut d, board, &mut rng);
        peripheral_woodland(&mut d, board, &mut rng);
        distant_woodland(&mut d, board, &mut rng);
        scatter(&mut d, board, &mut rng);
        meadow_cover(&mut d, board, &mut rng);

        Self { statics: d }
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

/// An irregular natural verge around the fixed tactical board.  The former
/// four slab walls made the world look like a boxed diorama and left a dead
/// grey frame on wide screens.  This is deliberately scattered in clusters:
/// a player sees a field continuing into scrub, boulders and trees, not a
/// perfectly square fortification that competes with the lane.
fn cliffs(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    // An edge is not a picket fence of alternating rocks and trees. Build a
    // few loose woodland clusters with gaps between them, so the field feels
    // like it continues into cover rather than sitting inside a square of
    // repeated map props.
    let (l, b, r, top) = (ARENA[0], ARENA[1], ARENA[2] + 1.0, ARENA[3] + 1.0);
    for _ in 0..16 {
        let edge = rng.next_u32() % 4;
        let along = rng.range(0.04, 0.96);
        // Let the roots sit just outside the playable meadow.  The full-board
        // camera still sees their canopies and contact shadows against the
        // extended terrain, but a line of oversized trunks no longer invades
        // the build field and makes the fixed board read as a toy diorama.
        let inset = rng.range(-0.10, 0.14);
        let (anchor, tangent, outward) = match edge {
            0 => ([l + along * (r - l), b + inset], [1.0, 0.0], [0.0, -1.0]),
            1 => ([l + along * (r - l), top - inset], [1.0, 0.0], [0.0, 1.0]),
            2 => ([l + inset, b + along * (top - b)], [0.0, 1.0], [-1.0, 0.0]),
            _ => ([r - inset, b + along * (top - b)], [0.0, 1.0], [1.0, 0.0]),
        };
        for member in 0..(3 + rng.next_u32() % 3) {
            let lateral = rng.range(-1.25, 1.25);
            let depth = rng.range(0.18, 1.36);
            let p = [
                anchor[0] + tangent[0] * lateral + outward[0] * depth,
                anchor[1] + tangent[1] * lateral + outward[1] * depth,
            ];
            // A woodland edge may touch the frame, but it may not turn a
            // legitimate shoulder socket into a misleading hidden tile.
            if !is_free(board, p, 0.90) {
                continue;
            }
            let roll = rng.unit();
            // The former foreground tree line was built from dozens of
            // repeated, high-canopy props. At whole-board scale it read as a
            // toy picket of green lollipops and competed with the route. Keep
            // the perimeter grounded with roots, stone and understory; only a
            // rare exterior canopy breaks the horizon.
            if member == 0 && roll < 0.18 {
                let scale = rng.range(0.92, 1.12);
                tree_scaled(d, rng, p, scale);
            } else if roll < 0.33 {
                rocks(d, rng, p);
            } else {
                nature_prop(
                    d,
                    if rng.chance(0.14) { "NatureFern" } else { "NatureBush" },
                    p,
                    rng.range(0.40, 0.66),
                    rng,
                    Material::FOLIAGE,
                );
            }
        }
    }
}
/// A physical boundary beyond the free-build meadow. The full-width camera
/// legitimately sees outside the square tactical world, but showing an empty
/// uninterrupted grass sheet there made that scenery look secretly buildable
/// and left the map floating in a blank studio. These irregular clusters sit
/// outside `BUILD_WORLD`: trunks, canopy, rock and scrub make the transition
/// explicit without stealing a single legal grass position beside the route.
fn outer_woodland(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    const ANCHORS: [[f32; 2]; 18] = [
        [-4.5, 2.0], [-5.1, 8.4], [-4.8, 15.1], [-3.7, 22.5],
        [1.7, 28.2], [8.8, 29.1], [16.2, 28.6], [23.3, 28.0],
        [28.7, 23.0], [29.2, 16.0], [28.7, 8.8], [27.8, 1.3],
        [22.3, -4.5], [15.0, -5.2], [7.2, -4.7], [0.5, -3.9],
        [-2.7, 25.6], [26.1, 26.4],
    ];
    for (cluster, anchor) in ANCHORS.into_iter().enumerate() {
        // The full-width camera sees this outer land directly.  A few isolated
        // props left the tactical route on an empty billiard table; these are
        // compact, irregular groves that make a real dark tree line without
        // invading a single legal grass placement inside BUILD_WORLD.
        let members = 7 + (rng.next_u32() % 4) as usize;
        for member in 0..members {
            let seed = cluster as f32 * 1.73 + member as f32 * 2.41;
            let p = [
                anchor[0] + seed.cos() * rng.range(0.25, 1.45),
                anchor[1] + seed.sin() * rng.range(0.25, 1.20),
            ];
            // This is deliberately an exterior frame, not a new invisible
            // obstacle rule. If a future map moves a cluster inward, do not
            // let it cover a real route shoulder or live build pad.
            if board.dist_to_road(p) < ROAD_HALF + 1.55
                || board.slots.iter().any(|slot| {
                    let dx = slot.pos[0] - p[0];
                    let dy = slot.pos[1] - p[1];
                    dx * dx + dy * dy < 2.0 * 2.0
                })
            {
                continue;
            }
            let tree_scale = rng.range(0.92, 1.24);
            match (cluster * 3 + member) % 7 {
                0 | 1 | 2 => tree_scaled(d, rng, p, tree_scale),
                3 | 4 => rocks(d, rng, p),
                5 => {
                    nature_prop(
                        d,
                        "NatureBush",
                        p,
                        rng.range(0.62, 0.92),
                        rng,
                        Material::FOLIAGE,
                    );
                }
                _ => {
                    let (name, material) = if rng.chance(0.16) {
                        ("NatureFern", Material::FOLIAGE)
                    } else {
                        ("NatureStump", Material::WOOD)
                    };
                    nature_prop(d, name, p, rng.range(0.62, 0.96), rng, material);
                }
            }
        }
    }
}

/// The full-width browser camera deliberately sees beyond the tactical square.
/// That extra land has to be a believable forest floor rather than a sea of
/// empty grass or an opaque decorative curtain.  These broad, irregular
/// exterior groves sit well outside `BUILD_WORLD`; they use real tree, rock,
/// stump and bush meshes so ultrawide players see grounded depth at the frame
/// edges without losing a route segment or a single legal grass placement.
fn peripheral_woodland(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    const GROVES: [[f32; 2]; 18] = [
        [-13.0, 1.8], [-14.4, 8.8], [-13.5, 16.4], [-11.8, 24.2],
        [-5.5, 32.0], [2.8, 34.1], [11.0, 34.7], [20.3, 33.6],
        [29.2, 29.2], [36.5, 23.8], [38.6, 15.8], [37.2, 7.1],
        [33.6, -2.2], [25.2, -8.8], [16.0, -10.8], [6.7, -10.0],
        [-2.0, -7.8], [-9.5, -3.2],
    ];
    for (grove, anchor) in GROVES.into_iter().enumerate() {
        let members = 4 + (rng.next_u32() % 3) as usize;
        for member in 0..members {
            let seed = grove as f32 * 1.931 + member as f32 * 2.719;
            let p = [
                anchor[0] + seed.cos() * rng.range(0.35, 1.85),
                anchor[1] + seed.sin() * rng.range(0.28, 1.45),
            ];
            // This stays a render-only exterior layer. Keep the defensive
            // guard anyway: a future larger arena must not accidentally turn
            // a visual grove into a hidden route/build obstruction.
            if board.dist_to_road(p) < ROAD_HALF + 2.2
                || board.slots.iter().any(|slot| {
                    let dx = slot.pos[0] - p[0];
                    let dy = slot.pos[1] - p[1];
                    dx * dx + dy * dy < 2.6 * 2.6
                })
            {
                continue;
            }
            let tree_scale = rng.range(1.18, 1.52);
            match (grove * 3 + member) % 6 {
                // These groves make the wide browser frame a woodland basin.
                // The crown assets vary by source and rotation, while grouped
                // scale/overlap keeps the perimeter from reading as a picket
                // row of identical map markers.
                0 | 1 | 2 => tree_scaled(d, rng, p, tree_scale),
                3 | 4 => rocks(d, rng, p),
                _ => {
                    let (name, material) = if rng.chance(0.56) {
                        ("NatureBush", Material::FOLIAGE)
                    } else if rng.chance(0.50) {
                        ("NatureStump", Material::WOOD)
                    } else {
                        ("NatureBush", Material::FOLIAGE)
                    };
                    nature_prop(
                        d,
                        name,
                        p,
                        rng.range(0.76, 1.12),
                        rng,
                        material,
                    );
                }
            }
        }
    }
}

/// The wide browser viewport legitimately shows much more land beside a
/// square tactical field.  Leaving that land as uninterrupted turf made the
/// route look like a small toy track dropped on a green table.  These are
/// distant *physical* groves well beyond the legal grass envelope: their
/// irregular trunks, canopy and contact shadows give the full-width camera a
/// forest basin without turning a side panel into a decorative backdrop or
/// hiding any accessible placement ground.
fn distant_woodland(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    const BANDS: [f32; 4] = [-34.0, -20.0, 44.0, 58.0];
    for (band, x) in BANDS.into_iter().enumerate() {
        for row in 0..8 {
            let seed = band as f32 * 2.11 + row as f32 * 1.73;
            // A wide screen needs a true woodland mass at its distant edges,
            // not one lonely tree every seven tiles.  Each anchor becomes a
            // small uneven grove: the overlapping canopies make a forest
            // read at overview scale while their trunks and low companions
            // still create real depth when the player zooms toward an edge.
            for member in 0..3 {
                let spread = member as f32 * 2.31 + seed;
                let p = [
                    x + seed.sin() * rng.range(0.55, 2.10) + spread.cos() * (member as f32 * 0.86),
                    -13.0 + row as f32 * 7.20 + seed.cos() * rng.range(0.40, 1.75)
                        + spread.sin() * (member as f32 * 0.70),
                ];
                // Never rely on this being off-board by inspection.  If the
                // tactical envelope grows in a later map pass, a visual grove may
                // not quietly sit across a route or a newly accessible tile.
                if board.dist_to_road(p) < ROAD_HALF + 2.8
                    || (p[0] > -2.5 && p[0] < 26.5 && p[1] > -2.5 && p[1] < 26.5)
                {
                    continue;
                }
                let scale = 0.92 + (seed * 1.37 + member as f32).sin().abs() * 0.44;
                tree_scaled(d, rng, p, scale);
                if (row + band + member) % 4 == 0 {
                    let q = [p[0] + seed.cos() * 0.72, p[1] + seed.sin() * 0.68];
                    nature_prop(d, "NatureFern", q, rng.range(0.42, 0.58), rng, Material::FOLIAGE);
                }
            }
        }
    }
}

/// A few actual woodland clearings keep the broad meadow islands from reading
/// as uninhabited carpet.  These are intentionally clusters of trunk, rock,
/// low cover and a cut stump rather than a repeated tree stamp; their root
/// clearance is checked against live sockets so the visual layer never alters
/// the build language.
fn landmarks(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    const CLEARINGS: [[f32; 2]; 5] = [
        [12.0, 11.6], [10.3, 6.2], [15.7, 6.1], [6.0, 18.1], [17.8, 11.3],
    ];
    for anchor in CLEARINGS {
        let p = [
            anchor[0] + rng.range(-0.42, 0.42),
            anchor[1] + rng.range(-0.36, 0.36),
        ];
        if !landmark_clear(board, p, 1.28) {
            continue;
        }
        // A landmark is a grounded forest-floor story, not a single giant
        // canopy sitting in otherwise buildable grass.
        nature_prop(d, "NatureStump", p, rng.range(0.78, 0.96), rng, Material::WOOD);
        for member in 0..(3 + rng.next_u32() % 3) {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let q = [
                p[0] + a.cos() * rng.range(0.75, 1.45),
                p[1] + a.sin() * rng.range(0.75, 1.45),
            ];
            if !landmark_clear(board, q, 0.42) {
                continue;
            }
            match member % 3 {
                0 => rocks(d, rng, q),
                1 => {
                    nature_prop(d, "NatureStump", q, rng.range(0.42, 0.56), rng, Material::WOOD);
                }
                _ => {
                    nature_prop(d, "NatureFern", q, rng.range(0.78, 1.02), rng, Material::FOLIAGE);
                }
            };
        }
    }
}

/// A handful of grounded woodland islands establish an intentional hierarchy:
/// broad clearing, route, then actual cover.  The old per-tile scatter left the
/// whole board technically decorated but visually vacant, while a perimeter
/// wall of trees would hide build sockets.  These centres are chosen from
/// genuine meadow pockets and checked against the live route/socket geometry.
fn forest_islands(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    const ISLANDS: [[f32; 2]; 7] = [
        // Verified clear pockets on the real rounded C1 route.  The former
        // guessed centres mostly sat inside its dense tower shoulder, so the
        // safety check did its job by rejecting almost every intended island.
        [5.5, 17.5], [8.5, 18.5], [11.5, 18.5], [14.5, 17.5],
        [20.5, 10.5], [21.5, 11.5], [21.5, 22.5],
    ];
    for (index, anchor) in ISLANDS.into_iter().enumerate() {
        let p = [anchor[0] + rng.range(-0.10, 0.10), anchor[1] + rng.range(-0.10, 0.10)];
        if !tree_clear(board, p, 1.14) {
            continue;
        }
        // An irregular field feature gives the route age without another
        // childlike full tree inside the active battlefield. Stump, mossy rock
        // and fern geometry preserve depth while the grass remains visibly
        // open for free tower placement.
        nature_prop(d, "NatureStump", p, rng.range(0.68, 0.90), rng, Material::WOOD);
        for member in 0..3 {
            let a = index as f32 * 1.47 + member as f32 * 2.13;
            let q = [p[0] + a.cos() * (0.70 + member as f32 * 0.16), p[1] + a.sin() * (0.70 + member as f32 * 0.16)];
            if !tree_clear(board, q, 0.36) {
                continue;
            }
            match member {
                0 => {
                    let _ = nature_prop(d, "NatureBush", q, rng.range(0.42, 0.58), rng, Material::FOLIAGE);
                }
                1 => {
                    let _ = nature_prop(d, "NatureStump", q, rng.range(0.38, 0.52), rng, Material::WOOD);
                }
                _ => rocks(d, rng, q),
            };
        }
    }
}

fn tree_clear(board: &Board, p: [f32; 2], radius: f32) -> bool {
    board.dist_to_road(p) > ROAD_HALF + radius
        && board.slots.iter().all(|slot| {
            let dx = slot.pos[0] - p[0];
            let dy = slot.pos[1] - p[1];
            dx * dx + dy * dy > (radius + 0.34) * (radius + 0.34)
        })
}

fn landmark_clear(board: &Board, p: [f32; 2], radius: f32) -> bool {
    board.dist_to_road(p) > ROAD_HALF + radius
        && board.slots.iter().all(|slot| {
            let dx = slot.pos[0] - p[0];
            let dy = slot.pos[1] - p[1];
            dx * dx + dy * dy > (radius + 0.32) * (radius + 0.32)
        })
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

/// Layered woodland over the open ground.
fn scatter(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    // Only the arena the player can see. The rest of the field is corridors
    // belonging to seven other players, and dressing it costs thousands of
    // instances nobody will ever look at.
    let a = crate::game::greentd_map::ARENA;
    for ty in a[1] as i32 - 1..=a[3] as i32 + 1 {
        for tx in a[0] as i32 - 1..=a[2] as i32 + 1 {
            let p = [
                tx as f32 + 0.5 + rng.range(-0.30, 0.30),
                ty as f32 + 0.5 + rng.range(-0.30, 0.30),
            ];
            let verge = board.dist_to_road(p) - ROAD_HALF;
            // Low grass is visual cover, never a build rule.  Keeping it out
            // of every old socket left a bare, artificial construction band;
            // the new free-grass contract requires visible grass to remain
            // placeable through its leaves. Only the travelled route itself
            // stays clear.
            if verge < 1.08 {
                continue;
            }
            let roll = rng.unit();
            // The old scatter was rich in count but not in hierarchy: dozens
            // of pale rocks read as polka dots across an otherwise natural
            // field. Keep a few weighty landmarks, then let lower vegetation
            // carry the meadow texture and leave clear grass legible.
            if roll < 0.026 {
                // Interior grass is first and foremost construction space.
                // Keep only occasional low organic cover there; the old mix
                // of pale stones, cut-stump pots and round bush balls made a
                // clear free-build meadow look arbitrarily obstructed.
                nature_prop(
                    d,
                    if rng.chance(0.08) { "NatureFern" } else { "NatureGrass" }, p,
                    rng.range(0.34, 0.52),
                    rng,
                    Material::FOLIAGE,
                );
            }
        }
    }
}

/// Clumped low cover gives the empty islands between lanes a physical surface
/// without consuming a build socket.  The cluster structure matters: a dozen
/// evenly spaced grass instances reads as a lawn pattern, whereas several
/// independently modelled blades and shrubs gathering around a quiet patch of
/// ground read as a place a player can still build beside.
fn meadow_cover(d: &mut DrawList, board: &Board, rng: &mut Rng) {
    let a = crate::game::greentd_map::ARENA;
    let mut clusters = 0usize;
    let mut attempts = 0usize;
    while clusters < 18 && attempts < 400 {
        attempts += 1;
        let p = [rng.range(a[0] + 1.2, a[2] - 0.2), rng.range(a[1] + 1.2, a[3] - 0.2)];
        // The road's useful shoulder ends around 1.5 tiles out. Keep a real
        // clean construction band, then let vegetation return in the meadow.
        if board.dist_to_road(p) < ROAD_HALF + 1.05 {
            continue;
        }
        clusters += 1;
        // A small clump reads as actual meadow growth; one isolated fern per
        // clearing was another repeated game-prop stamp.  These remain low
        // render-only ground cover, so any visible grass around them is still
        // legal free-build land rather than a hidden socket rule.
        let blades = 3 + (rng.next_u32() % 3) as usize;
        for _ in 0..blades {
            let q = [p[0] + rng.range(-0.46, 0.46), p[1] + rng.range(-0.46, 0.46)];
            if board.dist_to_road(q) > ROAD_HALF + 0.92 {
                nature_prop(
                    d,
                    if rng.chance(0.035) { "NatureFern" } else { "NatureGrass" },
                    q,
                    rng.range(0.40, 0.66),
                    rng,
                    Material::FOLIAGE,
                );
            }
        }
    }
}

fn nature_prop(
    d: &mut DrawList,
    name: &str,
    p: [f32; 2],
    scale: f32,
    rng: &mut Rng,
    material: Material,
) -> bool {
    let variation = rng.unit();
    crate::view::models::draw_downloaded(
        d,
        name,
        [p[0], p[1], 0.025],
        scale,
        rng.range(0.0, std::f32::consts::TAU),
        rgba(
            if matches!(name, "NatureFern" | "NatureGrass") {
                // Keep low cover in the same deep woodland value range as the
                // field.  Bright cyan fronds may be colourful in isolation,
                // but at a fixed tactical camera they become repeated plastic
                // stamps instead of understory.
                // Neutral, muted daylight leaves the source leaf materials in
                // charge.  A green-tinted instance multiplier made every
                // independent frond acquire the same fluorescent map-marker
                // hue in the actual browser renderer.
                [0.82 + variation * 0.035, 0.78 + variation * 0.035, 0.58 + variation * 0.025]
            } else if name == "NatureBush" {
                [0.70 + variation * 0.040, 0.68 + variation * 0.035, 0.48 + variation * 0.025]
            } else {
                [0.42 + variation * 0.055, 0.47 + variation * 0.055, 0.39 + variation * 0.040]
            },
            1.0,
        ),
        material,
        0.0,
    )
}

/// A sparse exterior canopy variant. The tactical meadow retains open grass;
/// only the peripheral horizon gets taller foliage for depth.
fn tree_scaled(d: &mut DrawList, rng: &mut Rng, p: [f32; 2], scale_mul: f32) {
    // A full-board tactical camera needs dark canopy mass at the perimeter,
    // not a handful of hero-sized low-poly trees.  These still stand higher
    // than towers, but preserve useful battle space and leave their detailed
    // authored branches as an edge treatment rather than the focal point.
    let scale = rng.range(0.92, 1.18) * scale_mul;
    let tint = rng.unit();
    let pick = rng.next_u32() % 10;
    let (name, model_scale) = match pick {
        // The dense rounded broadleaf sources carry the exterior silhouette
        // at the playable camera.  Keep the open-branch pine studies in the
        // asset roster for later biome use, but do not scatter their skeletal
        // close-up shape across a soft woodland meadow.
        0..=4 => ("NatureBroadleaf", scale * 1.68),
        _ => ("NatureOak", scale * 1.56),
    };
    if crate::view::models::draw_downloaded(
        d,
        name,
        [p[0], p[1], 0.04],
        model_scale,
        rng.range(0.0, std::f32::consts::TAU),
        rgba(
            // Source leaves retain their material/UV detail, but a bright
            // white instance tint was bleaching every canopy into lime map
            // icons. This keeps foliage in the dark organic hierarchy while
            // leaving lit leaf planes and cast shadows readable.
            [0.63 + tint * 0.050, 0.60 + tint * 0.045, 0.42 + tint * 0.030],
            1.0,
        ),
        Material::FOLIAGE,
        0.0,
    ) {
        return;
    }
    let trunk_h = 0.55 * scale;
    d.cylinder(
        [p[0], p[1], trunk_h * 0.5],
        0.17 * scale,
        trunk_h,
        0.0,
        rgba([0.185, 0.130, 0.085], 1.0),
        Material::WOOD,
    );

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
    if crate::view::models::draw_downloaded(
        d,
        if rng.chance(0.5) {
            "NatureRockA"
        } else {
            "NatureRockB"
        },
        [p[0], p[1], 0.035],
        rng.range(0.46, 0.70),
        rng.range(0.0, std::f32::consts::TAU),
        rgba([0.42, 0.47, 0.39], 1.0),
        Material::STONE,
        0.0,
    ) {
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Terrain richness is a live visual contract, not an assumption based on
    /// a source asset existing on disk.  Count the actual baked canopy buckets
    /// emitted for C1's static board so a clearance tweak cannot silently turn
    /// the field back into an empty carpet.
    #[test]
    fn c1_static_board_keeps_a_grounded_woodland_edge() {
        let board = Board::new();
        let decor = Decor::build(&board);
        let count = |name: &str| {
            crate::gfx::mesh::model_slots()
                .iter()
                .find(|(asset, _)| asset == name)
                .map(|(_, slot)| decor.statics.solid[crate::gfx::mesh::model_bucket(*slot)].len())
                .unwrap_or(0)
        };
        let canopy = count("NatureOak")
            + count("NatureBroadleaf")
            + count("NaturePineA")
            + count("NaturePineB");
        let grounded = count("NatureBush") + count("NatureFern") + count("NatureGrass");
        println!("C1_REAL_COVER: canopy={canopy} grounded={grounded}");
        // Tall trees are intentionally sparse: the tactical meadow should
        // read as real grass and route, not a toy orchard. Keep enough at the
        // exterior horizon for depth while requiring plentiful low, grounded
        // geometry around the actual combat field.
        assert!(canopy >= 10, "C1 lost its exterior horizon depth: {canopy}");
        assert!(grounded >= 45, "C1 lost its grounded low cover: {grounded}");
    }
}
