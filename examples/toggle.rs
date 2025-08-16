use plutonium_engine::app::run_app;
use plutonium_engine::ui::{draw_toggle, ToggleStyle};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::WindowConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Toggle Demo".into(),
        width: 480,
        height: 200,
    };

    let mut on = false;
    let focused = true;

    run_app(config, move |engine, frame| {
        engine.begin_frame();
        // Background
        let bg = Rectangle::new(0.0, 0.0, 480.0, 200.0);
        engine.draw_rect(bg, [0.08, 0.09, 0.12, 1.0], 0.0, None, 0);

        let track = Rectangle::new(140.0, 80.0, 200.0, 40.0);

        // Toggle interactions: click toggles; right click toggles focus for demo
        if frame.mouse_info.is_lmb_clicked && track.contains(frame.mouse_info.mouse_pos) {
            on = !on;
        }
        // Draw
        let style = ToggleStyle::default();
        draw_toggle(engine, track, on, focused, &style, 1);

        engine.end_frame().unwrap();
    })
}
