use crate::pluto_objects::{
    text2d::{HorizontalAlignment, TextContainer, VerticalAlignment},
    texture_atlas_2d::TextureAtlas2D,
};
use crate::utils::{Position, Size};
use rusttype::{point, Font, GlyphId, OutlineBuilder, Scale};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const DEFAULT_MSDF_PX_RANGE: f32 = 8.0;
pub const DEFAULT_TINY_RASTER_MAX_PX: f32 = 15.0;
pub const DEFAULT_MSDF_MIN_PX: f32 = 18.0;

// Character information for the font atlas
#[derive(Clone, Debug)]
pub struct CharacterInfo {
    pub tile_index: usize,
    pub advance_width: f32,
    pub bearing: (f32, f32),
    pub size: (u32, u32),
}

#[derive(Clone, Debug)]
pub struct MsdfGlyphInfo {
    pub advance_width: f32,   // em units
    pub plane_bounds: Bounds, // em units, y-up (top positive, bottom negative)
    pub uv_offset: [f32; 2],  // normalized
    pub uv_scale: [f32; 2],   // normalized
    pub padding_em: f32,      // atlas padding in em units (padding_px / generation_font_size)
}

#[derive(Clone, Debug)]
pub enum GlyphRenderMode {
    AtlasTile {
        tile_index: usize,
        scale: f32,
    },
    AtlasUv {
        uv_offset: [f32; 2],
        uv_scale: [f32; 2],
        is_msdf: bool,
        msdf_px_range: f32,
    },
}

pub struct CharacterRenderInfo {
    pub atlas_id: Uuid,
    pub position: Position,
    pub size: Size,
    pub mode: GlyphRenderMode,
}

#[derive(Clone, Debug)]
pub struct GlyphLayoutDebugRecord {
    pub index: usize,
    pub input_char: char,
    pub resolved_char: char,
    pub mode: &'static str,
    pub pen_x_before: f32,
    pub kerning_px: f32,
    pub glyph_left_px: f32,
    pub glyph_right_px: f32,
    pub advance_px: f32,
    pub letter_spacing_px: f32,
    pub pen_x_after: f32,
}

#[derive(Debug)]
pub enum FontError {
    IoError(std::io::Error),
    InvalidFontData,
    AtlasRenderError,
    MetadataParseError(String),
    UnsupportedAtlasFormat(String),
    MissingGlyphData(char),
    ImageDecodeError(String),
    FreeTypeError(String),
}

#[derive(Clone, Copy, Debug, Default)]
struct MsdfVec2 {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug)]
struct MsdfEdge {
    a: MsdfVec2,
    b: MsdfVec2,
    channel: usize, // 0=R,1=G,2=B
}

#[derive(Default)]
struct MsdfOutline {
    contours: Vec<Vec<MsdfVec2>>,
    current: Vec<MsdfVec2>,
    current_point: Option<MsdfVec2>,
}

impl MsdfOutline {
    fn finish(mut self) -> Vec<Vec<MsdfVec2>> {
        if !self.current.is_empty() {
            self.contours.push(std::mem::take(&mut self.current));
        }
        self.contours
    }
}

impl OutlineBuilder for MsdfOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        if !self.current.is_empty() {
            self.contours.push(std::mem::take(&mut self.current));
        }
        let p = MsdfVec2 { x, y };
        self.current.push(p);
        self.current_point = Some(p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = MsdfVec2 { x, y };
        self.current.push(p);
        self.current_point = Some(p);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let Some(p0) = self.current_point else {
            self.move_to(x, y);
            return;
        };
        let p1 = MsdfVec2 { x: x1, y: y1 };
        let p2 = MsdfVec2 { x, y };
        let mut points = Vec::new();
        flatten_quad(p0, p1, p2, 0.10, &mut points);
        for p in points {
            self.current.push(p);
            self.current_point = Some(p);
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let Some(p0) = self.current_point else {
            self.move_to(x, y);
            return;
        };
        let p1 = MsdfVec2 { x: x1, y: y1 };
        let p2 = MsdfVec2 { x: x2, y: y2 };
        let p3 = MsdfVec2 { x, y };
        let mut points = Vec::new();
        flatten_cubic(p0, p1, p2, p3, 0.10, &mut points);
        for p in points {
            self.current.push(p);
            self.current_point = Some(p);
        }
    }

    fn close(&mut self) {
        if !self.current.is_empty() {
            self.contours.push(std::mem::take(&mut self.current));
        }
        self.current_point = None;
    }
}

fn sqr(v: f32) -> f32 {
    v * v
}

fn vec_sub(a: MsdfVec2, b: MsdfVec2) -> MsdfVec2 {
    MsdfVec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn vec_len(v: MsdfVec2) -> f32 {
    (sqr(v.x) + sqr(v.y)).sqrt()
}

fn vec_dot(a: MsdfVec2, b: MsdfVec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn vec_normalize(v: MsdfVec2) -> MsdfVec2 {
    let l = vec_len(v).max(1e-6);
    MsdfVec2 {
        x: v.x / l,
        y: v.y / l,
    }
}

fn point_line_distance(p: MsdfVec2, a: MsdfVec2, b: MsdfVec2) -> f32 {
    let ab = vec_sub(b, a);
    let ap = vec_sub(p, a);
    let ab_len = vec_len(ab).max(1e-6);
    (ab.x * ap.y - ab.y * ap.x).abs() / ab_len
}

fn flatten_quad(p0: MsdfVec2, p1: MsdfVec2, p2: MsdfVec2, tolerance: f32, out: &mut Vec<MsdfVec2>) {
    if point_line_distance(p1, p0, p2) <= tolerance {
        out.push(p2);
        return;
    }
    let p01 = MsdfVec2 {
        x: (p0.x + p1.x) * 0.5,
        y: (p0.y + p1.y) * 0.5,
    };
    let p12 = MsdfVec2 {
        x: (p1.x + p2.x) * 0.5,
        y: (p1.y + p2.y) * 0.5,
    };
    let p012 = MsdfVec2 {
        x: (p01.x + p12.x) * 0.5,
        y: (p01.y + p12.y) * 0.5,
    };
    flatten_quad(p0, p01, p012, tolerance, out);
    flatten_quad(p012, p12, p2, tolerance, out);
}

fn flatten_cubic(
    p0: MsdfVec2,
    p1: MsdfVec2,
    p2: MsdfVec2,
    p3: MsdfVec2,
    tolerance: f32,
    out: &mut Vec<MsdfVec2>,
) {
    let d1 = point_line_distance(p1, p0, p3);
    let d2 = point_line_distance(p2, p0, p3);
    if d1.max(d2) <= tolerance {
        out.push(p3);
        return;
    }

    let p01 = MsdfVec2 {
        x: (p0.x + p1.x) * 0.5,
        y: (p0.y + p1.y) * 0.5,
    };
    let p12 = MsdfVec2 {
        x: (p1.x + p2.x) * 0.5,
        y: (p1.y + p2.y) * 0.5,
    };
    let p23 = MsdfVec2 {
        x: (p2.x + p3.x) * 0.5,
        y: (p2.y + p3.y) * 0.5,
    };
    let p012 = MsdfVec2 {
        x: (p01.x + p12.x) * 0.5,
        y: (p01.y + p12.y) * 0.5,
    };
    let p123 = MsdfVec2 {
        x: (p12.x + p23.x) * 0.5,
        y: (p12.y + p23.y) * 0.5,
    };
    let p0123 = MsdfVec2 {
        x: (p012.x + p123.x) * 0.5,
        y: (p012.y + p123.y) * 0.5,
    };

    flatten_cubic(p0, p01, p012, p0123, tolerance, out);
    flatten_cubic(p0123, p123, p23, p3, tolerance, out);
}

fn segment_distance(p: MsdfVec2, a: MsdfVec2, b: MsdfVec2) -> f32 {
    let ab = vec_sub(b, a);
    let ap = vec_sub(p, a);
    let ab_len2 = sqr(ab.x) + sqr(ab.y);
    if ab_len2 <= 1e-8 {
        return vec_len(ap);
    }
    let t = ((ap.x * ab.x) + (ap.y * ab.y)) / ab_len2;
    let t = t.clamp(0.0, 1.0);
    let q = MsdfVec2 {
        x: a.x + ab.x * t,
        y: a.y + ab.y * t,
    };
    vec_len(vec_sub(p, q))
}

fn contour_edges(contours: &[Vec<MsdfVec2>]) -> Vec<MsdfEdge> {
    const CORNER_COS_THRESHOLD: f32 = 0.75; // ~41 degrees

    let mut edges = Vec::new();
    for contour in contours {
        let n = contour.len();
        if n < 2 {
            continue;
        }

        let mut segments = Vec::new();
        for i in 0..n {
            let a = contour[i];
            let b = contour[(i + 1) % n];
            if vec_len(vec_sub(b, a)) <= 1e-6 {
                continue;
            }
            segments.push((i, a, b));
        }
        if segments.is_empty() {
            continue;
        }

        // Mark sharp corners to keep a stable channel per side between corners.
        let mut is_corner = vec![false; n];
        for i in 0..n {
            let p_prev = contour[(i + n - 1) % n];
            let p = contour[i];
            let p_next = contour[(i + 1) % n];
            let v0 = vec_sub(p, p_prev);
            let v1 = vec_sub(p_next, p);
            if vec_len(v0) <= 1e-6 || vec_len(v1) <= 1e-6 {
                continue;
            }
            let d0 = vec_normalize(v0);
            let d1 = vec_normalize(v1);
            let dot = vec_dot(d0, d1).clamp(-1.0, 1.0);
            if dot < CORNER_COS_THRESHOLD {
                is_corner[i] = true;
            }
        }

        let start_vertex = is_corner
            .iter()
            .position(|&flag| flag)
            .unwrap_or(segments[0].0);
        let start_segment = segments
            .iter()
            .position(|(vertex_idx, _, _)| *vertex_idx == start_vertex)
            .unwrap_or(0);

        let mut channel = 0usize;
        for step in 0..segments.len() {
            let idx = (start_segment + step) % segments.len();
            let (vertex_idx, a, b) = segments[idx];
            edges.push(MsdfEdge { a, b, channel });

            let end_vertex = (vertex_idx + 1) % n;
            if is_corner[end_vertex] {
                channel = (channel + 1) % 3;
            }
        }
    }
    edges
}

fn point_inside_winding(p: MsdfVec2, contours: &[Vec<MsdfVec2>]) -> bool {
    let mut winding = 0i32;
    for contour in contours {
        if contour.len() < 2 {
            continue;
        }
        for i in 0..contour.len() {
            let a = contour[i];
            let b = contour[(i + 1) % contour.len()];
            let crosses = (a.y <= p.y && b.y > p.y) || (a.y > p.y && b.y <= p.y);
            if !crosses {
                continue;
            }
            let dy = b.y - a.y;
            if dy.abs() <= 1e-8 {
                continue;
            }
            let x_intersect = a.x + (p.y - a.y) * (b.x - a.x) / dy;
            if x_intersect > p.x {
                winding += if b.y > a.y { 1 } else { -1 };
            }
        }
    }
    winding != 0
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct Bounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MsdfAtlasInfo {
    pub width: f32,
    pub height: f32,
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MsdfMetrics {
    pub font_size: f32,
    pub ascender: f32,
    pub descender: f32,
    pub line_height: f32,
    #[serde(default)]
    pub padding_em: f32, // atlas tile padding in em units
    #[serde(default = "default_msdf_px_range")]
    pub px_range: f32,
}

fn default_msdf_px_range() -> f32 {
    DEFAULT_MSDF_PX_RANGE
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MsdfGlyphRecord {
    pub unicode: u32,
    pub advance: f32,
    pub plane_bounds: Bounds,
    pub atlas_bounds: Bounds,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MsdfKerningRecord {
    pub left_unicode: u32,
    pub right_unicode: u32,
    pub advance: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MsdfFontMetadata {
    pub atlas: MsdfAtlasInfo,
    pub metrics: MsdfMetrics,
    pub glyphs: Vec<MsdfGlyphRecord>,
    #[serde(default, alias = "kernings")]
    pub kerning: Vec<MsdfKerningRecord>,
}

struct RasterAtlasData {
    // logical pixels (after dividing by DPI)
    padding: f32,
    // logical->physical pixel snap scale (typically window DPI scale factor)
    pixel_snap_scale: f32,
}

struct MsdfAtlasData {
    glyphs: HashMap<char, MsdfGlyphInfo>,
    kernings: HashMap<(char, char), f32>, // em units
    line_height: f32,                     // em units
    px_range: f32,
    tiny_raster: Option<TinyRasterFallback>,
}

enum FontAtlasData {
    Raster(RasterAtlasData),
    Msdf(MsdfAtlasData),
}

pub struct FontAtlas {
    atlas: TextureAtlas2D,
    char_map: HashMap<char, CharacterInfo>,
    font_size: f32,
    max_tile_size: Size,
    ascent: f32,
    descent: f32,
    data: FontAtlasData,
}

pub struct TinyRasterFallbackSpec {
    pub atlas: TextureAtlas2D,
    pub char_map: HashMap<char, CharacterInfo>,
    pub font_size: f32,
    pub ascent: f32,
    pub descent: f32,
    pub padding: f32,
}

struct TinyRasterFallback {
    atlas: TextureAtlas2D,
    char_map: HashMap<char, CharacterInfo>,
    font_size: f32,
    padding: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct TextQualitySettings {
    pub tiny_raster_max_px: f32,
    pub msdf_min_px: f32,
}

impl Default for TextQualitySettings {
    fn default() -> Self {
        Self {
            tiny_raster_max_px: DEFAULT_TINY_RASTER_MAX_PX,
            msdf_min_px: DEFAULT_MSDF_MIN_PX,
        }
    }
}

impl FontAtlas {
    // for debugging
    pub fn get_tile_dimensions(&self) -> Size {
        self.max_tile_size
    }

    pub fn get_char_info(&self, c: char) -> Option<&CharacterInfo> {
        self.char_map.get(&c)
    }

    pub fn atlas_id(&self) -> Uuid {
        self.atlas.get_id()
    }

    pub fn is_msdf(&self) -> bool {
        matches!(self.data, FontAtlasData::Msdf(_))
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    pub fn debug_save_atlas(&self) -> Result<(), std::io::Error> {
        match &self.data {
            FontAtlasData::Raster(_) => {
                for (c, info) in &self.char_map {
                    println!(
                        "Char '{}' (ASCII: {}) - Tile Index: {}, Size: {:?}, Bearing: {:?}, Advance: {}",
                        c, *c as u32, info.tile_index, info.size, info.bearing, info.advance_width
                    );
                }
            }
            FontAtlasData::Msdf(msdf) => {
                for (c, info) in &msdf.glyphs {
                    println!(
                        "MSDF '{}' (ASCII: {}) - advance: {}, plane: ({:.3}, {:.3}, {:.3}, {:.3}), uv: ({:.3}, {:.3}) scale: ({:.3}, {:.3})",
                        c,
                        *c as u32,
                        info.advance_width,
                        info.plane_bounds.left,
                        info.plane_bounds.top,
                        info.plane_bounds.right,
                        info.plane_bounds.bottom,
                        info.uv_offset[0],
                        info.uv_offset[1],
                        info.uv_scale[0],
                        info.uv_scale[1]
                    );
                }
            }
        }

        Ok(())
    }
}
#[derive(Default)]
pub struct TextRenderer {
    pub(crate) font_atlases: HashMap<String, FontAtlas>,
    pub(crate) fonts: HashMap<String, Font<'static>>,
    pub(crate) quality: TextQualitySettings,
}

impl TextRenderer {
    fn snap_to_pixel(value: f32, snap_scale: f32) -> f32 {
        let s = snap_scale.max(1.0);
        ((value * s).round()) / s
    }

    pub fn new() -> Self {
        Self {
            font_atlases: HashMap::new(),
            fonts: HashMap::new(),
            quality: TextQualitySettings::default(),
        }
    }

    pub fn set_quality_thresholds(&mut self, tiny_raster_max_px: f32, msdf_min_px: f32) {
        // Keep thresholds ordered and sane so mode switches remain predictable.
        let tiny = tiny_raster_max_px.max(1.0);
        let msdf = msdf_min_px.max(tiny);
        self.quality = TextQualitySettings {
            tiny_raster_max_px: tiny,
            msdf_min_px: msdf,
        };
    }

    pub fn parse_msdf_metadata(json: &str) -> Result<MsdfFontMetadata, FontError> {
        serde_json::from_str::<MsdfFontMetadata>(json)
            .map_err(|e| FontError::MetadataParseError(e.to_string()))
    }

    pub fn calculate_atlas_size(
        font: &Font,
        scale: Scale,
        padding: u32,
    ) -> (u32, u32, HashMap<char, (u32, u32)>, u32, u32) {
        let glyphs: Vec<char> = (32..=126).map(|c| c as u8 as char).collect();
        Self::calculate_atlas_size_for_chars(font, scale, padding, &glyphs)
    }

    pub fn calculate_atlas_size_for_chars(
        font: &Font,
        scale: Scale,
        padding: u32,
        glyphs: &[char],
    ) -> (u32, u32, HashMap<char, (u32, u32)>, u32, u32) {
        let mut max_width = 0;
        let mut max_height = 0;
        let mut char_dimensions = HashMap::new();

        for c in glyphs.iter().copied() {
            let glyph = font.glyph(c).scaled(scale).positioned(point(0.0, 0.0));

            if let Some(bb) = glyph.pixel_bounding_box() {
                let width = (bb.max.x - bb.min.x) as u32 + padding * 2;
                let height = (bb.max.y - bb.min.y) as u32 + padding * 2;

                max_width = max_width.max(width);
                max_height = max_height.max(height);
                char_dimensions.insert(c, (width, height));
            }
        }

        // Keep atlas grid valid even if every glyph has an empty bitmap bbox.
        if max_width == 0 {
            max_width = padding.saturating_mul(2).max(1);
        }
        if max_height == 0 {
            max_height = padding.saturating_mul(2).max(1);
        }

        // Add extra padding to ensure we have enough space
        let chars_count = glyphs.len().max(1) as u32;
        let min_width = max_width * 8; // Assume at least 8 characters per row
        let min_height = max_height * ((chars_count / 8) + 1); // Calculate required rows

        // Calculate atlas dimensions ensuring power of 2 and minimum size
        let total_width = min_width.next_power_of_two();
        let total_height = min_height.next_power_of_two();

        (
            total_width,
            total_height,
            char_dimensions,
            max_width,
            max_height,
        )
    }

    fn resolve_msdf_char(msdf: &MsdfAtlasData, c: char) -> Option<char> {
        if msdf.glyphs.contains_key(&c) {
            Some(c)
        } else if msdf.glyphs.contains_key(&'?') {
            Some('?')
        } else {
            None
        }
    }

    fn msdf_kerning(msdf: &MsdfAtlasData, left: Option<char>, right: char) -> f32 {
        if let Some(left_char) = left {
            msdf.kernings
                .get(&(left_char, right))
                .copied()
                .unwrap_or(0.0)
        } else {
            0.0
        }
    }

    pub fn build_ascii_shaping_metrics(
        font_data: &[u8],
    ) -> Option<(HashMap<char, f32>, Vec<MsdfKerningRecord>)> {
        let face = rustybuzz::Face::from_slice(font_data, 0)?;
        let units_per_em = face.units_per_em() as f32;
        if units_per_em <= 0.0 {
            return None;
        }

        let mut single_advances = HashMap::new();
        for code in 32u8..=126u8 {
            let ch = code as char;
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(&ch.to_string());
            let shaped = rustybuzz::shape(&face, &[], buffer);
            if shaped.glyph_positions().is_empty() {
                continue;
            }
            let advance_units: i32 = shaped.glyph_positions().iter().map(|p| p.x_advance).sum();
            single_advances.insert(ch, advance_units as f32 / units_per_em);
        }

        if single_advances.is_empty() {
            return None;
        }

        let mut kerning = Vec::new();
        // Ignore tiny noise below half a font unit.
        let threshold = 0.5 / units_per_em;

        for left_u in 32u8..=126u8 {
            let left = left_u as char;
            let Some(left_advance) = single_advances.get(&left).copied() else {
                continue;
            };
            for right_u in 32u8..=126u8 {
                let right = right_u as char;
                if !single_advances.contains_key(&right) {
                    continue;
                }

                let mut pair_buffer = rustybuzz::UnicodeBuffer::new();
                pair_buffer.push_str(&format!("{}{}", left, right));
                let shaped_pair = rustybuzz::shape(&face, &[], pair_buffer);

                // We don't support substitutions/ligatures in this renderer yet,
                // so only keep pair adjustments where shaping preserves two glyphs.
                if shaped_pair.glyph_infos().len() != 2 {
                    continue;
                }

                let pair_positions = shaped_pair.glyph_positions();
                let pair_total_advance: f32 = pair_positions
                    .iter()
                    .map(|p| p.x_advance as f32 / units_per_em)
                    .sum();
                let right_advance = single_advances.get(&right).copied().unwrap_or(0.0);
                // Our renderer only supports pen-based pair adjustment (no explicit glyph x_offset),
                // so use total pair width delta. This prevents offset-compensated pairs
                // (e.g. positive x_offset with reduced right advance) from over-spacing.
                let adjustment = pair_total_advance - (left_advance + right_advance);
                if adjustment.abs() > threshold {
                    kerning.push(MsdfKerningRecord {
                        left_unicode: left as u32,
                        right_unicode: right as u32,
                        advance: adjustment,
                    });
                }
            }
        }

        Some((single_advances, kerning))
    }

    pub fn build_ascii_kerning_from_font_data(font_data: &[u8]) -> Option<Vec<MsdfKerningRecord>> {
        Self::build_ascii_shaping_metrics(font_data).map(|(_, k)| k)
    }

    /// Compute correction factor for rusttype's em-unit normalisation.
    ///
    /// rusttype divides glyph metrics by `(hhea ascent − descent)` instead of
    /// `units_per_em`.  When these differ the plane-bounds, padding, and
    /// vertical metrics will be in a different unit system than the shaping
    /// advances (which use the correct `units_per_em`).
    ///
    /// Returns the ratio `height / upem` so that
    /// `rusttype_value / gen_size * correction` gives true em-unit values.
    /// Falls back to `1.0` when no shaping data is available.
    pub fn compute_rusttype_em_correction(
        shaping_advances: &Option<HashMap<char, f32>>,
        char_map: &HashMap<char, CharacterInfo>,
        gen_size: f32,
    ) -> f32 {
        let Some(advs) = shaping_advances else {
            return 1.0;
        };
        // Compare a rusttype advance (in rusttype-em) with the true advance
        // (in standard em) to derive the correction ratio.
        for code in 33u8..=126u8 {
            let ch = code as char;
            if let (Some(&true_adv), Some(info)) = (advs.get(&ch), char_map.get(&ch)) {
                let rt_adv = info.advance_width / gen_size;
                if rt_adv > 0.01 {
                    return true_adv / rt_adv;
                }
            }
        }
        1.0
    }

    /// Convert rusttype's baseline-relative bounds (Y-down) into y-up em-space bounds.
    /// Used by both runtime and offline MSDF metadata generation to keep metrics identical.
    pub fn msdf_plane_bounds_from_exact_bounds(
        exact_bounds: rusttype::Rect<f32>,
        denom: f32,
        em_correction: f32,
    ) -> Bounds {
        let units = denom.max(1.0);
        Bounds {
            left: exact_bounds.min.x / units * em_correction,
            top: -exact_bounds.min.y / units * em_correction,
            right: exact_bounds.max.x / units * em_correction,
            bottom: -exact_bounds.max.y / units * em_correction,
        }
    }

    /// Convert pixel bounding box (from positioned glyph rasterization) into
    /// y-up em-space bounds. This keeps plane geometry in the same coordinate
    /// space as atlas tile sampling and avoids per-glyph squeeze from mixed
    /// exact-bounds vs pixel-bounds units.
    pub fn msdf_plane_bounds_from_pixel_bounds(
        pixel_bounds: rusttype::Rect<i32>,
        bearing_y: f32,
        denom: f32,
        em_correction: f32,
    ) -> Bounds {
        let units = denom.max(1.0);
        Bounds {
            left: pixel_bounds.min.x as f32 / units * em_correction,
            top: (bearing_y - pixel_bounds.min.y as f32) / units * em_correction,
            right: pixel_bounds.max.x as f32 / units * em_correction,
            bottom: (bearing_y - pixel_bounds.max.y as f32) / units * em_correction,
        }
    }

    fn should_use_tiny_raster(&self, msdf: &MsdfAtlasData, target_font_size: f32) -> bool {
        if msdf.tiny_raster.is_none() {
            return false;
        }
        if target_font_size <= self.quality.tiny_raster_max_px {
            return true;
        }
        if target_font_size >= self.quality.msdf_min_px {
            return false;
        }
        let midpoint = (self.quality.tiny_raster_max_px + self.quality.msdf_min_px) * 0.5;
        target_font_size <= midpoint
    }

    pub fn measure_text(
        &self,
        text: &str,
        font_key: &str,
        letter_spacing: f32,
        word_spacing: f32,
        font_size_override: Option<f32>,
    ) -> (f32, usize) {
        if let Some(font_atlas) = self.font_atlases.get(font_key) {
            match &font_atlas.data {
                FontAtlasData::Raster(_) => {
                    let mut max_width: f32 = 0.0;
                    let mut current_width = 0.0;
                    let mut line_count = 1;
                    let mut prev: Option<GlyphId> = None;
                    let font_opt = self.fonts.get(font_key);
                    let scale_ratio =
                        font_size_override.unwrap_or(font_atlas.font_size) / font_atlas.font_size;
                    let scale = Scale::uniform(font_atlas.font_size);

                    let space_width = (font_atlas
                        .get_char_info(' ')
                        .map(|i| i.advance_width)
                        .unwrap_or(font_atlas.font_size * 0.3))
                        * scale_ratio;

                    let chars: Vec<char> = text.chars().collect();
                    for (i, c) in chars.iter().copied().enumerate() {
                        if c == '\n' {
                            max_width = max_width.max(current_width);
                            current_width = 0.0;
                            line_count += 1;
                            prev = None;
                            continue;
                        }

                        if c == ' ' {
                            current_width += space_width + word_spacing;
                            prev = font_opt.map(|f| f.glyph(' ').id());
                            continue;
                        }

                        let use_char = if font_atlas.char_map.contains_key(&c) {
                            c
                        } else {
                            '?'
                        };
                        if let Some(info) = font_atlas.char_map.get(&use_char) {
                            if let (Some(f), Some(p)) = (font_opt, prev) {
                                current_width +=
                                    f.pair_kerning(scale, p, f.glyph(use_char).id()) * scale_ratio;
                            }
                            current_width += info.advance_width * scale_ratio;
                            if i + 1 < chars.len() && chars[i + 1] != '\n' {
                                current_width += letter_spacing.max(0.0);
                            }
                            prev = font_opt.map(|f| f.glyph(use_char).id());
                        }
                    }

                    max_width = max_width.max(current_width);
                    (max_width, line_count)
                }
                FontAtlasData::Msdf(msdf) => {
                    let mut max_width: f32 = 0.0;
                    let mut current_width: f32 = 0.0;
                    let mut line_count = 1;
                    let mut prev: Option<char> = None;
                    let target_font_size =
                        font_size_override.unwrap_or(font_atlas.font_size).max(1.0);
                    let using_tiny = self.should_use_tiny_raster(msdf, target_font_size);
                    let tiny_scale_ratio = if using_tiny {
                        msdf.tiny_raster
                            .as_ref()
                            .map(|tiny| target_font_size / tiny.font_size.max(1.0))
                            .unwrap_or(1.0)
                    } else {
                        1.0
                    };
                    let space_width = if using_tiny {
                        msdf.tiny_raster
                            .as_ref()
                            .and_then(|tiny| tiny.char_map.get(&' '))
                            .map(|g| g.advance_width * tiny_scale_ratio)
                            .unwrap_or(target_font_size * 0.3)
                    } else {
                        msdf.glyphs
                            .get(&' ')
                            .map(|g| g.advance_width * target_font_size)
                            .unwrap_or(target_font_size * 0.3)
                    };

                    let chars: Vec<char> = text.chars().collect();
                    for (i, c) in chars.iter().copied().enumerate() {
                        if c == '\n' {
                            max_width = max_width.max(current_width);
                            current_width = 0.0;
                            line_count += 1;
                            prev = None;
                            continue;
                        }

                        if c == ' ' {
                            current_width += space_width + word_spacing;
                            prev = Some(' ');
                            continue;
                        }

                        if using_tiny {
                            let use_char = if msdf
                                .tiny_raster
                                .as_ref()
                                .is_some_and(|tiny| tiny.char_map.contains_key(&c))
                            {
                                c
                            } else {
                                '?'
                            };
                            if let Some(tiny_info) = msdf
                                .tiny_raster
                                .as_ref()
                                .and_then(|tiny| tiny.char_map.get(&use_char))
                            {
                                current_width +=
                                    Self::msdf_kerning(msdf, prev, use_char) * target_font_size;
                                current_width += tiny_info.advance_width * tiny_scale_ratio;
                                if i + 1 < chars.len() && chars[i + 1] != '\n' {
                                    current_width += letter_spacing.max(0.0);
                                }
                                prev = Some(use_char);
                            }
                        } else if let Some(use_char) = Self::resolve_msdf_char(msdf, c) {
                            if let Some(glyph) = msdf.glyphs.get(&use_char) {
                                current_width +=
                                    Self::msdf_kerning(msdf, prev, use_char) * target_font_size;
                                current_width += glyph.advance_width * target_font_size;
                                if i + 1 < chars.len() && chars[i + 1] != '\n' {
                                    current_width += letter_spacing.max(0.0);
                                }
                                prev = Some(use_char);
                            }
                        }
                    }

                    max_width = max_width.max(current_width);
                    (max_width, line_count)
                }
            }
        } else {
            (0.0, 1)
        }
    }

    /// Measure caret advance for a single line prefix using the same pen math as layout.
    /// Input should not contain newlines; if present, measurement stops at the first newline.
    pub fn render_glyphs_to_atlas(
        font: &Font,
        scale: Scale,
        atlas_size: (u32, u32),
        char_dimensions: &HashMap<char, (u32, u32)>,
        padding: u32,
    ) -> Option<(Vec<u8>, HashMap<char, CharacterInfo>)> {
        let glyphs: Vec<char> = (32..=126).map(|c| c as u8 as char).collect();
        Self::render_glyphs_to_atlas_for_chars(
            font,
            scale,
            atlas_size,
            char_dimensions,
            padding,
            &glyphs,
        )
    }

    pub fn render_glyphs_to_atlas_for_chars(
        font: &Font,
        scale: Scale,
        atlas_size: (u32, u32),
        char_dimensions: &HashMap<char, (u32, u32)>,
        padding: u32,
        glyphs: &[char],
    ) -> Option<(Vec<u8>, HashMap<char, CharacterInfo>)> {
        let (atlas_width, atlas_height) = atlas_size;
        println!("[FONT DEBUG] Atlas size: {}x{}", atlas_width, atlas_height);
        println!(
            "[FONT DEBUG] Char dimensions count: {}",
            char_dimensions.len()
        );
        println!("[FONT DEBUG] Scale: {:?}", scale);
        let mut texture_data = vec![0; (atlas_width * atlas_height * 4) as usize];
        let mut char_map = HashMap::new();
        // Use the pre-padded glyph cell size computed in calculate_atlas_size.
        // Do NOT add padding again here, or the packing grid and tile UVs will diverge.
        let max_width = (*char_dimensions.values().map(|(w, _)| w).max().unwrap_or(&0))
            .max(padding.saturating_mul(2))
            .max(1);
        let max_height = (*char_dimensions.values().map(|(_, h)| h).max().unwrap_or(&0))
            .max(padding.saturating_mul(2))
            .max(1);
        println!("[FONT DEBUG] Max tile size: {}x{}", max_width, max_height);
        // Start packing tiles at (0,0) on a strict grid so tile indices map
        // exactly to grid cells used by tile_uv_coordinates.
        let mut current_x = 0;
        let mut current_y = 0;
        let mut glyphs_drawn = 0;
        for (next_tile_index, c) in glyphs.iter().copied().enumerate() {
            let base_glyph = font.glyph(c).scaled(scale);
            let probe = base_glyph.clone().positioned(point(0.0, 0.0));
            // Use pixel-bounds bearings to keep per-glyph baseline placement
            // consistent with how glyph bitmaps are rasterized into tiles.
            let bearing_y = probe
                .pixel_bounding_box()
                .map(|bb| (-bb.min.y) as f32)
                .or_else(|| base_glyph.exact_bounding_box().map(|bb| -bb.min.y))
                .unwrap_or(0.0);
            let bearing_x = probe
                .pixel_bounding_box()
                .map(|bb| bb.min.x as f32)
                .unwrap_or_else(|| base_glyph.h_metrics().left_side_bearing);

            // Always allocate a cell per codepoint to keep tile_index deterministic
            if current_x + max_width > atlas_width {
                current_x = 0;
                current_y += max_height;
            }

            let (width, height) = char_dimensions
                .get(&c)
                .copied()
                .unwrap_or((padding * 2, padding * 2));

            let glyph = base_glyph.clone().positioned(point(0.0, bearing_y));
            let glyph_x = current_x + padding;
            let glyph_y = current_y + padding;

            if let Some(bb) = glyph.pixel_bounding_box() {
                if next_tile_index < 5 {
                    println!(
                        "[FONT DEBUG] Char '{}': bbox={:?}, pos=({}, {})",
                        c, bb, glyph_x, glyph_y
                    );
                }
                glyph.draw(|x, y, v| {
                    let px = glyph_x + x;
                    let py = glyph_y + y;
                    if px < atlas_width && py < atlas_height {
                        let index = ((py * atlas_width + px) * 4) as usize;
                        let alpha = (v * 255.0) as u8;
                        texture_data[index] = 255; // R
                        texture_data[index + 1] = 255; // G
                        texture_data[index + 2] = 255; // B
                        texture_data[index + 3] = alpha; // A
                        if v > 0.0 {
                            glyphs_drawn += 1;
                        }
                    }
                });
            } else {
                if next_tile_index < 5 {
                    println!("[FONT DEBUG] Char '{}': NO BOUNDING BOX", c);
                }
            }

            let h_metrics = base_glyph.clone().h_metrics();
            char_map.insert(
                c,
                CharacterInfo {
                    tile_index: next_tile_index,
                    advance_width: h_metrics.advance_width,
                    bearing: (bearing_x, bearing_y),
                    size: (width, height),
                },
            );
            current_x += max_width;
        }

        println!("[FONT DEBUG] Total glyphs drawn: {}", glyphs_drawn);
        println!("[FONT DEBUG] Total characters in map: {}", char_map.len());

        // Check if any pixels were written
        let non_zero_pixels = texture_data.iter().filter(|&&b| b != 0).count();
        println!(
            "[FONT DEBUG] Non-zero bytes in texture: {}/{}",
            non_zero_pixels,
            texture_data.len()
        );

        Some((texture_data, char_map))
    }

    pub fn render_msdf_glyphs_to_atlas(
        font: &Font,
        scale: Scale,
        atlas_size: (u32, u32),
        char_dimensions: &HashMap<char, (u32, u32)>,
        padding: u32,
        px_range: f32,
    ) -> Option<(Vec<u8>, HashMap<char, CharacterInfo>)> {
        let (atlas_width, atlas_height) = atlas_size;
        let mut texture_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];
        let mut char_map = HashMap::new();

        let max_width = *char_dimensions.values().map(|(w, _)| w).max().unwrap_or(&0);
        let max_height = *char_dimensions.values().map(|(_, h)| h).max().unwrap_or(&0);
        let mut current_x = 0u32;
        let mut current_y = 0u32;
        let range = px_range.max(1.0);

        for (next_tile_index, c) in (32..=126).map(|code| code as u8 as char).enumerate() {
            let base_glyph = font.glyph(c).scaled(scale);
            let bearing_y = base_glyph
                .exact_bounding_box()
                .map(|bb| -bb.min.y)
                .unwrap_or(0.0);

            if current_x + max_width > atlas_width {
                current_x = 0;
                current_y += max_height;
            }
            if current_y + max_height > atlas_height {
                return None;
            }

            let (width, height) = char_dimensions
                .get(&c)
                .copied()
                .unwrap_or((padding * 2, padding * 2));
            let glyph = base_glyph.clone().positioned(point(0.0, bearing_y));
            // Anchor atlas sampling to positioned pixel bounds so SDF texels,
            // metadata plane-bounds, and runtime quad placement share the same
            // coordinate space.
            let glyph_bb = glyph
                .pixel_bounding_box()
                .map(|bb| (bb.min.x as f32, bb.min.y as f32));

            let mut builder = MsdfOutline::default();
            glyph.build_outline(&mut builder);
            let contours = builder.finish();
            let edges = contour_edges(&contours);

            for y in 0..height {
                let py = current_y + y;
                if py >= atlas_height {
                    continue;
                }
                for x in 0..width {
                    let px = current_x + x;
                    if px >= atlas_width {
                        continue;
                    }
                    // Sample in the glyph's own outline coordinate space.
                    // The tile stores the glyph's bounding box at +padding inset.
                    let local_x = x as f32 + 0.5 - padding as f32;
                    let local_y = y as f32 + 0.5 - padding as f32;
                    let p = if let Some((bb_min_x, bb_min_y)) = glyph_bb {
                        MsdfVec2 {
                            x: bb_min_x + local_x,
                            // rusttype outline coordinates use Y-down, matching atlas row order.
                            y: bb_min_y + local_y,
                        }
                    } else {
                        MsdfVec2 {
                            x: local_x,
                            y: local_y,
                        }
                    };
                    let idx = ((py * atlas_width + px) * 4) as usize;

                    if edges.is_empty() {
                        texture_data[idx] = 0;
                        texture_data[idx + 1] = 0;
                        texture_data[idx + 2] = 0;
                        texture_data[idx + 3] = 255;
                        continue;
                    }

                    let inside = point_inside_winding(p, &contours);
                    let mut channel_dist = [f32::MAX; 3];
                    let mut min_dist = f32::MAX;
                    for edge in &edges {
                        let d = segment_distance(p, edge.a, edge.b);
                        if d < min_dist {
                            min_dist = d;
                        }
                        if d < channel_dist[edge.channel] {
                            channel_dist[edge.channel] = d;
                        }
                    }

                    for (ch_idx, out_idx) in [idx, idx + 1, idx + 2].into_iter().enumerate() {
                        let unsigned = if channel_dist[ch_idx].is_finite() {
                            channel_dist[ch_idx]
                        } else {
                            range
                        };
                        let signed = if inside { unsigned } else { -unsigned };
                        let encoded = (0.5 + signed / range).clamp(0.0, 1.0);
                        texture_data[out_idx] = (encoded * 255.0 + 0.5) as u8;
                    }
                    let sdf_unsigned = if min_dist.is_finite() {
                        min_dist
                    } else {
                        range
                    };
                    let sdf_signed = if inside { sdf_unsigned } else { -sdf_unsigned };
                    let sdf_encoded = (0.5 + sdf_signed / range).clamp(0.0, 1.0);
                    texture_data[idx + 3] = (sdf_encoded * 255.0 + 0.5) as u8;
                }
            }

            let h_metrics = base_glyph.clone().h_metrics();
            char_map.insert(
                c,
                CharacterInfo {
                    tile_index: next_tile_index,
                    advance_width: h_metrics.advance_width,
                    bearing: (h_metrics.left_side_bearing, bearing_y),
                    size: (width, height),
                },
            );

            current_x += max_width;
        }

        Some((texture_data, char_map))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn clear_raster_atlases(&mut self) -> Vec<Uuid> {
        let mut removed_ids = Vec::new();
        self.font_atlases.retain(|_, atlas| {
            if let FontAtlasData::Raster(_) = &atlas.data {
                removed_ids.push(atlas.atlas_id());
                false
            } else {
                true
            }
        });
        removed_ids
    }

    pub fn store_font_atlas(
        &mut self,
        font_key: &str,
        atlas: TextureAtlas2D,
        char_map: HashMap<char, CharacterInfo>,
        font_size: f32,
        ascent: f32,
        descent: f32,
        physical_tile_size: Size,
        dpi_scale_factor: f32,
        padding_pixels: u32,
    ) {
        let logical_tile_size = Size {
            width: physical_tile_size.width / dpi_scale_factor,
            height: physical_tile_size.height / dpi_scale_factor,
        };
        let logical_padding = (padding_pixels as f32) / dpi_scale_factor;

        let font_atlas = FontAtlas {
            atlas,
            char_map,
            font_size,
            max_tile_size: logical_tile_size, // Store in logical units
            ascent,
            descent,
            data: FontAtlasData::Raster(RasterAtlasData {
                padding: logical_padding,
                pixel_snap_scale: dpi_scale_factor.max(1.0),
            }),
        };
        self.font_atlases.insert(font_key.to_string(), font_atlas);
    }

    fn build_msdf_data(
        metadata: &MsdfFontMetadata,
    ) -> Result<(HashMap<char, MsdfGlyphInfo>, HashMap<(char, char), f32>), FontError> {
        if !metadata.atlas.kind.eq_ignore_ascii_case("msdf") {
            return Err(FontError::UnsupportedAtlasFormat(
                metadata.atlas.kind.clone(),
            ));
        }
        if metadata.atlas.width <= 0.0 || metadata.atlas.height <= 0.0 {
            return Err(FontError::MetadataParseError(
                "atlas width/height must be > 0".to_string(),
            ));
        }

        let mut glyphs = HashMap::new();
        for glyph in &metadata.glyphs {
            let ch = match char::from_u32(glyph.unicode) {
                Some(ch) => ch,
                None => continue,
            };
            // Phase 1 coverage: printable ASCII only.
            if !(32..=126).contains(&(ch as u32)) {
                continue;
            }

            let uv_left = glyph.atlas_bounds.left / metadata.atlas.width;
            let uv_top = glyph.atlas_bounds.top / metadata.atlas.height;
            let uv_width =
                (glyph.atlas_bounds.right - glyph.atlas_bounds.left) / metadata.atlas.width;
            let uv_height =
                (glyph.atlas_bounds.bottom - glyph.atlas_bounds.top) / metadata.atlas.height;
            if uv_width <= 0.0 || uv_height <= 0.0 {
                continue;
            }

            glyphs.insert(
                ch,
                MsdfGlyphInfo {
                    advance_width: glyph.advance,
                    plane_bounds: glyph.plane_bounds,
                    uv_offset: [uv_left, uv_top],
                    uv_scale: [uv_width, uv_height],
                    padding_em: metadata.metrics.padding_em,
                },
            );
        }

        if !glyphs.contains_key(&'?') {
            return Err(FontError::MissingGlyphData('?'));
        }

        let mut kernings = HashMap::new();
        for kerning in &metadata.kerning {
            let left = match char::from_u32(kerning.left_unicode) {
                Some(ch) => ch,
                None => continue,
            };
            let right = match char::from_u32(kerning.right_unicode) {
                Some(ch) => ch,
                None => continue,
            };
            if glyphs.contains_key(&left) && glyphs.contains_key(&right) {
                kernings.insert((left, right), kerning.advance);
            }
        }

        Ok((glyphs, kernings))
    }

    pub fn store_msdf_font_atlas(
        &mut self,
        font_key: &str,
        atlas: TextureAtlas2D,
        metadata: MsdfFontMetadata,
        tiny_raster: Option<TinyRasterFallbackSpec>,
    ) -> Result<(), FontError> {
        let (glyphs, kernings) = Self::build_msdf_data(&metadata)?;

        let max_w_em = glyphs
            .values()
            .map(|g| (g.plane_bounds.right - g.plane_bounds.left).max(0.0))
            .fold(0.0, f32::max);
        let max_h_em = glyphs
            .values()
            .map(|g| (g.plane_bounds.top - g.plane_bounds.bottom).abs())
            .fold(0.0, f32::max);
        let logical_font_size = metadata.metrics.font_size.max(1.0);

        let font_atlas = FontAtlas {
            atlas,
            char_map: HashMap::new(),
            font_size: logical_font_size,
            max_tile_size: Size {
                width: max_w_em * logical_font_size,
                height: max_h_em * logical_font_size,
            },
            ascent: metadata.metrics.ascender,
            descent: metadata.metrics.descender,
            data: FontAtlasData::Msdf(MsdfAtlasData {
                glyphs,
                kernings,
                line_height: metadata.metrics.line_height.max(0.001),
                px_range: metadata.metrics.px_range.max(1.0),
                tiny_raster: tiny_raster.map(|tiny| TinyRasterFallback {
                    atlas: tiny.atlas,
                    char_map: tiny.char_map,
                    font_size: tiny.font_size.max(1.0),
                    padding: tiny.padding.max(0.0),
                }),
            }),
        };
        self.font_atlases.insert(font_key.to_string(), font_atlas);
        Ok(())
    }

    pub fn calculate_text_layout(
        &self,
        text: &str,
        font_key: &str,
        container_pos: Position,
        container: &TextContainer,
        letter_spacing: f32,
        word_spacing: f32,
        font_size_override: Option<f32>,
    ) -> Vec<CharacterRenderInfo> {
        let mut chars_to_render = Vec::new();

        // Early return if text is empty
        if text.is_empty() {
            return chars_to_render;
        }

        let font_atlas = match self.font_atlases.get(font_key) {
            Some(atlas) => atlas,
            _ => {
                eprintln!("Font atlas not found: {}", font_key);
                return chars_to_render;
            }
        };

        let lines: Vec<&str> = text.split('\n').collect();
        let line_count = lines.len();

        match &font_atlas.data {
            FontAtlasData::Raster(raster) => {
                let scale_ratio =
                    font_size_override.unwrap_or(font_atlas.font_size) / font_atlas.font_size;

                let base_line_height = (font_atlas.ascent - font_atlas.descent).abs() * scale_ratio;
                let line_height = base_line_height * container.line_height_mul.max(0.8);
                let space_width = (font_atlas
                    .get_char_info(' ')
                    .map(|i| i.advance_width)
                    .unwrap_or(font_atlas.font_size * 0.3))
                    * scale_ratio;
                let scale = Scale::uniform(font_atlas.font_size);
                let font_opt = self.fonts.get(font_key);

                let line_widths: Vec<f32> = lines
                    .iter()
                    .map(|line| {
                        let chars: Vec<char> = line.chars().collect();
                        let mut width: f32 = 0.0;
                        let mut prev: Option<GlyphId> = None;
                        for (i, c) in chars.iter().copied().enumerate() {
                            if c == ' ' {
                                width += space_width + word_spacing;
                                prev = font_opt.map(|f| f.glyph(' ').id());
                            } else {
                                let use_char = if font_atlas.get_char_info(c).is_some() {
                                    c
                                } else {
                                    '?'
                                };
                                if let (Some(f), Some(p)) = (font_opt, prev) {
                                    width += f.pair_kerning(scale, p, f.glyph(use_char).id())
                                        * scale_ratio;
                                }
                                width += font_atlas
                                    .get_char_info(use_char)
                                    .map(|info| info.advance_width * scale_ratio)
                                    .unwrap_or(0.0);
                                if i + 1 < chars.len() {
                                    width += letter_spacing.max(0.0);
                                }
                                prev = font_opt.map(|f| f.glyph(use_char).id());
                            }
                        }
                        width
                    })
                    .collect();

                let total_text_height = line_height * (line_count as f32);

                let base_y = match container.v_align {
                    VerticalAlignment::Top => container_pos.y + font_atlas.ascent * scale_ratio,
                    VerticalAlignment::Middle => {
                        container_pos.y
                            + (container.dimensions.height - total_text_height) * 0.5
                            + font_atlas.ascent * scale_ratio
                    }
                    VerticalAlignment::Bottom => {
                        container_pos.y + container.dimensions.height
                            - total_text_height
                            - font_atlas.descent * scale_ratio
                    }
                };

                for (line_idx, (line, &line_width)) in
                    lines.iter().zip(line_widths.iter()).enumerate()
                {
                    let chars: Vec<char> = line.chars().collect();
                    let start_x = match container.h_align {
                        HorizontalAlignment::Left => container_pos.x,
                        HorizontalAlignment::Center => {
                            container_pos.x + (container.dimensions.width - line_width) * 0.5
                        }
                        HorizontalAlignment::Right => {
                            container_pos.x + container.dimensions.width - line_width
                        }
                    };

                    let baseline_y = Self::snap_to_pixel(
                        base_y + (line_idx as f32 * line_height),
                        raster.pixel_snap_scale,
                    );
                    let mut pen_x = start_x;
                    let mut prev: Option<GlyphId> = None;

                    for (i, c) in chars.iter().copied().enumerate() {
                        if c == ' ' {
                            pen_x += space_width + word_spacing;
                            prev = font_opt.map(|f| f.glyph(' ').id());
                            continue;
                        }

                        let use_char = if font_atlas.get_char_info(c).is_some() {
                            c
                        } else {
                            '?'
                        };
                        if let Some(char_info) = font_atlas.get_char_info(use_char) {
                            if let (Some(f), Some(p)) = (font_opt, prev) {
                                pen_x +=
                                    f.pair_kerning(scale, p, f.glyph(use_char).id()) * scale_ratio;
                            }

                            let top_left_x = Self::snap_to_pixel(
                                pen_x + (char_info.bearing.0 - raster.padding) * scale_ratio,
                                raster.pixel_snap_scale,
                            );
                            let top_left_y = Self::snap_to_pixel(
                                baseline_y - (char_info.bearing.1 + raster.padding) * scale_ratio,
                                raster.pixel_snap_scale,
                            );

                            chars_to_render.push(CharacterRenderInfo {
                                atlas_id: font_atlas.atlas_id(),
                                position: Position {
                                    x: top_left_x,
                                    y: top_left_y,
                                },
                                size: Size {
                                    width: (char_info.size.0 as f32 * scale_ratio).max(0.0),
                                    height: (char_info.size.1 as f32 * scale_ratio).max(0.0),
                                },
                                mode: GlyphRenderMode::AtlasTile {
                                    tile_index: char_info.tile_index,
                                    scale: scale_ratio,
                                },
                            });

                            pen_x += char_info.advance_width * scale_ratio;
                            if i + 1 < chars.len() {
                                pen_x += letter_spacing.max(0.0);
                            }
                            prev = font_opt.map(|f| f.glyph(use_char).id());
                        }
                    }
                }
            }
            FontAtlasData::Msdf(msdf) => {
                let target_font_size = font_size_override.unwrap_or(font_atlas.font_size).max(1.0);
                if self.should_use_tiny_raster(msdf, target_font_size) {
                    let Some(tiny) = &msdf.tiny_raster else {
                        return chars_to_render;
                    };
                    let scale_ratio = target_font_size / tiny.font_size.max(1.0);
                    // Keep tiny-raster baseline alignment in the same metric space as MSDF.
                    let base_line_height =
                        (font_atlas.ascent - font_atlas.descent).abs() * target_font_size;
                    let line_height = base_line_height * container.line_height_mul.max(0.8);
                    let space_width = tiny
                        .char_map
                        .get(&' ')
                        .map(|g| g.advance_width * scale_ratio)
                        .unwrap_or(target_font_size * 0.3);

                    let line_widths: Vec<f32> = lines
                        .iter()
                        .map(|line| {
                            let chars: Vec<char> = line.chars().collect();
                            let mut width = 0.0;
                            let mut prev: Option<char> = None;
                            for (i, c) in chars.iter().copied().enumerate() {
                                if c == ' ' {
                                    width += space_width + word_spacing;
                                    prev = Some(' ');
                                    continue;
                                }
                                let use_char = if tiny.char_map.contains_key(&c) {
                                    c
                                } else {
                                    '?'
                                };
                                if let Some(char_info) = tiny.char_map.get(&use_char) {
                                    width +=
                                        Self::msdf_kerning(msdf, prev, use_char) * target_font_size;
                                    width += char_info.advance_width * scale_ratio;
                                    if i + 1 < chars.len() {
                                        width += letter_spacing.max(0.0);
                                    }
                                    prev = Some(use_char);
                                }
                            }
                            width
                        })
                        .collect();

                    let total_text_height = line_height * line_count as f32;
                    let base_y = match container.v_align {
                        VerticalAlignment::Top => {
                            container_pos.y + font_atlas.ascent * target_font_size
                        }
                        VerticalAlignment::Middle => {
                            container_pos.y
                                + (container.dimensions.height - total_text_height) * 0.5
                                + font_atlas.ascent * target_font_size
                        }
                        VerticalAlignment::Bottom => {
                            container_pos.y + container.dimensions.height
                                - total_text_height
                                - font_atlas.descent * target_font_size
                        }
                    };

                    for (line_idx, (line, &line_width)) in
                        lines.iter().zip(line_widths.iter()).enumerate()
                    {
                        let chars: Vec<char> = line.chars().collect();
                        let start_x = match container.h_align {
                            HorizontalAlignment::Left => container_pos.x,
                            HorizontalAlignment::Center => {
                                container_pos.x + (container.dimensions.width - line_width) * 0.5
                            }
                            HorizontalAlignment::Right => {
                                container_pos.x + container.dimensions.width - line_width
                            }
                        };
                        let baseline_y = (base_y + (line_idx as f32 * line_height)).round();
                        let mut pen_x = start_x;
                        let mut prev: Option<char> = None;

                        for (i, c) in chars.iter().copied().enumerate() {
                            if c == ' ' {
                                pen_x += space_width + word_spacing;
                                prev = Some(' ');
                                continue;
                            }

                            let use_char = if tiny.char_map.contains_key(&c) {
                                c
                            } else {
                                '?'
                            };
                            let Some(char_info) = tiny.char_map.get(&use_char) else {
                                continue;
                            };

                            pen_x += Self::msdf_kerning(msdf, prev, use_char) * target_font_size;

                            let top_left_x = (pen_x
                                + (char_info.bearing.0 - tiny.padding) * scale_ratio)
                                .round();
                            let top_left_y = (baseline_y
                                - (char_info.bearing.1 + tiny.padding) * scale_ratio)
                                .round();
                            chars_to_render.push(CharacterRenderInfo {
                                atlas_id: tiny.atlas.get_id(),
                                position: Position {
                                    x: top_left_x,
                                    y: top_left_y,
                                },
                                size: Size {
                                    width: (char_info.size.0 as f32 * scale_ratio).max(1.0),
                                    height: (char_info.size.1 as f32 * scale_ratio).max(1.0),
                                },
                                mode: GlyphRenderMode::AtlasTile {
                                    tile_index: char_info.tile_index,
                                    scale: scale_ratio,
                                },
                            });

                            pen_x += char_info.advance_width * scale_ratio;
                            if i + 1 < chars.len() {
                                pen_x += letter_spacing.max(0.0);
                            }
                            prev = Some(use_char);
                        }
                    }
                } else {
                    let line_height =
                        msdf.line_height * target_font_size * container.line_height_mul.max(0.8);
                    let space_width = msdf
                        .glyphs
                        .get(&' ')
                        .map(|g| g.advance_width * target_font_size)
                        .unwrap_or(target_font_size * 0.3);

                    let line_widths: Vec<f32> = lines
                        .iter()
                        .map(|line| {
                            let chars: Vec<char> = line.chars().collect();
                            let mut width = 0.0;
                            let mut prev: Option<char> = None;
                            for (i, c) in chars.iter().copied().enumerate() {
                                if c == ' ' {
                                    width += space_width + word_spacing;
                                    prev = Some(' ');
                                    continue;
                                }
                                if let Some(use_char) = Self::resolve_msdf_char(msdf, c) {
                                    if let Some(glyph) = msdf.glyphs.get(&use_char) {
                                        width += Self::msdf_kerning(msdf, prev, use_char)
                                            * target_font_size;
                                        width += glyph.advance_width * target_font_size;
                                        if i + 1 < chars.len() {
                                            width += letter_spacing.max(0.0);
                                        }
                                        prev = Some(use_char);
                                    }
                                }
                            }
                            width
                        })
                        .collect();

                    let total_text_height = line_height * line_count as f32;
                    let base_y = match container.v_align {
                        VerticalAlignment::Top => {
                            container_pos.y + font_atlas.ascent * target_font_size
                        }
                        VerticalAlignment::Middle => {
                            container_pos.y
                                + (container.dimensions.height - total_text_height) * 0.5
                                + font_atlas.ascent * target_font_size
                        }
                        VerticalAlignment::Bottom => {
                            container_pos.y + container.dimensions.height
                                - total_text_height
                                - font_atlas.descent * target_font_size
                        }
                    };

                    for (line_idx, (line, &line_width)) in
                        lines.iter().zip(line_widths.iter()).enumerate()
                    {
                        let chars: Vec<char> = line.chars().collect();
                        let start_x = match container.h_align {
                            HorizontalAlignment::Left => container_pos.x,
                            HorizontalAlignment::Center => {
                                container_pos.x + (container.dimensions.width - line_width) * 0.5
                            }
                            HorizontalAlignment::Right => {
                                container_pos.x + container.dimensions.width - line_width
                            }
                        };
                        let baseline_y = base_y + (line_idx as f32 * line_height);
                        let mut pen_x = start_x;
                        let mut prev: Option<char> = None;

                        for (i, c) in chars.iter().copied().enumerate() {
                            if c == ' ' {
                                pen_x += space_width + word_spacing;
                                prev = Some(' ');
                                continue;
                            }

                            let Some(use_char) = Self::resolve_msdf_char(msdf, c) else {
                                continue;
                            };
                            let Some(glyph) = msdf.glyphs.get(&use_char) else {
                                continue;
                            };

                            pen_x += Self::msdf_kerning(msdf, prev, use_char) * target_font_size;

                            // Plane coordinates are in y-up em units, convert to logical y-down.
                            // Expand the quad by padding_em on each side so UVs cover the
                            // full padded atlas tile (the SDF field extends into the padding).
                            let pad_px = glyph.padding_em * target_font_size;
                            let glyph_left =
                                (glyph.plane_bounds.left - glyph.padding_em) * target_font_size;
                            let glyph_top =
                                (glyph.plane_bounds.top + glyph.padding_em) * target_font_size;
                            let glyph_width = (glyph.plane_bounds.right - glyph.plane_bounds.left)
                                .max(0.0)
                                * target_font_size
                                + pad_px * 2.0;
                            let glyph_height = (glyph.plane_bounds.top - glyph.plane_bounds.bottom)
                                .abs()
                                * target_font_size
                                + pad_px * 2.0;

                            let top_left_x = pen_x + glyph_left;
                            let top_left_y = baseline_y - glyph_top;

                            chars_to_render.push(CharacterRenderInfo {
                                atlas_id: font_atlas.atlas_id(),
                                position: Position {
                                    x: top_left_x,
                                    y: top_left_y,
                                },
                                size: Size {
                                    width: glyph_width.max(1.0),
                                    height: glyph_height.max(1.0),
                                },
                                mode: GlyphRenderMode::AtlasUv {
                                    uv_offset: glyph.uv_offset,
                                    uv_scale: glyph.uv_scale,
                                    is_msdf: true,
                                    msdf_px_range: msdf.px_range,
                                },
                            });

                            pen_x += glyph.advance_width * target_font_size;
                            if i + 1 < chars.len() {
                                pen_x += letter_spacing.max(0.0);
                            }
                            prev = Some(use_char);
                        }
                    }
                }
            }
        }

        chars_to_render
    }

    /// Measure caret advance for a single line prefix using the same pen math as layout.
    /// Input should not contain newlines; if present, measurement stops at the first newline.
    pub fn measure_caret_advance(
        &self,
        line_prefix: &str,
        font_key: &str,
        font_size: f32,
        letter_spacing: f32,
        word_spacing: f32,
    ) -> f32 {
        let font_atlas = match self.font_atlases.get(font_key) {
            Some(atlas) => atlas,
            None => return 0.0,
        };
        match &font_atlas.data {
            FontAtlasData::Raster(_) => {
                let scale_ratio = font_size / font_atlas.font_size;
                let scale = Scale::uniform(font_atlas.font_size);
                let font_opt = self.fonts.get(font_key);

                let space_width = font_atlas
                    .get_char_info(' ')
                    .map(|i| i.advance_width)
                    .unwrap_or(font_atlas.font_size * 0.3)
                    * scale_ratio;

                let mut pen_x = 0.0;
                let mut prev: Option<GlyphId> = None;

                let chars: Vec<char> = line_prefix.chars().collect();
                for (i, c) in chars.iter().enumerate() {
                    let c = *c;
                    if c == '\n' {
                        break;
                    }

                    if c == ' ' {
                        pen_x += space_width + word_spacing;
                        prev = font_opt.map(|f| f.glyph(' ').id());
                        continue;
                    }

                    let use_char = if font_atlas.get_char_info(c).is_some() {
                        c
                    } else {
                        '?'
                    };
                    if let Some(char_info) = font_atlas.get_char_info(use_char) {
                        if let (Some(f), Some(p)) = (font_opt, prev) {
                            pen_x += f.pair_kerning(scale, p, f.glyph(use_char).id()) * scale_ratio;
                        }
                        pen_x += char_info.advance_width * scale_ratio;
                        if i + 1 < chars.len() {
                            pen_x += letter_spacing.max(0.0);
                        }
                        prev = font_opt.map(|f| f.glyph(use_char).id());
                    }
                }

                pen_x
            }
            FontAtlasData::Msdf(msdf) => {
                if self.should_use_tiny_raster(msdf, font_size) {
                    let Some(tiny) = &msdf.tiny_raster else {
                        return 0.0;
                    };
                    let scale_ratio = font_size / tiny.font_size.max(1.0);
                    let space_width = tiny
                        .char_map
                        .get(&' ')
                        .map(|g| g.advance_width * scale_ratio)
                        .unwrap_or(font_size * 0.3);
                    let mut pen_x = 0.0;
                    let mut prev: Option<char> = None;

                    let chars: Vec<char> = line_prefix.chars().collect();
                    for (i, c) in chars.iter().enumerate() {
                        let c = *c;
                        if c == '\n' {
                            break;
                        }

                        if c == ' ' {
                            pen_x += space_width + word_spacing;
                            prev = Some(' ');
                            continue;
                        }

                        let use_char = if tiny.char_map.contains_key(&c) {
                            c
                        } else {
                            '?'
                        };
                        if let Some(char_info) = tiny.char_map.get(&use_char) {
                            pen_x += Self::msdf_kerning(msdf, prev, use_char) * font_size;
                            pen_x += char_info.advance_width * scale_ratio;
                            if i + 1 < chars.len() {
                                pen_x += letter_spacing.max(0.0);
                            }
                            prev = Some(use_char);
                        }
                    }

                    return pen_x;
                }

                let mut pen_x = 0.0;
                let mut prev: Option<char> = None;
                let space_width = msdf
                    .glyphs
                    .get(&' ')
                    .map(|g| g.advance_width * font_size)
                    .unwrap_or(font_size * 0.3);

                let chars: Vec<char> = line_prefix.chars().collect();
                for (i, c) in chars.iter().enumerate() {
                    let c = *c;
                    if c == '\n' {
                        break;
                    }

                    if c == ' ' {
                        pen_x += space_width + word_spacing;
                        prev = Some(' ');
                        continue;
                    }

                    if let Some(use_char) = Self::resolve_msdf_char(msdf, c) {
                        if let Some(glyph) = msdf.glyphs.get(&use_char) {
                            pen_x += Self::msdf_kerning(msdf, prev, use_char) * font_size;
                            pen_x += glyph.advance_width * font_size;
                            if i + 1 < chars.len() {
                                pen_x += letter_spacing.max(0.0);
                            }
                            prev = Some(use_char);
                        }
                    }
                }

                pen_x
            }
        }
    }

    /// Build per-glyph horizontal layout diagnostics for a single line.
    ///
    /// This mirrors the runtime pen math used by `measure_caret_advance` and
    /// `calculate_text_layout`, and is intended for debugging spacing issues.
    pub fn debug_line_layout_records(
        &self,
        line: &str,
        font_key: &str,
        font_size: f32,
        letter_spacing: f32,
        word_spacing: f32,
    ) -> Result<Vec<GlyphLayoutDebugRecord>, String> {
        let font_atlas = self
            .font_atlases
            .get(font_key)
            .ok_or_else(|| format!("font atlas not found for key '{}'", font_key))?;
        let mut records = Vec::new();
        let mut pen_x = 0.0f32;
        let chars: Vec<char> = line.chars().collect();

        match &font_atlas.data {
            FontAtlasData::Raster(raster) => {
                let scale_ratio = font_size / font_atlas.font_size.max(1.0);
                let scale = Scale::uniform(font_atlas.font_size);
                let font_opt = self.fonts.get(font_key);
                let mut prev: Option<GlyphId> = None;
                let space_width = font_atlas
                    .get_char_info(' ')
                    .map(|i| i.advance_width)
                    .unwrap_or(font_atlas.font_size * 0.3)
                    * scale_ratio;

                for (index, c) in chars.iter().copied().enumerate() {
                    if c == '\n' {
                        break;
                    }
                    let pen_before = pen_x;
                    if c == ' ' {
                        let advance_px = space_width + word_spacing;
                        let pen_after = pen_before + advance_px;
                        records.push(GlyphLayoutDebugRecord {
                            index,
                            input_char: c,
                            resolved_char: c,
                            mode: "raster-space",
                            pen_x_before: pen_before,
                            kerning_px: 0.0,
                            glyph_left_px: pen_before,
                            glyph_right_px: pen_after,
                            advance_px,
                            letter_spacing_px: 0.0,
                            pen_x_after: pen_after,
                        });
                        pen_x = pen_after;
                        prev = font_opt.map(|f| f.glyph(' ').id());
                        continue;
                    }

                    let use_char = if font_atlas.get_char_info(c).is_some() {
                        c
                    } else {
                        '?'
                    };
                    let Some(char_info) = font_atlas.get_char_info(use_char) else {
                        continue;
                    };

                    let kerning_px = if let (Some(f), Some(p)) = (font_opt, prev) {
                        f.pair_kerning(scale, p, f.glyph(use_char).id()) * scale_ratio
                    } else {
                        0.0
                    };
                    let pen_after_kerning = pen_before + kerning_px;
                    let advance_px = char_info.advance_width * scale_ratio;
                    let letter_spacing_px = if index + 1 < chars.len() {
                        letter_spacing.max(0.0)
                    } else {
                        0.0
                    };
                    let glyph_left_px =
                        pen_after_kerning + (char_info.bearing.0 - raster.padding) * scale_ratio;
                    let glyph_right_px = glyph_left_px + (char_info.size.0 as f32 * scale_ratio);
                    let pen_after = pen_after_kerning + advance_px + letter_spacing_px;

                    records.push(GlyphLayoutDebugRecord {
                        index,
                        input_char: c,
                        resolved_char: use_char,
                        mode: "raster",
                        pen_x_before: pen_before,
                        kerning_px,
                        glyph_left_px,
                        glyph_right_px,
                        advance_px,
                        letter_spacing_px,
                        pen_x_after: pen_after,
                    });

                    pen_x = pen_after;
                    prev = font_opt.map(|f| f.glyph(use_char).id());
                }
            }
            FontAtlasData::Msdf(msdf) => {
                let mut prev: Option<char> = None;
                if self.should_use_tiny_raster(msdf, font_size) {
                    let tiny = msdf
                        .tiny_raster
                        .as_ref()
                        .ok_or_else(|| "tiny raster fallback is missing".to_string())?;
                    let scale_ratio = font_size / tiny.font_size.max(1.0);
                    let space_width = tiny
                        .char_map
                        .get(&' ')
                        .map(|g| g.advance_width * scale_ratio)
                        .unwrap_or(font_size * 0.3);

                    for (index, c) in chars.iter().copied().enumerate() {
                        if c == '\n' {
                            break;
                        }
                        let pen_before = pen_x;
                        if c == ' ' {
                            let advance_px = space_width + word_spacing;
                            let pen_after = pen_before + advance_px;
                            records.push(GlyphLayoutDebugRecord {
                                index,
                                input_char: c,
                                resolved_char: c,
                                mode: "msdf-tiny-space",
                                pen_x_before: pen_before,
                                kerning_px: 0.0,
                                glyph_left_px: pen_before,
                                glyph_right_px: pen_after,
                                advance_px,
                                letter_spacing_px: 0.0,
                                pen_x_after: pen_after,
                            });
                            pen_x = pen_after;
                            prev = Some(' ');
                            continue;
                        }

                        let use_char = if tiny.char_map.contains_key(&c) {
                            c
                        } else {
                            '?'
                        };
                        let Some(char_info) = tiny.char_map.get(&use_char) else {
                            continue;
                        };
                        let kerning_px = Self::msdf_kerning(msdf, prev, use_char) * font_size;
                        let pen_after_kerning = pen_before + kerning_px;
                        let advance_px = char_info.advance_width * scale_ratio;
                        let letter_spacing_px = if index + 1 < chars.len() {
                            letter_spacing.max(0.0)
                        } else {
                            0.0
                        };
                        let glyph_left_px =
                            pen_after_kerning + (char_info.bearing.0 - tiny.padding) * scale_ratio;
                        let glyph_right_px =
                            glyph_left_px + (char_info.size.0 as f32 * scale_ratio);
                        let pen_after = pen_after_kerning + advance_px + letter_spacing_px;

                        records.push(GlyphLayoutDebugRecord {
                            index,
                            input_char: c,
                            resolved_char: use_char,
                            mode: "msdf-tiny",
                            pen_x_before: pen_before,
                            kerning_px,
                            glyph_left_px,
                            glyph_right_px,
                            advance_px,
                            letter_spacing_px,
                            pen_x_after: pen_after,
                        });

                        pen_x = pen_after;
                        prev = Some(use_char);
                    }
                } else {
                    let space_width = msdf
                        .glyphs
                        .get(&' ')
                        .map(|g| g.advance_width * font_size)
                        .unwrap_or(font_size * 0.3);

                    for (index, c) in chars.iter().copied().enumerate() {
                        if c == '\n' {
                            break;
                        }
                        let pen_before = pen_x;
                        if c == ' ' {
                            let advance_px = space_width + word_spacing;
                            let pen_after = pen_before + advance_px;
                            records.push(GlyphLayoutDebugRecord {
                                index,
                                input_char: c,
                                resolved_char: c,
                                mode: "msdf-space",
                                pen_x_before: pen_before,
                                kerning_px: 0.0,
                                glyph_left_px: pen_before,
                                glyph_right_px: pen_after,
                                advance_px,
                                letter_spacing_px: 0.0,
                                pen_x_after: pen_after,
                            });
                            pen_x = pen_after;
                            prev = Some(' ');
                            continue;
                        }

                        let Some(use_char) = Self::resolve_msdf_char(msdf, c) else {
                            continue;
                        };
                        let Some(glyph) = msdf.glyphs.get(&use_char) else {
                            continue;
                        };
                        let kerning_px = Self::msdf_kerning(msdf, prev, use_char) * font_size;
                        let pen_after_kerning = pen_before + kerning_px;
                        let advance_px = glyph.advance_width * font_size;
                        let letter_spacing_px = if index + 1 < chars.len() {
                            letter_spacing.max(0.0)
                        } else {
                            0.0
                        };
                        let pad_px = glyph.padding_em * font_size;
                        let glyph_left_px = pen_after_kerning
                            + (glyph.plane_bounds.left - glyph.padding_em) * font_size;
                        let glyph_width = (glyph.plane_bounds.right - glyph.plane_bounds.left)
                            .max(0.0)
                            * font_size
                            + pad_px * 2.0;
                        let glyph_right_px = glyph_left_px + glyph_width;
                        let pen_after = pen_after_kerning + advance_px + letter_spacing_px;

                        records.push(GlyphLayoutDebugRecord {
                            index,
                            input_char: c,
                            resolved_char: use_char,
                            mode: "msdf",
                            pen_x_before: pen_before,
                            kerning_px,
                            glyph_left_px,
                            glyph_right_px,
                            advance_px,
                            letter_spacing_px,
                            pen_x_after: pen_after,
                        });

                        pen_x = pen_after;
                        prev = Some(use_char);
                    }
                }
            }
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_msdf_metadata_json() {
        let json = r#"{
            "atlas": { "width": 256.0, "height": 256.0, "kind": "msdf" },
            "metrics": { "font_size": 32.0, "ascender": 0.8, "descender": -0.2, "line_height": 1.0 },
            "glyphs": [
                {
                    "unicode": 63,
                    "advance": 0.5,
                    "plane_bounds": { "left": 0.0, "top": 0.7, "right": 0.5, "bottom": -0.1 },
                    "atlas_bounds": { "left": 0.0, "top": 0.0, "right": 32.0, "bottom": 32.0 }
                }
            ],
            "kerning": []
        }"#;
        let parsed = TextRenderer::parse_msdf_metadata(json).expect("metadata should parse");
        assert_eq!(parsed.atlas.kind, "msdf");
        assert_eq!(parsed.glyphs.len(), 1);
        assert_eq!(parsed.metrics.px_range, DEFAULT_MSDF_PX_RANGE);
    }

    #[test]
    fn parses_msdf_metadata_with_explicit_px_range() {
        let json = r#"{
            "atlas": { "width": 256.0, "height": 256.0, "kind": "msdf" },
            "metrics": {
                "font_size": 32.0,
                "ascender": 0.8,
                "descender": -0.2,
                "line_height": 1.0,
                "px_range": 6.0
            },
            "glyphs": [],
            "kerning": []
        }"#;
        let parsed = TextRenderer::parse_msdf_metadata(json).expect("metadata should parse");
        assert_eq!(parsed.metrics.px_range, 6.0);
    }

    #[test]
    fn builds_msdf_glyph_map_from_ascii_only() {
        let metadata = MsdfFontMetadata {
            atlas: MsdfAtlasInfo {
                width: 128.0,
                height: 128.0,
                kind: "msdf".to_string(),
            },
            metrics: MsdfMetrics {
                font_size: 32.0,
                ascender: 0.8,
                descender: -0.2,
                line_height: 1.0,
                padding_em: 0.0,
                px_range: DEFAULT_MSDF_PX_RANGE,
            },
            glyphs: vec![
                MsdfGlyphRecord {
                    unicode: '?' as u32,
                    advance: 0.5,
                    plane_bounds: Bounds {
                        left: 0.0,
                        top: 0.7,
                        right: 0.5,
                        bottom: -0.1,
                    },
                    atlas_bounds: Bounds {
                        left: 0.0,
                        top: 0.0,
                        right: 32.0,
                        bottom: 32.0,
                    },
                },
                MsdfGlyphRecord {
                    unicode: 'A' as u32,
                    advance: 0.6,
                    plane_bounds: Bounds {
                        left: 0.0,
                        top: 0.7,
                        right: 0.6,
                        bottom: -0.1,
                    },
                    atlas_bounds: Bounds {
                        left: 32.0,
                        top: 0.0,
                        right: 64.0,
                        bottom: 32.0,
                    },
                },
                // Filtered out in phase 1
                MsdfGlyphRecord {
                    unicode: 'é' as u32,
                    advance: 0.6,
                    plane_bounds: Bounds {
                        left: 0.0,
                        top: 0.7,
                        right: 0.6,
                        bottom: -0.1,
                    },
                    atlas_bounds: Bounds {
                        left: 64.0,
                        top: 0.0,
                        right: 96.0,
                        bottom: 32.0,
                    },
                },
            ],
            kerning: vec![MsdfKerningRecord {
                left_unicode: 'A' as u32,
                right_unicode: '?' as u32,
                advance: -0.1,
            }],
        };

        let (glyphs, kernings) =
            TextRenderer::build_msdf_data(&metadata).expect("glyphs should build");
        assert!(glyphs.contains_key(&'?'));
        assert!(glyphs.contains_key(&'A'));
        assert!(!glyphs.contains_key(&'é'));
        assert_eq!(kernings.get(&('A', '?')).copied(), Some(-0.1));
    }

    #[test]
    fn converts_exact_bounds_to_msdf_plane_bounds() {
        let exact_bounds = rusttype::Rect {
            min: rusttype::point(-10.0, -40.0),
            max: rusttype::point(50.0, 12.0),
        };
        let out = TextRenderer::msdf_plane_bounds_from_exact_bounds(exact_bounds, 100.0, 1.25);
        assert!((out.left - (-0.125)).abs() < 1e-6);
        assert!((out.top - 0.5).abs() < 1e-6);
        assert!((out.right - 0.625).abs() < 1e-6);
        assert!((out.bottom - (-0.15)).abs() < 1e-6);
    }

    #[test]
    fn debug_msdf_vs_raster_advances() {
        let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));
        let json_path = format!(
            "{}/examples/media/roboto.msdf.json",
            env!("CARGO_MANIFEST_DIR")
        );

        let font_data = std::fs::read(&font_path).expect("read font");
        let font = Font::try_from_vec(font_data.clone()).expect("parse font");

        let json_text = std::fs::read_to_string(&json_path).expect("read json");
        let metadata = TextRenderer::parse_msdf_metadata(&json_text).expect("parse metadata");

        // Build glyph map from JSON
        let msdf_glyphs: HashMap<char, &MsdfGlyphRecord> = metadata
            .glyphs
            .iter()
            .filter_map(|g| char::from_u32(g.unicode).map(|ch| (ch, g)))
            .collect();

        // Get raster advances at font_size=32 (matching the MSDF JSON font_size)
        let font_size = 32.0f32;
        let scale = Scale::uniform(font_size);

        let text = "Sphinx of black quartz, judge my vow";
        let mut max_delta: f32 = 0.0;
        let mut msdf_pen = 0.0f32;
        let mut raster_pen = 0.0f32;

        for ch in text.chars() {
            if ch == ' ' {
                let msdf_space = msdf_glyphs
                    .get(&' ')
                    .map(|g| g.advance * font_size)
                    .unwrap_or(font_size * 0.3);
                let raster_space = font.glyph(' ').scaled(scale).h_metrics().advance_width;
                msdf_pen += msdf_space;
                raster_pen += raster_space;
                continue;
            }
            let msdf_g = msdf_glyphs.get(&ch).or(msdf_glyphs.get(&'?')).unwrap();
            let raster_adv = font.glyph(ch).scaled(scale).h_metrics().advance_width;
            let msdf_adv = msdf_g.advance * font_size;

            let delta = (msdf_adv - raster_adv).abs();
            if delta > max_delta {
                max_delta = delta;
            }

            msdf_pen += msdf_adv;
            raster_pen += raster_adv;
        }

        let total_delta = (msdf_pen - raster_pen).abs();
        eprintln!(
            "MSDF total: {:.2}px, Raster total: {:.2}px, delta: {:.4}px",
            msdf_pen, raster_pen, total_delta
        );
        eprintln!("Max per-char delta: {:.4}px", max_delta);

        // Compare quad width (from plane_bounds) vs actual tile width (from atlas_bounds)
        // These should match for correct UV mapping but use different bounding box methods
        let gen_scale = 4.0f32; // default gen_scale in msdf_bake
        let denom = font_size * gen_scale; // generation_font_size = 128
        let _padding_px = 10u32;

        eprintln!("\nQuad vs tile width mismatch (per glyph):");
        let mut total_width_error = 0.0f32;
        let mut count = 0;
        for code in 33u8..=126 {
            let ch = code as char;
            if let Some(msdf_g) = msdf_glyphs.get(&ch) {
                let pb = &msdf_g.plane_bounds;
                let ab = &msdf_g.atlas_bounds;

                // Quad width = plane_bounds width + 2*padding (in em, then *target)
                let quad_width_em = (pb.right - pb.left) + 2.0 * metadata.metrics.padding_em;
                let quad_width_gen = quad_width_em * denom; // in gen pixels

                // Tile width = atlas_bounds width (in gen pixels)
                let tile_width = ab.right - ab.left;

                let delta = tile_width - quad_width_gen;
                total_width_error += delta.abs();
                count += 1;

                if delta.abs() > 0.5 {
                    eprintln!(
                        "  '{}': quad_gen={:.2} tile={:.2} delta={:+.2}px",
                        ch, quad_width_gen, tile_width, delta
                    );
                }
            }
        }
        eprintln!(
            "Avg width error: {:.3} gen px ({:.3} screen px @32)",
            total_width_error / count as f32,
            total_width_error / count as f32 * font_size / denom
        );
    }
}
