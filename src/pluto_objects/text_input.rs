use crate::pluto_objects::{button::Button, text2d::Text2D};
use crate::text::TextRenderer;
use crate::traits::PlutoObject;
use crate::utils::{MouseInfo, Position, Rectangle};
use crate::PlutoniumEngine;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use winit::keyboard::{Key, NamedKey};

pub struct TextInputInternal {
    id: Uuid,
    button: Button,
    text: Text2D,
    cursor: Text2D,
    dimensions: Rectangle,
    focused: bool,
    cursor_index: usize,
}

impl TextInputInternal {
    pub fn new(
        id: Uuid,
        button: Button,
        text: Text2D,
        cursor: Text2D,
        dimensions: Rectangle,
    ) -> Self {
        let idx = text.get_content().len();
        Self {
            id,
            button,
            text,
            cursor,
            dimensions,
            focused: false,
            cursor_index: idx,
        }
    }

    pub fn set_focus(&mut self, focus: bool) {
        self.focused = focus;
    }

    pub fn set_content(&mut self, content: &str) {
        self.text.set_content(content);
    }

    pub fn clear(&mut self) {
        self.text.set_content("");
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.text.set_font_size(font_size);
        self.cursor.set_font_size(font_size);
    }
}

impl PlutoObject for TextInputInternal {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn render(&self, engine: &mut PlutoniumEngine) {
        self.button.render(engine);
        self.text.render(engine);
        self.cursor.render(engine);
    }

    fn update(
        &mut self,
        mouse_info: Option<MouseInfo>,
        key_pressed: &Option<Key>,
        _texture_map: &mut HashMap<Uuid, crate::texture_svg::TextureSVG>,
        _update_context: Option<crate::traits::UpdateContext>,
        _dpi_scale_factor: f32,
        text_renderer: &TextRenderer,
    ) {
        if let Some(mouse) = mouse_info {
            if mouse.is_lmb_clicked && self.dimensions.contains(mouse.mouse_pos) {
                self.set_focus(true);
                self.cursor_index = self
                    .text
                    .get_char_index_at_position(mouse.mouse_pos.x, text_renderer);
            }
        }
        let cursor_pos = self
            .text
            .get_cursor_position(self.cursor_index, text_renderer);
        self.cursor.set_pos(cursor_pos);

        // update content
        if !self.focused || key_pressed.is_none() {
            return;
        }
        match key_pressed.as_ref().unwrap() {
            Key::Character(c) => self.text.append_content(c),
            Key::Named(NamedKey::Backspace) => self.text.pop_content(),
            Key::Named(NamedKey::Space) => self.text.append_content(" "),
            _ => (),
        }
    }
    fn texture_key(&self) -> Uuid {
        self.button.texture_key()
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
}

pub struct TextInput {
    internal: Rc<RefCell<TextInputInternal>>,
}

impl TextInput {
    pub fn new(internal: Rc<RefCell<TextInputInternal>>) -> Self {
        // Set the focus callback on the button
        let internal_clone = Rc::clone(&internal);
        internal
            .borrow_mut()
            .button
            .set_callback(Some(Box::new(move || {
                internal_clone.borrow_mut().set_focus(true);
                println!("TextInput: Focused");
            })));

        Self { internal }
    }

    pub fn set_content(&self, content: &str) {
        self.internal.borrow_mut().set_content(content);
    }

    pub fn clear(&self) {
        self.internal.borrow_mut().clear();
    }

    pub fn set_font_size(&self, font_size: f32) {
        self.internal.borrow_mut().set_font_size(font_size);
    }

    pub fn set_focus(&self, focus: bool) {
        self.internal.borrow_mut().set_focus(focus);
    }

    pub fn internal(&self) -> Rc<RefCell<TextInputInternal>> {
        Rc::clone(&self.internal)
    }

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.internal.borrow().render(engine);
    }
}
