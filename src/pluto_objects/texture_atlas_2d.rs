use crate::traits::PlutoObject;
use crate::utils::{Position, Rectangle, Size};
use crate::PlutoniumEngine;
use uuid::Uuid;

pub struct TextureAtlas2D {
    texture_key: Uuid,
    dimensions: Rectangle,
    tile_size: Size,
}

impl TextureAtlas2D {
    pub fn new(texture_key: Uuid, dimensions: Rectangle, tile_size: Size) -> Self {
        TextureAtlas2D {
            texture_key,
            dimensions,
            tile_size,
        }
    }

    pub fn render_tile(
}

impl PlutoObject for TextureAtlas2D {
    fn texture_key(&self) -> &Uuid {
        &self.texture_key
    }

    fn dimensions(&self) -> &Rectangle {
        &self.dimensions
    }

    fn pos(&self) -> &Position {
        &self.position
    }

    fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.dimensions = new_dimensions;
    }

    fn set_pos(&mut self, new_position: Position) {
        self.position = new_position;
    }
}
