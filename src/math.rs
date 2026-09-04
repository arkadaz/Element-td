//! Just enough linear algebra for a 3D camera.
//!
//! World axes: +X right across the board, +Y away from the camera, +Z up.

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub fn add(self, o: Vec3) -> Vec3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: Vec3) -> Vec3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn mul(self, k: f32) -> Vec3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> Vec3 {
        let l = self.len();
        if l < 1e-6 {
            v3(0.0, 0.0, 1.0)
        } else {
            self.mul(1.0 / l)
        }
    }
}

/// Column-major 4x4, laid out the way WGSL expects it.
#[derive(Clone, Copy, Debug)]
pub struct Mat4(pub [f32; 16]);

impl Default for Mat4 {
    fn default() -> Self {
        Mat4::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]);

    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let (a, b) = (&self.0, &o.0);
        let mut m = [0.0f32; 16];
        for c in 0..4 {
            for r in 0..4 {
                m[c * 4 + r] = a[r] * b[c * 4]
                    + a[4 + r] * b[c * 4 + 1]
                    + a[8 + r] * b[c * 4 + 2]
                    + a[12 + r] * b[c * 4 + 3];
            }
        }
        Mat4(m)
    }

    /// Right-handed look-at with +Z up.
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let f = target.sub(eye).norm();
        let s = f.cross(up).norm();
        let u = s.cross(f);
        Mat4([
            s.x,
            u.x,
            -f.x,
            0.0, //
            s.y,
            u.y,
            -f.y,
            0.0, //
            s.z,
            u.z,
            -f.z,
            0.0, //
            -s.dot(eye),
            -u.dot(eye),
            f.dot(eye),
            1.0,
        ])
    }

    /// Perspective with a 0..1 depth range (what wgpu wants).
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let t = 1.0 / (fov_y * 0.5).tan();
        let d = near - far;
        Mat4([
            t / aspect.max(0.001),
            0.0,
            0.0,
            0.0, //
            0.0,
            t,
            0.0,
            0.0, //
            0.0,
            0.0,
            far / d,
            -1.0, //
            0.0,
            0.0,
            near * far / d,
            0.0,
        ])
    }

    /// Orthographic projection with a 0..1 depth range.
    pub fn ortho(half_w: f32, half_h: f32, near: f32, far: f32) -> Mat4 {
        let d = far - near;
        Mat4([
            1.0 / half_w,
            0.0,
            0.0,
            0.0, //
            0.0,
            1.0 / half_h,
            0.0,
            0.0, //
            0.0,
            0.0,
            -1.0 / d,
            0.0, //
            0.0,
            0.0,
            -near / d,
            1.0,
        ])
    }

    /// Transforms a point and divides by w. Returns None behind the camera.
    pub fn project(&self, p: Vec3) -> Option<[f32; 3]> {
        let m = &self.0;
        let x = m[0] * p.x + m[4] * p.y + m[8] * p.z + m[12];
        let y = m[1] * p.x + m[5] * p.y + m[9] * p.z + m[13];
        let z = m[2] * p.x + m[6] * p.y + m[10] * p.z + m[14];
        let w = m[3] * p.x + m[7] * p.y + m[11] * p.z + m[15];
        if w <= 1e-5 {
            return None;
        }
        Some([x / w, y / w, z / w])
    }

    pub fn inverse(&self) -> Mat4 {
        let m = &self.0;
        let mut inv = [0.0f32; 16];

        inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
            + m[9] * m[7] * m[14]
            + m[13] * m[6] * m[11]
            - m[13] * m[7] * m[10];
        inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
            - m[8] * m[7] * m[14]
            - m[12] * m[6] * m[11]
            + m[12] * m[7] * m[10];
        inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
            + m[8] * m[7] * m[13]
            + m[12] * m[5] * m[11]
            - m[12] * m[7] * m[9];
        inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
            - m[8] * m[6] * m[13]
            - m[12] * m[5] * m[10]
            + m[12] * m[6] * m[9];
        inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
            - m[9] * m[3] * m[14]
            - m[13] * m[2] * m[11]
            + m[13] * m[3] * m[10];
        inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
            + m[8] * m[3] * m[14]
            + m[12] * m[2] * m[11]
            - m[12] * m[3] * m[10];
        inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
            - m[8] * m[3] * m[13]
            - m[12] * m[1] * m[11]
            + m[12] * m[3] * m[9];
        inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
            + m[8] * m[2] * m[13]
            + m[12] * m[1] * m[10]
            - m[12] * m[2] * m[9];
        inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
            + m[5] * m[3] * m[14]
            + m[13] * m[2] * m[7]
            - m[13] * m[3] * m[6];
        inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
            - m[4] * m[3] * m[14]
            - m[12] * m[2] * m[7]
            + m[12] * m[3] * m[6];
        inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
            + m[4] * m[3] * m[13]
            + m[12] * m[1] * m[7]
            - m[12] * m[3] * m[5];
        inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
            - m[4] * m[2] * m[13]
            - m[12] * m[1] * m[6]
            + m[12] * m[2] * m[5];
        inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
            - m[5] * m[3] * m[10]
            - m[9] * m[2] * m[7]
            + m[9] * m[3] * m[6];
        inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
            + m[4] * m[3] * m[10]
            + m[8] * m[2] * m[7]
            - m[8] * m[3] * m[6];
        inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
            - m[4] * m[3] * m[9]
            - m[8] * m[1] * m[7]
            + m[8] * m[3] * m[5];
        inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
            + m[4] * m[2] * m[9]
            + m[8] * m[1] * m[6]
            - m[8] * m[2] * m[5];

        let det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
        if det.abs() < 1e-12 {
            return Mat4::IDENTITY;
        }
        let k = 1.0 / det;
        for v in inv.iter_mut() {
            *v *= k;
        }
        Mat4(inv)
    }
}

/// Vertical field of view, in radians. Every camera the game builds shares it,
/// so a distance solved by one framing means the same thing to another.
const FOV_Y: f32 = 42.0 * std::f32::consts::PI / 180.0;
/// How much slack a fitted framing leaves around what it was asked to fit, so
/// nothing sits exactly on the edge of the screen.
const FIT_MARGIN: f32 = 1.06;

/// A tilted diorama camera looking down at the board.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    /// Camera-space axes in world space, for billboarding.
    pub right: Vec3,
    pub up: Vec3,
    pub view_proj: Mat4,
    pub inv_view_proj: Mat4,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: v3(0.0, 0.0, 1.0),
            target: Vec3::default(),
            right: v3(1.0, 0.0, 0.0),
            up: v3(0.0, 1.0, 0.0),
            view_proj: Mat4::IDENTITY,
            inv_view_proj: Mat4::IDENTITY,
        }
    }
}

impl Camera {
    /// Frames a `w` x `h` board, tilted by `pitch` radians, from `yaw`.
    ///
    /// The distance is solved exactly rather than searched for. The old version
    /// projected the corners and stepped the camera in until they fitted, but a
    /// corner that falls behind the near plane cannot be projected at all - it
    /// was silently skipped, the remaining corners fitted trivially, and the
    /// loop happily walked the camera *inwards* until the board filled the
    /// screen from one corner. At some aspect ratios that made the game look
    /// half-rendered.
    ///
    /// Instead: put each corner in view space, where a point is visible when
    /// `|x| <= (z + d) * tan(fovx/2)` and `|y| <= (z + d) * tan(fovy/2)`. Since
    /// x, y and z are measured from the target they do not depend on `d`, so
    /// each corner gives a lower bound on `d` directly and the answer is the
    /// largest of them. One pass, no iteration, correct at every aspect.
    pub fn frame_board(w: f32, h: f32, aspect: f32, pitch: f32, yaw: f32, zoom: f32) -> Self {
        Self::frame_rect([0.0, 0.0, w, h], aspect, pitch, yaw, zoom)
    }

    /// Frames an arbitrary rectangle of the board: min x, min y, max x, max y.
    ///
    /// The map is ninety-six tiles square and one player defends a corner of
    /// it, so the camera frames that corner rather than the whole field - the
    /// other seven arenas are scenery, and framing them would put this
    /// player's lane in a twelfth of the screen.
    pub fn frame_rect(r: [f32; 4], aspect: f32, pitch: f32, yaw: f32, zoom: f32) -> Self {
        let (x0, y0, x1, y1) = (r[0], r[1], r[2], r[3]);
        let aspect = aspect.clamp(0.20, 8.0);
        let target = v3((x0 + x1) * 0.5, (y0 + y1) * 0.5, 0.0);

        let (sp, cp) = pitch.sin_cos();
        let (sy, cy) = yaw.sin_cos();
        // Unit vector from the target towards where the camera will sit.
        let dir = v3(-sy * cp, -cy * cp, sp);

        // Camera basis, which only depends on the direction, not the distance.
        let fwd = dir.mul(-1.0);
        let world_up = v3(0.0, 0.0, 1.0);
        let right = fwd.cross(world_up).norm();
        let up = right.cross(fwd).norm();

        let ty = (FOV_Y * 0.5).tan();
        let tx = ty * aspect;

        // Tall enough to keep a maxed tower and the gate arches in frame.
        const TOP: f32 = 3.6;
        let corners = [
            v3(x0, y0, 0.0),
            v3(x1, y0, 0.0),
            v3(x0, y1, 0.0),
            v3(x1, y1, 0.0),
            v3(x0, y0, TOP),
            v3(x1, y0, TOP),
            v3(x0, y1, TOP),
            v3(x1, y1, TOP),
        ];
        let mut dist = 1.0f32;
        for c in corners {
            let rel = c.sub(target);
            let x = rel.dot(right).abs();
            let y = rel.dot(up).abs();
            let z = rel.dot(fwd);
            dist = dist.max(x / tx - z).max(y / ty - z);
        }
        // Margin so nothing touches the edge, then the requested zoom.
        dist = (dist * FIT_MARGIN / zoom.clamp(0.2, 4.0)).max(2.0);
        Self::at(target, dist, aspect, pitch, yaw)
    }

    /// Puts the camera `dist` tiles from `target`, tilted `pitch` radians down
    /// from the horizon and turned to `yaw`.
    ///
    /// Both framings end here. One solves for a distance that fits a rectangle
    /// on screen; the pannable one is told the distance outright, because what
    /// it fits is a window the player chose rather than the whole board.
    pub fn at(target: Vec3, dist: f32, aspect: f32, pitch: f32, yaw: f32) -> Self {
        let aspect = aspect.clamp(0.20, 8.0);
        let dist = dist.max(0.01);
        let (sp, cp) = pitch.sin_cos();
        let (sy, cy) = yaw.sin_cos();
        let world_up = v3(0.0, 0.0, 1.0);
        let eye = target.add(v3(-sy * cp, -cy * cp, sp).mul(dist));
        let view = Mat4::look_at(eye, target, world_up);
        let proj = Mat4::perspective(FOV_Y, aspect, (dist * 0.05).max(0.5), dist * 3.0 + 200.0);
        let view_proj = proj.mul(&view);
        // Rows 0 and 1 of the view matrix are the camera's right and up axes.
        let m = &view.0;
        Self {
            eye,
            target,
            right: v3(m[0], m[4], m[8]),
            up: v3(m[1], m[5], m[9]),
            inv_view_proj: view_proj.inverse(),
            view_proj,
        }
    }

    /// Screen point (0..1 across the viewport) -> where it lands on the ground.
    pub fn ground_pick(&self, u: f32, v: f32) -> Option<[f32; 2]> {
        let ndc_x = u * 2.0 - 1.0;
        let ndc_y = 1.0 - v * 2.0;
        let near = unproject(&self.inv_view_proj, ndc_x, ndc_y, 0.0)?;
        let far = unproject(&self.inv_view_proj, ndc_x, ndc_y, 1.0)?;
        let dir = far.sub(near);
        if dir.z.abs() < 1e-6 {
            return None;
        }
        let t = -near.z / dir.z;
        if t < 0.0 {
            return None;
        }
        let hit = near.add(dir.mul(t));
        Some([hit.x, hit.y])
    }

    /// World point -> viewport fraction (0..1). None if behind the camera.
    pub fn to_screen(&self, p: Vec3) -> Option<[f32; 2]> {
        let c = self.view_proj.project(p)?;
        Some([c[0] * 0.5 + 0.5, 0.5 - c[1] * 0.5])
    }
}

/// The fixed half of the camera: how far it tilts, which way it faces, and the
/// shape of the viewport it draws into.
///
/// A tilted camera does not see a rectangle of ground. It sees a trapezium,
/// narrow along the bottom of the screen and wide along the top, and that
/// footprint scales exactly with how far back the camera sits, because it is
/// the frustum cut by the plane the camera aims at. Measuring the footprint
/// once, at unit distance, therefore answers both of the questions a scrolling
/// camera has to ask: how far back to sit to take in a wanted span of board,
/// and how far the player may scroll before the footprint slides off their own
/// arena.
///
/// The measurements assume the board runs square to the screen, which is the
/// quarter turn the game frames the lane with; a rig turned to some other yaw
/// still produces the right camera, but its footprint would need the corners
/// of a rotated rectangle rather than its sides.
#[derive(Clone, Copy, Debug)]
pub struct Rig {
    aspect: f32,
    pitch: f32,
    yaw: f32,
    /// Ground on screen at unit distance, relative to the point the camera
    /// aims at: min x, min y, max x, max y.
    foot: [f32; 4],
    /// Ground *guaranteed* on screen at unit distance, whatever else is in
    /// frame. The trapezium is narrowest along its near edge, so this is that
    /// width carried the whole depth of the footprint.
    inner: [f32; 4],
    /// Ground crossed per viewport width and per viewport height at unit
    /// distance, so a drag can put the patch of board it grabbed back under
    /// the cursor.
    grad: [[f32; 2]; 2],
}

impl Rig {
    /// Measures the footprint once, by casting the corners of the viewport at
    /// the ground, so that everything asked of the rig afterwards is
    /// arithmetic on the answer.
    ///
    /// The pitch is held steep enough to keep the horizon off the top of the
    /// screen: a camera that can see the horizon sees infinitely far, its
    /// footprint has no far edge, and there is nothing finite left to clamp a
    /// pan against.
    pub fn new(aspect: f32, pitch: f32, yaw: f32) -> Self {
        let aspect = aspect.clamp(0.20, 8.0);
        let pitch = pitch.clamp(FOV_Y * 0.5 + 0.12, 1.55);
        let unit = Camera::at(Vec3::default(), 1.0, aspect, pitch, yaw);
        let ground = |u: f32, v: f32| {
            unit.ground_pick(u, v).expect(
                "the horizon is above the viewport, so every ray through it meets the ground",
            )
        };

        let bbox = |pts: &[[f32; 2]]| {
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for p in pts {
                b[0] = b[0].min(p[0]);
                b[1] = b[1].min(p[1]);
                b[2] = b[2].max(p[0]);
                b[3] = b[3].max(p[1]);
            }
            b
        };
        let (near_l, near_r) = (ground(0.0, 1.0), ground(1.0, 1.0));
        let foot = bbox(&[near_l, near_r, ground(0.0, 0.0), ground(1.0, 0.0)]);
        // As wide as the near edge and as deep as the footprint: the far edge
        // only ever widens the trapezium, so the middle of it is enough to
        // carry the guaranteed rectangle the whole way back.
        let inner = bbox(&[near_l, near_r, ground(0.5, 0.0)]);

        // Measured across the middle quarter of the screen rather than corner
        // to corner: a drag then tracks the ground exactly where the cursor
        // usually is, and only drifts near the top of the screen where
        // perspective is stretching the ground away anyway.
        let mid = ground(0.5, 0.5);
        let across = ground(0.75, 0.5);
        let down = ground(0.5, 0.75);
        let grad = [
            [(across[0] - mid[0]) * 4.0, (across[1] - mid[1]) * 4.0],
            [(down[0] - mid[0]) * 4.0, (down[1] - mid[1]) * 4.0],
        ];

        Self {
            aspect,
            pitch,
            yaw,
            foot,
            inner,
            grad,
        }
    }

    /// The longer side of the footprint at unit distance, which is what `span`
    /// is measured against.
    fn reach(&self) -> f32 {
        (self.foot[2] - self.foot[0])
            .max(self.foot[3] - self.foot[1])
            .max(1e-3)
    }

    /// How far back the camera sits to take in `span` tiles of board.
    fn distance(&self, span: f32) -> f32 {
        span.max(1.0) / self.reach()
    }

    /// The camera for a view of `span` tiles centred on `centre`.
    pub fn camera(&self, centre: [f32; 2], span: f32) -> Camera {
        self.camera_at(centre, 0.0, span)
    }

    /// The same, aimed `lift` above the ground plane.
    ///
    /// Play always aims at the ground, because the ground is what the player is
    /// choosing tiles on. A shallow camera cannot: at twenty degrees a figure
    /// standing on the target rises most of the way up the frame, so a model
    /// sheet aimed at z = 0 photographs the grass with everything's feet along
    /// the top edge.
    pub fn camera_at(&self, centre: [f32; 2], lift: f32, span: f32) -> Camera {
        let target = v3(centre[0], centre[1], lift);
        Camera::at(
            target,
            self.distance(span),
            self.aspect,
            self.pitch,
            self.yaw,
        )
    }

    /// The span at which all of `bounds` is in frame, which is as far out as
    /// zooming is worth allowing.
    ///
    /// It is deliberately the framing the game used before it could scroll at
    /// all: every corner of the arena on screen at once, and a tower standing
    /// on the far corner still under the top edge.
    pub fn widest_span(&self, bounds: [f32; 4]) -> f32 {
        // Handing the margin back as the zoom cancels it, so the arena ends up
        // touching the edges of the screen: anything further out is the border
        // and the seven arenas beyond it, which is not a view worth offering.
        let fitted = Camera::frame_rect(bounds, self.aspect, self.pitch, self.yaw, FIT_MARGIN);
        fitted.eye.sub(fitted.target).len() * self.reach()
    }

    /// Holds the zoom between the tightest the player asked for and the widest
    /// that shows anything new.
    pub fn clamp_span(&self, span: f32, tightest: f32, bounds: [f32; 4]) -> f32 {
        let widest = self.widest_span(bounds).max(tightest);
        span.clamp(tightest, widest)
    }

    /// Where the pan centre may sit: min x, min y, max x, max y.
    ///
    /// Two rules meet here and they pull against each other. The footprint
    /// should stay inside `bounds`, because this player defends one corner of a
    /// ninety-seven tile map and the other seven arenas are not theirs to
    /// scroll around. But every build pad inside `reach` also has to be
    /// *lookable at*: hold the first rule strictly and the pads in the corners
    /// of the arena can never be brought on screen, so they cannot be clicked
    /// and nothing on screen says they are there - which is the defect the pad
    /// test in this module was written to catch in the first place.
    ///
    /// So the view is allowed to overhang the arena, but by no more than
    /// reaching the outermost pads takes: three or four tiles at the zoom the
    /// game plays at, a few more when it is most of the way out, and none at
    /// all once the whole arena is in frame. Nothing it lets the player reach
    /// is ground the fully zoomed-out shot was not already showing them, which
    /// is what a check in this module's tests holds it to.
    pub fn pan_range(&self, span: f32, bounds: [f32; 4], reach: [f32; 4]) -> [f32; 4] {
        let d = self.distance(span);
        let axis = |lo: f32, hi: f32, want: (f32, f32), foot: (f32, f32), inner: (f32, f32)| {
            // Where the arena alone would let the centre sit.
            let (mut blo, mut bhi) = (lo - foot.0 * d, hi - foot.1 * d);
            if blo > bhi {
                // Zoomed out far enough that one view already holds this axis
                // of the arena, so there is nothing left to scroll towards:
                // centre it. Collapsing *here*, before the reach below, is what
                // stops the allowance growing without limit as the camera pulls
                // back - it used to be applied to the combined range, so at full
                // zoom-out the player could scroll a third of a screen onto
                // ground with neither arena nor pads on it.
                let mid = (blo + bhi) * 0.5;
                blo = mid;
                bhi = mid;
            }
            // Then widened, if that is what it takes to bring the outermost
            // pads on screen. When the arena already fits, it is not.
            (blo.min(want.0 - inner.0 * d), bhi.max(want.1 - inner.1 * d))
        };
        let (x0, x1) = axis(
            bounds[0],
            bounds[2],
            (reach[0], reach[2]),
            (self.foot[0], self.foot[2]),
            (self.inner[0], self.inner[2]),
        );
        let (y0, y1) = axis(
            bounds[1],
            bounds[3],
            (reach[1], reach[3]),
            (self.foot[1], self.foot[3]),
            (self.inner[1], self.inner[3]),
        );
        [x0, y0, x1, y1]
    }

    /// Pulls a pan centre back into [`Rig::pan_range`].
    pub fn clamp_pan(
        &self,
        centre: [f32; 2],
        span: f32,
        bounds: [f32; 4],
        reach: [f32; 4],
    ) -> [f32; 2] {
        let r = self.pan_range(span, bounds, reach);
        [centre[0].clamp(r[0], r[2]), centre[1].clamp(r[1], r[3])]
    }

    /// How far the pan centre has to move for the ground under a cursor
    /// dragged `du` across the viewport and `dv` down it to stay under it.
    ///
    /// Both are fractions of the viewport, not pixels, so the same call serves
    /// a drag, a key held down and the cursor sitting on the screen edge.
    pub fn drag(&self, span: f32, du: f32, dv: f32) -> [f32; 2] {
        let d = self.distance(span);
        [
            -(self.grad[0][0] * du + self.grad[1][0] * dv) * d,
            -(self.grad[0][1] * du + self.grad[1][1] * dv) * d,
        ]
    }
}

fn unproject(inv: &Mat4, x: f32, y: f32, z: f32) -> Option<Vec3> {
    let m = &inv.0;
    let px = m[0] * x + m[4] * y + m[8] * z + m[12];
    let py = m[1] * x + m[5] * y + m[9] * z + m[13];
    let pz = m[2] * x + m[6] * y + m[10] * z + m[14];
    let pw = m[3] * x + m[7] * y + m[11] * z + m[15];
    if pw.abs() < 1e-9 {
        return None;
    }
    Some(v3(px / pw, py / pw, pz / pw))
}

/// Light's-eye view-projection used for the shadow pass.
pub fn shadow_view_proj(board_w: f32, board_h: f32, dir: [f32; 3]) -> Mat4 {
    let centre = v3(board_w * 0.5, board_h * 0.5, 0.6);
    let d = v3(dir[0], dir[1], dir[2]).norm();
    // Far enough back that nothing on the board is clipped out of the map.
    let dist = 46.0;
    let eye = centre.add(d.mul(dist));
    let half = 0.5 * (board_w * board_w + board_h * board_h).sqrt() + 2.5;
    let view = Mat4::look_at(eye, centre, v3(0.0, 0.0, 1.0));
    let proj = Mat4::ortho(half, half, 1.0, dist * 2.2);
    proj.mul(&view)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole board must be inside the frustum at every window shape the
    /// game can be given. The old fitter walked the camera inwards whenever a
    /// corner fell behind the near plane, which showed up in game as a
    /// magnified corner of the map and looked like a broken renderer.
    #[test]
    fn the_board_is_framed_at_every_aspect() {
        let a = crate::game::greentd_map::VIEW;
        for &aspect in &[0.35, 0.55, 0.75, 1.0, 1.33, 1.78, 2.4, 3.2, 5.0] {
            for &pitch_deg in &[35.0f32, 52.0, 70.0] {
                for &yaw in &[-0.3f32, 0.0, 0.3] {
                    let cam = Camera::frame_rect(a, aspect, pitch_deg.to_radians(), yaw, 1.06);
                    for &(x, y, z) in &[
                        (a[0], a[1], 0.0),
                        (a[2], a[1], 0.0),
                        (a[0], a[3], 0.0),
                        (a[2], a[3], 0.0),
                        ((a[0] + a[2]) * 0.5, (a[1] + a[3]) * 0.5, 3.5),
                    ] {
                        let n = cam
                            .view_proj
                            .project(v3(x, y, z))
                            .unwrap_or_else(|| panic!("corner ({x},{y},{z}) is behind the camera at aspect {aspect}, pitch {pitch_deg}"));
                        // The default zoom deliberately cancels the fit margin,
                        // so corners land exactly on the edge; allow for float
                        // rounding, not for actually being outside.
                        assert!(
                            n[0].abs() <= 1.002 && n[1].abs() <= 1.002,
                            "corner ({x},{y},{z}) falls outside the view at aspect {aspect},                              pitch {pitch_deg}, yaw {yaw}: ndc {n:?}"
                        );
                        assert!(
                            (0.0..=1.0).contains(&n[2]),
                            "corner ({x},{y},{z}) is outside the depth range: {}",
                            n[2]
                        );
                    }
                }
            }
        }
    }

    /// Every build pad must be reachable, and scrolling must never take the
    /// player anywhere the wide shot does not already show them.
    ///
    /// This used to assert that every pad was on screen *at once*, which was
    /// the right test while the camera framed the whole arena from one fixed
    /// position. It cannot be right now that the camera shows about
    /// twenty-six tiles and the player scrolls: being on screen is no longer a
    /// property of a pad at all. What has to survive is the reason the old
    /// assertion existed - a pad the player cannot get to is a plot they cannot
    /// build on, and nothing on screen says it is there - so the same claim is
    /// made about the camera they can move: for every pad there is a legal pan
    /// that puts it in frame, at every zoom and every window shape.
    ///
    /// The second half is the constraint panning brought with it. Scrolling
    /// must stay in this player's own arena, so nothing a legal pan can reach
    /// may show ground that the fully zoomed-out framing - the fixed camera the
    /// game used to have - was not already showing. The two tiles of slack are
    /// for the corner pads: the picture is at its narrowest along the bottom
    /// edge, and reaching the pads in the corners of the arena means letting
    /// the view hang a little way over its border.
    ///
    /// The pad is checked on the ground rather than at tower height. A tower
    /// standing at the very top of the screen has its head cropped, in this
    /// game and in the one it copies; what must never happen is the *plot*
    /// being unclickable.
    #[test]
    fn every_build_pad_is_reachable_and_the_view_stays_in_the_arena() {
        use crate::game::board::Board;
        let board = Board::new();
        assert!(!board.slots.is_empty());
        let view = crate::game::greentd_map::VIEW;
        let pads = crate::pad_bounds(&board);
        let middle = [(view[0] + view[2]) * 0.5, (view[1] + view[3]) * 0.5];
        const SLACK: f32 = 2.0;
        // The ground in shot, read off the camera the player is actually given
        // rather than out of the rig's own workings.
        let on_screen = |cam: &Camera| {
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let g = cam
                    .ground_pick(u, v)
                    .expect("the viewport meets the ground");
                b = [
                    b[0].min(g[0]),
                    b[1].min(g[1]),
                    b[2].max(g[0]),
                    b[3].max(g[1]),
                ];
            }
            b
        };

        for &aspect in &[0.6, 1.0, 1.4, 1.78, 2.6] {
            let rig = Rig::new(
                aspect,
                crate::CAM_PITCH_DEG.to_radians(),
                crate::CAM_YAW_DEG.to_radians(),
            );
            let widest = rig.widest_span(view);
            // What the player is looking at with the wheel rolled all the way
            // back: the whole arena, and the border it sits in.
            let outer = on_screen(&rig.camera(rig.clamp_pan(middle, widest, view, pads), widest));
            for step in 0..=12 {
                let span =
                    crate::CAM_SPAN_MIN + (widest - crate::CAM_SPAN_MIN) * step as f32 / 12.0;
                let span = rig.clamp_span(span, crate::CAM_SPAN_MIN, view);

                for s in &board.slots {
                    // Scrolling towards a pad and letting the clamp have its
                    // say is exactly what a player does to reach one.
                    let cam = rig.camera(rig.clamp_pan(s.pos, span, view, pads), span);
                    let n = cam
                        .view_proj
                        .project(v3(s.pos[0], s.pos[1], 0.0))
                        .expect("pad is behind the camera");
                    assert!(
                        n[0].abs() <= 1.0 && n[1].abs() <= 1.0,
                        "pad at {:?} cannot be scrolled to at aspect {aspect}, span {span:.1}: \
                         ndc {:.2},{:.2}",
                        s.pos,
                        n[0],
                        n[1]
                    );
                }

                // Scrolling may overhang the arena - `Rig::pan_range` says why,
                // and the pad check above is why it has to. What it may not do
                // is wander: everything the player can bring on screen must be
                // arena, or within a short reach of the outermost pad.
                //
                // Stated that way rather than as "inside the fully zoomed-out
                // view", which is not the same thing and stopped being true
                // when the arena became the map's real ring. A tall narrow
                // window cannot hold a square arena on both axes at once, so
                // the widest view is *smaller* than the arena in one of them,
                // and comparing against it fails on ground that was never out
                // of bounds.
                // The allowance is a share of what is on screen rather than a
                // fixed number of tiles, because the overhang `pan_range` grants
                // is proportional to the camera's distance and so grows with the
                // zoom. A tenth of the span, plus a few tiles at the near end.
                // Panning may overhang the arena - `Rig::pan_range` says why,
                // and the pad check above is why it has to. What it may not do
                // is wander.
                //
                // Stated as a bound on where the camera *looks*, not on how
                // much ground it takes in. Two earlier versions bounded the
                // footprint - first against the fully zoomed-out view, then
                // against a fraction of the span - and neither converges: the
                // ground a fifty-two degree camera covers grows faster than the
                // distance does, so every bound that held at one zoom failed at
                // the next one out. The look-at point does not have that
                // problem, and it is the thing the clamp actually controls.
                const STRAY: f32 = 12.0;
                let bound = [
                    view[0].min(pads[0]) - STRAY,
                    view[1].min(pads[1]) - STRAY,
                    view[2].max(pads[2]) + STRAY,
                    view[3].max(pads[3]) + STRAY,
                ];
                let r = rig.pan_range(span, view, pads);
                for c in [[r[0], r[1]], [r[2], r[1]], [r[0], r[3]], [r[2], r[3]]] {
                    assert!(
                        c[0] >= bound[0]
                            && c[1] >= bound[1]
                            && c[0] <= bound[2]
                            && c[1] <= bound[3],
                        "at aspect {aspect}, span {span:.1} the camera can be aimed at {c:?}, \
                         which is outside the arena and its pads {bound:?}"
                    );
                }
            }
        }
    }

    /// And it must not be framed so loosely that the arena is a stamp in the
    /// middle of the screen - a fit that is always "safe" is a useless fit.
    ///
    /// The `#[test]` this needs had drifted up onto the pad test above, so the
    /// assertion was not running at all and the compiler reported the whole
    /// function as dead code. Restored here, and pointed at the arena the
    /// camera is really asked to frame rather than a board shape the game no
    /// longer has.
    #[test]
    fn the_board_actually_fills_the_frame() {
        let a = crate::game::greentd_map::VIEW;
        for &aspect in &[0.6, 1.0, 1.78, 2.6] {
            let cam = Camera::frame_rect(
                a,
                aspect,
                crate::CAM_PITCH_DEG.to_radians(),
                crate::CAM_YAW_DEG.to_radians(),
                1.06,
            );
            let mut extent = 0.0f32;
            for &(x, y) in &[(a[0], a[1]), (a[2], a[1]), (a[0], a[3]), (a[2], a[3])] {
                let n = cam.view_proj.project(v3(x, y, 0.0)).expect("in front");
                extent = extent.max(n[0].abs()).max(n[1].abs());
            }
            assert!(
                extent > 0.72,
                "board only fills {extent:.2} of the frame at aspect {aspect}"
            );
        }
    }

    /// Picking and rendering share one camera, so a click has to land on the
    /// tile it looks like it lands on.
    #[test]
    fn picking_agrees_with_projection() {
        let cam = Camera::frame_board(30.0, 18.0, 1.6, 52f32.to_radians(), 0.0, 1.06);
        for &(x, y) in &[(2.5f32, 3.5f32), (15.0, 9.0), (27.5, 15.5)] {
            let n = cam.view_proj.project(v3(x, y, 0.0)).expect("on screen");
            let u = n[0] * 0.5 + 0.5;
            let v = 0.5 - n[1] * 0.5;
            let back = cam.ground_pick(u, v).expect("ray hits the ground");
            assert!(
                (back[0] - x).abs() < 0.02 && (back[1] - y).abs() < 0.02,
                "picked {back:?} for {:?}",
                (x, y)
            );
        }
    }
}
