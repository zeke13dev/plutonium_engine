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
        title: "Text Rendering Example".to_string(),
        width: 800,
        height: 600,
    };

    let mut text2d: Option<Text2D> = None;
    let mut debug_shape: Option<Shape> = None;

    run_app(config, move |engine, _frame| {
        // Initialize text objects on first frame
        if text2d.is_none() {
            // Load the font (absolute path from crate root)
            let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));
            match engine.load_font(&font_path, 50.0, "roboto") {
                Ok(_) => (),
                Err(FontError::IoError(err)) => println!("I/O error occurred: {}", err),
                Err(FontError::InvalidFontData) => println!("Invalid font data"),
                Err(FontError::AtlasRenderError) => println!("Atlas render error occurred"),
            }

            // Create text with the specified font
            let text_position = Position { x: 60.0, y: 60.0 };
            let mut t = engine.create_text2d(
                "Left/Center demo\nAligned across lines",
                "roboto",
                36.0,
                text_position,
            );
            // Give it a visible container and alignment; normalize left margin by layout logic
            let container = TextContainer::new(Rectangle::new(
                text_position.x,
                text_position.y,
                480.0,
                180.0,
            ))
            .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
            .with_line_height_mul(1.1)
            .with_padding(16.0);
            t.set_container(container);
            text2d = Some(t);

            // Create debug visualization if text was created successfully
            if let Some(text) = &text2d {
                debug_shape = Some(text.create_debug_visualization(engine));
            }
        }

        // Clear previous frame
        engine.clear_render_queue();

        // Update engine state
        engine.update(None, &None);

        // Render text and debug shape
        if let Some(text) = &text2d {
            text.render(engine);
            if let Some(debug) = &debug_shape {
                debug.render(engine);
            }
        }

        // Render everything
        engine.render().unwrap();
    })?;

    Ok(())
}
