use std::sync::Arc;

use plutonium_engine::{
    utils::{MouseInfo, Position},
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
            is_rmb_clicked: false,
            is_lmb_clicked: false,
            is_mmb_clicked: false,
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
            let scale = window_arc.scale_factor() as f32;
            let mut engine = PlutoniumEngine::new(surface, instance, size, scale);

            // Create the player texture
            engine.create_texture_svg(
                "player",
                "examples/media/player.svg",
                self.player_position,
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
                let mut update_position = |dx, dy| {
                    self.player_position.x += dx;
                    self.player_position.y += dy;
                    if let Some(engine) = &mut self.engine {
                        engine.set_texture_position("player", self.player_position);
                        self.window.as_ref().unwrap().request_redraw();
                    }
                };

                match key.as_ref() {
                    Key::Character("a") => update_position(-10.0, 0.0),
                    Key::Character("d") => update_position(10.0, 0.0),
                    Key::Character("w") => update_position(0.0, -10.0),
                    Key::Character("s") => update_position(0.0, 10.0),
                    _ => (),
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(engine) = &mut self.engine {
                    // Clear the render queue before each frame
                    engine.clear_render_queue();
                    engine.update(Some(self.mouse_info), &None);
                    engine.queue_texture("player", Some(self.player_position));
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
