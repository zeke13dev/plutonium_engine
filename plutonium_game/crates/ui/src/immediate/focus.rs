#![forbid(unsafe_code)]

use crate::immediate::types::WidgetId;

#[derive(Debug, Clone)]
pub struct FocusManager {
    focused: Option<WidgetId>,
    prev_focused: Option<WidgetId>,
    tab_order: Vec<WidgetId>,
    focus_request: Option<WidgetId>,
    focus_next_requested: bool,
    focus_prev_requested: bool,
}

impl FocusManager {
    pub fn new() -> Self {
        FocusManager {
            focused: None,
            prev_focused: None,
            tab_order: Vec::new(),
            focus_request: None,
            focus_next_requested: false,
            focus_prev_requested: false,
        }
    }

    pub fn begin_frame(&mut self) {
        self.tab_order.clear();
        self.focus_next_requested = false;
        self.focus_prev_requested = false;
    }

    pub fn register_focusable(&mut self, id: WidgetId) {
        self.tab_order.push(id);
    }

    pub fn request_focus(&mut self, id: WidgetId) {
        self.focus_request = Some(id);
    }

    pub fn request_focus_next(&mut self) {
        self.focus_next_requested = true;
    }

    pub fn request_focus_prev(&mut self) {
        self.focus_prev_requested = true;
    }

    /// Clear focus from all widgets
    pub fn clear_focus(&mut self) {
        self.focus_request = Some(WidgetId::new(0));
    }

    pub fn has_focus(&self, id: WidgetId) -> bool {
        self.focused == Some(id)
    }

    pub fn gained_focus(&self, id: WidgetId) -> bool {
        self.focused == Some(id) && self.prev_focused != Some(id)
    }

    pub fn lost_focus(&self, id: WidgetId) -> bool {
        self.prev_focused == Some(id) && self.focused != Some(id)
    }

    pub fn end_frame(&mut self) {
        self.prev_focused = self.focused;

        if let Some(id) = self.focus_request.take() {
            self.focused = Some(id);
            return;
        }

        if self.tab_order.is_empty() {
            return;
        }

        if self.focus_next_requested {
            self.focused = Some(self.next_in_order());
        } else if self.focus_prev_requested {
            self.focused = Some(self.prev_in_order());
        }
    }

    fn next_in_order(&self) -> WidgetId {
        match self.focused {
            None => self.tab_order[0],
            Some(current) => {
                let idx = self
                    .tab_order
                    .iter()
                    .position(|id| *id == current)
                    .unwrap_or(usize::MAX);
                if idx == usize::MAX || idx + 1 >= self.tab_order.len() {
                    self.tab_order[0]
                } else {
                    self.tab_order[idx + 1]
                }
            }
        }
    }

    fn prev_in_order(&self) -> WidgetId {
        match self.focused {
            None => self.tab_order[0],
            Some(current) => {
                let idx = self
                    .tab_order
                    .iter()
                    .position(|id| *id == current)
                    .unwrap_or(0);
                if idx == 0 {
                    *self.tab_order.last().unwrap()
                } else {
                    self.tab_order[idx - 1]
                }
            }
        }
    }
}
