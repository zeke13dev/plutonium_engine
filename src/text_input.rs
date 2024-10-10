use crate::texture_svg::TextureSVG;
use crate::utils::{Position, Size};
use winit::keyboard::Key;

pub struct TextInput {
    texture_key: String,
    focused: bool,
    content: String,
}

impl TextInput {
    // initializers
    pub fn new(texture_key: &str, svg_path: &str, size: Size, scale: f32) -> TextInput {
        TextInput {
            texture_key: texture_key.to_string(),
            focused: false,
            content: "".to_string(),
        }
    }

    // update functions

    /// mouse_pos is only passed if LMB is clicked
    pub fn update(
        &mut self,
        texture: &TextureSVG,
        mouse_pos: Option<Position>,
        key_pressed: Option<Key>,
    ) {
        if let Some(pos) = mouse_pos {
            self.focused = texture.contains(&pos);
        }

        if let Some(key) = key_pressed {
            if self.focused {
                if let Some(c) = key.to_text() {
                    &self.content.push_str(c);
                }
            }
        }
    }

    // rendering functions
    pub fn render() {
        // to implement
    }
}
