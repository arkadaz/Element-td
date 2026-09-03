// Bloom chain and final composite. Three fragment entry points share one
// fullscreen-triangle vertex shader.

struct PostU {
    dir: vec2<f32>,
    texel: vec2<f32>,
    // x: bright threshold, y: bloom strength, z: encode sRGB, w: unused
    params: vec4<f32>,
};

@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var tex0: texture_2d<f32>;
@group(0) @binding(2) var tex1: texture_2d<f32>;
@group(0) @binding(3) var<uniform> P: PostU;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> VsOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = corners[i];
    var o: VsOut;
    o.clip = vec4<f32>(p, 0.0, 1.0);
    o.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return o;
}

@fragment
fn fs_bright(o: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex0, samp, o.uv).rgb;
    let l = max(max(c.r, c.g), c.b);
    let k = max(l - P.params.x, 0.0) / max(l, 1e-4);
    // Glows and particles are drawn into this same buffer above 1.0, so they
    // clear the threshold on their own and need no second texture.
    return vec4<f32>(c * k, 1.0);
}

@fragment
fn fs_blur(o: VsOut) -> @location(0) vec4<f32> {
    var w = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    var acc = textureSample(tex0, samp, o.uv).rgb * w[0];
    for (var i = 1; i < 5; i = i + 1) {
        let off = P.dir * f32(i) * 1.35;
        acc = acc + textureSample(tex0, samp, o.uv + off).rgb * w[i];
        acc = acc + textureSample(tex0, samp, o.uv - off).rgb * w[i];
    }
    return vec4<f32>(acc, 1.0);
}

/// ACES filmic curve (Narkowicz fit). Rolls highlights off to white the way a
/// film response does instead of clipping each channel independently.
fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

/// Backdrop behind the board: a soft vertical gradient with a warm horizon.
fn sky(uv: vec2<f32>) -> vec3<f32> {
    let top = vec3<f32>(0.026, 0.036, 0.075);
    let bottom = vec3<f32>(0.075, 0.085, 0.125);
    var c = mix(top, bottom, smoothstep(0.0, 1.0, uv.y));
    // Faint glow rising behind the horizon line.
    let halo = exp(-pow((uv.y - 0.42) * 3.4, 2.0)) * 0.055;
    c = c + vec3<f32>(0.22, 0.30, 0.45) * halo;
    return c;
}

@fragment
fn fs_composite(o: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSample(tex0, samp, o.uv);
    // Anything the scene pass did not cover shows the sky.
    var c = mix(sky(o.uv), scene.rgb, clamp(scene.a, 0.0, 1.0));
    let b = textureSample(tex1, samp, o.uv).rgb;
    c = c + b * P.params.y;

    // Exposure, then a filmic curve.
    c = aces(c * 0.90);
    // Gentle grade: cool the shadows, warm the highlights. Deliberately gentle -
    // at (0.94, 0.97, 1.08) the shadow tint was pushing blue up by 8% across
    // almost the whole frame, on top of an ambient that was already too blue.
    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(c * vec3<f32>(0.98, 0.99, 1.03), c * vec3<f32>(1.03, 1.00, 0.98), lum);

    let d = distance(o.uv, vec2<f32>(0.5, 0.5));
    c = c * (1.0 - smoothstep(0.58, 1.10, d) * 0.5);

    if (P.params.z > 0.5) {
        c = srgb_encode(c);
    }
    return vec4<f32>(c, 1.0);
}
