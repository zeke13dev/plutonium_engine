use std::sync::Arc;

use plutonium::{
    utils::{Position, Size},
    PlutoniumEngine,
};
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
    _surface: Option<Surface<'a>>,
}

impl<'a> TextureSvgExample<'a> {
    pub fn new() -> Self {
        Self {
            window: None,
            _surface: None,
            engine: None,
        }
    }
}

impl<'a> ApplicationHandler<()> for TextureSvgExample<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create the window safely with proper error handling
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes = Window::default_attributes().with_title("Texture Atlas Example");
        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window_arc = Arc::new(window);
            let size = window_arc.as_ref().inner_size();
            let surface = instance.create_surface(window_arc.clone()).unwrap();
            let mut engine = PlutoniumEngine::new(surface, instance, size);

            // actual example stuff
            engine.create_texture_svg(
                "atlas",
                "examples/media/map_atlas.svg",
                Position { x: 0.0, y: 0.0 },
                1.0,
                Some(Size {
                    width: 512.0,
                    height: 512.0,
                }),
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
                let _window = self.window.as_ref();
                if let Some(engine) = &mut self.engine {
                    // Clear the queue before each frame
                    engine.clear_render_queue();
                    engine.update();

                    // Queue the tiles for rendering
                    engine.queue_tile("atlas", 0, Position { x: 0.0, y: 0.0 });
                    engine.queue_tile("atlas", 1, Position { x: 512.0, y: 0.0 });
                    engine.queue_tile("atlas", 0, Position { x: 512.0, y: 512.0 });
                    engine.queue_tile("atlas", 1, Position { x: 0.0, y: 512.0 });

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
