//! Raster-font loading, atlas building, warm-queue processing, and raster-texture loading.
//!
//! All items in this module are either `pub` (matching the public API surface in lib.rs) or
//! `pub(crate)` (cross-called from lib.rs but not part of the public API surface).

use crate::text::{CharacterInfo, FontError, OwnedFont, TextRenderer, TinyRasterFallbackSpec};
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use crate::texture_svg::TextureSVG;
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use crate::utils::Position;
use crate::utils::Size;
use crate::{
    FontLoadOptions, GlyphSet, PlutoniumEngine, PrewarmConfig, PrewarmPolicy, RasterHintingMode,
    WarmStats,
};
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use crate::{RasterTextureLoadError, RasterTextureUrlLoadHandle};
use rusttype::{Font, Scale};
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use std::cell::RefCell;
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use std::rc::Rc;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use wasm_bindgen::JsCast;
#[cfg(all(feature = "raster", target_arch = "wasm32"))]
use wasm_bindgen_futures::JsFuture;

// ── module-private constants ──────────────────────────────────────────────────

pub(crate) const LIGHT_PREWARM_SIZES: [f32; 6] = [12.0, 14.0, 16.0, 18.0, 24.0, 32.0];
pub(crate) const FONT_SIZE_QUANTIZATION: f32 = 100.0;
pub(crate) const DEFAULT_RUNTIME_GLYPH_BUDGET_PER_FRAME: usize = 128;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const AUTO_HINTED_RASTER_MAX_PX: f32 = 48.0;

// ── helper structs (pub(crate) — referenced by PlutoniumEngine fields) ────────

#[cfg(all(feature = "raster", target_arch = "wasm32"))]
#[derive(Clone)]
pub(crate) struct PendingRasterTextureUrlLoad {
    pub(crate) position: Position,
    pub(crate) state: Rc<RefCell<Option<Result<Vec<u8>, RasterTextureLoadError>>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct RasterSizeEntry {
    pub(crate) atlas_key: String,
}

#[derive(Debug)]
pub(crate) struct RasterFontFamily {
    pub(crate) font_data: Arc<[u8]>,
    pub(crate) default_size: f32,
    pub(crate) hinting: RasterHintingMode,
    pub(crate) runtime_budget_glyphs_per_frame: usize,
    pub(crate) runtime_glyphs: Vec<char>,
    pub(crate) loaded_sizes: HashMap<(u32, u32), RasterSizeEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingRasterWarmRequest {
    pub(crate) family_key: String,
    pub(crate) size_q: u32,
    pub(crate) dpi_q: u32,
}

pub(crate) struct RasterAtlasBuild {
    pub(crate) texture_data: Vec<u8>,
    pub(crate) char_map: HashMap<char, CharacterInfo>,
    pub(crate) atlas_width: u32,
    pub(crate) atlas_height: u32,
    pub(crate) max_tile_width: u32,
    pub(crate) max_tile_height: u32,
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    pub(crate) padding_pixels: u32,
}

// ── impl block: raster-font cluster ──────────────────────────────────────────

impl<'a> PlutoniumEngine<'a> {
    // ── wasm-only raster-texture helpers ──────────────────────────────────

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

    // ── static sanitize/quantize helpers ──────────────────────────────────

    fn sanitize_font_size(size: f32) -> f32 {
        size.max(1.0)
    }

    pub(crate) fn sanitize_dpi_scale_factor(scale_factor: f64) -> f32 {
        if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor as f32
        } else {
            1.0
        }
    }

    fn quantize_font_size(size: f32) -> u32 {
        (Self::sanitize_font_size(size) * FONT_SIZE_QUANTIZATION).round() as u32
    }

    pub(crate) fn dequantize_font_size(size_q: u32) -> f32 {
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

    // ── raster entry selection & font-key resolution ───────────────────────

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

    pub(crate) fn resolve_font_key_for_measure(
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

    // ── public font-loading API ────────────────────────────────────────────

    /// Loads font.
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

    /// Loads font with options.
    pub fn load_font_with_options(
        &mut self,
        font_path: &str,
        logical_font_size: f32,
        font_key: &str,
        options: FontLoadOptions,
    ) -> Result<(), FontError> {
        let font_data = std::fs::read(font_path).map_err(FontError::from)?;
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
        let font_bytes: Arc<[u8]> = Arc::from(font_data.into_boxed_slice());
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
                font_bytes.clone(),
                &runtime_glyphs,
                size,
                self.dpi_scale_factor,
                &atlas_key,
                options.hinting,
            )?;
            loaded_sizes.insert((size_q, current_dpi_q), RasterSizeEntry { atlas_key });
        }

        let family = RasterFontFamily {
            font_data: font_bytes,
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

    // ── warm_text_cache ────────────────────────────────────────────────────

    /// Warm text cache.
    pub fn warm_text_cache(
        &mut self,
        font_key: &str,
        prewarm: PrewarmConfig,
    ) -> Result<WarmStats, FontError> {
        if !self.loaded_fonts.contains_key(font_key) {
            return Err(FontError::InvalidFontData);
        }
        let Some(family) = self.raster_font_families.get(font_key) else {
            return Err(FontError::InvalidFontData);
        };

        let glyphs = Self::glyphs_from_set(&prewarm.glyph_set);
        let font_data = family.font_data.clone();
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
                font_data.clone(),
                &glyphs,
                size,
                self.dpi_scale_factor,
                &atlas_key,
                hinting,
            )
            .map_err(|e| {
                FontError::FreeTypeError(format!("failed to warm '{}': {}", font_key, e))
            })?;
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

    // ── private atlas builders & warm-queue internals ──────────────────────

    fn load_raster_font_variant_from_data(
        &mut self,
        font_data: Arc<[u8]>,
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
                font_data.as_ref(),
                logical_font_size,
                dpi_scale_factor,
                glyphs,
            )?
        } else {
            let physical_font_size = logical_font_size * self.dpi_scale_factor;
            let font =
                Font::try_from_bytes(font_data.as_ref()).ok_or(FontError::InvalidFontData)?;
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
        )?;

        let owned_font =
            OwnedFont::from_arc(font_data.clone()).ok_or(FontError::InvalidFontData)?;
        self.text_renderer
            .fonts
            .insert(atlas_key.to_string(), owned_font);

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

    pub(crate) fn queue_raster_warm_request(
        &mut self,
        font_key: &str,
        target_size: f32,
        target_dpi: f32,
    ) {
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

    pub(crate) fn resolve_font_key_for_render(
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

    pub(crate) fn process_runtime_raster_warm_queue(&mut self) {
        if self.pending_raster_warm.is_empty() {
            return;
        }

        let mut remaining_budget = std::mem::take(&mut self.runtime_raster_warm_budget);
        remaining_budget.clear();
        let mut deferred = std::mem::take(&mut self.pending_raster_warm_deferred);
        deferred.clear();

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
            let font_data = family.font_data.clone();
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
                    log::warn!(
                        "[FONT CACHE] failed to warm '{}' @ {:.2}px: {:?}",
                        req.family_key,
                        logical_size,
                        err
                    );
                    self.pending_raster_warm_dedupe.remove(&dedupe_key);
                }
            }
        }

        self.runtime_raster_warm_budget = remaining_budget;
        std::mem::swap(&mut self.pending_raster_warm, &mut deferred);
        self.pending_raster_warm_deferred = deferred;
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
            log::warn!(
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
    pub(crate) fn build_tiny_raster_fallback_from_font_data(
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
        )?;
        Ok(TinyRasterFallbackSpec {
            atlas,
            char_map,
            font_size: tiny_size,
            padding: padding as f32 / dpi_scale_factor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn build_tiny_raster_fallback_from_font_data(
        &mut self,
        _font_data: &[u8],
        _dpi_scale_factor: f32,
    ) -> Result<TinyRasterFallbackSpec, FontError> {
        Err(FontError::FreeTypeError(
            "tiny raster fallback generation is not available on wasm32".to_string(),
        ))
    }

    // ── wasm-only raster-texture public API ───────────────────────────────

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
}
