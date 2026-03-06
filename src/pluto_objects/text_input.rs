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
                self.sync_cursor_state();
            } else {
                self.set_focus(false);
            }

            // Update cursor position immediately after click
            if was_focused != self.focused || self.focused {
                self.update_cursor_position(text_renderer);
            }
        }
    }

    fn sync_cursor_state(&mut self) {
        let content = self.text.get_content();
        self.cursor_index = self.clamp_to_char_boundary(&content, self.cursor_index);
        self.current_line = self.line_for_index(&content, self.cursor_index);
    }

    fn clamp_to_char_boundary(&self, content: &str, index: usize) -> usize {
        let mut idx = index.min(content.len());
        while idx > 0 && !content.is_char_boundary(idx) {
            idx -= 1;
        }
        idx
    }

    fn prev_char_boundary(&self, content: &str, index: usize) -> usize {
        let idx = self.clamp_to_char_boundary(content, index);
        if idx == 0 {
            0
        } else {
            content[..idx]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0)
        }
    }

    fn next_char_boundary(&self, content: &str, index: usize) -> usize {
        let idx = self.clamp_to_char_boundary(content, index);
        if idx >= content.len() {
            content.len()
        } else {
            let mut iter = content[idx..].char_indices();
            if let Some((_, ch)) = iter.next() {
                idx + ch.len_utf8()
            } else {
                content.len()
            }
        }
    }

    fn line_for_index(&self, content: &str, index: usize) -> usize {
        content[..self.clamp_to_char_boundary(content, index)]
            .chars()
            .filter(|&ch| ch == '\n')
            .count()
    }

    fn get_cursor_line_and_column(&self, content: &str) -> (usize, usize) {
        let mut line = 0;
        let mut column = 0;
        let idx = self.clamp_to_char_boundary(content, self.cursor_index);
        for ch in content[..idx].chars() {
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        (line, column)
    }

    fn get_line_count(&self, content: &str) -> usize {
        content.chars().filter(|&ch| ch == '\n').count() + 1
    }

    fn line_start_index(&self, content: &str, target_line: usize) -> usize {
        if target_line == 0 {
            return 0;
        }
        let mut line = 0;
        for (i, ch) in content.char_indices() {
            if ch == '\n' {
                line += 1;
                if line == target_line {
                    return i + ch.len_utf8();
                }
            }
        }
        content.len()
    }

    fn get_index_from_line_and_column(&self, content: &str, line: usize, column: usize) -> usize {
        let start = self.line_start_index(content, line);
        let mut idx = start;
        let mut remaining = column;
        for (offset, ch) in content[start..].char_indices() {
            if ch == '\n' || remaining == 0 {
                break;
            }
            idx = start + offset + ch.len_utf8();
            remaining -= 1;
        }
        idx
    }

    fn handle_key_press(&mut self, key: &Key) -> bool {
        let mut cursor_moved = false;
        self.sync_cursor_state();
        let content = self.text.get_content();
        let (current_line_idx, current_column) = self.get_cursor_line_and_column(&content);
        let line_count = self.get_line_count(&content);

        match key {
            Key::Character(c) => {
                if !c.is_empty() {
                    self.text.insert_at(self.cursor_index, c);
                    self.cursor_index += c.len();
                    cursor_moved = true;
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if self.cursor_index > 0 {
                    let delete_index = self.prev_char_boundary(&content, self.cursor_index);
                    if self.text.remove_at(delete_index) {
                        self.cursor_index = delete_index;
                    }
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
                cursor_moved = true;
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if self.cursor_index > 0 {
                    self.cursor_index = self.prev_char_boundary(&content, self.cursor_index);
                    cursor_moved = true;
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                if self.cursor_index < content.len() {
                    self.cursor_index = self.next_char_boundary(&content, self.cursor_index);
                    cursor_moved = true;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if current_line_idx > 0 {
                    self.cursor_index = self.get_index_from_line_and_column(
                        &content,
                        current_line_idx - 1,
                        current_column,
                    );
                    cursor_moved = true;
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if current_line_idx + 1 < line_count {
                    self.cursor_index = self.get_index_from_line_and_column(
                        &content,
                        current_line_idx + 1,
                        current_column,
                    );
                    cursor_moved = true;
                }
            }
            _ => (),
        }

        self.sync_cursor_state();
        cursor_moved
    }

    pub fn set_focus(&mut self, focus: bool) {
        self.focused = focus;
    }

    pub fn set_content(&mut self, content: &str) {
        self.text.set_content(content);
        self.cursor_index = self.text.get_content().len();
        self.sync_cursor_state();
    }

    pub fn clear(&mut self) {
        self.text.set_content("");
        self.cursor_index = 0;
        self.current_line = 0;
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.text.set_font_size(font_size);
        // TODO: RESCALE CURSOR
    }

    // Helper method to update cursor position
    fn update_cursor_position(&mut self, text_renderer: &TextRenderer) {
        self.sync_cursor_state();
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
            .set_on_click(Some(Box::new(move || {
                internal_clone.borrow_mut().set_focus(true);
            })));

        let internal_clone = Rc::clone(&internal);
        internal
            .borrow_mut()
            .button
            .set_on_unfocus(Some(Box::new(move || {
                internal_clone.borrow_mut().set_focus(false);
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
