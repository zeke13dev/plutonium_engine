use std::sync::Arc;

use plutonium_engine::{utils::Position, PlutoniumEngine};
use wgpu::Surface;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::Key,
    window::{Window, WindowId},
};

struct TextureSvgExample<'a> {
    window: Option<Arc<Window>>,
    engine: Option<PlutoniumEngine<'a>>,
    svg_positions: Vec<Position>,
    _surface: Option<Surface<'a>>,
}

impl<'a> TextureSvgExample<'a> {
    pub fn new() -> Self {
        let square_size = 100.0;
        let stroke_width = 5.0;
        let total_size = square_size + stroke_width; // Effective size considering stroke width

        // Adjust the positions so the squares are flush (touching) each other
        let svg_positions = vec![
            Position { x: 0.0, y: 0.0 }, // Top-left
            Position {
                x: total_size,
                y: 0.0,
            }, // Top-right
            Position {
                x: 0.0,
                y: total_size,
            }, // Bottom-left
            Position {
                x: total_size,
                y: total_size,
            }, // Bottom-right
        ];

        Self {
            window: None,
            _surface: None,
            engine: None,
            svg_positions,
        }
    }
}

impl<'a> ApplicationHandler<()> for TextureSvgExample<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create the window safely with proper error handling
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes =
            Window::default_attributes().with_title("Flush Grid with Stroke SVG Example");
        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window_arc = Arc::new(window);
            let size = window_arc.as_ref().inner_size();
            let surface = instance.create_surface(window_arc.clone()).unwrap();
            let mut engine = PlutoniumEngine::new(surface, instance, size);

            // Create the SVG texture once
            engine.create_texture_svg(
                "tile_texture", // Use a single texture key
                "examples/media/square.svg",
                Position { x: 0.0, y: 0.0 }, // Position doesn't matter here; we'll set it when queuing
                1.0,
                None,
            );

            window_arc.request_redraw();

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
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(engine) = &mut self.engine {
                    engine.clear_render_queue();
                    engine.update(None, &None);

                    // Queue the same texture at different positions
                    for position in &self.svg_positions {
                        engine.queue_texture("tile_texture", Some(*position));
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
