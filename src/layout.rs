use crate::utils::{Position, Rectangle, Size};

#[derive(Clone, Copy, Debug)]
pub enum HAnchor {
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug)]
pub enum VAnchor {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug)]
pub struct Anchors {
    pub h: HAnchor,
    pub v: VAnchor,
}

impl Default for Anchors {
    fn default() -> Self {
        Self {
            h: HAnchor::Left,
            v: VAnchor::Top,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PercentSize {
    pub width_pct: f32,  // 0.0..=1.0
    pub height_pct: f32, // 0.0..=1.0
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Margins {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Default)]
pub struct LayoutParams {
    pub anchors: Anchors,
    pub percent: Option<PercentSize>,
    pub margins: Margins,
}

// Default is derived above

pub struct LayoutResult {
    pub position: Position,
    pub size: Size,
}

pub fn layout_node(container: Rectangle, desired: Size, params: LayoutParams) -> LayoutResult {
    // Resolve size
    let size = if let Some(p) = params.percent {
        Size {
            width: container.width * p.width_pct,
            height: container.height * p.height_pct,
        }
    } else {
        desired
    };

    // Apply margins to content area
    let content = Rectangle::new(
        container.x + params.margins.left,
        container.y + params.margins.top,
        (container.width - params.margins.left - params.margins.right).max(0.0),
        (container.height - params.margins.top - params.margins.bottom).max(0.0),
    );

    // Resolve anchored position within content
    let x = match params.anchors.h {
        HAnchor::Left => content.x,
        HAnchor::Center => content.x + (content.width - size.width) * 0.5,
        HAnchor::Right => content.x + content.width - size.width,
    };
    let y = match params.anchors.v {
        VAnchor::Top => content.y,
        VAnchor::Middle => content.y + (content.height - size.height) * 0.5,
        VAnchor::Bottom => content.y + content.height - size.height,
    };

    LayoutResult {
        position: Position { x, y },
        size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_percent_layout() {
        let container = Rectangle::new(0.0, 0.0, 200.0, 100.0);
        let desired = Size {
            width: 50.0,
            height: 20.0,
        };
        let params = LayoutParams {
            anchors: Anchors {
                h: HAnchor::Center,
                v: VAnchor::Middle,
            },
            percent: Some(PercentSize {
                width_pct: 0.5,
                height_pct: 0.5,
            }),
            margins: Margins::default(),
        };
        let out = layout_node(container, desired, params);
        assert!((out.size.width - 100.0).abs() < 1e-3);
        assert!((out.size.height - 50.0).abs() < 1e-3);
        assert!((out.position.x - 50.0).abs() < 1e-3);
        assert!((out.position.y - 25.0).abs() < 1e-3);
    }
}
