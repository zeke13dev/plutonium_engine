use crate::pluto_objects::text2d::Text2D;
use crate::texture_svg::TextureSVG;
use crate::traits::PlutoObject;
use crate::traits::UpdateContext;
use crate::utils::{MouseInfo, Position, Rectangle, Size};
use crate::PlutoniumEngine;
use std::collections::HashMap;
use uuid::Uuid;
use winit::keyboard::Key;
pub struct Button {
    texture_key: Uuid,
    text_object: Text2D,
    dimensions: Rectangle,
    callback: Option<Box<dyn Fn()>>,
    padding: f32, // currently set to 0 always, should effect where text is
}

impl Button {
    pub fn new(
        texture_key: Uuid,
        dimensions: Rectangle,
        text_object: Text2D,
        callback: Option<Box<dyn Fn()>>,
    ) -> Self {
        Button {
            texture_key,
            dimensions,
            text_object,
            callback,
            padding: 0.0,
        }
    }

    pub fn set_content(&mut self, new_content: &str) {
        self.text_object.set_content(new_content);
    }

    pub fn clear(&mut self) {
        Self::set_content(self, ""); // i don't think this is correct
    }

    pub fn set_callback(&mut self, callback: Option<Box<dyn Fn()>>) {
        self.callback = callback;
    }
}

impl PlutoObject for Button {
    fn render(&self, engine: &mut PlutoniumEngine) {
        engine.queue_texture(&self.texture_key, Some(self.dimensions.pos()));
        self.text_object.render(engine);
    }

    fn update(
        &mut self,
        mouse_info: Option<MouseInfo>,
        _key_pressed: &Option<Key>,
        texture_map: &mut HashMap<Uuid, TextureSVG>,
        _update_context: Option<UpdateContext>,
        _dpi_scale_factor: f32,
    ) {
        if let Some(mouse) = mouse_info {
            // eventually this will check other is_focused methods
            if mouse.is_lmb_clicked
                && texture_map
                    .get(&self.texture_key)
                    .expect("texture key should always refer to texture svg")
                    .dimensions()
                    .contains(mouse.mouse_pos)
            {
                if let Some(ref callback) = self.callback {
                    callback();
                }
            }
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
