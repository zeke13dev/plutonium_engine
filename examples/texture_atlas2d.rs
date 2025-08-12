use plutonium_engine::{
    app::run_app,
    pluto_objects::texture_atlas_2d::TextureAtlas2D,
    utils::{Position, Size},
    WindowConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Texture Atlas Example".to_string(),
        width: 800,
        height: 600,
    };

    let mut atlas: Option<TextureAtlas2D> = None;
    let scale_factor = 0.5;

    run_app(config, move |engine, _frame| {
        // Initialize atlas on first frame
        if atlas.is_none() {
            atlas = Some(engine.create_texture_atlas_2d(
                "examples/media/map_atlas.svg",
                Position::default(),
                scale_factor,
                Size {
                    width: 512.0,
                    height: 512.0,
                },
            ));
        }

        // Clear the render queue before each frame
        engine.clear_render_queue();

        // Step in logical space equals the tile size. Scaling is applied by the renderer.
        let tile_step = Size {
            width: 512.0,
            height: 512.0,
        };

        // Queue the tiles from the atlas for rendering
        if let Some(atlas) = &atlas {
            // Render each tile in a 2x2 grid pattern
            // Top-left (tile 0)
            atlas.render_tile(engine, 0, Position { x: 0.0, y: 0.0 });
            // Top-right (tile 1)
            atlas.render_tile(
                engine,
                1,
                Position {
                    x: tile_step.width,
                    y: 0.0,
                },
            );
            // Bottom-left (tile 1) for checkerboard
            atlas.render_tile(
                engine,
                1,
                Position {
                    x: 0.0,
                    y: tile_step.height,
                },
            );
            // Bottom-right (tile 0) for checkerboard
            atlas.render_tile(
                engine,
                0,
                Position {
                    x: tile_step.width,
                    y: tile_step.height,
                },
            );
        }

        // Submit the render queue
        engine.render().unwrap();
    })?;

    Ok(())
}
