use std::sync::Arc;

use plutonium_engine::{utils::Position, PlutoniumEngine};
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
}

impl<'a> TextRenderingExample<'a> {
    pub fn new() -> Self {
        Self {
            window: None,
            engine: None,
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
            let mut engine = PlutoniumEngine::new(surface, instance, size);

            // Create the text texture and store it with an identifier
            let text_position = Position { x: 0.0, y: 0.0 };
            engine.create_text_texture("greeting", "Hello, Plutonium!", 40.0, text_position);

            // Queue the text for rendering
            engine.queue_texture("greeting", Some(text_position));

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
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if let Some(engine) = &mut self.engine {
                    match key.as_ref() {
                        Key::Character("r") => {
                            // Clear the render queue and re-queue the text for rendering
                            engine.clear_render_queue();
                            let text_position = Position { x: 100.0, y: 100.0 };
                            engine.queue_texture("greeting", Some(text_position));

                            self.window.as_ref().unwrap().request_redraw();
                        }
                        _ => (),
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(engine) = &mut self.engine {
                    // Clear the render queue before each frame
                    engine.clear_render_queue();

                    engine.update(None, &None);
                    // Queue the text for rendering
                    let text_position = Position { x: 0.0, y: 0.0 };
                    engine.queue_texture("greeting", Some(text_position));

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
