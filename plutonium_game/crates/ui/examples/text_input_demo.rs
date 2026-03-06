use plutonium_engine::app::{run_app, FrameContext, WindowConfig};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use plutonium_game_input::InputState;
use plutonium_game_ui::immediate::UIContext;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = InputState::default();
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 640.0, 480.0));
    let mut text1 = String::new();
    let mut text2 = String::new();
    let mut search = String::new();
    let mut time = 0.0;

    run_app(
        WindowConfig {
            title: "Text Input Demo".to_string(),
            width: 640,
            height: 480,
        },
        move |engine: &mut PlutoniumEngine, frame: &FrameContext, _app| {
            time += frame.delta_time;

            input.update_from_keys(frame.pressed_keys.iter().map(|k| format!("{:?}", k)));
            input.update_mouse(
                frame.mouse_info.mouse_pos.x,
                frame.mouse_info.mouse_pos.y,
                frame.mouse_info.is_lmb_clicked,
            );
            input.text_input = frame.text_commits.join("");

            let screen = Rectangle::new(0.0, 0.0, 640.0, 480.0);
            ui.begin_frame(input.clone(), screen);
            ui.set_time_seconds(time);

            ui.label("Text Input Examples");
            ui.add_space(20.0);

            ui.label("Name:");
            ui.text_input(&mut text1);
            ui.add_space(10.0);

            ui.label("Email:");
            ui.text_input_with_hint(&mut text2, "user@example.com");
            ui.add_space(10.0);

            ui.label("Search:");
            if ui
                .text_input_with_hint(&mut search, "Type to search...")
                .focused
            {
                ui.label(&format!("Searching for: '{}'", search));
            }
            ui.add_space(20.0);

            ui.label(&format!("Entered: '{}' / '{}'", text1, text2));

            ui.end_frame();

            engine.begin_frame();
            ui.render(engine);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}
