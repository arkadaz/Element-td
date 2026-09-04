//! The board: Green Circle TD's own terrain, and the lane one player defends.
//!
//! The map is a ninety-six by ninety-six field cut into eight identical arenas
//! by three-tile corridors, with a spawn box in every corner and on every edge.
//! All of that comes out of `war3map.w3e` and lives in [`super::greentd_map`];
//! nothing about the shape is invented here.
//!
//! What one player defends is a lane: the map orders their creeps along four
//! regions and then shuttles them between the last two forever, so the lane is
//! a corridor walked down and back. Written as a closed loop - down one half of
//! the corridor and up the other, which is how two streams pass each other in a
//! three-tile passage - it means a creep's whole position is a single scalar:
//! how far along the lane it has travelled. There is no exit and no mazing.

use super::greentd_map::{ARENA, LAP, MAP_H, MAP_W, TEXTURE};

/// Board size in tiles. The whole map is drawn; only [`ARENA`] is played.
pub const BW: f32 = MAP_W as f32;
pub const BH: f32 = MAP_H as f32;

/// The ground texture index the map paints its corridors in.
pub const ROCK: u8 = 2;

/// Half-width of the lane surface, in tiles. The corridor itself is three tiles
/// across and the two directions of travel share it.
pub const ROAD_HALF: f32 = 0.62;

/// Corner rounding radius. The map's corners are square; a short arc is what
/// stops a monster spinning on the spot as it turns.
const CORNER_R: f32 = 0.8;

/// How close to the lane a plot may be. A tower stands on its own tile and the
/// corridor is the tile beside it, which is exactly how Warcraft III places
/// them; nothing further out is excluded, because in Green Circle TD the whole
/// field is yours to build on.
#[allow(dead_code)]
pub const BUILD_NEAR: f32 = 0.9;

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

    /// The build plots: **every** tile of the player's arena that is not a
    /// corridor.
    ///
    /// That is Green Circle TD's own rule and it is not a small one. Warcraft
    /// III lets you put a tower on any buildable ground you own, so the whole
    /// field between the corridors is yours - which is what makes the game a
    /// question of *where* as well as *what*, and what makes an eight hundred
    /// plot arena feel like a field rather than a row of sockets.
    ///
    /// The two rules that remain are both the map's. A plot must be inside the
    /// player's arena, because the rest of the field belongs to the other seven
    /// players. And it must not be corridor, because that is where the creeps
    /// walk and nothing may be built in their way.
    fn make_slots(&self) -> Vec<Slot> {
        let mut out = Vec::new();
        // Inset from the frame, so a tower on the outermost plot is still
        // fully on screen at every window shape rather than half over the edge.
        const INSET: f32 = 2.0;
        let (x0, y0, x1, y1) = (
            ARENA[0] + INSET,
            ARENA[1] + INSET,
            ARENA[2] - INSET,
            ARENA[3] - INSET,
        );
        for ty in y0 as i32..=y1 as i32 {
            for tx in x0 as i32..=x1 as i32 {
                if !buildable_tile(tx, ty) {
                    continue;
                }
                out.push(Slot {
                    pos: [tx as f32 + 0.5, ty as f32 + 0.5],
                    tower: None,
                });
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

    /// Shortest distance from a point to the lane centre line.
    pub fn dist_to_road(&self, p: [f32; 2]) -> f32 {
        let mut best = f32::MAX;
        for w in self.path.windows(2) {
            best = best.min(point_seg_dist(p, w[0], w[1]));
        }
        best
    }

    /// The build plot under a world position. Because plots are exactly the
    /// tile grid, this is a straight floor - the cursor always snaps cleanly.
    pub fn slot_at(&self, p: [f32; 2]) -> Option<usize> {
        self.tile_slot(p)
    }
}

/// The map's own ground texture at a tile: 0 dirt, 1 grass, 2 corridor, 3 dark
/// grass. Out of bounds reads as corridor, so nothing is ever built off the
/// edge of the world.
#[inline]
pub fn texture(tx: i32, ty: i32) -> u8 {
    if tx < 0 || ty < 0 || tx >= MAP_W as i32 || ty >= MAP_H as i32 {
        return ROCK;
    }
    TEXTURE[ty as usize * MAP_W + tx as usize]
}

/// Whether this tile is one of the map's corridors.
#[inline]
pub fn is_corridor(tx: i32, ty: i32) -> bool {
    texture(tx, ty) == ROCK
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
