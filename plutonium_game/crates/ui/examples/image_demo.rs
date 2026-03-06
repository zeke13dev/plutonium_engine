use plutonium_engine::app::{run_app, FrameContext, WindowConfig};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use plutonium_game_input::InputState;
use plutonium_game_ui::immediate::{vec2, UIContext};
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = InputState::default();
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 800.0, 600.0));
    let texture_id: Option<Uuid> = None;

    run_app(
        WindowConfig {
            title: "Image Widget Demo".to_string(),
            width: 800,
            height: 600,
        },
        move |engine: &mut PlutoniumEngine, frame: &FrameContext, _app| {
            if texture_id.is_none() {
                // TODO: Load an actual image file and set texture_id
                // texture_id = Some(engine.load_texture("path/to/image.png"));
            }

            input.update_from_keys(frame.pressed_keys.iter().map(|k| format!("{:?}", k)));
            input.update_mouse(
                frame.mouse_info.mouse_pos.x,
                frame.mouse_info.mouse_pos.y,
                frame.mouse_info.is_lmb_clicked,
            );

            let screen = Rectangle::new(0.0, 0.0, 800.0, 600.0);
            ui.begin_frame(input.clone(), screen);

            ui.label("Image Widget Examples");
            ui.add_space(20.0);

            if let Some(tex) = texture_id {
                ui.label("Regular image:");
                ui.image(tex, vec2(64.0, 64.0));
                ui.add_space(10.0);

                ui.label("Image button (hover to see highlight):");
                if ui.image_button(tex, vec2(64.0, 64.0)).clicked() {
                    println!("Image button clicked!");
                }
            } else {
                ui.label("No texture loaded");
            }

            ui.end_frame();

            engine.begin_frame();
            ui.render(engine);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}
