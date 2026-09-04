// Release builds are a game window, not a console app: without this Windows
// opens a terminal alongside it and steals focus from the game.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Green Circle TD - the Warcraft III map, ported to Rust and wgpu.
//!
//! Rendering runs on wgpu (WebGPU in the browser, WebGL2 as a fallback, native
//! Vulkan/DX12 on the desktop). egui draws the HUD on top of the same surface.

mod audio;
#[cfg(test)]
mod bench_tests;
mod decor;
mod game;
mod gfx;
mod math;
mod menu;
mod net;
mod rng;
mod save;
#[cfg(not(target_arch = "wasm32"))]
mod shot;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod shot_tests;
#[cfg(not(target_arch = "wasm32"))]
mod shot_ui;
mod ui;
mod ui_layout_tests;
mod view;

use eframe::egui_wgpu;
use egui::{Key, Rect, Sense};

use decor::Decor;
use game::Game;
use game::board::{BH, BW};
use game::fx::ParticleSpawn;
use gfx::Quality;
use gfx::Renderer;
use gfx::draw::DrawList;
use math::{Camera, Mat4, Rig, shadow_view_proj};
use menu::{MenuState, Screen};
use net::Net;

/// How far the camera tilts down towards the board. Every framing the game
/// builds shares it, so the ground meets the screen at one angle whether the
/// player is scrolling the board, watching the title backdrop turn or looking
/// at a screenshot.
pub const CAM_PITCH_DEG: f32 = 55.0;
/// Warcraft's tactical bearing: straight lanes stay horizontal or vertical on
/// screen, so their turns and two traffic directions are readable instantly.
pub const CAM_YAW_DEG: f32 = 0.0;
/// How tightly a fitted framing pulls in on what it was asked to fit. Only the
/// title screen's backdrop is framed that way now; play frames a window of the
/// board and scrolls it.
pub const CAM_ZOOM: f32 = 1.15;
/// How much of the compact solo board is in frame at the opening zoom.
///
/// The compact arena only needs a short pan to reach its opposite side. Keeping
/// this tighter than a full-board overview makes downloaded models and target
/// states readable instead of turning monsters into twelve-pixel dots.
pub const CAM_SPAN: f32 = 23.0;
/// The tightest the wheel may zoom in. Closer than this and a tower fills the
/// screen with the lane either side of it out of frame.
pub const CAM_SPAN_MIN: f32 = 13.0;
/// How fast the keys and the screen edge scroll, in viewports per second.
const CAM_PAN_SPEED: f32 = 0.6;
/// How close to the edge of the board the cursor scrolls the view, in points.
const CAM_EDGE: f32 = 14.0;
/// How much of the span one point of wheel travel takes off. A notch is around
/// fifty points, so a notch is about a fifth.
const CAM_WHEEL: f32 = 0.004;
/// Ceiling on how many device pixels the 3D scene is rendered at per point.
///
/// The HUD stays crisp at the display's real scale; the 3D scene does not need
/// to be supersampled, and on a 2x display the difference between 1.0 and 1.35
/// here is 1.8x the fill rate for something nobody can see.
const MAX_SCENE_DPR: f32 = 1.0;
/// Key light direction, shared by the shader and the shadow camera.
const LIGHT_DIR: [f32; 3] = [-0.40, -0.52, 0.76];

/// Browsers own the presentation compositor, so they must also choose the
/// adapter that can present to it. Forcing the discrete GPU can create a valid
/// WebGPU device on one adapter while Edge presents the canvas on another,
/// leaving a permanently black canvas after the first custom render callback.
fn browser_power_preference() -> wgpu::PowerPreference {
    wgpu::PowerPreference::None
}

/// -1 at the low edge, +1 at the high edge, and zero in the safe middle.
/// Kept separate from camera state so the literal window-edge behavior is
/// easy to regression-test without constructing an egui frame.
fn edge_scroll_axis(lo: f32, hi: f32, at: f32) -> f32 {
    f32::from(at >= hi - CAM_EDGE) - f32::from(at <= lo + CAM_EDGE)
}

#[cfg(test)]
mod camera_input_tests {
    use super::*;

    #[test]
    fn literal_top_and_bottom_edges_scroll_in_opposite_directions() {
        assert_eq!(edge_scroll_axis(0.0, 900.0, 0.0), -1.0);
        assert_eq!(edge_scroll_axis(0.0, 900.0, 899.0), 1.0);
        assert_eq!(edge_scroll_axis(0.0, 900.0, 450.0), 0.0);
    }
}

#[cfg(test)]
mod browser_gpu_tests {
    use super::*;

    #[test]
    fn browser_leaves_presentation_adapter_selection_to_the_compositor() {
        assert!(matches!(
            browser_power_preference(),
            wgpu::PowerPreference::None
        ));
    }
}

// ---------------------------------------------------------------- gpu bridge

/// Everything the render callback needs, double-buffered with the app so no
/// per-frame allocation happens on the hot path.
#[derive(Default)]
pub struct FrameData {
    pub list: DrawList,
    pub spawns: Vec<ParticleSpawn>,
    pub camera: Camera,
    pub light_view_proj: Mat4,
    pub px: [u32; 2],
    pub dt: f32,
}

/// Renderer and frame data live in one resource so the callback can borrow both.
pub struct GpuState {
    pub renderer: Renderer,
    pub frame: FrameData,
}

struct BoardCallback;

impl egui_wgpu::CallbackTrait for BoardCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        res: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(gs) = res.get_mut::<GpuState>() {
            let f = &gs.frame;
            gs.renderer.prepare(
                device,
                queue,
                encoder,
                &f.list,
                &f.spawns,
                &f.camera,
                &f.light_view_proj,
                f.px[0],
                f.px[1],
                f.dt,
            );
        }
        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        res: &egui_wgpu::CallbackResources,
    ) {
        let Some(gs) = res.get::<GpuState>() else {
            return;
        };
        let vp = info.viewport_in_pixels();
        gs.renderer.composite(
            pass,
            vp.left_px as f32,
            vp.top_px as f32,
            vp.width_px as f32,
            vp.height_px as f32,
        );
    }
}

// ---------------------------------------------------------------- input

#[derive(Default)]
struct Keys {
    pause: bool,
    speed: bool,
    send: bool,
    cancel: bool,
    upgrade: bool,
    sell: bool,
    help: bool,
    bloom: bool,
    shift: bool,
    digits: [bool; 11],
    /// Which way the player is scrolling, in screen terms: x across, y down,
    /// each -1, 0 or 1. Held rather than tapped, so it is read every frame.
    pan: [f32; 2],
    /// Debug builds only: fill pads / grant gold, for playtesting.
    dev_fill: bool,
    dev_gold: bool,
}

fn read_keys(ui: &egui::Ui) -> Keys {
    ui.input(|i| {
        const NUMS: [Key; 11] = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
            Key::Num0,
            Key::Minus,
        ];
        let mut digits = [false; 11];
        for (n, key) in NUMS.iter().enumerate() {
            digits[n] = i.key_pressed(*key);
        }
        let axis = |back: &[Key], fwd: &[Key]| {
            let held = |keys: &[Key]| keys.iter().any(|k| i.key_down(*k));
            f32::from(held(fwd)) - f32::from(held(back))
        };
        Keys {
            pause: i.key_pressed(Key::Space),
            speed: i.key_pressed(Key::F),
            send: i.key_pressed(Key::Enter),
            cancel: i.key_pressed(Key::Escape),
            upgrade: i.key_pressed(Key::U),
            sell: i.key_pressed(Key::S),
            help: i.key_pressed(Key::H),
            bloom: i.key_pressed(Key::B),
            shift: i.modifiers.shift,
            digits,
            pan: [
                axis(&[Key::ArrowLeft, Key::A], &[Key::ArrowRight, Key::D]),
                // S is missing from that row on purpose. It is the sell hotkey
                // the command bar advertises, and holding it to scroll south
                // would sell the selected tower for a fraction of what it cost,
                // with no way to undo it. Down arrow, the screen edge and the
                // middle mouse button all scroll that way instead.
                axis(&[Key::ArrowUp, Key::W], &[Key::ArrowDown]),
            ],
            dev_fill: cfg!(debug_assertions) && i.key_pressed(Key::T),
            dev_gold: cfg!(debug_assertions) && i.key_pressed(Key::G),
        }
    })
}

// ---------------------------------------------------------------- profiling

/// Exponentially smoothed millisecond costs for the parts of a frame we own.
/// Anything left over between `total` and the sum is the GPU and the swapchain.
#[derive(Default, Clone, Copy)]
pub struct Profile {
    pub sim: f32,
    pub build: f32,
    pub hud: f32,
    pub total: f32,
}

impl Profile {
    fn feed(slot: &mut f32, ms: f64) {
        *slot += (ms as f32 - *slot) * 0.08;
    }
    pub fn line(&self) -> String {
        format!(
            "sim {:.1} · scene {:.1} · hud {:.1} · frame {:.1}",
            self.sim, self.build, self.hud, self.total
        )
    }
}

/// Monotonic milliseconds, in f64.
///
/// It has to be f64: milliseconds since the epoch is about 1.8e12, and an f32
/// carries only seven significant digits, so every timestamp rounded to the
/// nearest ~131 seconds and the profiler dutifully reported that every part of
/// the frame took exactly zero. Only the *delta* is narrowed to f32.
fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------- app

struct App {
    audio: audio::Audio,
    game: Game,
    decor: Decor,
    ust: ui::UiState,
    menu: MenuState,
    net: Net,
    draw: DrawList,
    spawns: Vec<ParticleSpawn>,
    rs: egui_wgpu::RenderState,
    fps: f32,
    anim: f32,
    /// Rolling per-section frame cost in milliseconds. Guessing at where a
    /// frame goes is how you end up optimising a shader on a card that was
    /// never the bottleneck, so the HUD reports it.
    prof: Profile,
    /// Where the player has scrolled to, in tiles, and how much of the board
    /// is in frame. They live here rather than in [`ui::UiState`] because the
    /// camera belongs to the app: the HUD never moves it, and the shot paths
    /// frame the board without a `UiState` at all.
    pan: [f32; 2],
    span: f32,
    /// The rectangle every build pad falls inside. The pan clamp needs it: the
    /// pads stop short of the arena's edge, and a camera that cannot be aimed
    /// at one is a plot the player cannot build on.
    pads: [f32; 4],
    /// When the next frame is due, for the native frame limiter.
    #[cfg(not(target_arch = "wasm32"))]
    next_frame: Option<std::time::Instant>,
    /// Whether the one-time viewport-dependent setup has run.
    sized_once: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let rs = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu render state (the wgpu backend must be enabled)")
            .clone();

        let game = Game::new();
        let decor = Decor::build(&game.board);

        let mut renderer = Renderer::new(&rs.device, &rs.adapter, rs.target_format);
        renderer.upload_static(&rs.queue);
        // Terrain, road, scenery and the build grid never change, so they are
        // uploaded once instead of being rebuilt every frame.
        let statics = view::build_static(&game, &decor);
        renderer.set_static_scene(&rs.queue, &statics.casters, &statics.flat);
        rs.renderer.write().callback_resources.insert(GpuState {
            renderer,
            frame: FrameData::default(),
        });

        ui::install_style(&cc.egui_ctx);

        let pads = pad_bounds(&game.board);
        let mut app = Self {
            audio: audio::Audio::new(),
            game,
            decor,
            ust: ui::UiState::default(),
            menu: MenuState {
                saved: save::load(),
                ..MenuState::default()
            },
            net: Net::default(),
            draw: DrawList::default(),
            spawns: Vec::with_capacity(4096),
            rs,
            fps: 60.0,
            anim: 0.0,
            prof: Profile::default(),
            pan: lane_middle(),
            span: CAM_SPAN,
            pads,
            #[cfg(not(target_arch = "wasm32"))]
            next_frame: None,
            sized_once: false,
        };
        app.apply_demo_env();
        app
    }

    /// `TD_DEMO=1` seeds a played-in board and starts the first wave. Used for
    /// grabbing screenshots and for eyeballing balance without clicking through
    /// twenty waves first.
    fn apply_demo_env(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Ok(v) = std::env::var("TD_DEMO") else {
                return;
            };
            if v.is_empty() || v == "0" {
                return;
            }
            // How many steps up its path each demo tower is pushed.
            let level: u32 = v.parse().unwrap_or(1).clamp(1, 20);
            self.game.gold = 40_000_000;
            let shop = game::defs::shop_order();
            let mut n = 0usize;
            for slot in 0..self.game.board.slots.len() {
                if slot % 4 != 0 {
                    continue;
                }
                self.game.build_choice = Some((shop[n % shop.len()], 1));
                if self.game.try_build(slot) {
                    let ti = self.game.towers.len() - 1;
                    for _ in 0..level {
                        let before = self.game.towers[ti].def;
                        // At a fork, take the first branch: the demo wants a
                        // played-in board, not a considered one.
                        let choices = self.game.upgrade_choices(ti);
                        match choices.first() {
                            Some(&(into, _)) => self.game.upgrade_into(ti, into),
                            None => break,
                        }
                        // A loop that cannot advance is a hang.
                        if self.game.towers[ti].def == before {
                            break;
                        }
                    }
                    n += 1;
                }
            }
            self.game.build_choice = None;
            self.game.selected = None;
            self.game.gold = 30_000;
            self.menu.screen = Screen::Playing;
            self.game.send_wave();
        }
    }

    /// Scrolling and zooming, the way Warcraft III does it.
    ///
    /// Four ways in, and all of them end up as one drag measured in fractions
    /// of the viewport: the keys, the cursor resting against the edge of the
    /// board, the middle mouse button, and the wheel for the zoom. Going
    /// through the rig rather than adding tiles directly is what makes a drag
    /// keep the same patch of ground under the cursor at every zoom, and makes
    /// a key press scroll by the same fraction of the screen whether the
    /// camera is close in or right out.
    ///
    /// The clamp at the end is the whole reason this is not just an addition:
    /// the player should never be able to drift beyond the compact arena.
    fn camera_input(&mut self, resp: &egui::Response, rect: Rect, rig: &Rig, k: &Keys, dt: f32) {
        let mut scroll = [k.pan[0], k.pan[1]];
        let mut drag = [0.0f32, 0.0];

        if let Some(p) = resp.hover_pos() {
            // Horizontal edge scroll stays on the board, where it cannot fire
            // while the player is reading the side panels.
            scroll[0] += edge_scroll_axis(rect.left(), rect.right(), p.x);

            let wheel = resp.ctx.input(|i| i.smooth_scroll_delta.y);
            if wheel != 0.0 {
                // Geometric, so a notch feels the same at every zoom rather
                // than crawling when close in and jumping when far out.
                self.span *= (-wheel * CAM_WHEEL).exp();
            }
        }

        // The command bar covers the board's bottom edge and the status bar
        // covers its top edge. Looking only at `resp.hover_pos()` therefore
        // made vertical edge scrolling stop exactly where an RTS player moves
        // the cursor. Use the literal viewport edge for Y; the narrow 14-point
        // strip sits outside the actual controls, and holding a mouse button
        // suppresses it so clicking the HUD never drags the battlefield.
        let (pointer, button_down) = resp.ctx.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.primary_down() || i.pointer.secondary_down() || i.pointer.middle_down(),
            )
        });
        if !button_down && let Some(p) = pointer {
            let viewport = resp.ctx.content_rect();
            scroll[1] += edge_scroll_axis(viewport.top(), viewport.bottom(), p.y);
        }

        // A drag that started on the board keeps working past the edge of it,
        // which is what a player throwing the view across expects.
        if resp.dragged_by(egui::PointerButton::Middle) {
            let d = resp.drag_delta();
            drag[0] += d.x / rect.width().max(1.0);
            drag[1] += d.y / rect.height().max(1.0);
        }

        // Scrolling is dragging backwards: the keys and the screen edge move
        // the camera, a drag moves the ground under it. The direction is
        // capped at one so that a key held down while the cursor also rests on
        // the edge does not scroll at twice the speed.
        for (d, s) in drag.iter_mut().zip(scroll) {
            *d -= s.clamp(-1.0, 1.0) * CAM_PAN_SPEED * dt;
        }
        let step = rig.drag(self.span, drag[0], drag[1]);
        self.pan = [self.pan[0] + step[0], self.pan[1] + step[1]];

        let view = crate::game::greentd_map::VIEW;
        self.span = rig.clamp_span(self.span, CAM_SPAN_MIN, view);
        self.pan = rig.clamp_pan(self.pan, self.span, view, self.pads);
    }

    /// Board interaction. The cursor is cast as a ray onto the ground plane,
    /// then snapped to the nearest build pad.
    fn board_input(&mut self, resp: &egui::Response, rect: Rect, cam: &Camera, shift: bool) {
        let g = &mut self.game;
        g.hover_slot = None;

        if let Some(p) = resp.hover_pos() {
            let u = (p.x - rect.left()) / rect.width().max(1.0);
            let v = (p.y - rect.top()) / rect.height().max(1.0);
            if let Some(w) = cam.ground_pick(u, v) {
                g.hover_slot = g.board.slot_at(w);
            }
        }

        if resp.secondary_clicked() {
            g.build_choice = None;
            g.selected = None;
        }

        if resp.clicked() {
            match g.hover_slot {
                Some(slot) => {
                    if let Some(ti) = g.tower_in_slot(slot) {
                        // Clicking an existing tower always inspects it.
                        g.selected = Some(ti);
                        g.build_choice = None;
                    } else if g.build_choice.is_some() {
                        let built = g.try_build(slot);
                        if built && !shift {
                            g.build_choice = None;
                        }
                    }
                }
                None => g.selected = None,
            }
        }
    }

    fn apply_keys(&mut self, k: &Keys) {
        if k.help {
            self.ust.show_help = !self.ust.show_help;
        }
        if k.bloom {
            // B cycles the quality preset, and stops auto-tuning fighting it.
            self.ust.quality = self.ust.quality.raise().unwrap_or(Quality::Performance);
            self.ust.quality_dirty = true;
            self.ust.auto_quality = false;
        }
        let g = &mut self.game;
        if k.pause {
            g.paused = !g.paused;
        }
        if k.speed {
            g.cycle_speed();
        }
        if k.send {
            g.send_wave();
        }
        if k.cancel {
            g.build_choice = None;
            g.selected = None;
            self.ust.show_help = false;
        }
        if k.upgrade {
            if let Some(ti) = g.selected {
                g.upgrade(ti);
            }
        }
        if k.sell {
            if let Some(ti) = g.selected {
                g.sell(ti);
            }
        }
        if k.dev_gold {
            g.gold += 5_000;
        }
        if k.dev_fill {
            // Scatter a playable board of towers, for testing the view quickly.
            let mut n = 0usize;
            for slot in 0..g.board.slots.len() {
                if g.board.slots[slot].tower.is_some() || slot % 3 != 0 {
                    continue;
                }
                let shop = game::defs::shop_order();
                g.build_choice = Some((shop[n % shop.len()], 1));
                g.try_build(slot);
                n += 1;
            }
            g.build_choice = None;
            g.selected = None;
        }
        for (n, pressed) in k.digits.iter().enumerate() {
            if *pressed {
                if let Some(&def) = self.ust.hotkeys.get(n) {
                    g.build_choice = Some((def, 1));
                    g.selected = None;
                }
            }
        }
    }

    /// How long to wait before drawing the next frame.
    fn frame_budget(&self, dt: f32) -> std::time::Duration {
        let target = 1.0 / self.ust.fps_cap.max(15) as f32;
        let wait = (target - dt.min(target)).clamp(0.0, target);
        std::time::Duration::from_secs_f32(wait)
    }

    /// Holds the frame rate to [`UiState::fps_cap`].
    ///
    /// `request_repaint_after` alone does not do it: it sets a deadline, and
    /// anything else that asks for a repaint sooner wins, so on a 180 Hz
    /// display the game happily ran at 180 fps and threw two thirds of that
    /// work away - heat, fan noise and battery for pixels nobody sees.
    ///
    /// Only native needs this. In a browser the frame loop is already driven by
    /// `requestAnimationFrame`, which is capped to the display refresh.
    #[cfg(not(target_arch = "wasm32"))]
    fn throttle(&mut self) {
        let target = std::time::Duration::from_secs_f32(1.0 / self.ust.fps_cap.max(15) as f32);
        let now = std::time::Instant::now();
        if let Some(next) = self.next_frame {
            if let Some(wait) = next.checked_duration_since(now) {
                // A frame is 16 ms; oversleeping by a millisecond is not worth
                // burning a core to avoid, so this is a plain sleep.
                std::thread::sleep(wait);
            }
        }
        // Schedule from the deadline, not from now, so a slow frame does not
        // permanently shift the cadence.
        self.next_frame = Some(match self.next_frame {
            Some(prev) if now.duration_since(prev) < target => prev + target,
            _ => now + target,
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn throttle(&mut self) {}

    /// Tunes the quality preset to the machine, in both directions.
    ///
    /// Sustained slow frames step it down; sustained fast ones step it back up,
    /// but never above [`UiState::quality_ceiling`], which drops the first time
    /// a preset is found wanting. Without that ceiling a machine sitting right
    /// on the boundary would flip between two presets forever, rebuilding
    /// pipelines each time - which is itself a stall.
    fn auto_quality(&mut self) {
        if !self.ust.auto_quality {
            return;
        }
        if self.fps < 50.0 {
            self.ust.slow_frames += 1;
            self.ust.fast_frames = 0;
        } else {
            self.ust.slow_frames = self.ust.slow_frames.saturating_sub(2);
            if self.fps > 58.0 {
                self.ust.fast_frames += 1;
            }
        }
        if self.ust.slow_frames > 150 {
            self.ust.slow_frames = 0;
            self.ust.fast_frames = 0;
            self.ust.quality_ceiling = self.ust.quality;
            if let Some(q) = self.ust.quality.lower() {
                self.ust.quality = q;
                self.ust.quality_dirty = true;
                self.game
                    .toast(format!("Graphics lowered to {}", q.label()));
            } else {
                self.ust.auto_quality = false;
            }
        }
        // Climbing needs a much longer run of good frames than falling does:
        // dropping a preset costs a little fidelity, raising one that cannot be
        // sustained costs the player a visible stutter.
        if self.ust.fast_frames > 600 {
            self.ust.fast_frames = 0;
            if let Some(q) = self.ust.quality.raise() {
                if q <= self.ust.quality_ceiling {
                    self.ust.quality = q;
                    self.ust.quality_dirty = true;
                    self.game.toast(format!("Graphics raised to {}", q.label()));
                }
            }
        }
    }

    /// The title screen and lobby. The board is still drawn underneath, idling
    /// with its scenery animating, so the menu sits on a living scene instead of
    /// a black rectangle - and so the renderer is already warm when play starts.
    fn menu_frame(&mut self, ui: &mut egui::Ui, dt: f32) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let rect = ui.max_rect();
                if !menu::paint_backdrop(&ctx, ui.painter(), rect) {
                    let ppp = ctx.pixels_per_point().min(MAX_SCENE_DPR);
                    let px = [
                        (rect.width() * ppp).round().max(8.0) as u32,
                        (rect.height() * ppp).round().max(8.0) as u32,
                    ];
                    let camera = Camera::frame_rect(
                        crate::game::greentd_map::VIEW,
                        rect.width() / rect.height().max(1.0),
                        CAM_PITCH_DEG.to_radians(),
                        // A slow orbit, so the fallback is not a still frame.
                        CAM_YAW_DEG.to_radians() + (self.anim * 0.06).sin() * 0.22,
                        CAM_ZOOM * 1.04,
                    );
                    self.draw.clear();
                    view::draw_scene(&self.game, &self.decor, &mut self.draw, self.anim);
                    self.spawns.clear();
                    self.publish_frame(camera, px, dt);
                    ui.painter()
                        .add(egui_wgpu::Callback::new_paint_callback(rect, BoardCallback));
                }
            });

        match menu::show(&ctx, &mut self.menu, &mut self.net, dt) {
            menu::Action::SinglePlayer(difficulty) => {
                self.net.leave();
                self.game.start_run_with_difficulty(seed_now(), difficulty);
                save::clear();
                self.menu.screen = Screen::Playing;
            }
            menu::Action::Resume => {
                self.net.leave();
                match save::load().map(|s| s.restore(&mut self.game)) {
                    Some(true) => self.menu.screen = Screen::Playing,
                    _ => {
                        // The save was unreadable. Say so rather than silently
                        // dropping the player into a fresh run they did not ask
                        // for.
                        save::clear();
                        self.menu.saved = None;
                        self.game.error("That saved run could not be read");
                    }
                }
            }
            menu::Action::Cancelled => {
                self.game.restart();
            }
            menu::Action::None => {}
        }
        // The title screen is a slow orbit over a still board; half rate is
        // indistinguishable and costs half as much.
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }

    /// Hands this frame's geometry to the render callback without copying.
    fn publish_frame(&mut self, camera: Camera, px: [u32; 2], dt: f32) {
        let light = shadow_view_proj(BW, BH, LIGHT_DIR);
        let mut w = self.rs.renderer.write();
        let Some(gs) = w.callback_resources.get_mut::<GpuState>() else {
            return;
        };
        std::mem::swap(&mut gs.frame.list, &mut self.draw);
        std::mem::swap(&mut gs.frame.spawns, &mut self.spawns);
        gs.frame.camera = camera;
        gs.frame.light_view_proj = light;
        gs.frame.px = px;
        gs.frame.dt = dt;
        if self.ust.quality_dirty {
            gs.renderer.set_quality(&self.rs.device, self.ust.quality);
            self.ust.quality_dirty = false;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt).clamp(1.0 / 240.0, 0.1);
        self.anim += dt;
        self.fps += (1.0 / dt - self.fps) * 0.05;

        // A phone-sized viewport gets the compact HUD and the cheap preset.
        let view_w = ui.max_rect().width();
        self.ust.compact = ui::compact_for(view_w);
        if !self.sized_once {
            self.sized_once = true;
            if self.ust.compact || ctx.pixels_per_point() > 2.0 {
                self.ust.quality = Quality::Performance;
                self.ust.quality_dirty = true;
            }
        }

        // --- network: drain the socket before anything reads its state
        if let Some(net::Event::Started { seed, difficulty }) = self.net.poll() {
            self.game
                .start_run_with_difficulty(seed, crate::game::Difficulty::from_u8(difficulty));
            self.menu.screen = Screen::Playing;
        }
        self.ust.online = self.net.is_online();

        // Browser QA needs a semantic readiness signal. A changed screenshot
        // is not enough: the boot splash disappearing into a black swapchain is
        // also a changed screenshot. This marker lets the real Edge smoke test
        // wait for the title frame and prove that its click entered gameplay.
        #[cfg(target_arch = "wasm32")]
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            let screen = match self.menu.screen {
                Screen::Title => "title",
                Screen::Connect => "connect",
                Screen::Lobby => "lobby",
                Screen::Playing => "playing",
            };
            let _ = root.set_attribute("data-green-td-screen", screen);
        }

        // --- menu: the board keeps rendering behind it as a live backdrop
        if self.menu.screen != Screen::Playing {
            self.menu_frame(ui, dt);
            return;
        }
        if self.ust.want_menu {
            self.ust.want_menu = false;
            if !self.net.is_online() {
                save::store(&self.game);
            }
            self.net.leave();
            self.menu.saved = save::load();
            self.menu.screen = Screen::Title;
            self.game.restart();
            return;
        }

        let keys = read_keys(ui);
        self.apply_keys(&keys);
        self.auto_quality();

        // --- simulate
        let t_frame = now_ms();
        self.game.update(dt);
        self.net.push(self.game.snapshot(), dt);
        if std::mem::take(&mut self.game.wants_save) {
            // Solo runs only: a room's run belongs to the room, and resuming
            // into one nobody else is playing any more would be a lie.
            if !self.net.is_online() {
                save::store(&self.game);
            }
        }
        Profile::feed(&mut self.prof.sim, now_ms() - t_frame);

        // --- HUD
        self.ust.perf_ticks += 1;
        if self.ust.perf_ticks % 15 == 0 {
            self.ust.perf = format!(
                "{:>3.0} fps  {:>5} inst  {:>4} creeps",
                self.fps,
                self.draw.len(),
                self.game.creeps.len()
            );
            #[cfg(not(target_arch = "wasm32"))]
            if std::env::var("TD_PROFILE").is_ok() {
                println!("{}  |  {}", self.ust.perf, self.prof.line());
            }
        }
        let perf = self.ust.perf.clone();
        let t_hud = now_ms();
        egui::Panel::top("hud")
            .exact_size(ui::top_h(self.ust.compact))
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(ui::pal::PANEL)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ui, |ui| {
                ui::top_bar(&mut self.game, ui, &mut self.ust, &perf);
            });

        egui::Panel::bottom("shop")
            .exact_size(ui::command_h(self.ust.compact))
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(ui::pal::PANEL)
                    .inner_margin(egui::Margin::symmetric(10, 8)),
            )
            .show(ui, |ui| {
                ui::command_bar(&mut self.game, ui, &mut self.ust);
            });

        // --- board
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let (rect, resp) =
                    ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                // The browser reports devicePixelRatio here, which on a 2x display
                // asks for four times the pixels. The HUD stays crisp at native
                // scale; the 3D scene does not need to be supersampled.
                let ppp = ctx.pixels_per_point().min(MAX_SCENE_DPR);
                let px = [
                    (rect.width() * ppp).round().max(8.0) as u32,
                    (rect.height() * ppp).round().max(8.0) as u32,
                ];
                let rig = Rig::new(
                    rect.width() / rect.height().max(1.0),
                    CAM_PITCH_DEG.to_radians(),
                    CAM_YAW_DEG.to_radians(),
                );
                self.camera_input(&resp, rect, &rig, &keys, dt);
                // One camera drives rendering, picking and the text overlay, so
                // they can never disagree about where something is on screen.
                // That is why the scrolling is settled first: the ray a click
                // is cast along has to come from the frame the player is
                // looking at, not the one before it.
                let camera = rig.camera(self.pan, self.span);

                self.board_input(&resp, rect, &camera, keys.shift);

                let t_build = now_ms();
                self.draw.clear();
                view::draw_scene(&self.game, &self.decor, &mut self.draw, self.anim);
                self.spawns.clear();
                self.spawns.append(&mut self.game.fx.particles);
                Profile::feed(&mut self.prof.build, now_ms() - t_build);

                self.publish_frame(camera, px, dt);
                ui.painter()
                    .add(egui_wgpu::Callback::new_paint_callback(rect, BoardCallback));

                ui::board_text(&self.game, ui, &camera, rect);
                ui::board_hover(&self.game, &resp, &camera, rect);
            });

        menu::room_scoreboard(&ctx, &self.net, self.ust.compact);
        ui::modals(&mut self.game, &ctx, &mut self.ust);
        // Drain after board input and modal actions as well as simulation, so
        // a build click and an automatic wave boundary both sound this frame.
        if self.ust.sound_enabled {
            self.audio.play_cues(&mut self.game.sound_cues);
        } else {
            self.game.sound_cues.clear();
        }
        Profile::feed(
            &mut self.prof.hud,
            now_ms() - t_hud - self.prof.build as f64,
        );
        Profile::feed(&mut self.prof.total, now_ms() - t_frame);

        // Games animate constantly, but there is no point drawing frames the
        // display will never show. Uncapped, this ran at 180 fps on a 60 Hz
        // screen - two thirds of the GPU work, the fan noise and the battery
        // went straight in the bin. Asking for the next frame at a deadline
        // instead of "immediately" is the single biggest saving available.
        self.throttle();
        ctx.request_repaint_after(self.frame_budget(dt));
    }
}

/// Where the camera opens, in tiles: a point on the lane, near the spawn.
///
/// Two wrong answers preceded this one, and both are instructive.
///
/// The middle of the arena rectangle put the camera in the open with the
/// fighting off at the edge of frame. So it was changed to the centroid of the
/// lane - which is correct for a lane that runs *through* the field, and
/// nonsense for this one: the map's lane is a closed ring, and the centroid of
/// a ring is the hollow middle of it. That framing opened the game on sixty
/// tiles of empty grass with the entire circuit off-screen in every direction.
///
/// A point *on* the lane has neither failure. This is the first waypoint, which
/// is the corner the map's own Red player feeds creeps into.
pub fn lane_middle() -> [f32; 2] {
    let lap = game::greentd_map::LAP;
    match (lap.first(), lap.get(1)) {
        // Start a few tiles beyond the portal. The gate remains in frame as a
        // landmark without sitting over the exact point the player needs to
        // read, select and build around.
        (Some(a), Some(b)) => {
            let on_lane = [a[0] + (b[0] - a[0]) * 0.18, a[1] + (b[1] - a[1]) * 0.18];
            let v = game::greentd_map::VIEW;
            let centre = [(v[0] + v[2]) * 0.5, (v[1] + v[3]) * 0.5];
            // Bias a few tiles into the arena. The gate stays in view, but the
            // board—not empty space beyond its wall—owns the opening frame.
            [
                on_lane[0] + (centre[0] - on_lane[0]) * 0.12,
                on_lane[1] + (centre[1] - on_lane[1]) * 0.12,
            ]
        }
        (Some(p), None) => *p,
        _ => {
            let v = game::greentd_map::VIEW;
            [(v[0] + v[2]) * 0.5, (v[1] + v[3]) * 0.5]
        }
    }
}

/// The rectangle that holds every build pad, in tiles.
///
/// Read off the board rather than assumed from the arena, so that if the plots
/// ever move the camera's idea of where the player needs to be able to look
/// moves with them.
pub fn pad_bounds(board: &game::board::Board) -> [f32; 4] {
    let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for s in &board.slots {
        r[0] = r[0].min(s.pos[0]);
        r[1] = r[1].min(s.pos[1]);
        r[2] = r[2].max(s.pos[0]);
        r[3] = r[3].max(s.pos[1]);
    }
    r
}

/// A seed for a local run. There is no `getrandom` in the wasm build on
/// purpose, so this comes from the clock - good enough to vary a solo run, and
/// never used for a shared one (rooms take their seed from the server).
fn seed_now() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        let ms = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(1.0);
        (ms * 4096.0) as u64 ^ 0x9E37_79B9_7F4A_7C15
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x1234_5678)
            ^ 0x9E37_79B9_7F4A_7C15
    }
}

// ---------------------------------------------------------------- entry

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    eframe::run_native(
        "Green Circle TD",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1400.0, 900.0])
                .with_min_inner_size([900.0, 620.0])
                .with_title("Green Circle TD"),
            wgpu_options: high_performance_gpu(),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

/// Insist on the discrete GPU.
///
/// On a laptop with both an integrated and a discrete adapter, landing on the
/// integrated one is the difference between 20 fps and locked 60. The selector
/// is explicit rather than left to the power-preference hint because on Windows
/// the hint is advisory and a driver profile can quietly override it.
#[cfg(not(target_arch = "wasm32"))]
fn high_performance_gpu() -> egui_wgpu::WgpuConfiguration {
    use std::sync::Arc;
    let mut cfg = egui_wgpu::WgpuConfiguration::default();
    if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut cfg.wgpu_setup {
        setup.power_preference = wgpu::PowerPreference::HighPerformance;
        setup.native_adapter_selector = Some(Arc::new(|adapters, surface| {
            let usable: Vec<&wgpu::Adapter> = adapters
                .iter()
                .filter(|a| surface.is_none_or(|s| !s.get_capabilities(a).formats.is_empty()))
                .collect();
            let pick = |kind: wgpu::DeviceType| {
                usable
                    .iter()
                    .find(|a| a.get_info().device_type == kind)
                    .copied()
            };
            let chosen = pick(wgpu::DeviceType::DiscreteGpu)
                .or_else(|| pick(wgpu::DeviceType::IntegratedGpu))
                .or_else(|| usable.first().copied())
                .ok_or_else(|| "no usable GPU adapter".to_string())?;
            let info = chosen.get_info();
            log::info!(
                "GPU: {} ({:?}, {:?})",
                info.name,
                info.device_type,
                info.backend
            );
            println!(
                "GPU: {} ({:?}, {:?})",
                info.name, info.device_type, info.backend
            );
            Ok(chosen.clone())
        }));
    }
    cfg
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Warn).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id("gamecanvas")
            .expect("#gamecanvas missing")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        // Let the browser choose its proven presentation adapter. Forcing the
        // discrete adapter made some dual-GPU Edge installations initialise
        // successfully and then present a permanently black canvas: Edge's
        // compositor was running on a different adapter. Native builds can
        // safely make an explicit choice; a browser owns its compositor.
        let mut web = eframe::WebOptions::default();
        if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut web.wgpu_options.wgpu_setup {
            setup.power_preference = browser_power_preference();
        }
        let result = eframe::WebRunner::new()
            .start(canvas, web, Box::new(|cc| Ok(Box::new(App::new(cc)))))
            .await;
        match result {
            Ok(()) => {
                if let Some(el) = document.get_element_by_id("boot") {
                    // Keep the node available for the page-level device-loss
                    // handler. Removing it turned every later GPU error into
                    // an unexplained black canvas.
                    let _ = el.set_attribute("style", "display:none");
                }
            }
            Err(e) => {
                // Never replace a useful loading screen with an unexplained
                // black canvas. Browsers can deny WebGPU/WebGL for driver or
                // policy reasons; make that failure actionable on the page.
                if let Some(el) = document.get_element_by_id("boot") {
                    el.set_text_content(Some(&format!(
                        "Green Circle TD could not start its graphics renderer.\n\n{e:?}\n\nEnable hardware acceleration or try an updated Chrome, Edge, or Firefox."
                    )));
                }
                log::error!("failed to start: {e:?}");
            }
        }
    });
}
