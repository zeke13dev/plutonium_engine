use crate::pluto_objects::{button::Button, text2d::Text2D};
use crate::texture_svg::TextureSVG;
use crate::traits::PlutoObject;
use crate::traits::UpdateContext;
use crate::utils::{MouseInfo, Position, Rectangle};
use crate::PlutoniumEngine;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use winit::keyboard::{Key, NamedKey};

pub struct TextInputInternal {
    id: Uuid,
    button: Button, // Owned directly
    text: Text2D,   // Owned directly
    cursor: Text2D, // Owned directly
    dimensions: Rectangle,
    _padding: f32, // Placeholder
    focused: bool,
}

impl TextInputInternal {
    pub fn new(
        id: Uuid,
        button: Button,
        text: Text2D,
        cursor: Text2D,
        dimensions: Rectangle,
    ) -> Self {
        Self {
            id,
            button,
            text,
            cursor,
            dimensions,
            _padding: 0.0,
            focused: false,
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

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.button.render(engine);
        self.text.render(engine);
        self.cursor.render(engine);
    }

    pub fn update(&mut self, key_pressed: Option<&Key>) {
        if !self.focused || key_pressed.is_none() {
            return;
        }

        match key_pressed.unwrap() {
            Key::Character(c) => self.text.append_content(c),
            Key::Named(NamedKey::Backspace) => self.text.pop_content(),
            Key::Named(NamedKey::Space) => self.text.append_content(" "),
            Key::Named(NamedKey::Shift) => self.text.append_content("\n"),
            _ => (),
        }
    }
}

pub struct TextInput {
    inner: Rc<RefCell<TextInputInternal>>,
}

impl TextInput {
    pub fn new(
        id: Uuid,
        mut button: Button, // Button mutably required for setting callback
        text: Text2D,
        cursor: Text2D,
        dimensions: Rectangle,
    ) -> Self {
        let inner = Rc::new(RefCell::new(TextInputInternal::new(
            id, button, text, cursor, dimensions,
        )));

        // Clone the Rc, not the RefCell
        let inner_clone = Rc::clone(&inner);
        inner
            .borrow_mut()
            .button
            .set_callback(Some(Box::new(move || {
                inner_clone.borrow_mut().set_focus(true);
                println!("TextInput: Focused");
            })));

        Self { inner }
    }

    pub fn set_content(&self, content: &str) {
        self.inner.borrow_mut().set_content(content);
    }

    pub fn clear(&self) {
        self.inner.borrow_mut().clear();
    }

    pub fn set_font_size(&self, font_size: f32) {
        self.inner.borrow_mut().set_font_size(font_size);
    }

    pub fn set_focus(&self, focus: bool) {
        self.inner.borrow_mut().set_focus(focus);
    }
}

impl PlutoObject for TextInput {
    fn get_id(&self) -> Uuid {
        self.inner.borrow().id
    }

    fn render(&self, engine: &mut PlutoniumEngine) {
        self.inner.borrow().render(engine);
    }

    fn update(
        &mut self,
        mouse_info: Option<MouseInfo>,
        key_pressed: &Option<Key>,
        _texture_map: &mut HashMap<Uuid, TextureSVG>,
        _update_context: Option<UpdateContext>,
        _dpi_scale_factor: f32,
    ) {
        let mut inner = self.inner.borrow_mut();
        if let Some(mouse) = mouse_info {
            if mouse.is_lmb_clicked && inner.dimensions.contains(mouse.mouse_pos) {
                inner.set_focus(true);
            }
        }
        inner.update(key_pressed.as_ref());
    }

    fn texture_key(&self) -> Uuid {
        self.inner.borrow().button.texture_key()
    }

    fn dimensions(&self) -> Rectangle {
        self.inner.borrow().dimensions
    }

    fn pos(&self) -> Position {
        self.inner.borrow().dimensions.pos()
    }

    fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.inner.borrow_mut().dimensions = new_dimensions;
    }

    fn set_pos(&mut self, new_position: Position) {
        self.inner.borrow_mut().dimensions.set_pos(new_position);
    }
}
