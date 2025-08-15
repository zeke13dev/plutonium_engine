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

    run_app(config, move |engine, frame| {
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

        // Immediate-mode background
        engine.begin_frame();
        // Draw a simple field background for clarity
        use plutonium_engine::utils::Rectangle;
        let rect = Rectangle::new(20.0, 20.0, 360.0, 80.0);
        engine.draw_rect(
            rect,
            [0.16, 0.17, 0.22, 1.0],
            8.0,
            Some(([0.10, 0.11, 0.14, 1.0], 1.0)),
            0,
        );

        // Render the retained text input (handles IME commit via app event)
        if let Some(text_input) = &text_input {
            text_input.render(engine);
        }

        // Simple clipboard: Ctrl/Cmd+C copies current text commits of this frame; Ctrl/Cmd+V logs paste
        // (Demo only; real integration should wire to the widget's content.)
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            // Primitive key detection: check last pressed key text commits or key list
            if frame
                .pressed_keys
                .iter()
                .any(|k| format!("{:?}", k).contains("KeyC"))
            {
                let to_copy = frame.text_commits.join("");
                let _ = clipboard.set_text(to_copy);
            }
            if frame
                .pressed_keys
                .iter()
                .any(|k| format!("{:?}", k).contains("KeyV"))
            {
                if let Ok(pasted) = clipboard.get_text() {
                    eprintln!("pasted: {}", pasted);
                }
            }
        }

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
