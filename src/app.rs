use crate::{
    utils::{monotonic_now_seconds, FrameTimeMetrics, MouseInfo, Position},
    PlutoniumEngine,
};
#[cfg(target_arch = "wasm32")]
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::{collections::HashMap, fmt};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

type FrameCallback = Box<dyn FnMut(&mut PlutoniumEngine, &FrameContext, &mut PlutoniumApp)>;

#[doc(hidden)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct FrameInputRecordLocal {
    pub pressed_keys: Vec<String>,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub lmb_down: bool,
    pub rmb_down: bool,
    pub mmb_down: bool,
    pub scroll_dx: f32,
    pub scroll_dy: f32,
    pub committed_text: Vec<String>,
}

/// Logical keyboard key reported to frame callbacks.
///
/// This crate-owned wrapper keeps [`FrameContext`] independent from the exact
/// `winit::keyboard::Key` type while preserving the common character/named-key
/// queries used by applications.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Key {
    logical: winit::keyboard::Key,
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.logical.fmt(f)
    }
}

impl Key {
    pub(crate) fn from_winit(logical: winit::keyboard::Key) -> Self {
        Self { logical }
    }

    /// Returns the Unicode character for character keys.
    pub fn character(&self) -> Option<&str> {
        match &self.logical {
            winit::keyboard::Key::Character(value) => Some(value.as_str()),
            _ => None,
        }
    }

    /// Returns true when this key is a character equal to `value`, ignoring ASCII case.
    pub fn is_character_ignore_ascii_case(&self, value: &str) -> bool {
        self.character()
            .map(|character| character.eq_ignore_ascii_case(value))
            .unwrap_or(false)
    }

    /// Returns true when this key is a named key with the given `Debug` name.
    ///
    /// Names match `winit`'s `NamedKey` debug spelling, such as `"Enter"`,
    /// `"Escape"`, or `"ArrowLeft"`.
    pub fn is_named(&self, name: &str) -> bool {
        match &self.logical {
            winit::keyboard::Key::Named(named) => format!("{named:?}") == name,
            _ => false,
        }
    }

    /// Returns the stable debug spelling recorded by replay/snapshot tooling.
    pub fn debug_name(&self) -> String {
        format!("{:?}", self.logical)
    }
}

/// Frame-local collection of logical keyboard keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    inner: Vec<Key>,
}

impl Keys {
    /// Returns the number of key presses in this frame.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true when no key presses were reported for this frame.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterates over frame-local key presses.
    pub fn iter(&self) -> std::slice::Iter<'_, Key> {
        self.inner.iter()
    }

    /// Returns true when any key is a character equal to `value`, ignoring ASCII case.
    pub fn contains_character_ignore_ascii_case(&self, value: &str) -> bool {
        self.inner
            .iter()
            .any(|key| key.is_character_ignore_ascii_case(value))
    }

    /// Returns true when any key is a named key with the given `Debug` name.
    pub fn contains_named(&self, name: &str) -> bool {
        self.inner.iter().any(|key| key.is_named(name))
    }

    pub(crate) fn clear(&mut self) {
        self.inner.clear();
    }

    pub(crate) fn push_winit(&mut self, logical: winit::keyboard::Key) {
        self.inner.push(Key::from_winit(logical));
    }

    pub(crate) fn last_winit_cloned(&self) -> Option<winit::keyboard::Key> {
        self.inner.last().map(|key| key.logical.clone())
    }
}

impl<'a> IntoIterator for &'a Keys {
    type Item = &'a Key;
    type IntoIter = std::slice::Iter<'a, Key>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// WindowConfig data.
pub struct WindowConfig {
    /// Window title.
    pub title: String,
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
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

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy)]
pub struct WasmAppConfig {
    pub prevent_default: bool,
    pub focusable: bool,
}

#[cfg(target_arch = "wasm32")]
impl Default for WasmAppConfig {
    fn default() -> Self {
        Self {
            prevent_default: true,
            focusable: true,
        }
    }
}

#[derive(Clone)]
/// Per-frame input and timing passed to the application callback.
pub struct FrameContext {
    /// Logical keys pressed during the current frame.
    pub pressed_keys: Keys,
    /// Mouse info value.
    pub mouse_info: MouseInfo,
    /// Delta time value.
    pub delta_time: f32,
    /// Text commits value.
    pub text_commits: Vec<String>,
    /// Scroll delta value.
    pub scroll_delta: Position,
}

/// PlutoniumApp data.
pub struct PlutoniumApp {
    engine: Option<super::PlutoniumEngine<'static>>,
    window: Option<Arc<Window>>,
    last_frame_secs: f64,
    frame_callback: Option<FrameCallback>,
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
    // Fullscreen state
    is_fullscreen: bool,
    #[cfg(target_arch = "wasm32")]
    wasm_canvas: Option<HtmlCanvasElement>,
    #[cfg(target_arch = "wasm32")]
    engine_init_rx:
        Option<std::sync::mpsc::Receiver<Result<super::PlutoniumEngine<'static>, String>>>,
    #[cfg(target_arch = "wasm32")]
    engine_init_start_secs: Option<f64>,
    #[cfg(target_arch = "wasm32")]
    wasm_prevent_default: bool,
    #[cfg(target_arch = "wasm32")]
    wasm_focusable: bool,
}

#[derive(Debug, Clone, Copy)]
struct KeyRepeatState {
    is_down: bool,
    elapsed: f32,
    next_fire: f32,
}

#[cfg(target_arch = "wasm32")]
fn wasm_console_log(message: &str) {
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(message));
}

#[cfg(target_arch = "wasm32")]
fn set_wasm_debug_status(message: &str) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.set_title(message);
            if let Some(el) = document.get_element_by_id("pluto-debug") {
                el.set_text_content(Some(message));
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_canvas_backing_store(canvas: &HtmlCanvasElement, preferred_scale_factor: Option<f64>) {
    let fallback_dpr = web_sys::window()
        .map(|window| window.device_pixel_ratio())
        .unwrap_or(1.0);
    let dpr = preferred_scale_factor
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or(fallback_dpr)
        .max(0.000_1);

    let css_width = canvas.client_width() as f64;
    let css_height = canvas.client_height() as f64;
    let css_width = if css_width > 0.0 {
        css_width
    } else {
        (canvas.width().max(1) as f64) / dpr
    };
    let css_height = if css_height > 0.0 {
        css_height
    } else {
        (canvas.height().max(1) as f64) / dpr
    };

    let physical_width = (css_width * dpr).round().max(1.0) as u32;
    let physical_height = (css_height * dpr).round().max(1.0) as u32;
    if canvas.width() != physical_width {
        canvas.set_width(physical_width);
    }
    if canvas.height() != physical_height {
        canvas.set_height(physical_height);
    }
}

impl PlutoniumApp {
    /// Creates a new value.
    pub fn new<F>(config: WindowConfig, frame_callback: F) -> Self
    where
        F: FnMut(&mut super::PlutoniumEngine, &FrameContext, &mut PlutoniumApp) + 'static,
    {
        let mut app = Self {
            engine: None,
            window: None,
            last_frame_secs: monotonic_now_seconds(),
            frame_callback: Some(Box::new(frame_callback)),
            frame_context: FrameContext {
                pressed_keys: Keys::default(),
                mouse_info: MouseInfo {
                    is_rmb_clicked: false,
                    is_lmb_clicked: false,
                    is_mmb_clicked: false,
                    mouse_pos: Position::default(),
                    scroll_dx: 0.0,
                    scroll_dy: 0.0,
                },
                delta_time: 0.0,
                text_commits: Vec::new(),
                scroll_delta: Position::default(),
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
            is_fullscreen: false,
            #[cfg(target_arch = "wasm32")]
            wasm_canvas: None,
            #[cfg(target_arch = "wasm32")]
            engine_init_rx: None,
            #[cfg(target_arch = "wasm32")]
            engine_init_start_secs: None,
            #[cfg(target_arch = "wasm32")]
            wasm_prevent_default: true,
            #[cfg(target_arch = "wasm32")]
            wasm_focusable: true,
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

    /// Engine.
    pub fn engine(&mut self) -> Option<&mut super::PlutoniumEngine<'static>> {
        self.engine.as_mut()
    }

    /// Window.
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

    /// Starts recording.
    pub fn start_recording(&mut self, path: impl Into<String>) {
        self.record_log = Some(Vec::new());
        self.record_path = Some(path.into());
    }

    /// Stops recording.
    pub fn stop_recording(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let (Some(log), Some(path)) = (self.record_log.take(), self.record_path.take()) {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_string_pretty(&log)?)?;
        }
        Ok(())
    }

    /// Starts replay.
    ///
    /// Replay restores mouse buttons, cursor position, scroll, committed text,
    /// and frame timing from the recording. Logical key events are intentionally
    /// not reconstructed from their serialized debug names, so
    /// [`FrameContext::pressed_keys`] is empty during replay frames.
    pub fn start_replay(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let frames: Vec<FrameInputRecordLocal> = serde_json::from_str(&json)?;
        self.replay_cursor = 0;
        self.replay_frames = Some(frames);
        Ok(())
    }

    /// Stops replay.
    pub fn stop_replay(&mut self) {
        self.replay_frames = None;
        self.replay_cursor = 0;
    }

    /// Toggle or set fullscreen mode.
    /// On macOS, this uses exclusive fullscreen which works well with the green button.
    /// On other platforms, uses borderless fullscreen.
    /// The green button on macOS will trigger native fullscreen which is handled by the OS.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.is_fullscreen = fullscreen;
        if let Some(window) = &self.window {
            if fullscreen {
                if let Some(monitor) = window.current_monitor() {
                    // On macOS, use exclusive fullscreen which works better with the green button
                    // On other platforms, borderless is preferred
                    #[cfg(target_os = "macos")]
                    {
                        let video_mode = monitor.video_modes().next();
                        if let Some(video_mode) = video_mode {
                            window.set_fullscreen(Some(winit::window::Fullscreen::Exclusive(
                                video_mode,
                            )));
                        } else {
                            // Fallback to borderless if no video mode
                            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(
                                Some(monitor),
                            )));
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(
                            monitor,
                        ))));
                    }
                } else {
                    // Fallback: try to get any monitor
                    window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                }
            } else {
                window.set_fullscreen(None);
            }
        }
    }

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&mut self) {
        self.set_fullscreen(!self.is_fullscreen);
    }

    /// Check if window is currently in fullscreen mode
    pub fn is_fullscreen(&self) -> bool {
        self.is_fullscreen
    }

    /// Returns true when the given `winit` logical key is currently held down.
    ///
    /// This low-level helper is for manual integrations that already consume
    /// `winit`; frame callbacks should prefer [`FrameContext::pressed_keys`].
    pub fn is_key_down(&self, key: &winit::keyboard::Key) -> bool {
        let key_id = format!("{:?}", key);
        self.key_repeat_states
            .get(&key_id)
            .map(|(_, state)| state.is_down)
            .unwrap_or(false)
    }

    /// Returns true when a character key (case-insensitive) is currently held down.
    pub fn is_char_key_down(&self, ch: char) -> bool {
        let needle = ch.to_string();
        self.key_repeat_states.values().any(|(key, state)| {
            state.is_down
                && matches!(
                    key,
                    winit::keyboard::Key::Character(value) if value.eq_ignore_ascii_case(&needle)
                )
        })
    }

    /// Returns true when a named key is currently held down.
    pub fn is_named_key_down(&self, named: winit::keyboard::NamedKey) -> bool {
        self.key_repeat_states.values().any(|(key, state)| {
            state.is_down
                && matches!(
                    key,
                    winit::keyboard::Key::Named(current) if *current == named
                )
        })
    }

    fn sanitize_dpi_scale_factor(scale_factor: f64) -> f32 {
        if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor as f32
        } else {
            1.0
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn sync_wasm_canvas_backing_store(&self, preferred_scale_factor: Option<f64>) {
        if let Some(canvas) = self.wasm_canvas.as_ref() {
            sync_canvas_backing_store(canvas, preferred_scale_factor);
        }
    }

    fn apply_dpi_change(&mut self, scale_factor: f64, resize_to: Option<PhysicalSize<u32>>) {
        self.dpi_scale_factor = Self::sanitize_dpi_scale_factor(scale_factor);

        #[cfg(target_arch = "wasm32")]
        self.sync_wasm_canvas_backing_store(Some(scale_factor));

        let resize_size =
            resize_to.or_else(|| self.window.as_ref().map(|window| window.inner_size()));
        if let Some(engine) = &mut self.engine {
            engine.set_dpi_scale_factor(scale_factor);
            if let Some(size) = resize_size {
                // Always run resize/reconfigure on DPI changes to avoid stale swapchain/viewport state.
                engine.resize(&size);
            }
        }
    }
}

impl ApplicationHandler<()> for PlutoniumApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        set_wasm_debug_status("resumed; creating window...");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let window_attributes = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ));
        #[cfg(target_arch = "wasm32")]
        let window_attributes = if let Some(canvas) = self.wasm_canvas.as_ref() {
            window_attributes
                .with_canvas(Some(canvas.clone()))
                .with_prevent_default(self.wasm_prevent_default)
                .with_focusable(self.wasm_focusable)
        } else {
            window_attributes
        };

        if let Ok(window) = event_loop.create_window(window_attributes) {
            let window = Arc::new(window);
            let size = window.inner_size();
            let surface = match instance.create_surface(window.clone()) {
                Ok(surface) => surface,
                Err(err) => {
                    log::warn!("failed to create wgpu surface: {err}");
                    #[cfg(target_arch = "wasm32")]
                    set_wasm_debug_status(&format!("create_surface failed: {err}"));
                    return;
                }
            };
            let scale = window.scale_factor();
            self.window = Some(window);
            self.dpi_scale_factor = Self::sanitize_dpi_scale_factor(scale);
            #[cfg(target_arch = "wasm32")]
            self.sync_wasm_canvas_backing_store(Some(scale));
            #[cfg(not(target_arch = "wasm32"))]
            {
                match super::PlutoniumEngine::new(surface, instance, size, self.dpi_scale_factor) {
                    Ok(mut engine) => {
                        engine.set_dpi_scale_factor(scale);
                        self.engine = Some(engine);
                    }
                    Err(err) => {
                        log::warn!("failed to initialize PlutoniumEngine: {err}");
                        return;
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                set_wasm_debug_status("initializing engine (requesting GPU adapter)...");
                wasm_console_log("pluto app: spawn_local engine init started");
                let (tx, rx) = mpsc::channel();
                self.engine_init_rx = Some(rx);
                self.engine_init_start_secs = Some(monotonic_now_seconds());
                let window_for_init = self.window.as_ref().cloned();
                spawn_local(async move {
                    wasm_console_log("pluto app: inside engine init future before new_async");
                    let result =
                        super::PlutoniumEngine::new_async(surface, instance, size, scale as f32)
                            .await;
                    if let Err(err) = &result {
                        set_wasm_debug_status(&format!("engine init failed: {err}"));
                        wasm_console_log(&format!("pluto app: engine init future failed: {err}"));
                    } else {
                        set_wasm_debug_status("engine initialized");
                        wasm_console_log("pluto app: engine init future completed successfully");
                    }
                    let sent = tx.send(result).is_ok();
                    wasm_console_log(&format!(
                        "pluto app: sent init result over channel = {}",
                        sent
                    ));
                    if let Some(window) = window_for_init {
                        wasm_console_log("pluto app: requesting redraw after init future");
                        window.request_redraw();
                    }
                });
            }
            // Apply startup record/replay if requested
            if let Some(path) = self.startup_record_path.take() {
                self.start_recording(path);
                log::info!("recording started (cli)");
            }
            if let Some(path) = self.startup_replay_path.take() {
                if let Err(e) = self.start_replay(&path) {
                    log::warn!("failed to start replay: {}", e);
                } else {
                    log::info!("replay started (cli)");
                }
            }
        } else {
            #[cfg(target_arch = "wasm32")]
            set_wasm_debug_status("create_window failed");
        }
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                if !text.is_empty() {
                    self.frame_context.text_commits.push(text);
                }
            }
            WindowEvent::Ime(_) => {} // Handle other IME events
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    // Hotkeys: 'r' toggles recording; 'p' starts replay; F11 toggles fullscreen
                    if event.physical_key
                        == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F11)
                    {
                        self.toggle_fullscreen();
                    } else if let winit::keyboard::Key::Character(ch) = &event.logical_key {
                        if ch.eq_ignore_ascii_case("r") {
                            if self.record_log.is_some() {
                                let _ = self.stop_recording();
                                log::info!("recording stopped");
                            } else {
                                self.start_recording("snapshots/replays/app_session.json");
                                log::info!(
                                    "recording started -> snapshots/replays/app_session.json"
                                );
                            }
                        } else if ch.eq_ignore_ascii_case("p") {
                            let _ = self.start_replay("snapshots/replays/app_session.json");
                            log::info!("replay started from snapshots/replays/app_session.json");
                        }
                    }
                    // Record immediate press
                    self.frame_context
                        .pressed_keys
                        .push_winit(event.logical_key.clone());
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
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = if self.dpi_scale_factor > 0.0 {
                    self.dpi_scale_factor
                } else {
                    1.0
                };
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        let line_px = 40.0;
                        self.frame_context.scroll_delta.x += x * line_px;
                        self.frame_context.scroll_delta.y += y * line_px;
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        self.frame_context.scroll_delta.x += (pos.x as f32) / scale;
                        self.frame_context.scroll_delta.y += (pos.y as f32) / scale;
                    }
                }
                self.frame_context.mouse_info.scroll_dx = self.frame_context.scroll_delta.x;
                self.frame_context.mouse_info.scroll_dy = self.frame_context.scroll_delta.y;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                #[cfg(target_arch = "wasm32")]
                {
                    if self.engine.is_none() {
                        wasm_console_log(
                            "pluto app: RedrawRequested with no engine; polling init channel",
                        );
                        let init_poll = self.engine_init_rx.as_ref().map(|rx| rx.try_recv());
                        if let Some(init_result) = init_poll {
                            match init_result {
                                Ok(Ok(engine)) => {
                                    wasm_console_log(
                                        "pluto app: init channel yielded engine; marking ready",
                                    );
                                    self.engine = Some(engine);
                                    let scale = self
                                        .window
                                        .as_ref()
                                        .map(|window| window.scale_factor())
                                        .unwrap_or(self.dpi_scale_factor as f64);
                                    self.apply_dpi_change(
                                        scale,
                                        self.window.as_ref().map(|window| window.inner_size()),
                                    );
                                    self.engine_init_rx = None;
                                    set_wasm_debug_status("engine ready");
                                }
                                Ok(Err(err)) => {
                                    wasm_console_log(&format!(
                                        "pluto app: init channel yielded engine error: {}",
                                        err
                                    ));
                                    log::warn!(
                                        "failed to initialize plutonium engine (wasm): {err}"
                                    );
                                    self.engine_init_rx = None;
                                    set_wasm_debug_status(&format!("engine init error: {err}"));
                                }
                                Err(TryRecvError::Empty) => {
                                    wasm_console_log("pluto app: init channel empty");
                                }
                                Err(TryRecvError::Disconnected) => {
                                    wasm_console_log("pluto app: init channel disconnected");
                                    log::warn!("engine initialization channel disconnected");
                                    self.engine_init_rx = None;
                                    set_wasm_debug_status("engine init channel disconnected");
                                }
                            }
                        }
                    }
                    if self.engine.is_none() {
                        // Show elapsed time and hard-fail after 15s so the user
                        // gets a concrete error instead of hanging forever.
                        const INIT_TIMEOUT_SECS: f64 = 15.0;
                        if let Some(start) = self.engine_init_start_secs {
                            let elapsed = monotonic_now_seconds() - start;
                            if elapsed > INIT_TIMEOUT_SECS {
                                self.engine_init_rx = None;
                                self.engine_init_start_secs = None;
                                set_wasm_debug_status(&format!(
                                    "engine init timed out after {:.0}s — check browser console for details (WebGPU/WebGL2 may be unavailable)",
                                    elapsed
                                ));
                                return;
                            }
                            if elapsed > 2.0 {
                                set_wasm_debug_status(&format!(
                                    "waiting for GPU adapter… {:.1}s",
                                    elapsed
                                ));
                            }
                        }
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                        return;
                    }
                }

                let now = monotonic_now_seconds();
                let real_dt = (now - self.last_frame_secs).max(0.0) as f32;
                self.last_frame_secs = now;
                self.metrics.record(real_dt);
                // Provide either fixed dt or real dt to the callback
                self.frame_context.delta_time = self.fixed_dt.unwrap_or(real_dt);

                let mut should_stop_replay = false;
                // Temporarily take the engine to avoid borrow conflicts when passing self to callback
                let mut engine_opt = self.engine.take();
                if let Some(ref mut engine) = engine_opt {
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
                                    self.frame_context.pressed_keys.push_winit(k.clone());
                                    st.next_fire += rate_dt;
                                }
                            }
                        }
                    }
                    // If replaying, override frame context from recorded frame
                    if let Some(frames) = self.replay_frames.as_ref() {
                        if self.replay_cursor < frames.len() {
                            let fr = &frames[self.replay_cursor];
                            self.frame_context.pressed_keys.clear(); // key reconstruction is intentionally unsupported
                            self.frame_context.mouse_info.mouse_pos = Position {
                                x: fr.mouse_x,
                                y: fr.mouse_y,
                            };
                            self.frame_context.mouse_info.is_lmb_clicked = fr.lmb_down;
                            self.frame_context.mouse_info.is_mmb_clicked = fr.mmb_down;
                            self.frame_context.mouse_info.is_rmb_clicked = fr.rmb_down;
                            self.frame_context.text_commits = fr.committed_text.clone();
                            self.frame_context.scroll_delta = Position {
                                x: fr.scroll_dx,
                                y: fr.scroll_dy,
                            };
                            self.frame_context.mouse_info.scroll_dx = fr.scroll_dx;
                            self.frame_context.mouse_info.scroll_dy = fr.scroll_dy;
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
                                .map(Key::debug_name)
                                .collect(),
                            mouse_x: self.frame_context.mouse_info.mouse_pos.x,
                            mouse_y: self.frame_context.mouse_info.mouse_pos.y,
                            lmb_down: self.frame_context.mouse_info.is_lmb_clicked,
                            rmb_down: self.frame_context.mouse_info.is_rmb_clicked,
                            mmb_down: self.frame_context.mouse_info.is_mmb_clicked,
                            scroll_dx: self.frame_context.scroll_delta.x,
                            scroll_dy: self.frame_context.scroll_delta.y,
                            committed_text: self.frame_context.text_commits.clone(),
                        });
                    }

                    // Temporarily take the callback out of `self` so it can be called with
                    // `&mut self` without aliasing the callback field. Pass a cloned frame
                    // context snapshot to avoid borrowing `self.frame_context` across the
                    // mutable app borrow.
                    let frame_context = self.frame_context.clone();
                    let mut callback = self
                        .frame_callback
                        .take()
                        .expect("frame callback missing during frame dispatch");
                    callback(engine, &frame_context, self);
                    self.frame_callback = Some(callback);

                    // Update engine-side items (pluto objects and their textures)
                    // Pass the last pressed key this frame so interactive widgets receive input
                    let last_key = self.frame_context.pressed_keys.last_winit_cloned();
                    engine.update(
                        Some(self.frame_context.mouse_info),
                        &last_key,
                        self.frame_context.delta_time,
                    );
                    self.frame_context.scroll_delta = Position::default();
                    self.frame_context.mouse_info.scroll_dx = 0.0;
                    self.frame_context.mouse_info.scroll_dy = 0.0;
                }
                // Put engine back
                self.engine = engine_opt;

                // Request next frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }

                // Clear frame data
                self.frame_context.pressed_keys.clear();
                self.frame_context.text_commits.clear();

                if should_stop_replay {
                    self.stop_replay();
                }

                if let Some(line) = self.metrics.maybe_report() {
                    log::info!("{}", line);
                }
            }
            WindowEvent::Resized(new_size) => {
                #[cfg(target_arch = "wasm32")]
                self.sync_wasm_canvas_backing_store(
                    self.window.as_ref().map(|window| window.scale_factor()),
                );
                if let Some(engine) = &mut self.engine {
                    engine.resize(&new_size);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.apply_dpi_change(
                    scale_factor,
                    self.window.as_ref().map(|window| window.inner_size()),
                );
            }
            // On macOS, the green button triggers native fullscreen which macOS handles automatically.
            // Our set_fullscreen() uses exclusive fullscreen which works well with macOS.
            // We track our programmatic fullscreen state separately.
            WindowEvent::Focused(_focused) => {
                // Note: On macOS, native fullscreen (green button) is handled by the OS.
                // Our set_fullscreen() uses exclusive fullscreen which integrates well.
            }
            WindowEvent::CloseRequested => {
                // Auto-stop recording on exit
                let _ = self.stop_recording();
                event_loop.exit();
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request continuous redraws for smooth animation.
        // This is called after all events are processed, providing
        // more consistent timing than request_redraw() from event handlers.
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[cfg(target_arch = "wasm32")]
const DEFAULT_WASM_CANVAS_ID: &str = "game-canvas";

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct WasmRunAppError(String);

#[cfg(target_arch = "wasm32")]
impl fmt::Display for WasmRunAppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(target_arch = "wasm32")]
impl std::error::Error for WasmRunAppError {}

#[cfg(target_arch = "wasm32")]
fn js_value_to_string(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {value:?}"))
}

/// Runs a Plutonium application on the current platform.
///
/// Native targets create a `winit` window and block until the event loop exits.
/// WASM targets attach to an existing canvas with id `"game-canvas"`, spawn the
/// browser event loop, and return after startup. Use [`run_app_wasm_with_options`]
/// on WASM when a different canvas id or browser event option is needed.
pub fn run_app<F>(config: WindowConfig, frame_callback: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&mut super::PlutoniumEngine, &FrameContext, &mut PlutoniumApp) + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(target_arch = "wasm32")]
    {
        run_app_wasm_with_options_sync(
            config,
            DEFAULT_WASM_CANVAS_ID,
            WasmAppConfig::default(),
            frame_callback,
        )
        .map_err(js_value_to_string)
        .map_err(|message| Box::new(WasmRunAppError(message)) as Box<dyn std::error::Error>)
    }
}

#[cfg(target_arch = "wasm32")]
/// Runs a WASM app on the canvas identified by `canvas_id`.
pub async fn run_app_wasm<F>(
    config: WindowConfig,
    canvas_id: &str,
    frame_callback: F,
) -> Result<(), wasm_bindgen::JsValue>
where
    F: FnMut(&mut super::PlutoniumEngine, &FrameContext, &mut PlutoniumApp) + 'static,
{
    run_app_wasm_with_options(config, canvas_id, WasmAppConfig::default(), frame_callback).await
}

#[cfg(target_arch = "wasm32")]
/// Runs a WASM app with explicit browser event options.
pub async fn run_app_wasm_with_options<F>(
    config: WindowConfig,
    canvas_id: &str,
    wasm_config: WasmAppConfig,
    frame_callback: F,
) -> Result<(), wasm_bindgen::JsValue>
where
    F: FnMut(&mut super::PlutoniumEngine, &FrameContext, &mut PlutoniumApp) + 'static,
{
    run_app_wasm_with_options_sync(config, canvas_id, wasm_config, frame_callback)
}

#[cfg(target_arch = "wasm32")]
fn run_app_wasm_with_options_sync<F>(
    config: WindowConfig,
    canvas_id: &str,
    wasm_config: WasmAppConfig,
    frame_callback: F,
) -> Result<(), wasm_bindgen::JsValue>
where
    F: FnMut(&mut super::PlutoniumEngine, &FrameContext, &mut PlutoniumApp) + 'static,
{
    let window = web_sys::window()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("window is not available"))?;
    let document = window
        .document()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("document is not available"))?;
    let element = document.get_element_by_id(canvas_id).ok_or_else(|| {
        wasm_bindgen::JsValue::from_str(&format!("canvas element '{canvas_id}' was not found"))
    })?;
    let canvas = element.dyn_into::<HtmlCanvasElement>().map_err(|_| {
        wasm_bindgen::JsValue::from_str(&format!(
            "element '{canvas_id}' exists but is not an HtmlCanvasElement"
        ))
    })?;

    let event_loop = EventLoop::new().map_err(|err| {
        wasm_bindgen::JsValue::from_str(&format!("failed to create event loop: {err}"))
    })?;
    let mut app = PlutoniumApp::new(config, frame_callback);
    sync_canvas_backing_store(&canvas, Some(window.device_pixel_ratio()));
    app.wasm_canvas = Some(canvas);
    app.wasm_prevent_default = wasm_config.prevent_default;
    app.wasm_focusable = wasm_config.focusable;
    event_loop.spawn_app(app);
    Ok(())
}
