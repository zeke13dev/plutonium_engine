# Layout (feature: `layout`)

Minimal, optional helpers for positioning UI or sprites in logical space.

- Anchors: horizontal (Left, Center, Right), vertical (Top, Middle, Bottom)
- Percent sizing: specify width/height as a fraction of container
- Margins: left/right/top/bottom

## Enabling the Layout Feature

To use layout in your project, enable it in your `Cargo.toml`:

```toml
[dependencies]
plutonium_engine = { path = "../path/to/plutonium_engine", features = ["layout"] }
```

Or if using from crates.io:
```toml
[dependencies]
plutonium_engine = { version = "0.7.0", features = ["layout"] }
```

## API

- `engine.window_bounds() -> Rectangle` (preferred)
  - Returns the window bounds as a Rectangle for use with layout
  - This is the recommended way to get the window container

- `layout::window_bounds(width: f32, height: f32) -> Rectangle`
  - Standalone helper function to create a window bounds rectangle
  - Useful when you don't have access to the engine instance
  - Returns `Rectangle::new(0.0, 0.0, width, height)`

- `layout_node(container: Rectangle, desired: Size, params: LayoutParams) -> LayoutResult`
  - `container`: area to layout into (use `engine.window_bounds()` for window bounds)
  - `desired`: fallback size if percent is not set
  - `params`: anchors, percent sizing, margins
  - returns position and size in logical pixels

### Anchors

Horizontal anchors:
- `HAnchor::Left` - align to left edge
- `HAnchor::Center` - center horizontally
- `HAnchor::Right` - align to right edge
- `HAnchor::Percent(f32)` - position center at percentage (0.0 = left, 0.5 = center, 1.0 = right)

Vertical anchors:
- `VAnchor::Top` - align to top edge
- `VAnchor::Middle` - center vertically
- `VAnchor::Bottom` - align to bottom edge
- `VAnchor::Percent(f32)` - position center at percentage (0.0 = top, 0.5 = middle, 1.0 = bottom)

## Example: Drawing a Top Bar

```rust
use plutonium_engine::{
    layout::{layout_node, Anchors, HAnchor, LayoutParams, PercentSize, VAnchor},
    utils::Size,
};

// Get window bounds directly from the engine
let container = engine.window_bounds();

// Create a top bar: full width, 10% height, anchored to top-left
let layout_result = layout_node(
    container,
    Size {
        width: 100.0,  // Fallback if percent not used
        height: 50.0,  // Fallback if percent not used
    },
    LayoutParams {
        anchors: Anchors {
            h: HAnchor::Left,
            v: VAnchor::Top,
        },
        percent: Some(PercentSize {
            width_pct: 1.0,   // Full width
            height_pct: 0.1,  // 10% of container height
        }),
        margins: Margins::default(), // No margins
    },
);

// Draw the top bar rectangle
engine.draw_rect(
    Rectangle::new(
        layout_result.position.x,
        layout_result.position.y,
        layout_result.size.width,
        layout_result.size.height,
    ),
    [0.2, 0.2, 0.25, 1.0], // Color
    0.0,                    // Corner radius
    Some(([0.1, 0.1, 0.12, 1.0], 1.0)), // Border
    0,                      // Z-order
);
```

## Example: Positioning at Arbitrary Percentages

You can position an element's center at any percentage of the container:

```rust
use plutonium_engine::{
    layout::{layout_node, Anchors, HAnchor, LayoutParams, VAnchor},
    utils::Size,
};

let container = engine.window_bounds();

// Position element center at 1/3 width, 1/2 height
let layout_result = layout_node(
    container,
    Size { width: 100.0, height: 50.0 },
    LayoutParams {
        anchors: Anchors {
            h: HAnchor::Percent(1.0 / 3.0),  // 1/3 from left
            v: VAnchor::Percent(0.5),         // 1/2 from top (middle)
        },
        percent: None,
        margins: Margins::default(),
    },
);

// Draw the element
engine.draw_rect(
    Rectangle::new(
        layout_result.position.x,
        layout_result.position.y,
        layout_result.size.width,
        layout_result.size.height,
    ),
    [0.2, 0.4, 0.6, 1.0],
    0.0,
    None,
    0,
);
```

See `examples/layout_top_bar.rs` for a complete working example.

## Notes

- Camera transforms are applied after layout; layout runs in logical screen space.
- This is intentionally minimal. More advanced layouts (flex/grid/constraints) belong in the higher-level game engine.
- HUD coordinates are logical and unaffected by DPI; the engine applies DPI and camera after layout.
