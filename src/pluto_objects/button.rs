use crate::pluto_objects::text2d::Text2D;
use crate::texture_svg::TextureSVG;
use crate::traits::{PlutoObject, UpdateContext};
use crate::utils::{MouseInfo, Position, Rectangle};
use crate::PlutoniumEngine;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use winit::keyboard::Key;

// Internal Representation
pub struct ButtonInternal {
    id: Uuid,
    texture_key: Uuid,
    text_object: Text2D,
    dimensions: Rectangle,
    callback: Option<Box<dyn Fn()>>,
    padding: f32, // Currently unused but could affect positioning
}

impl ButtonInternal {
    pub fn new(
        id: Uuid,
        texture_key: Uuid,
        dimensions: Rectangle,
        text_object: Text2D,
        callback: Option<Box<dyn Fn()>>,
    ) -> Self {
        Self {
            id,
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
        self.text_object.set_content("");
    }

    pub fn set_callback(&mut self, callback: Option<Box<dyn Fn()>>) {
        self.callback = callback;
    }

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        engine.queue_texture(&self.texture_key, Some(self.dimensions.pos()));
        self.text_object.render(engine);
    }

    pub fn update(&mut self, mouse_info: Option<MouseInfo>, _key_pressed: &Option<Key>) {
        if let Some(mouse) = mouse_info {
            if mouse.is_lmb_clicked && self.dimensions.contains(mouse.mouse_pos) {
                if let Some(ref callback) = self.callback {
                    callback();
                }
            }
        }
    }
}

impl PlutoObject for ButtonInternal {
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
        mouse_info: Option<MouseInfo>,
        key_pressed: &Option<Key>,
        _texture_map: &mut HashMap<Uuid, TextureSVG>,
        _update_context: Option<UpdateContext>,
        _dpi_scale_factor: f32,
    ) {
        self.update(mouse_info, key_pressed);
    }
}

// Wrapper Representation
pub struct Button {
    internal: Rc<RefCell<ButtonInternal>>,
}

impl Button {
    pub fn new(internal: Rc<RefCell<ButtonInternal>>) -> Self {
        Self { internal }
    }

    pub fn set_content(&self, new_content: &str) {
        self.internal.borrow_mut().set_content(new_content);
    }

    pub fn clear(&self) {
        self.internal.borrow_mut().clear();
    }

    pub fn set_callback(&self, callback: Option<Box<dyn Fn()>>) {
        self.internal.borrow_mut().set_callback(callback);
    }

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.internal.borrow().render(engine);
    }

    pub fn update(&self, mouse_info: Option<MouseInfo>, key_pressed: Option<Key>) {
        self.internal.borrow_mut().update(mouse_info, &key_pressed);
    }

    pub fn get_id(&self) -> Uuid {
        self.internal.borrow().get_id()
    }

    pub fn texture_key(&self) -> Uuid {
        self.internal.borrow().texture_key()
    }
    pub fn get_dimensions(&self) -> Rectangle {
        self.internal.borrow().dimensions()
    }

    pub fn set_dimensions(&self, dimensions: Rectangle) {
        self.internal.borrow_mut().set_dimensions(dimensions);
    }

    pub fn set_pos(&self, position: Position) {
        self.internal.borrow_mut().set_pos(position);
    }
}
