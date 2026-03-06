use crate::utils::{Position, Rectangle, Size};

#[derive(Clone, Copy, Debug)]
pub enum HAnchor {
    Left,
    Center,
    Right,
    /// Position the center of the element at the given percentage of container width.
    /// `0.0` = left edge, `0.5` = center, `1.0` = right edge.
    Percent(f32),
}
#[derive(Clone, Copy, Debug)]
pub enum VAnchor {
    Top,
    Middle,
    Bottom,
    /// Position the center of the element at the given percentage of container height.
    /// `0.0` = top edge, `0.5` = middle, `1.0` = bottom edge.
    Percent(f32),
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

/// Returns a rectangle representing the window bounds from width and height.
/// This is a convenience helper for creating a container rectangle for layout.
///
/// **Note:** This function expects logical pixel dimensions (not physical pixels).
/// If you have physical pixels, divide by DPI scale factor first.
///
/// # Example
/// ```
/// use plutonium_engine::layout::window_bounds;
///
/// // Logical pixel dimensions (e.g., 800x600)
/// let container = window_bounds(800.0, 600.0);
/// // Equivalent to: Rectangle::new(0.0, 0.0, 800.0, 600.0)
/// ```
pub fn window_bounds(width: f32, height: f32) -> Rectangle {
    Rectangle::new(0.0, 0.0, width, height)
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
        HAnchor::Percent(pct) => {
            // Position center of element at pct of container width
            // Clamp percentage to valid range
            let pct = pct.clamp(0.0, 1.0);
            let center_x = content.x + content.width * pct;
            center_x - size.width * 0.5
        }
    };
    let y = match params.anchors.v {
        VAnchor::Top => content.y,
        VAnchor::Middle => content.y + (content.height - size.height) * 0.5,
        VAnchor::Bottom => content.y + content.height - size.height,
        VAnchor::Percent(pct) => {
            // Position center of element at pct of container height
            // Clamp percentage to valid range
            let pct = pct.clamp(0.0, 1.0);
            let center_y = content.y + content.height * pct;
            center_y - size.height * 0.5
        }
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

    #[test]
    fn percent_anchor_positioning() {
        let container = Rectangle::new(0.0, 0.0, 300.0, 200.0);
        let desired = Size {
            width: 100.0,
            height: 50.0,
        };

        // Position center at 1/3 width, 1/2 height
        let params = LayoutParams {
            anchors: Anchors {
                h: HAnchor::Percent(1.0 / 3.0),
                v: VAnchor::Percent(0.5),
            },
            percent: None,
            margins: Margins::default(),
        };
        let out = layout_node(container, desired, params);

        // Center should be at 100px (1/3 of 300), 100px (1/2 of 200)
        // So top-left should be at (100 - 50, 100 - 25) = (50, 75)
        assert!((out.position.x - 50.0).abs() < 1e-3);
        assert!((out.position.y - 75.0).abs() < 1e-3);

        // Size should be unchanged
        assert!((out.size.width - 100.0).abs() < 1e-3);
        assert!((out.size.height - 50.0).abs() < 1e-3);
    }

    #[test]
    fn percent_anchor_with_margins() {
        let container = Rectangle::new(0.0, 0.0, 400.0, 300.0);
        let desired = Size {
            width: 80.0,
            height: 40.0,
        };

        // Position center at 50% width, 75% height with margins
        let params = LayoutParams {
            anchors: Anchors {
                h: HAnchor::Percent(0.5),
                v: VAnchor::Percent(0.75),
            },
            percent: None,
            margins: Margins {
                left: 20.0,
                right: 20.0,
                top: 10.0,
                bottom: 10.0,
            },
        };
        let out = layout_node(container, desired, params);

        // Content area: (20, 10) to (380, 290), size (360, 280)
        // Center should be at: 20 + 360*0.5 = 200, 10 + 280*0.75 = 220
        // Position: (200 - 40, 220 - 20) = (160, 200)
        assert!((out.position.x - 160.0).abs() < 1e-3);
        assert!((out.position.y - 200.0).abs() < 1e-3);
    }
}
