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
use egui::{Key, PointerButton, Rect, Sense};

use decor::Decor;
use game::Game;
use game::board::{BH, BW};
#[cfg(target_arch = "wasm32")]
use game::board::TOWER_FOOTPRINT_RADIUS;
use game::fx::ParticleSpawn;
use gfx::Quality;
use gfx::Renderer;
use gfx::draw::DrawList;
use math::{Camera, Mat4, Rig, shadow_view_proj};
#[cfg(target_arch = "wasm32")]
use math::v3;
use menu::{MenuState, Screen};
use net::Net;

/// How far the title-screen camera tilts down towards the board.
pub const CAM_PITCH_DEG: f32 = 55.0;
/// Keep route turns aligned to the tactical frame. The fixed pitch already
/// exposes real tower sides and creature bodies; an oblique yaw had to zoom
/// too far out to retain every corner of the square route on ultrawide screens,
/// making the actual battle smaller. Depth belongs in the camera pitch,
/// materials and geometry, not in sacrificing useful battle scale.
pub const CAM_YAW_DEG: f32 = 0.0;
/// How tightly the title screen's backdrop frames the map.
pub const CAM_ZOOM: f32 = 1.15;
/// A deliberately oblique playable camera. Forty-seven degrees exposes the
/// sides of tower bodies, creature shoulders, trunks and grounded shadows
/// while still keeping the full route legible from the reset overview.
pub const PLAY_CAM_PITCH_DEG: f32 = 47.0;
/// Tightest useful free-camera span in terrain tiles.  It exposes real tower
/// construction without becoming a disorienting first-person camera.
pub const PLAY_CAM_TIGHTEST_SPAN: f32 = 7.2;
/// Space reserved for safe edges and touch/browser chrome; the game otherwise
/// uses the viewport rather than pretending a large display is unavailable.
pub const STAGE_PAD: f32 = 12.0;
/// Human-readable runtime provenance. This is intentionally exposed on the
/// browser root so a screenshot can identify its loaded WASM build instead of
/// relying on an ambiguous dev-server port or a stale tab.
/// Stamped into the DOM at startup so isolated browser evidence can prove the
/// exact WASM bundle it exercised rather than guessing from a server port.
pub const BUILD_ID: &str = "woodland-command-tempo-20260913.100";
/// Ceiling on how many device pixels the 3D scene is rendered at per point.
///
/// The HUD stays crisp at the display's real scale; the 3D scene does not need
/// to be supersampled, and on a 2x display the difference between 1.0 and 1.35
/// here is meaningful for fine tower silhouettes. Balanced quality clamps at
/// two device pixels per CSS point; the quality governor still protects frame
/// pacing on constrained adapters.
const MAX_SCENE_DPR: f32 = 2.0;
/// Key light direction, shared by the shader and the shadow camera.
const LIGHT_DIR: [f32; 3] = [-0.40, -0.52, 0.76];

/// Browsers own the presentation compositor, so they must also choose the
/// adapter that can present to it. Forcing the discrete GPU can create a valid
/// WebGPU device on one adapter while Edge presents the canvas on another,
/// leaving a permanently black canvas after the first custom render callback.
fn browser_power_preference() -> wgpu::PowerPreference {
    wgpu::PowerPreference::None
}

/// Reset/shot camera. Interactive play uses [`BattleView`] below, but still
/// starts at this exact fit-to-route overview rather than a different camera.
pub fn play_camera(aspect: f32) -> Camera {
    let rig = Rig::new(aspect, PLAY_CAM_PITCH_DEG.to_radians(), CAM_YAW_DEG.to_radians());
    rig.camera(lane_middle(), rig.widest_span(game::greentd_map::VIEW))
}

/// The 3D callback owns the entire usable central rectangle. The tactical
/// world remains square in world coordinates and the camera fits it into this
/// wide/portrait frustum; only the retired render target was square.
pub fn scene_rect(area: Rect) -> Rect {
    area
}

/// The fixed square play rectangle. The world never stretches, but it is sized
/// from the usable viewport rather than an arbitrary desktop-stage cap.
pub fn fixed_board_rect(area: Rect, beside_dock: bool) -> Rect {
    let dock = if beside_dock { ui::COMMAND_DOCK_W } else { 0.0 };
    let side = area.height().min((area.width() - dock).max(0.0)).max(0.0);
    let stage_w = side + dock;
    let x = area.left() + (area.width() - stage_w).max(0.0) * 0.5;
    Rect::from_min_size(
        egui::pos2(x, area.top() + (area.height() - side) * 0.5),
        egui::vec2(side, side),
    )
}

/// The right rail always shares the board's exact height and touches its edge.
pub fn command_dock_rect(board: Rect) -> Rect {
    command_dock_rect_with_width(board, ui::COMMAND_DOCK_W)
}

/// The rail is a deliberate command instrument, not an elastic spacer.  In
/// particular, a wide browser must not turn the extra aspect-ratio remainder
/// into a six-hundred-pixel dashboard and make the actual battle look like a
/// postage stamp.  The whole square board and its fixed-width rail therefore
/// form one centred tactical stage; the remaining browser area is symmetrical
/// quiet framing, while the top HUD continues to use the full viewport.
pub fn command_dock_rect_with_width(board: Rect, width: f32) -> Rect {
    Rect::from_min_size(
        board.right_top(),
        egui::vec2(width.max(ui::COMMAND_DOCK_W), board.height()),
    )
}

/// Desktop composition: a full-width HUD and the largest square that remains
/// beside a responsive rail. Surplus belongs to live command information, not
/// a centred fake application window.
#[derive(Clone, Copy)]
pub struct DesktopStage {
    pub hud: Rect,
    pub board: Rect,
    pub dock: Rect,
}

pub fn desktop_stage(viewport: Rect) -> DesktopStage {
    let usable_w = (viewport.width() - STAGE_PAD * 2.0).max(0.0);
    let side = (viewport.height() - ui::TOP_H - STAGE_PAD * 2.0)
        .min(usable_w - ui::COMMAND_DOCK_W)
        .max(0.0);
    let hud = Rect::from_min_size(
        egui::pos2(viewport.left(), viewport.top()),
        egui::vec2(viewport.width(), ui::TOP_H),
    );
    let stage_w = side + ui::COMMAND_DOCK_W;
    let stage_left = viewport.left() + (viewport.width() - stage_w).max(0.0) * 0.5;
    let board = Rect::from_min_size(
        egui::pos2(stage_left, hud.bottom() + STAGE_PAD),
        egui::vec2(side, side),
    );
    DesktopStage {
        hud,
        board,
        dock: command_dock_rect(board),
    }
}

/// The desktop HUD is another fixed piece of the stage, not a full-browser
/// bar. It is deliberately plain so gold, pressure and the next action win.
pub fn desktop_top_bar_area(
    ctx: &egui::Context,
    rect: Rect,
    game: &mut Game,
    ust: &mut ui::UiState,
    perf: &str,
) {
    egui::Area::new(egui::Id::new("hud"))
        .fixed_pos(rect.left_top())
        .default_size(rect.size())
        .movable(false)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            let outer = ui.max_rect();
            ui.painter().rect_filled(outer, 0.0, ui::pal::PANEL);
            let inner = outer.shrink2(egui::vec2(10.0, 4.0));
            let mut content = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            ui::top_bar(game, &mut content, ust, perf);
        });
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

    #[test]
    fn smallest_desktop_stage_can_contain_the_entire_command_rail() {
        let viewport = Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(ui::COMPACT_WIDTH, ui::MIN_DESKTOP_DOCK_H),
        );
        assert!(
            !ui::compact_for_view(viewport.width(), viewport.height()),
            "the declared desktop breakpoint unexpectedly uses compact chrome"
        );
        let stage = desktop_stage(viewport);
        assert!(
            stage.board.height() + 0.01 >= ui::MIN_DESKTOP_BOARD,
            "board {:?} cannot hold the {}px rail",
            stage.board,
            ui::MIN_DESKTOP_BOARD
        );
        assert_eq!(stage.board.height(), stage.dock.height());
        assert!((stage.board.left() - STAGE_PAD).abs() < 0.01);
        assert!((stage.dock.right() - (viewport.right() - STAGE_PAD)).abs() < 0.01);
    }

    #[test]
    fn wide_desktop_keeps_the_command_rail_compact_and_centres_the_stage() {
        let viewport = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1414.0, 860.0));
        let stage = desktop_stage(viewport);
        assert!((stage.dock.width() - ui::COMMAND_DOCK_W).abs() < 0.01);
        assert!((stage.board.left() - (viewport.width() - stage.board.width() - stage.dock.width()) * 0.5).abs() < 0.01);
        assert!((viewport.right() - stage.dock.right() - stage.board.left()).abs() < 0.01);
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
    reset_view: bool,
    send: bool,
    cancel: bool,
    upgrade: bool,
    sell: bool,
    help: bool,
    bloom: bool,
    digits: [bool; 11],
    /// Debug builds only: fill pads / grant gold, for playtesting.
    dev_fill: bool,
    dev_gold: bool,
    persist: bool,
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
        Keys {
            pause: i.key_pressed(Key::Space),
            speed: i.key_pressed(Key::F),
            reset_view: i.key_pressed(Key::R),
            send: i.key_pressed(Key::Enter),
            cancel: i.key_pressed(Key::Escape),
            upgrade: i.key_pressed(Key::U),
            sell: i.key_pressed(Key::S),
            help: i.key_pressed(Key::H),
            bloom: i.key_pressed(Key::B),
            digits,
            dev_fill: cfg!(debug_assertions) && i.key_pressed(Key::T),
            dev_gold: cfg!(debug_assertions) && i.key_pressed(Key::G),
            persist: i.key_pressed(Key::F7),
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

/// Mutable, bounded player view over the fixed tactical world.  The terrain
/// and route never move; only this camera target/span changes after a deliberate
/// wheel, pinch or pan gesture. Keeping it separate from `Game` guarantees
/// camera interaction cannot alter wave clocks, simulation state or saves.
#[derive(Clone, Copy, Debug)]
struct BattleView {
    centre: [f32; 2],
    span: f32,
    initialized: bool,
    reset_requested: bool,
    /// Set by a touch drag/pinch until its release click has been consumed.
    /// This is what prevents a gesture ending on grass from buying a tower.
    suppress_primary: bool,
}

impl Default for BattleView {
    fn default() -> Self {
        Self {
            centre: lane_middle(),
            span: 0.0,
            initialized: false,
            reset_requested: false,
            suppress_primary: false,
        }
    }
}

impl BattleView {
    fn rig(rect: Rect) -> Rig {
        Rig::new(
            rect.width() / rect.height().max(1.0),
            PLAY_CAM_PITCH_DEG.to_radians(),
            CAM_YAW_DEG.to_radians(),
        )
    }

    fn reset(&mut self, rig: &Rig) {
        self.centre = lane_middle();
        self.span = rig.widest_span(game::greentd_map::VIEW);
        self.initialized = true;
        self.reset_requested = false;
    }

    fn clamp(&mut self, rig: &Rig) {
        let bounds = game::board::BUILD_WORLD;
        self.span = rig.clamp_span(self.span, PLAY_CAM_TIGHTEST_SPAN, game::greentd_map::VIEW);
        self.centre = rig.clamp_pan(self.centre, self.span, bounds, bounds);
    }

    fn camera(&mut self, rect: Rect) -> Camera {
        let rig = Self::rig(rect);
        if !self.initialized || self.reset_requested {
            self.reset(&rig);
        }
        self.clamp(&rig);
        rig.camera(self.centre, self.span)
    }

    fn zoom_at(&mut self, rig: &Rig, camera: &Camera, rect: Rect, pointer: egui::Pos2, factor: f32) {
        let u = (pointer.x - rect.left()) / rect.width().max(1.0);
        let v = (pointer.y - rect.top()) / rect.height().max(1.0);
        let before = camera.ground_pick(u, v);
        self.span = rig.clamp_span(
            self.span / factor.clamp(0.78, 1.28),
            PLAY_CAM_TIGHTEST_SPAN,
            game::greentd_map::VIEW,
        );
        if let (Some(before), Some(after)) = (
            before,
            rig.camera(self.centre, self.span).ground_pick(u, v),
        ) {
            // Move the target by the exact ray-plane difference so the grass
            // under a wheel/pinch midpoint stays under that midpoint.
            self.centre[0] += before[0] - after[0];
            self.centre[1] += before[1] - after[1];
        }
        self.clamp(rig);
    }

    fn pan_by(&mut self, rig: &Rig, rect: Rect, delta: egui::Vec2) {
        if delta == egui::Vec2::ZERO {
            return;
        }
        let shift = rig.drag(
            self.span,
            delta.x / rect.width().max(1.0),
            delta.y / rect.height().max(1.0),
        );
        self.centre[0] += shift[0];
        self.centre[1] += shift[1];
        self.clamp(rig);
    }

    /// Applies only navigation gestures that originate over the scene
    /// rectangle. Returns whether a primary click must be ignored this frame
    /// because it completed a touch gesture rather than a placement tap.
    fn navigate(
        &mut self,
        ctx: &egui::Context,
        resp: &egui::Response,
        rect: Rect,
        camera: &Camera,
    ) -> bool {
        let rig = Self::rig(rect);
        if self.reset_requested {
            self.reset(&rig);
        }

        let pointer = resp.hover_pos();
        if let Some(pointer) = pointer.filter(|p| rect.contains(*p)) {
            let scroll = ctx.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > 0.01 {
                // Wheel ticks and trackpads report very different magnitudes;
                // exponential scaling keeps both smooth and bounded.
                self.zoom_at(&rig, camera, rect, pointer, (scroll * 0.0022).exp());
            }
        }

        let mut touch_gesture = false;
        if let Some(touch) = ctx.input(|input| input.multi_touch())
            && rect.contains(touch.center_pos)
        {
            touch_gesture = true;
            self.suppress_primary = true;
            self.zoom_at(&rig, camera, rect, touch.center_pos, touch.zoom_delta);
            self.pan_by(&rig, rect, touch.translation_delta);
        }

        // Middle drag is deliberately the desktop pan gesture, leaving a
        // normal left click unambiguous for build/selection. A one-finger
        // touch drag pans only after it has become a drag and the view is
        // zoomed in; a tap remains a placement tap.
        if resp.dragged_by(PointerButton::Middle) {
            self.pan_by(&rig, rect, resp.drag_delta());
        }
        let touch_active = ctx.input(|input| input.any_touches());
        let overview = rig.widest_span(game::greentd_map::VIEW);
        if touch_active
            && self.span < overview - 0.03
            && resp.dragged_by(PointerButton::Primary)
        {
            self.suppress_primary = true;
            self.pan_by(&rig, rect, resp.drag_delta());
        }

        let suppressed = self.suppress_primary || touch_gesture;
        // Keep suppression through the release frame: egui may emit the
        // primary click after the touch list has emptied.
        if self.suppress_primary && !touch_active && !ctx.input(|i| i.pointer.button_down(PointerButton::Primary)) {
            self.suppress_primary = false;
        }
        suppressed
    }
}

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
    /// When the next frame is due, for the native frame limiter.
    #[cfg(not(target_arch = "wasm32"))]
    next_frame: Option<std::time::Instant>,
    /// Whether the one-time viewport-dependent setup has run.
    sized_once: bool,
    view: BattleView,
    /// Opt-in URL-driven evidence state. This is never a player-facing cheat:
    /// it is accepted only after the normal Campaign menu action and the
    /// resulting run is never written to the real save envelope.
    campaign_diagnostic: Option<CampaignDiagnostic>,
    /// URL-only, non-saving art capture. Unlike transition diagnostics it does
    /// not use the instant-kill pilot: a granted mixed roster fights a live
    /// commander through ordinary simulation ticks.
    visual_showcase: bool,
}

/// One of the intentionally narrow, visibly labelled browser transition
/// traces. The pilot stops only after an actual target encounter has spawned,
/// leaving an inspectable live frame rather than racing past a number in a
/// table.
#[derive(Clone, Copy)]
struct CampaignDiagnostic {
    start: u16,
    target: u16,
    reached: bool,
}

fn requested_campaign_diagnostic() -> Option<CampaignDiagnostic> {
    #[cfg(target_arch = "wasm32")]
    {
        let search = web_sys::window()?.location().search().ok()?;
        let start = search
            .trim_start_matches('?')
            .split('&')
            .find_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                (key == "td_fixture").then_some(value)
            })
            .and_then(|value| value.parse::<u16>().ok())?;
        let target = Game::diagnostic_fixture_target(start)?;
        return Some(CampaignDiagnostic {
            start,
            target,
            reached: false,
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

fn requested_visual_showcase() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.location().search().ok())
            .is_some_and(|search| search.split('&').any(|pair| pair == "td_showcase=1" || pair == "?td_showcase=1"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    { false }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let rs = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu render state (the wgpu backend must be enabled)")
            .clone();

        #[cfg(target_arch = "wasm32")]
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            let _ = document.document_element().map(|root| {
                root.set_attribute(
                    "data-green-td-backend",
                    &format!("{:?}", rs.adapter.get_info().backend),
                )
            });
        }

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
            #[cfg(not(target_arch = "wasm32"))]
            next_frame: None,
            sized_once: false,
            view: BattleView::default(),
            campaign_diagnostic: requested_campaign_diagnostic(),
            visual_showcase: requested_visual_showcase(),
        };
        app.apply_demo_env();
        app
    }

    /// A diagnostic point for browser interaction checks. It deliberately
    /// searches for *visible* legal grass closest to the useful middle of the
    /// current frustum instead of publishing the first valid coordinate in a
    /// scan. At a close zoom that old point could sit on the bottom edge, where
    /// a wheel's final inertial frame made a correct pointer click look like an
    /// unreliable free-placement system.
    ///
    /// This never changes gameplay placement: players still aim directly at
    /// any grass. It only gives the isolated browser test a safe ordinary
    /// pointer coordinate after every camera/viewport change.
    #[cfg(target_arch = "wasm32")]
    fn visible_grass_probe(&self, camera: &Camera) -> Option<[f32; 2]> {
        let b = self.game.board.build_world();
        let mut best: Option<([f32; 2], f32)> = None;
        const CELLS: usize = 28;
        for gy in 0..=CELLS {
            let y = (b[1] + TOWER_FOOTPRINT_RADIUS)
                + (b[3] - b[1] - TOWER_FOOTPRINT_RADIUS * 2.0) * gy as f32 / CELLS as f32;
            for gx in 0..=CELLS {
                let x = (b[0] + TOWER_FOOTPRINT_RADIUS)
                    + (b[2] - b[0] - TOWER_FOOTPRINT_RADIUS * 2.0) * gx as f32 / CELLS as f32;
                let Ok(pos) = self.game.buildability_at([x, y], false) else {
                    continue;
                };
                let Some(screen) = camera.to_screen(v3(pos[0], pos[1], 0.0)) else {
                    continue;
                };
                // Keep this diagnostic coordinate away from the scene edge,
                // the top strip and the command console. It must be an actual
                // point a player could click, not merely a projected world
                // coordinate that happens to be numerically finite.
                if !(0.12..=0.88).contains(&screen[0]) || !(0.15..=0.82).contains(&screen[1]) {
                    continue;
                }
                let score = (screen[0] - 0.50).powi(2) + (screen[1] - 0.56).powi(2);
                if best.is_none_or(|(_, prior)| score < prior) {
                    best = Some((pos, score));
                }
            }
        }
        best.map(|(pos, _)| pos)
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

    /// A visibly labelled, non-persistent art fixture. It grants setup gold
    /// and moves to the final commander encounter, but never damages or clears
    /// enemies: every projectile/effect in its capture is production combat.
    fn start_visual_showcase(&mut self, difficulty: game::Difficulty) {
        self.game.start_campaign(seed_now(), difficulty);
        self.game.gold = 40_000_000;
        let shop = game::defs::shop_order();
        let slot_count = self.game.board.slots.len();
        for n in 0..slot_count {
            if n % 3 != 0 || n >= 30 { continue; }
            self.game.build_choice = Some((shop[n % shop.len()], 1));
            if self.game.try_build(n) {
                let ti = self.game.towers.len() - 1;
                for _ in 0..3 {
                    let Some(&(into, _)) = self.game.upgrade_choices(ti).first() else { break };
                    self.game.upgrade_into(ti, into);
                }
            }
        }
        self.game.build_choice = None;
        self.game.selected = None;
        if let Some(state) = self.game.campaign.as_mut() { state.encounter = 600; }
        self.game.wave = 599;
        self.game.phase = game::Phase::Build;
        self.game.prep = false;
        self.game.wave_timer = 0.0;
        self.game.speed = 10.0;
        self.game.campaign_pressure_grace = 1_000_000.0;
        self.game.wants_save = false;
        self.game.notice("SHOWCASE: granted roster / live commander / not saved".to_owned());
        self.game.send_wave();
    }

    /// Board interaction. The cursor is cast into the live camera's ground
    /// plane and stays in world coordinates: grass placement never falls back
    /// to an old nearest socket after a zoom, pan or viewport resize.
    fn board_input(
        &mut self,
        resp: &egui::Response,
        rect: Rect,
        cam: &Camera,
        suppress_primary: bool,
    ) {
        let g = &mut self.game;
        g.hover_slot = None;
        g.hover_pos = None;

        if let Some(p) = resp.hover_pos() {
            let u = (p.x - rect.left()) / rect.width().max(1.0);
            let v = (p.y - rect.top()) / rect.height().max(1.0);
            if let Some(w) = cam.ground_pick(u, v) {
                g.hover_pos = Some(w);
                // Compatibility only: older HUD hints can still identify a
                // historical pad, but the build path below never requires it.
                g.hover_slot = g.board.slot_at(w);
            }
        }

        if resp.secondary_clicked() {
            g.build_choice = None;
            g.selected = None;
        }

        if resp.clicked_by(PointerButton::Primary) && !suppress_primary {
            match g.hover_pos {
                Some(pos) if g.tower_at(pos).is_some() => {
                    // A real plinth always wins selection over a still-armed
                    // build card. This stays true for free grass positions.
                    g.selected = g.tower_at(pos);
                    g.build_choice = None;
                }
                Some(pos) if g.build_choice.is_some() => {
                    // Construction stays armed until cancel. A failed query
                    // cannot spend gold, and a successful click stores this
                    // actual world position instead of a pad ID.
                    g.try_build_at(pos);
                }
                _ => {
                    g.selected = None;
                }
            }
        }
    }

    fn apply_keys(&mut self, k: &Keys) {
        if self.ust.show_threat_intel {
            if k.cancel {
                self.ust.show_threat_intel = false;
                self.ust.threat_intel_rect = None;
            }
            return;
        }
        // The doctrine modal owns its choice input. Do not let keyboard
        // gameplay actions (notably Space) alter a frozen simulation behind
        // it; egui still receives the card input independently.
        if self.game.pending_doctrine {
            return;
        }
        if k.reset_view {
            self.view.reset_requested = true;
        }
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
        // Campaign's F ladder contains only rapid 10x/25x/50x/100x values;
        // Legacy keeps its separate historic tempo ladder.
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
        if k.persist && !self.net.is_online() {
            g.wants_save = true;
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
                    // A quiet automatic improvement is already visible in
                    // the top-strip quality label. A large cyan board toast
                    // was masking the opening horde in every fast browser
                    // capture, so reserve in-world notices for decisions and
                    // placement feedback instead.
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
            menu::Action::Campaign(difficulty) => {
                self.net.leave();
                let started_diagnostic = self.campaign_diagnostic.is_some_and(|fixture| {
                    self.game.start_campaign_diagnostic_fixture(
                        seed_now(),
                        difficulty,
                        fixture.start,
                    )
                });
                if self.visual_showcase {
                    self.start_visual_showcase(difficulty);
                } else if !started_diagnostic {
                    self.game.start_campaign(seed_now(), difficulty);
                }
                // A URL-driven evidence trace must never clear or replace a
                // player's ordinary Campaign/Legacy save. The isolated
                // browser checker owns a fresh profile, but this guard also
                // makes a copied diagnostic URL safe in a real browser.
                if !started_diagnostic && !self.visual_showcase {
                    save::clear();
                }
                self.view.reset_requested = true;
                self.menu.screen = Screen::Playing;
            }
            menu::Action::Legacy(difficulty) => {
                self.net.leave();
                self.game.start_run_with_difficulty(seed_now(), difficulty);
                save::clear();
                self.campaign_diagnostic = None;
                self.view.reset_requested = true;
                self.menu.screen = Screen::Playing;
            }
            menu::Action::Resume => {
                self.net.leave();
                match save::load().map(|s| s.restore(&mut self.game)) {
                    Some(true) => {
                        self.view.reset_requested = true;
                        self.menu.screen = Screen::Playing;
                        self.campaign_diagnostic = None;
                    }
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

/// A deliberately unmissable label for URL-driven Campaign evidence. It lives
/// above the 3D callback (not in its pixels), so its wording stays readable at
/// every DPR and cannot be mistaken for an ordinary player run in a capture.
fn diagnostic_banner(ctx: &egui::Context, diagnostic: CampaignDiagnostic, game: &Game) {
    let viewport = ctx.content_rect();
    let width = (viewport.width() - 24.0).clamp(180.0, 470.0);
    let x = (viewport.center().x - width * 0.5).max(8.0);
    let chapter = game.campaign_chapter().unwrap_or(0);
    let status = if diagnostic.reached {
        format!(
            "TARGET LIVE: C{chapter} E{} — PAUSED FOR INSPECTION",
            diagnostic.target
        )
    } else {
        format!(
            "PILOT RUNNING: E{} -> E{} THROUGH LIVE TRANSITIONS",
            diagnostic.start, diagnostic.target
        )
    };
    egui::Area::new(egui::Id::new("campaign_transition_diagnostic"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(x, ui::top_h(false) + 8.0))
        .show(ctx, |ui| {
            ui.set_width(width);
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(55, 35, 22))
                .stroke(egui::Stroke::new(1.0, ui::pal::GOLD))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("DIAGNOSTIC FIXTURE — NOT SAVED")
                            .strong()
                            .size(11.0)
                            .color(ui::pal::GOLD),
                    );
                    ui.label(
                        egui::RichText::new(status)
                            .size(10.0)
                            .color(ui::pal::INK),
                    );
                });
        });
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt).clamp(1.0 / 240.0, 0.1);
        self.anim += dt;
        self.fps += (1.0 / dt - self.fps) * 0.05;

        // A phone-sized viewport gets the compact HUD and the cheap preset.
        let view = ui.max_rect();
        self.ust.compact = ui::compact_for_view(view.width(), view.height());
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
            let _ = root.set_attribute("data-green-td-build", BUILD_ID);
            let _ = root.set_attribute("data-green-td-mode", self.game.mode.label());
            let _ = root.set_attribute(
                "data-green-td-encounter",
                &self
                    .game
                    .campaign_encounter()
                    .map(|encounter| encounter.to_string())
                    .unwrap_or_else(|| "legacy".to_owned()),
            );
            let _ = root.set_attribute("data-green-td-phase", &format!("{:?}", self.game.phase));
            let _ = root.set_attribute(
                "data-green-td-armed",
                if self.game.build_choice.is_some() { "true" } else { "false" },
            );
            let _ = root.set_attribute(
                "data-green-td-paused",
                if self.game.paused { "true" } else { "false" },
            );
            let _ = root.set_attribute("data-green-td-towers", &self.game.towers.len().to_string());
            let _ = root.set_attribute("data-green-td-creeps", &self.game.creeps.len().to_string());
            // Expose the simulation-owned tempo to the isolated browser test;
            // a painted HUD label cannot prove that the selected fast clock is
            // actually advancing
            // the fixed-step game clock.
            let _ = root.set_attribute(
                "data-green-td-speed",
                &format!("{:.0}", self.game.speed),
            );
            if let Some(diagnostic) = self
                .campaign_diagnostic
                .filter(|_| self.game.diagnostic_fixture.is_some())
            {
                let _ = root.set_attribute(
                    "data-green-td-diagnostic",
                    &format!(
                        "E{}->E{}:{}",
                        diagnostic.start,
                        diagnostic.target,
                        if diagnostic.reached { "reached" } else { "running" }
                    ),
                );
            } else {
                let _ = root.remove_attribute("data-green-td-diagnostic");
            }
            // The menu is drawn by egui, whose modal position follows the
            // *actual* CSS viewport. Publish its live Continue hitbox for the
            // isolated browser smoke instead of baking an assumed phone size
            // into an input test. This is diagnostic only; normal interaction
            // still goes through the same pointer events as a player.
            if self.menu.screen == Screen::Title {
                if let Some(rect) = self.menu.resume_rect {
                    let _ = root.set_attribute(
                        "data-green-td-resume-rect",
                        &format!(
                            "{:.2},{:.2},{:.2},{:.2}",
                            rect.left(),
                            rect.top(),
                            rect.width(),
                            rect.height()
                        ),
                    );
                } else {
                    let _ = root.remove_attribute("data-green-td-resume-rect");
                }
                if let Some(rect) = self.menu.campaign_rect {
                    let _ = root.set_attribute(
                        "data-green-td-campaign-rect",
                        &format!(
                            "{:.2},{:.2},{:.2},{:.2}",
                            rect.left(),
                            rect.top(),
                            rect.width(),
                            rect.height()
                        ),
                    );
                } else {
                    let _ = root.remove_attribute("data-green-td-campaign-rect");
                }
            } else {
                let _ = root.remove_attribute("data-green-td-resume-rect");
                let _ = root.remove_attribute("data-green-td-campaign-rect");
            }
        }

        // --- menu: the board keeps rendering behind it as a live backdrop
        if self.menu.screen != Screen::Playing {
            self.menu_frame(ui, dt);
            return;
        }
        if self.ust.want_menu {
            self.ust.want_menu = false;
            if !self.net.is_online() && self.game.diagnostic_fixture.is_none() && !self.visual_showcase {
                save::store(&self.game);
            }
            self.net.leave();
            self.menu.saved = save::load();
            self.menu.screen = Screen::Title;
            self.game.restart();
            return;
        }

        let keys = read_keys(ui);
        if self.ust.show_threat_intel {
            if keys.cancel {
                self.ust.show_threat_intel = false;
                self.ust.threat_intel_rect = None;
            }
        } else {
            self.apply_keys(&keys);
        }
        self.auto_quality();

        // --- simulate
        let t_frame = now_ms();
        if !self.ust.show_threat_intel {
            self.game.update(dt);
        }
        // Query-selected boundary evidence runs only after the ordinary update
        // has spawned and stepped its real bodies. The pilot damages those
        // bodies through the production combat path on the next line; it never
        // changes the campaign cursor or reward ledger itself. At the target,
        // pause on an actually spawned formation so the browser capture shows
        // the achieved transition rather than a transient number in a HUD.
        if let Some(diagnostic) = self.campaign_diagnostic.as_mut()
            && self.game.diagnostic_fixture.is_some()
        {
            let target_is_live = self.game.campaign_encounter() == Some(diagnostic.target)
                && self.game.wave == diagnostic.target as u32
                && self.game.phase == game::Phase::Combat
                && self
                    .game
                    .creeps
                    .iter()
                    .any(|creep| creep.campaign_encounter == diagnostic.target);
            if !diagnostic.reached && target_is_live {
                diagnostic.reached = true;
                self.game.paused = true;
                self.game.notice(format!(
                    "DIAGNOSTIC target E{} reached through live Campaign transition; paused for inspection",
                    diagnostic.target
                ));
            } else if !diagnostic.reached {
                self.game.diagnostic_pilot_tick();
            }
        }
        self.net.push(self.game.snapshot(), dt);
        if std::mem::take(&mut self.game.wants_save) {
            // Solo runs only: a room's run belongs to the room, and resuming
            // into one nobody else is playing any more would be a lie.
            if !self.net.is_online() && self.game.diagnostic_fixture.is_none() && !self.visual_showcase {
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
        // The same RTS hierarchy is used on every viewport: a slim resource
        // strip and a bottom command console.  In particular, the build grid
        // lives at the bottom-right rather than in a tall side catalogue, so
        // its click targets are physically separated from battlefield picking.
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

        if std::mem::take(&mut self.ust.reset_view) {
            self.view.reset_requested = true;
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            // Real browser QA provenance for the RTS build palette. These
            // values are read-only mirrors of the egui rectangles and game
            // state; the smoke test still uses normal pointer events to arm
            // the icon and place the tower.
            let palette = self.ust.palette_rect;
            let _ = root.set_attribute(
                "data-green-td-palette-css",
                &format!(
                    "{:.2},{:.2},{:.2},{:.2}",
                    palette.left(),
                    palette.top(),
                    palette.width(),
                    palette.height()
                ),
            );
            let _ = root.set_attribute(
                "data-green-td-card-css",
                &self
                    .ust
                    .card_rects
                    .iter()
                    .map(|card| format!(
                        "{:.2},{:.2},{:.2},{:.2}",
                        card.left(),
                        card.top(),
                        card.width(),
                        card.height()
                    ))
                    .collect::<Vec<_>>()
                    .join(";"),
            );
            let _ = root.set_attribute(
                "data-green-td-card-defs",
                &self
                    .ust
                    .hotkeys
                    .iter()
                    .map(|def| def.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            let view = self.ust.view_rect;
            let _ = root.set_attribute(
                "data-green-td-view-css",
                &format!(
                    "{:.2},{:.2},{:.2},{:.2}",
                    view.left(),
                    view.top(),
                    view.width(),
                    view.height()
                ),
            );
            let speed = self.ust.speed_rect;
            let _ = root.set_attribute(
                "data-green-td-speed-css",
                &format!(
                    "{:.2},{:.2},{:.2},{:.2}",
                    speed.left(),
                    speed.top(),
                    speed.width(),
                    speed.height()
                ),
            );
            let controls = self
                .ust
                .command_rects
                .iter()
                .map(|(name, rect)| format!(
                    "{}:{:.2},{:.2},{:.2},{:.2}",
                    name, rect.left(), rect.top(), rect.width(), rect.height()
                ))
                .collect::<Vec<_>>()
                .join(";");
            let _ = root.set_attribute("data-green-td-command-css", &controls);
            let armed = self
                .game
                .build_choice
                .and_then(|(def, _)| game::defs::TOWERS.get(def))
                .map_or("", |def| def.name);
            let armed_cost = self
                .game
                .build_choice
                .and_then(|(def, _)| game::defs::TOWERS.get(def))
                .map_or(0, |def| def.gold);
            let last_tower = self.game.towers.last().map_or("", |tower| tower.full_name());
            let _ = root.set_attribute("data-green-td-armed-tower", armed);
            let _ = root.set_attribute("data-green-td-armed-cost", &armed_cost.to_string());
            let _ = root.set_attribute("data-green-td-last-tower", last_tower);
            let _ = root.set_attribute("data-green-td-gold", &self.game.gold.to_string());
        }

        // --- board
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let area = ui.available_rect_before_wrap();
                // The scene owns the entire central viewport. The tactical
                // square is fitted by the oblique camera in world space; it
                // is no longer a square render target with black side gutters.
                ui.painter().rect_filled(area, 0.0, ui::pal::PANEL_DEEP);
                let rect = scene_rect(area);
                #[cfg(target_arch = "wasm32")]
                if let Some(root) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.document_element())
                {
                    // Read-only browser QA provenance: the same rectangle
                    // drives the egui callback, the camera, and picking. This
                    // lets an isolated test prove a resize did not leave input
                    // in an obsolete board coordinate system.
                    let _ = root.set_attribute(
                        "data-green-td-board-css",
                        &format!(
                            "{:.2},{:.2},{:.2},{:.2}",
                            rect.left(),
                            rect.top(),
                            rect.width(),
                            rect.height()
                        ),
                    );
                }
                let resp = ui.interact(
                    rect,
                    ui.id().with("world_scene"),
                    Sense::click_and_drag(),
                );
                // The browser reports devicePixelRatio here, which on a 2x display
                // asks for four times the pixels. The HUD stays crisp at native
                // scale; the 3D scene does not need to be supersampled.
                let ppp = ctx.pixels_per_point().min(MAX_SCENE_DPR);
                let px = [
                    (rect.width() * ppp).round().max(8.0) as u32,
                    (rect.height() * ppp).round().max(8.0) as u32,
                ];
                // The initial/reset pose fits the whole route. Thereafter a
                // wheel/pinch or deliberate pan changes this one camera, and
                // the rebuilt value below drives rendering, ghost picking and
                // labels together in the same frame.
                let initial_camera = self.view.camera(rect);
                let suppress_primary = self.view.navigate(&ctx, &resp, rect, &initial_camera);
                let camera = self.view.camera(rect);

                #[cfg(target_arch = "wasm32")]
                if let Some(root) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.document_element())
                {
                    let _ = root.set_attribute(
                        "data-green-td-camera",
                        &format!(
                            "{:.3},{:.3},{:.3}",
                            self.view.centre[0], self.view.centre[1], self.view.span
                        ),
                    );
                }

                #[cfg(target_arch = "wasm32")]
                if let Some(root) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.document_element())
                {
                    // This is an input probe, not a game command: automation
                    // still sends a normal pointer event through `board_input`.
                    // It makes viewport/picking regression checks use an
                    // actual legal grass point after every resize instead of a
                    // fragile guessed pixel coordinate.
                    if let Some(probe) = self.visible_grass_probe(&camera)
                        && let Some(point) = camera.to_screen(v3(probe[0], probe[1], 0.0))
                    {
                        let _ = root.set_attribute(
                            "data-green-td-probe-world",
                            &format!("{:.3},{:.3}", probe[0], probe[1]),
                        );
                        let _ = root.set_attribute(
                            "data-green-td-input-probe",
                            &format!(
                                "{:.2},{:.2}",
                                rect.left() + point[0] * rect.width(),
                                rect.top() + point[1] * rect.height()
                            ),
                        );
                    }
                    // A route point is deliberately not a build socket. It
                    // gives the browser check a deterministic invalid click
                    // that proves the palette/placement path never spends
                    // gold on illegal terrain.
                    let route = self.game.board.start();
                    if let Some(point) = camera.to_screen(v3(route[0], route[1], 0.0)) {
                        let _ = root.set_attribute(
                            "data-green-td-invalid-probe",
                            &format!(
                                "{:.2},{:.2}",
                                rect.left() + point[0] * rect.width(),
                                rect.top() + point[1] * rect.height()
                            ),
                        );
                    }
                }

                self.board_input(&resp, rect, &camera, suppress_primary);

                #[cfg(target_arch = "wasm32")]
                if let Some(root) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.document_element())
                {
                    // Read-only interaction provenance for the isolated
                    // browser check. It mirrors the exact world point the
                    // normal pointer path just used, so a compact-viewport
                    // regression can distinguish a lost click from a clean
                    // footprint rejection without inventing a test-only build
                    // command.
                    let hover = self.game.hover_pos.map_or_else(
                        || "".to_owned(),
                        |p| format!("{:.3},{:.3}", p[0], p[1]),
                    );
                    let toast = self
                        .game
                        .toast
                        .as_ref()
                        .map_or("", |(message, _, _)| message.as_str());
                    let hover_buildability = self.game.hover_pos.map_or_else(
                        || "".to_owned(),
                        |p| match self.game.buildability_at(p, true) {
                            Ok(pos) => format!("ok:{:.3},{:.3}", pos[0], pos[1]),
                            Err(issue) => format!("blocked:{}", issue.label()),
                        },
                    );
                    let _ = root.set_attribute("data-green-td-hover-world", &hover);
                    let _ = root.set_attribute("data-green-td-placement-feedback", toast);
                    let _ = root.set_attribute("data-green-td-hover-buildability", &hover_buildability);
                    // `board_input` can buy a tower in this very frame. Keep
                    // these read-only QA mirrors coherent with the ghost
                    // result above instead of exposing the pre-click count
                    // until the next repaint (which may be deliberately
                    // paused while a player is choosing a placement).
                    let armed = self
                        .game
                        .build_choice
                        .and_then(|(def, _)| game::defs::TOWERS.get(def))
                        .map_or("", |def| def.name);
                    let armed_cost = self
                        .game
                        .build_choice
                        .and_then(|(def, _)| game::defs::TOWERS.get(def))
                        .map_or(0, |def| def.gold);
                    let last_tower = self.game.towers.last().map_or("", |tower| tower.full_name());
                    let _ = root.set_attribute(
                        "data-green-td-towers",
                        &self.game.towers.len().to_string(),
                    );
                    let _ = root.set_attribute("data-green-td-armed-tower", armed);
                    let _ = root.set_attribute("data-green-td-armed-cost", &armed_cost.to_string());
                    let _ = root.set_attribute("data-green-td-last-tower", last_tower);
                    let _ = root.set_attribute("data-green-td-gold", &self.game.gold.to_string());
                }

                let t_build = now_ms();
                self.draw.clear();
                view::draw_scene(&self.game, &self.decor, &mut self.draw, self.anim);
                self.spawns.clear();
                self.spawns.append(&mut self.game.fx.particles);
                Profile::feed(&mut self.prof.build, now_ms() - t_build);

                self.publish_frame(camera, px, dt);
                ui.painter()
                    .add(egui_wgpu::Callback::new_paint_callback(rect, BoardCallback));

                // The build selector is now below the board at every size, so
                // board-local placement feedback stays available on desktop.
                ui::board_text(&self.game, ui, &camera, rect, true);
                ui::board_hover(&self.game, &resp, &camera, rect);
            });

        menu::room_scoreboard(&ctx, &self.net, self.ust.compact);
        ui::modals(&mut self.game, &ctx, &mut self.ust);

        #[cfg(target_arch = "wasm32")]
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            let controls = self
                .ust
                .command_rects
                .iter()
                .map(|(name, rect)| format!(
                    "{}:{:.2},{:.2},{:.2},{:.2}",
                    name, rect.left(), rect.top(), rect.width(), rect.height()
                ))
                .collect::<Vec<_>>()
                .join(";");
            let _ = root.set_attribute("data-green-td-command-css", &controls);
            let _ = root.set_attribute(
                "data-green-td-intel-open",
                if self.ust.show_threat_intel { "true" } else { "false" },
            );
            let intel_rect_str = self
                .ust
                .threat_intel_rect
                .map(|r| format!("{:.2},{:.2},{:.2},{:.2}", r.left(), r.top(), r.width(), r.height()))
                .unwrap_or_default();
            let _ = root.set_attribute("data-green-td-intel-rect", &intel_rect_str);
            let active_sec = self.game.campaign.as_ref().map_or(0.0, |c| c.active_seconds);
            let _ = root.set_attribute(
                "data-green-td-active-seconds",
                &format!("{:.2}", active_sec),
            );
            let _ = root.set_attribute(
                "data-green-td-paused",
                if self.game.paused { "true" } else { "false" },
            );
        }
        if let Some(diagnostic) = self
            .campaign_diagnostic
            .filter(|_| self.game.diagnostic_fixture.is_some())
        {
            diagnostic_banner(&ctx, diagnostic, &self.game);
        } else if self.visual_showcase {
            egui::Area::new(egui::Id::new("campaign_visual_showcase"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(14.0, ui::top_h(false) + 8.0))
                .show(&ctx, |ui| {
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(42, 31, 20))
                        .stroke(egui::Stroke::new(1.0, ui::pal::GOLD))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("VISUAL SHOWCASE — GRANTED ROSTER / LIVE COMMANDER / NOT SAVED")
                                .strong().size(10.0).color(ui::pal::GOLD));
                        });
                });
        }
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

/// Where the camera opens, in tiles.
///
/// The road now winds through the whole arena, so its geometric centre is the
/// useful tactical overview. It puts multiple lanes and their nearby build
/// tiles on screen immediately; players can still zoom to the spawn or a kill
/// zone with the wheel and drag controls.
pub fn lane_middle() -> [f32; 2] {
    let lap = game::greentd_map::LAP;
    if lap.len() > 8 {
        let v = game::greentd_map::VIEW;
        [(v[0] + v[2]) * 0.5, (v[1] + v[3]) * 0.5]
    } else {
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
