//! Minimal jitter/stutter test for debugging frame timing issues.
//!
//! Run with: cargo run --example jitter_test
//!
//! Controls:
//!   Arrow keys / WASD - Move the square
//!   C - Toggle camera follow (on/off)
//!   Space - Toggle camera lock (locked vs smoothed)
//!   T - Toggle dt smoothing
//!   L - Toggle CSV logging
//!   R - Reset position

use plutonium_engine::{
    app::{run_app, FrameContext, PlutoniumApp, WindowConfig},
    utils::{Position, Rectangle},
    PlutoniumEngine,
};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

// Track timing more precisely
struct FrameTiming {
    last_present_end: Instant,
}

impl FrameTiming {
    fn new() -> Self {
        Self {
            last_present_end: Instant::now(),
        }
    }

    fn mark_present_end(&mut self) -> f32 {
        let now = Instant::now();
        let dt = (now - self.last_present_end).as_secs_f32();
        self.last_present_end = now;
        dt
    }
}

struct TestState {
    // Position (logical)
    player_pos: Position,
    // Visual positions (smoothed for rendering)
    player_visual: Position,
    camera_pos: Position,
    speed: f32,

    // Settings
    camera_enabled: bool,
    camera_locked: bool,
    dt_smoothing: bool,
    player_smoothing: bool, // NEW: smooth player visual position
    avg_dt: f32,

    // Timing
    frame_count: u64,
    frame_times: VecDeque<f32>,
    last_report: Instant,

    // Logging
    log_file: Option<File>,
    logging: bool,

    // Debounce
    space_was_pressed: bool,
    t_was_pressed: bool,
    l_was_pressed: bool,
    p_was_pressed: bool,
    c_was_pressed: bool,
    r_was_pressed: bool,

    // Precise timing (measured after present)
    timing: FrameTiming,
    present_dts: VecDeque<f32>,
}

impl TestState {
    fn new() -> Self {
        Self {
            player_pos: Position { x: 400.0, y: 300.0 },
            player_visual: Position { x: 400.0, y: 300.0 },
            camera_pos: Position { x: 0.0, y: 0.0 },
            speed: 280.0,
            camera_enabled: true,
            camera_locked: false,
            dt_smoothing: false,
            player_smoothing: true, // Start with player smoothing ON
            avg_dt: 0.016,
            frame_count: 0,
            frame_times: VecDeque::with_capacity(120),
            last_report: Instant::now(),
            log_file: None,
            logging: false,
            space_was_pressed: false,
            t_was_pressed: false,
            l_was_pressed: false,
            p_was_pressed: false,
            c_was_pressed: false,
            r_was_pressed: false,
            timing: FrameTiming::new(),
            present_dts: VecDeque::with_capacity(120),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = WindowConfig {
        title: "Jitter Test".to_string(),
        width: 800,
        height: 600,
    };

    let mut state = TestState::new();

    run_app(
        config,
        move |engine: &mut PlutoniumEngine, frame: &FrameContext, app: &mut PlutoniumApp| {
            let raw_dt = frame.delta_time;
            state.avg_dt += (raw_dt - state.avg_dt) * 0.1;
            let dt = if state.dt_smoothing {
                state.avg_dt
            } else {
                raw_dt
            };

            // Input
            let mut dx = 0.0f32;
            let mut dy = 0.0f32;
            use winit::keyboard::NamedKey;
            if app.is_named_key_down(NamedKey::ArrowUp) || app.is_char_key_down('w') {
                dy -= 1.0;
            }
            if app.is_named_key_down(NamedKey::ArrowDown) || app.is_char_key_down('s') {
                dy += 1.0;
            }
            if app.is_named_key_down(NamedKey::ArrowLeft) || app.is_char_key_down('a') {
                dx -= 1.0;
            }
            if app.is_named_key_down(NamedKey::ArrowRight) || app.is_char_key_down('d') {
                dx += 1.0;
            }

            let space_pressed = app.is_named_key_down(NamedKey::Space);
            let t_pressed = app.is_char_key_down('t');
            let l_pressed = app.is_char_key_down('l');
            let p_pressed = app.is_char_key_down('p');
            let c_pressed = app.is_char_key_down('c');
            let r_pressed = app.is_char_key_down('r');

            if r_pressed && !state.r_was_pressed {
                state.player_pos = Position { x: 400.0, y: 300.0 };
                state.player_visual = Position { x: 400.0, y: 300.0 };
                state.camera_pos = Position { x: 0.0, y: 0.0 };
            }
            state.r_was_pressed = r_pressed;

            // Toggle camera lock (with debounce)
            if space_pressed && !state.space_was_pressed {
                state.camera_locked = !state.camera_locked;
                println!("Camera locked: {}", state.camera_locked);
            }
            state.space_was_pressed = space_pressed;

            // Toggle dt smoothing (with debounce)
            if t_pressed && !state.t_was_pressed {
                state.dt_smoothing = !state.dt_smoothing;
                println!("DT smoothing: {}", state.dt_smoothing);
            }
            state.t_was_pressed = t_pressed;

            // Toggle logging (with debounce)
            if l_pressed && !state.l_was_pressed {
                state.logging = !state.logging;
                if state.logging && state.log_file.is_none() {
                    let name = format!(
                        "jitter_{}.csv",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    );
                    if let Ok(mut f) = File::create(&name) {
                        writeln!(f, "frame,dt_ms,player_x,camera_x").ok();
                        state.log_file = Some(f);
                        println!("Logging to: {}", name);
                    }
                } else {
                    println!("Logging: {}", state.logging);
                }
            }
            state.l_was_pressed = l_pressed;

            // Toggle player smoothing (with debounce)
            if p_pressed && !state.p_was_pressed {
                state.player_smoothing = !state.player_smoothing;
                println!("Player smoothing: {}", state.player_smoothing);
            }
            state.p_was_pressed = p_pressed;

            // Toggle camera active/inactive (with debounce)
            if c_pressed && !state.c_was_pressed {
                state.camera_enabled = !state.camera_enabled;
                println!("Camera follow enabled: {}", state.camera_enabled);
            }
            state.c_was_pressed = c_pressed;

            // Normalize diagonal
            if dx != 0.0 || dy != 0.0 {
                let len = (dx * dx + dy * dy).sqrt();
                dx /= len;
                dy /= len;
            }

            // Move player (logical position)
            state.player_pos.x += dx * state.speed * dt;
            state.player_pos.y += dy * state.speed * dt;

            // Use smoothed dt for interpolation to get consistent visual smoothing
            // even when individual frame times vary wildly
            let smooth_dt = state.avg_dt;

            // ALWAYS smooth player visual - this is what gets rendered
            if state.player_smoothing {
                let smooth = 1.0 - (-25.0 * smooth_dt).exp();
                state.player_visual.x += (state.player_pos.x - state.player_visual.x) * smooth;
                state.player_visual.y += (state.player_pos.y - state.player_visual.y) * smooth;
            } else {
                state.player_visual = state.player_pos;
            }

            // Camera deadzone behavior:
            // when enabled, camera only moves after player exits the center deadzone.
            let (window_w, window_h) = if let Some(window) = app.window() {
                let size = window.inner_size();
                let scale = window.scale_factor() as f32;
                (
                    (size.width as f32) / scale.max(1.0),
                    (size.height as f32) / scale.max(1.0),
                )
            } else {
                (800.0, 600.0)
            };
            let deadzone_w = 220.0;
            let deadzone_h = 160.0;
            let dz_x = (window_w - deadzone_w) * 0.5;
            let dz_y = (window_h - deadzone_h) * 0.5;

            if state.camera_enabled {
                let deadzone_left = state.camera_pos.x + dz_x;
                let deadzone_top = state.camera_pos.y + dz_y;
                let deadzone_right = deadzone_left + deadzone_w;
                let deadzone_bottom = deadzone_top + deadzone_h;

                let target = Position {
                    x: state.camera_pos.x
                        + if state.player_visual.x > deadzone_right {
                            state.player_visual.x - deadzone_right
                        } else if state.player_visual.x < deadzone_left {
                            state.player_visual.x - deadzone_left
                        } else {
                            0.0
                        },
                    y: state.camera_pos.y
                        + if state.player_visual.y > deadzone_bottom {
                            state.player_visual.y - deadzone_bottom
                        } else if state.player_visual.y < deadzone_top {
                            state.player_visual.y - deadzone_top
                        } else {
                            0.0
                        },
                };

                if state.camera_locked {
                    state.camera_pos = target;
                } else {
                    let smooth = 1.0 - (-25.0 * smooth_dt).exp();
                    state.camera_pos.x += (target.x - state.camera_pos.x) * smooth;
                    state.camera_pos.y += (target.y - state.camera_pos.y) * smooth;
                }
            }

            // === RENDER ===
            engine.begin_frame();

            // Background
            engine.draw_rect(
                Rectangle::new(0.0, 0.0, window_w, window_h),
                [0.1, 0.1, 0.15, 1.0],
                0.0,
                None,
                0,
            );

            // Grid
            let grid_size = 50.0;
            let offset_x = (-state.camera_pos.x).rem_euclid(grid_size);
            let offset_y = (-state.camera_pos.y).rem_euclid(grid_size);

            for i in 0..((window_w / grid_size) as i32 + 2) {
                let x = offset_x + i as f32 * grid_size;
                engine.draw_rect(
                    Rectangle::new(x, 0.0, 1.0, window_h),
                    [0.25, 0.25, 0.3, 1.0],
                    0.0,
                    None,
                    1,
                );
            }
            for i in 0..((window_h / grid_size) as i32 + 2) {
                let y = offset_y + i as f32 * grid_size;
                engine.draw_rect(
                    Rectangle::new(0.0, y, window_w, 1.0),
                    [0.25, 0.25, 0.3, 1.0],
                    0.0,
                    None,
                    1,
                );
            }

            // Player (render at visual position, not logical)
            let px = state.player_visual.x - state.camera_pos.x;
            let py = state.player_visual.y - state.camera_pos.y;
            let size = 40.0;
            engine.draw_rect(
                Rectangle::new(px - size * 0.5, py - size * 0.5, size, size),
                [0.9, 0.3, 0.3, 1.0],
                0.0,
                None,
                10,
            );

            // Draw deadzone in screen-space at the center for easy camera-on/off comparison.
            let deadzone_color = if state.camera_enabled {
                [0.15, 0.85, 0.45, 1.0]
            } else {
                [0.85, 0.55, 0.15, 1.0]
            };
            engine.draw_rect(
                Rectangle::new(dz_x, dz_y, deadzone_w, 1.0),
                deadzone_color,
                0.0,
                None,
                11,
            );
            engine.draw_rect(
                Rectangle::new(dz_x, dz_y + deadzone_h, deadzone_w, 1.0),
                deadzone_color,
                0.0,
                None,
                11,
            );
            engine.draw_rect(
                Rectangle::new(dz_x, dz_y, 1.0, deadzone_h),
                deadzone_color,
                0.0,
                None,
                11,
            );
            engine.draw_rect(
                Rectangle::new(dz_x + deadzone_w, dz_y, 1.0, deadzone_h),
                deadzone_color,
                0.0,
                None,
                11,
            );

            engine.end_frame().unwrap();

            // Measure time AFTER present completes
            let present_dt = state.timing.mark_present_end();

            // Stats
            state.frame_times.push_back(raw_dt * 1000.0);
            state.present_dts.push_back(present_dt * 1000.0);
            if state.frame_times.len() > 120 {
                state.frame_times.pop_front();
            }
            if state.present_dts.len() > 120 {
                state.present_dts.pop_front();
            }
            state.frame_count += 1;

            // Log
            if state.logging {
                if let Some(ref mut f) = state.log_file {
                    writeln!(
                        f,
                        "{},{:.4},{:.4},{:.2},{:.2}",
                        state.frame_count,
                        raw_dt * 1000.0,
                        present_dt * 1000.0,
                        state.player_pos.x,
                        state.camera_pos.x
                    )
                    .ok();
                }
            }

            // Report every second - compare both timing methods
            if state.last_report.elapsed().as_secs_f32() >= 1.0 && !state.frame_times.is_empty() {
                // Engine's dt (measured at start of frame)
                let mut sorted: Vec<f32> = state.frame_times.iter().copied().collect();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let len = sorted.len();
                let avg = sorted.iter().sum::<f32>() / len as f32;
                let min = sorted[0];
                let max = sorted[len - 1];

                // Our dt (measured after present)
                let mut sorted_p: Vec<f32> = state.present_dts.iter().copied().collect();
                sorted_p.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let len_p = sorted_p.len();
                let avg_p = sorted_p.iter().sum::<f32>() / len_p as f32;
                let min_p = sorted_p[0];
                let max_p = sorted_p[len_p - 1];

                println!(
                "dt(start): avg={:.2} min={:.2} max={:.2} | dt(present): avg={:.2} min={:.2} max={:.2} | cam_on={} cam_lock={} dt_smooth={} player_smooth={}",
                avg,
                min,
                max,
                avg_p,
                min_p,
                max_p,
                state.camera_enabled,
                state.camera_locked,
                state.dt_smoothing,
                state.player_smoothing
            );
                state.last_report = Instant::now();
            }
        },
    )
}
