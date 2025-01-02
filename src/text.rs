use crate::pluto_objects::{
    text2d::{HorizontalAlignment, TextContainer, VerticalAlignment},
    texture_atlas_2d::TextureAtlas2D,
};
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
    max_tile_size: Size,
    ascent: f32,
    descent: f32,
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
    pub fn measure_text(&self, text: &str, font_key: &str) -> (f32, usize) {
        if let Some(font_atlas) = self.font_atlases.get(font_key) {
            let mut max_width: f32 = 0.0;
            let mut current_width = 0.0;
            let mut line_count = 1;

            for c in text.chars() {
                if c == '\n' {
                    // Track the maximum width and reset current line width
                    max_width = max_width.max(current_width);
                    current_width = 0.0;
                    line_count += 1;
                    continue;
                }

                if c == ' ' {
                    current_width += font_atlas.font_size * 0.3;
                    continue;
                }

                if let Some(info) = font_atlas.char_map.get(&c) {
                    current_width += info.advance_width;
                }
            }

            // Don't forget to compare the last line's width
            max_width = max_width.max(current_width);

            (max_width, line_count)
        } else {
            (0.0, 1)
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

    #[allow(clippy::too_many_arguments)]
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
    ) {
        let logical_tile_size = Size {
            width: physical_tile_size.width / dpi_scale_factor,
            height: physical_tile_size.height / dpi_scale_factor,
        };

        let font_atlas = FontAtlas {
            atlas,
            char_map,
            font_size,
            max_tile_size: logical_tile_size, // Store in logical units
            ascent,
            descent,
        };
        self.font_atlases.insert(font_key.to_string(), font_atlas);
    }

    pub fn calculate_text_layout(
        &self,
        text: &str,
        font_key: &str,
        container_pos: Position,
        container: &TextContainer,
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

        // Calculate metrics
        let line_height = font_atlas.font_size * 1.2;
        let space_width = font_atlas.font_size * 0.3;
        let lines: Vec<&str> = text.split('\n').collect();
        let line_count = lines.len();

        // Compute line widths
        let line_widths: Vec<f32> = lines
            .iter()
            .map(|line| {
                line.chars().fold(0.0, |width, c| {
                    width
                        + if c == ' ' {
                            space_width
                        } else {
                            font_atlas
                                .get_char_info(c)
                                .map(|info| info.advance_width)
                                .unwrap_or(0.0)
                        }
                })
            })
            .collect();

        let total_text_height = line_height * (line_count as f32);

        // Calculate baseline Y position
        let base_y = match container.v_align {
            // Align top of text block with top of container
            VerticalAlignment::Top => container_pos.y + font_atlas.ascent,

            // Center the text block vertically in the container
            VerticalAlignment::Middle => {
                container_pos.y
                    + (container.dimensions.height - total_text_height) / 2.0
                    + font_atlas.ascent
            }

            // Align bottom of text block with bottom of container
            VerticalAlignment::Bottom => {
                container_pos.y + container.dimensions.height
                    - total_text_height
                    - font_atlas.descent // Adjust for descent if you track it
            }
        };

        // Render characters
        for (line_idx, (line, &line_width)) in lines.iter().zip(line_widths.iter()).enumerate() {
            let start_x = match container.h_align {
                HorizontalAlignment::Left => container_pos.x,
                HorizontalAlignment::Center => {
                    container_pos.x + (container.dimensions.width - line_width) / 2.0
                }
                HorizontalAlignment::Right => {
                    container_pos.x + container.dimensions.width - line_width
                }
            };

            let baseline_y = base_y + (line_idx as f32 * line_height);
            let mut pen_x = start_x;

            for c in line.chars() {
                if c == ' ' {
                    pen_x += space_width;
                    continue;
                }

                if let Some(char_info) = font_atlas.get_char_info(c) {
                    let logical_x = pen_x + char_info.bearing.0;
                    let logical_y = baseline_y - char_info.bearing.1;

                    chars_to_render.push(CharacterRenderInfo {
                        atlas_id: font_atlas.atlas.get_id(),
                        tile_index: char_info.tile_index,
                        position: Position {
                            x: logical_x,
                            y: logical_y,
                        },
                    });

                    pen_x += char_info.advance_width;
                }
            }
        }

        chars_to_render
    }
}
