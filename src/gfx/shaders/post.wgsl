// Bloom chain and final composite. Three fragment entry points share one
// fullscreen-triangle vertex shader.

struct PostU {
    dir: vec2<f32>,
    texel: vec2<f32>,
    // x: bright threshold, y: bloom strength, z: encode sRGB, w: Ultra detail
    params: vec4<f32>,
    // xy: where the key light's bearing lands, z: wrapped time, w: unused
    sun: vec4<f32>,
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

/// What is left of a colour once the bright threshold has taken its share.
///
/// The knee is a quadratic ramp over the half below the threshold. A hard cut
/// makes anything hovering either side of the line pop in and out as the board
/// turns, and a tower whose gem is on the edge of blooming flickers.
fn over_threshold(c: vec3<f32>, thr: f32) -> vec3<f32> {
    let l = max(max(c.r, c.g), c.b);
    let knee = thr * 0.5;
    let soft = clamp(l - thr + knee, 0.0, 2.0 * knee);
    let k = max(soft * soft / max(4.0 * knee, 1e-4), l - thr) / max(l, 1e-4);
    return c * max(k, 0.0);
}

@fragment
fn fs_bright(o: VsOut) -> @location(0) vec4<f32> {
    // Four bilinear taps at the quadrant centres of this pixel's footprint,
    // thresholded before they are averaged rather than after. One tap read four
    // of the sixteen full-resolution pixels a quarter-resolution one stands
    // for, so a gem three pixels across bloomed or did not depending on where
    // it happened to fall in the grid - which on a moving board reads as the
    // gem flickering. Thresholding first keeps a small hot accent hot instead
    // of averaging it down into its dark neighbours.
    let h = P.texel * 0.25;
    let thr = P.params.x;
    var acc = over_threshold(textureSample(tex0, samp, o.uv + vec2<f32>(h.x, h.y)).rgb, thr);
    acc = acc + over_threshold(textureSample(tex0, samp, o.uv + vec2<f32>(-h.x, h.y)).rgb, thr);
    acc = acc + over_threshold(textureSample(tex0, samp, o.uv + vec2<f32>(h.x, -h.y)).rgb, thr);
    acc = acc + over_threshold(textureSample(tex0, samp, o.uv + vec2<f32>(-h.x, -h.y)).rgb, thr);
    // Glows and particles are drawn into this same buffer above 1.0, so they
    // clear the threshold on their own and need no second texture.
    return vec4<f32>(acc * 0.25, 1.0);
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

fn hash12(p: vec2<f32>) -> f32 {
    // Screen-space film grain. Its amplitude is deliberately sub-visible in a
    // still frame; in motion it breaks perfectly smooth gradients and stops
    // the tone-mapped fog from banding on an 8-bit browser swapchain.
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h + P.sun.z * 41.37) * 43758.5453);
}

/// Two sheared sine layers. One sine reads as a test card; shearing it against
/// x and beating it with a second, faster and weaker one breaks the repeat up
/// enough that the eye takes it for banded haze instead of counting the bands.
fn haze(uv: vec2<f32>) -> f32 {
    let a = sin(uv.y * 26.0 - uv.x * 3.1);
    let b = sin(uv.y * 61.0 + uv.x * 5.7 + 1.7);
    return a * 0.64 + b * 0.36;
}

/// Backdrop behind the board: a soft vertical gradient with a warm horizon,
/// banded haze, and warmth from the side the key light arrives on.
///
/// There is deliberately no sun disc here, and no cloud in the sense of a shape
/// with an edge. The camera sits at fifty-two degrees of pitch over a
/// twenty-one degree half-angle, so the top of the frame is still thirty
/// degrees *below* the horizon: every pixel this function paints is ground that
/// the finite board simply does not reach, and the sun is behind the viewer's
/// shoulder. A disc would be a light source drawn underground and a cloud would
/// be a cloud buried in a hillside. What the backdrop can honestly carry is
/// depth and the light's bearing, so that is what it carries.
fn sky(uv: vec2<f32>) -> vec3<f32> {
    // The tactical camera looks down beyond a finite forest plateau. A pale
    // blue clear made that empty space read as a missing level; deep woodland
    // haze lets the playable field remain the brightest readable surface.
    let top = vec3<f32>(0.025, 0.050, 0.070);
    let bottom = vec3<f32>(0.075, 0.125, 0.115);
    var c = mix(top, bottom, smoothstep(0.0, 1.0, uv.y));
    // Warm haze sitting on the horizon line, where the field meets the sky.
    let halo = exp(-pow((uv.y - 0.46) * 3.0, 2.0)) * 0.20;
    c = c + vec3<f32>(0.18, 0.16, 0.10) * halo;
    // Strata. The window is where the backdrop is actually seen: the board is
    // finite and fills the middle of the frame, so all that ever shows of this
    // is the top fifth and the two upper corners. Bands outside that are bands
    // nobody looks at, paid for on every pixel of the composite.
    let strata = smoothstep(0.0, 0.06, uv.y) * (1.0 - smoothstep(0.26, 0.52, uv.y));
    c = c + vec3<f32>(0.022, 0.026, 0.032) * haze(uv) * strata;
    // The light's bearing, so the backdrop brightens on the same side the
    // towers are lit from rather than being lit from nowhere.
    let d = (uv - P.sun.xy) / vec2<f32>(0.45, 0.40);
    c = c + vec3<f32>(0.12, 0.10, 0.05) * exp(-dot(d, d)) * 0.20;
    return c;
}

@fragment
fn fs_composite(o: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSample(tex0, samp, o.uv);
    var scene_rgb = scene.rgb;
    if (P.params.w > 0.5 && scene.a > 0.0) {
        // A restrained unsharp mask recovers the small model facets softened by
        // MSAA and the browser's final canvas scaling. This is the kind of
        // micro-contrast that makes metal edges and creature silhouettes feel
        // materially richer without an extra geometry or post-processing pass.
        let x = vec2<f32>(P.texel.x, 0.0);
        let y = vec2<f32>(0.0, P.texel.y);
        let neighbours = textureSampleLevel(tex0, samp, o.uv + x, 0.0).rgb
            + textureSampleLevel(tex0, samp, o.uv - x, 0.0).rgb
            + textureSampleLevel(tex0, samp, o.uv + y, 0.0).rgb
            + textureSampleLevel(tex0, samp, o.uv - y, 0.0).rgb;
        let detail = scene.rgb - neighbours * 0.25;
        scene_rgb = max(scene.rgb + detail * 0.16, vec3<f32>(0.0));
    }
    // Anything the scene pass did not cover shows the sky.
    var c = mix(sky(o.uv), scene_rgb, clamp(scene.a, 0.0, 1.0));
    let b = textureSample(tex1, samp, o.uv).rgb;
    c = c + b * P.params.y;

    // Exposure, then a filmic curve.
    // Actual browser captures showed the physically shaded field one stop
    // below readable once WebGPU's canvas compositor and the filmic toe both
    // applied. This modest exposure lift keeps the woodland dark, while
    // letting built stone, armor and road shoulders retain visible planes.
    c = aces(c * 1.28);
    // Grade: cool the shadows a touch, warm the highlights, then lift the
    // saturation. The battlefield already carries strong faction colours; the
    // grade only restores what the filmic shoulder removes instead of pushing
    // grass and spell effects into neon.
    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(c * vec3<f32>(0.97, 0.99, 1.04), c * vec3<f32>(1.04, 1.00, 0.96), lum);
    c = mix(vec3<f32>(lum), c, 1.10);
    c = (c - vec3<f32>(0.5)) * 1.035 + vec3<f32>(0.5);

    let vignette_uv = (o.uv - vec2<f32>(0.5)) * vec2<f32>(0.92, 1.08);
    let d = length(vignette_uv);
    c = c * (1.0 - smoothstep(0.48, 0.82, d) * 0.10);

    if (P.params.w > 0.5) {
        let grain = hash12(o.uv * vec2<f32>(1920.0, 1080.0)) - 0.5;
        c = c + vec3<f32>(grain * 0.0065);
    }

    if (P.params.z > 0.5) {
        c = srgb_encode(c);
    }
    return vec4<f32>(c, 1.0);
}
