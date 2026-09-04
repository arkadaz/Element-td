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
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;

/// Shadow filter tier, overridden per quality preset when the shader is built.
/// It names a filter rather than counting taps: 0 skips the lookup outright, 1
/// takes a single comparison, and 4 takes the nine-tap box. So the cheap preset
/// pays one comparison per pixel where the dear one pays nine.
const SHADOW_TAPS: i32 = 4;

struct VsIn {
    @location(0) v_pos: vec3<f32>,
    @location(1) v_nrm: vec3<f32>,
    @location(2) i_pos: vec3<f32>,
    @location(3) i_scale: vec3<f32>,
    @location(4) i_rot: vec2<f32>,
    @location(5) i_params: vec2<f32>,
    @location(6) i_color: vec4<f32>,
    @location(7) i_material: vec2<f32>,
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
    let world = in.i_pos + r * (in.v_pos * in.i_scale);
    // Non-uniform scale needs the inverse-transpose; for an axis-aligned scale
    // that is just dividing by the scale.
    let inv = vec3<f32>(1.0) / max(in.i_scale, vec3<f32>(1e-4));
    let nrm = normalize(r * (in.v_nrm * inv));

    var o: VsOut;
    o.clip = U.view_proj * vec4<f32>(world, 1.0);
    o.nrm = nrm;
    o.world = world;
    o.color = in.i_color;
    o.emissive = in.i_params.x;
    o.material = in.i_material;
    o.tint = 0.94 + fract(sin(dot(in.i_pos.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453) * 0.12;
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
        return textureSampleCompare(shadow_map, shadow_samp, uv, proj.z - bias);
    }
    // A nine tap box, wider than the old four. Four taps on a hard edge is a
    // stair; nine across a wider kernel is a soft edge, and soft edges are most
    // of what separates a lit scene from a diagram.
    var sum = 0.0;
    for (var i = -1; i <= 1; i = i + 1) {
        for (var j = -1; j <= 1; j = j + 1) {
            let o = vec2<f32>(f32(i), f32(j)) * t * 1.35;
            sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + o, proj.z - bias);
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
    var sum = textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(r, r), z);
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(-r, -r), z);
    if (SHADOW_TAPS == 1) {
        return sum * 0.5;
    }
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(-r, r), z);
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(r, -r), z);
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

// ---------------------------------------------------------------- fragment

@fragment
fn fs(o: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(o.nrm);
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
    let mat_rough = clamp(o.material.x, 0.045, 1.0);
    let rough = min(sqrt(mat_rough * mat_rough + smear), 1.0);
    let metal = clamp(o.material.y, 0.0, 1.0);

    let albedo = o.color.rgb * o.tint;

    // Dielectrics reflect ~4%; metals reflect their own colour.
    let f0 = mix(vec3<f32>(0.04), albedo, metal);

    // --- key light
    var shade = 1.0;
    if (ndl > 0.0) {
        shade = shadow_at(o.light_pos, ndl);
    }
    // Midday, warm. Warcraft III lights its outdoor tilesets with a strong
    // near-white key and lets the terrain's own colour carry the scene; the key
    // light is the only term here that carries a surface's albedo, so it has to
    // beat the ambient rather than lose to it.
    let sun = vec3<f32>(1.00, 0.96, 0.88) * 2.25;
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
    let sky = vec3<f32>(0.42, 0.54, 0.74) * 0.40;
    let ground = vec3<f32>(0.26, 0.30, 0.16) * 0.40;
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
    let amb_spec = mix(mirror, irradiance, rough * rough) * env_brdf(f0, rough, ndv);

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
    let ambient = (kda * albedo * irradiance + amb_spec + sky * rim * 0.45) * ao;

    var col = direct + ambient;
    // Emissive parts ignore lighting entirely - cores, runes, flames.
    col = mix(col, o.color.rgb * 2.2, clamp(o.emissive, 0.0, 1.0));

    // Distance fog, so the far edge of the board recedes.
    let dist = length(U.cam_pos.xyz - o.world);
    let fog_amount = 1.0 - exp(-max(dist - 44.0, 0.0) * U.fog.a);
    col = mix(col, U.fog.rgb, clamp(fog_amount, 0.0, 0.85));

    return vec4<f32>(col, o.color.a);
}
