use plutonium_engine::{
    app::run_app,
    pluto_objects::{texture_2d::Texture2D, texture_atlas_2d::TextureAtlas2D},
    utils::{Position, Rectangle, Size},
    WindowConfig,
};
use winit::keyboard::NamedKey;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Camera Example".to_string(),
        width: 800,
        height: 600,
    };

    let mut player_pos = Position::default();
    let mut player: Option<Texture2D> = None;
    let mut atlas: Option<TextureAtlas2D> = None;
    let mut boundary: Option<Texture2D> = None;
    let mut camera_enabled = true;
    let mut toggle_was_pressed = false;
    let scale_factor = 0.5;
    let move_speed = 260.0;

    run_app(config, move |engine, frame, app| {
        // Create game objects on first frame
        if player.is_none() {
            // Create texture atlas for the map
            atlas = Some(engine.create_texture_atlas_2d(
                "examples/media/map_atlas.svg",
                Position::default(),
                scale_factor,
                Size {
                    width: 512.0,
                    height: 512.0,
                },
            ));

            // Create player texture object
            player = Some(engine.create_texture_2d("examples/media/player.svg", player_pos, 0.3));

            // Create boundary texture object
            boundary = Some(engine.create_texture_2d(
                "examples/media/boundary.svg",
                Position::default(),
                2.0,
            ));

            // Set up camera and boundary
            if let Some(player) = &player {
                engine.set_camera_target(player.get_id());
            }
            let boundary_rect = Rectangle::new_square(0.0, 0.0, 200.0);
            engine.set_boundary(boundary_rect);
            engine.set_camera_smoothing(16.0);
        }

        // Camera mode toggle (debounced)
        let toggle_pressed = app.is_named_key_down(NamedKey::Space);
        if toggle_pressed && !toggle_was_pressed {
            camera_enabled = !camera_enabled;
            println!("camera_enabled={}", camera_enabled);
        }
        toggle_was_pressed = toggle_pressed;

        // Continuous, dt-based movement from held keys
        let mut input_x = 0.0f32;
        let mut input_y = 0.0f32;
        if app.is_char_key_down('w') {
            input_y -= 1.0;
        }
        if app.is_char_key_down('s') {
            input_y += 1.0;
        }
        if app.is_char_key_down('a') {
            input_x -= 1.0;
        }
        if app.is_char_key_down('d') {
            input_x += 1.0;
        }

        if input_x != 0.0 || input_y != 0.0 {
            let len = (input_x * input_x + input_y * input_y).sqrt();
            let dt = frame.delta_time.min(0.05);
            player_pos.x += (input_x / len) * move_speed * dt;
            player_pos.y += (input_y / len) * move_speed * dt;
        }

        // Update and render
        if let Some(player) = &player {
            player.set_pos(player_pos);

            engine.clear_render_queue();
            if camera_enabled {
                engine.activate_camera();
            } else {
                engine.deactivate_camera();
            }

            // Render atlas tiles
            if let Some(atlas) = &atlas {
                let scaled_tile_size = Size {
                    width: 512.0 * scale_factor,
                    height: 512.0 * scale_factor,
                };

                atlas.render_tile(engine, 0, Position::default());
                atlas.render_tile(
                    engine,
                    1,
                    Position {
                        x: scaled_tile_size.width,
                        y: 0.0,
                    },
                );
                atlas.render_tile(
                    engine,
                    0,
                    Position {
                        x: scaled_tile_size.width,
                        y: scaled_tile_size.height,
                    },
                );
                atlas.render_tile(
                    engine,
                    1,
                    Position {
                        x: 0.0,
                        y: scaled_tile_size.height,
                    },
                );
            }

            // Render player
            player.render(engine);

            // Render boundary
            engine.deactivate_camera();
            if let Some(boundary) = &boundary {
                boundary.render(engine);
            }

            engine.render().unwrap();
        }
    })?;

    Ok(())
}
