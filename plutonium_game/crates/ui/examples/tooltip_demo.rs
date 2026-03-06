use plutonium_engine::app::{run_app, FrameContext, WindowConfig};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use plutonium_game_input::InputState;
use plutonium_game_ui::immediate::UIContext;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = InputState::default();
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 640.0, 480.0));
    let mut time = 0.0;

    run_app(
        WindowConfig {
            title: "Tooltip Demo".to_string(),
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

            let screen = Rectangle::new(0.0, 0.0, 640.0, 480.0);
            ui.begin_frame(input.clone(), screen);
            ui.set_time_seconds(time);

            ui.label("Hover over widgets to see tooltips");
            ui.add_space(20.0);

            let resp = ui.button("Button with tooltip");
            ui.process_response(&resp.on_hover_text("Click to do something!"));

            ui.add_space(10.0);

            let resp = ui.button("Another button");
            ui.process_response(&resp.on_hover_text(
                "This is a longer tooltip that explains what this button does in more detail.",
            ));

            ui.add_space(10.0);

            let mut value = 0.5;
            let resp = ui.slider(&mut value, (0.0, 1.0));
            ui.process_response(&resp.on_hover_text(&format!("Value: {:.2}", value)));

            ui.end_frame();

            engine.begin_frame();
            ui.render(engine);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}
