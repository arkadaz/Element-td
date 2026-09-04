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
    // Particles: x = normalised age, y = random rotation seed.
    @location(3) aux: vec2<f32>,
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

fn face_camera_offset(centre: vec3<f32>, q: vec2<f32>) -> vec4<f32> {
    let world = centre + U.cam_right.xyz * q.x + U.cam_up.xyz * q.y;
    return U.view_proj * vec4<f32>(world, 1.0);
}

fn rotate2(p: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
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
    o.aux = vec2<f32>(0.0);
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
    // x = analytic sprite style, y = stable per-particle rotation seed.
    @location(5) style: vec2<f32>,
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
        o.aux = vec2<f32>(0.0);
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
    let kind = in.style.x;
    var screen = q * radius;
    // Sparks stretch along their projected velocity, so gunfire has a crisp
    // direction instead of reading as a spray of round confetti.
    if (kind > 0.5 && kind < 1.5) {
        let projected = vec2<f32>(dot(in.vel, U.cam_right.xyz), dot(in.vel, U.cam_up.xyz));
        let tangent = normalize(projected + vec2<f32>(0.0001, 0.0));
        let normal = vec2<f32>(-tangent.y, tangent.x);
        let length = radius * mix(3.8, 2.2, u);
        screen = tangent * (q.x * length) + normal * (q.y * radius * 0.48);
    } else if (kind > 1.5 && kind < 2.5) {
        // Smoke rolls slowly as it expands.  The lobed alpha mask below keeps
        // the rotated quad from ever being visible.
        screen = rotate2(q, in.style.y + t * 0.45) * radius;
        screen.x = screen.x * 1.14;
    } else if (kind > 4.5) {
        // Physical fragments tumble rapidly; the diamond mask turns that
        // motion into readable debris at a fraction of a mesh particle's cost.
        screen = rotate2(q, in.style.y + t * 9.0) * radius;
        screen.y = screen.y * 1.55;
    } else if (kind > 2.5 && kind < 3.5) {
        screen = rotate2(q, in.style.y * 0.25) * radius;
        screen.y = screen.y * 1.45;
    }

    o.clip = face_camera_offset(p, screen);
    o.local = q;
    o.color = in.color;
    o.power = kind;
    o.aux = vec2<f32>(u, in.style.y);
    return o;
}

@fragment
fn fs_part(o: Out) -> @location(0) vec4<f32> {
    let d = length(o.local);
    let u = o.aux.x;
    let kind = o.power;
    var mask = 0.0;
    var energy = 1.28;
    var fade = (1.0 - u) * (1.0 - u);

    if (kind < 0.5) {
        // Soft energy mote / projectile trail.
        mask = exp(-d * d * 3.8);
    } else if (kind < 1.5) {
        // Needle-bright spark, already stretched along velocity by the vertex
        // shader. A hot centre and soft tail survive downsampling into bloom.
        let shaft = exp(-o.local.y * o.local.y * 15.0)
            * (1.0 - smoothstep(0.25, 1.05, abs(o.local.x)));
        let core = exp(-dot(o.local, o.local) * 9.0);
        mask = shaft + core * 0.70;
        energy = 1.85;
    } else if (kind < 2.5) {
        // Procedural smoke: two cheap lobes break up the circular billboard.
        let wobble = sin((o.local.x + o.local.y) * 7.0 + o.aux.y)
            + sin(o.local.x * 11.0 - o.local.y * 5.0 - o.aux.y * 1.7);
        let edge = 0.76 + wobble * 0.055;
        mask = 1.0 - smoothstep(0.08, edge, d);
        fade = smoothstep(0.0, 0.12, u) * (1.0 - u);
        energy = 0.48;
    } else if (kind < 3.5) {
        // Teardrop ember with a hot base and a pointed crown.
        let flame_d = length(vec2<f32>(o.local.x * 1.25, o.local.y * 0.78 + 0.20));
        mask = exp(-flame_d * flame_d * 4.8) * (1.0 - smoothstep(-0.55, 1.0, o.local.y));
        mask = mask * (0.82 + 0.18 * sin(o.aux.y * 3.1 + u * 24.0));
        energy = 1.75;
    } else if (kind < 4.5) {
        // Arcane ring, expanding inside its billboard as the billboard itself
        // also grows. A faint core makes the first frames feel explosive.
        let ring_r = mix(0.24, 0.72, smoothstep(0.0, 0.72, u));
        let ring = exp(-abs(d - ring_r) * 22.0);
        let core = exp(-d * d * 8.0) * (1.0 - smoothstep(0.0, 0.35, u));
        mask = ring + core * 0.72;
        fade = pow(1.0 - u, 1.35);
        energy = 1.62;
    } else {
        // Tumbling physical shard / armour chip.
        let diamond = abs(o.local.x) + abs(o.local.y);
        mask = 1.0 - smoothstep(0.72, 1.04, diamond);
        energy = 1.14;
    }

    let a = mask * fade * o.color.a;
    if (a <= 0.002) {
        discard;
    }
    // Hot styles deliberately exceed 1.0 so the HDR bright pass catches them;
    // smoke stays below it and therefore retains its soft body.
    return vec4<f32>(o.color.rgb * energy, a);
}
