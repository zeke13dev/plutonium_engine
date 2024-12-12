use crate::traits::PlutoObject;
use crate::utils::{Position, Rectangle};
use crate::PlutoniumEngine;
use uuid::Uuid;

pub struct Text2D {
    texture_key: Uuid,
    dimensions: Rectangle,
    font_size: f32,
    text: String,
}

impl Text2D {
    pub fn new(texture_key: Uuid, dimensions: Rectangle, font_size: f32, text: &str) -> Self {
        Text2D {
            texture_key,
            dimensions,
            font_size,
            text: text.to_string(),
        }
    }
}

impl PlutoObject for Text2D {
    fn texture_key(&self) -> &Uuid {
        &self.texture_key
    }

    fn dimensions(&self) -> &Rectangle {
        &self.dimensions
    }

    fn pos(&self) -> Position {
        self.dimensions.pos()
    }

    fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.dimensions = new_dimensions;
    }

    fn set_pos(&mut self, new_position: Position) {
        self.dimensions.set_pos(new_position);
    }
}
