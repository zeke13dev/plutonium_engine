use plutonium_engine::app::run_app;
use plutonium_engine::input::{ActionMap, AxisSource, ButtonSource};
use plutonium_engine::ui::{draw_focus_ring, FocusRingStyle};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::WindowConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "UI Primitives".into(),
        width: 800,
        height: 600,
    };

    let mut focus: bool = true;
    let mut actions = ActionMap::new();
    actions.bind_button("toggle_focus", ButtonSource::Key("Character(\"F\")".into()));
    actions.bind_button("click", ButtonSource::MouseLeft);
    actions.bind_axis(
        "move_x",
        AxisSource::KeyPair {
            negative: "Named(ArrowLeft)".into(),
            positive: "Named(ArrowRight)".into(),
        },
        1.0,
        0.0,
    );
    run_app(config, move |engine, frame| {
        engine.begin_frame();

        // Panel
        let panel = Rectangle::new(80.0, 60.0, 640.0, 420.0);
        engine.draw_rect(
            panel,
            [0.12, 0.12, 0.15, 1.0],
            12.0,
            Some(([0.2, 0.2, 0.25, 1.0], 2.0)),
            0,
        );

        // Demonstrate nested clipping: clip to panel, then to an inner area
        engine.push_clip(panel);
        let inner = Rectangle::new(
            panel.x + 20.0,
            panel.y + 20.0,
            panel.width - 40.0,
            panel.height - 40.0,
        );
        engine.push_clip(inner);

        // Draw content inside inner clip (visible)
        let inside = Rectangle::new(inner.x + 10.0, inner.y + 10.0, 200.0, 80.0);
        engine.draw_rect(
            inside,
            [0.25, 0.3, 0.6, 1.0],
            8.0,
            Some(([0.15, 0.18, 0.4, 1.0], 2.0)),
            1,
        );

        // Draw content partially outside inner but inside panel (only the portion inside inner should show)
        let partly = Rectangle::new(inner.x - 30.0, inner.y + inner.height - 30.0, 120.0, 60.0);
        engine.draw_rect(partly, [0.2, 0.6, 0.5, 1.0], 6.0, None, 1);

        // Pop inner clip
        engine.pop_clip();

        // Resolve actions and move demo rect horizontally
        let (pressed, axes) = actions.resolve(frame);
        if pressed.contains("toggle_focus") || pressed.contains("click") {
            focus = !focus;
        }
        let dx = axes.get("move_x").copied().unwrap_or(0.0) * 2.0;

        // Draw something outside inner but inside panel (visible now that inner clip popped)
        let ring_area = Rectangle::new(panel.x + 300.0 + dx, panel.y + 280.0, 220.0, 100.0);
        engine.draw_rect(ring_area, [0.18, 0.18, 0.22, 1.0], 10.0, None, 1);

        // Focus ring toggle with mouse click
        if frame.mouse_info.is_lmb_clicked {
            focus = !focus;
        }
        if focus {
            draw_focus_ring(
                engine,
                Rectangle::new(
                    ring_area.x - 4.0,
                    ring_area.y - 4.0,
                    ring_area.width + 8.0,
                    ring_area.height + 8.0,
                ),
                FocusRingStyle {
                    thickness_px: 2.0,
                    color: [1.0, 0.9, 0.2, 1.0],
                    corner_radius_px: 10.0,
                    inset_px: 0.0,
                },
                2,
            );
        }

        // Pop outer clip (panel)
        engine.pop_clip();

        // Draw an out-of-panel rect to show clipping difference (should not be clipped anymore)
        let outside_panel = Rectangle::new(panel.x - 30.0, panel.y - 30.0, 40.0, 40.0);
        engine.draw_rect(outside_panel, [0.8, 0.2, 0.2, 1.0], 4.0, None, 0);

        // End frame
        engine.end_frame().unwrap();
    })?;

    Ok(())
}
