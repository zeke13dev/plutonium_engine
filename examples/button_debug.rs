use plutonium_engine::{
    app::run_app,
    pluto_objects::button::Button,
    pluto_objects::text2d::{HorizontalAlignment, Text2D, TextContainer, VerticalAlignment},
    text::FontError,
    utils::{Position, Rectangle},
    WindowConfig,
};
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Button Debug (SVG)".to_string(),
        width: 800,
        height: 200,
    };

    let mut initialized = false;
    let mut button: Option<Button> = None;
    let mut rect_button_text: Option<Text2D> = None;
    let mut svg_manual: Option<(Uuid, Rectangle)> = None;
    let mut svg_manual_text: Option<Text2D> = None;

    let window_rect = Rectangle::new(0.0, 0.0, config.width as f32, config.height as f32);
    let rect_button = Rectangle::new(20.0, 90.0, 240.0, 60.0);

    run_app(config, move |engine, frame, _app| {
        if !initialized {
            let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));
            match engine.load_font(&font_path, 48.0, "roboto") {
                Ok(_) => (),
                Err(FontError::IoError(err)) => println!("I/O error occurred: {}", err),
                Err(FontError::InvalidFontData) => println!("Invalid font data"),
                Err(FontError::AtlasRenderError) => println!("Atlas render error occurred"),
                Err(other) => println!("Font load error: {:?}", other),
            }

            let svg_path = format!("{}/examples/media/button.svg", env!("CARGO_MANIFEST_DIR"));
            let b = engine.create_button(
                &svg_path,
                "Inventory",
                "roboto",
                16.0,
                Position { x: 20.0, y: 12.0 },
                1.0,
            );
            b.set_on_click(Some(Box::new(|| {
                println!("[button_debug] Clicked");
            })));
            button = Some(b);

            let mut text = engine.create_text2d("Rect Button", "roboto", 16.0, rect_button.pos());
            let container = TextContainer::new(rect_button)
                .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                .with_padding(0.0);
            text.set_container(container);
            text.set_z(5);
            rect_button_text = Some(text);

            let manual_pos = Position { x: 300.0, y: 12.0 };
            let (svg_key, svg_dims) = engine.create_texture_svg(&svg_path, manual_pos, 1.0);
            let manual_rect =
                Rectangle::new(manual_pos.x, manual_pos.y, svg_dims.width, svg_dims.height);
            let mut manual_text =
                engine.create_text2d("Manual SVG", "roboto", 16.0, manual_rect.pos());
            let manual_container = TextContainer::new(manual_rect)
                .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                .with_padding(0.0);
            manual_text.set_container(manual_container);
            manual_text.set_z(5);
            svg_manual = Some((svg_key, manual_rect));
            svg_manual_text = Some(manual_text);
            initialized = true;
        }

        engine.begin_frame();
        engine.draw_rect(window_rect, [0.08, 0.09, 0.10, 1.0], 0.0, None, 0);

        if let Some(btn) = &button {
            btn.update(Some(frame.mouse_info), None);
            btn.render(engine);
        }

        if let Some((svg_key, manual_rect)) = &svg_manual {
            engine.queue_texture_with_layer(svg_key, Some(manual_rect.pos()), 2);
            if let Some(text) = &svg_manual_text {
                text.render(engine);
            }
        }

        engine.draw_rect(
            rect_button,
            [0.18, 0.20, 0.24, 1.0],
            10.0,
            Some(([0.08, 0.09, 0.10, 1.0], 2.0)),
            2,
        );
        if let Some(text) = &rect_button_text {
            text.render(engine);
        }

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
