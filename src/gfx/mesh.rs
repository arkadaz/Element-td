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
}

pub const SHAPE_COUNT: usize = 8;

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
}

/// Builds every shape into one buffer, recording each one's span.
pub fn build() -> Library {
    let mut v: Vec<Vertex> = Vec::with_capacity(4096);
    let mut spans = [Span::default(); SHAPE_COUNT];

    let mut record = |v: &mut Vec<Vertex>, idx: usize, tris: Vec<Vertex>| {
        spans[idx] = Span { first: v.len() as u32, count: tris.len() as u32 };
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

    Library { vertices: v, spans }
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
    out.push(Vertex { pos: a, nrm: n });
    out.push(Vertex { pos: b, nrm: n });
    out.push(Vertex { pos: c, nrm: n });
}

/// Triangle with explicit per-vertex normals, for anything curved.
fn tri_n(
    a: ([f32; 3], [f32; 3]),
    b: ([f32; 3], [f32; 3]),
    c: ([f32; 3], [f32; 3]),
    out: &mut Vec<Vertex>,
) {
    out.push(Vertex { pos: a.0, nrm: normalize(a.1) });
    out.push(Vertex { pos: b.0, nrm: normalize(b.1) });
    out.push(Vertex { pos: c.0, nrm: normalize(c.1) });
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-6 { [0.0, 0.0, 1.0] } else { [v[0] / l, v[1] / l, v[2] / l] }
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
            tri([0.0, 0.0, h], [p0[0], p0[1], h], [p1[0], p1[1], h], &mut out);
            tri([0.0, 0.0, -h], [p1[0], p1[1], -h], [p0[0], p0[1], -h], &mut out);
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
        tri([0.0, 0.0, -h], [c1 * r, s1 * r, -h], [c0 * r, s0 * r, -h], &mut out);
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
    let c = [
        [-h, -h, -h],
        [h, -h, -h],
        [h, h, -h],
        [-h, h, -h],
    ];
    let apex = [0.0, 0.0, h];
    for i in 0..4 {
        tri(c[i], c[(i + 1) % 4], apex, &mut out);
    }
    tri(c[0], c[2], c[1], &mut out);
    tri(c[0], c[3], c[2], &mut out);
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

    #[test]
    fn every_shape_is_present_and_well_formed() {
        let lib = build();
        assert!(!lib.vertices.is_empty());
        for i in 0..SHAPE_COUNT {
            let s = lib.spans[i];
            assert!(s.count > 0, "shape {i} produced no geometry");
            assert!(
                s.count % 3 == 0,
                "shape {i} has {} vertices, not a whole number of triangles",
                s.count
            );
            assert!((s.first + s.count) as usize <= lib.vertices.len());
        }
        // Spans must tile the buffer without overlapping.
        let mut ordered: Vec<Span> = lib.spans.to_vec();
        ordered.sort_by_key(|s| s.first);
        for w in ordered.windows(2) {
            assert!(w[0].first + w[0].count <= w[1].first, "shape spans overlap");
        }
    }

    #[test]
    fn vertices_stay_inside_the_unit_cell_and_have_real_normals() {
        let lib = build();
        for (i, v) in lib.vertices.iter().enumerate() {
            for k in 0..3 {
                assert!(
                    v.pos[k].abs() <= 0.5001,
                    "vertex {i} escapes the unit cell: {:?}",
                    v.pos
                );
            }
            let len =
                (v.nrm[0] * v.nrm[0] + v.nrm[1] * v.nrm[1] + v.nrm[2] * v.nrm[2]).sqrt();
            assert!((len - 1.0).abs() < 0.01, "vertex {i} normal is not unit: {len}");
        }
    }

    #[test]
    fn the_budget_stays_low_enough_for_a_phone() {
        let lib = build();
        let tris = lib.vertices.len() / 3;
        assert!(
            tris < 1200,
            "the shape library is {tris} triangles - too heavy to instance thousands of times"
        );
    }
}
