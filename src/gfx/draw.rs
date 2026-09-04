//! The drawing vocabulary shared by the game and the GPU.
//!
//! Every solid is an instance of one of the shapes in [`crate::gfx::mesh`] -
//! chamfered box, cylinder, cone, sphere, capsule, prism, pyramid, quad - with a
//! transform, a colour and a PBR material. Instances are bucketed by shape as
//! they are pushed, so the renderer can issue one draw per shape with no sorting.
//!
//! Glows are additive camera-facing sprites and live in their own list.

use bytemuck::{Pod, Zeroable};

pub use super::mesh::{SHAPE_COUNT, Shape};

pub type Color = [f32; 4];

/// One solid, or one glow billboard.
///
/// Solids: `pos` is the centre, `scale` the full size, `rot` is (yaw, pitch),
/// `params` is (emissive, unused), `material` is (roughness, metallic).
/// Glows: `pos` is the centre, `scale.x` the radius, `params.x` the falloff.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
pub struct Instance {
    pub pos: [f32; 3],
    pub scale: [f32; 3],
    pub rot: [f32; 2],
    pub params: [f32; 2],
    pub color: Color,
    pub material: [f32; 2],
    pub _pad: [f32; 2],
}

/// Which layer of `assets/textures.bin` a ground tile uses.
///
/// The order is the blob's own, set by `LAYERS` in `tools/bake_textures.py`.
#[derive(Clone, Copy, Debug)]
pub enum GroundTex {
    Grass = 0,
    Dirt = 1,
}

/// Common surface finishes, so call sites read as materials rather than numbers.
#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub roughness: f32,
    pub metallic: f32,
}

impl Material {
    pub const STONE: Material = Material {
        roughness: 0.92,
        metallic: 0.0,
    };
    pub const EARTH: Material = Material {
        roughness: 0.98,
        metallic: 0.0,
    };
    pub const FOLIAGE: Material = Material {
        roughness: 0.86,
        metallic: 0.0,
    };
    pub const WOOD: Material = Material {
        roughness: 0.78,
        metallic: 0.0,
    };
    // Warcraft III armour is a painted texture, not a mirror. Full metallic
    // zeroes the diffuse term, so a metal part's whole colour came from the
    // specular lobe: a blown white highlight on every rounded surface and the
    // pale blue sky probe everywhere else. That is what turned an army of
    // orcs, skeletons and demons into one crowd of identical blue-grey
    // figurines. Rough it right down and let most of the colour be diffuse.
    pub const METAL: Material = Material {
        roughness: 0.62,
        metallic: 0.30,
    };
    pub const DARK_METAL: Material = Material {
        roughness: 0.74,
        metallic: 0.25,
    };
    pub const GEM: Material = Material {
        roughness: 0.14,
        metallic: 0.2,
    };
    pub const CHITIN: Material = Material {
        roughness: 0.72,
        metallic: 0.0,
    };
    pub const WATER: Material = Material {
        roughness: 0.08,
        metallic: 0.0,
    };

    #[inline]
    fn pack(self) -> [f32; 2] {
        [self.roughness, self.metallic]
    }
}

impl Default for Material {
    fn default() -> Self {
        Material::STONE
    }
}

pub struct DrawList {
    /// Solids, bucketed by shape so the renderer never has to sort.
    pub solid: [Vec<Instance>; SHAPE_COUNT],
    /// Additive camera-facing sprites: glows, muzzle flashes, auras.
    pub glow: Vec<Instance>,
}

impl Default for DrawList {
    // Derived `Default` stops at thirty-two elements, and there are more
    // buckets than that once the baked models have their own.
    fn default() -> Self {
        DrawList {
            solid: std::array::from_fn(|_| Vec::new()),
            glow: Vec::new(),
        }
    }
}

impl DrawList {
    pub fn clear(&mut self) {
        for b in &mut self.solid {
            b.clear();
        }
        self.glow.clear();
    }

    pub fn solid_count(&self) -> usize {
        self.solid.iter().map(|b| b.len()).sum()
    }

    pub fn len(&self) -> usize {
        self.solid_count() + self.glow.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Appends another list's solids into this one, bucket for bucket.
    pub fn append_solids(&mut self, other: &DrawList) {
        for (dst, src) in self.solid.iter_mut().zip(other.solid.iter()) {
            dst.extend_from_slice(src);
        }
    }

    // -------------------------------------------------- core

    /// The one call everything else routes through.
    #[allow(clippy::too_many_arguments)]
    pub fn shape(
        &mut self,
        shape: Shape,
        pos: [f32; 3],
        scale: [f32; 3],
        yaw: f32,
        pitch: f32,
        color: Color,
        mat: Material,
        emissive: f32,
    ) {
        self.solid[shape as usize].push(Instance {
            pos,
            scale,
            rot: [yaw, pitch],
            params: [emissive, 0.0],
            color,
            material: mat.pack(),
            _pad: [0.0; 2],
        });
    }

    // -------------------------------------------------- convenience

    pub fn cube(&mut self, pos: [f32; 3], scale: [f32; 3], yaw: f32, color: Color) {
        self.shape(
            Shape::Box,
            pos,
            scale,
            yaw,
            0.0,
            color,
            Material::STONE,
            0.0,
        );
    }

    pub fn cube_mat(
        &mut self,
        pos: [f32; 3],
        scale: [f32; 3],
        yaw: f32,
        color: Color,
        mat: Material,
    ) {
        self.shape(Shape::Box, pos, scale, yaw, 0.0, color, mat, 0.0);
    }

    /// A box that glows from within (`em` 0..1).
    pub fn cube_lit(&mut self, pos: [f32; 3], scale: [f32; 3], yaw: f32, color: Color, em: f32) {
        self.shape(Shape::Box, pos, scale, yaw, 0.0, color, Material::GEM, em);
    }

    pub fn sphere(&mut self, pos: [f32; 3], d: f32, color: Color, mat: Material) {
        self.shape(Shape::Sphere, pos, [d, d, d], 0.0, 0.0, color, mat, 0.0);
    }

    pub fn sphere_lit(&mut self, pos: [f32; 3], d: f32, color: Color, em: f32) {
        self.shape(
            Shape::Sphere,
            pos,
            [d, d, d],
            0.0,
            0.0,
            color,
            Material::GEM,
            em,
        );
    }

    /// A cylinder standing on its Z axis.
    pub fn cylinder(
        &mut self,
        pos: [f32; 3],
        d: f32,
        h: f32,
        yaw: f32,
        color: Color,
        mat: Material,
    ) {
        self.shape(Shape::Cylinder, pos, [d, d, h], yaw, 0.0, color, mat, 0.0);
    }

    pub fn cone(&mut self, pos: [f32; 3], d: f32, h: f32, yaw: f32, color: Color, mat: Material) {
        self.shape(Shape::Cone, pos, [d, d, h], yaw, 0.0, color, mat, 0.0);
    }

    pub fn prism(&mut self, pos: [f32; 3], d: f32, h: f32, yaw: f32, color: Color, mat: Material) {
        self.shape(Shape::Prism, pos, [d, d, h], yaw, 0.0, color, mat, 0.0);
    }

    pub fn pyramid(
        &mut self,
        pos: [f32; 3],
        d: f32,
        h: f32,
        yaw: f32,
        color: Color,
        mat: Material,
    ) {
        self.shape(Shape::Pyramid, pos, [d, d, h], yaw, 0.0, color, mat, 0.0);
    }

    /// A box standing on the ground at `p`, `scale.z` tall.
    pub fn prop(&mut self, p: [f32; 2], scale: [f32; 3], yaw: f32, color: Color) {
        self.cube([p[0], p[1], scale[2] * 0.5], scale, yaw, color);
    }

    /// A flat slab lying on the ground - terrain, road, pads.
    pub fn slab(&mut self, p: [f32; 2], size: [f32; 2], z: f32, thickness: f32, color: Color) {
        self.shape(
            Shape::Box,
            [p[0], p[1], z - thickness * 0.5],
            [size[0], size[1], thickness],
            0.0,
            0.0,
            color,
            Material::EARTH,
            0.0,
        );
    }

    pub fn slab_mat(
        &mut self,
        p: [f32; 2],
        size: [f32; 2],
        z: f32,
        thickness: f32,
        color: Color,
        mat: Material,
    ) {
        self.shape(
            Shape::Box,
            [p[0], p[1], z - thickness * 0.5],
            [size[0], size[1], thickness],
            0.0,
            0.0,
            color,
            mat,
            0.0,
        );
    }

    /// A capsule stretched between two points - limbs, beams, struts.
    pub fn bar(&mut self, a: [f32; 3], b: [f32; 3], w: f32, color: Color, em: f32) {
        self.link(Shape::Capsule, a, b, w, color, Material::METAL, em);
    }

    /// Same, but you pick the shape and material.
    pub fn link(
        &mut self,
        shape: Shape,
        a: [f32; 3],
        b: [f32; 3],
        w: f32,
        color: Color,
        mat: Material,
        em: f32,
    ) {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let flat = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let len = (flat * flat + d[2] * d[2]).sqrt();
        if len < 1e-5 {
            return;
        }
        // The mesh runs along Z, so the instance is scaled on Z and pitched to
        // point from a to b.
        let centre = [
            (a[0] + b[0]) * 0.5,
            (a[1] + b[1]) * 0.5,
            (a[2] + b[2]) * 0.5,
        ];
        let yaw = d[1].atan2(d[0]);
        let pitch = d[2].atan2(flat) - std::f32::consts::FRAC_PI_2;
        self.shape(shape, centre, [w, w, len], yaw, pitch, color, mat, em);
    }

    /// A lofted limb from `a` to `b`, `w` thick at the `a` end.
    ///
    /// Prefer this to `link(Shape::Capsule, ..)` for anything anatomical. A
    /// capsule is the same width all the way along, so a figure assembled from
    /// them reads as pipework; [`Shape::Taper`] narrows towards `b` and carries
    /// a little muscle near `a`, which is most of what separates a drawn
    /// creature from a diagram of one.
    #[allow(clippy::too_many_arguments)]
    pub fn limb(
        &mut self,
        a: [f32; 3],
        b: [f32; 3],
        w: f32,
        color: Color,
        mat: Material,
        em: f32,
    ) {
        self.link(Shape::Taper, a, b, w, color, mat, em);
    }

    /// One instance of a baked model, in the bucket that model was loaded into.
    ///
    /// A model is a whole unit in a single draw - an orc's skin, leather and axe
    /// are colours inside the mesh rather than separate instances - so this is
    /// one call where a generated build was forty.
    #[allow(clippy::too_many_arguments)]
    pub fn model(
        &mut self,
        slot: usize,
        pos: [f32; 3],
        scale: [f32; 3],
        yaw: f32,
        pitch: f32,
        color: Color,
        mat: Material,
        em: f32,
    ) {
        let bucket = crate::gfx::mesh::model_bucket(slot);
        if bucket >= self.solid.len() {
            return;
        }
        self.solid[bucket].push(Instance {
            pos,
            scale,
            rot: [yaw, pitch],
            params: [em, bucket as f32],
            color,
            material: [mat.roughness, mat.metallic],
            _pad: [0.0; 2],
        });
    }

    /// A ground tile, textured by world position.
    ///
    /// The terrain has no UVs and does not need any: it is axis-aligned and
    /// lies flat, so the world x and y *are* the texture coordinates. That is
    /// what keeps the texture path down to one number on the instance, and why
    /// nothing else in the vertex format had to change to get a textured field.
    #[allow(clippy::too_many_arguments)]
    pub fn ground(
        &mut self,
        layer: GroundTex,
        pos: [f32; 3],
        size: [f32; 2],
        color: Color,
        mat: Material,
    ) {
        self.solid[Shape::Quad as usize].push(Instance {
            pos,
            scale: [size[0], size[1], 1.0],
            rot: [0.0, 0.0],
            // params.y is the texture layer, one-based: zero means untextured,
            // which is every other thing the game draws.
            params: [0.0, layer as u32 as f32 + 1.0],
            color,
            material: [mat.roughness, mat.metallic],
            _pad: [0.0; 2],
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
            material: [1.0, 0.0],
            _pad: [0.0; 2],
        });
    }

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

    /// A flat ring lying on the ground, for range indicators and shockwaves.
    pub fn ground_ring(&mut self, c: [f32; 2], r: f32, w: f32, color: Color, segments: u32) {
        let n = segments.max(8);
        let step = std::f32::consts::TAU / n as f32;
        let seg_len = (r * step) * 1.15;
        for i in 0..n {
            let a = i as f32 * step;
            let (s, co) = a.sin_cos();
            self.shape(
                // A flat quad, not a box: a ring painted on the ground should
                // never catch a specular highlight, and a sixty-segment ring of
                // boxes is most of a frame's instance count on a board of aura
                // towers.
                Shape::Quad,
                // Above the plot surface, or the ring disappears under it.
                [c[0] + co * r, c[1] + s * r, 0.17],
                [seg_len, w, 1.0],
                a + std::f32::consts::FRAC_PI_2,
                0.0,
                color,
                Material::GEM,
                0.45,
            );
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
