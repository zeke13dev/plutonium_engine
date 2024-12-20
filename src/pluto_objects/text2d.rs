use crate::texture_svg::TextureSVG;
use crate::traits::PlutoObject;
use crate::traits::UpdateContext;
use crate::utils::{MouseInfo, Position, Rectangle};
use crate::PlutoniumEngine;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use winit::keyboard::Key;

// Internal Representation
pub struct Text2DInternal {
    id: Uuid,
    texture_key: Uuid,
    dimensions: Rectangle,
    font_size: f32,
    content: String,
    content_changed: bool,
}

impl Text2DInternal {
    pub fn new(
        id: Uuid,
        texture_key: Uuid,
        dimensions: Rectangle,
        font_size: f32,
        content: &str,
    ) -> Self {
        Self {
            id,
            texture_key,
            dimensions,
            font_size,
            content: content.to_string(),
            content_changed: false,
        }
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

    pub fn update(
        &mut self,
        texture_map: &mut HashMap<Uuid, TextureSVG>,
        update_context: &UpdateContext,
        dpi_scale_factor: f32,
    ) {
        if self.content_changed {
            texture_map
                .get_mut(&self.texture_key)
                .expect("Texture key should always refer to texture SVG")
                .update_text(
                    update_context.device,
                    update_context.queue,
                    &self.content,
                    self.font_size * dpi_scale_factor,
                    *update_context.viewport_size,
                    *update_context.camera_position,
                )
                .unwrap();
            self.content_changed = false;
        }
    }
}

impl PlutoObject for Text2DInternal {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn texture_key(&self) -> Uuid {
        self.texture_key
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
        texture_map: &mut HashMap<Uuid, TextureSVG>,
        update_context: Option<UpdateContext>,
        dpi_scale_factor: f32,
    ) {
        if let Some(context) = update_context {
            self.update(texture_map, &context, dpi_scale_factor);
        }
    }
}

// Wrapper Representation
pub struct Text2D {
    internal: Rc<RefCell<Text2DInternal>>,
}

impl Text2D {
    pub fn new(internal: Rc<RefCell<Text2DInternal>>) -> Self {
        Self { internal }
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

    pub fn update(
        &self,
        texture_map: &mut HashMap<Uuid, TextureSVG>,
        update_context: UpdateContext,
        dpi_scale_factor: f32,
    ) {
        self.internal
            .borrow_mut()
            .update(texture_map, &update_context, dpi_scale_factor);
    }

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.internal.borrow().render(engine);
    }

    pub fn get_id(&self) -> Uuid {
        self.internal.borrow().get_id()
    }
}
