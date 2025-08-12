use crate::{
    utils::{MouseInfo, Position},
    PlutoniumEngine,
};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
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
    pub mouse_info: MouseInfo,
    pub delta_time: f32,
}

pub struct PlutoniumApp {
    engine: Option<super::PlutoniumEngine<'static>>,
    window: Option<Arc<Window>>,
    last_frame: std::time::Instant,
    frame_callback: Box<dyn FnMut(&mut PlutoniumEngine, &FrameContext)>,
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
                mouse_info: MouseInfo {
                    is_rmb_clicked: false,
                    is_lmb_clicked: false,
                    is_mmb_clicked: false,
                    mouse_pos: Position::default(),
                },
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
            .with_inner_size(winit::dpi::LogicalSize::new(
                WindowConfig::default().width,
                WindowConfig::default().height,
            ));

        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window = Arc::new(window);
            let size = window.inner_size();
            let surface = instance.create_surface(window.clone()).unwrap();
            let scale = window.scale_factor() as f32; // Get the scale factor
            let engine = super::PlutoniumEngine::new(surface, instance, size, scale); // Pass scale to engine
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
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.frame_context.mouse_info.mouse_pos = Position {
                    x: position.x as f32,
                    y: position.y as f32,
                };
            }

            WindowEvent::MouseInput { state, button, .. } => {
                match button {
                    MouseButton::Left => {
                        self.frame_context.mouse_info.is_lmb_clicked =
                            state == ElementState::Pressed;
                    }
                    MouseButton::Right => {
                        self.frame_context.mouse_info.is_rmb_clicked =
                            state == ElementState::Pressed;
                    }
                    MouseButton::Middle => {
                        self.frame_context.mouse_info.is_mmb_clicked =
                            state == ElementState::Pressed;
                    }
                    _ => {}
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                self.frame_context.delta_time = (now - self.last_frame).as_secs_f32();
                self.last_frame = now;

                if let Some(engine) = &mut self.engine {
                    // Run user's frame callback
                    (self.frame_callback)(engine, &self.frame_context);

                    // Update engine-side items (pluto objects and their textures)
                    // Pass the last pressed key this frame so interactive widgets receive input
                    let last_key = self.frame_context.pressed_keys.last().cloned();
                    engine.update(Some(self.frame_context.mouse_info), &last_key);

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
