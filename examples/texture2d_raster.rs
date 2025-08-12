use plutonium_engine::{app::run_app, utils::Position, DrawParams, WindowConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig { title: "Raster Example".to_string(), width: 800, height: 600 };
    let mut tex_id = None;

    run_app(config, move |engine, _frame| {
        if tex_id.is_none() {
            #[cfg(feature = "raster")]
            let (id, _dims) = engine.create_texture_raster_from_path("examples/media/drawing.svg", Position { x: 100.0, y: 100.0 });
            #[cfg(not(feature = "raster"))]
            let (id, _dims) = engine.create_texture_svg("examples/media/square.svg", Position { x: 100.0, y: 100.0 }, 1.0);
            tex_id = Some(id);
        }
        engine.begin_frame();
        if let Some(id) = tex_id {
            engine.draw_texture(&id, Position { x: 100.0, y: 100.0 }, DrawParams { z: 0, scale: 1.0, rotation: 0.0 });
        }
        engine.end_frame().unwrap();
    })?;

    Ok(())
}


