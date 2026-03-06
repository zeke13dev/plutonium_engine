#![forbid(unsafe_code)]

use crate::immediate::types::{UiRect, UiVec2, WidgetId};

#[derive(Debug, Clone)]
pub struct Response {
    pub id: WidgetId,
    pub rect: UiRect,
    pub hovered: bool,
    pub clicked: bool,
    pub right_clicked: bool,
    pub middle_clicked: bool,
    pub double_clicked: bool,
    pub drag_started: bool,
    pub dragging: bool,
    pub drag_released: bool,
    pub drag_delta: UiVec2,
    pub focused: bool,
    pub focus_gained: bool,
    pub focus_lost: bool,
    tooltip_text: Option<String>,
}

impl Response {
    pub fn new(id: WidgetId, rect: UiRect) -> Self {
        Response {
            id,
            rect,
            hovered: false,
            clicked: false,
            right_clicked: false,
            middle_clicked: false,
            double_clicked: false,
            drag_started: false,
            dragging: false,
            drag_released: false,
            drag_delta: UiVec2 { x: 0.0, y: 0.0 },
            focused: false,
            focus_gained: false,
            focus_lost: false,
            tooltip_text: None,
        }
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    pub fn clicked(&self) -> bool {
        self.clicked
    }

    /// Add a text tooltip to this widget
    pub fn on_hover_text(mut self, text: impl Into<String>) -> Self {
        self.tooltip_text = Some(text.into());
        self
    }

    pub fn tooltip_text(&self) -> Option<&str> {
        self.tooltip_text.as_deref()
    }
}
