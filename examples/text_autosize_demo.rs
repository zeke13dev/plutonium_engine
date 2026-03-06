use plutonium_engine::{
    app::run_app,
    pluto_objects::{
        shapes::Shape,
        text2d::{HorizontalAlignment, Text2D, TextContainer, VerticalAlignment},
    },
    text::FontError,
    utils::{Position, Rectangle},
    WindowConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Auto-Size Text Demo".to_string(),
        width: 800,
        height: 600,
    };

    let mut small_container_text: Option<Text2D> = None;
    let mut large_container_text: Option<Text2D> = None;
    let mut debug_shapes: Vec<Shape> = Vec::new();

    run_app(config, move |engine, frame, _app| {
        if small_container_text.is_none() {
            let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));
            match engine.load_font(&font_path, 50.0, "roboto") {
                Ok(_) => println!("Font loaded successfully"),
                Err(FontError::IoError(err)) => println!("I/O error occurred: {}", err),
                Err(FontError::InvalidFontData) => println!("Invalid font data"),
                Err(FontError::AtlasRenderError) => println!("Atlas render error occurred"),
                Err(other) => println!("Font load error: {:?}", other),
            }

            // Example 1: Small container with auto-sizing
            // Text will shrink to fit the small space
            let small_pos = Position { x: 50.0, y: 50.0 };
            let small_container = TextContainer::new(Rectangle::new(
                small_pos.x,
                small_pos.y,
                200.0, // Small width
                80.0,  // Small height
            ))
            .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle);

            let mut small_text = engine.create_text2d(
                "Tiny Box", "roboto", 32.0, // Initial size (will be adjusted)
                small_pos,
            );
            small_text.set_container(small_container);
            small_text.set_auto_size(true); // Enable auto-sizing
            small_text.set_min_font_size(8.0); // Minimum allowed
            small_text.set_max_font_size(128.0); // Maximum allowed (NEW!)
            debug_shapes.push(small_text.create_debug_visualization(engine));
            small_container_text = Some(small_text);

            // Example 2: Large container with auto-sizing
            // Text will GROW to fill the large space (this is the fix!)
            let large_pos = Position { x: 50.0, y: 200.0 };
            let large_container = TextContainer::new(Rectangle::new(
                large_pos.x,
                large_pos.y,
                700.0, // Large width
                350.0, // Large height
            ))
            .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle);

            let mut large_text = engine.create_text2d(
                "BIG TEXT\nAuto-Sized!",
                "roboto",
                32.0, // Small initial size, but will GROW to fill space
                large_pos,
            );
            large_text.set_container(large_container);
            large_text.set_auto_size(true); // Enable auto-sizing
            large_text.set_min_font_size(8.0); // Minimum allowed
            large_text.set_max_font_size(256.0); // Large maximum - text will grow!
            large_text.set_color([0.3, 0.8, 0.3, 1.0]); // Green
            debug_shapes.push(large_text.create_debug_visualization(engine));
            large_container_text = Some(large_text);
        }

        engine.clear_render_queue();
        engine.update(None, &None, frame.delta_time);

        // Render text and debug shapes
        if let Some(text) = &small_container_text {
            text.render(engine);
        }
        if let Some(text) = &large_container_text {
            text.render(engine);
        }
        for shape in &debug_shapes {
            shape.render(engine);
        }

        engine.render().unwrap();
    })?;

    Ok(())
}
