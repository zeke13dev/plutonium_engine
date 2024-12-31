use plutonium_engine::text::FontError;
use plutonium_engine::{pluto_objects::text2d::Text2D, utils::Position, PlutoniumEngine};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
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
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes = Window::default_attributes().with_title("Text Rendering Example");

        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window_arc = Arc::new(window);
            let size = window_arc.as_ref().inner_size();
            let surface = instance.create_surface(window_arc.clone()).unwrap();
            let scale_factor = window_arc.scale_factor() as f32;
            // Initialize the engine
            let mut engine = PlutoniumEngine::new(surface, instance, size, scale_factor);

            // Load the font
            match engine.load_font("examples/media/roboto.ttf", 50.0, "roboto") {
                Ok(_) => (),
                Err(FontError::IoError(err)) => println!("I/O error occurred: {}", err),
                Err(FontError::InvalidFontData) => println!("Invalid font data"),
                Err(FontError::AtlasRenderError) => println!("Atlas render error occurred"),
            }

            // Create text with the specified font
            let text_position = Position { x: 0.0, y: 0.0 };
            self.text2d = Some(engine.create_text2d(
                "Hello, World! \n New Line",
                "roboto", // Use the loaded font
                50.0,
                text_position,
            ));

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

                    // Update the engine state
                    engine.update(None, &None);

                    // Queue text for rendering
                    if let Some(text2d) = &self.text2d {
                        text2d.render(engine);
                    }

                    // Render everything
                    engine.render().unwrap();

                    // Request next frame
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
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
