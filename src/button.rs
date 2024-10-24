use crate::texture_svg::TextureSVG;
use crate::traits::UpdateContext;
use crate::utils::{MouseInfo, Rectangle};
use crate::PlutoObject;
use crate::PlutoniumEngine;
use std::collections::HashMap;
use winit::keyboard::Key;

pub struct Button {
    texture_key: String,
    text_texture_key: String,
    content: String,
    dimensions: Rectangle,
    padding: f32,
    callback: Option<Box<dyn Fn()>>,
}

impl Button {
    // initializers
    pub fn new(
        texture_key: &str,
        dimensions: Rectangle,
        padding: f32,
        content: &str,
        callback: Option<Box<dyn Fn()>>,
    ) -> Button {
        let text_texture_key = format!("text_{}", texture_key);

        Button {
            texture_key: texture_key.to_string(),
            text_texture_key,
            content: content.to_string(),
            dimensions,
            padding,
            callback,
        }
    }

    pub fn set_content(&mut self, content: &str) {
        self.content = content.to_string();
    }

    pub fn clear(&mut self) {
        self.content = "".to_string();
    }

    pub fn set_callback(&mut self, callback: Option<Box<dyn Fn()>>) {
        self.callback = callback;
    }
}

impl PlutoObject for Button {
    fn render(&self, engine: &mut PlutoniumEngine) {
        engine.queue_texture(&self.texture_key, None);
        engine.queue_text(&self.text_texture_key);
    }

    fn update(
        &mut self,
        mouse_info: Option<MouseInfo>,
        _key_pressed: &Option<Key>,
        texture_map: &mut HashMap<String, TextureSVG>,
        update_context: Option<UpdateContext>,
    ) {
        if let Some(mouse) = mouse_info {
            // eventually this will check other focused methods
            if mouse.is_lmb_clicked
                && texture_map
                    .get(&self.texture_key)
                    .expect("texture key should always refer to texture svg")
                    .dimensions()
                    .padded_contains(mouse.mouse_pos, self.padding)
            {
                if let Some(ref callback) = self.callback {
                    callback();
                }
            }
        }

        if let Some(update_context) = update_context {
            texture_map
                .get_mut(&self.text_texture_key)
                .expect("texture key should always refer to texture svg")
                .update_text(
                    update_context.device,
                    update_context.queue,
                    &self.content,
                    12.0,
                    *update_context.viewport_size,
                    *update_context.camera_position,
                )
                .unwrap();
        }
    }
}
