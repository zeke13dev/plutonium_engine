use crate::traits::PlutoObject;
use crate::utils::{Position, Rectangle};
use crate::PlutoniumEngine;
use uuid::Uuid;

pub struct Texture2D {
    texture_key: Uuid,
    dimensions: Rectangle,
    position: Position,
}

impl Texture2D {
    pub fn new(texture_key: Uuid, dimensions: Rectangle) -> Self {
        Texture2D {
            texture_key,
            dimensions,
            position: dimensions.pos(),
        }
    }
}

impl PlutoObject for Texture2D {
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
