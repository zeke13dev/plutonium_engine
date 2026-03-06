pub mod camera;
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

use crate::popup::{popup_layout_for_active, PopupRuntimeState};
use crate::traits::UpdateContext;
use camera::Camera;
#[cfg(feature = "widgets")]
use pluto_objects::button::{Button, ButtonInternal};
#[cfg(feature = "widgets")]
use pluto_objects::text_input::{TextInput, TextInputInternal};
use pluto_objects::{
    shapes::{Shape, ShapeInternal, ShapeType},
    text2d::{HorizontalAlignment, Text2D, Text2DInternal, TextContainer, VerticalAlignment},
    texture_2d::{Texture2D, Texture2DInternal},
    texture_atlas_2d::{TextureAtlas2D, TextureAtlas2DInternal},
};
use rusttype::{Font, Scale};

#[cfg(not(target_arch = "wasm32"))]
use pollster::block_on;
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
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use wasm_bindgen::JsCast;
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use wasm_bindgen_futures::JsFuture;
use wgpu::util::DeviceExt;
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

const LIGHT_PREWARM_SIZES: [f32; 6] = [12.0, 14.0, 16.0, 18.0, 24.0, 32.0];
const FONT_SIZE_QUANTIZATION: f32 = 100.0;
const DEFAULT_RUNTIME_GLYPH_BUDGET_PER_FRAME: usize = 128;
#[cfg(not(target_arch = "wasm32"))]
const AUTO_HINTED_RASTER_MAX_PX: f32 = 48.0;

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

#[cfg(all(feature = "raster", target_arch = "wasm32"))]
#[derive(Clone)]
struct PendingRasterTextureUrlLoad {
    position: Position,
    state: Rc<RefCell<Option<Result<Vec<u8>, RasterTextureLoadError>>>>,
}

#[derive(Debug, Clone)]
struct RasterSizeEntry {
    atlas_key: String,
}

#[derive(Debug)]
struct RasterFontFamily {
    font_data: &'static [u8],
    default_size: f32,
    hinting: RasterHintingMode,
    runtime_budget_glyphs_per_frame: usize,
    runtime_glyphs: Vec<char>,
    loaded_sizes: HashMap<(u32, u32), RasterSizeEntry>,
}

#[derive(Debug, Clone)]
struct PendingRasterWarmRequest {
    family_key: String,
    size_q: u32,
    dpi_q: u32,
}

struct RasterAtlasBuild {
    texture_data: Vec<u8>,
    char_map: HashMap<char, CharacterInfo>,
    atlas_width: u32,
    atlas_height: u32,
    max_tile_width: u32,
    max_tile_height: u32,
    ascent: f32,
    descent: f32,
    padding_pixels: u32,
}

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

struct TransformPool {
    buffers: Vec<wgpu::Buffer>,
    bind_groups: Vec<wgpu::BindGroup>,
    cursor: usize,
    cpu_mats: Vec<[[f32; 4]; 4]>,
}

struct RectInstanceBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    bind_group: wgpu::BindGroup,
    used_this_frame: bool,
    last_used_frame: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RectStyleKey {
    fill_rgba_u8: [u8; 4],
    border_rgba_u8: [u8; 4],
    corner_radius_10x: u16,    // quantized 0.1 px
    border_thickness_10x: u16, // quantized 0.1 px
}

fn to_rgba_u8(c: [f32; 4]) -> [u8; 4] {
    [
        (c[0].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[1].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[2].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[3].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
    ]
}

fn quant_10x(v: f32) -> u16 {
    ((v.max(0.0) * 10.0) + 0.5).floor() as u16
}

impl TransformPool {
    fn new() -> Self {
        Self {
            buffers: Vec::new(),
            bind_groups: Vec::new(),
            cursor: 0,
            cpu_mats: Vec::new(),
        }
    }
    fn reset(&mut self) {
        self.cursor = 0;
        self.cpu_mats.clear();
    }
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
    // GPU timing instrumentation (optional)
    #[allow(dead_code)]
    timestamp_query: Option<wgpu::QuerySet>,
    #[allow(dead_code)]
    timestamp_buf: Option<wgpu::Buffer>,
    #[allow(dead_code)]
    timestamp_staging: Option<wgpu::Buffer>,
    #[allow(dead_code)]
    timestamp_period_ns: f32,
    #[allow(dead_code)]
    timestamp_count: u32,
    #[allow(dead_code)]
    timestamp_frame_index: u32,
    #[allow(dead_code)]
    gpu_metrics: FrameTimeMetrics,
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
    #[cfg(all(feature = "raster", target_arch = "wasm32"))]
    fn create_texture_raster_from_bytes_internal(
        &mut self,
        bytes: &[u8],
        position: Position,
    ) -> Result<(Uuid, Size), RasterTextureLoadError> {
        let decoded = image::load_from_memory(bytes)
            .map_err(|e| RasterTextureLoadError::ImageDecodeFailed(e.to_string()))?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();

        let texture_key = Uuid::new_v4();
        let texture = TextureSVG::new_from_rgba(
            texture_key,
            &self.device,
            &self.queue,
            width,
            height,
            rgba.as_raw(),
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
        )
        .ok_or_else(|| {
            RasterTextureLoadError::TextureCreateFailed(
                "failed to create texture from decoded image".to_string(),
            )
        })?;

        let dimensions = texture.dimensions() / self.dpi_scale_factor;
        self.texture_map.insert(texture_key, texture);
        Ok((
            texture_key,
            Size {
                width: dimensions.width,
                height: dimensions.height,
            },
        ))
    }

    #[cfg(all(feature = "raster", target_arch = "wasm32"))]
    async fn fetch_raster_texture_bytes(url: &str) -> Result<Vec<u8>, RasterTextureLoadError> {
        let window = web_sys::window().ok_or_else(|| {
            RasterTextureLoadError::FetchFailed("window is unavailable".to_string())
        })?;

        let response_value = JsFuture::from(window.fetch_with_str(url))
            .await
            .map_err(|e| RasterTextureLoadError::FetchFailed(format!("{:?}", e)))?;
        let response: web_sys::Response = response_value
            .dyn_into()
            .map_err(|_| RasterTextureLoadError::InvalidResponse(url.to_string()))?;

        if !response.ok() {
            return Err(RasterTextureLoadError::HttpStatus {
                status: response.status(),
                status_text: response.status_text(),
                url: url.to_string(),
            });
        }

        let array_buffer = JsFuture::from(
            response
                .array_buffer()
                .map_err(|e| RasterTextureLoadError::BodyReadFailed(format!("{:?}", e)))?,
        )
        .await
        .map_err(|e| RasterTextureLoadError::BodyReadFailed(format!("{:?}", e)))?;

        Ok(js_sys::Uint8Array::new(&array_buffer).to_vec())
    }

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

    /// Configure hybrid text thresholds in logical pixels.
    ///
    /// - `tiny_raster_max_px`: use hinted tiny-raster path at or below this size.
    /// - `msdf_min_px`: prefer MSDF at or above this size.
    pub fn set_msdf_switch_thresholds(&mut self, tiny_raster_max_px: f32, msdf_min_px: f32) {
        self.text_renderer
            .set_quality_thresholds(tiny_raster_max_px, msdf_min_px);
    }

    fn sanitize_font_size(size: f32) -> f32 {
        size.max(1.0)
    }

    fn sanitize_dpi_scale_factor(scale_factor: f64) -> f32 {
        if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor as f32
        } else {
            1.0
        }
    }

    fn quantize_font_size(size: f32) -> u32 {
        (Self::sanitize_font_size(size) * FONT_SIZE_QUANTIZATION).round() as u32
    }

    fn dequantize_font_size(size_q: u32) -> f32 {
        (size_q as f32) / FONT_SIZE_QUANTIZATION
    }

    fn raster_variant_key(font_key: &str, size_q: u32, dpi_q: u32) -> String {
        format!("{font_key}::raster@{size_q}@{dpi_q}")
    }

    fn glyphs_from_set(glyph_set: &GlyphSet) -> Vec<char> {
        let mut out = match glyph_set {
            GlyphSet::AsciiCore => (32u8..=126u8).map(|c| c as char).collect(),
            GlyphSet::Custom(chars) => chars
                .iter()
                .copied()
                .filter(|c| !c.is_control())
                .collect::<Vec<char>>(),
        };
        if !out.contains(&' ') {
            out.push(' ');
        }
        if !out.contains(&'?') {
            out.push('?');
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn choose_loaded_raster_entry(
        &self,
        font_key: &str,
        target_size: f32,
        target_dpi: f32,
    ) -> Option<(&RasterSizeEntry, bool)> {
        let family = self.raster_font_families.get(font_key)?;
        let requested_size_q = Self::quantize_font_size(target_size);
        let requested_dpi_q = Self::quantize_font_size(target_dpi);
        let target_key = (requested_size_q, requested_dpi_q);

        if let Some(entry) = family.loaded_sizes.get(&target_key) {
            return Some((entry, true));
        }

        // nearest size, prioritize same DPI if possible
        let mut nearest: Option<(&RasterSizeEntry, u32, u32)> = None;
        for (&(size_q, dpi_q), entry) in &family.loaded_sizes {
            let size_delta = requested_size_q.abs_diff(size_q);
            let dpi_delta = requested_dpi_q.abs_diff(dpi_q);

            match nearest {
                Some((_, best_size_delta, best_dpi_delta)) => {
                    if size_delta < best_size_delta
                        || (size_delta == best_size_delta && dpi_delta < best_dpi_delta)
                    {
                        nearest = Some((entry, size_delta, dpi_delta));
                    }
                }
                _ => nearest = Some((entry, size_delta, dpi_delta)),
            }
        }
        nearest.map(|(entry, _, _)| (entry, false))
    }

    fn resolve_font_key_for_measure(
        &self,
        font_key: &str,
        font_size_override: Option<f32>,
    ) -> (String, Option<f32>) {
        let Some(family) = self.raster_font_families.get(font_key) else {
            return (font_key.to_string(), font_size_override);
        };
        let target = Self::sanitize_font_size(font_size_override.unwrap_or(family.default_size));
        match self.choose_loaded_raster_entry(font_key, target, self.dpi_scale_factor) {
            Some((entry, _)) => {
                let override_size = if font_size_override.is_some() {
                    Some(target)
                } else {
                    None
                };
                (entry.atlas_key.clone(), override_size)
            }
            None => (font_key.to_string(), font_size_override),
        }
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

    pub fn load_font(
        &mut self,
        font_path: &str,
        logical_font_size: f32,
        font_key: &str,
    ) -> Result<(), FontError> {
        self.load_font_with_options(
            font_path,
            logical_font_size,
            font_key,
            FontLoadOptions::default(),
        )
    }

    pub fn load_font_with_options(
        &mut self,
        font_path: &str,
        logical_font_size: f32,
        font_key: &str,
        options: FontLoadOptions,
    ) -> Result<(), FontError> {
        let font_data = std::fs::read(font_path).map_err(FontError::IoError)?;
        self.load_font_with_options_from_bytes_internal(
            font_data,
            logical_font_size,
            font_key,
            options,
        )
    }

    fn load_font_with_options_from_bytes_internal(
        &mut self,
        font_data: Vec<u8>,
        logical_font_size: f32,
        font_key: &str,
        options: FontLoadOptions,
    ) -> Result<(), FontError> {
        if self.loaded_fonts.contains_key(font_key) {
            return Ok(());
        }

        let base_size = Self::sanitize_font_size(logical_font_size);
        // SAFETY: cached for the engine lifetime to keep rusttype font slices valid.
        let leaked: &'static [u8] = Box::leak(font_data.into_boxed_slice());
        let mut prewarm_sizes = vec![base_size];
        let mut runtime_glyphs = Self::glyphs_from_set(&GlyphSet::AsciiCore);
        match &options.prewarm_policy {
            PrewarmPolicy::None => {}
            PrewarmPolicy::LightPreset => prewarm_sizes.extend(LIGHT_PREWARM_SIZES),
            PrewarmPolicy::Custom(config) => {
                prewarm_sizes.extend(config.sizes.iter().copied());
                runtime_glyphs = Self::glyphs_from_set(&config.glyph_set);
            }
        }

        let mut seen_sizes = HashSet::new();
        let mut normalized_sizes = Vec::new();
        for size in prewarm_sizes {
            let size_q = Self::quantize_font_size(size);
            if seen_sizes.insert(size_q) {
                normalized_sizes.push(Self::dequantize_font_size(size_q));
            }
        }
        normalized_sizes.sort_by(|a, b| a.total_cmp(b));

        let base_q = Self::quantize_font_size(base_size);
        let current_dpi_q = Self::quantize_font_size(self.dpi_scale_factor);
        let mut loaded_sizes = HashMap::new();
        for size in normalized_sizes {
            let size_q = Self::quantize_font_size(size);
            let atlas_key = if size_q == base_q && current_dpi_q == 10 {
                font_key.to_string()
            } else {
                Self::raster_variant_key(font_key, size_q, current_dpi_q)
            };
            self.load_raster_font_variant_from_data(
                leaked,
                &runtime_glyphs,
                size,
                self.dpi_scale_factor,
                &atlas_key,
                options.hinting,
            )?;
            loaded_sizes.insert((size_q, current_dpi_q), RasterSizeEntry { atlas_key });
        }

        let family = RasterFontFamily {
            font_data: leaked,
            default_size: base_size,
            hinting: options.hinting,
            runtime_budget_glyphs_per_frame: options.runtime_budget_glyphs_per_frame,
            runtime_glyphs,
            loaded_sizes,
        };
        self.raster_font_families
            .insert(font_key.to_string(), family);
        self.loaded_fonts.insert(font_key.to_string(), true);
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_font_from_bytes(
        &mut self,
        font_bytes: &[u8],
        logical_font_size: f32,
        font_key: &str,
    ) -> Result<(), FontError> {
        let mut options = FontLoadOptions::default();
        // Keep wasm startup responsive for byte-based loads used by browser paths.
        options.prewarm_policy = PrewarmPolicy::None;
        options.hinting = RasterHintingMode::None;
        self.load_font_with_options_from_bytes_internal(
            font_bytes.to_vec(),
            logical_font_size,
            font_key,
            options,
        )
    }

    pub fn warm_text_cache(
        &mut self,
        font_key: &str,
        prewarm: PrewarmConfig,
    ) -> Result<WarmStats, String> {
        if !self.loaded_fonts.contains_key(font_key) {
            return Err(format!("font '{font_key}' is not loaded"));
        }
        let Some(family) = self.raster_font_families.get(font_key) else {
            return Err(format!(
                "font '{font_key}' does not use raster cache warming"
            ));
        };

        let glyphs = Self::glyphs_from_set(&prewarm.glyph_set);
        let font_data = family.font_data;
        let base_q = Self::quantize_font_size(family.default_size);
        let current_dpi_q = Self::quantize_font_size(self.dpi_scale_factor);
        let hinting = family.hinting;

        let mut seen_sizes = HashSet::new();
        let mut sizes = Vec::new();
        for size in prewarm.sizes {
            let size_q = Self::quantize_font_size(size);
            if seen_sizes.insert(size_q) {
                sizes.push(Self::dequantize_font_size(size_q));
            }
        }

        let mut stats = WarmStats {
            requested_sizes: sizes.len(),
            ..WarmStats::default()
        };

        for size in sizes {
            let size_q = Self::quantize_font_size(size);
            if self
                .raster_font_families
                .get(font_key)
                .is_some_and(|f| f.loaded_sizes.contains_key(&(size_q, current_dpi_q)))
            {
                stats.already_loaded_sizes += 1;
                continue;
            }

            let atlas_key = if size_q == base_q && current_dpi_q == 10 {
                font_key.to_string()
            } else {
                Self::raster_variant_key(font_key, size_q, current_dpi_q)
            };
            self.load_raster_font_variant_from_data(
                font_data,
                &glyphs,
                size,
                self.dpi_scale_factor,
                &atlas_key,
                hinting,
            )
            .map_err(|e| format!("failed to warm '{}': {:?}", font_key, e))?;
            if let Some(family_mut) = self.raster_font_families.get_mut(font_key) {
                family_mut
                    .loaded_sizes
                    .insert((size_q, current_dpi_q), RasterSizeEntry { atlas_key });
            }
            stats.warmed_sizes += 1;
            stats.glyphs_rasterized += glyphs.len();
            self.pending_raster_warm_dedupe
                .remove(&(font_key.to_string(), size_q, current_dpi_q));
        }
        Ok(stats)
    }

    fn load_raster_font_variant_from_data(
        &mut self,
        font_data: &'static [u8],
        glyphs: &[char],
        logical_font_size: f32,
        dpi_scale_factor: f32,
        atlas_key: &str,
        _hinting: RasterHintingMode,
    ) -> Result<(), FontError> {
        if self.text_renderer.font_atlases.contains_key(atlas_key) {
            return Ok(());
        }
        let logical_font_size = Self::sanitize_font_size(logical_font_size);
        // FreeType hinting is unavailable on wasm, so force rusttype atlas generation there.
        #[cfg(target_arch = "wasm32")]
        let use_hinted = false;
        #[cfg(not(target_arch = "wasm32"))]
        let use_hinted = match _hinting {
            RasterHintingMode::None => false,
            RasterHintingMode::Auto => logical_font_size <= AUTO_HINTED_RASTER_MAX_PX,
        };

        let atlas_build = if use_hinted {
            self.build_hinted_raster_atlas_from_font_data(
                font_data,
                logical_font_size,
                dpi_scale_factor,
                glyphs,
            )?
        } else {
            let physical_font_size = logical_font_size * self.dpi_scale_factor;
            let font = Font::try_from_bytes(font_data).ok_or(FontError::InvalidFontData)?;
            let scale = Scale::uniform(physical_font_size);
            let padding = 2;

            let (atlas_width, atlas_height, char_dimensions, max_tile_width, max_tile_height) =
                TextRenderer::calculate_atlas_size_for_chars(&font, scale, padding, glyphs);
            let (texture_data, physical_char_map) = TextRenderer::render_glyphs_to_atlas_for_chars(
                &font,
                scale,
                (atlas_width, atlas_height),
                &char_dimensions,
                padding,
                glyphs,
            )
            .ok_or(FontError::AtlasRenderError)?;

            let mut logical_char_map = HashMap::new();
            for (c, physical_info) in physical_char_map {
                logical_char_map.insert(
                    c,
                    CharacterInfo {
                        tile_index: physical_info.tile_index,
                        advance_width: physical_info.advance_width / dpi_scale_factor,
                        bearing: (
                            physical_info.bearing.0 / dpi_scale_factor,
                            physical_info.bearing.1 / dpi_scale_factor,
                        ),
                        size: physical_info.size,
                    },
                );
            }

            let ascent = font.v_metrics(scale).ascent / dpi_scale_factor;
            let descent = font.v_metrics(scale).descent / dpi_scale_factor;
            RasterAtlasBuild {
                texture_data,
                char_map: logical_char_map,
                atlas_width,
                atlas_height,
                max_tile_width,
                max_tile_height,
                ascent,
                descent,
                padding_pixels: padding,
            }
        };

        let atlas_id = Uuid::new_v4();
        let atlas = self.create_font_texture_atlas(
            atlas_id,
            &atlas_build.texture_data,
            atlas_build.atlas_width,
            atlas_build.atlas_height,
            Size {
                width: atlas_build.max_tile_width as f32,
                height: atlas_build.max_tile_height as f32,
            },
            &atlas_build.char_map,
        );

        let font_static = Font::try_from_bytes(font_data).ok_or(FontError::InvalidFontData)?;
        self.text_renderer
            .fonts
            .insert(atlas_key.to_string(), font_static);

        self.text_renderer.store_font_atlas(
            atlas_key,
            atlas,
            atlas_build.char_map,
            logical_font_size,
            atlas_build.ascent,
            atlas_build.descent,
            Size {
                width: atlas_build.max_tile_width as f32,
                height: atlas_build.max_tile_height as f32,
            },
            dpi_scale_factor,
            atlas_build.padding_pixels,
        );
        Ok(())
    }

    fn queue_raster_warm_request(&mut self, font_key: &str, target_size: f32, target_dpi: f32) {
        let size_q = Self::quantize_font_size(target_size);
        let dpi_q = Self::quantize_font_size(target_dpi);
        if self
            .raster_font_families
            .get(font_key)
            .map(|f| f.loaded_sizes.contains_key(&(size_q, dpi_q)))
            .unwrap_or(true)
        {
            return;
        }
        let dedupe_key = (font_key.to_string(), size_q, dpi_q);
        if self.pending_raster_warm_dedupe.insert(dedupe_key.clone()) {
            self.pending_raster_warm
                .push_back(PendingRasterWarmRequest {
                    family_key: font_key.to_string(),
                    size_q,
                    dpi_q,
                });
        }
    }

    fn resolve_font_key_for_render(
        &mut self,
        font_key: &str,
        font_size_override: Option<f32>,
    ) -> (String, Option<f32>) {
        let Some(family) = self.raster_font_families.get(font_key) else {
            return (font_key.to_string(), font_size_override);
        };
        let target_size =
            Self::sanitize_font_size(font_size_override.unwrap_or(family.default_size));
        match self.choose_loaded_raster_entry(font_key, target_size, self.dpi_scale_factor) {
            Some((entry, is_exact)) => {
                let atlas_key = entry.atlas_key.clone();
                if !is_exact {
                    self.queue_raster_warm_request(font_key, target_size, self.dpi_scale_factor);
                }
                let override_size = if font_size_override.is_some() || !is_exact {
                    Some(target_size)
                } else {
                    None
                };
                (atlas_key, override_size)
            }
            None => {
                self.queue_raster_warm_request(font_key, target_size, self.dpi_scale_factor);
                (font_key.to_string(), font_size_override)
            }
        }
    }

    fn process_runtime_raster_warm_queue(&mut self) {
        if self.pending_raster_warm.is_empty() {
            return;
        }

        let mut remaining_budget: HashMap<String, usize> = HashMap::new();
        let mut deferred = VecDeque::new();

        while let Some(req) = self.pending_raster_warm.pop_front() {
            let dedupe_key = (req.family_key.clone(), req.size_q, req.dpi_q);
            let Some(family) = self.raster_font_families.get(&req.family_key) else {
                self.pending_raster_warm_dedupe.remove(&dedupe_key);
                continue;
            };
            if family.loaded_sizes.contains_key(&(req.size_q, req.dpi_q)) {
                self.pending_raster_warm_dedupe.remove(&dedupe_key);
                continue;
            }

            let glyph_cost = family.runtime_glyphs.len().max(1);
            let budget = remaining_budget
                .entry(req.family_key.clone())
                .or_insert(family.runtime_budget_glyphs_per_frame);
            if *budget < glyph_cost {
                deferred.push_back(req);
                continue;
            }

            let logical_size = Self::dequantize_font_size(req.size_q);
            let dpi_scale = Self::dequantize_font_size(req.dpi_q);
            let default_q = Self::quantize_font_size(family.default_size);
            let atlas_key = if req.size_q == default_q && req.dpi_q == 10 {
                req.family_key.clone()
            } else {
                Self::raster_variant_key(&req.family_key, req.size_q, req.dpi_q)
            };
            let font_data = family.font_data;
            let runtime_glyphs = family.runtime_glyphs.clone();
            let hinting = family.hinting;
            match self.load_raster_font_variant_from_data(
                font_data,
                &runtime_glyphs,
                logical_size,
                dpi_scale,
                &atlas_key,
                hinting,
            ) {
                Ok(()) => {
                    if let Some(family_mut) = self.raster_font_families.get_mut(&req.family_key) {
                        family_mut
                            .loaded_sizes
                            .insert((req.size_q, req.dpi_q), RasterSizeEntry { atlas_key });
                    }
                    *budget = budget.saturating_sub(glyph_cost);
                    self.pending_raster_warm_dedupe.remove(&dedupe_key);
                    self.font_cache_version = self.font_cache_version.wrapping_add(1);
                }
                Err(err) => {
                    eprintln!(
                        "[FONT CACHE] failed to warm '{}' @ {:.2}px: {:?}",
                        req.family_key, logical_size, err
                    );
                    self.pending_raster_warm_dedupe.remove(&dedupe_key);
                }
            }
        }

        self.pending_raster_warm = deferred;
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build_hinted_raster_atlas_from_font_data(
        &mut self,
        font_data: &[u8],
        logical_font_size: f32,
        dpi_scale_factor: f32,
        glyphs: &[char],
    ) -> Result<RasterAtlasBuild, FontError> {
        #[derive(Clone)]
        struct GlyphBitmap {
            ch: char,
            alpha: Vec<u8>,
            width: u32,
            height: u32,
            advance_px: f32,
            bearing_x_px: f32,
            bearing_y_px: f32,
        }

        let logical_font_size = Self::sanitize_font_size(logical_font_size);
        let physical_ppem = (logical_font_size * dpi_scale_factor).round().max(1.0) as u32;
        let padding = 2u32;

        let library = freetype::Library::init().map_err(|e| {
            FontError::FreeTypeError(format!("failed to initialize freetype: {}", e))
        })?;
        let face = library
            .new_memory_face(font_data.to_vec(), 0)
            .map_err(|e| FontError::FreeTypeError(format!("failed to load face: {}", e)))?;
        face.set_pixel_sizes(0, physical_ppem)
            .map_err(|e| FontError::FreeTypeError(format!("failed to set pixel size: {}", e)))?;

        let load_flags = freetype::face::LoadFlag::RENDER
            | freetype::face::LoadFlag::TARGET_LIGHT
            | freetype::face::LoadFlag::FORCE_AUTOHINT;

        let mut rendered = Vec::new();
        let mut max_tile_width = 0u32;
        let mut max_tile_height = 0u32;
        let mut missing = Vec::new();

        for ch in glyphs.iter().copied() {
            if let Err(err) = face.load_char(ch as usize, load_flags) {
                missing.push((ch, err));
                continue;
            }
            let slot = face.glyph();
            let bm = slot.bitmap();
            let width = bm.width().max(0) as u32;
            let rows = bm.rows().max(0) as u32;
            let pitch = bm.pitch().unsigned_abs() as usize;
            let src = bm.buffer();
            let mut alpha = vec![0u8; (width * rows) as usize];

            for y in 0..rows {
                let src_row = if bm.pitch() >= 0 {
                    y as usize
                } else {
                    (rows - 1 - y) as usize
                };
                for x in 0..width {
                    let src_idx = src_row * pitch + x as usize;
                    let dst_idx = (y * width + x) as usize;
                    alpha[dst_idx] = src.get(src_idx).copied().unwrap_or(0);
                }
            }

            let tile_w = width + padding * 2;
            let tile_h = rows + padding * 2;
            max_tile_width = max_tile_width.max(tile_w.max(padding * 2));
            max_tile_height = max_tile_height.max(tile_h.max(padding * 2));

            rendered.push(GlyphBitmap {
                ch,
                alpha,
                width,
                height: rows,
                advance_px: (slot.advance().x as f32) / 64.0,
                bearing_x_px: slot.bitmap_left() as f32,
                bearing_y_px: slot.bitmap_top() as f32,
            });
        }

        if rendered.is_empty() {
            return Err(FontError::AtlasRenderError);
        }
        if !missing.is_empty() {
            eprintln!(
                "[FONT CACHE] skipped {} missing glyphs while hint-rasterizing",
                missing.len()
            );
        }

        let glyph_count = rendered.len().max(1) as u32;
        let requested_cols = 8u32;
        let atlas_width = (max_tile_width.max(1) * requested_cols).next_power_of_two();
        let cols = (atlas_width / max_tile_width.max(1)).max(1);
        let rows = glyph_count.div_ceil(cols).max(1);
        let atlas_height = (max_tile_height.max(1) * rows).next_power_of_two();
        let mut texture_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];
        let mut char_map = HashMap::new();

        for (tile_index, glyph) in rendered.iter().enumerate() {
            let col = (tile_index as u32) % cols;
            let row = (tile_index as u32) / cols;
            let tile_left = col * max_tile_width;
            let tile_top = row * max_tile_height;

            for y in 0..glyph.height {
                for x in 0..glyph.width {
                    let alpha = glyph.alpha[(y * glyph.width + x) as usize];
                    let dst_x = tile_left + padding + x;
                    let dst_y = tile_top + padding + y;
                    let dst = ((dst_y * atlas_width + dst_x) * 4) as usize;
                    texture_data[dst] = 255;
                    texture_data[dst + 1] = 255;
                    texture_data[dst + 2] = 255;
                    texture_data[dst + 3] = alpha;
                }
            }

            char_map.insert(
                glyph.ch,
                CharacterInfo {
                    tile_index,
                    advance_width: glyph.advance_px / dpi_scale_factor,
                    bearing: (
                        glyph.bearing_x_px / dpi_scale_factor,
                        glyph.bearing_y_px / dpi_scale_factor,
                    ),
                    size: (max_tile_width, max_tile_height),
                },
            );
        }

        let size_metrics = face
            .size_metrics()
            .ok_or_else(|| FontError::FreeTypeError("missing freetype size metrics".to_string()))?;
        let ascent = (size_metrics.ascender as f32 / 64.0) / dpi_scale_factor;
        let descent = (size_metrics.descender as f32 / 64.0) / dpi_scale_factor;

        Ok(RasterAtlasBuild {
            texture_data,
            char_map,
            atlas_width,
            atlas_height,
            max_tile_width,
            max_tile_height,
            ascent,
            descent,
            padding_pixels: padding,
        })
    }

    #[cfg(target_arch = "wasm32")]
    fn build_hinted_raster_atlas_from_font_data(
        &mut self,
        _font_data: &[u8],
        _logical_font_size: f32,
        _dpi_scale_factor: f32,
        _glyphs: &[char],
    ) -> Result<RasterAtlasBuild, FontError> {
        Err(FontError::FreeTypeError(
            "hinted raster atlas generation is not available on wasm32".to_string(),
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build_tiny_raster_fallback_from_font_data(
        &mut self,
        font_data: &[u8],
        dpi_scale_factor: f32,
    ) -> Result<TinyRasterFallbackSpec, FontError> {
        #[derive(Clone)]
        struct GlyphBitmap {
            ch: char,
            alpha: Vec<u8>,
            width: u32,
            height: u32,
            advance_px: f32,
            bearing_x_px: f32,
            bearing_y_px: f32,
        }

        let tiny_size = self.text_renderer.quality.tiny_raster_max_px.max(1.0);
        let physical_ppem = (tiny_size * dpi_scale_factor).round().max(1.0) as u32;
        let padding = 1u32;

        let library = freetype::Library::init().map_err(|e| {
            FontError::FreeTypeError(format!("failed to initialize freetype: {}", e))
        })?;
        let face = library
            .new_memory_face(font_data.to_vec(), 0)
            .map_err(|e| FontError::FreeTypeError(format!("failed to load face: {}", e)))?;
        face.set_pixel_sizes(0, physical_ppem)
            .map_err(|e| FontError::FreeTypeError(format!("failed to set pixel size: {}", e)))?;

        // Light auto-hinting keeps tiny ASCII text stable and readable.
        let load_flags = freetype::face::LoadFlag::RENDER
            | freetype::face::LoadFlag::TARGET_LIGHT
            | freetype::face::LoadFlag::FORCE_AUTOHINT;

        let mut glyphs = Vec::new();
        let mut max_tile_width = 0u32;
        let mut max_tile_height = 0u32;

        for ch in (32u8..=126u8).map(|c| c as char) {
            face.load_char(ch as usize, load_flags).map_err(|e| {
                FontError::FreeTypeError(format!("failed to load glyph '{}': {}", ch, e))
            })?;
            let slot = face.glyph();
            let bm = slot.bitmap();
            let width = bm.width().max(0) as u32;
            let rows = bm.rows().max(0) as u32;
            let pitch = bm.pitch().unsigned_abs() as usize;
            let src = bm.buffer();
            let mut alpha = vec![0u8; (width * rows) as usize];

            for y in 0..rows {
                let src_row = if bm.pitch() >= 0 {
                    y as usize
                } else {
                    (rows - 1 - y) as usize
                };
                for x in 0..width {
                    let src_idx = src_row * pitch + x as usize;
                    let dst_idx = (y * width + x) as usize;
                    alpha[dst_idx] = src.get(src_idx).copied().unwrap_or(0);
                }
            }

            let tile_w = width + padding * 2;
            let tile_h = rows + padding * 2;
            max_tile_width = max_tile_width.max(tile_w.max(padding * 2));
            max_tile_height = max_tile_height.max(tile_h.max(padding * 2));
            glyphs.push(GlyphBitmap {
                ch,
                alpha,
                width,
                height: rows,
                advance_px: (slot.advance().x as f32) / 64.0,
                bearing_x_px: slot.bitmap_left() as f32,
                bearing_y_px: slot.bitmap_top() as f32,
            });
        }

        let chars_count = glyphs.len().max(1) as u32;
        let requested_cols = 8u32;
        let atlas_width = (max_tile_width.max(1) * requested_cols).next_power_of_two();
        // UV lookup computes columns as atlas_width / tile_width, so placement must match.
        let cols = (atlas_width / max_tile_width.max(1)).max(1);
        let rows = chars_count.div_ceil(cols).max(1);
        let atlas_height = (max_tile_height.max(1) * rows).next_power_of_two();
        let mut texture_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];
        let mut char_map = HashMap::new();

        for (tile_index, glyph) in glyphs.iter().enumerate() {
            let col = (tile_index as u32) % cols;
            let row = (tile_index as u32) / cols;
            let tile_left = col * max_tile_width;
            let tile_top = row * max_tile_height;

            for y in 0..glyph.height {
                for x in 0..glyph.width {
                    let alpha = glyph.alpha[(y * glyph.width + x) as usize];
                    let dst_x = tile_left + padding + x;
                    let dst_y = tile_top + padding + y;
                    let dst = ((dst_y * atlas_width + dst_x) * 4) as usize;
                    texture_data[dst] = alpha;
                    texture_data[dst + 1] = alpha;
                    texture_data[dst + 2] = alpha;
                    texture_data[dst + 3] = alpha;
                }
            }

            char_map.insert(
                glyph.ch,
                CharacterInfo {
                    tile_index,
                    advance_width: glyph.advance_px / dpi_scale_factor,
                    bearing: (
                        glyph.bearing_x_px / dpi_scale_factor,
                        glyph.bearing_y_px / dpi_scale_factor,
                    ),
                    size: (max_tile_width, max_tile_height),
                },
            );
        }

        let size_metrics = face
            .size_metrics()
            .ok_or_else(|| FontError::FreeTypeError("missing freetype size metrics".to_string()))?;
        let ascent = (size_metrics.ascender as f32 / 64.0) / dpi_scale_factor;
        let descent = (size_metrics.descender as f32 / 64.0) / dpi_scale_factor;

        let atlas_id = Uuid::new_v4();
        let atlas = self.create_font_texture_atlas(
            atlas_id,
            &texture_data,
            atlas_width,
            atlas_height,
            Size {
                width: max_tile_width as f32,
                height: max_tile_height as f32,
            },
            &char_map,
        );
        Ok(TinyRasterFallbackSpec {
            atlas,
            char_map,
            font_size: tiny_size,
            ascent,
            descent,
            padding: padding as f32 / dpi_scale_factor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    fn build_tiny_raster_fallback_from_font_data(
        &mut self,
        _font_data: &[u8],
        _dpi_scale_factor: f32,
    ) -> Result<TinyRasterFallbackSpec, FontError> {
        Err(FontError::FreeTypeError(
            "tiny raster fallback generation is not available on wasm32".to_string(),
        ))
    }

    pub fn load_msdf_font(
        &mut self,
        atlas_image_path: &str,
        metadata_json_path: &str,
        font_key: &str,
    ) -> Result<(), FontError> {
        if self.loaded_fonts.contains_key(font_key) {
            return Ok(());
        }

        let bake_help = format!(
            "MSDF assets are missing or invalid.\n\
             Bake them first with:\n\
               cargo run --bin msdf_bake -- --font /path/to/font.ttf --out-dir /path/to/output --name {font_key}\n\
             Then load:\n\
               atlas: {atlas_image_path}\n\
               meta : {metadata_json_path}\n\
               key  : {font_key}\n\
             Note: current bake pipeline targets ASCII charset (32..=126)."
        );

        let metadata_text = std::fs::read_to_string(metadata_json_path).map_err(|err| {
            FontError::MetadataParseError(format!(
                "failed to read MSDF metadata '{}': {}\n{}",
                metadata_json_path, err, bake_help
            ))
        })?;
        let metadata = TextRenderer::parse_msdf_metadata(&metadata_text).map_err(|err| {
            let detail = match err {
                FontError::MetadataParseError(msg) => msg,
                _ => format!("{:?}", err),
            };
            FontError::MetadataParseError(format!(
                "failed to parse MSDF metadata '{}': {}\n{}",
                metadata_json_path, detail, bake_help
            ))
        })?;

        let decoded = image::open(atlas_image_path).map_err(|e| {
            FontError::ImageDecodeError(format!(
                "failed to open MSDF atlas '{}': {}\n{}",
                atlas_image_path, e, bake_help
            ))
        })?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();

        let expected_w = metadata.atlas.width.round().max(0.0) as u32;
        let expected_h = metadata.atlas.height.round().max(0.0) as u32;
        if expected_w != width || expected_h != height {
            return Err(FontError::MetadataParseError(format!(
                "atlas size mismatch: image={}x{}, metadata={}x{}",
                width, height, expected_w, expected_h
            )));
        }

        let atlas_id = Uuid::new_v4();
        let empty_char_positions: HashMap<char, CharacterInfo> = HashMap::new();
        let atlas = self.create_font_texture_atlas_with_options(
            atlas_id,
            rgba.as_raw(),
            width,
            height,
            Size {
                width: width as f32,
                height: height as f32,
            },
            &empty_char_positions,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::FilterMode::Linear,
            wgpu::FilterMode::Linear,
            true,
        );

        self.text_renderer
            .store_msdf_font_atlas(font_key, atlas, metadata, None)?;
        self.loaded_fonts.insert(font_key.to_string(), true);
        Ok(())
    }

    /// Load an MSDF font from in-memory PNG bytes and a JSON metadata string.
    ///
    /// This is the portable equivalent of [`load_msdf_font`] for embedded or downloaded assets.
    /// Works on both native and WASM targets. Use `include_bytes!` / `include_str!` to embed
    /// pre-baked MSDF assets, avoiding runtime rasterization (critical for WASM performance).
    pub fn load_msdf_font_from_png_bytes(
        &mut self,
        png_bytes: &[u8],
        metadata_json: &str,
        font_key: &str,
    ) -> Result<(), FontError> {
        if self.loaded_fonts.contains_key(font_key) {
            return Ok(());
        }

        let metadata = TextRenderer::parse_msdf_metadata(metadata_json)?;

        let decoded = image::load_from_memory(png_bytes).map_err(|e| {
            FontError::ImageDecodeError(format!("failed to decode MSDF atlas PNG: {}", e))
        })?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();

        let expected_w = metadata.atlas.width.round().max(0.0) as u32;
        let expected_h = metadata.atlas.height.round().max(0.0) as u32;
        if expected_w != width || expected_h != height {
            return Err(FontError::MetadataParseError(format!(
                "atlas size mismatch: image={}x{}, metadata={}x{}",
                width, height, expected_w, expected_h
            )));
        }

        let atlas_id = Uuid::new_v4();
        let empty_char_positions: HashMap<char, CharacterInfo> = HashMap::new();
        let atlas = self.create_font_texture_atlas_with_options(
            atlas_id,
            rgba.as_raw(),
            width,
            height,
            Size {
                width: width as f32,
                height: height as f32,
            },
            &empty_char_positions,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::FilterMode::Linear,
            wgpu::FilterMode::Linear,
            true,
        );

        self.text_renderer
            .store_msdf_font_atlas(font_key, atlas, metadata, None)?;
        self.loaded_fonts.insert(font_key.to_string(), true);
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_msdf_font_from_bytes(
        &mut self,
        atlas_rgba: &[u8],
        width: u32,
        height: u32,
        metadata_json: &str,
        font_key: &str,
    ) -> Result<(), FontError> {
        if self.loaded_fonts.contains_key(font_key) {
            return Ok(());
        }

        let metadata = TextRenderer::parse_msdf_metadata(metadata_json)?;
        let expected_w = metadata.atlas.width.round().max(0.0) as u32;
        let expected_h = metadata.atlas.height.round().max(0.0) as u32;
        if expected_w != width || expected_h != height {
            return Err(FontError::MetadataParseError(format!(
                "atlas size mismatch: image={}x{}, metadata={}x{}",
                width, height, expected_w, expected_h
            )));
        }
        let expected_len = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if atlas_rgba.len() != expected_len {
            return Err(FontError::ImageDecodeError(format!(
                "rgba buffer size mismatch: got {} bytes, expected {}",
                atlas_rgba.len(),
                expected_len
            )));
        }

        let atlas_id = Uuid::new_v4();
        let empty_char_positions: HashMap<char, CharacterInfo> = HashMap::new();
        let atlas = self.create_font_texture_atlas_with_options(
            atlas_id,
            atlas_rgba,
            width,
            height,
            Size {
                width: width as f32,
                height: height as f32,
            },
            &empty_char_positions,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::FilterMode::Linear,
            wgpu::FilterMode::Linear,
            true,
        );

        self.text_renderer
            .store_msdf_font_atlas(font_key, atlas, metadata, None)?;
        self.loaded_fonts.insert(font_key.to_string(), true);
        Ok(())
    }

    pub fn load_msdf_font_with_tiny_raster(
        &mut self,
        font_path: &str,
        atlas_image_path: &str,
        metadata_json_path: &str,
        font_key: &str,
    ) -> Result<(), FontError> {
        if self.loaded_fonts.contains_key(font_key) {
            return Ok(());
        }

        let metadata_text = std::fs::read_to_string(metadata_json_path).map_err(|err| {
            FontError::MetadataParseError(format!(
                "failed to read MSDF metadata '{}': {}",
                metadata_json_path, err
            ))
        })?;
        let metadata = TextRenderer::parse_msdf_metadata(&metadata_text)?;
        let decoded = image::open(atlas_image_path).map_err(|e| {
            FontError::ImageDecodeError(format!("failed to open MSDF atlas: {}", e))
        })?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        let expected_w = metadata.atlas.width.round().max(0.0) as u32;
        let expected_h = metadata.atlas.height.round().max(0.0) as u32;
        if expected_w != width || expected_h != height {
            return Err(FontError::MetadataParseError(format!(
                "atlas size mismatch: image={}x{}, metadata={}x{}",
                width, height, expected_w, expected_h
            )));
        }

        let font_data = std::fs::read(font_path).map_err(FontError::IoError)?;
        let tiny_raster =
            self.build_tiny_raster_fallback_from_font_data(&font_data, self.dpi_scale_factor)?;

        let atlas_id = Uuid::new_v4();
        let empty_char_positions: HashMap<char, CharacterInfo> = HashMap::new();
        let atlas = self.create_font_texture_atlas_with_options(
            atlas_id,
            rgba.as_raw(),
            width,
            height,
            Size {
                width: width as f32,
                height: height as f32,
            },
            &empty_char_positions,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::FilterMode::Linear,
            wgpu::FilterMode::Linear,
            true,
        );
        self.text_renderer
            .store_msdf_font_atlas(font_key, atlas, metadata, Some(tiny_raster))?;
        self.loaded_fonts.insert(font_key.to_string(), true);
        Ok(())
    }

    /// Build an MSDF-compatible runtime atlas from a TTF and register it under `font_key`.
    ///
    /// Conversion is performed once at load time and cached in the engine under `font_key`.
    /// Phase 1 coverage matches current text behavior (ASCII 32..=126).
    pub fn load_msdf_font_from_ttf(
        &mut self,
        font_path: &str,
        logical_font_size: f32,
        font_key: &str,
    ) -> Result<(), FontError> {
        if self.loaded_fonts.contains_key(font_key) {
            return Ok(());
        }

        const RUNTIME_MSDF_GENERATION_SCALE: f32 = 2.0;
        const RUNTIME_MSDF_PX_RANGE: f32 = 8.0;

        let physical_font_size = logical_font_size * self.dpi_scale_factor;
        let generation_font_size =
            (physical_font_size * RUNTIME_MSDF_GENERATION_SCALE).max(physical_font_size);
        let font_data = std::fs::read(font_path).map_err(FontError::IoError)?;
        let font = Font::try_from_vec(font_data.clone()).ok_or(FontError::InvalidFontData)?;
        let scale = Scale::uniform(generation_font_size);
        let padding = 10u32;

        let (atlas_width, atlas_height, char_dimensions, max_tile_width, max_tile_height) =
            TextRenderer::calculate_atlas_size(&font, scale, padding);
        let (texture_data, physical_char_map) = TextRenderer::render_msdf_glyphs_to_atlas(
            &font,
            scale,
            (atlas_width, atlas_height),
            &char_dimensions,
            padding,
            RUNTIME_MSDF_PX_RANGE,
        )
        .ok_or(FontError::AtlasRenderError)?;

        let cols = (atlas_width / max_tile_width.max(1)).max(1);
        let denom = generation_font_size.max(1.0);
        // Keep advance metrics in the same rusttype space used by raster text.
        let shaping_advances: Option<HashMap<char, f32>> = None;
        // Match raster kerning behavior exactly. The runtime text layout path
        // applies pair pen adjustments only, so rusttype pair_kerning is the
        // stable source of truth for spacing parity.
        let mut kerning = Vec::new();
        for left_u in 32u8..=126u8 {
            let left = left_u as char;
            let left_id = font.glyph(left).id();
            for right_u in 32u8..=126u8 {
                let right = right_u as char;
                let right_id = font.glyph(right).id();
                let k = font.pair_kerning(scale, left_id, right_id) / denom;
                if k.abs() > f32::EPSILON {
                    kerning.push(MsdfKerningRecord {
                        left_unicode: left as u32,
                        right_unicode: right as u32,
                        advance: k,
                    });
                }
            }
        }

        // Correct for rusttype normalising by (ascent−descent) instead of upem.
        let em_correction = TextRenderer::compute_rusttype_em_correction(
            &shaping_advances,
            &physical_char_map,
            denom,
        );

        // Keep metadata in the same unit space as rusttype atlas sampling geometry.
        // Shaping data is in upem-space; convert to rusttype-space when available.
        let shaping_advances = shaping_advances.map(|mut advs| {
            for value in advs.values_mut() {
                *value /= em_correction.max(1e-6);
            }
            advs
        });
        let mut glyphs = Vec::new();
        for u in 32u8..=126u8 {
            let ch = u as char;
            let Some(info) = physical_char_map.get(&ch) else {
                continue;
            };
            let col = (info.tile_index as u32) % cols;
            let row = (info.tile_index as u32) / cols;
            let atlas_left = col as f32 * max_tile_width as f32;
            let atlas_top = row as f32 * max_tile_height as f32;

            let denom = generation_font_size.max(1.0);
            // Match plane bounds to the same pixel-box geometry used to build
            // atlas tiles, preventing subtle per-glyph squeeze/stretch.
            let positioned = font
                .glyph(ch)
                .scaled(scale)
                .positioned(rusttype::point(0.0, info.bearing.1));
            let plane_bounds = positioned
                .pixel_bounding_box()
                .map(|bb| {
                    TextRenderer::msdf_plane_bounds_from_pixel_bounds(
                        bb,
                        info.bearing.1,
                        denom,
                        1.0,
                    )
                })
                .unwrap_or(Bounds {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                });

            let advance = shaping_advances
                .as_ref()
                .and_then(|advs| advs.get(&ch).copied())
                .unwrap_or(info.advance_width / denom);

            glyphs.push(MsdfGlyphRecord {
                unicode: ch as u32,
                advance,
                plane_bounds,
                atlas_bounds: Bounds {
                    // Keep padded bounds so the render quad can include the full MSDF field.
                    left: atlas_left,
                    top: atlas_top,
                    right: atlas_left + info.size.0 as f32,
                    bottom: atlas_top + info.size.1 as f32,
                },
            });
        }

        let v_metrics = font.v_metrics(scale);
        let metadata = MsdfFontMetadata {
            atlas: MsdfAtlasInfo {
                width: atlas_width as f32,
                height: atlas_height as f32,
                kind: "msdf".to_string(),
            },
            metrics: MsdfMetrics {
                font_size: logical_font_size.max(1.0),
                ascender: v_metrics.ascent / denom,
                descender: v_metrics.descent / denom,
                line_height: (v_metrics.ascent - v_metrics.descent + v_metrics.line_gap).abs()
                    / denom,
                padding_em: padding as f32 / denom,
                px_range: RUNTIME_MSDF_PX_RANGE,
            },
            glyphs,
            kerning,
        };

        let atlas_id = Uuid::new_v4();
        let empty_char_positions: HashMap<char, CharacterInfo> = HashMap::new();
        let atlas = self.create_font_texture_atlas_with_options(
            atlas_id,
            &texture_data,
            atlas_width,
            atlas_height,
            Size {
                width: atlas_width as f32,
                height: atlas_height as f32,
            },
            &empty_char_positions,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::FilterMode::Linear,
            wgpu::FilterMode::Linear,
            true,
        );

        let tiny_raster = Some(
            self.build_tiny_raster_fallback_from_font_data(&font_data, self.dpi_scale_factor)?,
        );
        self.text_renderer
            .store_msdf_font_atlas(font_key, atlas, metadata, tiny_raster)?;
        self.loaded_fonts.insert(font_key.to_string(), true);
        Ok(())
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

    pub fn queue_texture(&mut self, texture_key: &Uuid, position: Option<Position>) {
        self.queue_texture_with_layer(texture_key, position, 0);
    }

    pub fn queue_texture_with_layer(
        &mut self,
        texture_key: &Uuid,
        position: Option<Position>,
        z: i32,
    ) {
        if let Some(texture) = self.texture_map.get(texture_key) {
            // Generate the transformation matrix based on the position and camera
            let position = position.unwrap_or_default() * self.dpi_scale_factor;
            let transform_uniform = texture.get_transform_uniform(
                self.viewport_size,
                position,
                self.camera.get_pos(self.dpi_scale_factor),
                0.0,
                1.0,
            );
            let transform_index = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z,
                clip_rect: None,
                item: RenderItem::Texture {
                    texture_key: *texture_key,
                    transform_index,
                },
            });
        }
    }

    fn inset_rect_uniform(rect: Rectangle, inset: f32) -> Option<Rectangle> {
        let inset = inset.max(0.0);
        let width = rect.width - (inset * 2.0);
        let height = rect.height - (inset * 2.0);
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        Some(Rectangle::new(
            rect.x + inset,
            rect.y + inset,
            width,
            height,
        ))
    }

    fn queue_texture_stretched_internal(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        z: i32,
        fit: TextureFit,
        clip_rect: Option<Rectangle>,
        camera_position_px: Position,
        inset: f32,
    ) {
        if dst.width <= 0.0 || dst.height <= 0.0 {
            return;
        }
        let Some(texture) = self.texture_map.get(texture_key) else {
            return;
        };

        // Apply inset to the destination rectangle
        let dst = Self::inset_rect_uniform(dst, inset).unwrap_or(dst);

        let dst_left_px = (dst.x * self.dpi_scale_factor) - camera_position_px.x;
        let dst_top_px = (dst.y * self.dpi_scale_factor) - camera_position_px.y;
        let dst_w_px = (dst.width * self.dpi_scale_factor).max(0.0);
        let dst_h_px = (dst.height * self.dpi_scale_factor).max(0.0);
        if dst_w_px <= 0.0 || dst_h_px <= 0.0 {
            return;
        }

        let source_size_px = texture.original_size();
        let (draw_w_px, draw_h_px) = match fit {
            TextureFit::Contain => {
                if source_size_px.width <= 0.0 || source_size_px.height <= 0.0 {
                    return;
                }
                let scale = (dst_w_px / source_size_px.width).min(dst_h_px / source_size_px.height);
                (source_size_px.width * scale, source_size_px.height * scale)
            }
            TextureFit::StretchFill => (dst_w_px, dst_h_px),
            TextureFit::Cover => {
                if source_size_px.width <= 0.0 || source_size_px.height <= 0.0 {
                    return;
                }
                let scale = (dst_w_px / source_size_px.width).max(dst_h_px / source_size_px.height);
                (source_size_px.width * scale, source_size_px.height * scale)
            }
        };
        if draw_w_px <= 0.0 || draw_h_px <= 0.0 {
            return;
        }

        // Contain mode letterboxes by centering inside the destination rect.
        let left_px = (dst_left_px + (dst_w_px - draw_w_px) * 0.5).round();
        let top_px = (dst_top_px + (dst_h_px - draw_h_px) * 0.5).round();

        let width_ndc = 2.0 * (draw_w_px / self.viewport_size.width);
        let height_ndc = 2.0 * (draw_h_px / self.viewport_size.height);
        let ndc_left = 2.0 * (left_px / self.viewport_size.width) - 1.0;
        let ndc_top = 1.0 - 2.0 * (top_px / self.viewport_size.height);

        let transform_uniform = TransformUniform {
            transform: [
                [width_ndc, 0.0, 0.0, 0.0],
                [0.0, height_ndc, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ndc_left, ndc_top, 0.0, 1.0],
            ],
        };
        let transform_index = self.allocate_transform_bind_group(transform_uniform);
        self.render_queue.push(QueuedItem {
            z,
            clip_rect,
            item: RenderItem::Texture {
                texture_key: *texture_key,
                transform_index,
            },
        });
    }

    pub fn queue_texture_stretched(&mut self, texture_key: &Uuid, dst: Rectangle) {
        self.queue_texture_stretched_with_layer_and_fit(
            texture_key,
            dst,
            0,
            TextureFit::Contain,
            0.0,
        );
    }

    pub fn queue_texture_stretched_with_fit(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        fit: TextureFit,
    ) {
        self.queue_texture_stretched_with_layer_and_fit(texture_key, dst, 0, fit, 0.0);
    }

    pub fn queue_texture_stretched_with_layer(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        z: i32,
    ) {
        self.queue_texture_stretched_with_layer_and_fit(
            texture_key,
            dst,
            z,
            TextureFit::Contain,
            0.0,
        );
    }

    pub fn queue_texture_stretched_with_layer_and_fit(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        z: i32,
        fit: TextureFit,
        inset: f32,
    ) {
        self.queue_texture_stretched_internal(
            texture_key,
            dst,
            z,
            fit,
            None,
            self.camera.get_pos(self.dpi_scale_factor),
            inset,
        );
    }

    /// Begin a frame-local slot for layered UI composition.
    ///
    /// The slot rectangle is defined in logical screen-space coordinates and is
    /// reused for both rendering and hit geometry.
    pub fn begin_slot(&mut self, slot_id: Uuid, rect: Rectangle, z_base: i32) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            self.slot_states.remove(&slot_id);
            return;
        }
        self.slot_states.insert(
            slot_id,
            SlotState {
                rect,
                z_base,
                is_open: true,
                clip_radius: None,
            },
        );
    }

    /// End a slot declaration for this frame.
    ///
    /// Slot hit geometry remains queryable until `begin_frame()` clears frame-local state.
    pub fn end_slot(&mut self, slot_id: &Uuid) {
        if let Some(slot) = self.slot_states.get_mut(slot_id) {
            slot.is_open = false;
        }
    }

    /// Returns the slot's logical hit rectangle in screen-space.
    pub fn slot_hit_rect(&self, slot_id: &Uuid) -> Option<Rectangle> {
        self.slot_states.get(slot_id).map(|slot| slot.rect)
    }

    /// Placeholder for future rounded-corner slot clipping.
    ///
    /// v1 uses rectangular scissor clipping; this value is stored but not applied yet.
    pub fn set_slot_clip_radius(&mut self, slot_id: &Uuid, radius: Option<f32>) {
        if let Some(slot) = self.slot_states.get_mut(slot_id) {
            slot.clip_radius = radius.map(|r| r.max(0.0));
        }
    }

    pub fn queue_slot_layer_texture(
        &mut self,
        slot_id: &Uuid,
        texture_key: &Uuid,
        fit: TextureFit,
        z_offset: i32,
        inset: Option<f32>,
    ) -> bool {
        let Some(slot) = self.slot_states.get(slot_id).copied() else {
            return false;
        };
        if !slot.is_open {
            return false;
        }
        let Some(layer_rect) = Self::inset_rect_uniform(slot.rect, inset.unwrap_or(0.0)) else {
            return false;
        };
        if !self.texture_map.contains_key(texture_key) {
            return false;
        }
        self.queue_texture_stretched_internal(
            texture_key,
            layer_rect,
            slot.z_base + z_offset,
            fit,
            Some(slot.rect),
            Position::default(),
            0.0,
        );
        true
    }

    fn queue_rect_internal(
        &mut self,
        bounds: Rectangle,
        color: [f32; 4],
        corner_radius_px: f32,
        border: Option<([f32; 4], f32)>,
        z: i32,
        clip_rect: Option<Rectangle>,
        camera_position_px: Position,
    ) {
        let pos = Position {
            x: bounds.x,
            y: bounds.y,
        } * self.dpi_scale_factor;
        let size = bounds.size() * self.dpi_scale_factor;
        let width_ndc = size.width / self.viewport_size.width;
        let height_ndc = size.height / self.viewport_size.height;
        let ndc_dx = (2.0 * (pos.x - camera_position_px.x)) / self.viewport_size.width - 1.0;
        let ndc_dy = 1.0 - (2.0 * (pos.y - camera_position_px.y)) / self.viewport_size.height;
        let ndc_x = ndc_dx + width_ndc;
        let ndc_y = ndc_dy - height_ndc;
        let model = [
            [width_ndc, 0.0, 0.0, 0.0],
            [0.0, height_ndc, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [ndc_x, ndc_y, 0.0, 1.0],
        ];
        let (border_color, border_thickness) = border.unwrap_or(([0.0, 0.0, 0.0, 0.0], 0.0));
        let cmd = RectCommand {
            width_px: size.width,
            height_px: size.height,
            color,
            corner_radius_px,
            border_thickness_px: border_thickness,
            border_color,
            transform: model,
            z,
        };
        self.render_queue.push(QueuedItem {
            z,
            clip_rect,
            item: RenderItem::Rect(cmd),
        });
    }

    pub fn queue_slot_layer_rect(
        &mut self,
        slot_id: &Uuid,
        color: [f32; 4],
        border: Option<([f32; 4], f32)>,
        z_offset: i32,
        inset: Option<f32>,
    ) -> bool {
        let Some(slot) = self.slot_states.get(slot_id).copied() else {
            return false;
        };
        if !slot.is_open {
            return false;
        }
        let Some(layer_rect) = Self::inset_rect_uniform(slot.rect, inset.unwrap_or(0.0)) else {
            return false;
        };
        self.queue_rect_internal(
            layer_rect,
            color,
            0.0,
            border,
            slot.z_base + z_offset,
            Some(slot.rect),
            Position::default(),
        );
        true
    }

    pub fn queue_tile(
        &mut self,
        texture_key: &Uuid,
        tile_index: usize,
        position: Position,
        user_scale: f32,
    ) {
        self.queue_tile_with_layer(texture_key, tile_index, position, user_scale, 0);
    }

    pub fn queue_tile_with_layer(
        &mut self,
        texture_key: &Uuid,
        tile_index: usize,
        position: Position,
        user_scale: f32,
        z: i32,
    ) {
        self.queue_tile_with_tint(
            texture_key,
            tile_index,
            position,
            user_scale,
            z,
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    pub fn queue_tile_with_tint(
        &mut self,
        texture_key: &Uuid,
        tile_index: usize,
        position: Position,
        user_scale: f32,
        z: i32,
        tint: [f32; 4],
    ) {
        if let Some(atlas) = self.atlas_map.get(texture_key) {
            let transform_uniform = atlas.get_transform_uniform(
                self.viewport_size,
                position,
                self.camera.get_pos(self.dpi_scale_factor),
                self.dpi_scale_factor, // position scale (DPI)
                user_scale,            // tile size scale (already physical px)
            );
            let transform_index = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z,
                clip_rect: None,
                item: RenderItem::AtlasTile {
                    texture_key: *texture_key,
                    transform_index,
                    tile_index,
                    tint,
                },
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn queue_atlas_uv_with_tint(
        &mut self,
        texture_key: &Uuid,
        position: Position,
        size: Size,
        uv_offset: [f32; 2],
        uv_scale: [f32; 2],
        z: i32,
        tint: [f32; 4],
        is_msdf: bool,
        msdf_px_range: f32,
    ) {
        self.queue_atlas_uv_with_tint_internal(
            texture_key,
            position,
            size,
            uv_offset,
            uv_scale,
            z,
            tint,
            is_msdf,
            msdf_px_range,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn queue_atlas_uv_with_tint_internal(
        &mut self,
        texture_key: &Uuid,
        position: Position,
        size: Size,
        uv_offset: [f32; 2],
        uv_scale: [f32; 2],
        z: i32,
        tint: [f32; 4],
        is_msdf: bool,
        msdf_px_range: f32,
    ) {
        if !self.atlas_map.contains_key(texture_key) {
            return;
        }
        if size.width <= 0.0 || size.height <= 0.0 {
            return;
        }

        let cam = self.camera.get_pos(self.dpi_scale_factor);
        let left_raw = (position.x * self.dpi_scale_factor) - cam.x;
        let top_raw = (position.y * self.dpi_scale_factor) - cam.y;
        let right_raw = ((position.x + size.width) * self.dpi_scale_factor) - cam.x;
        let bottom_raw = ((position.y + size.height) * self.dpi_scale_factor) - cam.y;
        // Keep MSDF quads in continuous space to avoid per-glyph quantization errors.
        let (left_px, top_px, right_px, bottom_px) = if is_msdf {
            (left_raw, top_raw, right_raw, bottom_raw)
        } else {
            (
                left_raw.round(),
                top_raw.round(),
                right_raw.round(),
                bottom_raw.round(),
            )
        };
        let px_w = (right_px - left_px).max(0.0);
        let px_h = (bottom_px - top_px).max(0.0);
        if px_w <= 0.0 || px_h <= 0.0 {
            return;
        }

        let width_ndc = 2.0 * (px_w / self.viewport_size.width);
        let height_ndc = 2.0 * (px_h / self.viewport_size.height);
        let ndc_left = 2.0 * (left_px / self.viewport_size.width) - 1.0;
        let ndc_top = -2.0 * (top_px / self.viewport_size.height) + 1.0;
        let ndc_x = ndc_left + width_ndc * 0.5;
        let ndc_y = ndc_top - height_ndc * 0.5;

        let transform_uniform = TransformUniform {
            transform: [
                [width_ndc, 0.0, 0.0, 0.0],
                [0.0, height_ndc, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ndc_x, ndc_y, 0.0, 1.0],
            ],
        };
        let transform_index = self.allocate_transform_bind_group(transform_uniform);
        self.render_queue.push(QueuedItem {
            z,
            clip_rect: None,
            item: RenderItem::AtlasGlyph {
                texture_key: *texture_key,
                transform_index,
                uv_offset,
                uv_scale,
                tint,
                is_msdf,
                msdf_px_range,
            },
        });
    }

    pub fn queue_text(
        &mut self,
        text: &str,
        font_key: &str,
        position: Position,
        container: &TextContainer,
    ) {
        self.queue_text_with_spacing(
            text,
            font_key,
            position,
            container,
            0.0,
            0.0,
            0,
            [1.0, 1.0, 1.0, 1.0],
            None,
        );
    }

    pub fn queue_text_with_spacing(
        &mut self,
        text: &str,
        font_key: &str,
        position: Position,
        container: &TextContainer,
        letter_spacing: f32,
        word_spacing: f32,
        z: i32,
        color: [f32; 4],
        font_size_override: Option<f32>,
    ) {
        let (resolved_font_key, resolved_size_override) =
            self.resolve_font_key_for_render(font_key, font_size_override);
        let chars = self.text_renderer.calculate_text_layout(
            text,
            &resolved_font_key,
            position,
            container,
            letter_spacing,
            word_spacing,
            resolved_size_override,
        );
        for char in chars {
            match char.mode {
                GlyphRenderMode::AtlasTile { tile_index, scale } => {
                    self.queue_tile_with_tint(
                        &char.atlas_id,
                        tile_index,
                        char.position,
                        scale,
                        z,
                        color,
                    );
                }
                GlyphRenderMode::AtlasUv {
                    uv_offset,
                    uv_scale,
                    is_msdf,
                    msdf_px_range,
                } => {
                    self.queue_atlas_uv_with_tint_internal(
                        &char.atlas_id,
                        char.position,
                        char.size,
                        uv_offset,
                        uv_scale,
                        z,
                        color,
                        is_msdf,
                        msdf_px_range,
                    );
                }
            }
        }
    }
    pub fn clear_render_queue(&mut self) {
        self.render_queue.clear();
    }

    pub fn debug_render_queue_len(&self) -> usize {
        self.render_queue.len()
    }

    pub fn debug_surface_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn debug_viewport_size(&self) -> Size {
        self.viewport_size
    }

    pub fn unload_texture(&mut self, texture_key: &Uuid) -> bool {
        self.texture_map.remove(texture_key).is_some()
    }

    pub fn unload_atlas(&mut self, atlas_key: &Uuid) -> bool {
        self.atlas_map.remove(atlas_key).is_some()
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(());
        }

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                match e {
                    wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                        // Reconfigure the surface and skip this frame
                        self.surface.configure(&self.device, &self.config);
                        return Ok(());
                    }
                    wgpu::SurfaceError::OutOfMemory => {
                        return Err(e);
                    }
                    wgpu::SurfaceError::Timeout => {
                        // Skip this frame and try again on the next one
                        return Ok(());
                    }
                }
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        // GPU timestamp begin
        let (qset, qbuf, qcount) = (
            self.timestamp_query.as_ref(),
            self.timestamp_buf.as_ref(),
            self.timestamp_count,
        );
        let qindex = if qcount >= 2 {
            self.timestamp_frame_index % (qcount / 2)
        } else {
            0
        };
        let q0 = qindex * 2;
        let q1 = q0 + 1;
        // We'll write timestamps via render pass timestamp_writes when supported.

        {
            // sort by z, stable to preserve submission order within same layer
            self.render_queue.sort_by(|a, b| a.z.cmp(&b.z));

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: qset.map(|qs| wgpu::RenderPassTimestampWrites {
                    query_set: qs,
                    beginning_of_pass_write_index: Some(q0),
                    end_of_pass_write_index: Some(q1),
                }),
                occlusion_query_set: None,
            });

            // Set default pipeline for texture/atlas draws; rect draws will override temporarily
            rpass.set_pipeline(&self.render_pipeline);

            // Streaming batcher that preserves z-order and interleaves atlas draws
            let mut current_tex: Option<Uuid> = None;
            let mut batch_indices: Vec<usize> = Vec::new();
            let mut current_atlas: Option<Uuid> = None;
            let mut current_atlas_is_msdf = false;
            let mut atlas_instances: Vec<crate::utils::InstanceRaw> = Vec::new();
            // Rect batching
            let mut rect_instances: Vec<crate::utils::RectInstanceRaw> = Vec::new();
            let mut rect_draw_calls: usize = 0;
            // Glow batching
            let mut glow_instances: Vec<crate::utils::GlowInstanceRaw> = Vec::new();
            let mut active_item_clip: Option<Rectangle> = None;
            let mut scissor_initialized = false;

            // Helper to flush a pending sprite batch
            let flush_batch = |rpass: &mut wgpu::RenderPass<'_>,
                               tex_id: Option<Uuid>,
                               indices: &mut Vec<usize>| {
                if indices.is_empty() {
                    return;
                }
                if let Some(tid) = tex_id {
                    if let Some(texture) = self.texture_map.get(&tid) {
                        // Build per-instance data: model + uv (full sprite)
                        let instances: Vec<crate::utils::InstanceRaw> = indices
                            .iter()
                            .map(|i| crate::utils::InstanceRaw {
                                model: self.transform_pool.cpu_mats[*i],
                                uv_offset: [0.0, 0.0],
                                uv_scale: [1.0, 1.0],
                                tint: [1.0, 1.0, 1.0, 1.0],
                                msdf_px_range: 0.0,
                                _msdf_pad: [0.0, 0.0, 0.0],
                            })
                            .collect();
                        let instance_buffer =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("instance data (sprite)"),
                                    contents: bytemuck::cast_slice(&instances),
                                    usage: wgpu::BufferUsages::STORAGE,
                                });
                        let instance_bg =
                            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                layout: &self.instance_bind_group_layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: instance_buffer.as_entire_binding(),
                                }],
                                label: Some("instance_bind_group"),
                            });

                        // Bind texture, identity world, uv and instance buffer
                        rpass.set_bind_group(0, texture.bind_group(), &[]);
                        rpass.set_bind_group(3, &instance_bg, &[]);

                        let identity = TransformUniform {
                            transform: [
                                [1.0, 0.0, 0.0, 0.0],
                                [0.0, 1.0, 0.0, 0.0],
                                [0.0, 0.0, 1.0, 0.0],
                                [0.0, 0.0, 0.0, 1.0],
                            ],
                        };
                        let id_buf =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("id-ubo"),
                                    contents: bytemuck::bytes_of(&identity),
                                    usage: wgpu::BufferUsages::UNIFORM,
                                });
                        let id_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &self.transform_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: id_buf.as_entire_binding(),
                            }],
                            label: Some("id-bg"),
                        });
                        rpass.set_bind_group(1, &id_bg, &[]);
                        rpass.set_bind_group(2, texture.uv_bind_group(), &[]);
                        rpass.set_vertex_buffer(0, texture.vertex_buffer_slice());
                        rpass.set_index_buffer(
                            texture.index_buffer_slice(),
                            wgpu::IndexFormat::Uint16,
                        );
                        rpass.draw_indexed(0..texture.num_indices(), 0, 0..(indices.len() as u32));
                    }
                }
                indices.clear();
            };

            // Macro to flush a pending rect batch (replaces closure to avoid borrow conflicts)
            macro_rules! flush_rect_batch {
                ($rpass:expr, $instances:expr) => {
                    if !$instances.is_empty() {
                        let bytes_needed = ($instances.len()
                            * std::mem::size_of::<crate::utils::RectInstanceRaw>())
                            as u64;
                        let mut chosen: Option<usize> = None;
                        for (i, entry) in self.rect_instance_pool.iter().enumerate() {
                            if !entry.used_this_frame && entry.capacity >= bytes_needed {
                                chosen = Some(i);
                                break;
                            }
                        }
                        let idx = if let Some(i) = chosen {
                            i
                        } else {
                            let cap = bytes_needed.next_power_of_two().max(256);
                            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("rect-instance-buffer"),
                                size: cap,
                                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            });
                            let bind_group =
                                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                    label: Some("rect-instance-bg"),
                                    layout: &self.instance_bind_group_layout,
                                    entries: &[wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: buffer.as_entire_binding(),
                                    }],
                                });
                            self.rect_instance_pool.push(RectInstanceBuffer {
                                buffer,
                                capacity: cap,
                                bind_group,
                                used_this_frame: false,
                                last_used_frame: self.frame_counter,
                            });
                            self.rect_instance_pool.len() - 1
                        };
                        {
                            let entry = &mut self.rect_instance_pool[idx];
                            if entry.capacity < bytes_needed {
                                let cap = bytes_needed.next_power_of_two();
                                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                                    label: Some("rect-instance-buffer"),
                                    size: cap,
                                    usage: wgpu::BufferUsages::STORAGE
                                        | wgpu::BufferUsages::COPY_DST,
                                    mapped_at_creation: false,
                                });
                                entry.buffer = buffer;
                                entry.capacity = cap;
                                entry.bind_group =
                                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                        label: Some("rect-instance-bg"),
                                        layout: &self.instance_bind_group_layout,
                                        entries: &[wgpu::BindGroupEntry {
                                            binding: 0,
                                            resource: entry.buffer.as_entire_binding(),
                                        }],
                                    });
                            }
                            self.queue.write_buffer(
                                &entry.buffer,
                                0,
                                bytemuck::cast_slice(&$instances),
                            );
                            entry.used_this_frame = true;
                            entry.last_used_frame = self.frame_counter;
                        }

                        if self.rect_identity_bg.is_none() {
                            let identity = TransformUniform {
                                transform: [
                                    [1.0, 0.0, 0.0, 0.0],
                                    [0.0, 1.0, 0.0, 0.0],
                                    [0.0, 0.0, 1.0, 0.0],
                                    [0.0, 0.0, 0.0, 1.0],
                                ],
                            };
                            let id_buf =
                                self.device
                                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                        label: Some("rect-id-ubo"),
                                        contents: bytemuck::bytes_of(&identity),
                                        usage: wgpu::BufferUsages::UNIFORM,
                                    });
                            let id_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                layout: &self.transform_bind_group_layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: id_buf.as_entire_binding(),
                                }],
                                label: Some("rect-id-bg"),
                            });
                            self.rect_identity_bg = Some(id_bg);
                        }

                        $rpass.set_pipeline(&self.rect_pipeline);
                        $rpass.set_bind_group(0, &self.rect_dummy_bg, &[]);
                        $rpass.set_bind_group(1, self.rect_identity_bg.as_ref().unwrap(), &[]);
                        $rpass.set_bind_group(2, &self.rect_dummy_bg, &[]);
                        $rpass.set_bind_group(3, &self.rect_instance_pool[idx].bind_group, &[]);
                        $rpass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
                        $rpass.set_index_buffer(
                            self.rect_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        $rpass.draw_indexed(0..6, 0, 0..($instances.len() as u32));
                        rect_draw_calls += 1;
                        $instances.clear();
                    }
                };
            }

            // Macro to flush pending glow instances inline
            macro_rules! flush_glow_batch {
                ($rpass:expr, $glow_insts:expr) => {
                    if !$glow_insts.is_empty() {
                        let glow_buf =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("glow-instance-buffer"),
                                    contents: bytemuck::cast_slice(&$glow_insts),
                                    usage: wgpu::BufferUsages::STORAGE,
                                });
                        let glow_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("glow-instance-bg"),
                            layout: &self.glow_instance_bgl,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: glow_buf.as_entire_binding(),
                            }],
                        });
                        if self.rect_identity_bg.is_none() {
                            let identity = TransformUniform {
                                transform: [
                                    [1.0, 0.0, 0.0, 0.0],
                                    [0.0, 1.0, 0.0, 0.0],
                                    [0.0, 0.0, 1.0, 0.0],
                                    [0.0, 0.0, 0.0, 1.0],
                                ],
                            };
                            let id_buf =
                                self.device
                                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                        label: Some("glow-id-ubo"),
                                        contents: bytemuck::bytes_of(&identity),
                                        usage: wgpu::BufferUsages::UNIFORM,
                                    });
                            let id_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                layout: &self.transform_bind_group_layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: id_buf.as_entire_binding(),
                                }],
                                label: Some("glow-id-bg"),
                            });
                            self.rect_identity_bg = Some(id_bg);
                        }
                        $rpass.set_pipeline(&self.glow_pipeline);
                        $rpass.set_bind_group(0, &self.rect_dummy_bg, &[]);
                        $rpass.set_bind_group(1, self.rect_identity_bg.as_ref().unwrap(), &[]);
                        $rpass.set_bind_group(2, &self.rect_dummy_bg, &[]);
                        $rpass.set_bind_group(3, &glow_bg, &[]);
                        $rpass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
                        $rpass.set_index_buffer(
                            self.rect_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        $rpass.draw_indexed(0..6, 0, 0..($glow_insts.len() as u32));
                        $glow_insts.clear();
                    }
                };
            }

            macro_rules! flush_atlas_batch {
                ($rpass:expr, $instances:expr, $atlas_id:expr, $is_msdf:expr) => {
                    if !$instances.is_empty() {
                        if let Some(aid) = $atlas_id {
                            if let Some(atlas) = self.atlas_map.get(&aid) {
                                let instance_buffer = self.device.create_buffer_init(
                                    &wgpu::util::BufferInitDescriptor {
                                        label: Some("instance data (atlas)"),
                                        contents: bytemuck::cast_slice(&$instances),
                                        usage: wgpu::BufferUsages::STORAGE,
                                    },
                                );
                                let instance_bg =
                                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                        layout: &self.instance_bind_group_layout,
                                        entries: &[wgpu::BindGroupEntry {
                                            binding: 0,
                                            resource: instance_buffer.as_entire_binding(),
                                        }],
                                        label: Some("atlas-instance-bg"),
                                    });
                                let identity = TransformUniform {
                                    transform: [
                                        [1.0, 0.0, 0.0, 0.0],
                                        [0.0, 1.0, 0.0, 0.0],
                                        [0.0, 0.0, 1.0, 0.0],
                                        [0.0, 0.0, 0.0, 1.0],
                                    ],
                                };
                                let id_buf = self.device.create_buffer_init(
                                    &wgpu::util::BufferInitDescriptor {
                                        label: Some("id-ubo"),
                                        contents: bytemuck::bytes_of(&identity),
                                        usage: wgpu::BufferUsages::UNIFORM,
                                    },
                                );
                                let id_bg =
                                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                        layout: &self.transform_bind_group_layout,
                                        entries: &[wgpu::BindGroupEntry {
                                            binding: 0,
                                            resource: id_buf.as_entire_binding(),
                                        }],
                                        label: Some("id-bg"),
                                    });
                                if $is_msdf {
                                    $rpass.set_pipeline(&self.msdf_render_pipeline);
                                } else {
                                    $rpass.set_pipeline(&self.render_pipeline);
                                }
                                $rpass.set_bind_group(0, &atlas.bind_group, &[]);
                                $rpass.set_bind_group(1, &id_bg, &[]);
                                $rpass.set_bind_group(2, atlas.default_uv_bind_group(), &[]);
                                $rpass.set_bind_group(3, &instance_bg, &[]);
                                $rpass.set_vertex_buffer(0, atlas.vertex_buffer.slice(..));
                                $rpass.set_index_buffer(
                                    atlas.index_buffer.slice(..),
                                    wgpu::IndexFormat::Uint16,
                                );
                                $rpass.draw_indexed(
                                    0..atlas.num_indices,
                                    0,
                                    0..($instances.len() as u32),
                                );
                            }
                        }
                        $instances.clear();
                    }
                };
            }

            for q in &self.render_queue {
                let effective_item_clip = self.effective_item_clip_rect(q.clip_rect);
                if !scissor_initialized || effective_item_clip != active_item_clip {
                    flush_batch(&mut rpass, current_tex, &mut batch_indices);
                    current_tex = None;
                    flush_atlas_batch!(
                        rpass,
                        atlas_instances,
                        current_atlas,
                        current_atlas_is_msdf
                    );
                    flush_rect_batch!(rpass, rect_instances);
                    flush_glow_batch!(rpass, glow_instances);
                    self.apply_scissor_logical(&mut rpass, effective_item_clip);
                    active_item_clip = effective_item_clip;
                    scissor_initialized = true;
                    rpass.set_pipeline(&self.render_pipeline);
                }

                match &q.item {
                    RenderItem::Texture {
                        texture_key,
                        transform_index,
                    } => {
                        // Switching away from rects/glows; flush pending batches
                        flush_rect_batch!(rpass, rect_instances);
                        flush_glow_batch!(rpass, glow_instances);
                        // Switch back to texture/atlas pipeline after rects
                        rpass.set_pipeline(&self.render_pipeline);
                        match current_tex {
                            Some(tid) if tid == *texture_key => {
                                batch_indices.push(*transform_index);
                            }
                            _ => {
                                // different texture; flush previous
                                flush_batch(&mut rpass, current_tex, &mut batch_indices);
                                flush_atlas_batch!(
                                    rpass,
                                    atlas_instances,
                                    current_atlas,
                                    current_atlas_is_msdf
                                );
                                current_tex = Some(*texture_key);
                                batch_indices.push(*transform_index);
                            }
                        }
                    }
                    RenderItem::AtlasTile {
                        texture_key,
                        transform_index,
                        tile_index,
                        tint,
                    } => {
                        // Switching away from rects/glows; flush pending batches
                        flush_rect_batch!(rpass, rect_instances);
                        flush_glow_batch!(rpass, glow_instances);
                        // Switch back to texture/atlas pipeline after rects
                        rpass.set_pipeline(&self.render_pipeline);
                        // flush any sprite batch first
                        flush_batch(&mut rpass, current_tex, &mut batch_indices);
                        current_tex = None;
                        // switch atlas batch if needed
                        if current_atlas != Some(*texture_key) || current_atlas_is_msdf {
                            flush_atlas_batch!(
                                rpass,
                                atlas_instances,
                                current_atlas,
                                current_atlas_is_msdf
                            );
                            current_atlas = Some(*texture_key);
                            current_atlas_is_msdf = false;
                        }
                        if let Some(atlas) = self.atlas_map.get(texture_key) {
                            let model = self.transform_pool.cpu_mats[*transform_index];
                            if let Some(uv_rect) =
                                crate::texture_atlas::TextureAtlas::tile_uv_coordinates(
                                    *tile_index,
                                    atlas.tile_size,
                                    atlas.dimensions.size(),
                                )
                            {
                                atlas_instances.push(crate::utils::InstanceRaw {
                                    model,
                                    uv_offset: [uv_rect.x, uv_rect.y],
                                    uv_scale: [uv_rect.width, uv_rect.height],
                                    tint: *tint,
                                    msdf_px_range: 0.0,
                                    _msdf_pad: [0.0, 0.0, 0.0],
                                });
                            }
                        }
                    }
                    RenderItem::AtlasGlyph {
                        texture_key,
                        transform_index,
                        uv_offset,
                        uv_scale,
                        tint,
                        is_msdf,
                        msdf_px_range,
                    } => {
                        flush_rect_batch!(rpass, rect_instances);
                        flush_glow_batch!(rpass, glow_instances);
                        flush_batch(&mut rpass, current_tex, &mut batch_indices);
                        current_tex = None;

                        if current_atlas != Some(*texture_key) || current_atlas_is_msdf != *is_msdf
                        {
                            flush_atlas_batch!(
                                rpass,
                                atlas_instances,
                                current_atlas,
                                current_atlas_is_msdf
                            );
                            current_atlas = Some(*texture_key);
                            current_atlas_is_msdf = *is_msdf;
                        }

                        let model = self.transform_pool.cpu_mats[*transform_index];
                        atlas_instances.push(crate::utils::InstanceRaw {
                            model,
                            uv_offset: *uv_offset,
                            uv_scale: *uv_scale,
                            tint: *tint,
                            msdf_px_range: *msdf_px_range,
                            _msdf_pad: [0.0, 0.0, 0.0],
                        });
                    }
                    RenderItem::Rect(cmd) => {
                        // Flush any pending sprite/atlas/glow batches before enqueueing rects
                        flush_batch(&mut rpass, current_tex, &mut batch_indices);
                        current_tex = None;
                        flush_glow_batch!(rpass, glow_instances);
                        flush_atlas_batch!(
                            rpass,
                            atlas_instances,
                            current_atlas,
                            current_atlas_is_msdf
                        );

                        // Enqueue rect instance for batching
                        rect_instances.push(crate::utils::RectInstanceRaw {
                            model: cmd.transform,
                            color: cmd.color,
                            corner_radius_px: cmd.corner_radius_px,
                            border_thickness_px: cmd.border_thickness_px,
                            _pad0: [0.0, 0.0],
                            border_color: cmd.border_color,
                            rect_size_px: [cmd.width_px, cmd.height_px],
                            _pad1: [0.0, 0.0],
                            _pad2: [0.0, 0.0, 0.0, 0.0],
                        });
                        // Metrics: track style diversity and counts (no reordering/grouping)
                        self.rect_instances_count = self.rect_instances_count.saturating_add(1);
                        let key = RectStyleKey {
                            fill_rgba_u8: to_rgba_u8(cmd.color),
                            border_rgba_u8: to_rgba_u8(cmd.border_color),
                            corner_radius_10x: quant_10x(cmd.corner_radius_px),
                            border_thickness_10x: quant_10x(cmd.border_thickness_px),
                        };
                        self.rect_style_keys.insert(key);
                    }
                    RenderItem::Glow(cmd) => {
                        // Flush any pending sprite/atlas/rect batches
                        flush_batch(&mut rpass, current_tex, &mut batch_indices);
                        current_tex = None;
                        flush_atlas_batch!(
                            rpass,
                            atlas_instances,
                            current_atlas,
                            current_atlas_is_msdf
                        );
                        flush_rect_batch!(rpass, rect_instances);

                        glow_instances.push(crate::utils::GlowInstanceRaw {
                            model: cmd.transform,
                            color: cmd.color,
                            rect_size_px: [cmd.width_px, cmd.height_px],
                            corner_radius_px: cmd.corner_radius_px,
                            glow_radius_px: cmd.glow_radius_px,
                            sigma: cmd.sigma,
                            max_alpha: cmd.max_alpha,
                            mode: cmd.mode,
                            border_width: cmd.border_width,
                            _pad: [0.0, 0.0, 0.0, 0.0],
                        });
                    }
                }
            }
            // flush any remaining sprite batch
            flush_batch(&mut rpass, current_tex, &mut batch_indices);
            // flush any remaining atlas batch
            flush_atlas_batch!(rpass, atlas_instances, current_atlas, current_atlas_is_msdf);
            // flush any remaining rects
            flush_rect_batch!(rpass, rect_instances);
            // flush any remaining glows
            flush_glow_batch!(rpass, glow_instances);
            self.rect_draw_calls_count = rect_draw_calls;
        }
        // End timestamp + resolve
        if let (Some(qs), Some(buf)) = (qset, qbuf) {
            let base = (((q0 as u64) * 8) / wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
                * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            encoder.resolve_query_set(qs, q0..(q1 + 1), buf, base);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        // Read back timestamps (synchronously for simplicity)
        if let (Some(src), Some(dst)) = (&self.timestamp_buf, &self.timestamp_staging) {
            // Copy resolved results into MAP_READ staging
            let mut enc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("copy ts"),
                });
            let base = (((q0 as u64) * 8) / wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
                * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            enc.copy_buffer_to_buffer(src, base, dst, base, wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT);
            self.queue.submit(Some(enc.finish()));
            let start = base;
            let end = start + 16;
            let slice = dst.slice(start..end);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res.is_ok());
            });
            // Block until mapping completes
            self.device.poll(wgpu::Maintain::Wait);
            if rx.recv().unwrap_or(false) {
                let data = slice.get_mapped_range();
                if data.len() >= 16 {
                    let t0 = u64::from_le_bytes(data[0..8].try_into().unwrap());
                    let t1 = u64::from_le_bytes(data[8..16].try_into().unwrap());
                    if t1 > t0 {
                        let dt_ns = (t1 - t0) as f64 * (self.timestamp_period_ns as f64);
                        let dt_s = (dt_ns / 1_000_000_000.0) as f32;
                        self.gpu_metrics.record(dt_s);
                        if let Some(line) = self.gpu_metrics.maybe_report() {
                            println!("gpu_{}", line);
                        }
                    }
                }
                drop(data);
                dst.unmap();
            }
        }
        self.timestamp_frame_index = self.timestamp_frame_index.wrapping_add(1);
        Ok(())
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

    fn popup_font_key(&self) -> Option<String> {
        if self.loaded_fonts.contains_key("roboto") {
            return Some("roboto".to_string());
        }
        if self.loaded_fonts.contains_key("default") {
            return Some("default".to_string());
        }
        let mut keys: Vec<&str> = self.loaded_fonts.keys().map(String::as_str).collect();
        keys.sort_unstable();
        keys.first().map(|k| (*k).to_string())
    }

    fn popup_button_colors(style: PopupActionStyle) -> ([f32; 4], [f32; 4]) {
        match style {
            PopupActionStyle::Primary => ([0.20, 0.45, 0.88, 1.0], [0.14, 0.33, 0.70, 1.0]),
            PopupActionStyle::Secondary => ([0.24, 0.27, 0.32, 1.0], [0.16, 0.18, 0.22, 1.0]),
            PopupActionStyle::Danger => ([0.70, 0.22, 0.18, 1.0], [0.55, 0.16, 0.14, 1.0]),
        }
    }

    fn render_popup_overlay(&mut self) {
        let Some(active) = self.popup_state.active().cloned() else {
            return;
        };
        let layout = popup_layout_for_active(&active, self.screen_space_viewport_rect().size());
        let is_custom = active.custom_panel_rect.is_some();

        let z_scrim = 900_000;
        let z_panel = z_scrim + 10;
        let z_text = z_scrim + 20;
        let z_button = z_scrim + 30;
        let z_button_text = z_scrim + 40;

        let world_viewport = self.halo_world_rect_from_screen_rect(layout.viewport);
        let world_panel = self.halo_world_rect_from_screen_rect(layout.panel);
        let world_title_rect = self.halo_world_rect_from_screen_rect(layout.title_rect);
        let world_message_rect = self.halo_world_rect_from_screen_rect(layout.message_rect);

        self.draw_rect(world_viewport, [0.0, 0.0, 0.0, 0.62], 0.0, None, z_scrim);
        self.draw_rect(
            world_panel,
            [0.11, 0.13, 0.17, 1.0],
            14.0,
            Some(([0.30, 0.34, 0.42, 1.0], 1.0)),
            z_panel,
        );

        if is_custom {
            for object_id in &active.custom_object_ids {
                if let Some(object) = self.pluto_objects.get(object_id).cloned() {
                    object.borrow().render(self);
                }
            }
            return;
        }

        for (idx, action_rect) in layout.action_rects.iter().enumerate() {
            let Some(action) = active.config.actions.get(idx) else {
                continue;
            };
            let world_action_rect = self.halo_world_rect_from_screen_rect(*action_rect);
            let (mut fill, border) = Self::popup_button_colors(action.style);
            if active.hovered_action == Some(idx) {
                fill = [
                    (fill[0] + 0.08).min(1.0),
                    (fill[1] + 0.08).min(1.0),
                    (fill[2] + 0.08).min(1.0),
                    1.0,
                ];
            }
            if active.pressed_action == Some(idx) {
                fill = [fill[0] * 0.85, fill[1] * 0.85, fill[2] * 0.85, 1.0];
            }
            self.draw_rect(world_action_rect, fill, 10.0, Some((border, 1.0)), z_button);
        }

        if let Some(font_key) = self.popup_font_key() {
            let title_container = TextContainer::new(world_title_rect)
                .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                .with_padding(0.0);
            self.queue_text_with_spacing(
                &active.config.title,
                &font_key,
                world_title_rect.pos(),
                &title_container,
                0.0,
                0.0,
                z_text,
                [0.96, 0.97, 1.0, 1.0],
                Some(26.0),
            );

            let message_container = TextContainer::new(world_message_rect)
                .with_alignment(HorizontalAlignment::Left, VerticalAlignment::Top)
                .with_padding(0.0);
            let wrapped_message = self.wrap_popup_message_text(
                &active.config.message,
                &font_key,
                18.0,
                world_message_rect.width,
            );
            self.queue_text_with_spacing(
                &wrapped_message,
                &font_key,
                world_message_rect.pos(),
                &message_container,
                0.0,
                0.0,
                z_text,
                [0.86, 0.88, 0.92, 1.0],
                Some(18.0),
            );

            for (idx, action_rect) in layout.action_rects.iter().enumerate() {
                let Some(action) = active.config.actions.get(idx) else {
                    continue;
                };
                let world_action_rect = self.halo_world_rect_from_screen_rect(*action_rect);
                let action_container = TextContainer::new(world_action_rect)
                    .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                    .with_padding(0.0);
                self.queue_text_with_spacing(
                    &action.label,
                    &font_key,
                    world_action_rect.pos(),
                    &action_container,
                    0.0,
                    0.0,
                    z_button_text,
                    [0.98, 0.98, 1.0, 1.0],
                    Some(16.0),
                );
            }
        }
    }

    fn wrap_popup_message_text(
        &self,
        message: &str,
        font_key: &str,
        font_size: f32,
        max_width: f32,
    ) -> String {
        if message.is_empty() || max_width <= 0.0 {
            return String::new();
        }

        let mut wrapped_lines: Vec<String> = Vec::new();
        for segment in message.split('\n') {
            if segment.trim().is_empty() {
                wrapped_lines.push(String::new());
                continue;
            }

            let mut current_line = String::new();
            for word in segment.split_whitespace() {
                let mut remaining_word = word.to_string();
                loop {
                    let candidate = if current_line.is_empty() {
                        remaining_word.clone()
                    } else {
                        format!("{} {}", current_line, remaining_word)
                    };
                    let candidate_width = self
                        .measure_text(&candidate, font_key, 0.0, 0.0, Some(font_size))
                        .0;
                    if candidate_width <= max_width {
                        current_line = candidate;
                        break;
                    }

                    if !current_line.is_empty() {
                        wrapped_lines.push(current_line);
                        current_line = String::new();
                        continue;
                    }

                    let chunk = Self::largest_fitting_prefix(&remaining_word, max_width, |s| {
                        self.measure_text(s, font_key, 0.0, 0.0, Some(font_size)).0
                    });
                    if chunk.is_empty() {
                        if let Some(first_char) = remaining_word.chars().next() {
                            wrapped_lines.push(first_char.to_string());
                            remaining_word = remaining_word[first_char.len_utf8()..].to_string();
                            if remaining_word.is_empty() {
                                break;
                            }
                            continue;
                        }
                        break;
                    }

                    wrapped_lines.push(chunk.clone());
                    remaining_word = remaining_word[chunk.len()..].to_string();
                    if remaining_word.is_empty() {
                        break;
                    }
                }
            }

            if !current_line.is_empty() {
                wrapped_lines.push(current_line);
            }
        }

        wrapped_lines.join("\n")
    }

    fn largest_fitting_prefix<F>(text: &str, max_width: f32, width_fn: F) -> String
    where
        F: Fn(&str) -> f32,
    {
        let mut best_end = 0usize;
        let mut ends: Vec<usize> = text.char_indices().skip(1).map(|(idx, _)| idx).collect();
        ends.push(text.len());
        for end in ends {
            let candidate = &text[..end];
            if width_fn(candidate) <= max_width {
                best_end = end;
            } else {
                break;
            }
        }
        if best_end == 0 {
            return String::new();
        }
        text[..best_end].to_string()
    }

    // Convenience immediate-mode draws for consistent naming
    pub fn draw_texture(&mut self, texture_key: &Uuid, position: Position, params: DrawParams) {
        // rotation only supported by direct draw path for sprites; augment transform
        if let Some(texture) = self.texture_map.get(texture_key) {
            let transform_uniform = texture.get_transform_uniform(
                self.viewport_size,
                position * self.dpi_scale_factor,
                self.camera.get_pos(self.dpi_scale_factor),
                params.rotation,
                params.scale,
            );
            let idx = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z: params.z,
                clip_rect: None,
                item: RenderItem::Texture {
                    texture_key: *texture_key,
                    transform_index: idx,
                },
            });
        }
    }

    pub fn draw_texture_stretched(&mut self, texture_key: &Uuid, dst: Rectangle) {
        self.queue_texture_stretched(texture_key, dst);
    }

    pub fn draw_texture_stretched_with_fit_and_inset(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        fit: TextureFit,
        inset: f32,
        z: i32,
    ) {
        self.queue_texture_stretched_with_layer_and_fit(texture_key, dst, z, fit, inset);
    }

    pub fn draw_tile(
        &mut self,
        atlas_key: &Uuid,
        tile_index: usize,
        position: Position,
        params: DrawParams,
    ) {
        let user_scale = if params.scale == 0.0 {
            1.0
        } else {
            params.scale
        };
        self.queue_tile_with_layer(atlas_key, tile_index, position, user_scale, params.z);
    }

    // Draw an atlas tile stretched to an arbitrary destination rectangle (non-uniform scale)
    pub fn draw_atlas_tile_stretched(
        &mut self,
        atlas_key: &Uuid,
        tile_index: usize,
        dst: Rectangle,
        z: i32,
    ) {
        if !self.atlas_map.contains_key(atlas_key) {
            return;
        }
        // Convert logical to physical pixels via DPI scale
        let cam = self.camera.get_pos(self.dpi_scale_factor);
        let left_px = ((dst.x * self.dpi_scale_factor) - cam.x).round();
        let top_px = ((dst.y * self.dpi_scale_factor) - cam.y).round();
        let right_px = (((dst.x + dst.width) * self.dpi_scale_factor) - cam.x).round();
        let bottom_px = (((dst.y + dst.height) * self.dpi_scale_factor) - cam.y).round();
        let px_w = (right_px - left_px).max(0.0);
        let px_h = (bottom_px - top_px).max(0.0);

        // NDC scale for 0..1 unit quad
        let width_ndc = 2.0 * (px_w / self.viewport_size.width);
        let height_ndc = 2.0 * (px_h / self.viewport_size.height);

        // NDC translation for top-left (0,0) of unit quad
        let ndc_left = 2.0 * (left_px / self.viewport_size.width) - 1.0;
        let ndc_top = 1.0 - 2.0 * (top_px / self.viewport_size.height);

        let transform_uniform = TransformUniform {
            transform: [
                [width_ndc, 0.0, 0.0, 0.0],
                [0.0, height_ndc, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ndc_left, ndc_top, 0.0, 1.0],
            ],
        };
        let transform_index = self.allocate_transform_bind_group(transform_uniform);
        self.render_queue.push(QueuedItem {
            z,
            clip_rect: None,
            item: RenderItem::AtlasTile {
                texture_key: *atlas_key,
                transform_index,
                tile_index,
                tint: [1.0, 1.0, 1.0, 1.0],
            },
        });
    }

    // Immediate-mode rect draw (UI primitive)
    pub fn draw_rect(
        &mut self,
        bounds: Rectangle,
        color: [f32; 4],
        corner_radius_px: f32,
        border: Option<([f32; 4], f32)>,
        z: i32,
    ) {
        self.queue_rect_internal(
            bounds,
            color,
            corner_radius_px,
            border,
            z,
            None,
            self.camera.get_pos(self.dpi_scale_factor),
        );
    }

    fn halo_world_rect_from_screen_rect(&self, rect: Rectangle) -> Rectangle {
        let cam_px = self.camera.get_pos(self.dpi_scale_factor);
        let cam_logical = Position {
            x: cam_px.x / self.dpi_scale_factor,
            y: cam_px.y / self.dpi_scale_factor,
        };
        Rectangle::new(
            rect.x + cam_logical.x,
            rect.y + cam_logical.y,
            rect.width,
            rect.height,
        )
    }

    fn halo_screen_rect_from_world_rect(&self, rect: Rectangle) -> Rectangle {
        let cam_px = self.camera.get_pos(self.dpi_scale_factor);
        let cam_logical = Position {
            x: cam_px.x / self.dpi_scale_factor,
            y: cam_px.y / self.dpi_scale_factor,
        };
        Rectangle::new(
            rect.x - cam_logical.x,
            rect.y - cam_logical.y,
            rect.width,
            rect.height,
        )
    }

    fn screen_space_viewport_rect(&self) -> Rectangle {
        Rectangle::new(
            0.0,
            0.0,
            self.viewport_size.width / self.dpi_scale_factor,
            self.viewport_size.height / self.dpi_scale_factor,
        )
    }

    fn rects_intersect(a: Rectangle, b: Rectangle) -> bool {
        if a.width <= 0.0 || a.height <= 0.0 || b.width <= 0.0 || b.height <= 0.0 {
            return false;
        }
        let ax2 = a.x + a.width;
        let ay2 = a.y + a.height;
        let bx2 = b.x + b.width;
        let by2 = b.y + b.height;
        a.x < bx2 && ax2 > b.x && a.y < by2 && ay2 > b.y
    }

    fn rect_intersection(a: Rectangle, b: Rectangle) -> Option<Rectangle> {
        let x1 = a.x.max(b.x);
        let y1 = a.y.max(b.y);
        let x2 = (a.x + a.width).min(b.x + b.width);
        let y2 = (a.y + a.height).min(b.y + b.height);
        let w = (x2 - x1).max(0.0);
        let h = (y2 - y1).max(0.0);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some(Rectangle::new(x1, y1, w, h))
    }

    fn effective_item_clip_rect(&self, item_clip: Option<Rectangle>) -> Option<Rectangle> {
        let global_clip = self.clip_stack.last().copied().or(self.current_scissor);
        match (global_clip, item_clip) {
            (Some(global), Some(item)) => {
                Self::rect_intersection(global, item).or(Some(Rectangle::new(0.0, 0.0, 0.0, 0.0)))
            }
            (Some(global), None) => Some(global),
            (None, Some(item)) => Some(item),
            (None, None) => None,
        }
    }

    fn apply_scissor_logical(
        &self,
        rpass: &mut wgpu::RenderPass<'_>,
        clip_rect: Option<Rectangle>,
    ) {
        if let Some(sc) = clip_rect {
            let x_phys = (sc.x * self.dpi_scale_factor).floor() as i32;
            let y_phys = (sc.y * self.dpi_scale_factor).floor() as i32;
            let w_phys = (sc.width * self.dpi_scale_factor).floor() as i32;
            let h_phys = (sc.height * self.dpi_scale_factor).floor() as i32;

            // Intersect physical rect with render target boundaries [0, 0, width, height]
            let x = x_phys.clamp(0, self.config.width as i32) as u32;
            let y = y_phys.clamp(0, self.config.height as i32) as u32;

            let x2 = (x_phys + w_phys).clamp(0, self.config.width as i32) as u32;
            let y2 = (y_phys + h_phys).clamp(0, self.config.height as i32) as u32;

            let w = x2.saturating_sub(x);
            let h = y2.saturating_sub(y);

            if w > 0 && h > 0 {
                rpass.set_scissor_rect(x, y, w, h);
            } else {
                // If the intersection is empty (off-screen), set a 1x1 rect at [0,0]
                // and we expect the batcher to ideally skip this or just let it draw nothing.
                // wgpu requires w > 0 and h > 0.
                rpass.set_scissor_rect(0, 0, 1, 1);
            }
        } else {
            rpass.set_scissor_rect(0, 0, self.config.width, self.config.height);
        }
    }

    fn draw_halo_world_rect(&mut self, world_rect: Rectangle, style: HaloStyle) {
        let radius = style.radius.max(0.0);
        if radius <= f32::EPSILON {
            return;
        }

        let glow_radius = radius + style.inner_padding.max(0.0);

        // Compute sigma
        let (mode_f, sigma) = match style.mode {
            HaloMode::Glow => (0.0_f32, glow_radius / 2.5),
            HaloMode::Border => (1.0_f32, style.border_width / 2.0),
        };

        // Effective alpha incorporating pulse
        let effective_alpha = style.alpha_at(0.0);
        if effective_alpha <= 0.0 {
            return;
        }

        // Build oversized quad: inner rect expanded by glow_radius on each side
        let oversized_bounds = Rectangle::new(
            world_rect.x - glow_radius,
            world_rect.y - glow_radius,
            world_rect.width + glow_radius * 2.0,
            world_rect.height + glow_radius * 2.0,
        );

        // Same coordinate math as draw_rect
        let pos = Position {
            x: oversized_bounds.x,
            y: oversized_bounds.y,
        } * self.dpi_scale_factor;
        let size = oversized_bounds.size() * self.dpi_scale_factor;
        let width_ndc = size.width / self.viewport_size.width;
        let height_ndc = size.height / self.viewport_size.height;
        let ndc_dx = (2.0 * (pos.x - self.camera.get_pos(self.dpi_scale_factor).x))
            / self.viewport_size.width
            - 1.0;
        let ndc_dy = 1.0
            - (2.0 * (pos.y - self.camera.get_pos(self.dpi_scale_factor).y))
                / self.viewport_size.height;
        let ndc_x = ndc_dx + width_ndc;
        let ndc_y = ndc_dy - height_ndc;
        let model = [
            [width_ndc, 0.0, 0.0, 0.0],
            [0.0, height_ndc, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [ndc_x, ndc_y, 0.0, 1.0],
        ];

        // Inner rect size in physical pixels
        let inner_size = world_rect.size() * self.dpi_scale_factor;

        let rgb = [
            style.color[0].clamp(0.0, 1.0),
            style.color[1].clamp(0.0, 1.0),
            style.color[2].clamp(0.0, 1.0),
        ];

        let cmd = GlowCommand {
            transform: model,
            color: [rgb[0], rgb[1], rgb[2], effective_alpha],
            width_px: inner_size.width,
            height_px: inner_size.height,
            corner_radius_px: style.corner_radius * self.dpi_scale_factor,
            glow_radius_px: glow_radius * self.dpi_scale_factor,
            sigma: sigma * self.dpi_scale_factor,
            max_alpha: style.max_alpha.clamp(0.0, 1.0),
            mode: mode_f,
            border_width: style.border_width * self.dpi_scale_factor,
        };
        self.render_queue.push(QueuedItem {
            z: style.z,
            clip_rect: None,
            item: RenderItem::Glow(cmd),
        });
    }

    /// Draw a configurable outward-radiating halo around a screen-space rectangle.
    ///
    /// `target_rect` is in logical screen-space coordinates (origin at top-left of viewport).
    ///
    /// Safety/clamp rules:
    /// - `radius <= 0` results in no draw
    /// - `ring_count` is clamped to at least `1`
    /// - `inner_padding < 0` is clamped to `0`
    /// - alpha behavior follows [`HaloStyle::alpha_at`]
    pub fn draw_halo(&mut self, target_rect: Rectangle, style: HaloStyle) {
        let world_rect = self.halo_world_rect_from_screen_rect(target_rect);
        self.draw_halo_world_rect(world_rect, style);
    }

    /// Draw a neon-style perimeter glow around a rectangle.
    ///
    /// This follows the SDF of a rounded rectangle, extending both inward and outward.
    /// `rect` is in logical screen-space coordinates.
    pub fn draw_rect_glow(
        &mut self,
        rect: Rectangle,
        color: [f32; 4],
        thickness: f32,     // Width of the core "sharp" line (0.0 for pure soft glow)
        glow_radius: f32,   // How far the soft glow extends from the edge
        corner_radius: f32, // Corner rounding radius
        intensity: f32,     // Alpha/brightness multiplier
        z: i32,
    ) {
        let world_rect = self.halo_world_rect_from_screen_rect(rect);
        let glow_radius_capped = glow_radius.max(0.0);
        let thickness_capped = thickness.max(0.0);

        // Quad must cover the rect plus the glow radius on each side.
        // The shader uses local_pos_ndc [-1, 1] mapped to this quad.
        let oversized_bounds = Rectangle::new(
            world_rect.x - glow_radius_capped,
            world_rect.y - glow_radius_capped,
            world_rect.width + glow_radius_capped * 2.0,
            world_rect.height + glow_radius_capped * 2.0,
        );

        // Compute NDC transform for the quad
        let pos = Position {
            x: oversized_bounds.x,
            y: oversized_bounds.y,
        } * self.dpi_scale_factor;
        let size = oversized_bounds.size() * self.dpi_scale_factor;
        let width_ndc = size.width / self.viewport_size.width;
        let height_ndc = size.height / self.viewport_size.height;
        let ndc_dx = (2.0 * (pos.x - self.camera.get_pos(self.dpi_scale_factor).x))
            / self.viewport_size.width
            - 1.0;
        let ndc_dy = 1.0
            - (2.0 * (pos.y - self.camera.get_pos(self.dpi_scale_factor).y))
                / self.viewport_size.height;
        let ndc_x = ndc_dx + width_ndc;
        let ndc_y = ndc_dy - height_ndc;
        let model = [
            [width_ndc, 0.0, 0.0, 0.0],
            [0.0, height_ndc, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [ndc_x, ndc_y, 0.0, 1.0],
        ];

        // Inner rect size in physical pixels
        let inner_size = world_rect.size() * self.dpi_scale_factor;

        let cmd = GlowCommand {
            transform: model,
            color: [color[0], color[1], color[2], color[3] * intensity],
            width_px: inner_size.width,
            height_px: inner_size.height,
            corner_radius_px: corner_radius * self.dpi_scale_factor,
            glow_radius_px: glow_radius_capped * self.dpi_scale_factor, // This field is for quad sizing in shader, but we use it for falloff (sigma) too
            sigma: glow_radius_capped * self.dpi_scale_factor,
            max_alpha: (color[3] * intensity).clamp(0.0, 1.0),
            mode: 2.0, // Use the new mode
            border_width: thickness_capped * self.dpi_scale_factor,
        };
        self.render_queue.push(QueuedItem {
            z,
            clip_rect: None,
            item: RenderItem::Glow(cmd),
        });
    }

    fn allocate_transform_bind_group(&mut self, transform_uniform: TransformUniform) -> usize {
        // Reuse existing entry if available, else create new
        if self.transform_pool.cursor < self.transform_pool.buffers.len() {
            let idx = self.transform_pool.cursor;
            self.queue.write_buffer(
                &self.transform_pool.buffers[idx],
                0,
                bytemuck::bytes_of(&transform_uniform),
            );
            self.transform_pool.cursor += 1;
            self.transform_pool
                .cpu_mats
                .push(transform_uniform.transform);
            idx
        } else {
            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Transform UBO (pooled)"),
                    contents: bytemuck::bytes_of(&transform_uniform),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.transform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
                label: Some("Transform BG (pooled)"),
            });
            self.transform_pool.buffers.push(buffer);
            self.transform_pool.bind_groups.push(bind_group);
            let idx = self.transform_pool.cursor;
            self.transform_pool.cursor += 1;
            self.transform_pool
                .cpu_mats
                .push(transform_uniform.transform);
            idx
        }
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

    #[cfg(all(feature = "raster", target_arch = "wasm32"))]
    pub async fn create_texture_raster_from_url(
        &mut self,
        url: &str,
        position: Position,
    ) -> Result<(Uuid, Size), RasterTextureLoadError> {
        let bytes = Self::fetch_raster_texture_bytes(url).await?;
        self.create_texture_raster_from_bytes_internal(&bytes, position)
    }

    #[cfg(all(feature = "raster", target_arch = "wasm32"))]
    pub fn begin_texture_raster_from_url(
        &mut self,
        url: &str,
        position: Position,
    ) -> RasterTextureUrlLoadHandle {
        let request_id = Uuid::new_v4();
        let state: Rc<RefCell<Option<Result<Vec<u8>, RasterTextureLoadError>>>> =
            Rc::new(RefCell::new(None));
        let state_for_task = Rc::clone(&state);
        let url_owned = url.to_string();

        wasm_bindgen_futures::spawn_local(async move {
            let result = Self::fetch_raster_texture_bytes(&url_owned).await;
            *state_for_task.borrow_mut() = Some(result);
        });

        self.pending_raster_url_loads
            .insert(request_id, PendingRasterTextureUrlLoad { position, state });
        RasterTextureUrlLoadHandle(request_id)
    }

    #[cfg(all(feature = "raster", target_arch = "wasm32"))]
    pub fn poll_texture_raster_from_url(
        &mut self,
        handle: RasterTextureUrlLoadHandle,
    ) -> Option<Result<(Uuid, Size), RasterTextureLoadError>> {
        let (position, maybe_result) = {
            let pending = self.pending_raster_url_loads.get(&handle.0)?;
            let ready = pending.state.borrow_mut().take();
            (pending.position, ready)
        };

        let result = maybe_result?;
        self.pending_raster_url_loads.remove(&handle.0);

        Some(match result {
            Ok(bytes) => self.create_texture_raster_from_bytes_internal(&bytes, position),
            Err(err) => Err(err),
        })
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

    /// Draw halo for a retained Pluto object.
    ///
    /// Returns `false` and draws nothing when:
    /// - object id was not found
    /// - object bounds are non-positive (`width <= 0` or `height <= 0`)
    /// - object is fully offscreen
    ///
    /// Returns `true` when halo is submitted.
    pub fn draw_halo_for_object(&mut self, object_id: &Uuid, style: HaloStyle) -> bool {
        let bounds = match self.pluto_objects.get(object_id) {
            Some(obj) => obj.borrow().dimensions(),
            None => return false,
        };
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return false;
        }
        let screen_bounds = self.halo_screen_rect_from_world_rect(bounds);
        if !Self::rects_intersect(screen_bounds, self.screen_space_viewport_rect()) {
            return false;
        }
        self.draw_halo(screen_bounds, style);
        true
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

        // Optional GPU timestamp query setup
        let mut timestamp_query: Option<wgpu::QuerySet> = None;
        let mut timestamp_buf: Option<wgpu::Buffer> = None;
        let mut timestamp_count: u32 = 0;
        let timestamp_period_ns: f32 = queue.get_timestamp_period();
        let mut timestamp_staging_buf: Option<wgpu::Buffer> = None;
        if device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            // 2 queries per frame across a small ring buffer
            timestamp_count = 128;
            timestamp_query = Some(device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("gpu-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: timestamp_count,
            }));
            timestamp_buf = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu-timestamps-buffer"),
                size: (timestamp_count as u64) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            // Staging buffer for CPU readback
            let staging = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu-timestamps-staging"),
                size: (timestamp_count as u64) * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            timestamp_staging_buf = Some(staging);
        }

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
            timestamp_query,
            timestamp_buf,
            timestamp_staging: timestamp_staging_buf,
            timestamp_period_ns,
            timestamp_count,
            timestamp_frame_index: 0,
            gpu_metrics: FrameTimeMetrics::new(600, 5.0),
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
