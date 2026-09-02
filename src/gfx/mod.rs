//! The GPU renderer.
//!
//! Passes per frame, by quality preset - two at Performance, three at Balanced,
//! six at Ultra:
//!   1. a depth-only shadow pass from the light (Balanced and above),
//!   2. the lit scene: static geometry, this frame's dynamic instances, then
//!      additive glows and the live slice of the particle ring in the *same*
//!      pass, depth-tested but not depth-writing,
//!   3. bright-pass and separable blur at quarter resolution (Ultra only),
//!   4. a composite triangle that adds sky, bloom and tone mapping.
//!
//! The pass count is the number that decides whether this runs in a browser.
//! A packed board costs 0.02 ms of CPU to build and 0.03 ms to simulate, so
//! nothing here is ever waiting on the game - it is waiting on framebuffer
//! binds, and every one removed is worth more than any amount of culling.
//!
//! Four things keep this cheap:
//!   - **Static geometry is uploaded once.** Terrain, scenery and the road never
//!     change, so they are not rebuilt or re-uploaded per frame.
//!   - **Only live particles are drawn.** The ring buffer tracks its live window,
//!     so a quiet frame draws a few hundred instead of the full 32k.
//!   - **The scene renders at a scale factor** and is upsampled in the composite,
//!     which is the biggest fill-rate lever available.

pub mod draw;
pub mod mesh;

use std::collections::VecDeque;

use bytemuck::{Pod, Zeroable};
use draw::{DrawList, Instance};

use crate::game::fx::ParticleSpawn;
use crate::math::{Camera, Mat4};
use mesh::{SHAPE_COUNT, Span};

pub const STATIC_CAP: usize = 24_576;
pub const INSTANCE_CAP: usize = 32_768;
pub const GLOW_CAP: usize = 8_192;
pub const PARTICLE_CAP: usize = 32_768;
/// Longest a particle can live, used to bound the live window of the ring.
const MAX_PARTICLE_LIFE: f32 = 2.0;
const BLOOM_DIV: u32 = 4;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Alpha 0 so the composite can paint the sky wherever nothing was drawn.
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

/// Builds the scene shader for a quality preset.
///
/// `SHADOW_TAPS` is patched into the source rather than passed as a uniform so
/// the branch folds away at compile time; a per-pixel `if` on a value that is
/// constant for the whole frame is exactly the kind of thing a mobile GPU makes
/// you pay for.
fn make_solid_shader(device: &wgpu::Device, quality: Quality) -> wgpu::ShaderModule {
    const SRC: &str = include_str!("shaders/solid.wgsl");
    let src = SRC.replace(
        "const SHADOW_TAPS: i32 = 4;",
        &format!("const SHADOW_TAPS: i32 = {};", quality.shadow_taps()),
    );
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("solid.wgsl"),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    })
}

// ---------------------------------------------------------------- quality

/// Ordered cheapest to most expensive; auto-tuning compares presets directly,
/// so the declaration order is load-bearing.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Quality {
    Performance,
    Balanced,
    Ultra,
}

impl Quality {
    pub fn label(self) -> &'static str {
        match self {
            Quality::Performance => "Performance",
            Quality::Balanced => "Balanced",
            Quality::Ultra => "Ultra",
        }
    }
    /// Short form, for a HUD that has run out of room.
    pub fn short(self) -> &'static str {
        match self {
            Quality::Performance => "Perf",
            Quality::Balanced => "Bal",
            Quality::Ultra => "Ultra",
        }
    }
    /// Fraction of the viewport the 3D scene is rendered at.
    pub fn render_scale(self) -> f32 {
        match self {
            Quality::Performance => 0.60,
            Quality::Balanced => 0.85,
            Quality::Ultra => 1.0,
        }
    }
    pub fn msaa(self) -> u32 {
        match self {
            Quality::Ultra => 4,
            _ => 1,
        }
    }
    pub fn shadow_size(self) -> u32 {
        match self {
            Quality::Performance => 0,
            Quality::Balanced => 1024,
            Quality::Ultra => 2048,
        }
    }
    pub fn bloom(self) -> bool {
        self == Quality::Ultra
    }
    /// Shadow filter taps. Zero means the shadow pass does not run at all and
    /// the lighting branch folds away at shader-build time.
    pub fn shadow_taps(self) -> u32 {
        match self {
            Quality::Performance => 0,
            Quality::Balanced => 1,
            Quality::Ultra => 4,
        }
    }
    pub fn shadows(self) -> bool {
        self.shadow_taps() > 0
    }
    /// How many render passes a frame costs at this tier. Named here because it
    /// is the number that actually decides whether the game runs in a browser,
    /// and it is easy to add one by accident.
    pub fn passes(self) -> u32 {
        // scene + composite, plus shadow, plus the three-blit bloom chain.
        2 + u32::from(self.shadows()) + if self.bloom() { 3 } else { 0 }
    }
    pub fn lower(self) -> Option<Quality> {
        match self {
            Quality::Ultra => Some(Quality::Balanced),
            Quality::Balanced => Some(Quality::Performance),
            Quality::Performance => None,
        }
    }
    pub fn raise(self) -> Option<Quality> {
        match self {
            Quality::Performance => Some(Quality::Balanced),
            Quality::Balanced => Some(Quality::Ultra),
            Quality::Ultra => None,
        }
    }
}

// ---------------------------------------------------------------- gpu types

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
struct Uniforms {
    view_proj: [f32; 16],
    light_view_proj: [f32; 16],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    cam_pos: [f32; 4],
    /// xyz = direction towards the light.
    light_dir: [f32; 4],
    /// x = particle drag, y = gravity, z = time, w = shadow texel size.
    misc: [f32; 4],
    /// rgb = fog colour, a = density.
    fog: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
struct PostU {
    dir: [f32; 2],
    texel: [f32; 2],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
pub struct GpuParticle {
    p0: [f32; 3],
    vel: [f32; 3],
    t0_life: [f32; 2],
    size: [f32; 2],
    color: [f32; 4],
}

// ---------------------------------------------------------------- targets

struct Targets {
    w: u32,
    h: u32,
    samples: u32,
    /// Multisampled colour, or None when MSAA is off.
    scene_ms: Option<wgpu::TextureView>,
    /// Resolved single-sample colour, read by the bloom chain.
    scene: wgpu::TextureView,
    depth: wgpu::TextureView,
    bloom_a: wgpu::TextureView,
    bloom_b: wgpu::TextureView,
    bg_bright: wgpu::BindGroup,
    bg_blur_h: wgpu::BindGroup,
    bg_blur_v: wgpu::BindGroup,
    bg_composite: wgpu::BindGroup,
}

struct ScenePipes {
    solid: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    particle: wgpu::RenderPipeline,
    samples: u32,
}

// ---------------------------------------------------------------- renderer

pub struct Renderer {
    // Kept so pipelines can be rebuilt when the sample count changes.
    solid_src: wgpu::ShaderModule,
    bb_src: wgpu::ShaderModule,
    scene_layout: wgpu::PipelineLayout,

    shadow_pipe: wgpu::RenderPipeline,
    pipes: ScenePipes,
    bright_pipe: wgpu::RenderPipeline,
    blur_pipe: wgpu::RenderPipeline,
    composite_pipe: wgpu::RenderPipeline,

    scene_bgl: wgpu::BindGroupLayout,
    post_bgl: wgpu::BindGroupLayout,
    scene_bg: wgpu::BindGroup,
    shadow_bg: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    shadow_size: u32,

    uniform: wgpu::Buffer,
    mesh: wgpu::Buffer,
    statics: wgpu::Buffer,
    instances: wgpu::Buffer,
    glows: wgpu::Buffer,
    particles: wgpu::Buffer,
    post_bright: wgpu::Buffer,
    post_blur_h: wgpu::Buffer,
    post_blur_v: wgpu::Buffer,
    post_comp: wgpu::Buffer,

    sampler: wgpu::Sampler,
    hdr_format: wgpu::TextureFormat,
    /// Which sample counts this device actually supports for the scene format.
    /// WebGPU only guarantees 1 and 4, so 2 has to be probed.
    can_msaa2: bool,
    can_msaa4: bool,
    srgb_encode: bool,
    targets: Option<Targets>,

    /// Where each shape lives in the shared vertex buffer.
    spans: [Span; SHAPE_COUNT],
    /// (instance offset, count) per shape for the static and dynamic buffers.
    static_batches: [(u32, u32); SHAPE_COUNT],
    dynamic_batches: [(u32, u32); SHAPE_COUNT],
    static_count: u32,
    part_head: u32,
    part_tail: u32,
    /// (time, head) marks used to retire particles that can no longer be alive.
    part_marks: VecDeque<(f32, u32)>,
    part_scratch: Vec<GpuParticle>,
    inst_scratch: Vec<Instance>,

    time: f32,
    solid_count: u32,
    glow_count: u32,
    live_particles: u32,

    /// Tunables the UI can change at runtime.
    pub quality: Quality,
    /// The prefix of each static shape range that casts a shadow.
    static_shadow: [(u32, u32); SHAPE_COUNT],
    pub bloom_strength: f32,
    pub bloom_threshold: f32,
    pub particle_drag: f32,
    pub particle_gravity: f32,
    pub light_dir: [f32; 3],
    pub fog: [f32; 4],

    pub last_instances: u32,
}

const MESH_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

/// Solid instances sit alongside the mesh, so they start at location 2.
/// Location 7 carries the PBR material (roughness, metallic).
const SOLID_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    2 => Float32x3, 3 => Float32x3, 4 => Float32x2, 5 => Float32x2, 6 => Float32x4,
    7 => Float32x2
];

/// Billboards have no mesh buffer, so their instance data starts at location 0.
/// Glow instances and particles share this layout exactly.
const BILLBOARD_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4
];

const ADD_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

fn mesh_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<mesh::Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MESH_ATTRS,
    }
}

fn solid_inst_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Instance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &SOLID_ATTRS,
    }
}

fn billboard_layout<'a>(stride: u64) -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: stride,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &BILLBOARD_ATTRS,
    }
}

/// Steps a requested sample count down to one the device actually supports.
fn resolve_samples(want: u32, can2: bool, can4: bool) -> u32 {
    if want >= 4 && can4 {
        4
    } else if want >= 2 && can2 {
        2
    } else if want >= 4 && can2 {
        2
    } else {
        1
    }
}

fn depth_state(write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: Default::default(),
        bias: Default::default(),
    }
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        adapter: &wgpu::Adapter,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        // Half-float keeps colours above 1.0 alive so the bloom pass has
        // something to find. Fall back to 8-bit if the backend refuses.
        let hdr_format = {
            let f = wgpu::TextureFormat::Rgba16Float;
            let feats = adapter.get_texture_format_features(f);
            if feats.allowed_usages.contains(
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ) {
                f
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            }
        };

        // The GL path often cannot resolve multisampled HDR targets, and the
        // WebGPU spec only guarantees 1 and 4 samples for this format - asking
        // for an unsupported count is a hard validation error, not a fallback.
        let (can_msaa2, can_msaa4) = {
            let flags = adapter.get_texture_format_features(hdr_format).flags;
            let gl = adapter.get_info().backend == wgpu::Backend::Gl;
            // The adapter reports what the hardware can do, but the device only
            // permits the spec-guaranteed counts (1 and 4) unless this feature
            // was requested at creation. Asking for 2 without it is a hard
            // validation error, so trust the device, not the adapter.
            let adapter_specific = device
                .features()
                .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
            if gl {
                (false, false)
            } else {
                (
                    adapter_specific && flags.sample_count_supported(2),
                    flags.sample_count_supported(4),
                )
            }
        };

        let quality = Quality::Balanced;
        let solid_src = make_solid_shader(device, quality);
        let shadow_src = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
        });
        let bb_src = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("billboard.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/billboard.wgsl").into()),
        });
        let post_src = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/post.wgsl").into()),
        });

        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow bgl"),
            entries: &[uniform_entry],
        });
        let scene_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene bgl"),
            entries: &[
                uniform_entry,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        let post_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene layout"),
            bind_group_layouts: &[Some(&scene_bgl)],
            immediate_size: 0,
        });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow layout"),
            bind_group_layouts: &[Some(&shadow_bgl)],
            immediate_size: 0,
        });
        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post layout"),
            bind_group_layouts: &[Some(&post_bgl)],
            immediate_size: 0,
        });

        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shadow_src,
                entry_point: Some("vs"),
                buffers: &[Some(mesh_layout()), Some(solid_inst_layout())],
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.5,
                    clamp: 0.0,
                },
                ..depth_state(true)
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let samples = resolve_samples(quality.msaa(), can_msaa2, can_msaa4);
        let pipes = Self::build_scene_pipes(
            device,
            &solid_src,
            &bb_src,
            &scene_layout,
            hdr_format,
            samples,
        );

        let make_post = |label: &str, entry: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&post_layout),
                vertex: wgpu::VertexState {
                    module: &post_src,
                    entry_point: Some("vs_full"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &post_src,
                    entry_point: Some(entry),
                    targets: &[Some(format.into())],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let bright_pipe = make_post("bloom bright", "fs_bright", hdr_format);
        let blur_pipe = make_post("bloom blur", "fs_blur", hdr_format);
        let composite_pipe = make_post("composite", "fs_composite", target_format);

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_size = quality.shadow_size();
        let shadow_view = Self::make_shadow_texture(device, shadow_size);
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let scene_bg =
            Self::make_scene_bg(device, &scene_bgl, &uniform, &shadow_view, &shadow_sampler);
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow bg"),
            layout: &shadow_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let lib = mesh::build();
        let mesh = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shape library"),
            size: (lib.vertices.len() * std::mem::size_of::<mesh::Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mk_instances = |label: &str, n: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (n * std::mem::size_of::<Instance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let statics = mk_instances("static geometry", STATIC_CAP);
        let instances = mk_instances("dynamic instances", INSTANCE_CAP);
        let glows = mk_instances("glow instances", GLOW_CAP);
        let particles = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle ring"),
            size: (PARTICLE_CAP * std::mem::size_of::<GpuParticle>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mk_post_u = |label: &str| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<PostU>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            static_shadow: [(0, 0); SHAPE_COUNT],
            solid_src,
            bb_src,
            scene_layout,
            shadow_pipe,
            pipes,
            bright_pipe,
            blur_pipe,
            composite_pipe,
            scene_bgl,
            post_bgl,
            scene_bg,
            shadow_bg,
            shadow_view,
            shadow_sampler,
            shadow_size,
            uniform,
            mesh,
            statics,
            instances,
            glows,
            particles,
            post_bright: mk_post_u("post bright u"),
            post_blur_h: mk_post_u("post blur h u"),
            post_blur_v: mk_post_u("post blur v u"),
            post_comp: mk_post_u("post composite u"),
            sampler,
            hdr_format,
            can_msaa2,
            can_msaa4,
            srgb_encode: !target_format.is_srgb(),
            targets: None,
            spans: lib.spans,
            static_batches: [(0, 0); SHAPE_COUNT],
            dynamic_batches: [(0, 0); SHAPE_COUNT],
            static_count: 0,
            part_head: 0,
            part_tail: 0,
            part_marks: VecDeque::with_capacity(256),
            part_scratch: Vec::with_capacity(4096),
            inst_scratch: Vec::with_capacity(INSTANCE_CAP),
            time: 0.0,
            solid_count: 0,
            glow_count: 0,
            live_particles: 0,
            quality,
            bloom_strength: 0.72,
            bloom_threshold: 0.78,
            particle_drag: 2.4,
            particle_gravity: -3.2,
            light_dir: [-0.40, -0.52, 0.76],
            fog: [0.055, 0.070, 0.115, 0.030],
            last_instances: 0,
        }
    }

    fn build_scene_pipes(
        device: &wgpu::Device,
        solid_src: &wgpu::ShaderModule,
        bb_src: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        hdr_format: wgpu::TextureFormat,
        samples: u32,
    ) -> ScenePipes {
        let ms = wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        };
        let solid = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("solid"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: solid_src,
                entry_point: Some("vs"),
                buffers: &[Some(mesh_layout()), Some(solid_inst_layout())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: solid_src,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            // Every cube is closed, so its back faces are always hidden. Culling
            // them halves the vertex work and most of the fragment work.
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(depth_state(true)),
            multisample: ms,
            multiview_mask: None,
            cache: None,
        });

        // Effects render into the scene target alongside the solids, so they
        // match its sample count and share its depth buffer - tested, so a glow
        // inside a tower is hidden by it, but not written, so two overlapping
        // glows still add.
        let make_billboard = |label: &str, vs: &str, fs: &str, stride: u64| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: bb_src,
                    entry_point: Some(vs),
                    buffers: &[Some(billboard_layout(stride))],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: bb_src,
                    entry_point: Some(fs),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_format,
                        blend: Some(ADD_BLEND),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth_state(false)),
                multisample: ms,
                multiview_mask: None,
                cache: None,
            })
        };

        ScenePipes {
            solid,
            glow: make_billboard(
                "glow",
                "vs_glow",
                "fs_glow",
                std::mem::size_of::<Instance>() as u64,
            ),
            particle: make_billboard(
                "particles",
                "vs_part",
                "fs_part",
                std::mem::size_of::<GpuParticle>() as u64,
            ),
            samples,
        }
    }

    /// The scene bind group always needs a shadow map to point at, even at the
    /// preset that never renders into one, so a disabled shadow becomes a 64px
    /// stub rather than an absent binding.
    fn make_shadow_texture(device: &wgpu::Device, size: u32) -> wgpu::TextureView {
        let size = size.max(64);
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("shadow map"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn make_scene_bg(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        shadow: &wgpu::TextureView,
        cmp: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(cmp),
                },
            ],
        })
    }

    pub fn upload_static(&self, queue: &wgpu::Queue) {
        let lib = mesh::build();
        queue.write_buffer(&self.mesh, 0, bytemuck::cast_slice(&lib.vertices));
    }

    /// Packs the shape buckets end to end into `buf` and records where each one
    /// landed, so drawing is one call per shape with no sorting.
    fn pack(
        queue: &wgpu::Queue,
        buf: &wgpu::Buffer,
        list: &DrawList,
        cap: usize,
        scratch: &mut Vec<Instance>,
    ) -> ([(u32, u32); SHAPE_COUNT], u32) {
        scratch.clear();
        let mut batches = [(0u32, 0u32); SHAPE_COUNT];
        for (i, bucket) in list.solid.iter().enumerate() {
            let room = cap.saturating_sub(scratch.len());
            let n = bucket.len().min(room);
            batches[i] = (scratch.len() as u32, n as u32);
            scratch.extend_from_slice(&bucket[..n]);
        }
        if !scratch.is_empty() {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(scratch));
        }
        (batches, scratch.len() as u32)
    }

    /// Issues one instanced draw per shape.
    fn draw_shapes(
        pass: &mut wgpu::RenderPass<'_>,
        spans: &[Span; SHAPE_COUNT],
        batches: &[(u32, u32); SHAPE_COUNT],
    ) {
        for (i, &(first, count)) in batches.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let span = spans[i];
            pass.draw(span.first..span.first + span.count, first..first + count);
        }
    }

    /// Uploads the geometry that never changes: terrain, road, scenery, pads.
    /// Called once at startup, not per frame.
    ///
    /// `casters` is the subset that should appear in the shadow map. It is
    /// packed to the front of every shape's range, so the shadow pass draws a
    /// prefix and the scene pass draws the whole thing - one buffer, two draw
    /// lists. Most of the static scene is flat ground whose shadow lands
    /// exactly where the ground already is, and re-rasterising sixteen hundred
    /// of those every frame for no visible result is the single most wasteful
    /// thing the renderer was doing.
    pub fn set_static_scene(&mut self, queue: &wgpu::Queue, casters: &DrawList, rest: &DrawList) {
        let mut scratch = Vec::new();
        let mut batches = [(0u32, 0u32); SHAPE_COUNT];
        let mut shadow = [(0u32, 0u32); SHAPE_COUNT];
        for i in 0..SHAPE_COUNT {
            let first = scratch.len() as u32;
            for (n, bucket) in [(0, &casters.solid[i]), (1, &rest.solid[i])] {
                let room = STATIC_CAP.saturating_sub(scratch.len());
                let take = bucket.len().min(room);
                scratch.extend_from_slice(&bucket[..take]);
                if n == 0 {
                    shadow[i] = (first, take as u32);
                }
            }
            batches[i] = (first, scratch.len() as u32 - first);
        }
        if !scratch.is_empty() {
            queue.write_buffer(&self.statics, 0, bytemuck::cast_slice(&scratch));
        }
        self.static_shadow = shadow;
        self.static_batches = batches;
        self.static_count = scratch.len() as u32;
    }

    #[allow(dead_code)]
    fn set_static_scene_flat(&mut self, queue: &wgpu::Queue, list: &DrawList) {
        let mut scratch = Vec::new();
        let (batches, n) = Self::pack(queue, &self.statics, list, STATIC_CAP, &mut scratch);
        self.static_batches = batches;
        self.static_count = n;
    }

    /// Switches quality preset, rebuilding whatever the change invalidates.
    pub fn set_quality(&mut self, device: &wgpu::Device, q: Quality) {
        if q == self.quality {
            return;
        }
        let taps_changed = q.shadow_taps() != self.quality.shadow_taps();
        self.quality = q;
        if taps_changed {
            self.solid_src = make_solid_shader(device, q);
        }

        let samples = resolve_samples(q.msaa(), self.can_msaa2, self.can_msaa4);
        if taps_changed || samples != self.pipes.samples {
            self.pipes = Self::build_scene_pipes(
                device,
                &self.solid_src,
                &self.bb_src,
                &self.scene_layout,
                self.hdr_format,
                samples,
            );
        }
        let shadow = q.shadow_size();
        if shadow != self.shadow_size {
            self.shadow_size = shadow;
            self.shadow_view = Self::make_shadow_texture(device, shadow);
            self.scene_bg = Self::make_scene_bg(
                device,
                &self.scene_bgl,
                &self.uniform,
                &self.shadow_view,
                &self.shadow_sampler,
            );
        }
        // Force the colour targets to be rebuilt at the new render scale.
        self.targets = None;
    }

    // ------------------------------------------------ targets

    fn ensure_targets(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        let w = w.max(8);
        let h = h.max(8);
        let samples = self.pipes.samples;
        if let Some(t) = &self.targets {
            if t.w == w && t.h == h && t.samples == samples {
                return;
            }
        }
        // The bloom chain only has a resolution worth having when it runs.
        // Off, they stay 4x4 and read back as the zeroes a fresh texture is
        // specified to hold, so the composite adds nothing without a branch.
        let (bw, bh) = if self.quality.bloom() {
            ((w / BLOOM_DIV).max(4), (h / BLOOM_DIV).max(4))
        } else {
            (4, 4)
        };

        let mk = |label: &str,
                  w: u32,
                  h: u32,
                  format: wgpu::TextureFormat,
                  samples: u32|
         -> wgpu::TextureView {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };

        let scene = mk("scene resolve", w, h, self.hdr_format, 1);
        let scene_ms = if samples > 1 {
            Some(mk("scene msaa", w, h, self.hdr_format, samples))
        } else {
            None
        };
        let depth = mk("depth", w, h, DEPTH_FORMAT, samples);
        let bloom_a = mk("bloom a", bw, bh, self.hdr_format, 1);
        let bloom_b = mk("bloom b", bw, bh, self.hdr_format, 1);
        let mk_bg =
            |label: &str, t0: &wgpu::TextureView, t1: &wgpu::TextureView, u: &wgpu::Buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(label),
                    layout: &self.post_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(t0),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(t1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: u.as_entire_binding(),
                        },
                    ],
                })
            };

        // Glows are already in the scene buffer at values above one, so the
        // bright pass picks them up for free.
        let bg_bright = mk_bg("bg bright", &scene, &scene, &self.post_bright);
        let bg_blur_h = mk_bg("bg blur h", &bloom_a, &bloom_a, &self.post_blur_h);
        let bg_blur_v = mk_bg("bg blur v", &bloom_b, &bloom_b, &self.post_blur_v);
        let bg_composite = mk_bg("bg composite", &scene, &bloom_a, &self.post_comp);

        self.targets = Some(Targets {
            w,
            h,
            samples,
            scene_ms,
            scene,
            depth,
            bloom_a,
            bloom_b,
            bg_bright,
            bg_blur_h,
            bg_blur_v,
            bg_composite,
        });
    }

    fn write_post_uniforms(&self, queue: &wgpu::Queue, bw: f32, bh: f32) {
        let srgb = if self.srgb_encode { 1.0 } else { 0.0 };
        let strength = if self.quality.bloom() {
            self.bloom_strength
        } else {
            0.0
        };
        let common = [self.bloom_threshold, strength, srgb, 0.0];
        let texel = [1.0 / bw, 1.0 / bh];
        for (buf, dir) in [
            (&self.post_bright, [0.0, 0.0]),
            (&self.post_blur_h, [1.0 / bw, 0.0]),
            (&self.post_blur_v, [0.0, 1.0 / bh]),
            (&self.post_comp, [0.0, 0.0]),
        ] {
            queue.write_buffer(
                buf,
                0,
                bytemuck::bytes_of(&PostU {
                    dir,
                    texel,
                    params: common,
                }),
            );
        }
    }

    // ------------------------------------------------ frame

    /// Uploads this frame's dynamic geometry and encodes everything except the
    /// final composite, which has to happen inside egui's render pass.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        list: &DrawList,
        spawns: &[ParticleSpawn],
        camera: &Camera,
        light_view_proj: &Mat4,
        px_w: u32,
        px_h: u32,
        dt: f32,
    ) {
        // Wrap the clock so f32 precision stays tight for particle ages.
        self.time = (self.time + dt) % 1024.0;

        // The scene renders at a fraction of the viewport and is upsampled by the
        // composite. This is the biggest fill-rate lever we have.
        let s = self.quality.render_scale();
        let rw = ((px_w as f32 * s).round() as u32).max(8);
        let rh = ((px_h as f32 * s).round() as u32).max(8);
        self.ensure_targets(device, rw, rh);

        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Uniforms {
                view_proj: camera.view_proj.0,
                light_view_proj: light_view_proj.0,
                cam_right: [camera.right.x, camera.right.y, camera.right.z, 0.0],
                cam_up: [camera.up.x, camera.up.y, camera.up.z, 0.0],
                cam_pos: [camera.eye.x, camera.eye.y, camera.eye.z, 1.0],
                light_dir: [self.light_dir[0], self.light_dir[1], self.light_dir[2], 0.0],
                misc: [
                    self.particle_drag,
                    self.particle_gravity,
                    self.time,
                    1.0 / self.shadow_size as f32,
                ],
                fog: self.fog,
            }),
        );
        self.write_post_uniforms(
            queue,
            (rw / BLOOM_DIV).max(4) as f32,
            (rh / BLOOM_DIV).max(4) as f32,
        );

        let mut scratch = std::mem::take(&mut self.inst_scratch);
        let (batches, n) = Self::pack(queue, &self.instances, list, INSTANCE_CAP, &mut scratch);
        self.inst_scratch = scratch;
        self.dynamic_batches = batches;
        self.solid_count = n;
        self.glow_count = list.glow.len().min(GLOW_CAP) as u32;
        if self.glow_count > 0 {
            queue.write_buffer(
                &self.glows,
                0,
                bytemuck::cast_slice(&list.glow[..self.glow_count as usize]),
            );
        }
        self.last_instances = self.static_count + self.solid_count + self.glow_count;

        self.upload_particles(queue, spawns);

        // Every mutation is done; from here on we only read.
        let Some(targets) = &self.targets else { return };

        // --- shadow map. Skipped outright at Performance: the pass is a full
        // depth-only redraw of every caster on the board, and it is the single
        // most expensive thing a weak GPU is asked to do here.
        if self.quality.shadows() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_pipe);
            pass.set_bind_group(0, &self.shadow_bg, &[]);
            pass.set_vertex_buffer(0, self.mesh.slice(..));
            if self.static_count > 0 {
                pass.set_vertex_buffer(1, self.statics.slice(..));
                // Casters only. The ground does not shadow itself.
                Self::draw_shapes(&mut pass, &self.spans, &self.static_shadow);
            }
            if self.solid_count > 0 {
                pass.set_vertex_buffer(1, self.instances.slice(..));
                Self::draw_shapes(&mut pass, &self.spans, &self.dynamic_batches);
            }
        }

        // --- main scene
        {
            let (view, resolve) = match &targets.scene_ms {
                Some(ms) => (ms, Some(&targets.scene)),
                None => (&targets.scene, None),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.scene_bg, &[]);
            pass.set_pipeline(&self.pipes.solid);
            pass.set_vertex_buffer(0, self.mesh.slice(..));
            if self.static_count > 0 {
                pass.set_vertex_buffer(1, self.statics.slice(..));
                Self::draw_shapes(&mut pass, &self.spans, &self.static_batches);
            }
            if self.solid_count > 0 {
                pass.set_vertex_buffer(1, self.instances.slice(..));
                Self::draw_shapes(&mut pass, &self.spans, &self.dynamic_batches);
            }

            // Glows and particles ride in the same pass as the solids, blended
            // additively and depth-tested without writing. They used to own a
            // separate low-resolution target that the composite added back over
            // everything, which cost a whole render pass, a whole texture, and
            // a texture fetch in three post shaders - and looked worse, because
            // a glow with no depth test shines straight through the tower it is
            // supposed to be inside.
            if self.glow_count > 0 {
                pass.set_pipeline(&self.pipes.glow);
                pass.set_vertex_buffer(0, self.glows.slice(..));
                pass.draw(0..4, 0..self.glow_count);
            }
            // Only the live slice of the particle ring, not the whole buffer.
            if self.live_particles > 0 {
                pass.set_pipeline(&self.pipes.particle);
                pass.set_vertex_buffer(0, self.particles.slice(..));
                let (tail, head) = (self.part_tail, self.part_head);
                if tail < head {
                    pass.draw(0..4, tail..head);
                } else {
                    pass.draw(0..4, tail..PARTICLE_CAP as u32);
                    if head > 0 {
                        pass.draw(0..4, 0..head);
                    }
                }
            }
        }

        if !self.quality.bloom() {
            return;
        }
        self.blit(
            encoder,
            &self.bright_pipe,
            &targets.bg_bright,
            &targets.bloom_a,
            "bloom bright",
        );
        self.blit(
            encoder,
            &self.blur_pipe,
            &targets.bg_blur_h,
            &targets.bloom_b,
            "bloom blur h",
        );
        self.blit(
            encoder,
            &self.blur_pipe,
            &targets.bg_blur_v,
            &targets.bloom_a,
            "bloom blur v",
        );
    }

    fn blit(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        bind: &wgpu::BindGroup,
        target: &wgpu::TextureView,
        label: &str,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }

    fn upload_particles(&mut self, queue: &wgpu::Queue, spawns: &[ParticleSpawn]) {
        if !spawns.is_empty() {
            let n = spawns.len().min(PARTICLE_CAP);
            self.part_scratch.clear();
            self.part_scratch
                .extend(spawns[..n].iter().map(|s| GpuParticle {
                    p0: s.pos,
                    vel: s.vel,
                    t0_life: [self.time, s.life],
                    size: s.size,
                    color: s.color,
                }));

            let stride = std::mem::size_of::<GpuParticle>() as u64;
            let head = self.part_head as usize;
            let first = n.min(PARTICLE_CAP - head);
            queue.write_buffer(
                &self.particles,
                head as u64 * stride,
                bytemuck::cast_slice(&self.part_scratch[..first]),
            );
            if n > first {
                queue.write_buffer(
                    &self.particles,
                    0,
                    bytemuck::cast_slice(&self.part_scratch[first..n]),
                );
            }
            self.part_head = ((head + n) % PARTICLE_CAP) as u32;
            self.part_marks.push_back((self.time, self.part_head));
        }

        // Retire marks older than the longest possible particle life; whatever
        // they pointed at can no longer be on screen.
        while let Some(&(t, h)) = self.part_marks.front() {
            let age = self.time - t;
            if age > MAX_PARTICLE_LIFE || age < 0.0 {
                self.part_tail = h;
                self.part_marks.pop_front();
            } else {
                break;
            }
        }
        self.live_particles =
            (self.part_head + PARTICLE_CAP as u32 - self.part_tail) % PARTICLE_CAP as u32;
    }

    /// Draws the tone-mapped result into egui's render pass, clipped to `viewport`.
    ///
    /// egui sets a viewport for a paint callback but leaves the scissor alone.
    /// The composite is an oversized fullscreen triangle whose corners sit well
    /// outside NDC, and a viewport does not clip - so without an explicit
    /// scissor the board spills across the whole surface and paints over the
    /// HUD panels.
    pub fn composite(&self, pass: &mut wgpu::RenderPass<'static>, x: f32, y: f32, w: f32, h: f32) {
        let Some(targets) = &self.targets else { return };
        if w < 1.0 || h < 1.0 {
            return;
        }
        pass.set_viewport(x, y, w, h, 0.0, 1.0);
        pass.set_scissor_rect(
            x.max(0.0) as u32,
            y.max(0.0) as u32,
            w.max(1.0) as u32,
            h.max(1.0) as u32,
        );
        pass.set_pipeline(&self.composite_pipe);
        pass.set_bind_group(0, &targets.bg_composite, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn live_particles(&self) -> u32 {
        self.live_particles
    }

    pub fn stats_line(&self) -> String {
        format!(
            "{} static · {} dyn · {} glow · {} fx · {}x{} msaa{}",
            self.static_count,
            self.solid_count,
            self.glow_count,
            self.live_particles,
            self.targets.as_ref().map(|t| t.w).unwrap_or(0),
            self.targets.as_ref().map(|t| t.h).unwrap_or(0),
            self.pipes.samples,
        )
    }
}
