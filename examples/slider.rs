use plutonium_engine::{
    app::run_app,
    ui::{draw_focus_ring, FocusRingStyle},
    utils::{Position, Rectangle},
    WindowConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "UI Primitives – Slider".to_string(),
        width: 480,
        height: 180,
    };

    // Slider state captured by the closure
    let mut value: f32 = 0.35; // 0..1
    let mut focused: bool = false;
    let mut dragging: bool = false;

    run_app(config, move |engine, frame| {
        // Immediate-mode frame
        engine.begin_frame();

        // Layout constants (logical px)
        let origin = Position { x: 40.0, y: 70.0 };
        let track_w = 360.0f32;
        let track_h = 10.0f32;
        let thumb_w = 20.0f32;
        let thumb_h = 28.0f32;
        let corner = 6.0f32;

        // Compute rects
        let track_rect = Rectangle::new(origin.x, origin.y, track_w, track_h);
        let thumb_x = origin.x + value.clamp(0.0, 1.0) * (track_w - thumb_w);
        let thumb_y = origin.y + track_h * 0.5 - thumb_h * 0.5;
        let thumb_rect = Rectangle::new(thumb_x.floor(), thumb_y.floor(), thumb_w, thumb_h);

        // Input handling
        let mouse = frame.mouse_info;
        let mpos = mouse.mouse_pos;
        let over_thumb = mpos.x >= thumb_rect.x
            && mpos.x <= thumb_rect.x + thumb_rect.width
            && mpos.y >= thumb_rect.y
            && mpos.y <= thumb_rect.y + thumb_rect.height;
        let over_track = mpos.x >= track_rect.x
            && mpos.x <= track_rect.x + track_rect.width
            && mpos.y >= track_rect.y - 10.0
            && mpos.y <= track_rect.y + track_rect.height + 10.0;

        // Click to focus / start drag, or clear focus when clicked elsewhere
        if mouse.is_lmb_clicked {
            if over_thumb || over_track {
                focused = true;
                let _ = dragging;
                dragging = true;
                // Snap immediately when clicking track
                let rel =
                    ((mpos.x - origin.x - thumb_w * 0.5) / (track_w - thumb_w)).clamp(0.0, 1.0);
                value = rel;
            } else {
                focused = false;
                dragging = false;
            }
        } else {
            // Mouse released
            dragging = false;
        }

        // Keyboard: toggle focus with Tab; clear focus with Escape
        for k in &frame.pressed_keys {
            match k {
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab) => {
                    focused = !focused;
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                    focused = false;
                    dragging = false;
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowLeft) if focused => {
                    value = (value - 0.02).clamp(0.0, 1.0);
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight) if focused => {
                    value = (value + 0.02).clamp(0.0, 1.0);
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Home) if focused => {
                    value = 0.0;
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::End) if focused => {
                    value = 1.0;
                }
                _ => {}
            }
        }

        // Drag to update value
        if dragging {
            let rel = ((mpos.x - origin.x - thumb_w * 0.5) / (track_w - thumb_w)).clamp(0.0, 1.0);
            value = rel;
        }

        // Draw track (background)
        engine.draw_rect(track_rect, [0.18, 0.20, 0.24, 1.0], track_h * 0.5, None, 0);
        // Draw filled portion
        let filled_w = (thumb_x + thumb_w * 0.5 - origin.x).clamp(0.0, track_w);
        let filled = Rectangle::new(origin.x, origin.y, filled_w, track_h);
        engine.draw_rect(filled, [0.36, 0.56, 0.98, 1.0], track_h * 0.5, None, 1);

        // Draw thumb
        engine.draw_rect(
            thumb_rect,
            [0.92, 0.94, 0.96, 1.0],
            corner,
            Some(([0.12, 0.14, 0.18, 1.0], 1.0)),
            2,
        );

        // Focus ring
        if focused {
            let style = FocusRingStyle {
                thickness_px: 3.0,
                color: [1.0, 0.85, 0.30, 1.0],
                corner_radius_px: corner + 2.0,
                inset_px: 2.0,
            };
            // Slightly outset ring around thumb
            let ring_rect = Rectangle::new(
                thumb_rect.x - style.inset_px,
                thumb_rect.y - style.inset_px,
                thumb_rect.width + style.inset_px * 2.0,
                thumb_rect.height + style.inset_px * 2.0,
            );
            draw_focus_ring(engine, ring_rect, style, 3);
        }

        // End frame (submit draw queue)
        let _ = engine.end_frame();
    })
    .map(|_| ())
}
