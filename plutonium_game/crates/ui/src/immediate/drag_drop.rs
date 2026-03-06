#![forbid(unsafe_code)]

use crate::immediate::types::{UiVec2, WidgetId};
use std::any::{Any, TypeId};

/// Type-erased drag data.
pub struct DragData {
    data: Box<dyn Any>,
    type_id: TypeId,
}

impl DragData {
    /// Create a new typed payload.
    pub fn new<T: 'static>(data: T) -> Self {
        DragData {
            type_id: TypeId::of::<T>(),
            data: Box::new(data),
        }
    }

    /// Return true if the payload is of type T.
    pub fn is_type<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    /// Consume the payload if its type matches T.
    pub fn take<T: 'static>(self) -> Option<T> {
        if self.is_type::<T>() {
            Some(*self.data.downcast::<T>().unwrap())
        } else {
            None
        }
    }

    /// Borrow the payload as type T if it matches.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.data.downcast_ref::<T>()
    }
}

/// Drag & drop state manager.
#[derive(Default)]
pub struct DragDropState {
    active_drag: Option<DragData>,
    dragging_id: Option<WidgetId>,
    drag_start_pos: UiVec2,
    current_pos: UiVec2,
    payload_preview: Option<String>,
}

impl DragDropState {
    /// Create a new drag & drop state container.
    pub fn new() -> Self {
        DragDropState {
            active_drag: None,
            dragging_id: None,
            drag_start_pos: UiVec2 { x: 0.0, y: 0.0 },
            current_pos: UiVec2 { x: 0.0, y: 0.0 },
            payload_preview: None,
        }
    }

    /// Start a drag operation with a typed payload.
    pub fn start_drag<T: 'static>(&mut self, id: WidgetId, data: T, start_pos: UiVec2) {
        self.active_drag = Some(DragData::new(data));
        self.dragging_id = Some(id);
        self.drag_start_pos = start_pos;
        self.current_pos = start_pos;
    }

    /// Update the current drag cursor position.
    pub fn update_drag_position(&mut self, pos: UiVec2) {
        self.current_pos = pos;
    }

    /// Return true if a drag is active.
    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    /// Return true if the active drag originated from the given widget id.
    pub fn is_dragging_id(&self, id: WidgetId) -> bool {
        self.dragging_id == Some(id)
    }

    /// Return true if the active payload matches type T.
    pub fn is_dragging_type<T: 'static>(&self) -> bool {
        self.active_drag
            .as_ref()
            .map(|d| d.is_type::<T>())
            .unwrap_or(false)
    }

    /// Drag delta from the start position.
    pub fn drag_delta(&self) -> UiVec2 {
        UiVec2 {
            x: self.current_pos.x - self.drag_start_pos.x,
            y: self.current_pos.y - self.drag_start_pos.y,
        }
    }

    /// Current drag position.
    pub fn drag_position(&self) -> UiVec2 {
        self.current_pos
    }

    /// Set a preview label for the drag payload.
    pub fn set_payload_preview(&mut self, preview: impl Into<String>) {
        self.payload_preview = Some(preview.into());
    }

    /// Read the current payload preview label.
    pub fn payload_preview(&self) -> Option<&str> {
        self.payload_preview.as_deref()
    }

    /// End the drag and return the payload.
    pub fn end_drag(&mut self) -> Option<DragData> {
        self.dragging_id = None;
        self.payload_preview = None;
        self.active_drag.take()
    }

    /// Cancel the drag without returning a payload.
    pub fn cancel_drag(&mut self) {
        self.active_drag = None;
        self.dragging_id = None;
        self.payload_preview = None;
    }

    /// Peek at the payload without consuming it.
    pub fn peek_data<T: 'static>(&self) -> Option<&T> {
        self.active_drag.as_ref()?.downcast_ref::<T>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_drop_type_safety() {
        let mut state = DragDropState::new();
        let id = WidgetId::new(1);

        state.start_drag(id, 42i32, UiVec2 { x: 0.0, y: 0.0 });

        assert!(state.is_dragging());
        assert!(state.is_dragging_type::<i32>());
        assert!(!state.is_dragging_type::<String>());

        assert_eq!(state.peek_data::<i32>(), Some(&42));
        assert_eq!(state.peek_data::<String>(), None);
    }

    #[test]
    fn drag_drop_lifecycle() {
        let mut state = DragDropState::new();
        let id = WidgetId::new(1);

        assert!(!state.is_dragging());

        state.start_drag(id, "test".to_string(), UiVec2 { x: 10.0, y: 10.0 });
        assert!(state.is_dragging());
        assert!(state.is_dragging_id(id));

        state.update_drag_position(UiVec2 { x: 20.0, y: 30.0 });
        let delta = state.drag_delta();
        assert_eq!(delta.x, 10.0);
        assert_eq!(delta.y, 20.0);

        let data = state.end_drag().unwrap();
        assert!(!state.is_dragging());
        assert_eq!(data.take::<String>(), Some("test".to_string()));
    }
}
