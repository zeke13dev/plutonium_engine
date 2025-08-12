use plutonium_engine::{app::run_app, utils::Position, WindowConfig};
use winit::keyboard::Key;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "My Game".to_string(),
        width: 800,
        height: 600,
    };

    let mut player_pos = Position::default();
    let mut player = None;

    run_app(config, move |engine, frame| {
        // Create game objects on first frame
        if player.is_none() {
            player = Some(engine.create_texture_2d(
                "examples/media/player.svg",
                Position::default(),
                1.0,
            ));
        }

        // Handle input
        for key in &frame.pressed_keys {
            match key.as_ref() {
                Key::Character("w") => player_pos.y -= 10.0,
                Key::Character("s") => player_pos.y += 10.0,
                Key::Character("a") => player_pos.x -= 10.0,
                Key::Character("d") => player_pos.x += 10.0,
                _ => (),
            }
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
