use plutonium_engine::app::{run_app, FrameContext, WindowConfig};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use plutonium_game_input::InputState;
use plutonium_game_ui::immediate::{HaloPreset, HaloStyle, UIContext};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = InputState::default();
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 640.0, 360.0));
    let mut counter = 0;
    let mut time_seconds = 0.0;

    run_app(
        WindowConfig {
            title: "Immediate UI Hello World".to_string(),
            width: 640,
            height: 360,
        },
        move |engine: &mut PlutoniumEngine, frame: &FrameContext, _app| {
            let key_strings = frame.pressed_keys.iter().map(|k| format!("{:?}", k));
            input.update_from_keys(key_strings);
            input.update_mouse(
                frame.mouse_info.mouse_pos.x,
                frame.mouse_info.mouse_pos.y,
                frame.mouse_info.is_lmb_clicked,
            );
            input.update_scroll(frame.scroll_delta.x, frame.scroll_delta.y);
            time_seconds += frame.delta_time;

            let screen = Rectangle::new(0.0, 0.0, 640.0, 360.0);
            ui.begin_frame(input.clone(), screen);
            ui.set_time_seconds(time_seconds);

            ui.label("Hello, Plutonium UI!");
            let click_me = ui.button("Click me!");
            if click_me.clicked() {
                counter += 1;
            }
            ui.halo_response(
                &click_me,
                HaloStyle {
                    time_seconds,
                    radius: 40.0,
                    max_alpha: 0.5,
                    ..HaloStyle::from_preset(HaloPreset::TutorialPrimary)
                },
            );
            ui.label(&format!("Clicked {} times", counter));

            ui.end_frame();

            engine.begin_frame();
            ui.render(engine);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}
