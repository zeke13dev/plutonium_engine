use plutonium_engine::app::{run_app, FrameContext, WindowConfig};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use plutonium_game_input::InputState;
use plutonium_game_ui::immediate::{vec2, UIContext};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = InputState::default();
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 800.0, 600.0));
    let mut time_seconds = 0.0;
    let mut selected_tab = 0;
    let mut modal_open = false;
    let mut slider_value = 0.5;
    let mut checked = true;

    run_app(
        WindowConfig {
            title: "Immediate UI Layout Demo".to_string(),
            width: 800,
            height: 600,
        },
        move |engine: &mut PlutoniumEngine, frame: &FrameContext, _app| {
            let key_strings = frame.pressed_keys.iter().map(|k| format!("{:?}", k));
            input.update_from_keys(key_strings);
            input.update_mouse_buttons(
                frame.mouse_info.mouse_pos.x,
                frame.mouse_info.mouse_pos.y,
                frame.mouse_info.is_lmb_clicked,
                frame.mouse_info.is_rmb_clicked,
                frame.mouse_info.is_mmb_clicked,
            );
            input.update_scroll(frame.scroll_delta.x, frame.scroll_delta.y);
            time_seconds += frame.delta_time;

            let screen = Rectangle::new(0.0, 0.0, 800.0, 600.0);
            ui.begin_frame(input.clone(), screen);
            ui.set_time_seconds(time_seconds);

            ui.panel(|ui| {
                ui.label("Layout Demo");
                ui.tabs(&mut selected_tab, &["Grid", "Scroll", "Mixed"]);
                ui.add_space(8.0);

                match selected_tab {
                    0 => {
                        let mut grid = ui.grid(4).cell_size(vec2(80.0, 80.0));
                        for i in 0..12 {
                            grid.cell(|ui| {
                                ui.button(&format!("Item {}", i));
                            });
                        }
                    }
                    1 => {
                        ui.scroll_area(|ui| {
                            for i in 0..40 {
                                ui.label(&format!("Row {}", i));
                                if i % 5 == 4 {
                                    ui.add_space(6.0);
                                }
                            }
                        });
                    }
                    _ => {
                        ui.checkbox(&mut checked, "Enable option");
                        ui.add_space(6.0);
                        ui.slider(&mut slider_value, (0.0, 1.0));
                        ui.add_space(6.0);
                        if ui.button("Open Modal").clicked() {
                            modal_open = true;
                        }
                    }
                }
            });

            let mut close_modal = false;
            ui.modal(&mut modal_open, |ui| {
                ui.label("Modal Panel");
                ui.add_space(8.0);
                ui.label("Press Escape or click Close.");
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    close_modal = true;
                }
            });
            if close_modal {
                modal_open = false;
            }

            ui.end_frame();

            engine.begin_frame();
            ui.render(engine);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}
