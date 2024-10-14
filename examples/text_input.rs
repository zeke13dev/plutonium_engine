use std::sync::Arc;

use plutonium_engine::{
    utils::{MouseInfo, Position, Rectangle},
    PlutoniumEngine,
};
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
    player_position: Position,
    _surface: Option<Surface<'a>>,
    mouse_info: MouseInfo,
}

impl<'a> TextureSvgExample<'a> {
    pub fn new() -> Self {
        let player_position = Position { x: 0.0, y: 0.0 };
        let mouse_info = MouseInfo {
            is_RMB_clicked: false,
            is_LMB_clicked: false,
            is_MMB_clicked: false,
            mouse_pos: Position { x: 0.0, y: 0.0 },
        };

        Self {
            window: None,
            _surface: None,
            engine: None,
            player_position,
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
            let size = window_arc.as_ref().inner_size();
            let surface = instance.create_surface(window_arc.clone()).unwrap();
            let mut engine = PlutoniumEngine::new(surface, instance, size);

            // Create the player texture
            engine.create_text_input(
                "input",
                "examples/media/input.svg",
                12.0,
                1.0,
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
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(engine) = &mut self.engine {
                    // Clear the render queue before each frame
                    engine.clear_render_queue();
                    engine.update(Some(self.mouse_info), &None);
                    if let Some(obj) = engine.borrow_obj("input") {
                        obj.render(engine);
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
