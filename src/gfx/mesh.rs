//! Procedural mesh library.
//!
//! There are no art assets in this project, so every shape is generated here at
//! startup. The point is that a tower is a *cylinder with a cone roof*, not a
//! stack of cubes: curved silhouettes and smooth shading are most of what
//! separates "programmer boxes" from something that reads as modelled.
//!
//! All meshes are unit-sized and centred on the origin so one instance
//! transform (position, scale, yaw, pitch) works for any of them. Vertex counts
//! are deliberately low - a sphere is 12x8 - because the whole scene is drawn
//! with a few thousand instances and has to hold up on a phone.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    /// The vertex's own colour, multiplied into the instance colour.
    ///
    /// White for every generated primitive, so those are tinted entirely by the
    /// instance as before. A baked model carries the colours its author gave
    /// it - an orc's skin, straps and axe are different colours within one mesh
    /// and one draw - which is the whole reason this attribute exists.
    pub col: [f32; 3],
}

/// Which shape an instance draws. Kept in `Instance::params.y`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Shape {
    /// Chamfered box. Even the "box" has bevels, so its edges catch light.
    Box = 0,
    /// Capped cylinder along Z.
    Cylinder = 1,
    /// Cone along Z, apex at +Z.
    Cone = 2,
    Sphere = 3,
    /// Rounded ends along Z - limbs, barrels, tails.
    Capsule = 4,
    /// Hexagonal prism along Z.
    Prism = 5,
    /// Four-sided pyramid, apex at +Z.
    Pyramid = 6,
    /// A flat unit square in the XY plane - ground, decals, bars.
    Quad = 7,
    /// A lofted limb along Z: thick at -Z, tapering to a rounded tip at +Z,
    /// with a slight swell near the base. Arms, legs, necks, tails, horns.
    Taper = 8,
}

/// Generated primitives. Baked models are numbered above these.
pub const PRIM_COUNT: usize = 9;

/// How many baked models the buffers make room for. Slots past what
/// `assets/models.bin` actually holds stay empty and are skipped when drawing,
/// so adding a model to the bake does not need any of this re-sized.
pub const MODEL_SLOTS: usize = 64;

/// Every bucket the renderer batches by: primitives first, then models.
pub const SHAPE_COUNT: usize = PRIM_COUNT + MODEL_SLOTS;

/// Which bucket a baked model lives in, given its slot.
#[inline]
pub fn model_bucket(slot: usize) -> usize {
    PRIM_COUNT + slot
}

impl Shape {
    #[inline]
    pub fn as_f32(self) -> f32 {
        self as u32 as f32
    }
    pub fn from_index(i: usize) -> Shape {
        match i {
            1 => Shape::Cylinder,
            2 => Shape::Cone,
            3 => Shape::Sphere,
            4 => Shape::Capsule,
            5 => Shape::Prism,
            6 => Shape::Pyramid,
            7 => Shape::Quad,
            8 => Shape::Taper,
            _ => Shape::Box,
        }
    }
}

/// Where one shape lives inside the shared vertex buffer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Span {
    pub first: u32,
    pub count: u32,
}

pub struct Library {
    pub vertices: Vec<Vertex>,
    pub spans: [Span; SHAPE_COUNT],
    /// Archetype name to model slot, for whatever `assets/models.bin` held.
    pub models: Vec<(String, usize)>,
}

/// The baked models, built by `tools/bake_models.py` from CC0 sources.
///
/// Embedded rather than loaded from disk because the game also runs as wasm,
/// where there is no filesystem to read and one fetch fewer is one failure mode
/// fewer.
static MODEL_BLOB: &[u8] = include_bytes!("../../assets/models.bin");

/// Reads the blob into spans appended after the generated primitives.
///
/// Format, all little-endian: magic `GTDM`, a version, a model count, then that
/// many `(u8 name length, name, u32 first vertex, u32 count)` records, then the
/// vertices themselves as nine floats each - position, normal, colour.
///
/// A blob that is missing, truncated or of an unknown version leaves the model
/// slots empty, and every archetype falls back to its generated build. That is
/// deliberate: a bad asset should cost detail, not the game.
fn load_models(v: &mut Vec<Vertex>, spans: &mut [Span; SHAPE_COUNT]) -> Vec<(String, usize)> {
    let b = MODEL_BLOB;
    let mut out = Vec::new();
    if b.len() < 12 {
        return out;
    }
    let u32_at = |o: usize| -> u32 {
        u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    };
    if u32_at(0) != 0x4D44_5447 || u32_at(4) != 1 {
        return out;
    }
    let count = u32_at(8) as usize;
    let mut o = 12;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count.min(MODEL_SLOTS) {
        if o >= b.len() {
            return out;
        }
        let n = b[o] as usize;
        o += 1;
        if o + n + 8 > b.len() {
            return out;
        }
        let name = String::from_utf8_lossy(&b[o..o + n]).into_owned();
        o += n;
        let first = u32_at(o) as usize;
        let vcount = u32_at(o + 4) as usize;
        o += 8;
        records.push((name, first, vcount));
    }
    let body = o;
    let stride = 36;
    for (slot, (name, first, vcount)) in records.into_iter().enumerate() {
        let start = body + first * stride;
        let end = start + vcount * stride;
        if end > b.len() {
            continue;
        }
        let base = v.len() as u32;
        for k in 0..vcount {
            let f = |j: usize| -> f32 {
                let o = start + k * stride + j * 4;
                f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
            };
            v.push(Vertex {
                pos: [f(0), f(1), f(2)],
                nrm: [f(3), f(4), f(5)],
                col: [f(6), f(7), f(8)],
            });
        }
        spans[model_bucket(slot)] = Span {
            first: base,
            count: vcount as u32,
        };
        out.push((name, slot));
    }
    out
}

/// The archetype names the blob carried, and the slot each landed in.
///
/// Built once and cached: `build()` is called per renderer and there may be
/// several, but the blob is a constant.
pub fn model_slots() -> &'static [(String, usize)] {
    static SLOTS: std::sync::OnceLock<Vec<(String, usize)>> = std::sync::OnceLock::new();
    SLOTS.get_or_init(|| {
        let mut spans = [Span::default(); SHAPE_COUNT];
        let mut verts: Vec<Vertex> = Vec::new();
        load_models(&mut verts, &mut spans)
    })
}

/// Builds every shape into one buffer, recording each one's span.
pub fn build() -> Library {
    let mut v: Vec<Vertex> = Vec::with_capacity(4096);
    let mut spans = [Span::default(); SHAPE_COUNT];

    let mut record = |v: &mut Vec<Vertex>, idx: usize, tris: Vec<Vertex>| {
        spans[idx] = Span {
            first: v.len() as u32,
            count: tris.len() as u32,
        };
        v.extend(tris);
    };

    record(&mut v, Shape::Box as usize, chamfered_box(0.12));
    record(&mut v, Shape::Cylinder as usize, cylinder(14, true));
    record(&mut v, Shape::Cone as usize, cone(14));
    record(&mut v, Shape::Sphere as usize, sphere(14, 9));
    record(&mut v, Shape::Capsule as usize, capsule(12, 5));
    record(&mut v, Shape::Prism as usize, prism(6));
    record(&mut v, Shape::Pyramid as usize, pyramid());
    record(&mut v, Shape::Quad as usize, quad());
    record(&mut v, Shape::Taper as usize, taper(14, 12));

    let models = load_models(&mut v, &mut spans);
    Library {
        vertices: v,
        spans,
        models,
    }
}

// ---------------------------------------------------------------- helpers

fn tri(a: [f32; 3], b: [f32; 3], c: [f32; 3], out: &mut Vec<Vertex>) {
    // Flat normal from the winding, so every face is lit correctly without
    // needing authored normals.
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = normalize([
        u[1] * w[2] - u[2] * w[1],
        u[2] * w[0] - u[0] * w[2],
        u[0] * w[1] - u[1] * w[0],
    ]);
    const W: [f32; 3] = [1.0, 1.0, 1.0];
    out.push(Vertex { pos: a, nrm: n, col: W });
    out.push(Vertex { pos: b, nrm: n, col: W });
    out.push(Vertex { pos: c, nrm: n, col: W });
}

/// Triangle with explicit per-vertex normals, for anything curved.
fn tri_n(
    a: ([f32; 3], [f32; 3]),
    b: ([f32; 3], [f32; 3]),
    c: ([f32; 3], [f32; 3]),
    out: &mut Vec<Vertex>,
) {
    const W: [f32; 3] = [1.0, 1.0, 1.0];
    out.push(Vertex {
        pos: a.0,
        nrm: normalize(a.1),
        col: W,
    });
    out.push(Vertex {
        pos: b.0,
        nrm: normalize(b.1),
        col: W,
    });
    out.push(Vertex {
        pos: c.0,
        nrm: normalize(c.1),
        col: W,
    });
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-6 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

// ---------------------------------------------------------------- shapes

/// A unit box whose edges and corners are cut back by `c`. The bevels are what
/// make it catch a highlight along every edge instead of reading as a slab.
fn chamfered_box(c: f32) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(600);
    let h = 0.5;
    let i = h - c; // inset where the flat face ends

    // Six flat faces, shrunk by the chamfer.
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
    ];
    for (n, u, w) in faces {
        let p = |su: f32, sw: f32| {
            [
                n[0] * h + u[0] * su * i + w[0] * sw * i,
                n[1] * h + u[1] * su * i + w[1] * sw * i,
                n[2] * h + u[2] * su * i + w[2] * sw * i,
            ]
        };
        let (a, b, cc, d) = (p(-1.0, -1.0), p(1.0, -1.0), p(1.0, 1.0), p(-1.0, 1.0));
        tri(a, b, cc, &mut out);
        tri(a, cc, d, &mut out);
    }

    // Twelve edge bevels, each a quad bridging two faces.
    let edges: [([f32; 3], [f32; 3]); 12] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, -1.0, 0.0], [0.0, 0.0, -1.0]),
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
    ];
    for (na, nb) in edges {
        // The axis the edge runs along is the one neither normal uses.
        let axis = [
            1.0 - na[0].abs() - nb[0].abs(),
            1.0 - na[1].abs() - nb[1].abs(),
            1.0 - na[2].abs() - nb[2].abs(),
        ];
        let mid = |s: f32| {
            [
                na[0] * h + nb[0] * i + axis[0] * s * i,
                na[1] * h + nb[1] * i + axis[1] * s * i,
                na[2] * h + nb[2] * i + axis[2] * s * i,
            ]
        };
        let mid2 = |s: f32| {
            [
                na[0] * i + nb[0] * h + axis[0] * s * i,
                na[1] * i + nb[1] * h + axis[1] * s * i,
                na[2] * i + nb[2] * h + axis[2] * s * i,
            ]
        };
        let bevel = normalize([na[0] + nb[0], na[1] + nb[1], na[2] + nb[2]]);
        let (a, b, cc, d) = (mid(-1.0), mid(1.0), mid2(1.0), mid2(-1.0));
        tri_n((a, bevel), (b, bevel), (cc, bevel), &mut out);
        tri_n((a, bevel), (cc, bevel), (d, bevel), &mut out);
    }
    out
}

/// Radius 0.5, height 1, axis along Z.
fn cylinder(sides: usize, capped: bool) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(sides * 12);
    let r = 0.5;
    let h = 0.5;
    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let (p0, p1) = ([c0 * r, s0 * r], [c1 * r, s1 * r]);
        let (n0, n1) = ([c0, s0, 0.0], [c1, s1, 0.0]);

        // Side wall, smooth-shaded around the ring.
        tri_n(
            ([p0[0], p0[1], -h], n0),
            ([p1[0], p1[1], -h], n1),
            ([p1[0], p1[1], h], n1),
            &mut out,
        );
        tri_n(
            ([p0[0], p0[1], -h], n0),
            ([p1[0], p1[1], h], n1),
            ([p0[0], p0[1], h], n0),
            &mut out,
        );
        if capped {
            tri(
                [0.0, 0.0, h],
                [p0[0], p0[1], h],
                [p1[0], p1[1], h],
                &mut out,
            );
            tri(
                [0.0, 0.0, -h],
                [p1[0], p1[1], -h],
                [p0[0], p0[1], -h],
                &mut out,
            );
        }
    }
    out
}

/// Base radius 0.5 at -Z, apex at +Z.
fn cone(sides: usize) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(sides * 6);
    let r = 0.5;
    let h = 0.5;
    // Slope of the side, used for the normals so the cone is smooth around.
    let slant = (r / (2.0f32 * h)).atan();
    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let n = |c: f32, s: f32| [c * slant.cos(), s * slant.cos(), slant.sin()];
        tri_n(
            ([c0 * r, s0 * r, -h], n(c0, s0)),
            ([c1 * r, s1 * r, -h], n(c1, s1)),
            ([0.0, 0.0, h], n((c0 + c1) * 0.5, (s0 + s1) * 0.5)),
            &mut out,
        );
        tri(
            [0.0, 0.0, -h],
            [c1 * r, s1 * r, -h],
            [c0 * r, s0 * r, -h],
            &mut out,
        );
    }
    out
}

/// Unit-diameter sphere.
fn sphere(segments: usize, rings: usize) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(segments * rings * 6);
    let r = 0.5;
    for y in 0..rings {
        let v0 = y as f32 / rings as f32 * std::f32::consts::PI;
        let v1 = (y + 1) as f32 / rings as f32 * std::f32::consts::PI;
        for x in 0..segments {
            let u0 = x as f32 / segments as f32 * std::f32::consts::TAU;
            let u1 = (x + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let p = |u: f32, v: f32| {
                let n = [v.sin() * u.cos(), v.sin() * u.sin(), v.cos()];
                ([n[0] * r, n[1] * r, n[2] * r], n)
            };
            let (a, b, c, d) = (p(u0, v0), p(u1, v0), p(u1, v1), p(u0, v1));
            if y != 0 {
                tri_n(a, b, c, &mut out);
            }
            if y != rings - 1 {
                tri_n(a, c, d, &mut out);
            }
        }
    }
    out
}

/// Radius 0.25 hemispherical ends, total height 1 along Z.
fn capsule(segments: usize, rings: usize) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(segments * rings * 12);
    let r = 0.25;
    let half = 0.5 - r; // centre of each hemisphere

    // Barrel.
    for x in 0..segments {
        let u0 = x as f32 / segments as f32 * std::f32::consts::TAU;
        let u1 = (x + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let (s0, c0) = u0.sin_cos();
        let (s1, c1) = u1.sin_cos();
        tri_n(
            ([c0 * r, s0 * r, -half], [c0, s0, 0.0]),
            ([c1 * r, s1 * r, -half], [c1, s1, 0.0]),
            ([c1 * r, s1 * r, half], [c1, s1, 0.0]),
            &mut out,
        );
        tri_n(
            ([c0 * r, s0 * r, -half], [c0, s0, 0.0]),
            ([c1 * r, s1 * r, half], [c1, s1, 0.0]),
            ([c0 * r, s0 * r, half], [c0, s0, 0.0]),
            &mut out,
        );
    }
    // Two hemispheres.
    for (sign, zc) in [(1.0f32, half), (-1.0f32, -half)] {
        for y in 0..rings {
            let v0 = y as f32 / rings as f32 * std::f32::consts::FRAC_PI_2;
            let v1 = (y + 1) as f32 / rings as f32 * std::f32::consts::FRAC_PI_2;
            for x in 0..segments {
                let u0 = x as f32 / segments as f32 * std::f32::consts::TAU;
                let u1 = (x + 1) as f32 / segments as f32 * std::f32::consts::TAU;
                let p = |u: f32, v: f32| {
                    let n = [v.cos() * u.cos(), v.cos() * u.sin(), v.sin() * sign];
                    ([n[0] * r, n[1] * r, zc + n[2] * r], n)
                };
                let (a, b, c, d) = (p(u0, v0), p(u1, v0), p(u1, v1), p(u0, v1));
                if sign > 0.0 {
                    tri_n(a, b, c, &mut out);
                    tri_n(a, c, d, &mut out);
                } else {
                    tri_n(a, c, b, &mut out);
                    tri_n(a, d, c, &mut out);
                }
            }
        }
    }
    out
}

fn prism(sides: usize) -> Vec<Vertex> {
    cylinder(sides, true)
}

fn pyramid() -> Vec<Vertex> {
    let mut out = Vec::with_capacity(18);
    let h = 0.5;
    let c = [[-h, -h, -h], [h, -h, -h], [h, h, -h], [-h, h, -h]];
    let apex = [0.0, 0.0, h];
    for i in 0..4 {
        tri(c[i], c[(i + 1) % 4], apex, &mut out);
    }
    tri(c[0], c[2], c[1], &mut out);
    tri(c[0], c[3], c[2], &mut out);
    out
}

/// The radius of a limb a fraction `t` along it, and how fast that radius is
/// changing - which is what the surface normal needs.
///
/// A cylinder is the wrong shape for an arm and it is why a figure built out of
/// them reads as plumbing. A real limb is thickest a little below the shoulder,
/// thins towards the wrist, and ends in a curve rather than a flat cap. This is
/// that profile, and returning the derivative alongside the radius is what lets
/// the normal be exact instead of averaged from neighbouring triangles.
fn limb_profile(t: f32) -> (f32, f32) {
    use std::f32::consts::PI;
    // Taper: full at the base, 38% at the tip.
    let taper = 1.0 - 0.62 * t;
    let d_taper = -0.62;
    // Swell: a little muscle in the first half, gone by the tip.
    let sw = (t * PI).sin() * (1.0 - t);
    let d_sw = PI * (t * PI).cos() * (1.0 - t) - (t * PI).sin();
    let muscle = 1.0 + 0.20 * sw;
    let d_muscle = 0.20 * d_sw;
    // Rounded ends, as a circular blend over the outer tenth at each end.
    let e = 0.10f32;
    let (cap, d_cap) = if t < e {
        let u = (e - t) / e;
        let c = (1.0 - u * u).max(1e-4).sqrt();
        (c, u / (e * c))
    } else if t > 1.0 - e {
        let u = (t - (1.0 - e)) / e;
        let c = (1.0 - u * u).max(1e-4).sqrt();
        (c, -u / (e * c))
    } else {
        (1.0, 0.0)
    };
    let base = 0.25;
    let r = base * taper * muscle * cap;
    let dr = base * (d_taper * muscle * cap + taper * d_muscle * cap + taper * muscle * d_cap);
    (r, dr)
}

/// A limb lofted along Z from -0.5 (base) to +0.5 (tip).
fn taper(segments: usize, rings: usize) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(segments * rings * 6);
    // dr/dt is per unit of t, and t spans the mesh's whole unit length, so the
    // slope in mesh space is the same number. The surface normal of a body of
    // revolution is (cos, sin, -dr/dz), normalised.
    let ring = |t: f32| {
        let (r, dr) = limb_profile(t);
        (r, t - 0.5, -dr)
    };
    for y in 0..rings {
        let (r0, z0, s0) = ring(y as f32 / rings as f32);
        let (r1, z1, s1) = ring((y + 1) as f32 / rings as f32);
        for x in 0..segments {
            let u0 = x as f32 / segments as f32 * std::f32::consts::TAU;
            let u1 = (x + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let (sa, ca) = u0.sin_cos();
            let (sb, cb) = u1.sin_cos();
            let n = |c: f32, sn: f32, slope: f32| normalize([c, sn, slope]);
            let a = ([ca * r0, sa * r0, z0], n(ca, sa, s0));
            let b = ([cb * r0, sb * r0, z0], n(cb, sb, s0));
            let c = ([cb * r1, sb * r1, z1], n(cb, sb, s1));
            let e = ([ca * r1, sa * r1, z1], n(ca, sa, s1));
            tri_n(a, b, c, &mut out);
            tri_n(a, c, e, &mut out);
        }
    }
    out
}

fn quad() -> Vec<Vertex> {
    let mut out = Vec::with_capacity(6);
    let h = 0.5;
    let n = [0.0, 0.0, 1.0];
    let p = [[-h, -h, 0.0], [h, -h, 0.0], [h, h, 0.0], [-h, h, 0.0]];
    tri_n((p[0], n), (p[1], n), (p[2], n), &mut out);
    tri_n((p[0], n), (p[2], n), (p[3], n), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The number of vertices a primitive may hold.
    ///
    /// Split from the model budget below because the two are drawn completely
    /// differently: a primitive is instanced thousands of times a frame and its
    /// triangle count is multiplied by that, while a baked model is one unit.
    const PRIM_TRI_BUDGET: usize = 1200;

    #[test]
    fn every_primitive_is_present_and_well_formed() {
        let lib = build();
        assert!(!lib.vertices.is_empty());
        for i in 0..PRIM_COUNT {
            let s = lib.spans[i];
            assert!(s.count > 0, "shape {i} produced no geometry");
            assert!(
                s.count % 3 == 0,
                "shape {i} has {} vertices, not a whole number of triangles",
                s.count
            );
            assert!((s.first + s.count) as usize <= lib.vertices.len());
        }
        // Spans must tile the buffer without overlapping. Empty model slots are
        // not spans of anything, so they are not in the comparison.
        let mut ordered: Vec<Span> = lib.spans.iter().copied().filter(|s| s.count > 0).collect();
        ordered.sort_by_key(|s| s.first);
        for w in ordered.windows(2) {
            assert!(w[0].first + w[0].count <= w[1].first, "shape spans overlap");
        }
    }

    /// Every model the blob claims is a model that actually loaded.
    ///
    /// The blob is built by `tools/bake_models.py` and read by `load_models`,
    /// and the two agreeing is the whole contract - a silent mismatch shows up
    /// as an archetype quietly falling back to its generated build, which is
    /// exactly the kind of thing nobody notices.
    #[test]
    fn every_baked_model_loaded() {
        let lib = build();
        assert!(
            !lib.models.is_empty(),
            "assets/models.bin loaded nothing - the bake and the reader disagree"
        );
        for (name, slot) in &lib.models {
            let s = lib.spans[model_bucket(*slot)];
            assert!(s.count > 0, "{name} claims slot {slot} but has no geometry");
            assert!(s.count % 3 == 0, "{name} is not whole triangles");
            assert!((s.first + s.count) as usize <= lib.vertices.len());
        }
    }

    #[test]
    fn primitive_vertices_stay_inside_the_unit_cell_and_have_real_normals() {
        let lib = build();
        let end = (0..PRIM_COUNT)
            .map(|i| lib.spans[i].first + lib.spans[i].count)
            .max()
            .unwrap_or(0) as usize;
        for (i, v) in lib.vertices[..end].iter().enumerate() {
            for k in 0..3 {
                assert!(
                    v.pos[k].abs() <= 0.5001,
                    "vertex {i} escapes the unit cell: {:?}",
                    v.pos
                );
            }
            let len = (v.nrm[0] * v.nrm[0] + v.nrm[1] * v.nrm[1] + v.nrm[2] * v.nrm[2]).sqrt();
            assert!(
                (len - 1.0).abs() < 0.01,
                "vertex {i} normal is not unit: {len}"
            );
        }
    }

    /// A baked model stands on the ground and is one unit tall.
    ///
    /// Not the unit *cell* - a dragon's wings are wider than it is tall, and
    /// that is fine. What matters is the two things the game assumes: feet at
    /// zero, so a model does not float or sink, and a height of one, so
    /// `Pose::r` scales every archetype the same way.
    #[test]
    fn every_baked_model_stands_on_the_ground_at_unit_height() {
        let lib = build();
        for (name, slot) in &lib.models {
            let s = lib.spans[model_bucket(*slot)];
            let v = &lib.vertices[s.first as usize..(s.first + s.count) as usize];
            let lo = v.iter().fold(f32::MAX, |m, x| m.min(x.pos[2]));
            let hi = v.iter().fold(f32::MIN, |m, x| m.max(x.pos[2]));
            assert!(lo.abs() < 0.01, "{name} does not stand on the ground: {lo}");
            assert!((hi - 1.0).abs() < 0.01, "{name} is {hi} tall, not one");
            let wide = v
                .iter()
                .fold(0.0f32, |m, x| m.max(x.pos[0].abs().max(x.pos[1].abs())));
            assert!(wide < 4.0, "{name} is {wide} wide - that is not a unit");
        }
    }

    #[test]
    fn the_primitive_budget_stays_low_enough_for_a_phone() {
        let lib = build();
        let end = (0..PRIM_COUNT)
            .map(|i| lib.spans[i].first + lib.spans[i].count)
            .max()
            .unwrap_or(0) as usize;
        let tris = end / 3;
        assert!(
            tris < PRIM_TRI_BUDGET,
            "the primitives are {tris} triangles - too heavy to instance thousands of times"
        );
    }

    /// No single model is heavy enough to sink a wave of it.
    ///
    /// The flood limit is seven hundred monsters, so a model's triangle count is
    /// multiplied by seven hundred in the worst frame the game allows. Eight
    /// thousand is about five and a half million triangles, which a desktop
    /// manages and a phone does not enjoy; anything above it needs a lighter
    /// model rather than a faster renderer.
    #[test]
    fn no_baked_model_is_too_heavy_for_a_full_wave() {
        let lib = build();
        for (name, slot) in &lib.models {
            let tris = lib.spans[model_bucket(*slot)].count / 3;
            assert!(
                tris < 8_000,
                "{name} is {tris} triangles; seven hundred of them is too many"
            );
        }
    }

}
