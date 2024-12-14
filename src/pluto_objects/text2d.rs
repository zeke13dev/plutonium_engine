use crate::texture_svg::TextureSVG;
use crate::traits::PlutoObject;
use crate::traits::UpdateContext;
use crate::utils::{MouseInfo, Position, Rectangle};
use crate::PlutoniumEngine;
use std::collections::HashMap;
use uuid::Uuid;
use winit::keyboard::Key;

pub struct Text2D {
    texture_key: Uuid,
    dimensions: Rectangle,
    font_size: f32,
    content: String,
    content_changed: bool,
}

impl Text2D {
    pub fn new(texture_key: Uuid, dimensions: Rectangle, font_size: f32, content: &str) -> Self {
        Text2D {
            texture_key,
            dimensions,
            font_size,
            content: content.to_string(),
            content_changed: false,
        }
    }

    pub fn set_content(&mut self, new_content: &str) {
        self.content_changed = self.content != new_content; // string_equals in rust?
        self.content = new_content.to_string(); //
    }

    pub fn append_content(&mut self, new_content: &str) {
        self.content_changed = true;
        self.content.push_str(new_content);
    }

    pub fn pop_content(&mut self) {
        self.content_changed = true;
        self.content.pop();
    }
}

impl PlutoObject for Text2D {
    fn update(
        &mut self,
        _mouse_info: Option<MouseInfo>,
        _key_pressed: &Option<Key>,
        texture_map: &mut HashMap<Uuid, TextureSVG>,
        update_context: Option<UpdateContext>,
        dpi_scale_factor: f32,
    ) {
        if self.content_changed {
            if let Some(update_context) = update_context {
                texture_map
                    .get_mut(self.texture_key())
                    .expect("texture key should always refer to texture svg")
                    .update_text(
                        update_context.device,
                        update_context.queue,
                        &self.content,
                        self.font_size * dpi_scale_factor,
                        *update_context.viewport_size,
                        *update_context.camera_position,
                    )
                    .unwrap();
            }
            self.content_changed = false;
        }
    }

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
