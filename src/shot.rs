//! Offscreen frame capture.
//!
//! The game renders a frame to a texture with no window and no surface, reads
//! it back, and writes a PNG. It exists so the board can be *looked at* without
//! anything being on anyone's screen.
//!
//! The obvious alternative - grabbing the game's window off the desktop - is
//! both unreliable and rude. It captures whatever is actually in front of those
//! coordinates, which on a machine somebody is using is very often not the
//! game, and a GPU swapchain frequently comes back as a black rectangle anyway.
//! This path cannot pick up anything but the game, because there is nothing
//! else in the device.
//!
//! What it draws is the 3D board only: terrain, the circuit, towers, monsters,
//! projectiles and glows. The HUD lives in egui and would need a window, so it
//! is not here.

use std::path::Path;

use crate::decor::Decor;
use crate::game::Game;
use crate::game::board::{BH, BW};
use crate::gfx::draw::DrawList;
use crate::gfx::{Quality, Renderer};
use crate::math::{Camera, shadow_view_proj};
use crate::view;

/// Matches the live app, so a capture frames the board the way play does.
const CAM_PITCH_DEG: f32 = crate::CAM_PITCH_DEG;
/// Overridable so the framing can be swept and measured rather than eyeballed.
fn cam_zoom() -> f32 {
    std::env::var("TD_ZOOM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(crate::CAM_ZOOM)
}
const LIGHT_DIR: [f32; 3] = [-0.42, -0.62, 0.66];

/// The format the capture renders and encodes. Rgba8UnormSrgb everywhere, so
/// the bytes that come back are already the bytes a PNG wants.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub struct Shot {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA, top row first.
    pub rgba: Vec<u8>,
}

/// Renders one frame of `game` and returns the pixels.
pub fn capture(game: &Game, decor: &Decor, width: u32, height: u32, quality: Quality) -> Shot {
    pollster_block(async move {
        // No display handle: there is no window and no surface to present to,
        // which is the whole point of this path.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                ..Default::default()
            })
            .await
            .expect("no adapter for offscreen capture");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("capture"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                memory_hints: Default::default(),
                trace: Default::default(),
                ..Default::default()
            })
            .await
            .expect("no device for offscreen capture");

        let mut renderer = Renderer::new(&device, &adapter, FORMAT);
        renderer.quality = quality;
        renderer.set_quality(&device, quality);

        let statics = view::build_static(game, decor);
        renderer.set_static_scene(&queue, &statics.casters, &statics.flat);
        renderer.upload_static(&queue);

        let mut list = DrawList::default();
        view::draw_scene(game, decor, &mut list, game.time);

        let camera = Camera::frame_board(
            BW,
            BH,
            width as f32 / height.max(1) as f32,
            CAM_PITCH_DEG.to_radians(),
            0.0,
            cam_zoom(),
        );
        let light = shadow_view_proj(BW, BH, LIGHT_DIR);

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view_tex = target.create_view(&Default::default());

        // wgpu requires each copied row to start on a 256-byte boundary.
        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        // Two frames: the particle ring and the effects buffer both carry state
        // from the previous frame, and a single-frame capture of a freshly
        // created renderer catches them mid-initialisation.
        for _ in 0..2 {
            renderer.prepare(
                &device,
                &queue,
                &mut encoder,
                &list,
                &[],
                &camera,
                &light,
                width,
                height,
                1.0 / 60.0,
            );
        }
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("capture composite"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view_tex,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            renderer.composite(&mut pass, 0.0, 0.0, width as f32, height as f32);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv()
            .expect("readback never resolved")
            .expect("readback failed");

        let mapped = slice.get_mapped_range().expect("mapped range");
        let mut rgba = Vec::with_capacity((unpadded * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            rgba.extend_from_slice(&mapped[start..start + unpadded as usize]);
        }
        drop(mapped);
        readback.unmap();

        Shot {
            width,
            height,
            rgba,
        }
    })
}

/// Writes a PNG. Uncompressed deflate blocks, so there is no dependency to add
/// for what is a debugging convenience - a 1280x720 frame lands around 3.7 MB,
/// which is fine for something nobody ships.
pub fn write_png(path: &Path, shot: &Shot) -> std::io::Result<()> {
    let mut png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&shot.width.to_be_bytes());
    ihdr.extend_from_slice(&shot.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit, RGBA, no interlace
    chunk(&mut png, b"IHDR", &ihdr);

    // Raw scanlines, each prefixed with filter type 0.
    let mut raw = Vec::with_capacity((shot.width * 4 + 1) as usize * shot.height as usize);
    for row in 0..shot.height {
        raw.push(0);
        let start = (row * shot.width * 4) as usize;
        raw.extend_from_slice(&shot.rgba[start..start + (shot.width * 4) as usize]);
    }
    chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    chunk(&mut png, b"IEND", &[]);

    std::fs::write(path, png)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = Crc::new();
    crc.push(kind);
    crc.push(data);
    out.extend_from_slice(&crc.finish().to_be_bytes());
}

/// A zlib stream of stored (uncompressed) deflate blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01]; // deflate, no dictionary, fastest
    let mut rest = data;
    while !rest.is_empty() {
        let n = rest.len().min(0xFFFF);
        let last = if n == rest.len() { 1 } else { 0 };
        out.push(last);
        out.extend_from_slice(&(n as u16).to_le_bytes());
        out.extend_from_slice(&(!(n as u16)).to_le_bytes());
        out.extend_from_slice(&rest[..n]);
        rest = &rest[n..];
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

struct Crc(u32);

impl Crc {
    fn new() -> Self {
        Crc(0xFFFF_FFFF)
    }
    fn push(&mut self, data: &[u8]) {
        for &byte in data {
            self.0 ^= byte as u32;
            for _ in 0..8 {
                let mask = (self.0 & 1).wrapping_neg();
                self.0 = (self.0 >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
    }
    fn finish(self) -> u32 {
        !self.0
    }
}

/// The smallest async executor that will do. `pollster` is not a dependency and
/// this is the only place in the crate that blocks on a future.
fn pollster_block<F: Future>(fut: F) -> F::Output {
    use std::sync::{Arc, Condvar, Mutex};
    use std::task::{Context, Poll, Wake, Waker};

    struct Flag(Mutex<bool>, Condvar);
    impl Wake for Flag {
        fn wake(self: Arc<Self>) {
            *self.0.lock().unwrap() = true;
            self.1.notify_one();
        }
    }

    let flag = Arc::new(Flag(Mutex::new(true), Condvar::new()));
    let waker = Waker::from(flag.clone());
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        let mut woken = flag.0.lock().unwrap();
        if *woken {
            *woken = false;
            drop(woken);
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        } else {
            let _unused = flag.1.wait(woken).unwrap();
        }
    }
}
