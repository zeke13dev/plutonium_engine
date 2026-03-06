use plutonium_engine::app::{run_app, FrameContext, WindowConfig};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use plutonium_game_input::InputState;
use plutonium_game_ui::immediate::{vec2, Color, UIContext};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = InputState::default();
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 640.0, 480.0));
    let mut time = 0.0f32;

    run_app(
        WindowConfig {
            title: "Progress Bar Demo".to_string(),
            width: 640,
            height: 480,
        },
        move |engine: &mut PlutoniumEngine, frame: &FrameContext, _app| {
            time += frame.delta_time;
            let progress = ((time * 0.5).sin() + 1.0) * 0.5;

            input.update_from_keys(frame.pressed_keys.iter().map(|k| format!("{:?}", k)));
            input.update_mouse(
                frame.mouse_info.mouse_pos.x,
                frame.mouse_info.mouse_pos.y,
                frame.mouse_info.is_lmb_clicked,
            );

            let screen = Rectangle::new(0.0, 0.0, 640.0, 480.0);
            ui.begin_frame(input.clone(), screen);

            ui.label("Progress Bar Examples");
            ui.add_space(20.0);

            ui.label("Basic:");
            ui.progress_bar(progress);
            ui.add_space(10.0);

            ui.label("Wide and thin:");
            ui.progress_bar_sized(progress, vec2(300.0, 10.0));
            ui.add_space(10.0);

            ui.label("Color-coded (freshness example):");
            let color = if progress > 0.7 {
                Color::GREEN
            } else if progress > 0.3 {
                Color::YELLOW
            } else {
                Color::RED
            };
            ui.progress_bar_colored(progress, vec2(200.0, 20.0), color);
            ui.add_space(10.0);

            ui.label("With percentage:");
            ui.progress_bar_labeled(progress, format!("{:.0}%", progress * 100.0));

            ui.end_frame();

            engine.begin_frame();
            ui.render(engine);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}
