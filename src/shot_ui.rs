//! Offscreen capture of the **whole game**, HUD included.
//!
//! [`crate::shot`] draws the board. This draws what a player actually looks at:
//! the same panels, the same widgets, the same modals, laid out by real egui at
//! a real window size and composited over the 3D scene exactly as the app does
//! it - board first, HUD on top.
//!
//! It matters because every check in this codebase so far has been a simulation
//! check. The board was verified by capture and the rules by tests, and the
//! interface between them - the thing the player uses - had never once been
//! looked at or clicked.

use std::path::Path;

use eframe::egui_wgpu;
use egui::{Context, RawInput, Rect, pos2, vec2};

use crate::decor::Decor;
use crate::game::Game;
use crate::game::board::{BH, BW};
use crate::gfx::draw::DrawList;
use crate::gfx::{Quality, Renderer};
use crate::math::{Camera, shadow_view_proj};
use crate::shot::Shot;
use crate::ui::{self, UiState};
use crate::view;

const LIGHT_DIR: [f32; 3] = [-0.42, -0.62, 0.66];
/// egui wants a gamma-space target; the board's composite writes gamma too.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Lays out one frame of the real HUD and returns the paint jobs plus the rect
/// the board should be drawn into.
///
/// Mirrors `App::game_frame`. Kept beside it rather than shared because the app
/// version also owns input, timing and the network, none of which a still frame
/// has any use for.
type Deltas = Vec<(egui::TextureId, epaint::ImageDelta)>;

fn run_ui(
    ctx: &Context,
    g: &mut Game,
    ust: &mut UiState,
    size: [f32; 2],
) -> (Vec<egui::ClippedPrimitive>, Rect, Deltas) {
    ust.compact = ui::compact_for(size[0]);
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]))),
        ..Default::default()
    };
    let mut board = Rect::NOTHING;
    // Texture deltas accumulate across *every* frame, not just the last one.
    //
    // This is the whole reason the first attempt rendered a completely empty
    // HUD. egui draws everything through its font atlas - not only glyphs but
    // solid rectangles too, via a white texel in the same texture - so a
    // renderer that never receives the atlas draws nothing at all, silently.
    // The atlas arrives on the *first* frame, and the first frame's output was
    // being dropped to get the second frame's settled panel sizes.
    let mut deltas: Vec<(egui::TextureId, epaint::ImageDelta)> = Vec::new();
    // Several frames, with the clock actually advancing.
    //
    // Two frames with a frozen clock is not enough, and the way it fails is
    // quietly misleading rather than obviously broken: panels settle their
    // sizes on the second frame, but anything in a *foreground* layer - modals,
    // the scoreboard, tooltips - fades in over about a fifth of a second, and a
    // capture taken before that finishes shows them at partial opacity. The
    // first attempt at this produced a draft modal you could read the board
    // through, and the bug was in the camera rather than the game.
    let mut out = None;
    for i in 0..12 {
        input.time = Some(i as f64 / 60.0);
        input.predicted_dt = 1.0 / 60.0;
        // `run_ui` hands back the same root Ui that `eframe::App::ui` receives,
        // so this lays out exactly what the game lays out.
        out = Some(ctx.run_ui(input.clone(), |ui| {
            let ctx = ui.ctx().clone();
            egui::Panel::top("hud")
                .exact_size(ui::top_h(ust.compact))
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(ui::pal::PANEL)
                        .inner_margin(egui::Margin::symmetric(10, 4)),
                )
                .show(ui, |ui| ui::top_bar(g, ui, ust, "60 fps"));
            egui::Panel::bottom("shop")
                .exact_size(ui::command_h(ust.compact))
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(ui::pal::PANEL)
                        .inner_margin(egui::Margin::symmetric(10, 8)),
                )
                .show(ui, |ui| ui::command_bar(g, ui, ust));
            board = egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| ui.available_rect_before_wrap())
                .inner;
            ui::scoreboard(g, &ctx);
            ui::modals(g, &ctx, ust);
        }));
        if let Some(o) = &mut out {
            for (id, parts) in std::mem::take(&mut o.textures_delta.set) {
                for part in parts {
                    deltas.push((id, part));
                }
            }
        }
    }
    let full = out.expect("ran at least once");
    let jobs = ctx.tessellate(full.shapes, full.pixels_per_point);
    (jobs, board, deltas)
}

/// Renders the board and the HUD together and reads the frame back.
pub fn capture(g: &mut Game, decor: &Decor, width: u32, height: u32, quality: Quality) -> Shot {
    let ctx = Context::default();
    ui::install_style(&ctx);
    let mut ust = UiState::default();
    ust.quality = quality;
    let (jobs, board_rect, deltas) = run_ui(&ctx, g, &mut ust, [width as f32, height as f32]);

    crate::shot::render_to_image(
        width,
        height,
        FORMAT,
        |device, queue, adapter, encoder, view_tex| {
            let mut scene = Renderer::new(device, adapter, FORMAT);
            scene.quality = quality;
            scene.set_quality(device, quality);
            let statics = view::build_static(g, decor);
            scene.set_static_scene(queue, &statics.casters, &statics.flat);
            scene.upload_static(queue);

            let mut list = DrawList::default();
            view::draw_scene(g, decor, &mut list, g.time);

            let bw = (board_rect.width().max(8.0)) as u32;
            let bh = (board_rect.height().max(8.0)) as u32;
            let camera = Camera::frame_board(
                BW,
                BH,
                bw as f32 / bh as f32,
                crate::CAM_PITCH_DEG.to_radians(),
                0.0,
                crate::CAM_ZOOM,
            );
            let light = shadow_view_proj(BW, BH, LIGHT_DIR);

            let mut egui_renderer =
                egui_wgpu::Renderer::new(device, FORMAT, egui_wgpu::RendererOptions::PREDICTABLE);
            for (id, delta) in &deltas {
                egui_renderer.update_texture(device, queue, *id, delta);
            }

            let screen = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [width, height],
                pixels_per_point: 1.0,
            };

            // Two scene frames: the particle ring and the effect buffers carry state
            // between frames, and a fresh renderer's first frame is mid-initialisation.
            for _ in 0..2 {
                scene.prepare(
                    device,
                    queue,
                    encoder,
                    &list,
                    &[],
                    &camera,
                    &light,
                    bw,
                    bh,
                    1.0 / 60.0,
                );
            }
            let extra = egui_renderer.update_buffers(device, queue, encoder, &jobs, &screen);

            {
                let mut pass = encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("ui capture pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: view_tex,
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
                // Board first, into the rect the central panel left for it, then the
                // HUD over the top - the same order and the same viewport the live
                // paint callback uses.
                scene.composite(
                    &mut pass,
                    board_rect.left(),
                    board_rect.top(),
                    board_rect.width(),
                    board_rect.height(),
                );
                pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
                pass.set_scissor_rect(0, 0, width, height);
                egui_renderer.render(&mut pass, &jobs, &screen);
            }

            extra
        },
    )
}

pub fn write_png(path: &Path, shot: &Shot) -> std::io::Result<()> {
    crate::shot::write_png(path, shot)
}
