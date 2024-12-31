use plutonium_engine::{
    pluto_objects::{texture_2d::Texture2D, texture_atlas_2d::TextureAtlas2D},
    utils::{Position, Rectangle, Size},
    PlutoniumEngine,
};
use std::sync::Arc;
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
    player: Option<Texture2D>,
    atlas: Option<TextureAtlas2D>,
    boundary: Option<Texture2D>,
    player_position: Position,
    _surface: Option<Surface<'a>>,
    scale_factor: f32,
}

impl<'a> TextureSvgExample<'a> {
    pub fn new() -> Self {
        let player_position = Position { x: 0.0, y: 0.0 };

        Self {
            window: None,
            _surface: None,
            engine: None,
            player: None,
            atlas: None,
            boundary: None,
            player_position,
            scale_factor: 0.5,
        }
    }
}

impl<'a> ApplicationHandler<()> for TextureSvgExample<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes = Window::default_attributes().with_title("Camera Example");

        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window_arc = Arc::new(window);
            let size = window_arc.as_ref().inner_size();
            let surface = instance.create_surface(window_arc.clone()).unwrap();
            let scale_factor = window_arc.scale_factor() as f32;
            let mut engine = PlutoniumEngine::new(surface, instance, size, scale_factor);

            // Create texture atlas for the map
            let atlas = engine.create_texture_atlas_2d(
                "examples/media/map_atlas.svg",
                Position { x: 0.0, y: 0.0 },
                0.5,
                Size {
                    width: 512.0,
                    height: 512.0,
                },
            );

            // Create player texture object
            let player =
                engine.create_texture_2d("examples/media/player.svg", self.player_position, 0.3);
            // Create boundary texture object
            let boundary = engine.create_texture_2d(
                "examples/media/boundary.svg",
                Position { x: 0.0, y: 0.0 },
                2.0,
            );

            // Set up camera and boundary
            engine.set_camera_target(player.get_id());
            let boundary_rect = Rectangle::new_square(0.0, 0.0, 200.0);
            engine.set_boundary(boundary_rect);

            window_arc.request_redraw();

            // Store all created objects
            self.player = Some(player);
            self.boundary = Some(boundary);
            self.atlas = Some(atlas);
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
                    if let Some(player) = &self.player {
                        player.set_pos(self.player_position);
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
                    engine.clear_render_queue();
                    engine.activate_camera();
                    engine.update(None, &None);

                    // Calculate scaled tile size
                    let scaled_tile_size = Size {
                        width: 512.0 * self.scale_factor,
                        height: 512.0 * self.scale_factor,
                    };

                    // Render atlas tiles with camera active
                    if let Some(atlas) = &self.atlas {
                        atlas.render_tile(engine, 0, Position { x: 0.0, y: 0.0 });
                        atlas.render_tile(
                            engine,
                            1,
                            Position {
                                x: scaled_tile_size.width,
                                y: 0.0,
                            },
                        );
                        atlas.render_tile(
                            engine,
                            0,
                            Position {
                                x: scaled_tile_size.width,
                                y: scaled_tile_size.height,
                            },
                        );
                        atlas.render_tile(
                            engine,
                            1,
                            Position {
                                x: 0.0,
                                y: scaled_tile_size.height,
                            },
                        );
                    }

                    // Render player with camera active
                    if let Some(player) = &self.player {
                        player.render(engine);
                    }

                    // Render boundary with camera deactivated
                    engine.deactivate_camera();
                    if let Some(boundary) = &self.boundary {
                        boundary.render(engine);
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
