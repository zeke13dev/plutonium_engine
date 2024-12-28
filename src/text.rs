use crate::pluto_objects::texture_atlas_2d::TextureAtlas2D;
use crate::traits::PlutoObject;
use crate::utils::{Position, Rectangle, Size};
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
    atlas_size: (u32, u32),
    padding: u32,
    max_tile_width: u32,  // Added
    max_tile_height: u32, // Added
}

impl FontAtlas {
    // for debugging
    pub fn get_tile_dimensions(&self) -> (f32, f32) {
        (self.max_tile_width as f32, self.max_tile_height as f32)
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
    ) -> Vec<CharacterRenderInfo> {
        let mut chars_to_render = Vec::new();
        let font_atlas = match self.font_atlases.get(font_key) {
            Some(atlas) => atlas,
            None => return chars_to_render,
        };
        let (tile_width, tile_height) = font_atlas.get_tile_dimensions();

        let mut current_x = position.x;
        let mut current_y = position.y;

        for c in text.chars() {
            if let Some(char_info) = font_atlas.get_char_info(c) {
                if c == '\n' {
                    current_y += font_atlas.font_size * 1.2;
                    current_x = position.x;
                    continue;
                }

                let char_pos = Position {
                    x: current_x + char_info.bearing.0,
                    y: current_y - char_info.bearing.1,
                };

                chars_to_render.push(CharacterRenderInfo {
                    atlas_id: font_atlas.atlas.get_id(),
                    tile_index: char_info.tile_index,
                    position: char_pos,
                });

                current_x += char_info.advance_width;
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
        atlas_size: (u32, u32),
        padding: u32,
        max_tile_width: u32,  // Added
        max_tile_height: u32, // Added
    ) {
        let font_atlas = FontAtlas {
            atlas,
            char_map,
            font_size,
            atlas_size,
            padding,
            max_tile_width,
            max_tile_height,
        };
        self.font_atlases.insert(font_key.to_string(), font_atlas);
    }

    pub fn calculate_atlas_size(
        font: &Font,
        scale: Scale,
        padding: u32,
    ) -> (u32, u32, HashMap<char, (u32, u32)>, u32, u32) {
        let mut total_area = 0;
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
                total_area += width * height;
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

        // Find maximum glyph dimensions for consistent tile size
        let max_width = char_dimensions.values().map(|(w, _)| w).max().unwrap_or(&0) + padding * 2;
        let max_height = char_dimensions.values().map(|(_, h)| h).max().unwrap_or(&0) + padding * 2;

        let tiles_per_row = atlas_width / max_width;

        let mut current_x = padding;
        let mut current_y = padding;
        let mut next_tile_index = 0;

        for c in (32..=126).map(|c| c as u8 as char) {
            let glyph = font.glyph(c).scaled(scale).positioned(point(0.0, 0.0));

            if let (Some((width, height)), Some(bb)) =
                (char_dimensions.get(&c), glyph.pixel_bounding_box())
            {
                // Check if we need to move to next row - use max_width for consistent spacing
                if current_x + max_width > atlas_width {
                    current_x = padding;
                    current_y += max_height; // Use max_height for consistent row height
                }

                // Center the glyph within its tile
                let x_offset = (max_width - width) / 2;
                let y_offset = (max_height - height) / 2;
                let glyph_x = current_x + x_offset;
                let glyph_y = current_y + y_offset;

                // Draw the glyph
                glyph.draw(|x, y, v| {
                    let px = glyph_x + x as u32;
                    let py = glyph_y + y as u32;

                    if px < atlas_width && py < atlas_height {
                        let index = ((py * atlas_width + px) * 4) as usize;
                        let alpha = (v * 255.0) as u8;
                        texture_data[index] = 255; // R
                        texture_data[index + 1] = 255; // G
                        texture_data[index + 2] = 255; // B
                        texture_data[index + 3] = alpha; // A
                    }
                });

                // Store character information
                let char_pos = Position {
                    // Ensure positive coordinates by offsetting any negative values
                    x: (bb.min.x as f32).max(0.0) + current_x as f32,
                    y: (bb.max.y as f32).max(0.0) + current_y as f32,
                };

                // When storing character info
                char_map.insert(
                    c,
                    CharacterInfo {
                        tile_index: next_tile_index,
                        advance_width: glyph.unpositioned().h_metrics().advance_width,
                        bearing: (
                            bb.min.x.max(0) as f32, // Ensure non-negative bearing
                            bb.max.y as f32,
                        ),
                        size: (*width, *height),
                    },
                );
                // Move to next tile position - use max_width for consistent spacing
                current_x += max_width;
                next_tile_index += 1;
            }
        }

        Some((texture_data, char_map))
    }
}
