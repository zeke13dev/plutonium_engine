use crate::utils::{MouseInfo, Rectangle};
use crate::PlutoObject;
use crate::PlutoniumEngine;
use winit::keyboard::Key;

pub struct TextInput {
    texture_key: String,
    text_texture_key: String,
    focused: bool,
    content: String,
    dimensions: Rectangle,
    padding: f32,
}

impl TextInput {
    // initializers
    pub fn new(texture_key: &str, scale: f32, dimensions: Rectangle, padding: f32) -> TextInput {
        let text_texture_key = format!("text_{}", texture_key);

        TextInput {
            texture_key: texture_key.to_string(),
            text_texture_key,
            focused: false,
            content: "".to_string(),
            dimensions,
            padding,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn clear(&mut self) {
        self.content = "".to_string();
    }
}

impl PlutoObject for TextInput {
    fn render(&self, engine: &mut PlutoniumEngine) {
        engine.queue_texture(&self.texture_key, None);
        engine.queue_texture(&self.text_texture_key, None);
    }

    fn update(&mut self, mouse_info: Option<MouseInfo>, key_pressed: &Option<Key>) {
        if let Some(mouse) = mouse_info {
            if mouse.is_lmb_clicked {
                self.focused = self
                    .dimensions
                    .padded_contains(mouse.mouse_pos, self.padding);
            }
        }

        if let Some(key) = key_pressed {
            if self.focused {
                // should match for which logical key it is here!
                if let Some(c) = key.to_text() {
                    self.content.push_str(c);
                }
            }
        }
    }
}
