# Creating a UI Library Crate

Yes! You can absolutely create a separate UI library crate that depends on `plutonium_engine` and handles UI rendering like bars, panels, buttons, etc.

This is a common pattern and there's already an example in this codebase: `plutonium_game/crates/ui`.

## Setup

### 1. Create a new crate

Create a new crate in your workspace or as a standalone project:

```toml
# my_ui_lib/Cargo.toml
[package]
name = "my_ui_lib"
version = "0.1.0"
edition = "2021"

[dependencies]
plutonium_engine = { path = "../plutonium_engine", features = ["layout"] }
```

Or if using from crates.io:
```toml
[dependencies]
plutonium_engine = { version = "0.7.0", features = ["layout"] }
```

### 2. Create UI helpers

In your UI library, you can create helpers that use the engine:

```rust
// my_ui_lib/src/lib.rs
use plutonium_engine::{PlutoniumEngine, utils::Rectangle};
use plutonium_engine::layout::{layout_node, Anchors, HAnchor, LayoutParams, PercentSize, VAnchor, Margins};

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

/// Draws a top bar in the window
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

/// Draws a bottom bar in the window
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

### 3. Use in your project

In your main project:

```toml
# Cargo.toml
[dependencies]
plutonium_engine = { path = "../plutonium_engine" }
my_ui_lib = { path = "../my_ui_lib" }
```

```rust
// src/main.rs
use my_ui_lib::{draw_top_bar, BarStyle};
use plutonium_engine::{app::run_app, WindowConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "My App".to_string(),
        width: 800,
        height: 600,
    };

    run_app(config, move |engine, _, _app| {
        engine.begin_frame();

        // Use your UI library helpers
        let style = BarStyle::default();
        draw_top_bar(&mut engine, 0.1, style, 0);

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
```

## Benefits

1. **Separation of concerns**: UI logic is separate from engine code
2. **Reusability**: Use the same UI library across multiple projects
3. **Customization**: Build UI components specific to your needs
4. **Lightweight engine**: Keep the engine focused on rendering, not UI abstractions
5. **Feature flags**: Your UI lib can enable features like `layout` without affecting the engine

## Example in this Codebase

See `plutonium_game/crates/ui/` for a real example of a UI library built on top of `plutonium_engine`. It includes:
- Render command batching
- Theme support
- Panel rendering with 9-slice
- Text rendering helpers

## Notes

- Your UI library should depend on `plutonium_engine` with the features it needs (like `layout`)
- The engine provides the rendering primitives (`draw_rect`, `draw_texture`, etc.)
- Your UI library provides higher-level abstractions (bars, panels, widgets, etc.)
- This pattern allows you to build UI frameworks tailored to your specific needs

