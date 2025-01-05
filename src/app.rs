use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Plutonium Engine".to_string(),
            width: 800,
            height: 600,
        }
    }
}

pub struct FrameContext {
    pub pressed_keys: Vec<winit::keyboard::Key>,
    pub mouse_position: Option<crate::utils::Position>,
    pub delta_time: f32,
}

pub struct PlutoniumApp {
    engine: Option<super::PlutoniumEngine<'static>>,
    window: Option<Arc<Window>>,
    last_frame: std::time::Instant,
    frame_callback: Box<dyn FnMut(&mut super::PlutoniumEngine, &FrameContext)>,
    frame_context: FrameContext,
}

impl PlutoniumApp {
    pub fn new<F>(config: WindowConfig, frame_callback: F) -> Self
    where
        F: FnMut(&mut super::PlutoniumEngine, &FrameContext) + 'static,
    {
        Self {
            engine: None,
            window: None,
            last_frame: std::time::Instant::now(),
            frame_callback: Box::new(frame_callback),
            frame_context: FrameContext {
                pressed_keys: Vec::new(),
                mouse_position: None,
                delta_time: 0.0,
            },
        }
    }

    pub fn engine(&mut self) -> Option<&mut super::PlutoniumEngine<'static>> {
        self.engine.as_mut()
    }

    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref().map(|w| w.as_ref())
    }
}

impl ApplicationHandler<()> for PlutoniumApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes = Window::default_attributes()
            .with_title(&WindowConfig::default().title)
            .with_inner_size(winit::dpi::PhysicalSize::new(
                WindowConfig::default().width,
                WindowConfig::default().height,
            ));

        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window = Arc::new(window);
            let size = window.inner_size();
            let surface = instance.create_surface(window.clone()).unwrap();
            let scale = window.scale_factor() as f32;
            let engine = super::PlutoniumEngine::new(surface, instance, size, scale);
            self.engine = Some(engine);
            self.window = Some(window);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    self.frame_context.pressed_keys.push(event.logical_key);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.frame_context.mouse_position = Some(crate::utils::Position {
                    x: position.x as f32,
                    y: position.y as f32,
                });
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                self.frame_context.delta_time = (now - self.last_frame).as_secs_f32();
                self.last_frame = now;

                if let Some(engine) = &mut self.engine {
                    // Run user's frame callback
                    (self.frame_callback)(engine, &self.frame_context);

                    // Request next frame
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }

                    // Clear frame data
                    self.frame_context.pressed_keys.clear();
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(engine) = &mut self.engine {
                    engine.resize(&new_size);
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }
}

pub fn run_app<F>(config: WindowConfig, frame_callback: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&mut super::PlutoniumEngine, &FrameContext) + 'static,
{
    let event_loop = EventLoop::new()?;
    let mut app = PlutoniumApp::new(config, frame_callback);

    event_loop.run_app(&mut app)?;
    Ok(())
}
