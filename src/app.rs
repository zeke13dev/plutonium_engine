use crate::{
    utils::{FrameTimeMetrics, MouseInfo, Position},
    PlutoniumEngine,
};
use std::collections::HashMap;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FrameInputRecordLocal {
    pub pressed_keys: Vec<String>,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub lmb_down: bool,
    pub committed_text: Vec<String>,
}

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
    pub text_commits: Vec<String>,
}

pub struct PlutoniumApp {
    engine: Option<super::PlutoniumEngine<'static>>,
    window: Option<Arc<Window>>,
    last_frame: std::time::Instant,
    frame_callback: Box<dyn FnMut(&mut PlutoniumEngine, &FrameContext)>,
    frame_context: FrameContext,
    config: WindowConfig,
    metrics: FrameTimeMetrics,
    // Recording state
    record_log: Option<Vec<FrameInputRecordLocal>>,
    record_path: Option<String>,
    // Replay state
    replay_frames: Option<Vec<FrameInputRecordLocal>>,
    replay_cursor: usize,
    // Fixed timestep (if set, overrides delta_time passed to frame callback)
    fixed_dt: Option<f32>,
    // DPI scale factor for converting physical input coordinates to logical
    dpi_scale_factor: f32,
    // Startup flags parsed by CLI
    startup_record_path: Option<String>,
    startup_replay_path: Option<String>,
    // Key repeat config and state
    key_repeat_enabled: bool,
    key_repeat_delay: f32,   // seconds before first repeat
    key_repeat_rate_hz: f32, // repeats per second after delay
    key_repeat_states: HashMap<String, (winit::keyboard::Key, KeyRepeatState)>,
}

#[derive(Debug, Clone, Copy)]
struct KeyRepeatState {
    is_down: bool,
    elapsed: f32,
    next_fire: f32,
}

impl PlutoniumApp {
    pub fn new<F>(config: WindowConfig, frame_callback: F) -> Self
    where
        F: FnMut(&mut super::PlutoniumEngine, &FrameContext) + 'static,
    {
        let mut app = Self {
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
                text_commits: Vec::new(),
            },
            config,
            metrics: FrameTimeMetrics::new(600, 5.0), // ~10s at 60 FPS
            record_log: None,
            record_path: None,
            replay_frames: None,
            replay_cursor: 0,
            fixed_dt: None,
            dpi_scale_factor: 1.0,
            startup_record_path: None,
            startup_replay_path: None,
            key_repeat_enabled: true,
            key_repeat_delay: 0.5,
            key_repeat_rate_hz: 12.0,
            key_repeat_states: HashMap::new(),
        };
        // Env knobs: PLUTO_FIXED_DT (seconds) or PLUTO_FIXED_FPS
        if let Ok(s) = std::env::var("PLUTO_FIXED_DT") {
            if let Ok(v) = s.parse::<f32>() {
                if v >= 0.0 {
                    app.fixed_dt = Some(v);
                }
            }
        } else if let Ok(s) = std::env::var("PLUTO_FIXED_FPS") {
            if let Ok(fps) = s.parse::<f32>() {
                if fps > 0.0 {
                    app.fixed_dt = Some(1.0 / fps);
                }
            }
        }
        // Clipboard (optional): expose simple copy/paste helpers via env flags for demos
        // Not a full API; kept minimal and opt-in per example.
        app
    }

    pub fn engine(&mut self) -> Option<&mut super::PlutoniumEngine<'static>> {
        self.engine.as_mut()
    }

    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref().map(|w| w.as_ref())
    }

    /// Set a fixed timestep for the frame callback (e.g., 1.0/60.0).
    /// When set, `FrameContext.delta_time` uses this value each frame.
    pub fn set_fixed_timestep(&mut self, dt_seconds: f32) {
        self.fixed_dt = Some(dt_seconds.max(0.0));
    }

    /// Disable fixed timestep; `FrameContext.delta_time` will be real time.
    pub fn clear_fixed_timestep(&mut self) {
        self.fixed_dt = None;
    }

    pub fn start_recording(&mut self, path: impl Into<String>) {
        self.record_log = Some(Vec::new());
        self.record_path = Some(path.into());
    }

    pub fn stop_recording(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let (Some(log), Some(path)) = (self.record_log.take(), self.record_path.take()) {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_string_pretty(&log)?)?;
        }
        Ok(())
    }

    pub fn start_replay(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let frames: Vec<FrameInputRecordLocal> = serde_json::from_str(&json)?;
        self.replay_cursor = 0;
        self.replay_frames = Some(frames);
        Ok(())
    }

    pub fn stop_replay(&mut self) {
        self.replay_frames = None;
        self.replay_cursor = 0;
    }
}

impl ApplicationHandler<()> for PlutoniumApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ));

        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window = Arc::new(window);
            let size = window.inner_size();
            let surface = instance.create_surface(window.clone()).unwrap();
            let scale = window.scale_factor() as f32; // Get the scale factor
            let engine = super::PlutoniumEngine::new(surface, instance, size, scale); // Pass scale to engine
            self.engine = Some(engine);
            self.window = Some(window);
            self.dpi_scale_factor = scale;
            // Apply startup record/replay if requested
            if let Some(path) = self.startup_record_path.take() {
                self.start_recording(path);
                println!("recording started (cli)");
            }
            if let Some(path) = self.startup_replay_path.take() {
                if let Err(e) = self.start_replay(&path) {
                    eprintln!("failed to start replay: {}", e);
                } else {
                    println!("replay started (cli)");
                }
            }
        }
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Ime(ime) => {
                if let winit::event::Ime::Commit(text) = ime {
                    if !text.is_empty() {
                        self.frame_context.text_commits.push(text);
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    // Hotkeys: 'r' toggles recording; 'p' starts replay
                    if let winit::keyboard::Key::Character(ch) = &event.logical_key {
                        if ch.eq_ignore_ascii_case("r") {
                            if self.record_log.is_some() {
                                let _ = self.stop_recording();
                                println!("recording stopped");
                            } else {
                                self.start_recording("snapshots/replays/app_session.json");
                                println!("recording started -> snapshots/replays/app_session.json");
                            }
                        } else if ch.eq_ignore_ascii_case("p") {
                            let _ = self.start_replay("snapshots/replays/app_session.json");
                            println!("replay started from snapshots/replays/app_session.json");
                        }
                    }
                    // Record immediate press
                    self.frame_context
                        .pressed_keys
                        .push(event.logical_key.clone());
                    // Initialize key repeat state
                    let key_id = format!("{:?}", event.logical_key);
                    self.key_repeat_states.insert(
                        key_id,
                        (
                            event.logical_key,
                            KeyRepeatState {
                                is_down: true,
                                elapsed: 0.0,
                                next_fire: self.key_repeat_delay,
                            },
                        ),
                    );
                } else {
                    // Key released: stop repeating
                    let key_id = format!("{:?}", event.logical_key);
                    if let Some((_key, st)) = self.key_repeat_states.get_mut(&key_id) {
                        st.is_down = false;
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // Convert physical coordinates to logical using the window's DPI scale
                let scale = if self.dpi_scale_factor > 0.0 {
                    self.dpi_scale_factor
                } else {
                    1.0
                };
                self.frame_context.mouse_info.mouse_pos = Position {
                    x: (position.x as f32) / scale,
                    y: (position.y as f32) / scale,
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
                let real_dt = (now - self.last_frame).as_secs_f32();
                self.last_frame = now;
                self.metrics.record(real_dt);
                // Provide either fixed dt or real dt to the callback
                self.frame_context.delta_time = self.fixed_dt.unwrap_or(real_dt);

                let mut should_stop_replay = false;
                if let Some(engine) = &mut self.engine {
                    // Key repeat synthesis before running user callback
                    if self.key_repeat_enabled {
                        let dt = self.frame_context.delta_time;
                        let rate_dt = if self.key_repeat_rate_hz > 0.0 {
                            1.0 / self.key_repeat_rate_hz
                        } else {
                            f32::INFINITY
                        };
                        for (_id, (k, st)) in self.key_repeat_states.iter_mut() {
                            if st.is_down {
                                st.elapsed += dt;
                                if st.elapsed >= st.next_fire {
                                    self.frame_context.pressed_keys.push(k.clone());
                                    st.next_fire += rate_dt;
                                }
                            }
                        }
                    }
                    // If replaying, override frame context from recorded frame
                    if let Some(frames) = self.replay_frames.as_ref() {
                        if self.replay_cursor < frames.len() {
                            let fr = &frames[self.replay_cursor];
                            self.frame_context.pressed_keys.clear(); // skip key reconstruction
                            self.frame_context.mouse_info.mouse_pos = Position {
                                x: fr.mouse_x,
                                y: fr.mouse_y,
                            };
                            self.frame_context.mouse_info.is_lmb_clicked = fr.lmb_down;
                            self.frame_context.mouse_info.is_mmb_clicked = false;
                            self.frame_context.mouse_info.is_rmb_clicked = false;
                            self.frame_context.text_commits = fr.committed_text.clone();
                            self.replay_cursor += 1;
                        } else {
                            // End of replay; delay stop to avoid borrow conflict
                            should_stop_replay = true;
                        }
                    }

                    // If recording, append frame
                    if let Some(log) = self.record_log.as_mut() {
                        log.push(FrameInputRecordLocal {
                            pressed_keys: self
                                .frame_context
                                .pressed_keys
                                .iter()
                                .map(|k| format!("{:?}", k))
                                .collect(),
                            mouse_x: self.frame_context.mouse_info.mouse_pos.x,
                            mouse_y: self.frame_context.mouse_info.mouse_pos.y,
                            lmb_down: self.frame_context.mouse_info.is_lmb_clicked,
                            committed_text: self.frame_context.text_commits.clone(),
                        });
                    }

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
                    self.frame_context.text_commits.clear();
                }

                if should_stop_replay {
                    self.stop_replay();
                }

                if let Some(line) = self.metrics.maybe_report() {
                    println!("{}", line);
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(engine) = &mut self.engine {
                    engine.resize(&new_size);
                }
            }
            WindowEvent::CloseRequested => {
                // Auto-stop recording on exit
                let _ = self.stop_recording();
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

    // Parse simple CLI flags: --record <path>, --replay <path>, --dt <seconds>, --fps <hz>, --keyrepeat on|off, --keydelay <s>, --keyrate <hz>
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--record" => {
                if let Some(p) = it.next() {
                    app.startup_record_path = Some(p);
                }
            }
            "--replay" => {
                if let Some(p) = it.next() {
                    app.startup_replay_path = Some(p);
                }
            }
            "--dt" => {
                if let Some(v) = it.next() {
                    if let Ok(dt) = v.parse::<f32>() {
                        app.set_fixed_timestep(dt);
                    }
                }
            }
            "--fps" => {
                if let Some(v) = it.next() {
                    if let Ok(fps) = v.parse::<f32>() {
                        if fps > 0.0 {
                            app.set_fixed_timestep(1.0 / fps);
                        }
                    }
                }
            }
            "--keyrepeat" => {
                if let Some(v) = it.next() {
                    app.key_repeat_enabled =
                        matches!(v.to_ascii_lowercase().as_str(), "on" | "1" | "true");
                }
            }
            "--keydelay" => {
                if let Some(v) = it.next() {
                    if let Ok(d) = v.parse::<f32>() {
                        app.key_repeat_delay = d.max(0.0);
                    }
                }
            }
            "--keyrate" => {
                if let Some(v) = it.next() {
                    if let Ok(hz) = v.parse::<f32>() {
                        app.key_repeat_rate_hz = hz.max(0.0);
                    }
                }
            }
            _ => {}
        }
    }

    event_loop.run_app(&mut app)?;
    Ok(())
}
