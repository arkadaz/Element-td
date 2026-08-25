// Physically based shading for the instanced shape library.
//
// Cook-Torrance GGX with a shadow-mapped key light, a hemisphere ambient term
// standing in for image-based lighting, and a cheap horizon-occlusion specular
// ambient. Materials come in per instance as (roughness, metallic), so stone,
// wood, foliage, polished metal and gems all respond differently to the same
// light instead of looking like tinted plastic.

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
    // Offset along the normal to keep sloped faces off their own shadow.
    o.light_pos = U.light_view_proj * vec4<f32>(world + nrm * 0.045, 1.0);
    return o;
}

// ---------------------------------------------------------------- helpers

fn hash21(p: vec2<f32>) -> f32 {
    var h = fract(p * vec2<f32>(0.1031, 0.1030));
    h = h + dot(h, h.yx + 33.33);
    return fract((h.x + h.y) * h.x);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn shadow_at(light_pos: vec4<f32>, ndl: f32) -> f32 {
    if (light_pos.w <= 0.0) {
        return 1.0;
    }
    let proj = light_pos.xyz / light_pos.w;
    let uv = vec2<f32>(proj.x * 0.5 + 0.5, 0.5 - proj.y * 0.5);
    if (uv.x < 0.001 || uv.x > 0.999 || uv.y < 0.001 || uv.y > 0.999 || proj.z > 1.0) {
        return 1.0;
    }
    let bias = mix(0.0016, 0.0004, ndl);
    let t = U.misc.w;
    var sum = 0.0;
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(-0.7, -0.7) * t, proj.z - bias);
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(0.7, -0.7) * t, proj.z - bias);
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(-0.7, 0.7) * t, proj.z - bias);
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(0.7, 0.7) * t, proj.z - bias);
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

/// Roughness-aware Fresnel, used for the ambient specular so rough surfaces do
/// not pick up a rim of environment reflection they should not have.
fn fresnel_roughness(cos_theta: f32, f0: vec3<f32>, rough: f32) -> vec3<f32> {
    let inv = vec3<f32>(1.0 - rough);
    return f0 + (max(inv, f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
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

    let rough = clamp(o.material.x, 0.045, 1.0);
    let metal = clamp(o.material.y, 0.0, 1.0);

    var albedo = o.color.rgb;
    // Surface break-up so flat faces are not uniform, projected on whichever
    // axis the face points along.
    let uv = select(
        select(o.world.yz, o.world.xz, abs(n.y) > 0.5),
        o.world.xy,
        abs(n.z) > 0.5,
    );
    albedo = albedo * (0.94 + noise2(uv * 2.7) * 0.13);

    // Dielectrics reflect ~4%; metals reflect their own colour.
    let f0 = mix(vec3<f32>(0.04), albedo, metal);

    // --- key light
    var shade = 1.0;
    if (ndl > 0.0) {
        shade = shadow_at(o.light_pos, ndl);
    }
    let sun = vec3<f32>(1.00, 0.945, 0.86) * 3.1;
    let d = distribution_ggx(ndh, rough);
    let g = geometry_smith(ndv, ndl, rough);
    let f = fresnel_schlick(vdh, f0);
    let spec = (d * g) * f / max(4.0 * ndv * ndl, 1e-4);
    let kd = (vec3<f32>(1.0) - f) * (1.0 - metal);
    let direct = (kd * albedo / 3.14159265 + spec) * sun * ndl * shade;

    // --- ambient: sky above, warm bounce below, standing in for an IBL probe
    let sky = vec3<f32>(0.33, 0.45, 0.66) * 1.15;
    let ground = vec3<f32>(0.20, 0.16, 0.13);
    let irradiance = mix(ground, sky, n.z * 0.5 + 0.5);
    let fa = fresnel_roughness(ndv, f0, rough);
    let kda = (vec3<f32>(1.0) - fa) * (1.0 - metal);
    // A rough surface scatters the environment; a smooth one mirrors it.
    let amb_spec = mix(sky, irradiance, rough) * fa * (1.0 - rough * 0.72);
    let ambient = kda * albedo * irradiance + amb_spec;

    var col = direct + ambient;
    // Emissive parts ignore lighting entirely - cores, runes, flames.
    col = mix(col, o.color.rgb * 2.2, clamp(o.emissive, 0.0, 1.0));

    // Distance fog, so the far edge of the board recedes.
    let dist = length(U.cam_pos.xyz - o.world);
    let fog_amount = 1.0 - exp(-max(dist - 26.0, 0.0) * U.fog.a);
    col = mix(col, U.fog.rgb, clamp(fog_amount, 0.0, 0.85));

    return vec4<f32>(col, o.color.a);
}
