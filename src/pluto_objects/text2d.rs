use crate::TextureSVG;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
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
    z: i32,
    color: [f32; 4], // RGBA
    auto_size_enabled: bool,
    wrap_enabled: bool,
    min_font_size: f32,
    max_font_size: f32,
    cached_font_size: Option<f32>,
    cached_wrapped_text: Option<String>,
    last_content_hash: Option<u64>,
    last_container_dims: Option<(f32, f32)>,
    last_dpi_scale_factor: Option<f32>,
    last_font_cache_version: u32,
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
            z: 0,
            color: [1.0, 1.0, 1.0, 1.0], // Default to white
            auto_size_enabled: false,
            wrap_enabled: false,
            min_font_size: 8.0,
            max_font_size: 128.0,
            cached_font_size: None,
            cached_wrapped_text: None,
            last_content_hash: None,
            last_container_dims: None,
            last_dpi_scale_factor: None,
            last_font_cache_version: 0,
        }
    }

    pub fn set_dimensions(&mut self, dimensions: Rectangle) {
        self.dimensions = dimensions;
        // Update container bounds to match new dimensions if using default container
        if self.container.get_dimensions() == self.dimensions {
            self.container.set_dimensions(dimensions);
        }
        self.last_container_dims = None;
        self.cached_font_size = None;
        self.cached_wrapped_text = None;
    }
    pub fn get_render_position(&self, text_width: f32) -> Position {
        self.container
            .calculate_text_position(text_width, self.font_size)
    }

    pub fn set_container(&mut self, container: TextContainer) {
        self.container = container;
        self.last_container_dims = None;
        self.cached_font_size = None;
        self.cached_wrapped_text = None;
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
        _current_line: usize,
    ) -> Position {
        let line_height = self.font_size * 1.2;

        let mut idx = char_index.min(self.content.len());
        while idx > 0 && !self.content.is_char_boundary(idx) {
            idx -= 1;
        }
        let text_prefix = &self.content[..idx];
        let derived_line = text_prefix.chars().filter(|&ch| ch == '\n').count();
        let current_line_text = text_prefix.rsplit('\n').next().unwrap_or("");
        let line_width = text_renderer.measure_caret_advance(
            current_line_text,
            &self.font_key,
            self.font_size,
            0.0,
            0.0,
        );

        Position {
            x: self.dimensions.x + line_width,
            y: self.dimensions.y + derived_line as f32 * line_height,
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

        let line_width = text_renderer.measure_caret_advance(
            line_content,
            &self.font_key,
            self.font_size,
            0.0,
            0.0,
        );

        // Handle click beyond line end
        if relative_x >= line_width {
            return (index_offset + line_content.len(), clicked_line);
        }

        // Measure each character position in the current line
        let mut prev_width = 0.0;
        for (idx, ch) in line_content.char_indices() {
            let next_idx = idx + ch.len_utf8();
            let substr = &line_content[..next_idx];
            let width = text_renderer.measure_caret_advance(
                substr,
                &self.font_key,
                self.font_size,
                0.0,
                0.0,
            );

            // Find the midpoint between current and previous character
            let char_midpoint = (width + prev_width) / 2.0;

            // If click position is before the midpoint, place cursor before current char
            if relative_x < char_midpoint {
                return (index_offset + idx, clicked_line);
            }

            // If this is the last character and we haven't returned yet, cursor goes after it
            if next_idx == line_content.len() {
                return (index_offset + next_idx, clicked_line);
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
        self.last_content_hash = None;
        self.cached_font_size = None;
        self.cached_wrapped_text = None;
    }

    pub fn append_content(&mut self, new_content: &str) {
        self.content.push_str(new_content);
        self.last_content_hash = None;
        self.cached_font_size = None;
        self.cached_wrapped_text = None;
    }

    pub fn pop_content(&mut self) -> bool {
        if !self.content.is_empty() {
            self.content.pop();
            self.last_content_hash = None;
            self.cached_font_size = None;
            self.cached_wrapped_text = None;
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

    pub fn set_z(&mut self, z: i32) {
        self.z = z;
    }

    pub fn get_z(&self) -> i32 {
        self.z
    }

    pub fn set_color(&mut self, color: [f32; 4]) {
        self.color = color;
    }

    pub fn get_color(&self) -> [f32; 4] {
        self.color
    }

    fn wrap_text_to_lines(
        &self,
        text: &str,
        font_size: f32,
        available_width: f32,
        text_renderer: &TextRenderer,
    ) -> String {
        if text.is_empty() || available_width <= 0.0 {
            return String::new();
        }

        let mut result = Vec::new();

        // Split by existing newlines to preserve manual line breaks
        for segment in text.split('\n') {
            if segment.is_empty() {
                result.push(String::new());
                continue;
            }

            let words: Vec<&str> = segment.split_whitespace().collect();
            if words.is_empty() {
                result.push(String::new());
                continue;
            }

            let mut current_line = String::new();

            for word in words {
                let test_line = if current_line.is_empty() {
                    word.to_string()
                } else {
                    format!("{} {}", current_line, word)
                };

                // Measure the test line
                let (line_width, _) = text_renderer.measure_text(
                    &test_line,
                    &self.font_key,
                    0.0,
                    0.0,
                    Some(font_size),
                );

                let scaled_width = line_width;

                if scaled_width <= available_width {
                    current_line = test_line;
                } else {
                    // Line would be too long, start new line
                    if !current_line.is_empty() {
                        result.push(current_line);
                        current_line = word.to_string();
                    } else {
                        // Single word is too long, keep it anyway
                        result.push(word.to_string());
                    }
                }
            }

            // Don't forget the last line
            if !current_line.is_empty() {
                result.push(current_line);
            }
        }

        result.join("\n")
    }

    fn calculate_fitted_font_size(
        &self,
        text: &str,
        available_width: f32,
        available_height: f32,
        text_renderer: &TextRenderer,
    ) -> f32 {
        if text.is_empty() || available_width <= 0.0 || available_height <= 0.0 {
            return self.font_size;
        }

        // Helper function to check if text fits at a given font size
        let text_fits = |font_size: f32| -> bool {
            let text_to_measure = if self.wrap_enabled {
                self.wrap_text_to_lines(text, font_size, available_width, text_renderer)
            } else {
                text.to_string()
            };

            let (text_width, line_count) = text_renderer.measure_text(
                &text_to_measure,
                &self.font_key,
                0.0,
                0.0,
                Some(font_size),
            );

            let scaled_width = text_width;
            let text_height = font_size * line_count as f32 * self.container.line_height_mul;

            scaled_width <= available_width && text_height <= available_height
        };

        // Binary search to find the largest font size that fits
        let mut low = self.min_font_size;
        let mut high = self.max_font_size;

        // Quick optimization: if even max size fits, return it
        if text_fits(high) {
            return high;
        }

        // Quick check: if min size doesn't fit, return it anyway
        if !text_fits(low) {
            return low;
        }

        // Binary search when difference is large (> 4px)
        if high - low > 4.0 {
            while high - low > 1.0 {
                let mid = (low + high) / 2.0;
                if text_fits(mid) {
                    low = mid;
                } else {
                    high = mid;
                }
            }
        }

        // Linear refinement - find the largest size that fits
        let mut size = high.floor();
        while size >= self.min_font_size {
            if text_fits(size) {
                return size;
            }
            size -= 1.0;
        }

        // Return minimum size even if text doesn't fit
        self.min_font_size
    }

    fn needs_recalculation(&self, dpi_scale_factor: f32, font_cache_version: u32) -> bool {
        if !self.auto_size_enabled && !self.wrap_enabled {
            // Even if auto-size/wrap is off, we might need to re-layout if DPI changed
            // because measure_text results might change or font atlases might be rebuilt.
            return self.last_dpi_scale_factor != Some(dpi_scale_factor)
                || self.last_font_cache_version != font_cache_version;
        }

        let mut hasher = DefaultHasher::new();
        self.content.hash(&mut hasher);
        let content_hash = hasher.finish();

        let dims = (
            self.container.dimensions.width,
            self.container.dimensions.height,
        );

        self.last_content_hash != Some(content_hash)
            || self.last_container_dims != Some(dims)
            || self.last_dpi_scale_factor != Some(dpi_scale_factor)
            || self.last_font_cache_version != font_cache_version
    }

    pub fn render_with_z(&self, engine: &mut PlutoniumEngine, z: i32) {
        // Compute values on-the-fly if cache is empty
        let available_width = self.container.dimensions.width - (self.container.padding * 2.0);
        let available_height = self.container.dimensions.height - (self.container.padding * 2.0);

        // Determine font size to use (from cache or compute on-the-fly)
        let font_size_to_use = if self.auto_size_enabled {
            if let Some(cached) = self.cached_font_size {
                cached
            } else {
                // Compute on-the-fly if cache is empty
                self.calculate_fitted_font_size(
                    &self.content,
                    available_width,
                    available_height,
                    &engine.text_renderer,
                )
            }
        } else {
            self.font_size
        };

        // Determine text to render (from cache or compute on-the-fly)
        let text_to_render_owned: Option<String> = if self.wrap_enabled {
            if self.cached_wrapped_text.is_some() {
                None // Use cached version
            } else {
                // Compute on-the-fly if cache is empty
                Some(self.wrap_text_to_lines(
                    &self.content,
                    font_size_to_use,
                    available_width,
                    &engine.text_renderer,
                ))
            }
        } else {
            None
        };

        let text_to_render: &str = if let Some(ref owned) = text_to_render_owned {
            owned
        } else if self.wrap_enabled {
            self.cached_wrapped_text.as_ref().unwrap_or(&self.content)
        } else {
            &self.content
        };

        // Measure using processed text
        let (text_width, line_count) = engine.text_renderer.measure_text(
            text_to_render,
            &self.font_key,
            0.0,
            0.0,
            Some(font_size_to_use),
        );

        let scaled_width = text_width;
        let text_height = font_size_to_use * line_count as f32;

        // Compute a single alignment via container, then render with a neutral container
        let container_pos = self
            .container
            .calculate_text_position(scaled_width, text_height);

        // Avoid applying alignment twice: provide a neutral container for layout
        let mut neutral = self.container.clone();
        neutral.h_align = HorizontalAlignment::Left;
        neutral.v_align = VerticalAlignment::Top;

        // Always honor the resolved font size when laying out glyphs.
        let font_override = Some(font_size_to_use);

        engine.queue_text_with_spacing(
            text_to_render,
            &self.font_key,
            container_pos,
            &neutral,
            0.0,
            0.0,
            z,
            self.color,
            font_override,
        );
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
        update_context: Option<UpdateContext>,
        dpi_scale_factor: f32,
        text_renderer: &TextRenderer,
    ) {
        let font_cache_version = update_context
            .as_ref()
            .map(|c| c.font_cache_version)
            .unwrap_or(0);

        // Check if recalculation is needed
        let needs_recalc = self.needs_recalculation(dpi_scale_factor, font_cache_version);

        if needs_recalc {
            let available_width = self.container.dimensions.width - (self.container.padding * 2.0);
            let available_height =
                self.container.dimensions.height - (self.container.padding * 2.0);

            // Step 1: Determine font size to use
            let font_size_to_use = if self.auto_size_enabled {
                let fitted = self.calculate_fitted_font_size(
                    &self.content,
                    available_width,
                    available_height,
                    text_renderer,
                );
                self.cached_font_size = Some(fitted);
                fitted
            } else {
                self.font_size
            };

            // Step 2: Apply wrapping if enabled
            let text_to_measure = if self.wrap_enabled {
                let wrapped = self.wrap_text_to_lines(
                    &self.content,
                    font_size_to_use,
                    available_width,
                    text_renderer,
                );
                self.cached_wrapped_text = Some(wrapped.clone());
                wrapped
            } else {
                self.content.clone()
            };

            // Step 3: Measure final dimensions
            let (text_width, line_count) = text_renderer.measure_text(
                &text_to_measure,
                &self.font_key,
                0.0,
                0.0,
                Some(font_size_to_use),
            );

            let scaled_width = text_width;

            self.dimensions.width = scaled_width;
            self.dimensions.height = font_size_to_use * line_count as f32;

            // Update cache tracking
            let mut hasher = DefaultHasher::new();
            self.content.hash(&mut hasher);
            self.last_content_hash = Some(hasher.finish());
            self.last_container_dims = Some((
                self.container.dimensions.width,
                self.container.dimensions.height,
            ));
            self.last_dpi_scale_factor = Some(dpi_scale_factor);
            self.last_font_cache_version = font_cache_version;
        } else {
            // No recalculation needed, but still update dimensions if not using adaptive features
            if !self.auto_size_enabled && !self.wrap_enabled {
                let (text_width, line_count) = text_renderer.measure_text(
                    &self.content,
                    &self.font_key,
                    0.0,
                    0.0,
                    Some(self.font_size),
                );
                self.dimensions.width = text_width;
                self.dimensions.height = self.font_size * line_count as f32;
            }
        }
    }

    fn render(&self, engine: &mut PlutoniumEngine) {
        self.render_with_z(engine, self.z);
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

    pub fn get_font_key(&self) -> String {
        self.internal.borrow().font_key.clone()
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

    pub fn render_with_z(&self, engine: &mut PlutoniumEngine, z: i32) {
        self.internal.borrow().render_with_z(engine, z);
    }

    pub fn set_z(&self, z: i32) {
        self.internal.borrow_mut().set_z(z);
    }

    pub fn get_z(&self) -> i32 {
        self.internal.borrow().get_z()
    }

    pub fn with_z(self, z: i32) -> Self {
        self.set_z(z);
        self
    }

    pub fn set_color(&self, color: [f32; 4]) {
        self.internal.borrow_mut().set_color(color);
    }

    pub fn get_color(&self) -> [f32; 4] {
        self.internal.borrow().get_color()
    }

    pub fn with_color(self, color: [f32; 4]) -> Self {
        self.set_color(color);
        self
    }

    /// Enable or disable automatic font sizing.
    /// When enabled, the font size will be automatically adjusted to fit the container,
    /// searching between min_font_size and max_font_size to find the largest size that fits.
    pub fn with_auto_size(self, enabled: bool) -> Self {
        self.internal.borrow_mut().auto_size_enabled = enabled;
        self.internal.borrow_mut().cached_font_size = None; // Invalidate cache
        self
    }

    /// Enable or disable text wrapping.
    /// When enabled, text will wrap to multiple lines to fit the container width.
    pub fn with_wrap(self, enabled: bool) -> Self {
        self.internal.borrow_mut().wrap_enabled = enabled;
        self.internal.borrow_mut().cached_wrapped_text = None; // Invalidate cache
        self
    }

    /// Set the minimum font size for auto-sizing.
    /// When auto-sizing is enabled, the font will never be smaller than this value.
    /// Default: 8.0
    pub fn with_min_font_size(self, size: f32) -> Self {
        self.internal.borrow_mut().min_font_size = size;
        self
    }

    /// Set the maximum font size for auto-sizing.
    /// When auto-sizing is enabled, the font will never be larger than this value.
    /// This allows text to grow to fill available space.
    /// Default: 128.0
    pub fn with_max_font_size(self, size: f32) -> Self {
        self.internal.borrow_mut().max_font_size = size;
        self
    }

    pub fn set_auto_size(&self, enabled: bool) {
        self.internal.borrow_mut().auto_size_enabled = enabled;
        self.internal.borrow_mut().cached_font_size = None; // Invalidate cache
    }

    pub fn set_wrap(&self, enabled: bool) {
        self.internal.borrow_mut().wrap_enabled = enabled;
        self.internal.borrow_mut().cached_wrapped_text = None; // Invalidate cache
    }

    pub fn set_min_font_size(&self, size: f32) {
        self.internal.borrow_mut().min_font_size = size;
    }

    pub fn set_max_font_size(&self, size: f32) {
        self.internal.borrow_mut().max_font_size = size;
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
        if !content.is_char_boundary(index) {
            return;
        }

        // Handle different insertion cases
        if content.is_empty() || index == content.len() {
            content.push_str(c);
        } else {
            content.insert_str(index, c);
        }

        internal.last_content_hash = None;
        internal.cached_font_size = None;
        internal.cached_wrapped_text = None;
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        let mut internal = self.internal.borrow_mut();
        let content = &mut internal.content;

        // Check if index is valid
        if index >= content.len() {
            return false;
        }
        if !content.is_char_boundary(index) {
            return false;
        }

        let next_index = content[index..]
            .chars()
            .next()
            .map(|ch| index + ch.len_utf8())
            .unwrap_or(index);

        if next_index == index {
            return false;
        }

        content.replace_range(index..next_index, "");
        internal.last_content_hash = None;
        internal.cached_font_size = None;
        internal.cached_wrapped_text = None;
        true
    }
}
