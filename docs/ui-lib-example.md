# UI Library Example: Creating Bar Helpers

Here's a complete example of how to create a UI library crate with bar helpers.

## Project Structure

```
my_project/
├── Cargo.toml
├── my_ui_lib/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── src/
    └── main.rs
```

## UI Library Crate

### `my_ui_lib/Cargo.toml`

```toml
[package]
name = "my_ui_lib"
version = "0.1.0"
edition = "2021"

[dependencies]
plutonium_engine = { path = "../../plutonium_engine", features = ["layout"] }
```

### `my_ui_lib/src/lib.rs`

```rust
use plutonium_engine::{
    PlutoniumEngine,
    utils::Rectangle,
    layout::{
        layout_node, Anchors, HAnchor, LayoutParams, 
        Margins, PercentSize, VAnchor
    },
};

/// Style configuration for bars
#[derive(Clone, Copy, Debug)]
pub struct BarStyle {
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    pub border_thickness_px: f32,
    pub corner_radius_px: f32,
    pub margins: Margins,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            color: [0.2, 0.2, 0.25, 1.0],
            border_color: [0.1, 0.1, 0.12, 1.0],
            border_thickness_px: 1.0,
            corner_radius_px: 0.0,
            margins: Margins::default(),
        }
    }
}

/// Draws a top bar that spans the full width of the window
pub fn draw_top_bar(
    engine: &mut PlutoniumEngine,
    height_pct: f32,
    style: BarStyle,
    z: i32,
) {
    let container = engine.window_bounds();
    
    let layout_result = layout_node(
        container,
        plutonium_engine::utils::Size {
            width: 100.0,
            height: container.height * height_pct,
        },
        LayoutParams {
            anchors: Anchors {
                h: HAnchor::Left,
                v: VAnchor::Top,
            },
            percent: Some(PercentSize {
                width_pct: 1.0,
                height_pct,
            }),
            margins: style.margins,
        },
    );

    let border = if style.border_thickness_px > 0.0 {
        Some((style.border_color, style.border_thickness_px))
    } else {
        None
    };

    engine.draw_rect(
        Rectangle::new(
            layout_result.position.x,
            layout_result.position.y,
            layout_result.size.width,
            layout_result.size.height,
        ),
        style.color,
        style.corner_radius_px,
        border,
        z,
    );
}

/// Draws a bottom bar that spans the full width of the window
pub fn draw_bottom_bar(
    engine: &mut PlutoniumEngine,
    height_pct: f32,
    style: BarStyle,
    z: i32,
) {
    let container = engine.window_bounds();
    
    let layout_result = layout_node(
        container,
        plutonium_engine::utils::Size {
            width: 100.0,
            height: container.height * height_pct,
        },
        LayoutParams {
            anchors: Anchors {
                h: HAnchor::Left,
                v: VAnchor::Bottom,
            },
            percent: Some(PercentSize {
                width_pct: 1.0,
                height_pct,
            }),
            margins: style.margins,
        },
    );

    let border = if style.border_thickness_px > 0.0 {
        Some((style.border_color, style.border_thickness_px))
    } else {
        None
    };

    engine.draw_rect(
        Rectangle::new(
            layout_result.position.x,
            layout_result.position.y,
            layout_result.size.width,
            layout_result.size.height,
        ),
        style.color,
        style.corner_radius_px,
        border,
        z,
    );
}
```

## Main Project

### `Cargo.toml` (workspace root)

```toml
[workspace]
members = ["my_ui_lib", "."]

[package]
name = "my_app"
version = "0.1.0"
edition = "2021"

[dependencies]
plutonium_engine = { path = "../plutonium_engine" }
my_ui_lib = { path = "my_ui_lib" }
```

### `src/main.rs`

```rust
use my_ui_lib::{draw_top_bar, BarStyle};
use plutonium_engine::{app::run_app, WindowConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "My App with UI Library".to_string(),
        width: 800,
        height: 600,
    };

    run_app(config, move |engine, _, _app| {
        engine.begin_frame();

        // Use your UI library to draw a top bar
        let style = BarStyle {
            color: [0.15, 0.15, 0.18, 1.0],
            ..Default::default()
        };
        draw_top_bar(&mut engine, 0.1, style, 0);

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
```

## Key Points

1. **Your UI library depends on `plutonium_engine`**: It uses the engine's rendering primitives
2. **You control the API**: Design your UI helpers however you want
3. **Features are opt-in**: Enable only the features your UI library needs (like `layout`)
4. **Reusable**: The same UI library can be used across multiple projects
5. **Separation**: UI logic stays in your library, engine stays focused on rendering

## Advanced: Panel Systems, Widgets, etc.

You can extend this pattern to create:
- Panel/container systems
- Widget libraries (buttons, sliders, etc.)
- Layout managers
- Theming systems
- UI state management

The engine provides the primitives (`draw_rect`, `draw_texture`, etc.), and your UI library builds higher-level abstractions on top!

