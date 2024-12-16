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

pub struct TextInput {
    inner: Rc<RefCell<TextInputInternal>>,
}

struct TextInputInternal {
    button_object: Button,
    text_object: Text2D,
    cursor_object: Text2D,
    dimensions: Rectangle,
    _padding: f32, // currently set to 0 always, should effect where text is
    focused: bool,
}

impl TextInput {
    pub fn new(
        button_object: Button,
        text_object: Text2D,
        dimensions: Rectangle,
        cursor_object: Text2D,
    ) -> Self {
        let inner = Rc::new(RefCell::new(TextInputInternal {
            button_object,
            text_object,
            cursor_object,
            dimensions,
            _padding: 0.0,
            focused: false,
        }));

        let inner_clone = Rc::clone(&inner);
        let callback = move || {
            let mut inner = inner_clone.borrow_mut();
            inner.focused = true;
            println!("TextInput.active set to true");
        };
        inner
            .borrow_mut()
            .button_object
            .set_callback(Some(Box::new(callback)));
        TextInput { inner }
    }

    pub fn set_content(&mut self, new_content: &str) {
        self.inner.borrow_mut().text_object.set_content(new_content);
    }

    pub fn clear(&mut self) {
        Self::set_content(self, ""); // i don't think this is correct
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        let mut text_input = self.inner.borrow_mut();
        text_input.text_object.set_font_size(font_size);
        text_input.cursor_object.set_font_size(font_size);
    }
}

impl PlutoObject for TextInput {
    fn render(&self, engine: &mut PlutoniumEngine) {
        let text_input = self.inner.borrow();
        text_input.button_object.render(engine); // renders text as well
        text_input.cursor_object.render(engine);
    }

    fn update(
        &mut self,
        _mouse_info: Option<MouseInfo>,
        key_pressed: &Option<Key>,
        _texture_map: &mut HashMap<Uuid, TextureSVG>,
        _update_context: Option<UpdateContext>,
        _dpi_scale_factor: f32,
    ) {
        let mut text_input = self.inner.borrow_mut();
        if !text_input.focused || key_pressed.is_none() {
            return;
        }

        match key_pressed.as_ref().unwrap() {
            Key::Character(c) => text_input.text_object.append_content(c),
            Key::Named(NamedKey::Shift) => text_input.text_object.append_content("\n"),
            Key::Named(NamedKey::Backspace) => {
                text_input.text_object.pop_content();
            }
            Key::Named(NamedKey::Space) => text_input.text_object.append_content(" "),
            _ => (),
        }

        // update cursor
        let pos = text_input.dimensions.pos();
        text_input.cursor_object.set_pos(pos);
    }

    fn texture_key(&self) -> Uuid {
        self.inner.borrow().button_object.texture_key() // maybe should error or smth
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
