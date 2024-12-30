use crate::pluto_objects::texture_atlas_2d::TextureAtlas2D;
use crate::utils::{Position, Size};
use rusttype::{point, Font, Scale};
use std::collections::HashMap;
use uuid::Uuid;

// Character information for the font atlas
#[derive(Clone, Debug)]
pub struct CharacterInfo {
    pub tile_index: usize,
    pub advance_width: f32,
    pub bearing: (f32, f32),
    pub size: (u32, u32),
}

pub struct CharacterRenderInfo {
    pub atlas_id: Uuid,
    pub tile_index: usize,
    pub position: Position,
}

pub enum FontError {
    IoError(std::io::Error),
    InvalidFontData,
    AtlasRenderError,
}

// Holds font-specific data including its atlas
pub struct FontAtlas {
    atlas: TextureAtlas2D,
    char_map: HashMap<char, CharacterInfo>,
    font_size: f32,
    _padding: u32,
    max_tile_size: Size,
}

impl FontAtlas {
    // for debugging
    pub fn get_tile_dimensions(&self) -> Size {
        self.max_tile_size
    }

    pub fn get_char_info(&self, c: char) -> Option<&CharacterInfo> {
        self.char_map.get(&c)
    }

    pub fn debug_save_atlas(&self) -> Result<(), std::io::Error> {
        for (c, info) in &self.char_map {
            println!(
                "Char '{}' (ASCII: {}) - Tile Index: {}, Size: {:?}, Bearing: {:?}, Advance: {}",
                c, *c as u32, info.tile_index, info.size, info.bearing, info.advance_width
            );
        }

        Ok(())
    }
}
#[derive(Default)]
pub struct TextRenderer {
    font_atlases: HashMap<String, FontAtlas>,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            font_atlases: HashMap::new(),
        }
    }

    pub fn calculate_text_layout(
        &self,
        text: &str,
        font_key: &str,
        position: Position,
        scale_factor: f32,
    ) -> Vec<CharacterRenderInfo> {
        let mut chars_to_render = Vec::new();
        let font_atlas = match self.font_atlases.get(font_key) {
            Some(atlas) => atlas,
            _ => return chars_to_render,
        };
        let mut pen_x = position.x;
        // Calculate the initial baseline by offsetting from the top by the font ascender
        let initial_baseline = position.y + (font_atlas.font_size * 0.35); // Approximate ascender height
        let mut baseline_y = initial_baseline;

        for c in text.chars() {
            if c == '\n' {
                baseline_y += font_atlas.font_size * 0.8;
                pen_x = position.x;
                continue;
            }

            // Handle space character
            if c == ' ' {
                // Use a fraction of the font size for space width
                pen_x += (font_atlas.font_size * 0.25) / scale_factor;
                continue;
            }

            if let Some(char_info) = font_atlas.get_char_info(c) {
                let char_pos = Position {
                    x: pen_x + char_info.bearing.0 / scale_factor,
                    y: baseline_y - char_info.bearing.1 / scale_factor,
                };

                chars_to_render.push(CharacterRenderInfo {
                    atlas_id: font_atlas.atlas.get_id(),
                    tile_index: char_info.tile_index,
                    position: char_pos,
                });

                pen_x += char_info.advance_width / scale_factor;
            }
        }
        chars_to_render
    }
    pub fn store_font_atlas(
        &mut self,
        font_key: &str,
        atlas: TextureAtlas2D,
        char_map: HashMap<char, CharacterInfo>,
        font_size: f32,
        _padding: u32,
        max_tile_size: Size,
    ) {
        let font_atlas = FontAtlas {
            atlas,
            char_map,
            font_size,
            _padding,
            max_tile_size,
        };
        self.font_atlases.insert(font_key.to_string(), font_atlas);
    }

    pub fn calculate_atlas_size(
        font: &Font,
        scale: Scale,
        padding: u32,
    ) -> (u32, u32, HashMap<char, (u32, u32)>, u32, u32) {
        let mut max_width = 0;
        let mut max_height = 0;
        let mut char_dimensions = HashMap::new();

        // Calculate dimensions for all printable ASCII characters
        for c in (32..=126).map(|c| c as u8 as char) {
            let glyph = font.glyph(c).scaled(scale).positioned(point(0.0, 0.0));

            if let Some(bb) = glyph.pixel_bounding_box() {
                let width = (bb.max.x - bb.min.x) as u32 + padding * 2;
                let height = (bb.max.y - bb.min.y) as u32 + padding * 2;

                max_width = max_width.max(width);
                max_height = max_height.max(height);
                char_dimensions.insert(c, (width, height));
            }
        }

        // Add extra padding to ensure we have enough space
        let chars_count = (126 - 32 + 1) as u32; // Number of printable ASCII characters
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
    pub fn measure_text(&self, text: &str, font_key: &str) -> f32 {
        if let Some(font_atlas) = self.font_atlases.get(font_key) {
            text.chars()
                .filter_map(|c| font_atlas.char_map.get(&c))
                .map(|info| info.advance_width)
                .sum()
        } else {
            0.0
        }
    }
    pub fn render_glyphs_to_atlas(
        font: &Font,
        scale: Scale,
        atlas_size: (u32, u32),
        char_dimensions: &HashMap<char, (u32, u32)>,
        padding: u32,
    ) -> Option<(Vec<u8>, HashMap<char, CharacterInfo>)> {
        let (atlas_width, atlas_height) = atlas_size;
        let mut texture_data = vec![0; (atlas_width * atlas_height * 4) as usize];
        let mut char_map = HashMap::new();
        let max_width = char_dimensions.values().map(|(w, _)| w).max().unwrap_or(&0) + padding * 2;
        let max_height = char_dimensions.values().map(|(_, h)| h).max().unwrap_or(&0) + padding * 2;
        let mut current_x = padding;
        let mut current_y = padding;
        let mut next_tile_index = 0;

        for c in (32..=126).map(|c| c as u8 as char) {
            let base_glyph = font.glyph(c).scaled(scale);
            let bearing_y = base_glyph
                .exact_bounding_box()
                .map(|bb| -bb.min.y)
                .unwrap_or(0.0);

            if let Some((width, height)) = char_dimensions.get(&c) {
                if current_x + max_width > atlas_width {
                    current_x = padding;
                    current_y += max_height;
                }

                let glyph = base_glyph.positioned(point(0.0, bearing_y));
                let glyph_x = current_x + padding;
                let glyph_y = current_y + padding;

                if let Some(_bb) = glyph.pixel_bounding_box() {
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
                        }
                    });

                    let h_metrics = glyph.unpositioned().h_metrics();
                    char_map.insert(
                        c,
                        CharacterInfo {
                            tile_index: next_tile_index,
                            advance_width: h_metrics.advance_width,
                            bearing: (h_metrics.left_side_bearing, bearing_y),
                            size: (*width, *height),
                        },
                    );
                    current_x += max_width;
                    next_tile_index += 1;
                }
            }
        }

        Some((texture_data, char_map))
    }
}
