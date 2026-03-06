#![cfg_attr(not(feature = "layout"), allow(dead_code))]
#[cfg(feature = "layout")]
use plutonium_engine::{
    app::run_app,
    layout::{layout_node, Anchors, HAnchor, LayoutParams, VAnchor},
    utils::{Rectangle, Size},
    WindowConfig,
};

#[cfg(feature = "layout")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Layout Percent Position Example".to_string(),
        width: 800,
        height: 600,
    };

    run_app(config, move |engine, _, _app| {
        engine.begin_frame();

        let container = engine.window_bounds();

        // Position element center at 1/3 width, 1/2 height
        let layout_result = layout_node(
            container,
            Size {
                width: 150.0,
                height: 100.0,
            },
            LayoutParams {
                anchors: Anchors {
                    h: HAnchor::Percent(1.0 / 3.0), // 1/3 from left
                    v: VAnchor::Percent(0.5),       // 1/2 from top (middle)
                },
                percent: None,
                margins: Default::default(),
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
            [0.2, 0.4, 0.6, 1.0],              // Blue
            5.0,                               // Rounded corners
            Some(([0.1, 0.2, 0.3, 1.0], 2.0)), // Border
            0,
        );

        engine.end_frame().unwrap();
    })?;

    Ok(())
}

#[cfg(not(feature = "layout"))]
fn main() {
    println!("Run with --features layout to enable this example.");
}
