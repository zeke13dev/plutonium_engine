#![forbid(unsafe_code)]

use crate::immediate::types::{rect_from_min_max, vec2, RectExt, UiRect, UiVec2};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LayoutDirection {
    Vertical,
    Horizontal,
    Free,
}

#[derive(Debug, Clone)]
struct LayoutContext {
    direction: LayoutDirection,
    cursor: UiVec2,
    available_rect: UiRect,
    spacing: f32,
    max_pos: UiVec2,
}

impl LayoutContext {
    fn new(direction: LayoutDirection, rect: UiRect) -> Self {
        let cursor = vec2(rect.x, rect.y);
        LayoutContext {
            direction,
            cursor,
            available_rect: rect,
            spacing: 4.0,
            max_pos: cursor,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayoutEngine {
    stack: Vec<LayoutContext>,
    pub screen_rect: UiRect,
}

impl LayoutEngine {
    pub fn new(screen_rect: UiRect) -> Self {
        let root = LayoutContext::new(LayoutDirection::Vertical, screen_rect);
        LayoutEngine {
            stack: vec![root],
            screen_rect,
        }
    }

    pub fn push_layout(&mut self, direction: LayoutDirection, rect: UiRect) {
        self.stack.push(LayoutContext::new(direction, rect));
    }

    pub fn pop_layout(&mut self) -> Option<UiRect> {
        if self.stack.len() <= 1 {
            return None;
        }
        let ctx = self.stack.pop()?;
        Some(rect_from_min_max(ctx.available_rect.min(), ctx.max_pos))
    }

    pub fn allocate(&mut self, size: UiVec2) -> UiRect {
        let ctx = self.stack.last_mut().expect("layout stack empty");
        let rect = UiRect::new(ctx.cursor.x, ctx.cursor.y, size.x, size.y);
        ctx.max_pos = vec2(
            ctx.max_pos.x.max(rect.right()),
            ctx.max_pos.y.max(rect.bottom()),
        );
        match ctx.direction {
            LayoutDirection::Vertical => {
                ctx.cursor.y += size.y + ctx.spacing;
            }
            LayoutDirection::Horizontal => {
                ctx.cursor.x += size.x + ctx.spacing;
            }
            LayoutDirection::Free => {}
        }
        rect
    }

    pub fn add_space(&mut self, space: f32) {
        let ctx = self.stack.last_mut().expect("layout stack empty");
        match ctx.direction {
            LayoutDirection::Vertical => ctx.cursor.y += space,
            LayoutDirection::Horizontal => ctx.cursor.x += space,
            LayoutDirection::Free => {}
        }
    }

    pub fn allocate_remaining(&mut self) -> UiRect {
        let ctx = self.stack.last_mut().expect("layout stack empty");
        let remaining_w = (ctx.available_rect.right() - ctx.cursor.x).max(0.0);
        let remaining_h = (ctx.available_rect.bottom() - ctx.cursor.y).max(0.0);
        let rect = UiRect::new(ctx.cursor.x, ctx.cursor.y, remaining_w, remaining_h);
        ctx.max_pos = vec2(
            ctx.max_pos.x.max(rect.right()),
            ctx.max_pos.y.max(rect.bottom()),
        );
        match ctx.direction {
            LayoutDirection::Vertical => {
                ctx.cursor.y = rect.bottom();
            }
            LayoutDirection::Horizontal => {
                ctx.cursor.x = rect.right();
            }
            LayoutDirection::Free => {}
        }
        rect
    }

    pub fn set_spacing(&mut self, spacing: f32) {
        let ctx = self.stack.last_mut().expect("layout stack empty");
        ctx.spacing = spacing;
    }

    pub fn cursor(&self) -> UiVec2 {
        self.stack.last().expect("layout stack empty").cursor
    }

    pub fn available_rect(&self) -> UiRect {
        self.stack
            .last()
            .expect("layout stack empty")
            .available_rect
    }

    pub fn current_direction(&self) -> LayoutDirection {
        self.stack.last().expect("layout stack empty").direction
    }
}
