use plutonium_engine::{app::run_app, utils::Position, WindowConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "My Game".to_string(),
        width: 800,
        height: 600,
    };

    let mut player_pos = Position::default();
    let mut player = None;

    run_app(config, move |engine, frame, _app| {
        // Create game objects on first frame
        if player.is_none() {
            let Ok(texture) =
                engine.create_texture_2d("examples/media/player.svg", Position::default(), 1.0)
            else {
                eprintln!("failed to create player texture");
                return;
            };
            player = Some(texture);
        }

        // Handle input
        if frame.pressed_keys.contains_character_ignore_ascii_case("w") {
            player_pos.y -= 10.0;
        }
        if frame.pressed_keys.contains_character_ignore_ascii_case("s") {
            player_pos.y += 10.0;
        }
        if frame.pressed_keys.contains_character_ignore_ascii_case("a") {
            player_pos.x -= 10.0;
        }
        if frame.pressed_keys.contains_character_ignore_ascii_case("d") {
            player_pos.x += 10.0;
        }

        if let Some(player) = &mut player {
            player.set_pos(player_pos);
            engine.begin_frame();
            // Use immediate-mode draw with rotation example
            let _params = plutonium_engine::DrawParams {
                z: 0,
                scale: 1.0,
                rotation: 0.0,
                tint: [1.0, 1.0, 1.0, 1.0],
            };
            player.render(engine);
            engine.end_frame().unwrap();
        }
    })?;

    Ok(())
}
