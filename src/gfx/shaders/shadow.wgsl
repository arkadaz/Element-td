// Depth-only pass from the light's point of view. Shares the instance buffer
// with the main solid pass, so casting shadows costs one extra draw call.

struct Uniforms {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_pos: vec4<f32>,
    light_dir: vec4<f32>,
    misc: vec4<f32>,
    fog: vec4<f32>,
};
@group(0) @binding(0) var<uniform> U: Uniforms;

struct VsIn {
    @location(0) v_pos: vec3<f32>,
    @location(1) v_nrm: vec3<f32>,
    @location(2) i_pos: vec3<f32>,
    @location(3) i_scale: vec3<f32>,
    @location(4) i_rot: vec2<f32>,
    @location(5) i_params: vec2<f32>,
    @location(6) i_color: vec4<f32>,
};

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
fn vs(in: VsIn) -> @builtin(position) vec4<f32> {
    // Fully transparent props (build ghosts) should not cast.
    if (in.i_color.a < 0.5) {
        return vec4<f32>(-10.0, -10.0, -10.0, 1.0);
    }
    let r = rot_of(in.i_rot.x, in.i_rot.y);
    let world = in.i_pos + r * (in.v_pos * in.i_scale);
    return U.light_view_proj * vec4<f32>(world, 1.0);
}
