//! A small retained/immediate 2D graphics engine built on `wgpu`.
//!
//! `plutonium_engine` provides a window/app runner, sprite and atlas rendering,
//! SVG and raster texture loading, MSDF/raster text rendering, simple retained UI
//! objects, popups, layout helpers, animation helpers, and deterministic RNG
//! utilities for examples and tests.
//!
//! # Quick start
//!
//! ```no_run
//! use plutonium_engine::{
//!     app::{run_app, WindowConfig},
//!     utils::Position,
//! };
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = WindowConfig {
//!         title: "Plutonium".to_string(),
//!         width: 800,
//!         height: 600,
//!         ..Default::default()
//!     };
//!
//!     let mut sprite = None;
//!     run_app(config, move |engine, _frame, _app| {
//!         if sprite.is_none() {
//!             match engine.create_texture_2d("examples/media/player.svg", Position::default(), 1.0) {
//!                 Ok(texture) => sprite = Some(texture),
//!                 Err(err) => {
//!                     log::warn!("failed to load sprite: {err}");
//!                     return;
//!                 }
//!             }
//!         }
//!
//!         engine.clear_render_queue();
//!         if let Some(texture) = &sprite {
//!             texture.render(engine);
//!         }
//!     })?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Feature flags
//!
//! - `widgets` (default): retained widgets such as buttons and text inputs.
//! - `layout`: anchor/percent layout helpers.
//! - `anim`: tweening and animation helpers.
//! - `raster`: PNG/JPEG/raster font helper APIs.
//! - `wasm`: WebAssembly support helpers, including JavaScript entropy support.
//!
//! # Coordinates and threading
//!
//! Positions and sizes are logical pixels with the origin at the top-left corner;
//! `+x` points right and `+y` points down. The engine converts logical pixels to
//! device pixels internally using the current DPI scale factor.
//!
//! Engine objects are single-thread affine. Most retained objects wrap
//! `Rc<RefCell<_>>` internals and are intentionally `!Send`/`!Sync`; create and
//! use them on the same UI/render thread that owns the `PlutoniumEngine`.
//!
//! # Backend type stability
//!
//! Low-level/manual integration APIs intentionally expose `wgpu` and `winit`
//! types such as `wgpu::Surface`, `wgpu::SurfaceError`, `winit::dpi::PhysicalSize`,
//! and `winit::keyboard::Key`. These signatures are the backend interop layer:
//! callers that create their own surfaces, drive their own event loops, or handle
//! raw keyboard state use the exact backend types pinned by this crate. Upgrading
//! `wgpu` or `winit` is therefore a public API change and is handled as part of
//! this crate's semver surface rather than hidden behind lossy wrapper types.
//!
#![warn(missing_docs)]

/// Documentation and public API for camera.
pub mod camera;
mod draw;
/// Documentation and public API for error.
pub mod error;
mod font_msdf;
mod font_raster;
mod objects;
mod render;
/// Documentation and public API for pluto objects.
pub mod pluto_objects {
    #[cfg(feature = "widgets")]
    /// Documentation and public API for button.
    pub mod button;
    /// Documentation and public API for shapes.
    pub mod shapes;
    /// Documentation and public API for text2d.
    pub mod text2d;
    #[cfg(feature = "widgets")]
    /// Documentation and public API for text input.
    pub mod text_input;
    /// Documentation and public API for texture 2d.
    pub mod texture_2d;
    /// Documentation and public API for texture atlas 2d.
    pub mod texture_atlas_2d;
}
/// Documentation and public API for app.
pub mod app;
pub use app::{FrameContext, PlutoniumApp, WindowConfig};
pub use error::EngineError;
#[cfg(feature = "anim")]
/// Documentation and public API for anim.
pub mod anim;
mod glow;
mod gpu_timer;
/// Documentation and public API for input.
pub mod input;
#[cfg(feature = "layout")]
/// Documentation and public API for layout.
pub mod layout;
/// Documentation and public API for popup.
pub mod popup;
mod popup_render;
/// Documentation and public API for renderer.
pub mod renderer;
pub mod rng;
/// Documentation and public API for text.
pub mod text;
/// Documentation and public API for texture atlas.
pub mod texture_atlas;
/// Documentation and public API for texture svg.
pub mod texture_svg;
/// Documentation and public API for traits.
pub mod traits;
/// Documentation and public API for ui.
pub mod ui;
/// Documentation and public API for utils.
pub mod utils;
pub use popup::{
    PopupAction, PopupActionStyle, PopupConfig, PopupDismissReason, PopupEvent, PopupSize,
};

#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use crate::font_raster::PendingRasterTextureUrlLoad;
use crate::font_raster::{
    PendingRasterWarmRequest, RasterFontFamily, DEFAULT_RUNTIME_GLYPH_BUDGET_PER_FRAME,
};
use crate::popup::PopupRuntimeState;
use crate::traits::UpdateContext;
use camera::Camera;
#[cfg(not(target_arch = "wasm32"))]
use pollster::block_on;
use render::{
    create_identity_bind_group, evict_instance_pool, reset_instance_pool_usage,
    InstanceBufferPoolEntry, RectStyleKey, TransformPool,
};
use renderer::{GlowCommand, RectCommand};
use std::cell::RefCell;
use std::rc::Rc;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque},
};
use text::*;
use texture_atlas::TextureAtlas;
use texture_svg::*;
use traits::PlutoObject;
use utils::*;
use uuid::Uuid;
use winit::dpi::PhysicalSize;
use winit::keyboard::Key;

// renderer seam reserved for future use

#[derive(Debug, Clone, Copy, Default)]
/// DrawParams data.
pub struct DrawParams {
    /// Draw-order layer; larger values render above smaller values.
    pub z: i32,
    /// Uniform scale multiplier.
    pub scale: f32,
    /// Clockwise rotation in radians.
    pub rotation: f32,
    /// RGBA tint multiplier.
    pub tint: [f32; 4],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Options for texture fit.
pub enum TextureFit {
    #[default]
    /// Contain option.
    Contain,
    /// Stretch fill option.
    StretchFill,
    /// Cover option.
    Cover,
}

#[derive(Debug, Clone)]
/// Options for glyph set.
pub enum GlyphSet {
    /// Ascii core option.
    AsciiCore,
    /// Custom option.
    Custom(Vec<char>),
}

impl Default for GlyphSet {
    fn default() -> Self {
        Self::AsciiCore
    }
}

#[derive(Debug, Clone, Copy, Default)]
/// Options for raster hinting mode.
pub enum RasterHintingMode {
    #[default]
    /// Auto option.
    Auto,
    /// None option.
    None,
}

#[derive(Debug, Clone)]
/// PrewarmConfig data.
pub struct PrewarmConfig {
    /// Font sizes to prewarm.
    pub sizes: Vec<f32>,
    /// Glyphs to prepare or render.
    pub glyph_set: GlyphSet,
}

#[derive(Debug, Clone, Default)]
/// Options for prewarm policy.
pub enum PrewarmPolicy {
    /// None option.
    None,
    #[default]
    /// Light preset option.
    LightPreset,
    /// Custom option.
    Custom(PrewarmConfig),
}

#[derive(Debug, Clone)]
/// FontLoadOptions data.
pub struct FontLoadOptions {
    /// Font prewarm policy.
    pub prewarm_policy: PrewarmPolicy,
    /// Maximum runtime glyphs to rasterize per frame.
    pub runtime_budget_glyphs_per_frame: usize,
    /// Raster-font hinting mode.
    pub hinting: RasterHintingMode,
}

impl Default for FontLoadOptions {
    fn default() -> Self {
        Self {
            prewarm_policy: PrewarmPolicy::LightPreset,
            runtime_budget_glyphs_per_frame: DEFAULT_RUNTIME_GLYPH_BUDGET_PER_FRAME,
            hinting: RasterHintingMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
/// WarmStats data.
pub struct WarmStats {
    /// Requested sizes value.
    pub requested_sizes: usize,
    /// Warmed sizes value.
    pub warmed_sizes: usize,
    /// Already loaded sizes value.
    pub already_loaded_sizes: usize,
    /// Glyphs rasterized value.
    pub glyphs_rasterized: usize,
}

#[cfg(feature = "raster")]
#[derive(Debug, Clone)]
/// Options for raster texture load error.
pub enum RasterTextureLoadError {
    /// Fetch failed option.
    FetchFailed(String),
    /// Invalid response option.
    InvalidResponse(String),
    /// Http status option.
    HttpStatus {
        /// Item value.
        status: u16,
        /// Item value.
        status_text: String,
        /// Item value.
        url: String,
    },
    /// Body read failed option.
    BodyReadFailed(String),
    /// Image decode failed option.
    ImageDecodeFailed(String),
    /// Texture create failed option.
    TextureCreateFailed(String),
}

#[cfg(feature = "raster")]
impl std::fmt::Display for RasterTextureLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RasterTextureLoadError::FetchFailed(msg) => write!(f, "fetch failed: {}", msg),
            RasterTextureLoadError::InvalidResponse(url) => {
                write!(f, "invalid fetch response type for '{}'", url)
            }
            RasterTextureLoadError::HttpStatus {
                status,
                status_text,
                url,
            } => write!(
                f,
                "http error while fetching '{}': {} {}",
                url, status, status_text
            ),
            RasterTextureLoadError::BodyReadFailed(msg) => {
                write!(f, "failed reading response body: {}", msg)
            }
            RasterTextureLoadError::ImageDecodeFailed(msg) => {
                write!(f, "image decode failed: {}", msg)
            }
            RasterTextureLoadError::TextureCreateFailed(msg) => {
                write!(f, "texture creation failed: {}", msg)
            }
        }
    }
}

#[cfg(feature = "raster")]
impl std::error::Error for RasterTextureLoadError {}

#[cfg(all(feature = "raster", target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterTextureUrlLoadHandle(Uuid);

/// Curve used to fade halo intensity as it radiates outward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HaloFalloff {
    /// Uniform linear fade from center to edge.
    Linear,
    /// Strong center emphasis that fades quickly near the edge.
    EaseOut,
    /// Smooth S-curve fade.
    Smoothstep,
    /// Power curve; larger values keep the center brighter for longer.
    Exponential(f32),
    /// Physically-inspired inverse-square-ish fade.
    InverseSquare,
}

impl HaloFalloff {
    /// Sample the falloff curve at normalized distance (0 = near target, 1 = outer edge).
    pub fn sample(self, distance_01: f32) -> f32 {
        let d = distance_01.clamp(0.0, 1.0);
        let fade = match self {
            HaloFalloff::Linear => 1.0 - d,
            HaloFalloff::EaseOut => {
                let inv = 1.0 - d;
                inv * inv
            }
            HaloFalloff::Smoothstep => 1.0 - (d * d * (3.0 - 2.0 * d)),
            HaloFalloff::Exponential(power) => (1.0 - d).powf(power.max(0.001)),
            HaloFalloff::InverseSquare => 1.0 / (1.0 + 8.0 * d * d),
        };
        fade.clamp(0.0, 1.0)
    }
}

/// Halo rendering mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HaloMode {
    /// Soft outside glow (default).
    Glow,
    /// Border-only highlight.
    Border,
}

impl Default for HaloMode {
    fn default() -> Self {
        HaloMode::Glow
    }
}

/// Predefined halo styles for common tutorial/highlight scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaloPreset {
    /// Default tutorial highlight: clear, balanced attention cue.
    TutorialPrimary,
    /// Softer highlight for low-priority guidance.
    TutorialSubtle,
    /// Strong highlight for urgent/critical interactions.
    TutorialUrgent,
}

/// Configurable halo/highlight style for tutorials and guided interactions.
#[derive(Debug, Clone, Copy)]
pub struct HaloStyle {
    /// Base halo color in linear RGBA (alpha is source opacity, clamped to [0, 1]).
    pub color: [f32; 4],
    /// Overall alpha multiplier applied after falloff (negative values clamp to 0).
    pub intensity: f32,
    /// Hard upper bound for per-ring alpha (clamped to [0, 1]).
    pub max_alpha: f32,
    /// Outward spread in logical pixels (negative values clamp to 0).
    pub radius: f32,
    /// Gap between target bounds and first ring in logical pixels (negative values clamp to 0).
    pub inner_padding: f32,
    /// Number of concentric rings (minimum effective value is 1).
    pub ring_count: u16,
    /// Base Z layer for the halo.
    pub z: i32,
    /// Corner radius used for ring rects.
    pub corner_radius: f32,
    /// Optional pulse amount (0..1+).
    pub pulse_amplitude: f32,
    /// Pulse speed in Hz.
    pub pulse_speed_hz: f32,
    /// Time input used by pulse (typically elapsed tutorial time in seconds).
    pub time_seconds: f32,
    /// Falloff profile across radius.
    pub falloff: HaloFalloff,
    /// Rendering mode: soft glow or border-only highlight.
    pub mode: HaloMode,
    /// Border width in pixels (used only in `HaloMode::Border`).
    pub border_width: f32,
}

impl Default for HaloStyle {
    fn default() -> Self {
        Self {
            color: [0.38, 0.72, 1.0, 0.95],
            intensity: 1.0,
            max_alpha: 0.65,
            radius: 52.0,
            inner_padding: 4.0,
            ring_count: 10,
            z: 50,
            corner_radius: 8.0,
            pulse_amplitude: 0.18,
            pulse_speed_hz: 1.8,
            time_seconds: 0.0,
            falloff: HaloFalloff::EaseOut,
            mode: HaloMode::Glow,
            border_width: 3.0,
        }
    }
}

impl HaloStyle {
    /// Build a style from a predefined preset.
    pub fn from_preset(preset: HaloPreset) -> Self {
        match preset {
            HaloPreset::TutorialPrimary => HaloStyle::default(),
            HaloPreset::TutorialSubtle => HaloStyle {
                color: [0.45, 0.78, 1.0, 0.75],
                intensity: 0.8,
                max_alpha: 0.4,
                radius: 36.0,
                inner_padding: 2.0,
                ring_count: 8,
                pulse_amplitude: 0.1,
                pulse_speed_hz: 1.2,
                ..HaloStyle::default()
            },
            HaloPreset::TutorialUrgent => HaloStyle {
                color: [1.0, 0.66, 0.18, 1.0],
                intensity: 1.35,
                max_alpha: 0.82,
                radius: 62.0,
                inner_padding: 5.0,
                ring_count: 12,
                pulse_amplitude: 0.32,
                pulse_speed_hz: 2.4,
                falloff: HaloFalloff::Smoothstep,
                ..HaloStyle::default()
            },
        }
    }

    /// Effective alpha at a normalized ring distance.
    ///
    /// Safety rules:
    /// - `color[3]` and `max_alpha` are clamped to `[0, 1]`
    /// - `intensity < 0` is clamped to `0`
    pub fn alpha_at(self, distance_01: f32) -> f32 {
        let base_alpha = self.color[3].clamp(0.0, 1.0);
        let intensity = self.intensity.max(0.0);
        let pulse = if self.pulse_amplitude.abs() <= f32::EPSILON || self.pulse_speed_hz <= 0.0 {
            1.0
        } else {
            let phase = self.time_seconds * self.pulse_speed_hz * std::f32::consts::TAU;
            let wave = 0.5 + 0.5 * phase.sin();
            1.0 + self.pulse_amplitude.max(0.0) * wave
        };
        let alpha = base_alpha * intensity * self.falloff.sample(distance_01) * pulse;
        alpha.clamp(0.0, self.max_alpha.clamp(0.0, 1.0))
    }
}

pub(crate) enum RenderItem {
    Texture {
        texture_key: Uuid,
        transform_index: usize,
    },
    AtlasTile {
        texture_key: Uuid,
        transform_index: usize,
        tile_index: usize,
        tint: [f32; 4],
    },
    AtlasGlyph {
        texture_key: Uuid,
        transform_index: usize,
        uv_offset: [f32; 2],
        uv_scale: [f32; 2],
        tint: [f32; 4],
        is_msdf: bool,
        msdf_px_range: f32,
    },
    Rect(RectCommand),
    Glow(GlowCommand),
}

/// QueuedItem data.
pub struct QueuedItem {
    z: i32,
    clip_rect: Option<Rectangle>,
    item: RenderItem,
}

#[derive(Debug, Clone, Copy)]
struct SlotState {
    rect: Rectangle,
    z_base: i32,
    is_open: bool,
    // Placeholder for future rounded-corner clipping; rectangular scissor is v1.
    clip_radius: Option<f32>,
}

/// Core renderer and retained-object registry.
///
/// Manual integration methods on this type deliberately use `wgpu` and `winit`
/// types so callers can pass surfaces, physical window sizes, and keyboard events
/// directly from the backend versions selected by this crate.
pub struct PlutoniumEngine<'a> {
    /// Current physical surface size from `winit`.
    ///
    /// This is intentionally a `winit::dpi::PhysicalSize<u32>` because manual
    /// event-loop integrations pass the value directly from window resize events.
    pub size: PhysicalSize<u32>,
    dpi_scale_factor: f32,
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    #[allow(dead_code)]
    render_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    msdf_render_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    rect_pipeline: wgpu::RenderPipeline,
    glow_pipeline: wgpu::RenderPipeline,
    glow_instance_bgl: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    rect_dummy_bgl: wgpu::BindGroupLayout,
    rect_dummy_bg: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    transform_bind_group_layout: wgpu::BindGroupLayout,
    instance_bind_group_layout: wgpu::BindGroupLayout,
    texture_map: HashMap<Uuid, TextureSVG>,
    atlas_map: HashMap<Uuid, TextureAtlas>,
    pluto_objects: HashMap<Uuid, Rc<RefCell<dyn PlutoObject>>>,
    update_queue: Vec<Uuid>,
    render_queue: Vec<QueuedItem>,
    viewport_size: Size,
    camera: Camera,
    text_renderer: TextRenderer,
    loaded_fonts: HashMap<String, bool>,
    raster_font_families: HashMap<String, RasterFontFamily>,
    pending_raster_warm: VecDeque<PendingRasterWarmRequest>,
    pending_raster_warm_dedupe: HashSet<(String, u32, u32)>,
    runtime_raster_warm_budget: HashMap<String, usize>,
    pending_raster_warm_deferred: VecDeque<PendingRasterWarmRequest>,
    #[cfg(all(feature = "raster", target_arch = "wasm32"))]
    pending_raster_url_loads: HashMap<Uuid, PendingRasterTextureUrlLoad>,
    transform_pool: TransformPool,
    // Static geometry for rects
    rect_vertex_buffer: wgpu::Buffer,
    rect_index_buffer: wgpu::Buffer,
    // Cached identity UBO bind group used by batched render paths.
    identity_transform_bg: wgpu::BindGroup,
    // Per-flush storage-buffer pools. Entries are reused across frames and never
    // overwritten twice in the same frame while queued GPU work may still read them.
    sprite_instance_pool: Vec<InstanceBufferPoolEntry>,
    atlas_instance_pool: Vec<InstanceBufferPoolEntry>,
    rect_instance_pool: Vec<InstanceBufferPoolEntry>,
    glow_instance_pool: Vec<InstanceBufferPoolEntry>,
    frame_counter: u64,
    // GPU timing instrumentation (optional, owned by GpuTimer)
    gpu_timer: gpu_timer::GpuTimer,
    // Global UI clip rectangle (logical coords)
    current_scissor: Option<Rectangle>,
    // Nested clip stack (logical coords); top-most is applied. Each push intersects with previous.
    clip_stack: Vec<Rectangle>,
    // Rect batching metrics (style diversity and counts) — preserved-order strategy
    rect_style_keys: HashSet<RectStyleKey>,
    rect_instances_count: usize,
    rect_draw_calls_count: usize,
    // Frame-local slot definitions for deterministic layered UI composition.
    slot_states: HashMap<Uuid, SlotState>,
    popup_state: PopupRuntimeState,
    // Incremented whenever the font cache is invalidated (e.g. DPI change)
    font_cache_version: u32,
}

impl<'a> PlutoniumEngine<'a> {
    /* CAMERA STUFF */
    /// Sets the boundary.
    pub fn set_boundary(&mut self, boundary: Rectangle) {
        self.camera.set_boundary(boundary);
    }
    /// Clear boundary.
    pub fn clear_boundary(&mut self) {
        self.camera.clear_boundary();
    }

    /// Activate camera.
    pub fn activate_camera(&mut self) {
        self.camera.activate();
    }

    /// Deactivate camera.
    pub fn deactivate_camera(&mut self) {
        self.camera.deactivate();
    }

    /// Sets the camera smoothing.
    pub fn set_camera_smoothing(&mut self, smoothing_strength: f32) {
        self.camera.set_smoothing_strength(smoothing_strength);
    }

    /// Measure text width (logical px) and line count using the same layout math as rendering.
    pub fn measure_text(
        &self,
        text: &str,
        font_key: &str,
        letter_spacing: f32,
        word_spacing: f32,
        font_size_override: Option<f32>,
    ) -> (f32, usize) {
        let (resolved_font_key, resolved_size_override) =
            self.resolve_font_key_for_measure(font_key, font_size_override);
        self.text_renderer.measure_text(
            text,
            &resolved_font_key,
            letter_spacing,
            word_spacing,
            resolved_size_override,
        )
    }

    /// Print per-glyph horizontal layout diagnostics for a single line.
    ///
    /// Useful for debugging pair overlap/spacing issues (`j/u`, etc.) by showing
    /// pen position, kerning, glyph bounds, and advance for each glyph.
    pub fn debug_print_text_line_layout(
        &self,
        line: &str,
        font_key: &str,
        font_size: f32,
        letter_spacing: f32,
        word_spacing: f32,
    ) -> Result<(), String> {
        let records = self.text_renderer.debug_line_layout_records(
            line,
            font_key,
            font_size,
            letter_spacing,
            word_spacing,
        )?;

        log::info!(
            "[TEXT LAYOUT DEBUG] font='{}' size={:.2}px text={:?}",
            font_key,
            font_size,
            line
        );
        let mut prev_right: Option<f32> = None;
        for rec in records {
            let gap_from_prev = prev_right.map(|right| rec.glyph_left_px - right);
            log::info!("  #{:02} {} '{}'->'{}' pen={:+7.2} kern={:+6.2} left={:+7.2} right={:+7.2} adv={:+6.2} ls={:+5.2} next_pen={:+7.2} gap_prev={}",
            rec.index,
            rec.mode,
            rec.input_char,
            rec.resolved_char,
            rec.pen_x_before,
            rec.kerning_px,
            rec.glyph_left_px,
            rec.glyph_right_px,
            rec.advance_px,
            rec.letter_spacing_px,
            rec.pen_x_after,
            match gap_from_prev {
                Some(v) => format!("{:+.2}", v),
                None => "-".to_string(),
            });
            prev_right = Some(rec.glyph_right_px);
        }

        Ok(())
    }

    /// Returns the window bounds as a Rectangle for use with layout.
    /// This is a convenience method that returns a rectangle covering the entire window.
    /// Returns logical pixel coordinates (not physical pixels).
    #[cfg(feature = "layout")]
    pub fn window_bounds(&self) -> Rectangle {
        // Return logical pixels: physical size divided by DPI scale factor
        Rectangle::new(
            0.0,
            0.0,
            (self.size.width as f32) / self.dpi_scale_factor,
            (self.size.height as f32) / self.dpi_scale_factor,
        )
    }

    /// Returns the current logical window size (logical px, DPI-aware).
    pub fn logical_window_size(&self) -> Size {
        Size {
            width: self.viewport_size.width / self.dpi_scale_factor,
            height: self.viewport_size.height / self.dpi_scale_factor,
        }
    }

    /// Returns the current DPI scale factor used by input/layout/render transforms.
    pub fn dpi_scale_factor(&self) -> f32 {
        self.dpi_scale_factor
    }

    /// Save a loaded font atlas to disk for visual debugging.
    ///
    /// `font_key` must match a font loaded through `load_font`, `load_msdf_font`,
    /// or `load_msdf_font_from_ttf`.
    pub fn debug_dump_font_atlas_png(
        &self,
        font_key: &str,
        output_path: &str,
    ) -> Result<(), String> {
        let Some(font_atlas) = self.text_renderer.font_atlases.get(font_key) else {
            return Err(format!("font atlas not found for key '{}'", font_key));
        };
        let atlas_id = font_atlas.atlas_id();
        let Some(atlas) = self.atlas_map.get(&atlas_id) else {
            return Err(format!(
                "atlas texture not found for font key '{}' (atlas id: {})",
                font_key, atlas_id
            ));
        };
        atlas
            .save_debug_png(&self.device, &self.queue, output_path)
            .map_err(|e| {
                format!(
                    "failed to save atlas png for font key '{}' to '{}': {}",
                    font_key, output_path, e
                )
            })
    }

    /// Sets the texture position.
    pub fn set_texture_position(&mut self, key: &Uuid, position: Position) {
        if let Some(texture) = self.texture_map.get_mut(key) {
            texture.set_position(
                &self.device,
                &self.queue,
                position,
                self.viewport_size,
                self.camera.get_pos(self.dpi_scale_factor),
            );
        }
    }

    /// Updates the DPI scale factor used by input, camera, and text/layout transforms.
    /// Accepts f64 to match windowing APIs and stores a sanitized internal f32.
    pub fn set_dpi_scale_factor(&mut self, scale_factor: f64) {
        let old_dpi = self.dpi_scale_factor;
        self.dpi_scale_factor = Self::sanitize_dpi_scale_factor(scale_factor);

        if (self.dpi_scale_factor - old_dpi).abs() > f32::EPSILON {
            self.font_cache_version = self.font_cache_version.wrapping_add(1);
            // Trigger re-warming of all currently loaded variants at the new DPI
            let mut to_warm = Vec::new();
            for (font_key, family) in &self.raster_font_families {
                for &(size_q, _) in family.loaded_sizes.keys() {
                    to_warm.push((font_key.clone(), Self::dequantize_font_size(size_q)));
                }
            }
            for (key, size) in to_warm {
                self.queue_raster_warm_request(&key, size, self.dpi_scale_factor);
            }
        }
    }

    /// Resize the GPU surface and viewport for manual event-loop integrations.
    ///
    /// The argument intentionally uses `winit::dpi::PhysicalSize` to match the
    /// resize events emitted by the crate-pinned `winit` version.
    pub fn resize(&mut self, new_size: &PhysicalSize<u32>) {
        // MAYBE NEEDS TO TAKE INTO ACCOUNT NEW SCALE FACTOR IF RESIZE CHANGES DEVICE
        self.size = *new_size;
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.viewport_size = Size {
            width: self.size.width as f32,
            height: self.size.height as f32,
        };
    }

    /// Update retained Pluto objects for manual event-loop integrations.
    ///
    /// The `key` argument intentionally uses `winit::keyboard::Key`; applications
    /// that need a stable abstraction can translate their input before calling
    /// this low-level method.
    pub fn update(&mut self, mouse_info: Option<MouseInfo>, key: &Option<Key>, delta_time: f32) {
        // text doesn't seem to be getting updated
        let scaled_mouse_info = mouse_info.map(|info| MouseInfo {
            is_rmb_clicked: info.is_rmb_clicked,
            is_lmb_clicked: info.is_lmb_clicked,
            is_mmb_clicked: info.is_mmb_clicked,
            mouse_pos: info.mouse_pos / self.dpi_scale_factor,
            scroll_dx: info.scroll_dx / self.dpi_scale_factor,
            scroll_dy: info.scroll_dy / self.dpi_scale_factor,
        });
        let popup_blocked_before_update = self.popup_state.blocks_input_behind_popup();
        self.popup_state
            .update(mouse_info, key, delta_time, self.logical_window_size());
        for object_id in self.popup_state.drain_popup_object_cleanup() {
            self.remove_object(object_id);
        }
        let input_consumed =
            popup_blocked_before_update || self.popup_state.blocks_input_behind_popup();
        let object_mouse_info = if input_consumed {
            None
        } else {
            scaled_mouse_info
        };
        let empty_key = None;
        let object_key = if input_consumed { &empty_key } else { key };

        for id in &self.update_queue {
            if let Some(obj) = self.pluto_objects.get(id) {
                obj.borrow_mut().update(
                    object_mouse_info,
                    object_key,
                    &mut self.texture_map,
                    Some(UpdateContext {
                        device: &self.device,
                        queue: &self.queue,
                        viewport_size: &self.viewport_size,
                        camera_position: &self.camera.get_pos(self.dpi_scale_factor),
                        font_cache_version: self.font_cache_version,
                    }),
                    self.dpi_scale_factor,
                    &self.text_renderer,
                );
            }
        }

        // Handle camera tethering with DPI scaling
        let (camera_position, tether_size) = if let Some(tether_target) = &self.camera.tether_target
        {
            if let Some(tether) = self.pluto_objects.get(tether_target) {
                let tether_ref = tether.borrow();
                let tether_dimensions = tether_ref.dimensions();
                (tether_dimensions.pos(), Some(tether_dimensions.size()))
            } else {
                (self.camera.logical_pos(), None)
            }
        } else {
            (self.camera.logical_pos(), None)
        };

        self.camera.set_pos_with_dt(camera_position, delta_time);
        self.camera.set_tether_size(tether_size);

        // update actual location of where object buffers are
        for texture in self.texture_map.values_mut() {
            texture.update_transform_uniform(
                &self.device,
                &self.queue,
                self.viewport_size,
                self.camera.get_pos(self.dpi_scale_factor),
            );
        }
        for atlas in self.atlas_map.values_mut() {
            atlas.update_transform_uniform(
                &self.device,
                &self.queue,
                self.viewport_size,
                self.camera.get_pos(self.dpi_scale_factor),
            );
        }
    }

    /// Show popup.
    pub fn show_popup(&mut self, config: PopupConfig) {
        self.popup_state.show_popup(config);
    }

    /// Show popup with objects.
    pub fn show_popup_with_objects(
        &mut self,
        config: PopupConfig,
        panel_rect: Rectangle,
        object_ids: Vec<Uuid>,
    ) {
        self.popup_state
            .show_popup_with_objects(config, panel_rect, object_ids);
    }

    /// Close popup.
    pub fn close_popup(&mut self, popup_id: &str) -> bool {
        self.popup_state.close_popup(popup_id)
    }

    /// Popup is open.
    pub fn popup_is_open(&self) -> bool {
        self.popup_state.is_open()
    }

    /// Drain popup events.
    pub fn drain_popup_events(&mut self) -> Vec<PopupEvent> {
        self.popup_state.drain_events()
    }

    /// Sets the camera target.
    pub fn set_camera_target(&mut self, texture_key: Uuid) {
        self.camera.tether_target = Some(texture_key);
    }

    // Frame helpers for an immediate-mode style
    /// Begin frame.
    pub fn begin_frame(&mut self) {
        self.process_runtime_raster_warm_queue();
        self.clear_render_queue();
        self.transform_pool.reset();
        self.frame_counter = self.frame_counter.wrapping_add(1);
        reset_instance_pool_usage(&mut self.sprite_instance_pool);
        reset_instance_pool_usage(&mut self.atlas_instance_pool);
        reset_instance_pool_usage(&mut self.rect_instance_pool);
        reset_instance_pool_usage(&mut self.glow_instance_pool);
        self.current_scissor = None;
        self.clip_stack.clear();
        self.rect_style_keys.clear();
        self.rect_instances_count = 0;
        self.rect_draw_calls_count = 0;
        self.slot_states.clear();
    }

    /// Finish the frame and present queued GPU work.
    ///
    /// Returns the raw `wgpu::SurfaceError` so manual render loops can make the
    /// same lost/outdated/out-of-memory decisions they would make when using
    /// `wgpu` directly.
    pub fn end_frame(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.render_popup_overlay();
        // Periodically evict least recently used instance buffers to cap memory.
        const MAX_POOL: usize = 32;
        const EVICT_AGE: u64 = 600; // frames
        evict_instance_pool(
            &mut self.sprite_instance_pool,
            self.frame_counter,
            MAX_POOL,
            EVICT_AGE,
        );
        evict_instance_pool(
            &mut self.atlas_instance_pool,
            self.frame_counter,
            MAX_POOL,
            EVICT_AGE,
        );
        evict_instance_pool(
            &mut self.rect_instance_pool,
            self.frame_counter,
            MAX_POOL,
            EVICT_AGE,
        );
        evict_instance_pool(
            &mut self.glow_instance_pool,
            self.frame_counter,
            MAX_POOL,
            EVICT_AGE,
        );
        self.render()
    }

    /// Remove object.
    pub fn remove_object(&mut self, id: Uuid) {
        self.pluto_objects.remove(&id);
    }

    /// Logical bounds for a retained Pluto object.
    pub fn object_bounds(&self, object_id: &Uuid) -> Option<Rectangle> {
        self.pluto_objects
            .get(object_id)
            .map(|obj| obj.borrow().dimensions())
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Creates an engine from caller-owned `wgpu` surface and instance objects.
    ///
    /// The `wgpu` and `winit` argument types are intentional backend interop
    /// points and track the versions pinned by `plutonium_engine`.
    pub fn new(
        surface: wgpu::Surface<'a>,
        instance: wgpu::Instance,
        size: PhysicalSize<u32>,
        dpi_scale_factor: f32,
    ) -> Result<Self, EngineError> {
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        }))
        .ok_or(EngineError::AdapterUnavailable)?;

        let required_features = wgpu::Features::empty();
        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                memory_hints: Default::default(),
            },
            None,
        ))
        .map_err(|err| EngineError::DeviceRequestError(err.to_string()))?;

        Ok(Self::new_with_device_and_adapter(
            surface,
            size,
            dpi_scale_factor,
            adapter,
            device,
            queue,
        ))
    }

    #[cfg(target_arch = "wasm32")]
    /// Returns an error on wasm; use [`Self::new_async`] for wasm initialization.
    pub fn new(
        _surface: wgpu::Surface<'a>,
        _instance: wgpu::Instance,
        _size: PhysicalSize<u32>,
        _dpi_scale_factor: f32,
    ) -> Result<Self, EngineError> {
        Err(EngineError::SurfaceError(
            "PlutoniumEngine::new is not available on wasm32; use PlutoniumEngine::new_async"
                .to_string(),
        ))
    }

    #[cfg(target_arch = "wasm32")]
    /// Creates an engine asynchronously for wasm targets.
    pub async fn new_async(
        surface: wgpu::Surface<'a>,
        instance: wgpu::Instance,
        size: PhysicalSize<u32>,
        dpi_scale_factor: f32,
    ) -> Result<Self, String> {
        let wasm_log = |msg: &str| {
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(msg));
        };

        wasm_log("pluto new_async: requesting adapter (WebGPU/WebGL2)...");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await;

        let adapter = match adapter {
            Some(a) => {
                wasm_log(&format!(
                    "pluto new_async: adapter found (backend={:?})",
                    a.get_info().backend
                ));
                a
            }
            None => {
                wasm_log("pluto new_async: primary adapter failed, retrying with fallback...");
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::default(),
                        force_fallback_adapter: true,
                        compatible_surface: Some(&surface),
                    })
                    .await
                    .ok_or_else(|| {
                        "no WebGPU/WebGL2 adapter found (tried primary and fallback)".to_string()
                    })?
            }
        };

        wasm_log("pluto new_async: requesting device...");
        let required_features = wgpu::Features::empty();
        // Use WebGL2-safe limits so we don't request compute/storage tiers the
        // adapter may not support.  .using_resolution() caps texture dimensions
        // to what the adapter actually allows.
        let required_limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features,
                    required_limits,
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("failed to create device: {e}"))?;

        wasm_log("pluto new_async: device ready; configuring surface and building engine...");
        wasm_log("pluto new_async: entering new_with_device_and_adapter");
        let engine = Self::new_with_device_and_adapter(
            surface,
            size,
            dpi_scale_factor,
            adapter,
            device,
            queue,
        );
        wasm_log("pluto new_async: new_with_device_and_adapter returned");
        Ok(engine)
    }

    fn new_with_device_and_adapter(
        surface: wgpu::Surface<'a>,
        size: PhysicalSize<u32>,
        dpi_scale_factor: f32,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        let dpi_scale_factor = Self::sanitize_dpi_scale_factor(dpi_scale_factor as f64);

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str("pluto new_with: start"));
        let surface_caps = surface.get_capabilities(&adapter);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
            "pluto new_with: caps formats={} present_modes={} alpha_modes={}",
            surface_caps.formats.len(),
            surface_caps.present_modes.len(),
            surface_caps.alpha_modes.len()
        )));
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8UnormSrgb
                        | wgpu::TextureFormat::Rgba8UnormSrgb
                        | wgpu::TextureFormat::Bgra8Unorm
                        | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .or_else(|| surface_caps.formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);
        let present_mode = if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Fifo)
        {
            wgpu::PresentMode::Fifo
        } else {
            surface_caps
                .present_modes
                .first()
                .copied()
                .unwrap_or(wgpu::PresentMode::Fifo)
        };
        let alpha_mode = surface_caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);

        let config = wgpu::SurfaceConfiguration {
            // NOTE: Using 1 to force immediate vsync blocking (reduces frame queuing jitter)
            desired_maximum_frame_latency: 1,
            alpha_mode,
            view_formats: vec![],
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
        };
        surface.configure(&device, &config);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(
            "pluto new_with: surface configured",
        ));

        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("transform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX, // Transformation matrix is used in the vertex shader
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<TransformUniform>() as _,
                        ),
                    },
                    count: None,
                }],
            });

        let uv_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uv_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT, // UV offsets and scales are used in the fragment shader
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        // The size must match the UVUniform structure defined in the shader
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<UVTransform>() as _
                        ),
                    },
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT, // Texture is used in the fragment shader
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT, // Sampler is used in the fragment shader
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // shader and related devices
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/shader.wgsl"))),
        });
        let msdf_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("msdf-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../shaders/text_msdf.wgsl"
            ))),
        });

        // Now update the pipeline layout to include all four bind group layouts

        // Persistent instance bind group layout (group 3)
        let instance_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("instance_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Texture Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &transform_bind_group_layout,
                &uv_bind_group_layout,
                &instance_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // set up render pipeline (textured quads)

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(
            "pluto new_with: creating texture pipeline",
        ));
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    // Use standard non-premultiplied alpha blending so glyph quads don't appear as solid white
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(
            "pluto new_with: creating msdf pipeline",
        ));
        let msdf_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("msdf-render-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &msdf_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &msdf_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Rect pipeline (SDF rects with optional border)
        // Create an empty bind group layout for slots we don't use (group 0 and 2)
        let rect_dummy_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rect-dummy-bgl"),
            entries: &[],
        });
        let rect_dummy_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect-dummy-bg"),
            layout: &rect_dummy_bgl,
            entries: &[],
        });
        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/rect.wgsl"))),
        });
        let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Rect Pipeline Layout"),
            bind_group_layouts: &[
                // group(0) unused (no texture) — use empty layout
                &rect_dummy_bgl,
                &transform_bind_group_layout,
                // group(2) unused — use empty layout
                &rect_dummy_bgl,
                &instance_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(
            "pluto new_with: creating rect pipeline",
        ));
        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect-pipeline"),
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &rect_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Glow pipeline (SDF analytical glow shader)
        let glow_instance_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glow-instance-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let glow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glow-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/glow.wgsl"))),
        });
        let glow_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Glow Pipeline Layout"),
            bind_group_layouts: &[
                &rect_dummy_bgl,
                &transform_bind_group_layout,
                &rect_dummy_bgl,
                &glow_instance_bgl,
            ],
            push_constant_ranges: &[],
        });
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(
            "pluto new_with: creating glow pipeline",
        ));
        let glow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glow-pipeline"),
            layout: Some(&glow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &glow_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &glow_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    // Premultiplied alpha blending for clean glow compositing
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Static centered quad for rects
        let (rect_vertex_buffer, rect_index_buffer) = create_centered_quad_buffers(&device);

        let texture_map: HashMap<Uuid, TextureSVG> = HashMap::new();
        let atlas_map: HashMap<Uuid, TextureAtlas> = HashMap::new();
        let pluto_objects = HashMap::new();
        let viewport_size = Size {
            width: config.width as f32,
            height: config.height as f32,
        };
        let render_queue = Vec::new();
        let update_queue = Vec::new();
        let camera = Camera::new(Position { x: 0.0, y: 0.0 });

        let text_renderer = TextRenderer::new();
        let loaded_fonts = HashMap::new();
        let raster_font_families = HashMap::new();
        #[cfg(all(feature = "raster", target_arch = "wasm32"))]
        let pending_raster_url_loads = HashMap::new();
        let transform_pool = TransformPool::new();
        let identity_transform_bg = create_identity_bind_group(
            &device,
            &transform_bind_group_layout,
            "identity-transform-bg",
        );

        // Optional GPU timestamp query setup (construction delegated to GpuTimer)
        let gpu_timer = gpu_timer::GpuTimer::new(&device, &queue);

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(
            "pluto new_with: finalizing engine struct",
        ));
        Self {
            size,
            surface,
            device,
            dpi_scale_factor,
            queue,
            config,
            render_pipeline,
            msdf_render_pipeline,
            texture_bind_group_layout,
            transform_bind_group_layout,
            texture_map,
            atlas_map,
            pluto_objects,
            render_queue,
            update_queue,
            viewport_size,
            camera,
            text_renderer,
            loaded_fonts,
            raster_font_families,
            pending_raster_warm: VecDeque::new(),
            pending_raster_warm_dedupe: HashSet::new(),
            runtime_raster_warm_budget: HashMap::new(),
            pending_raster_warm_deferred: VecDeque::new(),
            #[cfg(all(feature = "raster", target_arch = "wasm32"))]
            pending_raster_url_loads,
            transform_pool,
            rect_pipeline,
            glow_pipeline,
            glow_instance_bgl,
            rect_dummy_bgl,
            rect_dummy_bg,
            rect_vertex_buffer,
            rect_index_buffer,
            identity_transform_bg,
            sprite_instance_pool: Vec::new(),
            atlas_instance_pool: Vec::new(),
            rect_instance_pool: Vec::new(),
            glow_instance_pool: Vec::new(),
            frame_counter: 0,
            instance_bind_group_layout,
            gpu_timer,
            current_scissor: None,
            clip_stack: Vec::new(),
            rect_style_keys: HashSet::new(),
            rect_instances_count: 0,
            rect_draw_calls_count: 0,
            slot_states: HashMap::new(),
            popup_state: PopupRuntimeState::new(),
            font_cache_version: 0,
        }
    }

    // UI clipping (logical coordinates); applies a scissor rect for the render pass of this frame
    /// Sets the clip.
    pub fn set_clip(&mut self, rect: Rectangle) {
        self.current_scissor = Some(rect);
    }
    /// Clear clip.
    pub fn clear_clip(&mut self) {
        self.current_scissor = None;
    }

    // Push a clip rectangle (intersect with prior top if present)
    /// Push clip.
    pub fn push_clip(&mut self, rect: Rectangle) {
        if let Some(&prev) = self.clip_stack.last() {
            let x1 = prev.x.max(rect.x);
            let y1 = prev.y.max(rect.y);
            let x2 = (prev.x + prev.width).min(rect.x + rect.width);
            let y2 = (prev.y + prev.height).min(rect.y + rect.height);
            let w = (x2 - x1).max(0.0);
            let h = (y2 - y1).max(0.0);
            self.clip_stack.push(Rectangle::new(x1, y1, w, h));
        } else {
            self.clip_stack.push(rect);
        }
    }
    /// Pop clip.
    pub fn pop_clip(&mut self) {
        let _ = self.clip_stack.pop();
    }
}
