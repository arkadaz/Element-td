//! The board: one fixed road the monsters walk, and the pads you may build on.
//!
//! There is no mazing - the route never changes - so a creep's whole position is
//! a single scalar: how far along the road it has travelled.

/// Board size in tiles.
pub const BW: f32 = 30.0;
pub const BH: f32 = 18.0;

/// Half-width of the road surface, in tiles.
pub const ROAD_HALF: f32 = 0.62;

/// Corner rounding radius.
const CORNER_R: f32 = 1.0;

/// A tile is buildable when its centre sits in this band beside the road:
/// far enough not to overlap the surface, near enough for a tower to reach it.
pub const BUILD_NEAR: f32 = 1.05;
pub const BUILD_FAR: f32 = 3.05;

/// The corners of the circuit, in order. It closes back onto the first, so this
/// is a ring rather than a route. Edit this to redraw the level.
///
/// A rounded rectangle rather than a true circle: the straights give towers
/// long clean firing lines and give the eye something to read, and the four
/// corners are where monsters bunch up and where the area towers want to stand.
const WAYPOINTS: [[f32; 2]; 8] = [
    [7.5, 3.5],
    [22.5, 3.5],
    [26.5, 6.5],
    [26.5, 11.5],
    [22.5, 14.5],
    [7.5, 14.5],
    [3.5, 11.5],
    [3.5, 6.5],
];

/// Where the monsters enter the circuit, as a distance along it. They keep
/// walking from there and never leave.
pub const SPAWN_DIST: f32 = 0.0;

#[derive(Clone, Copy)]
pub struct Slot {
    pub pos: [f32; 2],
    /// Index into `Game::towers`, if something is standing here.
    pub tower: Option<usize>,
}

pub struct Board {
    /// The road as a dense polyline (corners already rounded).
    pub path: Vec<[f32; 2]>,
    /// Distance along the road at each polyline point.
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
        let path = round_ring(&WAYPOINTS, CORNER_R);
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
            lookup: vec![-1; (BW * BH) as usize],
        };
        b.slots = b.make_slots();
        for (i, s) in b.slots.iter().enumerate() {
            let tx = s.pos[0].floor() as usize;
            let ty = s.pos[1].floor() as usize;
            b.lookup[ty * BW as usize + tx] = i as i32;
        }
        b
    }

    /// Build plots are a plain, regular grid: every tile that is clear of the road
    /// and close enough to it to be useful. One tile, one plot, no exceptions -
    /// so the buildable area reads as a field, not as scattered platforms.
    /// The build plots, on a checkerboard inside the band beside the road.
    ///
    /// Every tile in the band used to be a plot, which gave a hundred and
    /// ninety-eight of them - so many that *where* a tower went never mattered
    /// and filling the board with cheap towers beat levelling good ones. A
    /// campaign purse that can cover two hundred plots is a campaign with no
    /// placement decision in it. Half as many, on alternating tiles, makes each
    /// one worth thinking about and leaves levels as the real sink for gold.
    fn make_slots(&self) -> Vec<Slot> {
        let mut out = Vec::new();
        for ty in 0..BH as i32 {
            for tx in 0..BW as i32 {
                if (tx + ty) % 2 != 0 {
                    continue;
                }
                let p = [tx as f32 + 0.5, ty as f32 + 0.5];
                let d = self.dist_to_road(p);
                if (BUILD_NEAR..=BUILD_FAR).contains(&d) {
                    out.push(Slot {
                        pos: p,
                        tower: None,
                    });
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
        match self.lookup[ty * BW as usize + tx] {
            -1 => None,
            i => Some(i as usize),
        }
    }

    /// Where monsters appear on the circuit. There is no matching exit - see
    /// the module docs.
    pub fn start(&self) -> [f32; 2] {
        *self.path.first().unwrap_or(&[0.0, 0.0])
    }

    /// Position at `dist` tiles along the road.
    /// Wraps a distance into the circuit. Everything that reads a position goes
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

    /// Unit heading at `dist` tiles along the road.
    pub fn heading(&self, dist: f32) -> [f32; 2] {
        // No clamping: on a closed circuit the sample either side of a monster
        // standing on the seam has to come from the other end of the ring, or
        // everything crossing that point spins to face down the wrong axis.
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

    /// Shortest distance from a point to the road centre line.
    pub fn dist_to_road(&self, p: [f32; 2]) -> f32 {
        let mut best = f32::MAX;
        for w in self.path.windows(2) {
            best = best.min(point_seg_dist(p, w[0], w[1]));
        }
        best
    }

    /// Is this tile part of the road surface? Used only for rendering.
    pub fn is_road_tile(&self, tx: i32, ty: i32) -> bool {
        self.dist_to_road([tx as f32 + 0.5, ty as f32 + 0.5]) <= ROAD_HALF + 0.45
    }

    /// The build plot under a world position. Because plots are exactly the tile
    /// grid, this is a straight floor - the cursor always snaps cleanly.
    pub fn slot_at(&self, p: [f32; 2]) -> Option<usize> {
        self.tile_slot(p)
    }
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

/// Replaces each interior corner with a short arc so creeps bank through turns.
/// Rounds every corner of a **closed** polygon and returns the ring as a
/// polyline whose last point repeats its first.
///
/// The open version of this left the first and last vertices sharp, which is
/// right for a route with two ends and wrong for a ring: run over a rounded
/// rectangle it produced a circuit with a thirty-four tile chord across it,
/// and monsters walked that chord straight through the middle of the board.
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
