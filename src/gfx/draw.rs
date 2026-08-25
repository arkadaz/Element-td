//! The drawing vocabulary shared by the game and the GPU.
//!
//! Everything solid on the board is one instance of the same unit cube, lit per
//! face in the fragment shader. Glows and particles are camera-facing quads.
//! Adding a new prop means composing cubes - never a new pipeline or draw call.

use bytemuck::{Pod, Zeroable};

pub type Color = [f32; 4];

/// One cube (solid pass) or one billboard (glow pass).
///
/// For solids: `pos` is the centre, `scale` the full size, `rot` is (yaw, pitch),
/// `params.x` is how self-lit it is (0 = fully shaded, 1 = pure emissive).
/// For glows: `pos` is the centre, `scale.x` the radius, `params.x` the falloff.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
pub struct Instance {
    pub pos: [f32; 3],
    pub scale: [f32; 3],
    pub rot: [f32; 2],
    pub params: [f32; 2],
    pub color: Color,
}

#[derive(Default)]
pub struct DrawList {
    /// Depth-tested, lit cubes: terrain, pads, towers, monsters, shots.
    pub solid: Vec<Instance>,
    /// Additive camera-facing sprites: glows, muzzle flashes, auras.
    pub glow: Vec<Instance>,
}

impl DrawList {
    pub fn clear(&mut self) {
        self.solid.clear();
        self.glow.clear();
    }

    pub fn len(&self) -> usize {
        self.solid.len() + self.glow.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // -------------------------------------------------- solids

    /// A box centred on `pos`.
    pub fn cube(&mut self, pos: [f32; 3], scale: [f32; 3], yaw: f32, color: Color) {
        self.solid.push(Instance {
            pos,
            scale,
            rot: [yaw, 0.0],
            params: [0.0, 0.0],
            color,
        });
    }

    /// A box that glows from within (`em` 0..1).
    pub fn cube_lit(&mut self, pos: [f32; 3], scale: [f32; 3], yaw: f32, color: Color, em: f32) {
        self.solid.push(Instance {
            pos,
            scale,
            rot: [yaw, 0.0],
            params: [em, 0.0],
            color,
        });
    }

    /// A box standing on the ground at `p`, `scale.z` tall.
    pub fn prop(&mut self, p: [f32; 2], scale: [f32; 3], yaw: f32, color: Color) {
        self.cube([p[0], p[1], scale[2] * 0.5], scale, yaw, color);
    }

    /// A flat slab lying on the ground - terrain, road, pads.
    pub fn slab(&mut self, p: [f32; 2], size: [f32; 2], z: f32, thickness: f32, color: Color) {
        self.cube(
            [p[0], p[1], z - thickness * 0.5],
            [size[0], size[1], thickness],
            0.0,
            color,
        );
    }

    /// A box stretched between two points in 3D - beams, struts, lances.
    pub fn bar(&mut self, a: [f32; 3], b: [f32; 3], w: f32, color: Color, em: f32) {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let flat = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let len = (flat * flat + d[2] * d[2]).sqrt();
        if len < 1e-5 {
            return;
        }
        self.solid.push(Instance {
            pos: [
                (a[0] + b[0]) * 0.5,
                (a[1] + b[1]) * 0.5,
                (a[2] + b[2]) * 0.5,
            ],
            scale: [len, w, w],
            rot: [d[1].atan2(d[0]), d[2].atan2(flat)],
            params: [em, 0.0],
            color,
        });
    }

    // -------------------------------------------------- glows

    /// Soft additive sprite, always facing the camera.
    pub fn glow(&mut self, pos: [f32; 3], radius: f32, power: f32, color: Color) {
        self.glow.push(Instance {
            pos,
            scale: [radius, radius, radius],
            rot: [0.0, 0.0],
            params: [power, 0.0],
            color,
        });
    }

    /// Glow spread along a line, for beams.
    pub fn glow_line(&mut self, a: [f32; 3], b: [f32; 3], radius: f32, color: Color, steps: u32) {
        let n = steps.max(1);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.glow(
                [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    a[2] + (b[2] - a[2]) * t,
                ],
                radius,
                1.6,
                color,
            );
        }
    }

    /// A flat ring lying on the ground, drawn as short bars. Used for range
    /// indicators and shockwaves.
    pub fn ground_ring(&mut self, c: [f32; 2], r: f32, w: f32, color: Color, segments: u32) {
        let n = segments.max(8);
        let step = std::f32::consts::TAU / n as f32;
        let seg_len = (r * step) * 1.15;
        for i in 0..n {
            let a = i as f32 * step;
            let (s, co) = a.sin_cos();
            self.solid.push(Instance {
                // Above the plot surface, or the ring disappears under it.
                pos: [c[0] + co * r, c[1] + s * r, 0.17],
                scale: [seg_len, w, 0.04],
                rot: [a + std::f32::consts::FRAC_PI_2, 0.0],
                params: [0.85, 0.0],
                color,
            });
        }
    }
}

/// Multiply a colour's alpha.
#[inline]
pub fn fade(c: Color, a: f32) -> Color {
    [c[0], c[1], c[2], c[3] * a]
}

/// Build an RGBA colour from an RGB triple.
#[inline]
pub fn rgba(c: [f32; 3], a: f32) -> Color {
    [c[0], c[1], c[2], a]
}

/// Scale RGB (pushes a colour into HDR so it blooms).
#[inline]
pub fn boost(c: Color, k: f32) -> Color {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

/// Blend two colours.
#[inline]
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
