use crate::TextureSVG;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use winit::keyboard::Key;

use crate::pluto_objects::shapes::Shape;
use crate::utils::{MouseInfo, Position, Rectangle};
use crate::{
    traits::{PlutoObject, UpdateContext},
    PlutoniumEngine,
};

use crate::text::TextRenderer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HorizontalAlignment {
    Left,   // Text starts from left edge
    Center, // Text is centered horizontally
    Right,  // Text ends at right edge
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalAlignment {
    Top,    // Text starts from top edge
    Middle, // Text is centered vertically
    Bottom, // Text starts from bottom edge (default)
}

#[derive(Debug, Clone)]
pub struct TextContainer {
    pub dimensions: Rectangle,
    pub h_align: HorizontalAlignment,
    pub v_align: VerticalAlignment,
    pub padding: f32,
    pub line_height_mul: f32, // extra leading multiplier
}

impl Default for TextContainer {
    fn default() -> Self {
        Self {
            dimensions: Rectangle::new(0.0, 0.0, 0.0, 0.0), // Zero-sized by default
            h_align: HorizontalAlignment::Left,
            v_align: VerticalAlignment::Bottom,
            padding: 5.0,
            line_height_mul: 1.0,
        }
    }
}

impl TextContainer {
    pub fn new(dimensions: Rectangle) -> Self {
        Self {
            dimensions,
            h_align: HorizontalAlignment::Left,
            v_align: VerticalAlignment::Top,
            padding: 5.0,
            line_height_mul: 1.0,
        }
    }

    pub fn with_alignment(
        mut self,
        h_align: HorizontalAlignment,
        v_align: VerticalAlignment,
    ) -> Self {
        self.h_align = h_align;
        self.v_align = v_align;
        self
    }

    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn with_line_height_mul(mut self, mul: f32) -> Self {
        self.line_height_mul = mul;
        self
    }

    pub fn calculate_text_position(&self, text_width: f32, text_height: f32) -> Position {
        // Calculate the content area after padding
        let content_dimensions = Rectangle::new(
            self.dimensions.x + self.padding,
            self.dimensions.y + self.padding,
            self.dimensions.width.max(0.0) - (self.padding * 2.0),
            self.dimensions.height.max(0.0) - (self.padding * 2.0),
        );

        // Horizontal positioning (this part is correct and remains the same)
        let x = match self.h_align {
            HorizontalAlignment::Left => content_dimensions.x,
            HorizontalAlignment::Center => {
                content_dimensions.x + (content_dimensions.width - text_width) / 2.0
            }
            HorizontalAlignment::Right => {
                content_dimensions.x + content_dimensions.width - text_width
            }
        };

        // Vertical positioning needs adjustment
        let y = match self.v_align {
            // For top alignment, baseline starts at top of content area
            VerticalAlignment::Top => content_dimensions.y,

            // For middle alignment, center the text vertically
            VerticalAlignment::Middle => {
                content_dimensions.y + (content_dimensions.height - text_height) / 2.0
            }

            // For bottom alignment, baseline is at bottom of content area minus descent
            VerticalAlignment::Bottom => {
                content_dimensions.y + content_dimensions.height - text_height
            }
        };

        Position { x, y }
    }
    pub fn set_dimensions(&mut self, dimensions: Rectangle) {
        self.dimensions = dimensions;
    }

    fn get_dimensions(&self) -> Rectangle {
        self.dimensions
    }
}
// Text2D Implementation
pub struct Text2DInternal {
    id: Uuid,
    font_key: String,
    dimensions: Rectangle,
    font_size: f32,
    content: String,
    container: TextContainer,
}

impl Text2DInternal {
    pub fn new(
        id: Uuid,
        font_key: String,
        dimensions: Rectangle,
        font_size: f32,
        content: &str,
        container: Option<TextContainer>,
    ) -> Self {
        let default_container = TextContainer::new(Rectangle::new(
            dimensions.x,
            dimensions.y,
            dimensions.width,
            dimensions.height,
        ))
        .with_alignment(HorizontalAlignment::Left, VerticalAlignment::Top)
        .with_padding(0.0);

        Self {
            id,
            font_key,
            dimensions,
            font_size,
            content: content.to_string(),
            container: container.unwrap_or(default_container),
        }
    }

    pub fn set_dimensions(&mut self, dimensions: Rectangle) {
        self.dimensions = dimensions;
        // Update container bounds to match new dimensions if using default container
        if self.container.get_dimensions() == self.dimensions {
            self.container.set_dimensions(dimensions);
        }
    }
    pub fn get_render_position(&self, text_width: f32) -> Position {
        self.container
            .calculate_text_position(text_width, self.font_size)
    }

    pub fn set_container(&mut self, container: TextContainer) {
        self.container = container;
    }

    pub fn get_container(&self) -> &TextContainer {
        &self.container
    }

    pub fn get_container_mut(&mut self) -> &mut TextContainer {
        &mut self.container
    }

    pub fn reset_container(&mut self) {
        self.container = TextContainer::new(self.dimensions);
    }

    pub fn get_cursor_position(
        &self,
        char_index: usize,
        text_renderer: &TextRenderer,
        current_line: usize,
    ) -> Position {
        let padding = self.font_size * 0.15;
        let line_height = self.font_size * 1.2;

        // Split text into lines
        let text = &self.content[..char_index.min(self.content.len())];
        let lines: Vec<&str> = text.split('\n').collect();

        // Handle case where current_line is beyond available lines
        if current_line >= lines.len() {
            return Position {
                x: self.dimensions.x + padding,
                y: self.dimensions.y + current_line as f32 * line_height,
            };
        }

        // Get the text of just the current line
        let current_line_text = lines[current_line];

        // Measure the width of the current line
        let line_width = text_renderer
            .measure_text(current_line_text, &self.font_key)
            .0;

        Position {
            x: self.dimensions.x + line_width + padding,
            y: self.dimensions.y + current_line as f32 * line_height,
        }
    }
    pub fn get_cursor_position_info(
        &self,
        x_pos: f32,
        y_pos: f32,
        text_renderer: &TextRenderer,
    ) -> (usize, usize) {
        // Returns (cursor_index, line_number)
        // Split content into lines
        let lines: Vec<&str> = self.content.split('\n').collect();

        // Calculate line height
        let line_height = self.font_size * 1.2;

        // Find which line was clicked
        let relative_y = y_pos - self.dimensions.y;
        let clicked_line = (relative_y / line_height).floor() as usize;
        let clicked_line = clicked_line.min(lines.len().saturating_sub(1));

        // Get text up to the clicked line
        let index_offset = lines
            .iter()
            .take(clicked_line)
            .fold(0, |offset, line| offset + line.len() + 1);

        // Now handle horizontal position within the line
        let line_content = lines[clicked_line];
        let relative_x = x_pos - self.dimensions.x;

        // Handle click before line start
        if relative_x <= 0.0 {
            return (index_offset, clicked_line);
        }

        let line_width = text_renderer.measure_text(line_content, &self.font_key).0;

        // Handle click beyond line end
        if relative_x >= line_width {
            return (index_offset + line_content.len(), clicked_line);
        }

        // Measure each character position in the current line
        let mut prev_width = 0.0;
        for (idx, _) in line_content.char_indices() {
            let substr = &line_content[..=idx];
            let width = text_renderer.measure_text(substr, &self.font_key).0;

            // Find the midpoint between current and previous character
            let char_midpoint = (width + prev_width) / 2.0;

            // If click position is before the midpoint, place cursor before current char
            if relative_x < char_midpoint {
                return (index_offset + idx, clicked_line);
            }

            // If this is the last character and we haven't returned yet,
            // the cursor should go after it
            if idx == line_content.len() - 1 {
                return (index_offset + idx + 1, clicked_line);
            }

            prev_width = width;
        }

        // If line is empty, return cursor at line start
        (index_offset, clicked_line)
    }
    pub fn get_font_size(&mut self) -> f32 {
        self.font_size
    }
    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
    }

    pub fn set_content(&mut self, new_content: &str) {
        self.content = new_content.to_string();
    }

    pub fn append_content(&mut self, new_content: &str) {
        self.content.push_str(new_content);
    }

    pub fn pop_content(&mut self) -> bool {
        if !self.content.is_empty() {
            self.content.pop();
            true
        } else {
            false
        }
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
        // Measure the text dimensions
        let (text_width, line_count) = text_renderer.measure_text(&self.content, &self.font_key);

        // Update the dimensions of the text container
        self.dimensions.width = text_width;
        self.dimensions.height = self.font_size * line_count as f32;

        // Update the container's dimensions to reflect changes
        self.container.set_dimensions(self.dimensions);
    }

    fn render(&self, engine: &mut PlutoniumEngine) {
        // First measure the text
        let (text_width, line_count) = engine
            .text_renderer
            .measure_text(&self.content, &self.font_key);
        let text_height = self.font_size * line_count as f32;

        // Compute a single alignment via container, then render with a neutral container
        let container_pos = self
            .container
            .calculate_text_position(text_width, text_height);

        // Avoid applying alignment twice: provide a neutral container for layout
        let mut neutral = self.container.clone();
        neutral.h_align = HorizontalAlignment::Left;
        neutral.v_align = VerticalAlignment::Top;
        engine.queue_text_with_spacing(
            &self.content,
            &self.font_key,
            container_pos,
            &neutral,
            0.0,
            0.0,
        );
    }
}

pub struct Text2D {
    internal: Rc<RefCell<Text2DInternal>>,
}

impl Text2D {
    pub fn create_debug_visualization(&self, engine: &mut PlutoniumEngine) -> Shape {
        let inner = self.internal.borrow();
        let container = inner.get_container();

        // Create a rectangle shape that matches the container's dimensions
        let rect = engine.create_rect(
            container.dimensions,
            container.dimensions.pos(),
            "rgba(0, 0, 255, 0.1)".to_string(), // Semi-transparent blue fill
            "rgba(0, 0, 255, 0.8)".to_string(), // Solid blue outline
            1.0,
        );

        // If you want to visualize the padding area, create another rectangle
        let content_area = Rectangle::new(
            container.dimensions.x + container.padding,
            container.dimensions.y + container.padding,
            container.dimensions.width - (container.padding * 2.0),
            container.dimensions.height - (container.padding * 2.0),
        );

        let _padding_rect = engine.create_rect(
            content_area,
            content_area.pos(),
            "rgba(255, 0, 0, 0.1)".to_string(), // Semi-transparent red fill
            "rgba(255, 0, 0, 0.8)".to_string(), // Solid red outline
            1.0,
        );

        rect
    }

    pub fn new(internal: Rc<RefCell<Text2DInternal>>) -> Self {
        Self { internal }
    }

    pub fn get_cursor_position(
        &self,
        char_index: usize,
        text_renderer: &TextRenderer,
        current_line: usize,
    ) -> Position {
        self.internal
            .borrow()
            .get_cursor_position(char_index, text_renderer, current_line)
    }

    pub fn get_cursor_position_info(
        &self,
        x_pos: f32,
        y_pos: f32,
        text_renderer: &TextRenderer,
    ) -> (usize, usize) {
        self.internal
            .borrow()
            .get_cursor_position_info(x_pos, y_pos, text_renderer)
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

    pub fn pop_content(&self) -> bool {
        self.internal.borrow_mut().pop_content()
    }

    pub fn get_font_size(&self) -> f32 {
        self.internal.borrow().font_size
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

    pub fn set_container(&mut self, container: TextContainer) {
        self.internal.borrow_mut().set_container(container);
    }

    pub fn log_container(&self) {
        println!("{:?}", self.internal.borrow().get_container());
    }

    pub fn container_bounds(&self) -> Rectangle {
        self.internal.borrow().get_container().dimensions
    }

    pub fn reset_container(&self) {
        self.internal.borrow_mut().reset_container();
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.internal.borrow().content.len()
    }

    pub fn insert_at(&mut self, index: usize, c: &str) {
        let mut internal = self.internal.borrow_mut();
        let content = &mut internal.content;

        // Ensure index is within bounds (including allowing insertion at the end)
        if index > content.len() {
            return;
        }

        // Handle different insertion cases
        if content.is_empty() || index == content.len() {
            content.push_str(c);
        } else {
            let (before, after) = content.split_at(index);
            *content = format!("{}{}{}", before, c, after);
        }
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        let mut internal = self.internal.borrow_mut();
        let content = &mut internal.content;

        // Check if index is valid
        if index >= content.len() {
            return false;
        }

        // Remove character at index
        let (before, after) = content.split_at(index);
        let after = &after[1..]; // Skip the character we're removing
        *content = format!("{}{}", before, after);
        true
    }
}
