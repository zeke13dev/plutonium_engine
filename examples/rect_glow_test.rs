use plutonium_engine::app::run_app;
use plutonium_engine::utils::Rectangle;
use plutonium_engine::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Rect Glow Test".to_string(),
        width: 800,
        height: 600,
    };

    run_app(config, move |engine, _, _| {
        engine.begin_frame();

        // Draw some background to see transparency
        engine.draw_rect(
            Rectangle::new(0.0, 0.0, 800.0, 600.0),
            [0.1, 0.1, 0.1, 1.0],
            0.0,
            None,
            -1,
        );

        // 1. Neon Green Glow
        engine.draw_rect_glow(
            Rectangle::new(100.0, 100.0, 200.0, 150.0),
            [0.0, 1.0, 0.0, 1.0],
            2.0,  // thickness
            20.0, // glow_radius
            20.0, // corner_radius
            1.0,  // intensity
            0,    // z
        );

        // 2. Soft Blue Glow (no core line)
        engine.draw_rect_glow(
            Rectangle::new(400.0, 100.0, 200.0, 150.0),
            [0.0, 0.5, 1.0, 1.0],
            0.0,  // thickness
            40.0, // glow_radius
            50.0, // corner_radius
            0.8,  // intensity
            0,    // z
        );

        // 3. Sharp Red Border with tight glow
        engine.draw_rect_glow(
            Rectangle::new(100.0, 350.0, 200.0, 150.0),
            [1.0, 0.0, 0.0, 1.0],
            5.0,  // thickness
            10.0, // glow_radius
            0.0,  // corner_radius
            1.0,  // intensity
            0,    // z
        );

        // 4. White "Lamp" style
        engine.draw_rect_glow(
            Rectangle::new(400.0, 350.0, 200.0, 150.0),
            [1.0, 1.0, 1.0, 1.0],
            1.0,  // thickness
            60.0, // glow_radius
            75.0, // corner_radius
            0.5,  // intensity
            0,    // z
        );

        engine.end_frame().unwrap();
    })?;
    Ok(())
}
