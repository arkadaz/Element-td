//! The board: Green Circle TD's own terrain, and the lane one player defends.
//!
//! The source map is a ninety-six by ninety-six field cut into eight identical
//! arenas. This port keeps its closed lane and tower rules but compacts one
//! arena into a purpose-built solo battlefield; carrying the other seven empty
//! sectors into a one-player game only added travel and scrolling.
//!
//! What one player defends is a lane. In the source trigger script Red enters
//! at the north-west junction, then a fifty-fifty order sends each creep either
//! clockwise or anticlockwise around the outer circuit. Both directions meet
//! back at that junction and continue circling. The compact board preserves
//! that trigger order and split instead of turning the reference route into a
//! one-way racetrack.

use super::greentd_map::{ARENA, LAP, MAP_H, MAP_W, TEXTURE};

/// Runtime board size in tiles. The extracted 97x97 texture remains available
/// as source data, but the renderer and simulation only carry the solo arena.
pub const BW: f32 = ARENA[2] + 1.0;
pub const BH: f32 = ARENA[3] + 1.0;

/// The ground texture index the map paints its corridors in.
#[allow(dead_code)]
pub const ROCK: u8 = 2;

/// Half-width of the compact lane surface, in tiles. The two directions of
/// travel share it with a small lateral offset.
pub const ROAD_HALF: f32 = 0.62;

/// Corner rounding radius. The map's corners are square; a short arc is what
/// stops a monster spinning on the spot as it turns.
const CORNER_R: f32 = 0.8;

/// Every offered pad belongs to the lane's tactical shoulder. The old square
/// lattice reached more than six tiles into empty grass: legal towers looked
/// disconnected from combat and several starter ranges barely touched the
/// road. These bounds are an explicit visual/gameplay contract.
pub const PAD_ROAD_MIN: f32 = 1.40;
pub const PAD_ROAD_MAX: f32 = 2.80;

/// A baked turret can reach about 1.55 tiles across at its apex. This leaves a
/// visible strip of ground between neighbours instead of merging them into a
/// single mechanical wall.
pub const PAD_SPACING: f32 = 2.20;

/// The solo board has enough positions for build variety but not enough for a
/// thoughtless tower carpet. Candidates are considered in route order and the
/// cap keeps both the tactical and performance budget stable.
pub const PAD_LIMIT: usize = 56;

/// Where the monsters enter the lane, as a distance along it. They keep walking
/// from there and never leave.
pub const SPAWN_DIST: f32 = 0.0;

#[derive(Clone, Copy)]
pub struct Slot {
    pub pos: [f32; 2],
    /// Index into `Game::towers`, if something is standing here.
    pub tower: Option<usize>,
}

pub struct Board {
    /// The lane as a dense polyline (corners already rounded).
    pub path: Vec<[f32; 2]>,
    /// Distance along the lane at each polyline point.
    pub cum: Vec<f32>,
    pub total: f32,
    pub slots: Vec<Slot>,
    /// tile index -> slot index, so picking is a constant-time lookup.
    lookup: Vec<i32>,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    pub fn new() -> Self {
        let path = round_ring(LAP, CORNER_R);
        let mut cum = Vec::with_capacity(path.len());
        let mut total = 0.0;
        for (i, p) in path.iter().enumerate() {
            if i > 0 {
                let q = path[i - 1];
                total += ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt();
            }
            cum.push(total);
        }
        let mut b = Self {
            path,
            cum,
            total,
            slots: Vec::new(),
            lookup: vec![-1; MAP_W * MAP_H],
        };
        b.slots = b.make_slots();
        for (i, s) in b.slots.iter().enumerate() {
            let tx = s.pos[0].floor() as usize;
            let ty = s.pos[1].floor() as usize;
            if tx < MAP_W && ty < MAP_H {
                b.lookup[ty * MAP_W + tx] = i as i32;
            }
        }
        b
    }

    /// Road-hugging build plots sampled from both sides of the lane.
    ///
    /// Free placement is retained from Green Circle TD, but the useless ground
    /// belonging to the other seven players is gone. The exact rounded route is
    /// checked as well as the tile mask so a tower can never clip a corner.
    fn make_slots(&self) -> Vec<Slot> {
        let mut out: Vec<Slot> = Vec::with_capacity(PAD_LIMIT);
        // Thirty paired samples offer sixty candidates before corner snapping
        // and separation. In practice this produces a stable fifty-something
        // pads on the compact reference circuit.
        const SAMPLES: usize = 30;
        const OFFSETS: [f32; 3] = [1.85, 2.25, 2.65];
        const INSET: f32 = 1.25;
        let min_d2 = PAD_SPACING * PAD_SPACING;

        for i in 0..SAMPLES {
            let along = (i as f32 + 0.5) / SAMPLES as f32 * self.total;
            let road = self.sample(along);
            let heading = self.heading(along);
            let normal = [-heading[1], heading[0]];
            // Alternate which shoulder is considered first so a capped list
            // never favours only the inside or outside of the ring.
            let first = if i % 2 == 0 { 1.0 } else { -1.0 };
            for side in [first, -first] {
                let mut accepted = None;
                for offset in OFFSETS {
                    let raw = [
                        road[0] + normal[0] * side * offset,
                        road[1] + normal[1] * side * offset,
                    ];
                    // Half-tile centres retain the crisp Warcraft placement
                    // language while the candidates themselves follow the
                    // curved route instead of an unrelated global grid.
                    let pos = [raw[0].floor() + 0.5, raw[1].floor() + 0.5];
                    let tx = pos[0].floor() as i32;
                    let ty = pos[1].floor() as i32;
                    let inside = pos[0] >= ARENA[0] + INSET
                        && pos[1] >= ARENA[1] + INSET
                        && pos[0] <= ARENA[2] - INSET
                        && pos[1] <= ARENA[3] - INSET;
                    let road_dist = self.dist_to_road(pos);
                    let separated = out.iter().all(|slot| {
                        (slot.pos[0] - pos[0]).powi(2) + (slot.pos[1] - pos[1]).powi(2) >= min_d2
                    });
                    if inside
                        && buildable_tile(tx, ty)
                        && (PAD_ROAD_MIN..=PAD_ROAD_MAX).contains(&road_dist)
                        && separated
                    {
                        accepted = Some(pos);
                        break;
                    }
                }
                if let Some(pos) = accepted {
                    out.push(Slot { pos, tower: None });
                    if out.len() == PAD_LIMIT {
                        return out;
                    }
                }
            }
        }
        out
    }

    /// Which build plot a world position falls on, if any.
    pub fn tile_slot(&self, p: [f32; 2]) -> Option<usize> {
        if p[0] < 0.0 || p[1] < 0.0 || p[0] >= BW || p[1] >= BH {
            return None;
        }
        let tx = p[0] as usize;
        let ty = p[1] as usize;
        match self.lookup[ty * MAP_W + tx] {
            -1 => None,
            i => Some(i as usize),
        }
    }

    /// Where monsters appear on the lane. There is no matching exit - see the
    /// module docs.
    pub fn start(&self) -> [f32; 2] {
        *self.path.first().unwrap_or(&[0.0, 0.0])
    }

    /// Wraps a distance into the lap. Everything that reads a position goes
    /// through here, so a monster on its fourth lap is handled by exactly the
    /// same code as one on its first.
    #[inline]
    pub fn wrap(&self, dist: f32) -> f32 {
        if self.total <= 0.0 {
            return 0.0;
        }
        dist.rem_euclid(self.total)
    }

    pub fn sample(&self, dist: f32) -> [f32; 2] {
        if self.path.is_empty() {
            return [0.0, 0.0];
        }
        let dist = self.wrap(dist);
        if dist <= 0.0 {
            return self.path[0];
        }
        let i = match self.cum.binary_search_by(|c| c.partial_cmp(&dist).unwrap()) {
            Ok(i) => i,
            Err(i) => i,
        }
        .clamp(1, self.path.len() - 1);
        let (a, b) = (self.path[i - 1], self.path[i]);
        let (ca, cb) = (self.cum[i - 1], self.cum[i]);
        let t = if cb > ca {
            (dist - ca) / (cb - ca)
        } else {
            0.0
        };
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }

    /// Position for a creep travelling in one of the source map's two route
    /// directions. `progress` always increases, so targeting and lap pressure
    /// remain comparable even though the physical route coordinate reverses.
    #[inline]
    pub fn sample_travel(&self, progress: f32, direction: f32) -> [f32; 2] {
        self.sample(progress * direction.signum())
    }

    /// Unit heading at `dist` tiles along the lane.
    pub fn heading(&self, dist: f32) -> [f32; 2] {
        // No clamping: on a closed lap the sample either side of a monster
        // standing on the seam has to come from the other end, or everything
        // crossing that point spins to face down the wrong axis.
        let a = self.sample(dist - 0.25);
        let b = self.sample(dist + 0.25);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dy * dy).sqrt();
        if l < 1e-5 {
            [1.0, 0.0]
        } else {
            [dx / l, dy / l]
        }
    }

    /// Unit heading in the direction this creep actually travels.
    #[inline]
    pub fn heading_travel(&self, progress: f32, direction: f32) -> [f32; 2] {
        let direction = direction.signum();
        let h = self.heading(progress * direction);
        [h[0] * direction, h[1] * direction]
    }

    /// Shortest distance from a point to the lane centre line.
    pub fn dist_to_road(&self, p: [f32; 2]) -> f32 {
        let mut best = f32::MAX;
        for w in self.path.windows(2) {
            best = best.min(point_seg_dist(p, w[0], w[1]));
        }
        best
    }

    /// The nearest build plot to a world position. Exact-tile lookup remains
    /// the fast path; the small capture radius makes building feel magnetic
    /// without ever selecting the next pad along the lane.
    pub fn slot_at(&self, p: [f32; 2]) -> Option<usize> {
        if let Some(exact) = self.tile_slot(p) {
            return Some(exact);
        }
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| {
                let d2 = (slot.pos[0] - p[0]).powi(2) + (slot.pos[1] - p[1]).powi(2);
                (d2 <= 1.10 * 1.10).then_some((i, d2))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
}

/// The map's own ground texture at a tile: 0 dirt, 1 grass, 2 corridor, 3 dark
/// grass. Out of bounds reads as corridor, so nothing is ever built off the
/// edge of the world.
#[allow(dead_code)]
#[inline]
pub fn texture(tx: i32, ty: i32) -> u8 {
    if tx < 0 || ty < 0 || tx >= MAP_W as i32 || ty >= MAP_H as i32 {
        return ROCK;
    }
    TEXTURE[ty as usize * MAP_W + tx as usize]
}

/// Whether this tile belongs to the compact runtime lane.
///
/// The old implementation queried the extracted 97x97 texture, which still
/// describes the eight-player source map. Runtime geometry is deliberately
/// smaller, so the tile mask must follow [`LAP`] instead.
#[inline]
pub fn is_corridor(tx: i32, ty: i32) -> bool {
    if tx < ARENA[0] as i32 || ty < ARENA[1] as i32 || tx > ARENA[2] as i32 || ty > ARENA[3] as i32
    {
        return true;
    }
    let p = [tx as f32 + 0.5, ty as f32 + 0.5];
    (0..LAP.len()).any(|i| point_seg_dist(p, LAP[i], LAP[(i + 1) % LAP.len()]) <= ROAD_HALF + 0.48)
}

/// Whether a tower may stand here. A tower occupies its own tile, so the only
/// tile it may not have is one the creeps walk down.
pub fn buildable_tile(tx: i32, ty: i32) -> bool {
    !is_corridor(tx, ty)
}

fn point_seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let abx = b[0] - a[0];
    let aby = b[1] - a[1];
    let len2 = abx * abx + aby * aby;
    if len2 < 1e-9 {
        return ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
    }
    let t = (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / len2).clamp(0.0, 1.0);
    let cx = a[0] + abx * t;
    let cy = a[1] + aby * t;
    ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt()
}

/// Rounds every corner of a **closed** polygon and returns the ring as a
/// polyline whose last point repeats its first.
///
/// The open version of this left the first and last vertices sharp, which is
/// right for a route with two ends and wrong for a ring: it produced a lap with
/// a long chord across it, and monsters walked that chord straight through the
/// middle of the board.
fn round_ring(pts: &[[f32; 2]], r: f32) -> Vec<[f32; 2]> {
    const ARC_STEPS: usize = 6;
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let mut out: Vec<[f32; 2]> = Vec::with_capacity(n * (ARC_STEPS + 2) + 1);
    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let cur = pts[i];
        let next = pts[(i + 1) % n];
        let d0 = norm(sub(prev, cur));
        let d1 = norm(sub(next, cur));
        // Never eat more than half of either leg, or adjacent corners overlap.
        let leg = r
            .min(len(sub(prev, cur)) * 0.45)
            .min(len(sub(next, cur)) * 0.45);
        let a = [cur[0] + d0[0] * leg, cur[1] + d0[1] * leg];
        let b = [cur[0] + d1[0] * leg, cur[1] + d1[1] * leg];
        out.push(a);
        // Quadratic bend through the corner.
        for st in 1..ARC_STEPS {
            let t = st as f32 / ARC_STEPS as f32;
            let it = 1.0 - t;
            out.push([
                it * it * a[0] + 2.0 * it * t * cur[0] + t * t * b[0],
                it * it * a[1] + 2.0 * it * t * cur[1] + t * t * b[1],
            ]);
        }
        out.push(b);
    }
    // Close it explicitly, so `total` counts the final segment and `sample`
    // interpolates across the seam like any other.
    out.push(out[0]);
    out
}

fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn len(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}
fn norm(v: [f32; 2]) -> [f32; 2] {
    let l = len(v);
    if l < 1e-6 {
        [0.0, 0.0]
    } else {
        [v[0] / l, v[1] / l]
    }
}
