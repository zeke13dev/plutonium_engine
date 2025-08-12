#![cfg_attr(not(feature = "layout"), allow(dead_code))]
#[cfg(feature = "layout")]
use plutonium_engine::{
    app::run_app,
    layout::{layout_node, Anchors, HAnchor, LayoutParams, PercentSize, VAnchor},
    utils::{Position, Rectangle, Size},
    WindowConfig,
};

#[cfg(feature = "layout")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Layout Example".to_string(),
        width: 800,
        height: 600,
    };
    let mut sprite = None;
    run_app(config, move |engine, _| {
        if sprite.is_none() {
            sprite = Some(engine.create_texture_2d(
                "examples/media/square.svg",
                Position { x: 0.0, y: 0.0 },
                1.0,
            ));
        }
        engine.begin_frame();
        // simple centered 50% sized element in window
        let container = Rectangle::new(
            0.0,
            0.0,
            engine.size.width as f32,
            engine.size.height as f32,
        );
        let desired = Size {
            width: 100.0,
            height: 100.0,
        };
        let params = LayoutParams {
            anchors: Anchors {
                h: HAnchor::Center,
                v: VAnchor::Middle,
            },
            percent: Some(PercentSize {
                width_pct: 0.2,
                height_pct: 0.2,
            }),
            margins: Default::default(),
        };
        let _out = layout_node(container, desired, params);
        if let Some(s) = &sprite {
            s.render(engine);
        }
        engine.end_frame().unwrap();
    })?;
    Ok(())
}

#[cfg(not(feature = "layout"))]
fn main() {
    println!("Run with --features layout to enable this example.");
}
