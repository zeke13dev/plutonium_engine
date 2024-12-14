use crate::pluto_objects::{button::Button, text2d::Text2D};
use crate::texture_svg::TextureSVG;
use crate::traits::PlutoObject;
use crate::traits::UpdateContext;
use crate::utils::{MouseInfo, Position, Rectangle};
use crate::PlutoniumEngine;
use std::collections::HashMap;
use uuid::Uuid;
use winit::keyboard::{Key, NamedKey};

pub struct TextInput {
    button_object: Button,
    text_object: Text2D,
    dimensions: Rectangle,
    padding: f32, // currently set to 0 always, should effect where text is
    focused: bool,
}

impl TextInput {
    pub fn new(button_object: Button, text_object: Text2D, dimensions: Rectangle) -> Self {
        TextInput {
            button_object,
            text_object,
            dimensions,
            padding: 0.0,
            focused: false,
        }
    }

    pub fn set_content(&mut self, new_content: &str) {
        self.text_object.set_content(new_content);
    }

    pub fn clear(&mut self) {
        Self::set_content(self, ""); // i don't think this is correct
    }
}

impl PlutoObject for TextInput {
    fn render(&self, engine: &mut PlutoniumEngine) {
        self.button_object.render(engine);
        // render cursor?
    }

    fn update(
        &mut self,
        _mouse_info: Option<MouseInfo>,
        key_pressed: &Option<Key>,
        _texture_map: &mut HashMap<Uuid, TextureSVG>,
        _update_context: Option<UpdateContext>,
        _dpi_scale_factor: f32,
    ) {
        if !self.focused || key_pressed.is_none() {
            return;
        }

        match key_pressed.as_ref().unwrap() {
            Key::Character(c) => self.text_object.append_content(c),
            Key::Named(NamedKey::Shift) => self.text_object.append_content("\n"),
            Key::Named(NamedKey::Backspace) => {
                self.text_object.pop_content();
            }
            Key::Named(NamedKey::Space) => self.text_object.append_content(" "),
            _ => (),
        }

        // update cursor
        /*
        let text_cursor = texture_map
            .get_mut("text_cursor")
            .expect("text cursor should exist if we have a text input obj");
        text_cursor
            .update_text(
                update_context.device,
                update_context.queue,
                "|",
                self.font_size * dpi_scale_factor,
                *update_context.viewport_size,
                *update_context.camera_position,
            )
            .unwrap();

         text_cursor.set_position(
            texture_map
                .get(&self.texture_key)
                .expect("")
                .dimensions()
                .pos(),
        ) */
    }

    fn texture_key(&self) -> &Uuid {
        self.button_object.texture_key() // maybe should error or smth
    }

    fn dimensions(&self) -> &Rectangle {
        &self.dimensions
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
