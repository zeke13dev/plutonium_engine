# Positioning Elements at Specific Coordinates

## Positioning Center at 1/3 Width, 1/2 Height

To position an element so its **center** is at 1/3 width and 1/2 height of the container:

### Option 1: Using Percent Anchors (Recommended)

The layout system now supports percentage-based positioning directly:

```rust
use plutonium_engine::{
    layout::{layout_node, Anchors, HAnchor, LayoutParams, VAnchor},
    utils::Size,
};

let container = engine.window_bounds();

// Position center at 1/3 width, 1/2 height using percent anchors
let layout_result = layout_node(
    container,
    Size { width: 100.0, height: 50.0 },
    LayoutParams {
        anchors: Anchors {
            h: HAnchor::Percent(1.0 / 3.0),  // Center at 1/3 from left
            v: VAnchor::Percent(0.5),         // Center at 1/2 from top
        },
        percent: None,
        margins: Margins::default(),
    },
);

engine.draw_rect(
    Rectangle::new(
        layout_result.position.x,
        layout_result.position.y,
        layout_result.size.width,
        layout_result.size.height,
    ),
    [0.2, 0.2, 0.25, 1.0],
    0.0,
    None,
    0,
);
```

### Option 2: Manual Calculation

```rust
use plutonium_engine::{
    layout::layout_node,
    utils::{Rectangle, Size},
};

let container = engine.window_bounds();
let element_size = Size { width: 100.0, height: 50.0 };

// Calculate position so center is at 1/3w, 1/2h
let center_x = container.width * (1.0 / 3.0);
let center_y = container.height * (1.0 / 2.0);
let position = Position {
    x: center_x - element_size.width / 2.0,
    y: center_y - element_size.height / 2.0,
};

// Draw the element
engine.draw_rect(
    Rectangle::new(position.x, position.y, element_size.width, element_size.height),
    [0.2, 0.2, 0.25, 1.0],
    0.0,
    None,
    0,
);
```

### Option 2: Using Layout with Margins (More Complex)

You can use the layout system with margins to shift the content area, but it's more complex:

```rust
use plutonium_engine::{
    layout::{layout_node, Anchors, HAnchor, LayoutParams, Margins, VAnchor},
    utils::Size,
};

let container = engine.window_bounds();
let element_size = Size { width: 100.0, height: 50.0 };

// Calculate margins to shift content area so center aligns with 1/3w, 1/2h
// This is tricky because margins reduce the content area
// For center at 1/3w: we want the content area to be centered such that
// its center aligns with 1/3 of container width
let target_center_x = container.width * (1.0 / 3.0);
let target_center_y = container.height * (1.0 / 2.0);

// Create a sub-container positioned at the target center
// Then use Center/Middle anchors within that sub-container
let sub_container = Rectangle::new(
    target_center_x - container.width / 2.0,  // Shift left so center aligns
    target_center_y - container.height / 2.0, // Shift up so center aligns
    container.width,
    container.height,
);

let layout_result = layout_node(
    sub_container,
    element_size,
    LayoutParams {
        anchors: Anchors {
            h: HAnchor::Center,
            v: VAnchor::Middle,
        },
        percent: None,
        margins: Margins::default(),
    },
);

engine.draw_rect(
    Rectangle::new(
        layout_result.position.x,
        layout_result.position.y,
        layout_result.size.width,
        layout_result.size.height,
    ),
    [0.2, 0.2, 0.25, 1.0],
    0.0,
    None,
    0,
);
```

### Option 3: Helper Function

Create a helper function for arbitrary center positioning:

```rust
use plutonium_engine::utils::{Position, Rectangle, Size};

/// Positions an element so its center is at the specified percentage of the container
pub fn position_center_at(
    container: Rectangle,
    element_size: Size,
    center_x_pct: f32,  // 0.0 to 1.0
    center_y_pct: f32,  // 0.0 to 1.0
) -> Position {
    let center_x = container.width * center_x_pct;
    let center_y = container.height * center_y_pct;
    Position {
        x: center_x - element_size.width / 2.0,
        y: center_y - element_size.height / 2.0,
    }
}

// Usage:
let container = engine.window_bounds();
let element_size = Size { width: 100.0, height: 50.0 };
let pos = position_center_at(container, element_size, 1.0 / 3.0, 1.0 / 2.0);

engine.draw_rect(
    Rectangle::new(pos.x, pos.y, element_size.width, element_size.height),
    [0.2, 0.2, 0.25, 1.0],
    0.0,
    None,
    0,
);
```

## Complete Example

```rust
use plutonium_engine::{
    app::run_app,
    utils::{Position, Rectangle, Size},
    WindowConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Position Example".to_string(),
        width: 800,
        height: 600,
    };

    run_app(config, move |engine, _, _app| {
        engine.begin_frame();

        let container = engine.window_bounds();
        let element_size = Size { width: 150.0, height: 100.0 };

        // Position center at 1/3 width, 1/2 height
        let center_x = container.width * (1.0 / 3.0);
        let center_y = container.height * (1.0 / 2.0);
        let pos = Position {
            x: center_x - element_size.width / 2.0,
            y: center_y - element_size.height / 2.0,
        };

        engine.draw_rect(
            Rectangle::new(pos.x, pos.y, element_size.width, element_size.height),
            [0.2, 0.4, 0.6, 1.0], // Blue
            5.0,                   // Rounded corners
            Some(([0.1, 0.2, 0.3, 1.0], 2.0)), // Border
            0,
        );

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
```

## Notes

- **Option 1 (Manual)** is the simplest and most direct
- **Option 2 (Layout)** uses the layout system but is more complex for arbitrary positions
- **Option 3 (Helper)** provides a reusable abstraction
- All coordinates are in logical pixels
- The element's size must be known (either fixed or calculated)

