use std::sync::Arc;

use plutonium::{
    utils::{Position, Rectangle, Size},
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
}

impl<'a> TextureSvgExample<'a> {
    pub fn new() -> Self {
        let player_position = Position { x: 0.0, y: 0.0 };

        Self {
            window: None,
            _surface: None,
            engine: None,
            player_position,
        }
    }
}

impl<'a> ApplicationHandler<()> for TextureSvgExample<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create the window safely with proper error handling
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes = Window::default_attributes().with_title("Camera Example");
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
                0.5,
                Some(Size {
                    width: 512.0,
                    height: 512.0,
                }),
            );
            engine.create_texture_svg(
                "player",
                "examples/media/player.svg",
                self.player_position,
                0.2,
                None,
            );
            engine.create_texture_svg(
                "boundary",
                "examples/media/boundary.svg",
                Position { x: 0.0, y: 0.0 },
                2.0,
                None,
            );

            // camera setup
            engine.set_camera_target("player");
            let boundary = Rectangle::new_square(0.0, 0.0, 200.0);
            engine.set_boundary(boundary);

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
                let _window = self.window.as_ref();
                if let Some(engine) = &mut self.engine {
                    // Clear the tile queue before each frame
                    engine.clear_render_queue();

                    // activate camera since the camera is deactivated later in the call
                    engine.activate_camera();

                    // queue the tiles for rendering
                    engine.update();
                    engine.queue_tile("atlas", 0, Position { x: 0.0, y: 0.0 });
                    engine.queue_tile(
                        "atlas",
                        1,
                        Position {
                            x: 512.0 * 0.5,
                            y: 0.0,
                        },
                    );
                    engine.queue_tile(
                        "atlas",
                        0,
                        Position {
                            x: 512.0 * 0.5,
                            y: 512.0 * 0.5,
                        },
                    );
                    engine.queue_tile(
                        "atlas",
                        1,
                        Position {
                            x: 0.0,
                            y: 512.0 * 0.5,
                        },
                    );
                    engine.queue_texture("player");

                    // deactivate camera to absolutely render the boundary
                    engine.deactivate_camera();
                    engine.queue_texture("boundary");

                    // submit the queue for rendering
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
