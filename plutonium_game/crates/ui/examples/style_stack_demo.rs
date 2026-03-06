use plutonium_engine::app::{run_app, FrameContext, WindowConfig};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use plutonium_game_input::InputState;
use plutonium_game_ui::immediate::context::UiStyle;
use plutonium_game_ui::immediate::{Color, UIContext};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = InputState::default();
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 640.0, 480.0));

    run_app(
        WindowConfig {
            title: "Style Stack Demo".to_string(),
            width: 640,
            height: 480,
        },
        move |engine: &mut PlutoniumEngine, frame: &FrameContext, _app| {
            input.update_from_keys(frame.pressed_keys.iter().map(|k| format!("{:?}", k)));
            input.update_mouse(
                frame.mouse_info.mouse_pos.x,
                frame.mouse_info.mouse_pos.y,
                frame.mouse_info.is_lmb_clicked,
            );

            let screen = Rectangle::new(0.0, 0.0, 640.0, 480.0);
            ui.begin_frame(input.clone(), screen);

            ui.label("Style Stack Examples");
            ui.add_space(20.0);

            ui.button("Default Button");
            ui.add_space(10.0);

            ui.with_accent_color(Color::RED, |ui| {
                ui.button("Red Button");

                let mut value = 0.5;
                ui.slider(&mut value, (0.0, 1.0));
            });
            ui.add_space(10.0);

            ui.with_font_size(24.0, |ui| {
                ui.label("Large Text");
            });
            ui.add_space(10.0);

            ui.with_accent_color(Color::GREEN, |ui| {
                ui.button("Green Outer");

                ui.with_accent_color(Color::BLUE, |ui| {
                    ui.button("Blue Inner");
                });

                ui.button("Green Again");
            });
            ui.add_space(10.0);

            let mut custom_style = UiStyle::default();
            custom_style.accent_color = Color::ORANGE;
            custom_style.button_color = Color::rgb(50, 30, 10);
            custom_style.font_size = 20.0;

            ui.with_style(custom_style, |ui| {
                ui.label("Custom Theme Section");
                ui.button("Orange Theme Button");
            });

            ui.end_frame();

            engine.begin_frame();
            ui.render(engine);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}
