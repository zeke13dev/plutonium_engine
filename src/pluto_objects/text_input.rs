use crate::pluto_objects::{button::Button, text2d::Text2D, texture_2d::Texture2D};
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
    cursor: Texture2D,
    dimensions: Rectangle,
    focused: bool,
    cursor_index: usize,
    current_line: usize,
}

impl TextInputInternal {
    pub fn new(
        id: Uuid,
        button: Button,
        text: Text2D,
        cursor: Texture2D,
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
            current_line: 0,
        }
    }

    // Mouse handling in update method
    pub fn handle_mouse(&mut self, mouse_info: &MouseInfo, text_renderer: &TextRenderer) {
        if mouse_info.is_lmb_clicked {
            let was_focused = self.focused;
            if self.dimensions.contains(mouse_info.mouse_pos) {
                self.set_focus(true);
                let (new_cursor_index, new_line) = self.text.get_cursor_position_info(
                    mouse_info.mouse_pos.x,
                    mouse_info.mouse_pos.y,
                    text_renderer,
                );
                self.cursor_index = new_cursor_index;
                self.current_line = new_line;
            } else {
                self.set_focus(false);
            }

            // Update cursor position immediately after click
            if was_focused != self.focused || self.focused {
                self.update_cursor_position(text_renderer);
            }
        }
    }

    fn handle_key_press(&mut self, key: &Key) -> bool {
        let mut cursor_moved = false;

        // Split content into lines for navigation
        let content = self.text.get_content();
        let lines: Vec<&str> = content.split('\n').collect();
        let (current_line_idx, current_column) = self.get_cursor_line_and_column(&lines);

        match key {
            Key::Character(c) => {
                self.text.insert_at(self.cursor_index, c);
                self.cursor_index += 1;
                cursor_moved = true;
            }
            Key::Named(NamedKey::Backspace) => {
                if self.cursor_index > 0 {
                    self.text.remove_at(self.cursor_index - 1);
                    self.cursor_index -= 1;
                    cursor_moved = true;
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.focused = false;
            }
            Key::Named(NamedKey::Space) => {
                self.text.insert_at(self.cursor_index, " ");
                self.cursor_index += 1;
                cursor_moved = true;
            }
            Key::Named(NamedKey::Enter) => {
                self.text.insert_at(self.cursor_index, "\n");
                self.cursor_index += 1;
                self.current_line += 1;
                cursor_moved = true;
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if self.cursor_index > 0 {
                    self.cursor_index -= 1;
                    cursor_moved = true;
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                if self.cursor_index < self.text.len() {
                    self.cursor_index += 1;
                    cursor_moved = true;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if current_line_idx > 0 {
                    // Move to previous line at same column if possible
                    let prev_line_len = lines[current_line_idx - 1].len();
                    let new_column = current_column.min(prev_line_len);

                    self.cursor_index = self.get_index_from_line_and_column(
                        current_line_idx - 1,
                        new_column,
                        &lines,
                    );
                    self.current_line -= 1;
                    cursor_moved = true;
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if current_line_idx < lines.len() - 1 {
                    // Move to next line at same column if possible
                    let next_line_len = lines[current_line_idx + 1].len();
                    let new_column = current_column.min(next_line_len);

                    self.cursor_index = self.get_index_from_line_and_column(
                        current_line_idx + 1,
                        new_column,
                        &lines,
                    );
                    self.current_line += 1;
                    cursor_moved = true;
                }
            }
            _ => (),
        }

        cursor_moved
    }

    // Helper function to get current line and column from cursor_index
    fn get_cursor_line_and_column(&self, lines: &[&str]) -> (usize, usize) {
        let mut remaining_chars = self.cursor_index;
        for (line_idx, line) in lines.iter().enumerate() {
            if remaining_chars <= line.len() {
                return (line_idx, remaining_chars);
            }
            remaining_chars -= line.len() + 1; // +1 for the newline character
        }
        (lines.len() - 1, lines.last().map(|l| l.len()).unwrap_or(0))
    }

    // Helper function to convert line and column to absolute index
    fn get_index_from_line_and_column(&self, line: usize, column: usize, lines: &[&str]) -> usize {
        let mut index = 0;
        for (i, line_content) in lines.iter().enumerate() {
            if i == line {
                return index + column;
            }
            index += line_content.len() + 1; // +1 for newline
        }
        index
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
        // TODO: RESCALE CURSOR
    }

    // Helper method to update cursor position
    fn update_cursor_position(&mut self, text_renderer: &TextRenderer) {
        self.cursor.set_pos(self.text.get_cursor_position(
            self.cursor_index,
            text_renderer,
            self.current_line,
        ));
    }
}

impl PlutoObject for TextInputInternal {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn render(&self, engine: &mut PlutoniumEngine) {
        self.button.render(engine);
        self.text.render(engine);
        if self.focused {
            self.cursor.render(engine);
        }
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
            self.handle_mouse(&mouse, text_renderer);
        }

        if !self.focused || key_pressed.is_none() {
            return;
        }

        if let Some(key) = key_pressed.as_ref() {
            if self.handle_key_press(key) {
                self.update_cursor_position(text_renderer);
            }
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
