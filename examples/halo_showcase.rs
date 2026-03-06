use plutonium_engine::app::run_app;
use plutonium_engine::pluto_objects::text2d::TextContainer;
use plutonium_engine::utils::{Position, Rectangle};
use plutonium_engine::{HaloPreset, HaloStyle, PlutoniumEngine, WindowConfig};
use winit::keyboard::{Key, NamedKey};

fn key_char_down(keys: &[Key], value: &str) -> bool {
    keys.iter()
        .any(|k| matches!(k.as_ref(), Key::Character(s) if s.eq_ignore_ascii_case(value)))
}

fn key_named_down(keys: &[Key], value: NamedKey) -> bool {
    keys.iter()
        .any(|k| matches!(k.as_ref(), Key::Named(named) if named == value))
}

fn queue_label(engine: &mut PlutoniumEngine, text: &str, pos: Position, color: [f32; 4]) {
    let container = TextContainer::new(Rectangle::new(pos.x, pos.y, 880.0, 24.0)).with_padding(0.0);
    engine.queue_text_with_spacing(
        text,
        "roboto",
        pos,
        &container,
        0.0,
        0.0,
        40,
        color,
        Some(18.0),
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Halo Showcase".to_string(),
        width: 960,
        height: 540,
    };

    let mut initialized = false;
    let mut elapsed = 0.0_f32;

    let mut focus_target = None;
    let mut offscreen_target = None;
    let mut focus_pos = Position { x: 190.0, y: 220.0 };
    let mut offscreen_visible = false;
    let mut toggle_offscreen_prev = false;
    let mut cycle_preset_prev = false;
    let mut active_preset = HaloPreset::TutorialPrimary;

    let mut focus_halo_drawn = false;
    let mut offscreen_halo_drawn = false;

    let object_svg = format!("{}/examples/media/button.svg", env!("CARGO_MANIFEST_DIR"));
    let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));
    let screen_rect_target = Rectangle::new(560.0, 200.0, 220.0, 120.0);

    run_app(config, move |engine, frame, _app| {
        elapsed += frame.delta_time;

        if !initialized {
            if engine.load_font(&font_path, 24.0, "roboto").is_err() {
                panic!("failed to load demo font");
            }

            focus_target = Some(engine.create_texture_2d(&object_svg, focus_pos, 1.0));
            // Starts fully offscreen to verify draw_halo_for_object -> false.
            offscreen_target = Some(engine.create_texture_2d(
                &object_svg,
                Position {
                    x: 1200.0,
                    y: 220.0,
                },
                1.0,
            ));
            initialized = true;
        }

        let move_speed = 260.0 * frame.delta_time.max(1.0 / 120.0);
        if key_named_down(&frame.pressed_keys, NamedKey::ArrowLeft)
            || key_char_down(&frame.pressed_keys, "a")
        {
            focus_pos.x -= move_speed;
        }
        if key_named_down(&frame.pressed_keys, NamedKey::ArrowRight)
            || key_char_down(&frame.pressed_keys, "d")
        {
            focus_pos.x += move_speed;
        }
        if key_named_down(&frame.pressed_keys, NamedKey::ArrowUp)
            || key_char_down(&frame.pressed_keys, "w")
        {
            focus_pos.y -= move_speed;
        }
        if key_named_down(&frame.pressed_keys, NamedKey::ArrowDown)
            || key_char_down(&frame.pressed_keys, "s")
        {
            focus_pos.y += move_speed;
        }
        focus_pos.x = focus_pos.x.clamp(40.0, 460.0);
        focus_pos.y = focus_pos.y.clamp(140.0, 420.0);

        let toggle_offscreen_down = key_char_down(&frame.pressed_keys, "o");
        if toggle_offscreen_down && !toggle_offscreen_prev {
            offscreen_visible = !offscreen_visible;
        }
        toggle_offscreen_prev = toggle_offscreen_down;

        let cycle_preset_down = key_char_down(&frame.pressed_keys, "p");
        if cycle_preset_down && !cycle_preset_prev {
            active_preset = match active_preset {
                HaloPreset::TutorialPrimary => HaloPreset::TutorialSubtle,
                HaloPreset::TutorialSubtle => HaloPreset::TutorialUrgent,
                HaloPreset::TutorialUrgent => HaloPreset::TutorialPrimary,
            };
        }
        cycle_preset_prev = cycle_preset_down;

        engine.begin_frame();

        engine.draw_rect(
            Rectangle::new(0.0, 0.0, 960.0, 540.0),
            [0.07, 0.08, 0.10, 1.0],
            0.0,
            None,
            0,
        );
        engine.draw_rect(
            Rectangle::new(24.0, 24.0, 912.0, 92.0),
            [0.12, 0.14, 0.18, 1.0],
            8.0,
            Some(([0.22, 0.26, 0.32, 1.0], 1.5)),
            1,
        );
        engine.draw_rect(
            Rectangle::new(80.0, 160.0, 360.0, 300.0),
            [0.12, 0.12, 0.14, 1.0],
            10.0,
            Some(([0.20, 0.22, 0.26, 1.0], 1.0)),
            1,
        );
        engine.draw_rect(
            Rectangle::new(520.0, 160.0, 360.0, 300.0),
            [0.12, 0.12, 0.14, 1.0],
            10.0,
            Some(([0.20, 0.22, 0.26, 1.0], 1.0)),
            1,
        );

        if let Some(target) = &focus_target {
            target.set_pos(focus_pos);
            target.render_with_z(engine, 12);

            let mut style = HaloStyle::from_preset(active_preset);
            style.time_seconds = elapsed;
            style.inner_padding = 3.0;
            focus_halo_drawn = engine.draw_halo_for_object(&target.get_id(), style);
        }

        engine.draw_rect(
            screen_rect_target,
            [0.20, 0.24, 0.30, 1.0],
            8.0,
            Some(([0.27, 0.33, 0.42, 1.0], 1.0)),
            10,
        );
        let mut screen_space_style = HaloStyle::from_preset(HaloPreset::TutorialSubtle);
        screen_space_style.time_seconds = elapsed;
        screen_space_style.radius = 48.0;
        engine.draw_halo(screen_rect_target, screen_space_style);

        if let Some(target) = &offscreen_target {
            let offscreen_pos = if offscreen_visible {
                Position { x: 660.0, y: 350.0 }
            } else {
                Position {
                    x: 1300.0,
                    y: 350.0,
                }
            };
            target.set_pos(offscreen_pos);
            target.render_with_z(engine, 12);

            let mut style = HaloStyle::from_preset(HaloPreset::TutorialUrgent);
            style.time_seconds = elapsed;
            style.radius = 56.0;
            offscreen_halo_drawn = engine.draw_halo_for_object(&target.get_id(), style);
        }

        let focus_indicator_color = if focus_halo_drawn {
            [0.18, 0.82, 0.40, 1.0]
        } else {
            [0.86, 0.20, 0.22, 1.0]
        };
        let offscreen_indicator_color = if offscreen_halo_drawn {
            [0.18, 0.82, 0.40, 1.0]
        } else {
            [0.86, 0.20, 0.22, 1.0]
        };
        engine.draw_rect(
            Rectangle::new(34.0, 78.0, 12.0, 12.0),
            focus_indicator_color,
            2.0,
            None,
            50,
        );
        engine.draw_rect(
            Rectangle::new(34.0, 100.0, 12.0, 12.0),
            offscreen_indicator_color,
            2.0,
            None,
            50,
        );

        let preset_name = match active_preset {
            HaloPreset::TutorialPrimary => "TutorialPrimary",
            HaloPreset::TutorialSubtle => "TutorialSubtle",
            HaloPreset::TutorialUrgent => "TutorialUrgent",
        };
        queue_label(
            engine,
            "Arrow Keys / WASD: move left target   P: cycle halo preset   O: toggle offscreen object",
            Position { x: 52.0, y: 36.0 },
            [0.92, 0.94, 0.98, 1.0],
        );
        queue_label(
            engine,
            &format!(
                "focus halo draw_halo_for_object returned: {}",
                focus_halo_drawn
            ),
            Position { x: 52.0, y: 72.0 },
            [0.80, 0.86, 0.96, 1.0],
        );
        queue_label(
            engine,
            &format!(
                "offscreen halo draw_halo_for_object returned: {} (should be false when hidden)",
                offscreen_halo_drawn
            ),
            Position { x: 52.0, y: 94.0 },
            [0.80, 0.86, 0.96, 1.0],
        );
        queue_label(
            engine,
            &format!("active preset: {}", preset_name),
            Position { x: 52.0, y: 116.0 },
            [0.96, 0.88, 0.62, 1.0],
        );
        queue_label(
            engine,
            "Left panel: object-space halo via draw_halo_for_object | Right panel: screen-space halo via draw_halo(Rect)",
            Position { x: 80.0, y: 472.0 },
            [0.72, 0.78, 0.88, 1.0],
        );

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
