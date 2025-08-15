use plutonium_engine::{
    app::run_app,
    pluto_objects::text2d::{HorizontalAlignment, TextContainer, VerticalAlignment},
    utils::{Position, Rectangle},
    PlutoniumEngine, WindowConfig,
};

fn draw_demo(engine: &mut PlutoniumEngine) {
    // Left/Center/Right across Top/Middle/Bottom, with visible bounds
    let w = engine.size.width as f32;
    let h = engine.size.height as f32;
    let cell_w = w / 3.0;
    let cell_h = h / 3.0;
    let labels = [
        (
            HorizontalAlignment::Left,
            VerticalAlignment::Top,
            "Left/Top",
        ),
        (
            HorizontalAlignment::Center,
            VerticalAlignment::Top,
            "Center/Top",
        ),
        (
            HorizontalAlignment::Right,
            VerticalAlignment::Top,
            "Right/Top",
        ),
        (
            HorizontalAlignment::Left,
            VerticalAlignment::Middle,
            "Left/Middle",
        ),
        (
            HorizontalAlignment::Center,
            VerticalAlignment::Middle,
            "Center/Middle",
        ),
        (
            HorizontalAlignment::Right,
            VerticalAlignment::Middle,
            "Right/Middle",
        ),
        (
            HorizontalAlignment::Left,
            VerticalAlignment::Bottom,
            "Left/Bottom",
        ),
        (
            HorizontalAlignment::Center,
            VerticalAlignment::Bottom,
            "Center/Bottom",
        ),
        (
            HorizontalAlignment::Right,
            VerticalAlignment::Bottom,
            "Right/Bottom",
        ),
    ];
    for (i, (h_align, v_align, label)) in labels.iter().enumerate() {
        let cx = (i as i32 % 3) as f32;
        let cy = (i as i32 / 3) as f32;
        let rect = Rectangle::new(
            cx * cell_w + 16.0,
            cy * cell_h + 16.0,
            cell_w - 32.0,
            cell_h - 32.0,
        );
        let shape = engine.create_rect(
            rect,
            rect.pos(),
            "rgba(0,0,0,0.0)".to_string(),
            "rgba(0,255,0,0.6)".to_string(),
            1.0,
        );
        shape.render(engine);
        let pos = rect.pos();
        let container = TextContainer::new(rect)
            .with_alignment(*h_align, *v_align)
            .with_line_height_mul(1.1)
            .with_padding(16.0);
        // Avoid double alignment: neutralize inner layout alignment and pass computed origin
        let mut neutral = container.clone();
        neutral.h_align = HorizontalAlignment::Left;
        neutral.v_align = VerticalAlignment::Top;
        engine.queue_text(
            &format!("{}\nSample text", label),
            "roboto",
            Position { x: pos.x, y: pos.y },
            &container,
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Text Alignment Demo".to_string(),
        width: 900,
        height: 600,
    };
    run_app(config, move |engine, _| {
        engine.clear_render_queue();
        // Ensure font once
        let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));
        if engine.load_font(&font_path, 20.0, "roboto").is_err() {
            // If font fails, continue without drawing
        }
        draw_demo(engine);
        engine.render().unwrap();
    })?;
    Ok(())
}
