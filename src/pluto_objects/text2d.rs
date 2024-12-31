use crate::TextureSVG;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use winit::keyboard::Key;

use crate::traits::{PlutoObject, UpdateContext};
use crate::utils::{MouseInfo, Position, Rectangle};
use crate::PlutoniumEngine;

use crate::text::TextRenderer;
// Text2D Implementation
pub struct Text2DInternal {
    id: Uuid,
    font_key: String,
    dimensions: Rectangle,
    font_size: f32,
    content: String,
    content_changed: bool,
}

impl Text2DInternal {
    pub fn new(
        id: Uuid,
        font_key: String,
        dimensions: Rectangle,
        font_size: f32,
        content: &str,
    ) -> Self {
        Self {
            id,
            font_key,
            dimensions,
            font_size,
            content: content.to_string(),
            content_changed: false,
        }
    }

    // Add this new method to get cursor position at a specific character index
    pub fn get_cursor_position(&self, char_index: usize, text_renderer: &TextRenderer) -> Position {
        let text = &self.content[..char_index.min(self.content.len())];
        let width = text_renderer.measure_text(text, &self.font_key);

        Position {
            x: self.dimensions.x + width,
            y: self.dimensions.y,
        }
    }

    // Helper method to find character index at a given x position
    pub fn get_char_index_at_position(&self, x_pos: f32, text_renderer: &TextRenderer) -> usize {
        let mut current_width = 0.0;
        let relative_x = x_pos - self.dimensions.x;

        for (idx, _) in self.content.char_indices() {
            let substr = &self.content[..idx];
            let width = text_renderer.measure_text(substr, &self.font_key);

            // If we're closer to the previous character's position, return that index
            if width > relative_x {
                if idx > 0 && (width - relative_x > relative_x - current_width) {
                    return idx - 1;
                }
                return idx;
            }
            current_width = width;
        }

        self.content.len()
    }

    pub fn get_font_size(&self) -> f32 {
        self.font_size
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.content_changed = font_size != self.font_size;
        self.font_size = font_size;
    }

    pub fn set_content(&mut self, new_content: &str) {
        self.content_changed = self.content != new_content;
        self.content = new_content.to_string();
    }

    pub fn append_content(&mut self, new_content: &str) {
        self.content_changed = true;
        self.content.push_str(new_content);
    }

    pub fn pop_content(&mut self) {
        self.content_changed = true;
        self.content.pop();
    }

    pub fn get_text(&self) -> &str {
        &self.content
    }

    pub fn get_font(&self) -> &str {
        &self.font_key
    }
}

impl PlutoObject for Text2DInternal {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn texture_key(&self) -> Uuid {
        self.id
    }

    fn dimensions(&self) -> Rectangle {
        self.dimensions
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

    fn update(
        &mut self,
        _mouse_info: Option<MouseInfo>,
        _key_pressed: &Option<Key>,
        _texture_map: &mut HashMap<Uuid, TextureSVG>,
        _update_context: Option<UpdateContext>,
        _dpi_scale_factor: f32,
        text_renderer: &TextRenderer, // Add this parameter
    ) {
        if self.content_changed {
            let width = text_renderer // Use the parameter directly
                .measure_text(&self.content, &self.font_key);
            self.dimensions.width = width;
            self.dimensions.height = self.font_size;
            self.content_changed = false;
        }
    }
    fn render(&self, engine: &mut PlutoniumEngine) {
        engine.queue_text(&self.content, &self.font_key, self.dimensions.pos());
    }
}

pub struct Text2D {
    internal: Rc<RefCell<Text2DInternal>>,
}

impl Text2D {
    pub fn new(internal: Rc<RefCell<Text2DInternal>>) -> Self {
        Self { internal }
    }

    pub fn get_cursor_position(&self, char_index: usize, text_renderer: &TextRenderer) -> Position {
        self.internal
            .borrow()
            .get_cursor_position(char_index, text_renderer)
    }

    pub fn get_char_index_at_position(&self, x_pos: f32, text_renderer: &TextRenderer) -> usize {
        self.internal
            .borrow()
            .get_char_index_at_position(x_pos, text_renderer)
    }

    pub fn set_font_size(&self, font_size: f32) {
        self.internal.borrow_mut().set_font_size(font_size);
    }

    pub fn get_content(&self) -> String {
        self.internal.borrow().content.clone()
    }

    pub fn set_content(&self, content: &str) {
        self.internal.borrow_mut().set_content(content);
    }

    pub fn append_content(&self, content: &str) {
        self.internal.borrow_mut().append_content(content);
    }

    pub fn pop_content(&self) {
        self.internal.borrow_mut().pop_content();
    }

    pub fn get_font_size(&self) -> f32 {
        self.internal.borrow().get_font_size()
    }

    pub fn get_dimensions(&self) -> Rectangle {
        self.internal.borrow().dimensions()
    }

    pub fn get_pos(&self) -> Position {
        self.internal.borrow().pos()
    }

    pub fn set_pos(&self, position: Position) {
        self.internal.borrow_mut().set_pos(position);
    }

    pub fn set_dimensions(&self, dimensions: Rectangle) {
        self.internal.borrow_mut().set_dimensions(dimensions);
    }

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.internal.borrow().render(engine);
    }

    pub fn get_id(&self) -> Uuid {
        self.internal.borrow().get_id()
    }
}
