use plutonium_engine::{
    app::run_app,
    pluto_objects::{shapes::Shape, text2d::Text2D},
    text::FontError,
    utils::Position,
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
            // Load the font
            match engine.load_font("examples/media/roboto.ttf", 50.0, "roboto") {
                Ok(_) => (),
                Err(FontError::IoError(err)) => println!("I/O error occurred: {}", err),
                Err(FontError::InvalidFontData) => println!("Invalid font data"),
                Err(FontError::AtlasRenderError) => println!("Atlas render error occurred"),
            }

            // Create text with the specified font
            let text_position = Position { x: 100.0, y: 100.0 };
            text2d = Some(engine.create_text2d(
                "Hello, World!\nNew Line",
                "roboto",
                50.0,
                text_position,
            ));

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
