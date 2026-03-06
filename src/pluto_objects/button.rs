use crate::pluto_objects::text2d::{HorizontalAlignment, Text2D, TextContainer, VerticalAlignment};
use crate::text::TextRenderer;
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
    on_click: Option<Box<dyn Fn()>>,
    on_focus: Option<Box<dyn Fn()>>,
    on_unfocus: Option<Box<dyn Fn()>>,
    is_focused: bool,
}

impl ButtonInternal {
    pub fn new(id: Uuid, texture_key: Uuid, dimensions: Rectangle, text_object: Text2D) -> Self {
        Self {
            id,
            texture_key,
            dimensions,
            text_object,
            on_click: None,
            on_focus: None,
            on_unfocus: None,
            is_focused: false,
        }
    }

    pub fn set_content(&mut self, new_content: &str) {
        self.text_object.set_content(new_content);
    }

    pub fn clear(&mut self) {
        self.text_object.set_content("");
    }

    pub fn set_on_click(&mut self, callback: Option<Box<dyn Fn()>>) {
        self.on_click = callback;
    }

    pub fn set_on_focus(&mut self, callback: Option<Box<dyn Fn()>>) {
        self.on_focus = callback;
    }

    pub fn set_on_unfocus(&mut self, callback: Option<Box<dyn Fn()>>) {
        self.on_unfocus = callback;
    }

    pub fn render(&mut self, engine: &mut PlutoniumEngine) {
        let container = TextContainer::new(self.dimensions)
            .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
            .with_padding(0.0);
        self.text_object.set_container(container.clone());

        engine.queue_texture_with_layer(&self.texture_key, Some(self.dimensions.pos()), 1);

        let content = self.text_object.get_content();
        let font_key = self.text_object.get_font_key();
        let font_size = self.text_object.get_font_size();
        let color = self.text_object.get_color();

        let (text_width, line_count) =
            engine
                .text_renderer
                .measure_text(&content, &font_key, 0.0, 0.0, Some(font_size));

        let scaled_width = text_width;
        let text_height = font_size * line_count as f32;

        let container_pos = container.calculate_text_position(scaled_width, text_height);

        let mut neutral = container.clone();
        neutral.h_align = HorizontalAlignment::Left;
        neutral.v_align = VerticalAlignment::Top;

        engine.queue_text_with_spacing(
            &content,
            &font_key,
            container_pos,
            &neutral,
            0.0,
            0.0,
            10,
            color,
            Some(font_size),
        );
    }

    pub fn update(&mut self, mouse_info: Option<MouseInfo>, _key_pressed: &Option<Key>) {
        if let Some(mouse) = mouse_info {
            let contains_mouse = self.dimensions.contains(mouse.mouse_pos);

            // Handle focus/unfocus events
            match (self.is_focused, contains_mouse) {
                (false, true) => {
                    self.is_focused = true;
                    if let Some(ref callback) = self.on_focus {
                        callback();
                    }
                }
                (true, false) => {
                    self.is_focused = false;
                    if let Some(ref callback) = self.on_unfocus {
                        callback();
                    }
                }
                _ => {}
            }

            // Handle click events
            if mouse.is_lmb_clicked && contains_mouse {
                if let Some(ref callback) = self.on_click {
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
        _text_renderer: &TextRenderer,
    ) {
        self.update(mouse_info, key_pressed);
    }

    fn render(&self, engine: &mut PlutoniumEngine) {
        // We need &mut self for our custom render, but trait requires &self
        // This is a workaround - we'll need to refactor or use interior mutability
        // For now, fallback to the simple texture queue
        engine.queue_texture_with_layer(&self.texture_key, Some(self.dimensions.pos()), 1);
        // TODO: Render text properly here
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

    pub fn set_on_click(&self, callback: Option<Box<dyn Fn()>>) {
        self.internal.borrow_mut().set_on_click(callback);
    }

    pub fn set_on_focus(&self, callback: Option<Box<dyn Fn()>>) {
        self.internal.borrow_mut().set_on_focus(callback);
    }

    pub fn set_on_unfocus(&self, callback: Option<Box<dyn Fn()>>) {
        self.internal.borrow_mut().set_on_unfocus(callback);
    }

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        // Call the inherent render method directly on ButtonInternal
        ButtonInternal::render(&mut *self.internal.borrow_mut(), engine);
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
