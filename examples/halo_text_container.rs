use plutonium_engine::app::run_app;
use plutonium_engine::pluto_objects::text2d::{
    HorizontalAlignment, Text2D, TextContainer, VerticalAlignment,
};
use plutonium_engine::utils::{Position, Rectangle};
use plutonium_engine::{HaloMode, HaloStyle, PlutoniumEngine, WindowConfig};

fn queue_label(engine: &mut PlutoniumEngine, text: &str, pos: Position) {
    let container = TextContainer::new(Rectangle::new(pos.x, pos.y, 420.0, 24.0)).with_padding(0.0);
    engine.queue_text_with_spacing(
        text,
        "roboto",
        pos,
        &container,
        0.0,
        0.0,
        80,
        [0.88, 0.92, 1.0, 1.0],
        Some(20.0),
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Halo Text Container".to_string(),
        width: 960,
        height: 540,
    };

    let mut initialized = false;
    let mut elapsed = 0.0_f32;
    let mut glow_text: Option<Text2D> = None;
    let mut border_text: Option<Text2D> = None;
    let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));

    run_app(config, move |engine, frame, _app| {
        elapsed += frame.delta_time;

        if !initialized {
            if engine.load_font(&font_path, 28.0, "roboto").is_err() {
                panic!("failed to load demo font");
            }
            initialized = true;
        }

        let glow_rect = Rectangle::new(80.0, 170.0, 360.0, 210.0);
        let border_rect = Rectangle::new(520.0, 170.0, 360.0, 210.0);
        let glow_container = TextContainer::new(glow_rect)
            .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
            .with_padding(18.0);
        let border_container = TextContainer::new(border_rect)
            .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
            .with_padding(18.0);

        if glow_text.is_none() {
            let mut text = engine.create_text2d(
                "Glow halo around\nthis TextContainer",
                "roboto",
                32.0,
                Position {
                    x: glow_rect.x,
                    y: glow_rect.y,
                },
            );
            text.set_container(glow_container.clone());
            text.set_z(40);
            text.set_color([0.95, 0.97, 1.0, 1.0]);
            glow_text = Some(text);
        }

        if border_text.is_none() {
            let mut text = engine.create_text2d(
                "Border-only highlight\nfor this container",
                "roboto",
                32.0,
                Position {
                    x: border_rect.x,
                    y: border_rect.y,
                },
            );
            text.set_container(border_container.clone());
            text.set_z(40);
            text.set_color([0.95, 0.97, 1.0, 1.0]);
            border_text = Some(text);
        }

        engine.begin_frame();
        engine.draw_rect(
            Rectangle::new(0.0, 0.0, 960.0, 540.0),
            [0.08, 0.09, 0.11, 1.0],
            0.0,
            None,
            0,
        );
        queue_label(
            engine,
            "Glow Variant (smooth GPU glow, single draw call)",
            Position { x: 80.0, y: 130.0 },
        );
        queue_label(
            engine,
            "Border-Only Variant (GPU border highlight)",
            Position { x: 520.0, y: 130.0 },
        );
        engine.draw_rect(glow_rect, [0.12, 0.14, 0.18, 1.0], 12.0, None, 1);
        engine.draw_rect(
            border_rect,
            [0.08, 0.09, 0.11, 1.0],
            12.0,
            Some(([0.18, 0.21, 0.27, 1.0], 1.0)),
            1,
        );

        // Soft glow mode: single draw call with smooth Gaussian falloff
        let glow_halo = HaloStyle {
            color: [0.40, 0.78, 1.0, 1.0],
            radius: 64.0,
            max_alpha: 0.35,
            corner_radius: 12.0,
            inner_padding: 0.0,
            pulse_amplitude: 0.16,
            pulse_speed_hz: 1.8,
            time_seconds: elapsed,
            mode: HaloMode::Glow,
            z: 12,
            ..HaloStyle::default()
        };
        engine.draw_halo(glow_rect, glow_halo);

        // Border-only highlight: single draw call with narrow edge glow
        let border_halo = HaloStyle {
            color: [0.58, 0.86, 1.0, 1.0],
            radius: 18.0,
            max_alpha: 0.55,
            corner_radius: 14.0,
            inner_padding: 0.0,
            pulse_amplitude: 0.22,
            pulse_speed_hz: 2.2,
            time_seconds: elapsed,
            mode: HaloMode::Border,
            border_width: 3.0,
            z: 12,
            ..HaloStyle::default()
        };
        engine.draw_halo(border_rect, border_halo);

        if let Some(text) = &glow_text {
            text.render(engine);
        }
        if let Some(text) = &border_text {
            text.render(engine);
        }

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
