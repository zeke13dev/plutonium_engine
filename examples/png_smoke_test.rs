use plutonium_engine::{
    app::run_app,
    utils::{Position, Rectangle},
    DrawParams, TextureFit, WindowConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "PNG Smoke Test - Parrot".to_string(),
        width: 1200,
        height: 900,
    };

    #[allow(unused_mut)]
    let mut parrot_tex: Option<(uuid::Uuid, Rectangle)> = None;

    run_app(config, move |engine, _frame, _app| {
        // Load the PNG on the first frame
        if parrot_tex.is_none() {
            #[cfg(feature = "raster")]
            {
                let (id, dims) = engine.create_texture_raster_from_path(
                    "examples/media/parrot.png",
                    Position { x: 0.0, y: 0.0 },
                );
                parrot_tex = Some((id, dims));
                println!("Loaded parrot.png: {:?}", dims);
            }
            #[cfg(not(feature = "raster"))]
            {
                panic!("Raster feature is required for this smoke test!");
            }
        }

        engine.begin_frame();

        if let Some((id, _dims)) = parrot_tex {
            // 1. Normal Rendering (Original Size)
            engine.draw_texture(
                &id,
                Position { x: 50.0, y: 50.0 },
                DrawParams {
                    z: 0,
                    scale: 0.5, // Scaling down slightly because the image might be large
                    rotation: 0.0,
                    tint: [1.0, 1.0, 1.0, 1.0],
                },
            );

            // 2. Scaled Up
            engine.draw_texture(
                &id,
                Position { x: 400.0, y: 50.0 },
                DrawParams {
                    z: 0,
                    scale: 0.8,
                    rotation: 0.0,
                    tint: [1.0, 1.0, 1.0, 1.0],
                },
            );

            // 3. Scaled Down (Tiny)
            engine.draw_texture(
                &id,
                Position { x: 50.0, y: 500.0 },
                DrawParams {
                    z: 0,
                    scale: 0.1,
                    rotation: 0.0,
                    tint: [1.0, 1.0, 1.0, 1.0],
                },
            );

            // 4. Wrapped in a container (Stretched/Fit)
            // Define a container rectangle
            let container = Rectangle::new(400.0, 450.0, 300.0, 400.0);

            // Draw a background rect to see the container bounds
            engine.draw_rect(
                container,
                [0.2, 0.2, 0.2, 1.0],              // Fill
                5.0,                               // Corner radius
                Some(([1.0, 1.0, 1.0, 1.0], 1.0)), // Border (color, thickness)
                -1,                                // Z layer (behind)
            );

            // Fit: Contain
            engine.queue_texture_stretched_with_layer_and_fit(
                &id,
                container,
                0,
                TextureFit::Contain,
                0.0,
            );

            // Another container for StretchFill
            let container2 = Rectangle::new(750.0, 450.0, 300.0, 400.0);
            engine.draw_rect(
                container2,
                [0.2, 0.2, 0.2, 1.0],
                5.0,
                Some(([1.0, 1.0, 1.0, 1.0], 1.0)),
                -1,
            );
            engine.queue_texture_stretched_with_layer_and_fit(
                &id,
                container2,
                0,
                TextureFit::StretchFill,
                0.0,
            );
            // 5. Cover mode (Fills the container, might crop)
            let container3 = Rectangle::new(50.0, 650.0, 300.0, 200.0);
            engine.draw_rect(
                container3,
                [0.2, 0.2, 0.2, 1.0],
                5.0,
                Some(([1.0, 1.0, 1.0, 1.0], 1.0)),
                -1,
            );
            engine.draw_texture_stretched_with_fit_and_inset(
                &id,
                container3,
                TextureFit::Cover,
                0.0,
                0,
            );

            // 6. Contain with Inset (Uniform Padding)
            let container4 = Rectangle::new(400.0, 650.0, 300.0, 200.0);
            engine.draw_rect(
                container4,
                [0.2, 0.2, 0.2, 1.0],
                5.0,
                Some(([1.0, 1.0, 1.0, 1.0], 1.0)),
                -1,
            );
            engine.draw_texture_stretched_with_fit_and_inset(
                &id,
                container4,
                TextureFit::Contain,
                20.0, // 20px uniform padding
                0,
            );

            // 7. StretchFill with Inset
            let container5 = Rectangle::new(750.0, 650.0, 300.0, 200.0);
            engine.draw_rect(
                container5,
                [0.2, 0.2, 0.2, 1.0],
                5.0,
                Some(([1.0, 1.0, 1.0, 1.0], 1.0)),
                -1,
            );
            engine.draw_texture_stretched_with_fit_and_inset(
                &id,
                container5,
                TextureFit::StretchFill,
                20.0,
                0,
            );
        }

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
