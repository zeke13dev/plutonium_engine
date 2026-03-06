#![forbid(unsafe_code)]

use crate::immediate::drag_drop::DragDropState;
use crate::immediate::draw_list::DrawList;
use crate::immediate::focus::FocusManager;
use crate::immediate::input::{InputStateExt, UiInputState};
use crate::immediate::input_map::{Action, InputMap};
use crate::immediate::layout::{LayoutDirection, LayoutEngine};
use crate::immediate::painter::Painter;
use crate::immediate::response::Response;
use crate::immediate::state::StateCache;
use crate::immediate::tooltip::{TooltipContent, TooltipManager};
use crate::immediate::types::{
    rect_from_center_size, rect_from_min_max, vec2, Color, RectExt, UiRect, UiVec2, WidgetId,
};
use plutonium_engine::{HaloStyle, PlutoniumEngine};
use std::hash::{Hash, Hasher};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UiStyle {
    pub spacing: f32,
    pub button_padding: f32,
    pub panel_padding: f32,
    pub text_color: Color,
    pub button_color: Color,
    pub button_hover_color: Color,
    pub widget_bg: Color,
    pub accent_color: Color,
    pub panel_color: Color,
    pub font_key: String,
    pub font_size: f32,
    pub rounding: f32,
}

impl Default for UiStyle {
    fn default() -> Self {
        UiStyle {
            spacing: 4.0,
            button_padding: 8.0,
            panel_padding: 8.0,
            text_color: Color::WHITE,
            button_color: Color::rgb(60, 60, 70),
            button_hover_color: Color::rgb(80, 80, 90),
            widget_bg: Color::rgb(45, 45, 55),
            accent_color: Color::rgb(100, 150, 255),
            panel_color: Color::rgb(30, 30, 35),
            font_key: "roboto".to_string(),
            font_size: 16.0,
            rounding: 4.0,
        }
    }
}

pub struct UIContext {
    input: UiInputState,
    layout: LayoutEngine,
    painter: DrawList,
    pub style: UiStyle,
    style_stack: Vec<UiStyle>,
    input_map: InputMap,
    focus: FocusManager,
    state_cache: StateCache,
    drag_drop: DragDropState,
    tooltip_manager: TooltipManager,
    pending_tooltips: Vec<(WidgetId, String, UiVec2)>,
    drag_active: Option<WidgetId>,
    prev_pointer: UiVec2,
    pointer: UiVec2,
    time_seconds: f32,
    double_click_max_delay: f32,
    last_click_time: Option<f32>,
    last_click_id: Option<WidgetId>,
    paint_offset: UiVec2,
    scroll_offsets: std::collections::HashMap<WidgetId, f32>,
    id_counter: u64,
    id_stack: Vec<WidgetId>,
}

impl UIContext {
    pub fn new(screen_rect: UiRect) -> Self {
        UIContext {
            input: UiInputState::default(),
            layout: LayoutEngine::new(screen_rect),
            painter: DrawList::new(),
            style: UiStyle::default(),
            style_stack: Vec::new(),
            input_map: InputMap::default_bindings(),
            focus: FocusManager::new(),
            state_cache: StateCache::new(),
            drag_drop: DragDropState::new(),
            tooltip_manager: TooltipManager::new(),
            pending_tooltips: Vec::new(),
            drag_active: None,
            prev_pointer: vec2(0.0, 0.0),
            pointer: vec2(0.0, 0.0),
            time_seconds: 0.0,
            double_click_max_delay: 0.3,
            last_click_time: None,
            last_click_id: None,
            paint_offset: vec2(0.0, 0.0),
            scroll_offsets: std::collections::HashMap::new(),
            id_counter: 0,
            id_stack: Vec::new(),
        }
    }

    pub fn begin_frame(&mut self, input: UiInputState, screen_rect: UiRect) {
        self.input = input;
        self.id_counter = 0;
        self.layout = LayoutEngine::new(screen_rect);
        self.layout.set_spacing(self.style.spacing);
        self.painter.clear();
        self.prev_pointer = self.pointer;
        self.pointer = self.input.pointer_pos();
        self.tooltip_manager.begin_frame(self.time_seconds);
        self.pending_tooltips.clear();
        if self.drag_drop.is_dragging() {
            self.drag_drop.update_drag_position(self.pointer);
            if self.input.is_just_pressed("Escape") {
                self.drag_drop.cancel_drag();
            }
        }
        self.focus.begin_frame();
        let next_tab = self
            .input_map
            .is_action_just_pressed(Action::NextTab, &self.input);
        let prev_tab = self
            .input_map
            .is_action_just_pressed(Action::PrevTab, &self.input);
        if prev_tab {
            self.focus.request_focus_prev();
        } else if next_tab {
            if self.input.is_pressed("ShiftLeft") || self.input.is_pressed("ShiftRight") {
                self.focus.request_focus_prev();
            } else {
                self.focus.request_focus_next();
            }
        }
        self.state_cache.set_time(self.time_seconds);
    }

    pub fn end_frame(&mut self) {
        self.focus.end_frame();
        if self.drag_drop.is_dragging() && self.input.lmb_just_released {
            self.drag_drop.cancel_drag();
        }
        self.render_drag_preview();
        for (id, text, pos) in &self.pending_tooltips {
            self.tooltip_manager
                .set_tooltip(*id, TooltipContent::Text(text.clone()), *pos);
        }
        self.tooltip_manager.end_frame();
        self.render_tooltips();
        self.state_cache.cleanup_old_state(60.0);
    }

    pub fn set_time_seconds(&mut self, time_seconds: f32) {
        self.time_seconds = time_seconds;
        self.state_cache.set_time(time_seconds);
        self.tooltip_manager.set_time(time_seconds);
    }

    pub fn set_double_click_max_delay(&mut self, seconds: f32) {
        self.double_click_max_delay = seconds.max(0.0);
    }

    pub fn input_map_mut(&mut self) -> &mut InputMap {
        &mut self.input_map
    }

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.painter.render(engine);
    }

    /// Push a style override onto the stack
    pub fn push_style(&mut self, style: UiStyle) {
        self.style_stack.push(self.style.clone());
        self.style = style;
    }

    /// Pop the most recent style override
    pub fn pop_style(&mut self) {
        if let Some(prev) = self.style_stack.pop() {
            self.style = prev;
        }
    }

    /// Temporarily override style for a scope
    pub fn with_style<R>(&mut self, style: UiStyle, f: impl FnOnce(&mut Self) -> R) -> R {
        self.push_style(style);
        let result = f(self);
        self.pop_style();
        result
    }

    /// Override a single color temporarily
    pub fn with_color<R>(
        &mut self,
        setter: impl Fn(&mut UiStyle, Color),
        color: Color,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let mut new_style = self.style.clone();
        setter(&mut new_style, color);
        self.with_style(new_style, f)
    }

    /// Convenience: override text color
    pub fn with_text_color<R>(&mut self, color: Color, f: impl FnOnce(&mut Self) -> R) -> R {
        self.with_color(|style, c| style.text_color = c, color, f)
    }

    /// Convenience: override accent color
    pub fn with_accent_color<R>(&mut self, color: Color, f: impl FnOnce(&mut Self) -> R) -> R {
        self.with_color(|style, c| style.accent_color = c, color, f)
    }

    /// Convenience: override font size
    pub fn with_font_size<R>(&mut self, size: f32, f: impl FnOnce(&mut Self) -> R) -> R {
        let mut new_style = self.style.clone();
        new_style.font_size = size;
        self.with_style(new_style, f)
    }

    /// Register tooltip text for a response
    pub fn process_response(&mut self, response: &Response) {
        if let Some(text) = response.tooltip_text() {
            if response.hovered {
                self.pending_tooltips.push((
                    response.id,
                    text.to_string(),
                    vec2(response.rect.center().x, response.rect.top()),
                ));
            }
        }
    }

    pub fn set_tooltip_delay(&mut self, seconds: f32) {
        self.tooltip_manager.set_delay(seconds);
    }

    /// Draw a configurable halo around an arbitrary UI rectangle.
    pub fn halo_rect(&mut self, rect: UiRect, style: HaloStyle) {
        let ring_count = style.ring_count.max(1) as usize;
        let radius = style.radius.max(0.0);
        if radius <= f32::EPSILON {
            return;
        }

        let step = radius / ring_count as f32;
        let base_thickness = step.max(1.0);
        let inner_padding = style.inner_padding.max(0.0);
        let color = Color {
            r: style.color[0].clamp(0.0, 1.0),
            g: style.color[1].clamp(0.0, 1.0),
            b: style.color[2].clamp(0.0, 1.0),
            a: 1.0,
        };

        for ring_idx in 0..ring_count {
            let t = (ring_idx as f32 + 1.0) / ring_count as f32;
            let expansion = inner_padding + radius * t;
            let ring_rect = rect.expand(expansion);
            let alpha = style.alpha_at(t);
            if alpha <= 0.0 {
                continue;
            }
            let thickness = base_thickness * (1.0 + (1.0 - t) * 0.35);
            self.paint_rect_outline_rounded(
                ring_rect,
                color.with_alpha(alpha),
                thickness,
                (style.corner_radius + expansion).max(0.0),
            );
        }
    }

    /// Draw a configurable halo around a widget response bounds.
    pub fn halo_response(&mut self, response: &Response, style: HaloStyle) {
        self.halo_rect(response.rect, style);
    }

    /// Get persistent state for a widget
    pub fn get_state<T: 'static + Clone>(&mut self, id: WidgetId) -> Option<T> {
        let value = self.state_cache.get(id);
        if value.is_some() {
            self.state_cache.touch(id);
        }
        value
    }

    /// Set persistent state for a widget
    pub fn set_state<T: 'static + Clone>(&mut self, id: WidgetId, value: T) {
        self.state_cache.set(id, value);
    }

    /// Convenience: get or insert default state
    pub fn get_state_or<T: 'static + Clone>(&mut self, id: WidgetId, default: T) -> T {
        self.get_state(id).unwrap_or_else(|| {
            self.set_state(id, default.clone());
            default
        })
    }

    fn next_id(&mut self) -> WidgetId {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for id in &self.id_stack {
            id.hash(&mut hasher);
        }
        self.id_counter.hash(&mut hasher);
        let id = WidgetId::new(hasher.finish());
        self.id_counter += 1;
        id
    }

    pub fn push_id(&mut self, id: impl Hash) {
        let widget_id = WidgetId::from_hash(id);
        self.id_stack.push(widget_id);
    }

    pub fn pop_id(&mut self) {
        self.id_stack.pop();
    }

    pub fn label(&mut self, text: impl Into<String>) -> Response {
        let id = self.next_id();
        let text = text.into();
        let font_key = self.style.font_key.clone();
        let font_size = self.style.font_size;
        let text_color = self.style.text_color;
        let size = self.painter.measure_text(&text, &font_key, font_size);
        let rect = self.layout.allocate(size);
        self.paint_text(rect.min(), &text, text_color, &font_key, font_size);
        Response::new(id, rect)
    }

    pub fn button(&mut self, text: impl Into<String>) -> Response {
        let id = self.next_id();
        let text = text.into();
        let font_key = self.style.font_key.clone();
        let font_size = self.style.font_size;
        let text_color = self.style.text_color;
        let text_size = self.painter.measure_text(&text, &font_key, font_size);
        let size = vec2(
            text_size.x + self.style.button_padding * 2.0,
            text_size.y + self.style.button_padding * 2.0,
        );
        let rect = self.layout.allocate(size);
        let response = self.interact(id, rect);
        let bg = if response.hovered {
            self.style.button_hover_color
        } else {
            self.style.button_color
        };
        self.paint_rect(rect, bg, self.style.rounding);
        self.paint_text_centered(rect, &text, text_color, &font_key, font_size);
        response
    }

    pub fn checkbox(&mut self, checked: &mut bool, label: impl Into<String>) -> Response {
        let id = self.next_id();
        let label = label.into();
        let font_key = self.style.font_key.clone();
        let font_size = self.style.font_size;
        let text_color = self.style.text_color;
        let box_size = 20.0;
        let spacing = 8.0;
        let text_size = self.painter.measure_text(&label, &font_key, font_size);
        let total_size = vec2(box_size + spacing + text_size.x, box_size.max(text_size.y));
        let rect = self.layout.allocate(total_size);
        let response = self.interact(id, rect);
        if response.clicked() {
            *checked = !*checked;
        }

        let box_rect = UiRect::new(rect.x, rect.y, box_size, box_size);
        self.painter.rect(
            self.offset_rect(box_rect),
            self.style.widget_bg,
            self.style.rounding,
        );
        if *checked {
            let inner = box_rect.shrink(4.0);
            self.paint_rect(inner, self.style.accent_color, 2.0);
        }

        let text_pos = vec2(
            rect.x + box_size + spacing,
            rect.y + (rect.height() - text_size.y) * 0.5,
        );
        self.paint_text(text_pos, &label, text_color, &font_key, font_size);

        response
    }

    pub fn slider(&mut self, value: &mut f32, range: (f32, f32)) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate(vec2(200.0, 20.0));
        let response = self.interact(id, rect);

        if response.dragging || response.clicked {
            let t = ((self.pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *value = range.0 + t * (range.1 - range.0);
        }
        if response.focused {
            let step = (range.1 - range.0) * 0.05;
            if self
                .input_map
                .is_action_just_pressed(Action::MenuLeft, &self.input)
                || self
                    .input_map
                    .is_action_just_pressed(Action::MenuDown, &self.input)
            {
                *value = (*value - step).clamp(range.0, range.1);
            }
            if self
                .input_map
                .is_action_just_pressed(Action::MenuRight, &self.input)
                || self
                    .input_map
                    .is_action_just_pressed(Action::MenuUp, &self.input)
            {
                *value = (*value + step).clamp(range.0, range.1);
            }
        }

        self.paint_rect(rect, self.style.widget_bg, rect.height() * 0.5);

        let t = (*value - range.0) / (range.1 - range.0);
        let filled_rect = UiRect::new(rect.x, rect.y, rect.width() * t, rect.height());
        self.paint_rect(filled_rect, self.style.accent_color, rect.height() * 0.5);

        let handle_size = rect.height();
        let handle_x = rect.left() + rect.width() * t;
        let handle_rect = UiRect::new(
            handle_x - handle_size * 0.5,
            rect.y,
            handle_size,
            rect.height(),
        );
        let handle_color = if response.dragging {
            self.style.accent_color.lighten(0.15)
        } else if response.hovered {
            self.style.accent_color.lighten(0.08)
        } else {
            self.style.accent_color
        };
        self.paint_rect(handle_rect, handle_color, rect.height() * 0.5);

        response
    }

    /// Single-line text input field
    pub fn text_input(&mut self, text: &mut String) -> Response {
        self.text_input_with_hint(text, "")
    }

    /// Text input with placeholder hint
    pub fn text_input_with_hint(&mut self, text: &mut String, hint: &str) -> Response {
        let id = self.next_id();
        let size = vec2(200.0, 24.0);
        let rect = self.layout.allocate(size);
        let response = self.interact(id, rect);

        if response.clicked() {
            self.focus.request_focus(id);
        }

        let bg_color = if response.focused {
            self.style.widget_bg.lighten(0.1)
        } else if response.hovered {
            self.style.widget_bg.lighten(0.05)
        } else {
            self.style.widget_bg
        };
        self.paint_rect(rect, bg_color, 4.0);

        if response.focused {
            self.paint_rect_outline(rect, self.style.accent_color, 2.0);
        }

        if response.focused {
            if self.input.is_just_pressed("Backspace") {
                text.pop();
            }
            if self.input.is_just_pressed("Delete") {
                text.pop();
            }
            if self.input.is_just_pressed("Escape") {
                self.focus.clear_focus();
            }
            if self.input.is_just_pressed("Enter") {
                self.focus.clear_focus();
            }
            for ch in self.input.text_input_chars() {
                if !ch.is_control() {
                    text.push(ch);
                }
            }
        }

        let display_text = if text.is_empty() && !response.focused {
            hint
        } else {
            text.as_str()
        };

        let text_color = if text.is_empty() && !response.focused {
            self.style.text_color.with_alpha(0.5)
        } else {
            self.style.text_color
        };

        let mut display = display_text.to_string();
        if response.focused {
            let blink = ((self.time_seconds * 2.0) % 1.0) < 0.5;
            if blink {
                display.push('|');
            }
        }

        let text_pos = vec2(rect.x + 6.0, rect.y + 4.0);
        let font_key = self.style.font_key.clone();
        let font_size = self.style.font_size;
        self.paint_text(text_pos, &display, text_color, &font_key, font_size);

        response
    }

    /// Progress bar with default size.
    pub fn progress_bar(&mut self, progress: f32) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate(vec2(200.0, 20.0));
        let clamped = progress.clamp(0.0, 1.0);
        self.paint_rect(rect, self.style.widget_bg, self.style.rounding);
        let filled = UiRect::new(rect.x, rect.y, rect.width() * clamped, rect.height());
        self.paint_rect(filled, self.style.accent_color, self.style.rounding);
        Response::new(id, rect)
    }

    /// Progress bar with custom size.
    pub fn progress_bar_sized(&mut self, progress: f32, size: UiVec2) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate(size);
        let clamped = progress.clamp(0.0, 1.0);
        let rounding = size.y * 0.5;
        self.paint_rect(rect, self.style.widget_bg, rounding);
        let filled = UiRect::new(rect.x, rect.y, rect.width() * clamped, rect.height());
        self.paint_rect(filled, self.style.accent_color, rounding);
        Response::new(id, rect)
    }

    /// Progress bar with custom fill color.
    pub fn progress_bar_colored(&mut self, progress: f32, size: UiVec2, color: Color) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate(size);
        let clamped = progress.clamp(0.0, 1.0);
        let rounding = size.y * 0.5;
        self.paint_rect(rect, self.style.widget_bg, rounding);
        let filled = UiRect::new(rect.x, rect.y, rect.width() * clamped, rect.height());
        self.paint_rect(filled, color, rounding);
        Response::new(id, rect)
    }

    /// Progress bar with centered label.
    pub fn progress_bar_labeled(&mut self, progress: f32, label: impl Into<String>) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate(vec2(200.0, 24.0));
        let label = label.into();
        let clamped = progress.clamp(0.0, 1.0);
        self.paint_rect(rect, self.style.widget_bg, self.style.rounding);
        let filled = UiRect::new(rect.x, rect.y, rect.width() * clamped, rect.height());
        self.paint_rect(filled, self.style.accent_color, self.style.rounding);
        let font_key = self.style.font_key.clone();
        let font_size = self.style.font_size;
        let text_color = self.style.text_color;
        self.paint_text_centered(rect, &label, text_color, &font_key, font_size);
        Response::new(id, rect)
    }

    /// Display an image/texture.
    pub fn image(&mut self, texture: Uuid, size: UiVec2) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate(size);
        let response = self.interact(id, rect);
        self.paint_image(texture, rect.min(), size);
        response
    }

    /// Display an image/texture with a tint.
    pub fn image_tinted(&mut self, texture: Uuid, size: UiVec2, tint: Color) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate(size);
        let response = self.interact(id, rect);
        self.paint_image_tinted(texture, rect.min(), size, tint);
        response
    }

    /// Display a clickable image button.
    pub fn image_button(&mut self, texture: Uuid, size: UiVec2) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate(size);
        let response = self.interact(id, rect);

        let tint = if response.hovered {
            Color::WHITE.lighten(0.2)
        } else {
            Color::WHITE
        };

        self.paint_image_tinted(texture, rect.min(), size, tint);
        response
    }

    /// Begin a drag operation with typed data
    /// Begin a drag operation with typed data.
    pub fn drag_source<T: 'static>(
        &mut self,
        id: impl Hash,
        data: T,
        content: impl FnOnce(&mut Self),
    ) -> Response {
        let widget_id = WidgetId::from_hash(&id);

        let start_cursor = self.layout.cursor();
        content(self);
        let end_cursor = self.layout.cursor();
        let rect = rect_from_min_max(start_cursor, end_cursor);

        let response = self.interact(widget_id, rect);

        if response.drag_started {
            self.drag_drop.start_drag(widget_id, data, self.pointer);
        }

        response
    }

    /// Drop target that receives dragged data
    /// Drop target that receives dragged data.
    pub fn drop_target<T: 'static>(
        &mut self,
        content: impl FnOnce(&mut Self),
    ) -> (Response, Option<T>) {
        let id = self.next_id();

        let start_cursor = self.layout.cursor();
        content(self);
        let end_cursor = self.layout.cursor();
        let rect = rect_from_min_max(start_cursor, end_cursor);

        let response = self.interact(id, rect);

        let dropped = if response.hovered
            && self.drag_drop.is_dragging_type::<T>()
            && self.input.lmb_just_released
        {
            self.drag_drop.end_drag().and_then(|data| data.take::<T>())
        } else {
            None
        };

        if response.hovered && self.drag_drop.is_dragging_type::<T>() {
            self.paint_rect_outline(rect, self.style.accent_color, 2.0);
        }

        (response, dropped)
    }

    /// Check if currently dragging specific type
    /// Check if currently dragging specific type.
    pub fn is_dragging_type<T: 'static>(&self) -> bool {
        self.drag_drop.is_dragging_type::<T>()
    }

    /// Render drag preview (call in end_frame)
    /// Render drag preview (called in end_frame).
    pub fn render_drag_preview(&mut self) {
        if !self.drag_drop.is_dragging() {
            return;
        }

        let pos = self.drag_drop.drag_position();
        let preview_rect = UiRect::new(pos.x + 10.0, pos.y + 10.0, 120.0, 40.0);
        let bg = self.style.panel_color.with_alpha(0.9);
        self.paint_rect(preview_rect, bg, 4.0);

        let label = self
            .drag_drop
            .payload_preview()
            .unwrap_or("Dragging...")
            .to_string();
        let font_key = self.style.font_key.clone();
        let font_size = self.style.font_size;
        let text_color = self.style.text_color;
        self.paint_text_centered(preview_rect, &label, text_color, &font_key, font_size);
    }

    pub fn panel(&mut self, content: impl FnOnce(&mut Self)) -> Response {
        let id = self.next_id();
        let start = self.layout.cursor();
        let insert_at = self.painter.len();
        content(self);
        let end = self.layout.cursor();
        let rect = rect_from_min_max(start, end).expand(self.style.panel_padding);
        self.painter.insert_rect(
            insert_at,
            self.offset_rect(rect),
            self.style.panel_color,
            self.style.rounding,
        );
        Response::new(id, rect)
    }

    pub fn scroll_area(&mut self, content: impl FnOnce(&mut Self)) -> Response {
        let id = self.next_id();
        let rect = self.layout.allocate_remaining();
        let offset = self.scroll_offsets.get(&id).copied().unwrap_or(0.0);
        let response = self.interact(id, rect);
        if response.hovered() {
            let mut new_offset = offset;
            if self.input.scroll_delta_y.abs() > 0.0 {
                new_offset = (new_offset - self.input.scroll_delta_y).max(0.0);
            }
            let scroll_step = 40.0;
            if self.input.is_pressed("PageDown") {
                new_offset = (new_offset + scroll_step).max(0.0);
            }
            if self.input.is_pressed("PageUp") {
                new_offset = (new_offset - scroll_step).max(0.0);
            }
            if (new_offset - offset).abs() > f32::EPSILON {
                self.scroll_offsets.insert(id, new_offset);
            }
        }

        self.push_clip_rect(rect);
        let content_rect = UiRect::new(rect.x, rect.y - offset, rect.width, rect.height);
        self.layout
            .push_layout(LayoutDirection::Vertical, content_rect);
        content(self);
        self.layout.pop_layout();
        self.pop_clip_rect();

        response
    }

    pub fn vertical<R>(&mut self, content: impl FnOnce(&mut Self) -> R) -> R {
        let rect = self.layout.available_rect();
        self.layout.push_layout(LayoutDirection::Vertical, rect);
        let out = content(self);
        self.layout.pop_layout();
        out
    }

    pub fn horizontal<R>(&mut self, content: impl FnOnce(&mut Self) -> R) -> R {
        let rect = self.layout.available_rect();
        self.layout.push_layout(LayoutDirection::Horizontal, rect);
        let out = content(self);
        self.layout.pop_layout();
        out
    }

    pub fn add_space(&mut self, space: f32) {
        self.layout.add_space(space);
    }

    pub fn grid(&mut self, columns: usize) -> GridBuilder<'_> {
        GridBuilder::new(self, columns)
    }

    pub fn tabs(&mut self, selected: &mut usize, labels: &[impl AsRef<str>]) {
        self.horizontal(|ui| {
            for (i, label) in labels.iter().enumerate() {
                let is_selected = *selected == i;
                let original = ui.style.button_color;
                ui.style.button_color = if is_selected {
                    ui.style.accent_color
                } else {
                    ui.style.widget_bg
                };
                if ui.button(label.as_ref()).clicked() {
                    *selected = i;
                }
                ui.style.button_color = original;
            }
        });
    }

    pub fn modal(&mut self, open: &mut bool, content: impl FnOnce(&mut Self)) -> Response {
        if !*open {
            return Response::new(WidgetId::new(0), UiRect::new(0.0, 0.0, 0.0, 0.0));
        }
        let id = self.next_id();
        let screen = self.layout.screen_rect;
        let overlay = Color::BLACK.with_alpha(0.5);
        self.paint_rect(screen, overlay, 0.0);

        let modal_rect = rect_from_center_size(screen.center(), vec2(400.0, 300.0));
        self.paint_rect(modal_rect, self.style.panel_color, self.style.rounding);

        let content_rect = modal_rect.shrink(self.style.panel_padding);
        self.layout
            .push_layout(LayoutDirection::Vertical, content_rect);
        content(self);
        self.layout.pop_layout();

        if self.input.is_just_pressed("Escape") {
            *open = false;
        }

        Response::new(id, modal_rect)
    }

    fn interact(&mut self, id: WidgetId, rect: UiRect) -> Response {
        let hovered = rect.contains(self.pointer);
        let clicked = hovered && self.input.lmb_just_pressed;
        let right_clicked = hovered && self.input.rmb_just_pressed;
        let middle_clicked = hovered && self.input.mmb_just_pressed;
        let mut double_clicked = false;
        self.focus.register_focusable(id);
        if hovered {
            self.tooltip_manager.register_hover(id);
        }
        if clicked {
            self.focus.request_focus(id);
            if let (Some(last_time), Some(last_id)) = (self.last_click_time, self.last_click_id) {
                if last_id == id && (self.time_seconds - last_time) <= self.double_click_max_delay {
                    double_clicked = true;
                }
            }
            self.last_click_time = Some(self.time_seconds);
            self.last_click_id = Some(id);
        }
        let focused = self.focus.has_focus(id);
        let keyboard_select = focused
            && self
                .input_map
                .is_action_just_pressed(Action::Select, &self.input);
        let drag_started = hovered && self.input.lmb_just_pressed && self.drag_active.is_none();
        if drag_started {
            self.drag_active = Some(id);
        }
        let dragging = self.drag_active == Some(id) && self.input.lmb_down;
        let drag_released = self.drag_active == Some(id) && self.input.lmb_just_released;
        if drag_released {
            self.drag_active = None;
        }
        let drag_delta = if dragging {
            vec2(
                self.pointer.x - self.prev_pointer.x,
                self.pointer.y - self.prev_pointer.y,
            )
        } else {
            vec2(0.0, 0.0)
        };
        let mut response = Response::new(id, rect);
        response.hovered = hovered;
        response.clicked = clicked || keyboard_select;
        response.right_clicked = right_clicked;
        response.middle_clicked = middle_clicked;
        response.double_clicked = double_clicked;
        response.focused = focused;
        response.focus_gained = self.focus.gained_focus(id);
        response.focus_lost = self.focus.lost_focus(id);
        response.drag_started = drag_started;
        response.dragging = dragging;
        response.drag_released = drag_released;
        response.drag_delta = drag_delta;
        response
    }

    fn offset_rect(&self, rect: UiRect) -> UiRect {
        UiRect::new(
            rect.x + self.paint_offset.x,
            rect.y + self.paint_offset.y,
            rect.width,
            rect.height,
        )
    }

    fn offset_pos(&self, pos: UiVec2) -> UiVec2 {
        vec2(pos.x + self.paint_offset.x, pos.y + self.paint_offset.y)
    }

    fn paint_rect(&mut self, rect: UiRect, color: Color, corner_radius: f32) {
        self.painter
            .rect(self.offset_rect(rect), color, corner_radius);
    }

    fn paint_rect_outline(&mut self, rect: UiRect, color: Color, thickness: f32) {
        self.paint_rect_outline_rounded(rect, color, thickness, 0.0);
    }

    fn paint_rect_outline_rounded(
        &mut self,
        rect: UiRect,
        color: Color,
        thickness: f32,
        corner_radius: f32,
    ) {
        self.painter
            .rect_outline(self.offset_rect(rect), color, thickness, corner_radius);
    }

    fn paint_text(&mut self, pos: UiVec2, text: &str, color: Color, font_key: &str, size: f32) {
        self.painter
            .text(self.offset_pos(pos), text, color, font_key, size);
    }

    fn paint_text_centered(
        &mut self,
        rect: UiRect,
        text: &str,
        color: Color,
        font_key: &str,
        size: f32,
    ) {
        self.painter
            .text_centered(self.offset_rect(rect), text, color, font_key, size);
    }

    fn paint_image(&mut self, texture: Uuid, pos: UiVec2, size: UiVec2) {
        let pos = self.offset_pos(pos);
        self.painter.image(texture, pos, size);
    }

    fn paint_image_tinted(&mut self, texture: Uuid, pos: UiVec2, size: UiVec2, tint: Color) {
        let pos = self.offset_pos(pos);
        self.painter.image_tinted(texture, pos, size, tint);
    }

    fn render_tooltips(&mut self) {
        let (text, pos) = match self.tooltip_manager.active_tooltip() {
            Some((_, TooltipContent::Text(text), pos)) => (Some(text.clone()), Some(*pos)),
            _ => (None, None),
        };

        if let (Some(text), Some(pos)) = (text, pos) {
            let text_size =
                self.painter
                    .measure_text(&text, &self.style.font_key, self.style.font_size);
            let padding = 8.0;
            let tooltip_size = vec2(text_size.x + padding * 2.0, text_size.y + padding * 2.0);
            let mut tooltip_pos = vec2(pos.x - tooltip_size.x * 0.5, pos.y - tooltip_size.y - 5.0);
            tooltip_pos = vec2(
                tooltip_pos
                    .x
                    .max(5.0)
                    .min(self.layout.screen_rect.width() - tooltip_size.x - 5.0),
                tooltip_pos.y.max(5.0),
            );
            let tooltip_rect =
                UiRect::new(tooltip_pos.x, tooltip_pos.y, tooltip_size.x, tooltip_size.y);

            let bg = self.style.panel_color.with_alpha(0.95);
            self.painter.rect(tooltip_rect, bg, 4.0);
            self.painter.rect_outline(
                tooltip_rect,
                self.style.accent_color.with_alpha(0.5),
                1.0,
                4.0,
            );

            let text_pos = vec2(tooltip_rect.x + padding, tooltip_rect.y + padding);
            self.painter.text(
                text_pos,
                &text,
                self.style.text_color,
                &self.style.font_key,
                self.style.font_size,
            );
        }
    }

    fn push_clip_rect(&mut self, rect: UiRect) {
        self.painter.push_clip_rect(self.offset_rect(rect));
    }

    fn pop_clip_rect(&mut self) {
        self.painter.pop_clip_rect();
    }
}

pub struct GridBuilder<'a> {
    ui: &'a mut UIContext,
    columns: usize,
    col_gap: f32,
    row_gap: f32,
    cell_size: UiVec2,
    origin: UiVec2,
    index: usize,
    max_x: f32,
    max_y: f32,
    parent_direction: LayoutDirection,
}

impl<'a> GridBuilder<'a> {
    fn new(ui: &'a mut UIContext, columns: usize) -> Self {
        let origin = ui.layout.cursor();
        let parent_direction = ui.layout.current_direction();
        GridBuilder {
            ui,
            columns: columns.max(1),
            col_gap: 8.0,
            row_gap: 8.0,
            cell_size: vec2(80.0, 80.0),
            origin,
            index: 0,
            max_x: origin.x,
            max_y: origin.y,
            parent_direction,
        }
    }

    pub fn cell_size(mut self, size: UiVec2) -> Self {
        self.cell_size = size;
        self
    }

    pub fn gaps(mut self, col_gap: f32, row_gap: f32) -> Self {
        self.col_gap = col_gap;
        self.row_gap = row_gap;
        self
    }

    pub fn cell(&mut self, content: impl FnOnce(&mut UIContext)) {
        let col = self.index % self.columns;
        let row = self.index / self.columns;
        let x = self.origin.x + col as f32 * (self.cell_size.x + self.col_gap);
        let y = self.origin.y + row as f32 * (self.cell_size.y + self.row_gap);
        let rect = UiRect::new(x, y, self.cell_size.x, self.cell_size.y);
        self.ui.layout.push_layout(LayoutDirection::Free, rect);
        content(self.ui);
        self.ui.layout.pop_layout();
        self.max_x = self.max_x.max(rect.right());
        self.max_y = self.max_y.max(rect.bottom());
        self.index += 1;
    }
}

impl<'a> Drop for GridBuilder<'a> {
    fn drop(&mut self) {
        let width = self.max_x - self.origin.x;
        let height = self.max_y - self.origin.y;
        match self.parent_direction {
            LayoutDirection::Vertical => self.ui.layout.add_space(height),
            LayoutDirection::Horizontal => self.ui.layout.add_space(width),
            LayoutDirection::Free => {}
        }
    }
}
