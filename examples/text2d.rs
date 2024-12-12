use std::sync::Arc;

use plutonium_engine::{
    pluto_objects::text2d::Text2D, traits::PlutoObject, utils::Position, PlutoniumEngine,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::Key,
    window::{Window, WindowId},
};

struct TextRenderingExample<'a> {
    window: Option<Arc<Window>>,
    engine: Option<PlutoniumEngine<'a>>,
    text2d: Option<Text2D>,
}

impl<'a> TextRenderingExample<'a> {
    pub fn new() -> Self {
        Self {
            window: None,
            engine: None,
            text2d: None,
        }
    }
}

impl<'a> ApplicationHandler<()> for TextRenderingExample<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create the window safely with proper error handling
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes = Window::default_attributes().with_title("Text Rendering Example");
        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window_arc = Arc::new(window);
            let size = window_arc.as_ref().inner_size();
            let surface = instance.create_surface(window_arc.clone()).unwrap();
            let scale_factor = window_arc.scale_factor() as f32; // Get DPI scaling factor

            // Initialize the PlutoniumEngine with the adjusted size.
            let mut engine = PlutoniumEngine::new(surface, instance, size, scale_factor);

            // Create the text texture and store it with an identifier
            let text_position = Position { x: 0.0, y: 0.0 };
            self.text2d =
                Some(engine.create_text2d("Hello, Plutonium!", 40.0, text_position, scale_factor));

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
                    // Clear the render queue before each frame
                    engine.clear_render_queue();

                    engine.update(None, &None);
                    // Queue the text for rendering
                    if let Some(text2d) = &self.text2d {
                        text2d.render(engine);
                    }

                    // Submit the queue for rendering
                    engine.render().unwrap();
                }
            }
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new().unwrap();
    let mut app = TextRenderingExample::new();

    match event_loop.run_app(&mut app) {
        Ok(_) => println!("Application terminated gracefully."),
        Err(e) => eprintln!("Error running application: {:?}", e),
    }

    Ok(())
}
