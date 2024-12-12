use std::sync::Arc;

use plutonium_engine::{
    pluto_objects::texture_atlas_2d::TextureAtlas2D,
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
    scale_factor: f32, // Added field to store the scale factor
    atlas: Option<TextureAtlas2D>,
}

impl<'a> TextureSvgExample<'a> {
    pub fn new() -> Self {
        Self {
            window: None,
            _surface: None,
            engine: None,
            scale_factor: 1.0, // Initialize with default scale factor
            atlas: None,
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
            let scale_factor = window_arc.scale_factor() as f32;
            let mut engine = PlutoniumEngine::new(surface, instance, size, scale_factor);

            // Manually set the scale factor for the tiles
            self.scale_factor = 0.5; // For example, scale down to half size

            // Load and create the texture atlas with the manual scale factor
            self.atlas = Some(engine.create_texture_atlas_2d(
                "examples/media/map_atlas.svg",
                Position { x: 0.0, y: 0.0 },
                self.scale_factor,
                Size {
                    width: 512.0,
                    height: 512.0,
                },
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
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(engine) = &mut self.engine {
                    // Clear the render queue before each frame
                    engine.clear_render_queue();
                    engine.update(None, &None);

                    // Queue the tiles from the atlas for rendering, adjusting for the scale factor
                    let scaled_tile_size = Size {
                        width: 512.0 * self.scale_factor,
                        height: 512.0 * self.scale_factor,
                    };

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
                    };

                    // Submit the render queue
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
