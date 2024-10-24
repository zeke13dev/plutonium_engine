use plutonium_engine::{
    utils::{MouseInfo, Position, Rectangle},
    PlutoniumEngine,
};
use std::sync::Arc;
use wgpu::Surface;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::{
    application::ApplicationHandler,
    event::KeyEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct TextureSvgExample<'a> {
    window: Option<Arc<Window>>,
    engine: Option<PlutoniumEngine<'a>>,
    _surface: Option<Surface<'a>>,
    mouse_info: MouseInfo,
}

impl<'a> TextureSvgExample<'a> {
    pub fn new() -> Self {
        let mouse_info = MouseInfo {
            is_rmb_clicked: false,
            is_lmb_clicked: false,
            is_mmb_clicked: false,
            mouse_pos: Position { x: 0.0, y: 0.0 },
        };

        Self {
            window: None,
            _surface: None,
            engine: None,
            mouse_info,
        }
    }
}

impl<'a> ApplicationHandler<()> for TextureSvgExample<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create the window safely with proper error handling
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes =
            Window::default_attributes().with_title("Moveable Texture SVG Example");
        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window_arc = Arc::new(window);
            let surface = instance.create_surface(window_arc.clone()).unwrap();
            let size = window_arc.inner_size(); // Get window size
            let scale_factor = window_arc.scale_factor() as f32; // Get DPI scaling factor

            // Initialize the PlutoniumEngine with the adjusted size.
            let mut engine = PlutoniumEngine::new(surface, instance, size, scale_factor);
            // Create the player texture
            engine.create_text_input(
                "input",
                "examples/media/input.svg",
                12.0,
                "Roboto",
                Rectangle::new(0.0, 0.0, 53.0, 16.0),
                5.0,
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
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_info.mouse_pos.x = position.x as f32;
                self.mouse_info.mouse_pos.y = position.y as f32;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left && state == ElementState::Pressed {
                    self.mouse_info.is_lmb_clicked = true;
                }
                if let Some(engine) = &mut self.engine {
                    engine.update(Some(self.mouse_info), &None);
                }
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
                    engine.update(Some(self.mouse_info), &Some(key));
                    self.window.as_ref().unwrap().request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(engine) = &mut self.engine {
                    // Clear the render queue before each frame
                    engine.clear_render_queue();
                    engine.update(Some(self.mouse_info), &None);
                    engine.render_obj("input");
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