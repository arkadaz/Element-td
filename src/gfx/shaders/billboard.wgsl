// Camera-facing sprites: additive glows, and particles whose entire motion is
// solved here from spawn state - the CPU never touches a live particle again.

struct Uniforms {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_pos: vec4<f32>,
    light_dir: vec4<f32>,
    misc: vec4<f32>,        // x = drag, y = gravity, z = time
    fog: vec4<f32>,
};
@group(0) @binding(0) var<uniform> U: Uniforms;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) power: f32,
};

// Unit quad corners from the vertex index (triangle strip).
fn corner(i: u32) -> vec2<f32> {
    let x = select(-1.0, 1.0, (i & 1u) == 1u);
    let y = select(-1.0, 1.0, (i & 2u) == 2u);
    return vec2<f32>(x, y);
}

fn face_camera(centre: vec3<f32>, q: vec2<f32>, radius: f32) -> vec4<f32> {
    let world = centre + U.cam_right.xyz * (q.x * radius) + U.cam_up.xyz * (q.y * radius);
    return U.view_proj * vec4<f32>(world, 1.0);
}

// ---------------------------------------------------------------- glows

struct GlowIn {
    @location(0) i_pos: vec3<f32>,
    @location(1) i_scale: vec3<f32>,
    @location(2) i_rot: vec2<f32>,
    @location(3) i_params: vec2<f32>,
    @location(4) i_color: vec4<f32>,
};

@vertex
fn vs_glow(in: GlowIn, @builtin(vertex_index) vi: u32) -> Out {
    let q = corner(vi);
    var o: Out;
    o.clip = face_camera(in.i_pos, q, in.i_scale.x);
    o.local = q;
    o.color = in.i_color;
    o.power = max(in.i_params.x, 0.05);
    return o;
}

@fragment
fn fs_glow(o: Out) -> @location(0) vec4<f32> {
    let d = clamp(1.0 - length(o.local), 0.0, 1.0);
    let a = pow(d, o.power) * o.color.a;
    if (a <= 0.002) {
        discard;
    }
    return vec4<f32>(o.color.rgb, a);
}

// ---------------------------------------------------------------- particles

struct PartIn {
    @location(0) p0: vec3<f32>,
    @location(1) vel: vec3<f32>,
    @location(2) t0_life: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) color: vec4<f32>,
};

@vertex
fn vs_part(in: PartIn, @builtin(vertex_index) vi: u32) -> Out {
    var o: Out;
    let life = in.t0_life.y;
    let t = U.misc.z - in.t0_life.x;

    // Dead or not yet born: collapse off-screen so it costs nothing to raster.
    if (life <= 0.0 || t < 0.0 || t > life) {
        o.clip = vec4<f32>(-10.0, -10.0, 0.0, 1.0);
        o.local = vec2<f32>(0.0, 0.0);
        o.color = vec4<f32>(0.0);
        o.power = 1.0;
        return o;
    }

    // Closed-form integral of velocity under linear drag, plus gravity.
    let k = max(U.misc.x, 0.001);
    let travel = (1.0 - exp(-k * t)) / k;
    var p = in.p0 + in.vel * travel;
    p.z = p.z + 0.5 * U.misc.y * t * t;
    // Never sink through the ground.
    p.z = max(p.z, 0.02);

    let u = t / life;
    let radius = mix(in.size.x, in.size.y, u);
    let q = corner(vi);

    o.clip = face_camera(p, q, radius);
    o.local = q;
    let fade = (1.0 - u) * (1.0 - u);
    o.color = vec4<f32>(in.color.rgb, in.color.a * fade);
    o.power = 1.0;
    return o;
}

@fragment
fn fs_part(o: Out) -> @location(0) vec4<f32> {
    let d = length(o.local);
    let a = exp(-d * d * 3.6) * o.color.a;
    if (a <= 0.002) {
        discard;
    }
    // Hot core: pushing above 1.0 is what makes the bloom pass catch it.
    return vec4<f32>(o.color.rgb * 1.7, a);
}
