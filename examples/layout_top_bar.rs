#![cfg_attr(not(feature = "layout"), allow(dead_code))]
#[cfg(feature = "layout")]
use plutonium_engine::{
    app::run_app,
    layout::{layout_node, Anchors, HAnchor, LayoutParams, Margins, PercentSize, VAnchor},
    utils::{Rectangle, Size},
    WindowConfig,
};

#[cfg(feature = "layout")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Layout Top Bar Example".to_string(),
        width: 800,
        height: 600,
    };

    run_app(config, move |engine, _, _app| {
        engine.begin_frame();

        // Get window bounds directly from the engine
        let container = engine.window_bounds();

        // Create a top bar using layout: full width, 10% height, anchored to top-left
        let layout_result = layout_node(
            container,
            Size {
                width: 100.0, // Fallback if percent not used
                height: 50.0, // Fallback if percent not used
            },
            LayoutParams {
                anchors: Anchors {
                    h: HAnchor::Left,
                    v: VAnchor::Top,
                },
                percent: Some(PercentSize {
                    width_pct: 1.0,  // Full width
                    height_pct: 0.1, // 10% of container height
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
            [0.2, 0.2, 0.25, 1.0],              // Dark gray color
            0.0,                                // No corner radius
            Some(([0.1, 0.1, 0.12, 1.0], 1.0)), // Border
            0,                                  // Z-order
        );

        engine.end_frame().unwrap();
    })?;

    Ok(())
}

#[cfg(not(feature = "layout"))]
fn main() {
    println!("Run with --features layout to enable this example.");
}
