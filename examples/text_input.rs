use plutonium_engine::{
    app::run_app, pluto_objects::text_input::TextInput, utils::Position, WindowConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Text Input Example".to_string(),
        width: 800,
        height: 600,
    };

    let mut text_input: Option<TextInput> = None;

    run_app(config, move |engine, _frame| {
        // Initialize text input on first frame
        if text_input.is_none() {
            // Load font
            engine
                .load_font("examples/media/roboto.ttf", 20.0, "roboto")
                .ok();

            // Create text input
            text_input = Some(engine.create_text_input(
                "examples/media/input.svg",
                "roboto",
                20.0,
                Position::default(),
                1.0,
            ));
        }

        // Clear and render
        engine.clear_render_queue();

        if let Some(text_input) = &text_input {
            text_input.render(engine);
        }

        engine.render().unwrap();
    })?;

    Ok(())
}
