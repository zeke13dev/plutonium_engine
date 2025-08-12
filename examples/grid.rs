/// THIS IS AN EXAMPLE USING THE OLD STEUP WHICH IS STILL COMPATIBLE AND ALLOWS MORE CONTROL
use plutonium_engine::{pluto_objects::texture_2d::Texture2D, utils::Position, PlutoniumEngine};
use std::sync::Arc;
use wgpu::Surface;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct TextureSvgExample<'a> {
    window: Option<Arc<Window>>,
    engine: Option<PlutoniumEngine<'a>>,
    tiles: Vec<(Texture2D, Position)>, // Store both texture objects and their positions
    _surface: Option<Surface<'a>>,
}

impl<'a> TextureSvgExample<'a> {
    pub fn new() -> Self {
        Self {
            window: None,
            _surface: None,
            engine: None,
            tiles: Vec::new(), // We'll populate this when engine is initialized
        }
    }
}

impl<'a> ApplicationHandler<()> for TextureSvgExample<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes =
            Window::default_attributes().with_title("Flush Grid with Stroke SVG Example");

        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window_arc = Arc::new(window);
            let size = window_arc.as_ref().inner_size();
            let surface = instance.create_surface(window_arc.clone()).unwrap();
            let scale_factor = window_arc.scale_factor() as f32;
            let mut engine = PlutoniumEngine::new(surface, instance, size, scale_factor);

            // Create textures for each position
            let square_size = 100.0;
            let stroke_width = 5.0;
            let total_size = square_size + stroke_width;
            let positions = vec![
                Position { x: 0.0, y: 0.0 },
                Position {
                    x: total_size,
                    y: 0.0,
                },
                Position {
                    x: 0.0,
                    y: total_size,
                },
                Position {
                    x: total_size,
                    y: total_size,
                },
            ];

            // Create a texture for each position
            let tiles: Vec<(Texture2D, Position)> = positions
                .into_iter()
                .map(|pos| {
                    let texture = engine.create_texture_2d("examples/media/square.svg", pos, 1.0);
                    (texture, pos)
                })
                .collect();

            window_arc.request_redraw();

            self.tiles = tiles;
            self.engine = Some(engine);
            self.window = Some(window_arc);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(new_size) => {
                if let Some(engine) = &mut self.engine {
                    engine.resize(&new_size)
                }
                self.window.as_ref().unwrap().request_redraw();
            }

            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(engine) = &mut self.engine {
                    engine.clear_render_queue();
                    engine.update(None, &None);

                    // Render all tiles
                    for (tile, position) in &self.tiles {
                        tile.set_pos(*position); // Update position
                        tile.render(engine); // Render the tile
                    }

                    engine.render().unwrap();
                }
            }
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new().unwrap();
    let mut app = TextureSvgExample::new();

    match event_loop.run_app(&mut app) {
        Ok(_) => println!("Application terminated gracefully."),
        Err(e) => eprintln!("Error running application: {:?}", e),
    }

    Ok(())
}
