pub mod camera;
mod draw;
mod font_msdf;
mod font_raster;
mod render;
pub mod pluto_objects {
    #[cfg(feature = "widgets")]
    pub mod button;
    pub mod shapes;
    pub mod text2d;
    #[cfg(feature = "widgets")]
    pub mod text_input;
    pub mod texture_2d;
    pub mod texture_atlas_2d;
}
pub mod app;
pub use app::{FrameContext, PlutoniumApp, WindowConfig};
#[cfg(feature = "anim")]
pub mod anim;
pub mod input;
#[cfg(feature = "layout")]
pub mod layout;
pub mod popup;
mod glow;
mod gpu_timer;
mod popup_render;
pub mod renderer;
pub mod rng;
pub mod text;
pub mod texture_atlas;
pub mod texture_svg;
pub mod traits;
pub mod ui;
pub mod utils;
pub use popup::{
    PopupAction, PopupActionStyle, PopupConfig, PopupDismissReason, PopupEvent, PopupSize,
};

use crate::font_raster::{
    PendingRasterWarmRequest, RasterFontFamily, DEFAULT_RUNTIME_GLYPH_BUDGET_PER_FRAME,
};
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use crate::font_raster::PendingRasterTextureUrlLoad;
use crate::popup::PopupRuntimeState;
use crate::traits::UpdateContext;
use camera::Camera;
#[cfg(feature = "widgets")]
use pluto_objects::button::{Button, ButtonInternal};
#[cfg(feature = "widgets")]
use pluto_objects::text_input::{TextInput, TextInputInternal};
use pluto_objects::{
    shapes::{Shape, ShapeInternal, ShapeType},
    text2d::{Text2D, Text2DInternal},
    texture_2d::{Texture2D, Texture2DInternal},
    texture_atlas_2d::{TextureAtlas2D, TextureAtlas2DInternal},
};
#[cfg(not(target_arch = "wasm32"))]
use pollster::block_on;
use render::{RectInstanceBuffer, RectStyleKey, TransformPool};
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
pub struct DrawParams {
    pub z: i32,
    pub scale: f32,
    pub rotation: f32,
    pub tint: [f32; 4],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextureFit {
    #[default]
    Contain,
    StretchFill,
    Cover,
}


#[derive(Debug, Clone)]
pub enum GlyphSet {
    AsciiCore,
    Custom(Vec<char>),
}

impl Default for GlyphSet {
    fn default() -> Self {
        Self::AsciiCore
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum RasterHintingMode {
    #[default]
    Auto,
    None,
}

#[derive(Debug, Clone)]
pub struct PrewarmConfig {
    pub sizes: Vec<f32>,
    pub glyph_set: GlyphSet,
}

#[derive(Debug, Clone, Default)]
pub enum PrewarmPolicy {
    None,
    #[default]
    LightPreset,
    Custom(PrewarmConfig),
}

#[derive(Debug, Clone)]
pub struct FontLoadOptions {
    pub prewarm_policy: PrewarmPolicy,
    pub runtime_budget_glyphs_per_frame: usize,
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
pub struct WarmStats {
    pub requested_sizes: usize,
    pub warmed_sizes: usize,
    pub already_loaded_sizes: usize,
    pub glyphs_rasterized: usize,
}

#[cfg(feature = "raster")]
#[derive(Debug, Clone)]
pub enum RasterTextureLoadError {
    FetchFailed(String),
    InvalidResponse(String),
    HttpStatus {
        status: u16,
        status_text: String,
        url: String,
    },
    BodyReadFailed(String),
    ImageDecodeFailed(String),
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


pub struct PlutoniumEngine<'a> {
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
    #[cfg(all(feature = "raster", target_arch = "wasm32"))]
    pending_raster_url_loads: HashMap<Uuid, PendingRasterTextureUrlLoad>,
    transform_pool: TransformPool,
    // Static geometry for rects
    rect_vertex_buffer: wgpu::Buffer,
    rect_index_buffer: wgpu::Buffer,
    // Per-frame cached identity UBO bind group
    rect_identity_bg: Option<wgpu::BindGroup>,
    // Rect instance buffer pool
    rect_instance_pool: Vec<RectInstanceBuffer>,
    rect_pool_cursor: usize,
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
    pub fn set_boundary(&mut self, boundary: Rectangle) {
        self.camera.set_boundary(boundary);
    }
    pub fn clear_boundary(&mut self) {
        self.camera.clear_boundary();
    }

    pub fn activate_camera(&mut self) {
        self.camera.activate();
    }

    pub fn deactivate_camera(&mut self) {
        self.camera.deactivate();
    }

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

        println!(
            "[TEXT LAYOUT DEBUG] font='{}' size={:.2}px text={:?}",
            font_key, font_size, line
        );
        let mut prev_right: Option<f32> = None;
        for rec in records {
            let gap_from_prev = prev_right.map(|right| rec.glyph_left_px - right);
            println!(
                "  #{:02} {} '{}'->'{}' pen={:+7.2} kern={:+6.2} left={:+7.2} right={:+7.2} adv={:+6.2} ls={:+5.2} next_pen={:+7.2} gap_prev={}",
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
                }
            );
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

    pub fn show_popup(&mut self, config: PopupConfig) {
        self.popup_state.show_popup(config);
    }

    pub fn show_popup_with_objects(
        &mut self,
        config: PopupConfig,
        panel_rect: Rectangle,
        object_ids: Vec<Uuid>,
    ) {
        self.popup_state
            .show_popup_with_objects(config, panel_rect, object_ids);
    }

    pub fn close_popup(&mut self, popup_id: &str) -> bool {
        self.popup_state.close_popup(popup_id)
    }

    pub fn popup_is_open(&self) -> bool {
        self.popup_state.is_open()
    }

    pub fn drain_popup_events(&mut self) -> Vec<PopupEvent> {
        self.popup_state.drain_events()
    }

    pub fn set_camera_target(&mut self, texture_key: Uuid) {
        self.camera.tether_target = Some(texture_key);
    }

    // Frame helpers for an immediate-mode style
    pub fn begin_frame(&mut self) {
        self.process_runtime_raster_warm_queue();
        self.clear_render_queue();
        self.transform_pool.reset();
        self.rect_identity_bg = None;
        self.rect_pool_cursor = 0;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        for entry in &mut self.rect_instance_pool {
            entry.used_this_frame = false;
        }
        self.current_scissor = None;
        self.clip_stack.clear();
        self.rect_style_keys.clear();
        self.rect_instances_count = 0;
        self.rect_draw_calls_count = 0;
        self.slot_states.clear();
    }

    pub fn end_frame(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.render_popup_overlay();
        // Periodically evict least recently used rect instance buffers to cap memory
        const MAX_POOL: usize = 32;
        const EVICT_AGE: u64 = 600; // frames
        if self.rect_instance_pool.len() > MAX_POOL {
            // Retain entries that are either recently used or needed
            self.rect_instance_pool
                .retain(|e| self.frame_counter.saturating_sub(e.last_used_frame) < EVICT_AGE);
            if self.rect_instance_pool.len() > MAX_POOL {
                // Sort by last_used_frame ascending and truncate
                self.rect_instance_pool.sort_by_key(|e| e.last_used_frame);
                self.rect_instance_pool.truncate(MAX_POOL);
            }
        }
        self.render()
    }

    pub fn create_texture_svg(
        &mut self,
        file_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> (Uuid, Rectangle) {
        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new(
            texture_key,
            &self.device,
            &self.queue,
            file_path,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            scale_factor * self.dpi_scale_factor,
        );

        let texture = svg_texture.expect("texture should always be created properly");
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        (texture_key, dimensions)
    }

    pub fn create_texture_svg_from_data(
        &mut self,
        svg_data: &str,
        position: Position,
        scale_factor: f32,
    ) -> (Uuid, Rectangle) {
        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new_from_data(
            texture_key,
            &self.device,
            &self.queue,
            svg_data,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            scale_factor * self.dpi_scale_factor,
        );

        let texture = svg_texture.expect("texture should always be created properly");
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        (texture_key, dimensions)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn create_texture_svg_from_str(
        &mut self,
        svg_source: &str,
        position: Position,
        scale_factor: f32,
    ) -> (Uuid, Rectangle) {
        self.create_texture_svg_from_data(svg_source, position, scale_factor)
    }

    #[cfg(feature = "raster")]
    pub fn create_texture_raster_from_path(
        &mut self,
        path: &str,
        position: Position,
    ) -> (Uuid, Rectangle) {
        let img = image::open(path).expect("failed to open image").to_rgba8();
        let (width, height) = img.dimensions();
        let rgba = img.as_raw();

        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new_from_rgba(
            texture_key,
            &self.device,
            &self.queue,
            width,
            height,
            rgba,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
        );

        let texture = svg_texture.expect("texture should always be created properly");
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        (texture_key, dimensions)
    }

    pub fn create_texture_atlas(
        &mut self,
        svg_path: &str,
        position: Position,
        tile_size: Size,
    ) -> (Uuid, Rectangle) {
        let texture_key = Uuid::new_v4();

        // Update to match new TextureAtlas interface
        if let Some(atlas) = TextureAtlas::new(
            texture_key,
            &self.device,
            &self.queue,
            svg_path,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            tile_size,
        ) {
            let dimensions = atlas.dimensions();

            let positioned_dimensions =
                Rectangle::new(position.x, position.y, dimensions.width, dimensions.height);

            self.atlas_map.insert(texture_key, atlas);
            (texture_key, positioned_dimensions)
        } else {
            panic!("Failed to create texture atlas")
        }
    }

    pub fn create_font_texture_atlas(
        &mut self,
        atlas_id: Uuid,
        texture_data: &[u8],
        width: u32,
        height: u32,
        tile_size: Size,
        char_positions: &HashMap<char, CharacterInfo>,
    ) -> TextureAtlas2D {
        self.create_font_texture_atlas_with_options(
            atlas_id,
            texture_data,
            width,
            height,
            tile_size,
            char_positions,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::FilterMode::Nearest,
            wgpu::FilterMode::Nearest,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create_font_texture_atlas_with_options(
        &mut self,
        atlas_id: Uuid,
        texture_data: &[u8],
        width: u32,
        height: u32,
        tile_size: Size,
        char_positions: &HashMap<char, CharacterInfo>,
        texture_format: wgpu::TextureFormat,
        mag_filter: wgpu::FilterMode,
        min_filter: wgpu::FilterMode,
        force_base_mip_level: bool,
    ) -> TextureAtlas2D {
        if force_base_mip_level {
            debug_assert!(
                !matches!(
                    texture_format,
                    wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb
                ),
                "MSDF atlases must use linear (non-sRGB) texture formats"
            );
        }

        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Font Atlas Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[texture_format],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            texture_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        // Create texture view and sampler
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter,
            min_filter,
            // MSDF textures are encoded distance data, not colors. Keep sampling at mip 0.
            mipmap_filter: if force_base_mip_level {
                wgpu::FilterMode::Nearest
            } else {
                min_filter
            },
            lod_min_clamp: 0.0,
            lod_max_clamp: if force_base_mip_level { 0.0 } else { 32.0 },
            ..Default::default()
        });

        // Create the texture bind group
        let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("Font Atlas Bind Group"),
        });

        // Create TextureAtlas and add it to the atlas_map
        if let Some(atlas) = TextureAtlas::new_from_texture(
            atlas_id,
            texture,
            texture_bind_group,
            Position { x: 0.0, y: 0.0 },
            Size::new(width as f32, height as f32),
            tile_size,
            &self.device,
            &self.queue,
            &self.transform_bind_group_layout,
            char_positions,
        ) {
            #[cfg(not(target_arch = "wasm32"))]
            if texture_format == wgpu::TextureFormat::Rgba8UnormSrgb {
                let _ = atlas.save_debug_png(&self.device, &self.queue, "debug_atlas.png");
            }
            // Add to atlas_map
            self.atlas_map.insert(atlas_id, atlas);

            // Create the internal representation
            let internal = TextureAtlas2DInternal::new(
                atlas_id,
                atlas_id,
                1.0,
                Rectangle::new(0.0, 0.0, width as f32, height as f32),
                tile_size,
            );
            let rc_internal = Rc::new(RefCell::new(internal));

            self.pluto_objects.insert(atlas_id, rc_internal.clone());
            self.update_queue.push(atlas_id);

            TextureAtlas2D::new(rc_internal)
        } else {
            panic!("Failed to create font texture atlas");
        }
    }
    pub fn remove_object(&mut self, id: Uuid) {
        self.pluto_objects.remove(&id);
    }

    /// Logical bounds for a retained Pluto object.
    pub fn object_bounds(&self, object_id: &Uuid) -> Option<Rectangle> {
        self.pluto_objects
            .get(object_id)
            .map(|obj| obj.borrow().dimensions())
    }

    /* OBJECT CREATION FUNCTIONS */
    pub fn create_texture_2d(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> Texture2D {
        let id = Uuid::new_v4();

        // Create the underlying texture
        let (texture_key, dimensions) = self.create_texture_svg(svg_path, position, scale_factor);

        // Create the internal representation
        let internal = Texture2DInternal::new(id, texture_key, dimensions);
        let rc_internal = Rc::new(RefCell::new(internal));

        // Add to pluto objects and update queue
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        // Return the wrapper
        Texture2D::new(rc_internal)
    }
    pub fn create_text2d(
        &mut self,
        text: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
    ) -> Text2D {
        self.create_text2d_with_z(text, font_key, font_size, position, 0)
    }

    pub fn create_text2d_with_z(
        &mut self,
        text: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
        z: i32,
    ) -> Text2D {
        let id = Uuid::new_v4();
        // Ensure font is loaded, now with proper error handling
        if !self.loaded_fonts.contains_key(font_key) {
            panic!("Failed to load font");
        }

        // Create text dimensions based on measurement - now needs font_key
        let text_size = self.measure_text(text, font_key, 0.0, 0.0, Some(font_size));
        let dimensions = Rectangle::new(
            position.x,
            position.y,
            text_size.0,
            text_size.1 as f32 * font_size,
        );

        let internal = Text2DInternal::new(
            id,
            font_key.to_string(), // Changed from font_path to font_key
            dimensions,
            font_size,
            text,
            None,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        // Set z after creation
        rc_internal.borrow_mut().set_z(z);
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Text2D::new(rc_internal)
    }

    pub fn create_texture_atlas_2d(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
        tile_size: Size,
    ) -> TextureAtlas2D {
        let id = Uuid::new_v4();

        // Create texture atlas instead of regular texture
        let (texture_key, dimensions) = self.create_texture_atlas(svg_path, position, tile_size);

        // Create the internal representation
        let internal =
            TextureAtlas2DInternal::new(id, texture_key, scale_factor, dimensions, tile_size);
        let rc_internal = Rc::new(RefCell::new(internal));

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        TextureAtlas2D::new(rc_internal)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_button(
        &mut self,
        svg_path: &str,
        text: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> Button {
        let id = Uuid::new_v4();

        // Create button texture
        let (button_texture_key, button_dimensions) =
            self.create_texture_svg(svg_path, position, scale_factor);

        // Create text object
        let text_position = Position {
            x: button_dimensions.x + (button_dimensions.width * 0.1),
            y: button_dimensions.y + (button_dimensions.height / 2.0),
        };
        let text_object = self.create_text2d(text, font_key, font_size, text_position);
        text_object.set_z(10000);

        // Create internal representation
        let internal = ButtonInternal::new(id, button_texture_key, button_dimensions, text_object);

        // Wrap in Rc<RefCell> and store
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        // Return the wrapper
        Button::new(rc_internal)
    }

    pub fn create_text_input(
        &mut self,
        svg_path: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> TextInput {
        let input_id = Uuid::new_v4();

        // Create button
        let button = self.create_button(svg_path, "", font_key, font_size, position, scale_factor);

        // Create text object
        let text_position = Position {
            x: button.get_dimensions().x + (button.get_dimensions().width * 0.01),
            y: button.get_dimensions().y + (button.get_dimensions().height * 0.05),
        };
        let text = self.create_text2d("", font_key, font_size, text_position);

        // Create cursor using Texture2D with embedded SVG data
        let cursor_height = font_size * 1.05 / self.dpi_scale_factor;
        let cursor_width = scale_factor / self.dpi_scale_factor;
        let cursor_svg_data = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">
        <rect width=\"{}\" height=\"{}\" fill=\"#000\">
            <animate 
                attributeName=\"opacity\"
                values=\"1;0;1\" 
                dur=\"1s\"
                repeatCount=\"indefinite\"/>
        </rect>
    </svg>",
            cursor_width, cursor_height, cursor_width, cursor_height
        );

        let cursor_position = Position {
            x: text_position.x,
            y: button.get_dimensions().y + (button.get_dimensions().height * 0.1),
        };

        let cursor_id = Uuid::new_v4();
        let (texture_key, dimensions) =
            self.create_texture_svg_from_data(&cursor_svg_data, cursor_position, scale_factor);

        // Create the internal representation for cursor
        let cursor_internal = Texture2DInternal::new(cursor_id, texture_key, dimensions);
        let rc_cursor_internal = Rc::new(RefCell::new(cursor_internal));

        // Add cursor to pluto objects and update queue
        self.pluto_objects
            .insert(cursor_id, rc_cursor_internal.clone());
        self.update_queue.push(cursor_id);

        let cursor = Texture2D::new(rc_cursor_internal);

        // Create internal representation for text input
        let dimensions = button.get_dimensions();
        let internal = TextInputInternal::new(input_id, button, text, cursor, dimensions);

        // Wrap in Rc<RefCell> and store
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(input_id, rc_internal.clone());
        self.update_queue.push(input_id);

        // Return the wrapper
        TextInput::new(rc_internal)
    }

    pub fn create_rect(
        &mut self,
        bounds: Rectangle,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Shape {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Rectangle,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        // Create the texture using svg data directly
        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0);

        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Shape::new(rc_internal)
    }

    pub fn create_circle(
        &mut self,
        radius: f32,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Shape {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();
        let bounds = Rectangle::new(0.0, 0.0, radius * 2.0, radius * 2.0);

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Circle,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0);
        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Shape::new(rc_internal)
    }

    pub fn create_polygon(
        &mut self,
        radius: f32,
        points: u32,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Shape {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();
        let bounds = Rectangle::new(0.0, 0.0, radius * 2.0, radius * 2.0);

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Polygon(points),
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0);
        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Shape::new(rc_internal)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(
        surface: wgpu::Surface<'a>,
        instance: wgpu::Instance,
        size: PhysicalSize<u32>,
        dpi_scale_factor: f32,
    ) -> Self {
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        }))
        .expect("Failed to find an appropriate adapter");

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
        .expect("Failed to create device");

        Self::new_with_device_and_adapter(surface, size, dpi_scale_factor, adapter, device, queue)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(
        _surface: wgpu::Surface<'a>,
        _instance: wgpu::Instance,
        _size: PhysicalSize<u32>,
        _dpi_scale_factor: f32,
    ) -> Self {
        panic!("PlutoniumEngine::new is not available on wasm32. Use PlutoniumEngine::new_async.");
    }

    #[cfg(target_arch = "wasm32")]
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
            rect_identity_bg: None,
            rect_instance_pool: Vec::new(),
            rect_pool_cursor: 0,
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
    pub fn set_clip(&mut self, rect: Rectangle) {
        self.current_scissor = Some(rect);
    }
    pub fn clear_clip(&mut self) {
        self.current_scissor = None;
    }

    // Push a clip rectangle (intersect with prior top if present)
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
    pub fn pop_clip(&mut self) {
        let _ = self.clip_stack.pop();
    }
}
