#![forbid(unsafe_code)]

pub mod context;
pub mod drag_drop;
pub mod draw_list;
pub mod focus;
pub mod input;
pub mod input_map;
pub mod layout;
pub mod painter;
pub mod response;
pub mod state;
pub mod tooltip;
pub mod types;

pub use context::UIContext;
pub use drag_drop::{DragData, DragDropState};
pub use draw_list::{DrawCommand, DrawList};
pub use focus::FocusManager;
pub use input::{InputStateExt, UiInputState};
pub use input_map::{Action, InputBinding, InputMap, MouseButton};
pub use layout::{LayoutDirection, LayoutEngine};
pub use painter::Painter;
pub use plutonium_engine::{HaloFalloff, HaloPreset, HaloStyle};
pub use response::Response;
pub use state::StateCache;
pub use tooltip::{TooltipContent, TooltipManager};
pub use types::{
    rect_from_center_size, rect_from_min_max, vec2, Color, RectExt, UiRect, UiVec2, WidgetId,
};
