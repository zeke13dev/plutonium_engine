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

        println!("Calculating layout for text: {}", text);
        println!("Using font: {}", font_key);
        println!("Atlas dimensions: {:?}", font_atlas.atlas_size);
        let (tile_width, tile_height) = font_atlas.get_tile_dimensions();
        println!("Tile dimensions: {}x{}", tile_width, tile_height);

        let mut current_x = position.x;
        let mut current_y = position.y;

        for c in text.chars() {
            if let Some(char_info) = font_atlas.get_char_info(c) {
                println!(
                    "Character '{}' - tile_index: {}, bearing: {:?}, size: {:?}",
                    c, char_info.tile_index, char_info.bearing, char_info.size
                );

                if c == '\n' {
                    current_y += font_atlas.font_size * 1.2;
                    current_x = position.x;
                    continue;
                }

                let char_pos = Position {
                    x: current_x + char_info.bearing.0,
                    y: current_y + (font_atlas.font_size - char_info.bearing.1),
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
        // Added max dimensions to return
        let mut total_area = 0;
        let mut max_width = 0;
        let mut max_height = 0;
        let mut char_dimensions = HashMap::new();

        // Calculate maximum dimensions across all characters
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

        // Calculate atlas dimensions
        let total_width = (total_area as f32).sqrt().ceil() as u32;
        let total_height = ((total_area as f32 / total_width as f32).ceil() as u32).max(max_height);

        (
            total_width,
            total_height,
            char_dimensions,
            max_width,
            max_height,
        )
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
        let mut debug_char_positions = HashMap::new();

        // Find maximum glyph dimensions for tile size
        let max_width = char_dimensions.values().map(|(w, _)| w).max().unwrap_or(&0) + padding * 2;
        let max_height = char_dimensions.values().map(|(_, h)| h).max().unwrap_or(&0) + padding * 2;

        println!("Atlas dimensions: {}x{}", atlas_width, atlas_height);
        println!("Max tile dimensions: {}x{}", max_width, max_height);

        // Calculate grid layout
        let tiles_per_row = atlas_width / max_width;
        let tiles_per_col = atlas_height / max_height;

        println!(
            "Tiles per row: {}, Tiles per column: {}",
            tiles_per_row, tiles_per_col
        );

        let mut current_x = padding;
        let mut current_y = padding;
        let mut current_row_height = 0;
        let mut next_tile_index = 0;

        // Process each printable ASCII character
        for c in (32..=126).map(|c| c as u8 as char) {
            let glyph = font.glyph(c).scaled(scale).positioned(point(0.0, 0.0));

            if let (Some((width, height)), Some(bb)) =
                (char_dimensions.get(&c), glyph.pixel_bounding_box())
            {
                // Check if we need to move to next row
                if current_x + width + padding > atlas_width {
                    current_x = padding;
                    current_y += current_row_height + padding;
                    current_row_height = 0;
                }

                // Store debug position information
                let debug_rect = Rectangle {
                    x: current_x as f32,
                    y: current_y as f32,
                    width: *width as f32,
                    height: *height as f32,
                };
                debug_char_positions.insert(c, debug_rect);

                println!(
                    "Character '{}' at tile index {} - Position: ({}, {})",
                    c, next_tile_index, current_x, current_y
                );

                // Draw the glyph into texture data
                glyph.draw(|x, y, v| {
                    let px = current_x + x as u32;
                    let py = current_y + y as u32;

                    if px < atlas_width && py < atlas_height {
                        let index = ((py * atlas_width + px) * 4) as usize;
                        let alpha = (v * 255.0) as u8;
                        texture_data[index] = 255; // R
                        texture_data[index + 1] = 255; // G
                        texture_data[index + 2] = 255; // B
                        texture_data[index + 3] = alpha; // A
                    }
                });

                // Store character information with sequential indexing
                char_map.insert(
                    c,
                    CharacterInfo {
                        tile_index: next_tile_index,
                        advance_width: glyph.unpositioned().h_metrics().advance_width,
                        bearing: (bb.min.x as f32, bb.max.y as f32),
                        size: (*width, *height),
                    },
                );

                // Update position tracking
                current_x += width + padding;
                current_row_height = current_row_height.max(*height + padding);
                next_tile_index += 1;
            }
        }

        // Store debug positions in a static or global for debugging if needed
        println!("Debug character positions:");
        for (c, rect) in &debug_char_positions {
            println!(
                "Char '{}' at: x={}, y={}, w={}, h={}",
                c, rect.x, rect.y, rect.width, rect.height
            );
        }

        Some((texture_data, char_map))
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
}
