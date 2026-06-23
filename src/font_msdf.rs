//! MSDF font loading — pre-baked atlas variants and runtime TTF-to-MSDF conversion.
//!
//! All items in this module are either `pub` (matching the public API surface in lib.rs) or
//! `pub(crate)` (cross-called from lib.rs but not part of the public API surface).

use crate::text::{
    Bounds, CharacterInfo, FontError, MsdfAtlasInfo, MsdfFontMetadata, MsdfGlyphRecord,
    MsdfKerningRecord, MsdfMetrics, TextRenderer,
};
use crate::utils::Size;
use crate::PlutoniumEngine;
use rusttype::{Font, Scale};
use std::collections::HashMap;
use uuid::Uuid;

impl<'a> PlutoniumEngine<'a> {
    /// Configure hybrid text thresholds in logical pixels.
    ///
    /// - `tiny_raster_max_px`: use hinted tiny-raster path at or below this size.
    /// - `msdf_min_px`: prefer MSDF at or above this size.
    pub fn set_msdf_switch_thresholds(&mut self, tiny_raster_max_px: f32, msdf_min_px: f32) {
        self.text_renderer
            .set_quality_thresholds(tiny_raster_max_px, msdf_min_px);
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
}
