// Every solid thing on the board is this one unit cube, instanced.
// Shading is a full lighting model: hemisphere ambient, a shadow-mapped key
// light with PCF, a rim term, specular, procedural surface break-up and
// distance fog.

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
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) nrm: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) emissive: f32,
    @location(4) light_pos: vec4<f32>,
};

// Yaw about Z, then pitch tilting the local +X axis up towards +Z.
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
    let nrm = normalize(r * in.v_nrm);

    var o: VsOut;
    o.clip = U.view_proj * vec4<f32>(world, 1.0);
    o.nrm = nrm;
    o.world = world;
    o.color = in.i_color;
    o.emissive = in.i_params.x;
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

// Value noise: enough to break up flat faces without looking like a pattern.
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

/// Percentage-closer filtering, 3x3.
fn shadow_at(light_pos: vec4<f32>, ndl: f32) -> f32 {
    if (light_pos.w <= 0.0) {
        return 1.0;
    }
    let proj = light_pos.xyz / light_pos.w;
    let uv = vec2<f32>(proj.x * 0.5 + 0.5, 0.5 - proj.y * 0.5);
    if (uv.x < 0.001 || uv.x > 0.999 || uv.y < 0.001 || uv.y > 0.999 || proj.z > 1.0) {
        return 1.0;
    }
    // Steeper surfaces need more bias or they self-shadow into stripes.
    let bias = mix(0.0016, 0.0004, ndl);
    let texel = U.misc.w;

    // Four rotated taps. The comparison sampler already does hardware bilinear
    // PCF, so this is plenty soft at this shadow density and costs less than half
    // of a 3x3 kernel.
    var sum = 0.0;
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(-0.7, -0.7) * texel, proj.z - bias);
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(0.7, -0.7) * texel, proj.z - bias);
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(-0.7, 0.7) * texel, proj.z - bias);
    sum = sum + textureSampleCompare(shadow_map, shadow_samp, uv + vec2<f32>(0.7, 0.7) * texel, proj.z - bias);
    return sum * 0.25;
}

// ---------------------------------------------------------------- fragment

@fragment
fn fs(o: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(o.nrm);
    let l = normalize(U.light_dir.xyz);
    let v = normalize(U.cam_pos.xyz - o.world);
    let ndl = max(dot(n, l), 0.0);

    var albedo = o.color.rgb;

    // Surface break-up, projected on whichever axis the face points along.
    // Flat unlit colour is the main thing that reads as "untextured".
    let uv = select(
        select(o.world.yz, o.world.xz, abs(n.y) > 0.5),
        o.world.xy,
        abs(n.z) > 0.5,
    );
    let grain = noise2(uv * 2.7) * 0.13;
    albedo = albedo * (0.94 + grain);

    // Hemisphere ambient: sky above, warm bounce from below.
    let sky = vec3<f32>(0.34, 0.44, 0.62);
    let ground = vec3<f32>(0.16, 0.14, 0.12);
    let hemi = mix(ground, sky, n.z * 0.5 + 0.5);

    // A face turned away from the light is already dark; sampling the shadow map
    // for it would change nothing.
    var shade = 1.0;
    if (ndl > 0.0) {
        shade = shadow_at(o.light_pos, ndl);
    }
    let key = vec3<f32>(1.00, 0.94, 0.82) * ndl * shade * 1.05;

    // Specular only where the light actually reaches.
    let h = normalize(l + v);
    let spec = pow(max(dot(n, h), 0.0), 42.0) * 0.35 * shade;
    // Rim light lifts silhouettes off the background.
    let rim = pow(1.0 - max(dot(n, v), 0.0), 3.2) * 0.30;

    var col = albedo * (hemi * 0.55 + key) + vec3<f32>(spec) + albedo * rim;
    col = mix(col, o.color.rgb * 2.0, clamp(o.emissive, 0.0, 1.0));

    // Distance fog, so the far edge of the board recedes.
    let d = length(U.cam_pos.xyz - o.world);
    let f = 1.0 - exp(-max(d - 26.0, 0.0) * U.fog.a);
    col = mix(col, U.fog.rgb, clamp(f, 0.0, 0.85));

    return vec4<f32>(col, o.color.a);
}
