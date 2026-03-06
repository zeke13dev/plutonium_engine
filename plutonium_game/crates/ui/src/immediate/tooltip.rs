#![forbid(unsafe_code)]

use crate::immediate::types::{UiVec2, WidgetId};
use std::collections::{HashMap, HashSet};

/// Tooltip content.
pub enum TooltipContent {
    Text(String),
    Custom(Box<dyn Fn(&mut crate::immediate::UIContext)>),
}

/// Manages tooltip display with hover delay.
pub struct TooltipManager {
    hover_start_time: HashMap<WidgetId, f32>,
    hovered_this_frame: HashSet<WidgetId>,
    active_tooltip: Option<(WidgetId, TooltipContent, UiVec2)>,
    delay: f32,
    current_time: f32,
}

impl TooltipManager {
    /// Create a tooltip manager with default delay.
    pub fn new() -> Self {
        TooltipManager {
            hover_start_time: HashMap::new(),
            hovered_this_frame: HashSet::new(),
            active_tooltip: None,
            delay: 0.5,
            current_time: 0.0,
        }
    }

    /// Set the hover delay before showing tooltips.
    pub fn set_delay(&mut self, delay_seconds: f32) {
        self.delay = delay_seconds.max(0.0);
    }

    /// Set the current time without resetting hover state.
    pub fn set_time(&mut self, time: f32) {
        self.current_time = time;
    }

    /// Begin a frame and reset per-frame tracking.
    pub fn begin_frame(&mut self, time: f32) {
        self.current_time = time;
        self.hovered_this_frame.clear();
        self.active_tooltip = None;
    }

    /// End a frame and drop hover records that were not refreshed.
    pub fn end_frame(&mut self) {
        self.hover_start_time
            .retain(|id, _| self.hovered_this_frame.contains(id));
    }

    /// Register a hover event for a widget.
    pub fn register_hover(&mut self, id: WidgetId) {
        self.hover_start_time.entry(id).or_insert(self.current_time);
        self.hovered_this_frame.insert(id);
    }

    /// Return true if the hover delay has elapsed for this widget.
    pub fn should_show_tooltip(&self, id: WidgetId) -> bool {
        self.hover_start_time
            .get(&id)
            .map(|start| (self.current_time - start) >= self.delay)
            .unwrap_or(false)
    }

    /// Set the active tooltip if the hover delay has elapsed.
    pub fn set_tooltip(&mut self, id: WidgetId, content: TooltipContent, pos: UiVec2) {
        if self.should_show_tooltip(id) {
            self.active_tooltip = Some((id, content, pos));
        }
    }

    /// Return the active tooltip data for rendering.
    pub fn active_tooltip(&self) -> Option<&(WidgetId, TooltipContent, UiVec2)> {
        self.active_tooltip.as_ref()
    }

    /// Clear the active tooltip.
    pub fn clear(&mut self) {
        self.active_tooltip = None;
    }
}
