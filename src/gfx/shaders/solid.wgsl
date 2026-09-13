// Physically based shading for the instanced shape library.
//
// Cook-Torrance GGX with a shadow-mapped key light, a hemisphere ambient term
// standing in for image-based lighting, and an analytic split-sum environment
// BRDF for the ambient specular. Materials come in per instance as (roughness,
// metallic), so stone, wood, foliage, polished metal and gems all respond
// differently to the same light instead of looking like tinted plastic.
//
// Two terms here exist purely to make the scene read rather than to be correct:
// a wide sky-coloured rim along every silhouette, and an ambient occlusion
// approximated from the shadow map. Both are called out where they are applied.

struct Uniforms {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_pos: vec4<f32>,
    light_dir: vec4<f32>,   // xyz = direction towards the light
    misc: vec4<f32>,        // x = drag, y = gravity, z = time, w = shadow texel
    fog: vec4<f32>,         // rgb = fog colour, a = density
};
@group(0) @binding(0) var<uniform> U: Uniforms;
/// How hard the ground's normal map bends the surface normal. One would be the
/// map's own strength; this is softer, because the camera looks down at fifty-
/// two degrees and a full-strength normal on a floor seen from above turns into
/// noise rather than relief.
const GROUND_RELIEF: f32 = 0.14;

/// Must match `mesh::POS_RANGE`. Vertex positions are stored as a fraction of
/// it so they fit in sixteen bits.
const POS_RANGE: f32 = 4.0;

/// How many colour layers `assets/textures.bin` holds. Each one's normal map is
/// that many layers further on, so this must match `LAYERS` in
/// `tools/bake_textures.py`; `the_texture_blob_matches_the_shader` checks it.
const GROUND_LAYERS: i32 = 5;

@group(0) @binding(3) var ground_tex: texture_2d_array<f32>;
@group(0) @binding(4) var ground_smp: sampler;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;

/// Shadow filter tier, overridden per quality preset when the shader is built.
/// It names a filter rather than counting taps: 0 skips the lookup outright, 1
/// takes a single comparison, and 4 takes the nine-tap box. So the cheap preset
/// pays one comparison per pixel where the dear one pays nine.
const SHADOW_TAPS: i32 = 4;

struct VsIn {
    // Packed - see `mesh::GpuVertex`. Positions arrive as a fraction of
    // POS_RANGE and are scaled back below; normals and colours arrive already
    // normalised by the snorm/unorm formats.
    @location(0) v_pos: vec4<f32>,
    @location(1) v_nrm: vec4<f32>,
    @location(2) i_pos: vec3<f32>,
    @location(3) i_scale: vec3<f32>,
    @location(4) i_rot: vec2<f32>,
    @location(5) i_params: vec2<f32>,
    @location(6) i_color: vec4<f32>,
    @location(7) i_material: vec2<f32>,
    // A baked model's own vertex colour; white on every generated primitive.
    @location(8) v_col: vec4<f32>,
    @location(9) v_dpos: vec4<f32>,
    @location(10) v_dnrm: vec4<f32>,
    @location(11) i_anim: vec2<f32>,
    // x/y are authored source UV, z says the source actually carried UVs.
    @location(12) v_uv: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) nrm: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) emissive: f32,
    @location(4) light_pos: vec4<f32>,
    @location(5) material: vec2<f32>,
    // Per-instance albedo jitter, so flat faces are not all exactly one colour.
    // Computed once per vertex rather than sampled per pixel - the old value
    // noise cost four hashes on every fragment on screen for an effect nobody
    // can see at gameplay distance.
    @location(6) tint: f32,
    // Ground texture layer, one-based. Zero for everything that is not terrain.
    // This is a discrete texture-array selector, never a spatial value. A
    // perspective-interpolated 5.0 can arrive as 4.999 and truncate Mosswatch
    // to the preceding layer in horizontal screen bands.
    @interpolate(flat) @location(7) tex: f32,
    // Per-triangle source material id. Zero means use the instance material.
    @location(8) part_material: f32,
    // Authored UV.xy and source-UV flag in z, retained through the bake.
    @location(9) model_uv: vec3<f32>,
};

// Yaw about Z, then pitch tilting the local +Z axis.
fn rot_of(yaw: f32, pitch: f32) -> mat3x3<f32> {
    let cy = cos(yaw);
    let sy = sin(yaw);
    let cp = cos(pitch);
    let sp = sin(pitch);
    let ry = mat3x3<f32>(
        vec3<f32>(cp, 0.0, sp),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(-sp, 0.0, cp),
    );
    let rz = mat3x3<f32>(
        vec3<f32>(cy, sy, 0.0),
        vec3<f32>(-sy, cy, 0.0),
        vec3<f32>(0.0, 0.0, 1.0),
    );
    return rz * ry;
}

@vertex
fn vs(in: VsIn) -> VsOut {
    let r = rot_of(in.i_rot.x, in.i_rot.y);
    // The walk. Two poses half a stride apart are baked into the mesh - the
    // second as an offset - and this blends between them. A cosine rather than
    // a sawtooth so the figure eases through each contact instead of snapping
    // back at the end of the cycle, and because half a walk cycle IS the other
    // half with the legs swapped, going back and forth between two poses is the
    // whole stride rather than half of one.
    let step = 0.5 - 0.5 * cos(in.i_anim.x * 6.2831853);
    let v_pos = (in.v_pos.xyz + in.v_dpos.xyz * step) * POS_RANGE;
    let v_nrm = in.v_nrm.xyz + in.v_dnrm.xyz * step;
    let world = in.i_pos + r * (v_pos * in.i_scale);
    // Non-uniform scale needs the inverse-transpose; for an axis-aligned scale
    // that is just dividing by the scale.
    let inv = vec3<f32>(1.0) / max(in.i_scale, vec3<f32>(1e-4));
    let nrm = normalize(r * (v_nrm * inv));

    var o: VsOut;
    o.clip = U.view_proj * vec4<f32>(world, 1.0);
    o.nrm = nrm;
    o.world = world;
    o.color = in.i_color;
    o.emissive = in.i_params.x;
    o.color = vec4<f32>(o.color.rgb * in.v_col.rgb, o.color.a);
    o.tex = in.i_params.y;
    o.material = in.i_material;
    o.part_material = in.v_col.a;
    o.model_uv = vec3<f32>(in.v_uv.xy, in.v_uv.z);
    // Variation helps repeated props, but on one-instance-per-tile terrain it
    // exposes the mesh grid as a checkerboard. Ground gets continuous
    // world-space variation in the fragment shader instead.
    if (in.i_params.y >= 0.5) {
        o.tint = 1.0;
    } else {
        o.tint = 0.94 + fract(sin(dot(in.i_pos.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453) * 0.12;
    }
    // Offset along the normal to keep sloped faces off their own shadow.
    o.light_pos = U.light_view_proj * vec4<f32>(world + nrm * 0.045, 1.0);
    return o;
}

// ---------------------------------------------------------------- helpers

fn shadow_at(light_pos: vec4<f32>, ndl: f32) -> f32 {
    if (light_pos.w <= 0.0) {
        return 1.0;
    }
    // Zero taps means the shadow pass never ran and the map holds nothing.
    // SHADOW_TAPS is a build-time constant, so this whole function folds to a
    // constant 1.0 at the Performance preset.
    if (SHADOW_TAPS == 0) {
        return 1.0;
    }
    let proj = light_pos.xyz / light_pos.w;
    let uv = vec2<f32>(proj.x * 0.5 + 0.5, 0.5 - proj.y * 0.5);
    if (uv.x < 0.001 || uv.x > 0.999 || uv.y < 0.001 || uv.y > 0.999 || proj.z > 1.0) {
        return 1.0;
    }
    let bias = mix(0.0016, 0.0004, ndl);
    let t = U.misc.w;
    if (SHADOW_TAPS == 1) {
        return textureSampleCompareLevel(shadow_map, shadow_samp, uv, proj.z - bias);
    }
    // A nine tap box, wider than the old four. Four taps on a hard edge is a
    // stair; nine across a wider kernel is a soft edge, and soft edges are most
    // of what separates a lit scene from a diagram.
    var sum = 0.0;
    for (var i = -1; i <= 1; i = i + 1) {
        for (var j = -1; j <= 1; j = j + 1) {
            let o = vec2<f32>(f32(i), f32(j)) * t * 1.35;
            sum = sum + textureSampleCompareLevel(shadow_map, shadow_samp, uv + o, proj.z - bias);
        }
    }
    return sum / 9.0;
}

/// How much of the sky this point can see, approximated from the shadow map.
///
/// There is no depth prepass, so there is nothing to run a real screen-space
/// occlusion against; the only occluder geometry the frame has already
/// rasterised is the shadow map. So the ambient is attenuated by how much of a
/// wide neighbourhood around this point is in shadow. That buys the one thing
/// the frame was missing most: a dark skirt where a tower, a tuft or a monster
/// meets the ground. Without it every model sat on the field with a hard,
/// evenly lit seam and read as pasted on rather than standing there.
///
/// Where it is a lie, and it is worth being clear about it: this only knows
/// about occlusion along the sun's direction, so the skirt leans the way the
/// shadow leans instead of wrapping the whole base.
///
/// Tiered on `SHADOW_TAPS` exactly as the shadow filter is, so Performance -
/// which never renders a shadow map at all - folds the whole function to a
/// constant, Balanced pays two extra comparisons for a diagonal pair, and Ultra
/// pays four for the full ring.
fn sky_occlusion(light_pos: vec4<f32>) -> f32 {
    if (SHADOW_TAPS == 0 || light_pos.w <= 0.0) {
        return 1.0;
    }
    let proj = light_pos.xyz / light_pos.w;
    let uv = vec2<f32>(proj.x * 0.5 + 0.5, 0.5 - proj.y * 0.5);
    if (uv.x < 0.001 || uv.x > 0.999 || uv.y < 0.001 || uv.y > 0.999 || proj.z > 1.0) {
        return 1.0;
    }
    // Half a tile, held in UV rather than in texels so the skirt is the same
    // width on the 1024 map and the 2048 one. A radius in texels would have
    // made Balanced's contact shadow twice as wide as Ultra's, which is the
    // wrong way round and would have shown up as the preset changing the shape
    // of the scene rather than its fidelity.
    let r = 0.0034;
    // Deliberately blunt next to the shadow's own bias: this is asking whether
    // anything stands *near* the point, not whether the point is lit, and a
    // tight bias turns the answer into a second copy of the shadow.
    let z = proj.z - 0.0035;
    var sum = textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(r, r), z);
    sum = sum + textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(-r, -r), z);
    if (SHADOW_TAPS == 1) {
        return sum * 0.5;
    }
    sum = sum + textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(-r, r), z);
    sum = sum + textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(r, -r), z);
    return sum * 0.25;
}

// ---------------------------------------------------------------- BRDF

/// GGX / Trowbridge-Reitz normal distribution.
fn distribution_ggx(ndh: f32, rough: f32) -> f32 {
    let a = rough * rough;
    let a2 = a * a;
    let d = ndh * ndh * (a2 - 1.0) + 1.0;
    return a2 / max(3.14159265 * d * d, 1e-5);
}

/// Smith geometry term with the Schlick-GGX approximation, direct-light k.
fn geometry_smith(ndv: f32, ndl: f32, rough: f32) -> f32 {
    let r = rough + 1.0;
    let k = (r * r) / 8.0;
    let gv = ndv / (ndv * (1.0 - k) + k);
    let gl = ndl / (ndl * (1.0 - k) + k);
    return gv * gl;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

/// Roughness-aware Fresnel. Used to work out how much light is *left* for the
/// diffuse term after the environment has taken its specular share, so rough
/// surfaces do not lose energy they never reflected.
fn fresnel_roughness(cos_theta: f32, f0: vec3<f32>, rough: f32) -> vec3<f32> {
    let inv = vec3<f32>(1.0 - rough);
    return f0 + (max(inv, f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

/// The split-sum environment BRDF, Karis's analytic fit to the lookup table.
///
/// This replaces a hand-tuned `fresnel * (1 - roughness * 0.72)` fudge. The
/// fudge got the shape roughly right but scaled everything by one factor, so
/// polished metal reflected far too little of its surroundings and grass a
/// little too much - the exact reason the sun never showed on a blade edge and
/// the field had a faint sheen it should not have had. The real curve is four
/// more instructions and no texture.
fn env_brdf(f0: vec3<f32>, rough: f32, ndv: f32) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = vec4<f32>(rough) * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * ndv)) * r.x + r.y;
    return f0 * (a004 * -1.04 + r.z) + vec3<f32>(a004 * 1.04 + r.w);
}

// Material ids are authored into each baked vertex by `bake_models.py`.
// They deliberately describe real substance rather than a tower family tint:
// a ballista can therefore have limestone footings, oak beams, black iron
// pivots and bronze fittings in a single indexed draw.
fn part_layer(id: i32) -> i32 {
    switch id {
        case 1: { return 3; }  // limestone
        case 2: { return 2; }  // oiled oak
        case 3, 4, 8, 13, 14: { return 3; } // iron, bronze, plate
        case 9, 10: { return 4; } // leaf, moss: dark local woodland texture
        case 11: { return 1; } // heavy banner cloth
        case 12: { return 3; } // inset amber rune / eye
        case 15: { return 1; } // coarse packrunner fur / mane
        default: { return 1; } // hide, skin, chitin
    }
}

fn part_roughness(id: i32, fallback: f32) -> f32 {
    switch id {
        case 1: { return 0.91; } // limestone
        case 2: { return 0.72; } // oak
        case 3: { return 0.37; } // forged iron
        case 4: { return 0.43; } // worn bronze
        case 5: { return 0.83; } // hide
        case 6: { return 0.67; } // skin
        case 7: { return 0.36; } // chitin shell
        case 8: { return 0.62; } // weathered plate edge
        case 9: { return 0.88; } // leaf
        case 10: { return 0.95; } // moss
        case 11: { return 0.86; } // woven tabard cloth
        case 12: { return 0.34; } // polished amber inset
        case 13: { return 0.74; } // cold worn warplate
        case 14: { return 0.82; } // dull oxidised raider plate
        case 15: { return 0.96; } // coarse, matte brindled fur
        default: { return fallback; }
    }
}

fn part_metallic(id: i32, fallback: f32) -> f32 {
    switch id {
        // At full-board scale a mirror-like metal response turns every helmet
        // into the same sky-blue dot.  These remain physically distinct forged
        // metals, but retain enough diffuse albedo for black iron, bronze and
        // pale plate to read as separate constructed pieces in a moving horde.
        case 3: { return 0.56; } // forged black iron
        case 4: { return 0.58; } // worn bronze
        case 7: { return 0.22; } // glossy chitin, not mirror metal
        case 8: { return 0.28; } // weathered plated bone/iron edge
        case 12: { return 0.10; } // runic amber is a stone/glass inset
        case 13: { return 0.18; } // weathered forged warplate
        case 14: { return 0.12; } // scavenged oxidised plate
        case 15: { return 0.0; } // fur absorbs its light; it is not leathered metal
        case 1, 2, 5, 6, 9, 10, 11: { return 0.0; }
        default: { return fallback; }
    }
}

/// Tiny authored runes, eyes and sights are the only permanently luminous
/// material on a normal battlefield.  Their light is deliberately local: it
/// gives a horned brute a readable face and a constructed weapon a focal point
/// without painting cyan rings, every grass blade, or whole units with glow.
fn part_emission(id: i32) -> f32 {
    switch id {
        case 12: { return 0.72; } // RunicAmber
        default: { return 0.0; }
    }
}

fn part_detail_strength(id: i32) -> f32 {
    switch id {
        case 1: { return 0.28; }
        case 2: { return 0.42; }
        // Do not borrow a pale rock/dirt photograph for broad character
        // armour, chitin or cloth.  It was the direct cause of the white and
        // orange bands in a live horde: real source materials were present,
        // but a generic terrain texture overwhelmed them at tactical scale.
        // These retain a tiny grain response while construction, source UVs
        // and lighting decide what each physical part is.
        case 3, 4, 8, 13, 14: { return 0.045; }
        case 7: { return 0.050; }
        case 15: { return 0.030; }
        // Foliage carries real mesh silhouette and shadow.  A strong bright
        // grass texture over every tiny authored leaf was what made the live
        // tree crowns neon despite their dark source materials; keep only a
        // quiet physical variation on the actual leaf geometry.
        case 9, 10: { return 0.12; }
        case 11: { return 0.035; }
        case 12: { return 0.10; }
        default: { return 0.16; }
    }
}

// ---------------------------------------------------------------- fragment

@fragment
fn fs(o: VsOut) -> @location(0) vec4<f32> {
    // Every visible surface is textured. Terrain uses planar UVs; props, towers
    // and creatures use material-selected triplanar detail because the baked
    // browser mesh deliberately carries no UV/tangent stream. This keeps one
    // compact texture array and still gives vertical stone, wood, hide, leaves
    // and metal real grain instead of flat vertex colour.
    var albedo_tex = vec3<f32>(1.0);
    var n = normalize(o.nrm);
    // Derivatives must be evaluated before the material branch. Passing them
    // explicitly keeps mip filtering while satisfying WebGPU's uniform-control
    // rule on Chromium/Edge.
    // A terrain material repeats across a landscape, not once per logical
    // build tile. The former scale made the grass look like striped wallpaper.
    // The former 0.18 scale tiled the photo surface almost five times across
    // the compact board, producing a faint square repeat that looked like a
    // hidden grid.  At this world scale the same authored aggregate spans a
    // real clearing before it repeats, while normal relief still resolves at
    // tactical camera distance.
    // Mosswatch is an aggregate seen from a high tactical camera, not a
    // close-up tiling sample. A lower frequency keeps broad dark grass value
    // while letting real fern, root and rock meshes carry local relief.
    // Packed earth receives a smaller, denser repeat than the broad grass
    // substrate. It is a physical material at the edge of every footstep, not
    // a single flat brown ribbon; the grass remains deliberately lower
    // frequency so it does not turn into a tiled lawn.
    let terrain_layer_hint = i32(round(o.tex)) - 1;
    let terrain_scale = select(0.060, 0.115, terrain_layer_hint == 1);
    let ground_uv = o.world.xy * terrain_scale;
    let ground_uv_dx = dpdx(ground_uv);
    let ground_uv_dy = dpdy(ground_uv);
    let world_dx = dpdx(o.world);
    let world_dy = dpdy(o.world);
    // Like world-space derivatives, source-UV derivatives must be evaluated
    // before the non-uniform terrain/material branch below. Chromium's WebGPU
    // validator correctly rejects derivatives nested under a varying branch,
    // even when the eventual texture sample uses explicit gradients.
    let model_uv_dx = dpdx(o.model_uv.xy);
    let model_uv_dy = dpdy(o.model_uv.xy);
    let part_id = i32(round(o.part_material * 255.0));
    if (o.tex >= 0.5) {
        let layer = i32(round(o.tex)) - 1;
        // The terrain/material selector is a per-fragment value, so this branch
        // is non-uniform. WebGPU therefore forbids an implicit-derivative
        // textureSample here even though native backends accept it.
        albedo_tex = textureSampleGrad(
            ground_tex,
            ground_smp,
            ground_uv,
            layer,
            ground_uv_dx,
            ground_uv_dy,
        ).rgb;

        // The surface's shape, not just its colour.
        //
        // The ground is flat and axis-aligned, so its tangent frame is the
        // world's: +x is tangent, +y is bitangent, +z is up. That makes this a
        // three-line perturbation with no tangent basis to carry through the
        // vertex format - and it is the single largest difference between a
        // field that reads as ground and one that reads as painted card.
        let tn = textureSampleGrad(
            ground_tex,
            ground_smp,
            ground_uv,
            layer + GROUND_LAYERS,
            ground_uv_dx,
            ground_uv_dy,
        ).xyz * 2.0 - 1.0;
        // Mosswatch's source aggregate is intentionally rich; at a complete
        // tactical-board camera it must not turn into a shimmering green
        // carpet.  Physical ferns, roots and rocks provide the close relief,
        // while this remaining micro-normal only catches the daylight.
        // Mosswatch's derived micro-normal is intentionally gentler than dirt
        // at tactical height. Its discrete layer selector is flat-interpolated
        // above, so this response cannot leak into the preceding stone layer.
        let relief = select(GROUND_RELIEF, GROUND_RELIEF * 0.42, layer == 4);
        n = normalize(vec3<f32>(
            n.x + tn.x * relief,
            n.y + tn.y * relief,
            n.z,
        ));
        // The bake normalises each layer to a linear mean of one half, so twice
        // the sample averages to exactly one: the texture multiplies the tile's
        // measured colour without changing how bright it is on average. The mix
        // is how much grain to let through.
        // Let authored terrain carry enough material identity to read at
        // tactical scale, but keep the high-frequency photo reference from
        // becoming a tiled carpet. Packed dirt stays more pronounced than the
        // wide moss field; physical verge meshes supply the latter's close
        // silhouette instead of an ever-stronger albedo blend.
        let terrain_detail = select(
            // Grass is a broad substrate at this camera, not a full-screen
            // photo. Its real moss/grass aggregate needs enough material
            // response to avoid reading as flat green paint; rooted ferns,
            // rocks and trees still provide the close silhouette and depth.
            select(0.18, 0.48, layer == 1),
            // The authored Mosswatch field covers the tactical meadow at a
            // low world repeat, so it has room for this much real turf/pebble
            // response without becoming wallpaper.  At 0.15 the browser
            // capture flattened it into one green table and made the route
            // look pasted on; this restores material depth beneath the actual
            // grass, roots and contact shadows rather than adding a UI grid.
            0.30,
            layer == 4,
        );
        albedo_tex = mix(vec3<f32>(1.0), albedo_tex * 2.0, terrain_detail);
        if (layer == 0) {
            // Broad world-space variation, continuous over tile boundaries.
            // It keeps the field alive without photographic speckle or square
            // patches competing with units.
            let broad = sin(o.world.x * 0.23 + 0.8) * sin(o.world.y * 0.19 - 1.4);
            albedo_tex = albedo_tex * (0.96 + broad * 0.035);
        }
    } else {
        // Pick a physical texture from the material numbers carried by each
        // instance: grass for foliage, timber for wood, rock for masonry and
        // metal, packed earth for skin/chitin. `textureSampleGrad` keeps this
        // dynamic array selection valid on strict Chromium WebGPU backends.
        var layer = 1;
        var strength = 0.25;
        if (o.material.y > 0.10) {
            layer = 3;
            strength = 0.16;
        } else if (o.material.x > 0.945) {
            layer = 1;
            strength = 0.24;
        } else if (o.material.x > 0.885) {
            layer = 3;
            strength = 0.31;
        } else if (o.material.x > 0.815) {
            layer = 0;
            strength = 0.27;
        } else if (o.material.x > 0.745) {
            layer = 2;
            strength = 0.34;
        }

        // Mixed-material props can combine masonry and timber in one baked
        // draw. Their material colour survives per vertex, so brown beams can
        // receive wood grain while stone courses keep rock detail without
        // another binding or draw call.
        let timber = o.color.r > o.color.g * 1.24
            && o.color.g > o.color.b * 1.18;
        if (o.material.x > 0.885 && o.material.x < 0.945 && timber) {
            layer = 2;
            strength = 0.36;
        }

        // Sharpened triplanar weights avoid muddy diagonal faces. World-space
        // addressing also means adjacent wall pieces share their stone grain.
        var weights = pow(abs(n), vec3<f32>(5.0));
        weights = weights / max(weights.x + weights.y + weights.z, 1e-4);
        let detail_scale = select(1.72, 2.35, o.material.y > 0.10);
        let uv_x = o.world.yz * detail_scale;
        let uv_y = o.world.xz * detail_scale;
        let uv_z = o.world.xy * detail_scale;
        let sx = textureSampleGrad(
            ground_tex,
            ground_smp,
            uv_x,
            layer,
            world_dx.yz * detail_scale,
            world_dy.yz * detail_scale,
        ).rgb;
        let sy = textureSampleGrad(
            ground_tex,
            ground_smp,
            uv_y,
            layer,
            world_dx.xz * detail_scale,
            world_dy.xz * detail_scale,
        ).rgb;
        let sz = textureSampleGrad(
            ground_tex,
            ground_smp,
            uv_z,
            layer,
            world_dx.xy * detail_scale,
            world_dy.xy * detail_scale,
        ).rgb;
        let surface = sx * weights.x + sy * weights.y + sz * weights.z;
        albedo_tex = mix(vec3<f32>(1.0), surface * 2.0, strength);

        // A second, broad variation breaks up large tower walls and creature
        // bodies without erasing their authored vertex colours.
        let macro_tone = sin(o.world.x * 3.17 + o.world.z * 2.31)
            * sin(o.world.y * 2.73 - o.world.z * 1.91);
        albedo_tex = albedo_tex * (0.965 + macro_tone * 0.035);

        // Original Blender assets preserve their unwrap instead of relying on
        // the object-wide material heuristic above. A source UV gives timber
        // grain a continuous direction around a stock and keeps stone blocks
        // from acquiring arbitrary world-axis seams as the tower turns.
        if (part_id > 0 && o.model_uv.z > 0.5) {
            let layer = part_layer(part_id);
            let uv_scale = select(1.75, 2.40, part_id == 2);
            let uv = o.model_uv.xy * uv_scale;
            let uv_dx = model_uv_dx * uv_scale;
            let uv_dy = model_uv_dy * uv_scale;
            let authored = textureSampleGrad(
                ground_tex, ground_smp, uv, layer, uv_dx, uv_dy
            ).rgb;
            // Leaf volume already has real overlapping geometry, shadow and
            // material-separated source colour. Sampling the broad ground
            // aggregate across every tiny lobe made isolated bright stones
            // read as silver leaf faces in the live forest. Keep foliage
            // physical, but let its own construction carry the close detail
            // instead of borrowing an unrelated ground photograph.
            let authored_strength = select(
                part_detail_strength(part_id), 0.0, part_id == 9 || part_id == 10
            );
            albedo_tex = mix(vec3<f32>(1.0), authored * 2.0, authored_strength);
        }
    }

    let l = normalize(U.light_dir.xyz);
    let v = normalize(U.cam_pos.xyz - o.world);
    let h = normalize(l + v);

    let ndl = max(dot(n, l), 0.0);
    let ndv = max(dot(n, v), 1e-4);
    let ndh = max(dot(n, h), 0.0);
    let vdh = max(dot(v, h), 0.0);

    // Widen the specular lobe by however far the normal swings inside this one
    // pixel. A gem at roughness 0.14 on a sphere six pixels across has a
    // highlight narrower than a pixel, so it landed on a fragment or it did
    // not: the tower sparkled as the board turned, and the bloom pass smeared
    // that sparkle over half the model. Kaplanyan's filter, applied to the
    // alpha the GGX distribution actually uses.
    let dnx = dpdx(o.nrm);
    let dny = dpdy(o.nrm);
    let smear = min(0.5 * (dot(dnx, dnx) + dot(dny, dny)), 0.20);
    var mat_rough = clamp(part_roughness(part_id, o.material.x), 0.045, 1.0);
    let rough = min(sqrt(mat_rough * mat_rough + smear), 1.0);
    let metal = clamp(part_metallic(part_id, o.material.y), 0.0, 1.0);

    var albedo = o.color.rgb * o.tint * albedo_tex;
    if (o.tex < 0.5) {
        // Tiny models lose authored material separation faster than terrain
        // does. Lift their midtones and colour contrast before lighting so an
        // orc stays green and leather stays brown instead of both becoming
        // grey silhouettes at tactical zoom.
        // Small real models otherwise spend too much of their available
        // dynamic range below the filmic toe. This is a material-space
        // midtone lift, so it preserves highlights/shadows and separate
        // stone, wood, metal and hide responses rather than painting a UI
        // brightness layer over the battlefield.
        let organic_foliage = part_id == 9 || part_id == 10;
        // The general small-object lift makes armour and construction visible
        // through the filmic toe.  Applying it to already-lit leaf planes
        // lifts green much more than red and turns a physical forest into
        // fluorescent toy crowns, so foliage stays closer to its authored
        // albedo while retaining directional light and cast shadow.
        let lift_exp = select(0.70, 0.96, organic_foliage);
        albedo = pow(max(albedo, vec3<f32>(0.0)), vec3<f32>(lift_exp));
        let object_lum = dot(albedo, vec3<f32>(0.2126, 0.7152, 0.0722));
        if (!organic_foliage) {
            albedo = mix(vec3<f32>(object_lum), albedo, 1.14);
        }
    }

    // Dielectrics reflect ~4%; metals reflect their own colour.  Broad leaves
    // are rough, absorbent plant tissue rather than clear-coated plastic: at
    // this oblique whole-map camera their physically tiny Fresnel reflection
    // otherwise becomes a bright blue-white outline on every crown lobe.
    let foliage_surface = part_id == 9 || part_id == 10;
    let f0_base = mix(vec3<f32>(0.04), albedo, metal);
    let f0 = select(f0_base, vec3<f32>(0.015), foliage_surface);

    // --- key light
    var shade = 1.0;
    if (ndl > 0.0) {
        shade = shadow_at(o.light_pos, ndl);
    }
    // Midday, warm. Warcraft III lights its outdoor tilesets with a strong
    // near-white key and lets the terrain's own colour carry the scene; the key
    // light is the only term here that carries a surface's albedo, so it has to
    // beat the ambient rather than lose to it.
    let sun = vec3<f32>(1.00, 0.96, 0.88) * 1.90;
    let d = distribution_ggx(ndh, rough);
    let g = geometry_smith(ndv, ndl, rough);
    let f = fresnel_schlick(vdh, f0);
    let spec = (d * g) * f / max(4.0 * ndv * ndl, 1e-4);
    let kd = (vec3<f32>(1.0) - f) * (1.0 - metal);
    let direct = (kd * albedo / 3.14159265 + spec) * sun * ndl * shade;

    // --- ambient: sky above, bounce below, standing in for an IBL probe
    //
    // Daylight: a blue sky overhead and green bounce off the field, which is
    // what a grass level actually looks like and what makes a tower's shaded
    // side read as *in shadow on grass* rather than as grey.
    // Give shaded bark, armour and road shoulders enough sky/bounce to retain
    // their material separation in a browser's filmic output. Direct sun and
    // shadow remain unchanged, so this is a lifted daylight fill rather than
    // flat unshadowed colour.
    let sky = vec3<f32>(0.42, 0.54, 0.74) * 0.52;
    let ground = vec3<f32>(0.24, 0.28, 0.18) * 0.50;
    let irradiance = mix(ground, sky, n.z * 0.5 + 0.5);
    let fa = fresnel_roughness(ndv, f0, rough);
    let kda = (vec3<f32>(1.0) - fa) * (1.0 - metal);
    // The probe is read along the reflection vector, not along the normal, so a
    // metal face tilted up catches sky and one tilted down catches the field.
    // Reading it at the normal gave every face of a cube the same reflection,
    // which is most of why polished metal came out looking like painted
    // plastic; a rough surface scatters the lobe back towards the normal, which
    // is what the mix by roughness does.
    let refl = reflect(-v, n);
    let mirror = mix(ground, sky, clamp(refl.z * 0.5 + 0.5, 0.0, 1.0));
    let amb_spec_base = mix(mirror, irradiance, rough * rough) * env_brdf(f0, rough, ndv);
    let amb_spec = select(amb_spec_base, vec3<f32>(0.0), foliage_surface);

    // A wide Fresnel in the sky's own colour along the silhouette. Looking
    // almost straight down at a green field, a unit standing on it is within a
    // few percent of the grass in value and reads as a flat shape cut out of
    // the ground; this is the cheapest thing that puts an edge back.
    //
    // Weighted towards upright surfaces because the ground is one enormous
    // plane which grazes the camera near the top of the frame - an ungated rim
    // paints a bright band right across the far half of the field, and the
    // field's colour is measured against a screenshot and may not move.
    let rim = pow(1.0 - ndv, 3.0) * (1.0 - abs(n.z));

    // Occlusion belongs on the ambient only: the key light already has a shadow
    // and darkening it twice turns every shaded face to mud.
    let ao = mix(1.0, sky_occlusion(o.light_pos), 0.60);
    let rim_strength = select(0.45, 0.08, foliage_surface);
    let ambient = (kda * albedo * irradiance + amb_spec + sky * rim * rim_strength) * ao;

    var col = direct + ambient;
    // Emissive parts ignore lighting entirely - cores, runes and the handful
    // of authored amber eyes/sights.  The per-instance term is still used for
    // transient shots and spell effects; material emission makes real source
    // construction survive the bake instead of requiring a whole-object tint.
    let source_emission = part_emission(part_id);
    col = mix(col, o.color.rgb * 1.72, clamp(max(o.emissive, source_emission), 0.0, 1.0));

    // Keep atmosphere outside the tactical focus, not between a phone's
    // farther fit camera and its own board. Camera-distance fog made the
    // complete 390px overview wash into blue-grey while the same tiles stayed
    // readable on desktop. A board-centred falloff preserves a light haze in
    // the outer woodland, yet leaves the playable 24x24 field clear at every
    // aspect ratio and zoom.
    let focus_distance = length(o.world.xy - vec2<f32>(12.0, 12.0));
    let fog_amount = 1.0 - exp(-max(focus_distance - 16.0, 0.0) * U.fog.a);
    col = mix(col, U.fog.rgb, clamp(fog_amount, 0.0, 0.85));

    return vec4<f32>(col, o.color.a);
}
