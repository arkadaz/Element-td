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
        if l < 1e-6 { v3(0.0, 0.0, 1.0) } else { self.mul(1.0 / l) }
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
            s.x, u.x, -f.x, 0.0, //
            s.y, u.y, -f.y, 0.0, //
            s.z, u.z, -f.z, 0.0, //
            -s.dot(eye), -u.dot(eye), f.dot(eye), 1.0,
        ])
    }

    /// Perspective with a 0..1 depth range (what wgpu wants).
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let t = 1.0 / (fov_y * 0.5).tan();
        let d = near - far;
        Mat4([
            t / aspect.max(0.001), 0.0, 0.0, 0.0, //
            0.0, t, 0.0, 0.0, //
            0.0, 0.0, far / d, -1.0, //
            0.0, 0.0, near * far / d, 0.0,
        ])
    }

    /// Orthographic projection with a 0..1 depth range.
    pub fn ortho(half_w: f32, half_h: f32, near: f32, far: f32) -> Mat4 {
        let d = far - near;
        Mat4([
            1.0 / half_w, 0.0, 0.0, 0.0, //
            0.0, 1.0 / half_h, 0.0, 0.0, //
            0.0, 0.0, -1.0 / d, 0.0, //
            0.0, 0.0, -near / d, 1.0,
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
        let fov_y = 42f32.to_radians();
        let aspect = aspect.clamp(0.20, 8.0);
        let target = v3(w * 0.5, h * 0.5, 0.0);

        let (sp, cp) = pitch.sin_cos();
        let (sy, cy) = yaw.sin_cos();
        // Unit vector from the target towards where the camera will sit.
        let dir = v3(-sy * cp, -cy * cp, sp);

        // Camera basis, which only depends on the direction, not the distance.
        let fwd = dir.mul(-1.0);
        let world_up = v3(0.0, 0.0, 1.0);
        let right = fwd.cross(world_up).norm();
        let up = right.cross(fwd).norm();

        let ty = (fov_y * 0.5).tan();
        let tx = ty * aspect;

        // Tall enough to keep a maxed tower and the gate arches in frame.
        const TOP: f32 = 3.6;
        let corners = [
            v3(0.0, 0.0, 0.0),
            v3(w, 0.0, 0.0),
            v3(0.0, h, 0.0),
            v3(w, h, 0.0),
            v3(0.0, 0.0, TOP),
            v3(w, 0.0, TOP),
            v3(0.0, h, TOP),
            v3(w, h, TOP),
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
        dist = (dist * 1.06 / zoom.clamp(0.2, 4.0)).max(2.0);

        let eye = target.add(dir.mul(dist));
        let view = Mat4::look_at(eye, target, world_up);
        let proj = Mat4::perspective(fov_y, aspect, (dist * 0.05).max(0.5), dist * 3.0 + 200.0);
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
        const W: f32 = 30.0;
        const H: f32 = 18.0;
        for &aspect in &[0.35, 0.55, 0.75, 1.0, 1.33, 1.78, 2.4, 3.2, 5.0] {
            for &pitch_deg in &[35.0f32, 52.0, 70.0] {
                for &yaw in &[-0.3f32, 0.0, 0.3] {
                    let cam =
                        Camera::frame_board(W, H, aspect, pitch_deg.to_radians(), yaw, 1.06);
                    for &(x, y, z) in &[
                        (0.0, 0.0, 0.0),
                        (W, 0.0, 0.0),
                        (0.0, H, 0.0),
                        (W, H, 0.0),
                        (W * 0.5, H * 0.5, 3.5),
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

    /// And it must not be framed so loosely that the board is a stamp in the
    /// middle of the screen - a fit that is always "safe" is a useless fit.
    #[test]
    fn the_board_actually_fills_the_frame() {
        for &aspect in &[0.6, 1.0, 1.78, 2.6] {
            let cam = Camera::frame_board(30.0, 18.0, aspect, 52f32.to_radians(), 0.0, 1.06);
            let mut extent = 0.0f32;
            for &(x, y) in &[(0.0, 0.0), (30.0, 0.0), (0.0, 18.0), (30.0, 18.0)] {
                let n = cam.view_proj.project(v3(x, y, 0.0)).expect("in front");
                extent = extent.max(n[0].abs()).max(n[1].abs());
            }
            assert!(extent > 0.72, "board only fills {extent:.2} of the frame at aspect {aspect}");
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
