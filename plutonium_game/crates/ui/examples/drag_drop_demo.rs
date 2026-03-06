use plutonium_engine::app::{run_app, FrameContext, WindowConfig};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use plutonium_game_input::InputState;
use plutonium_game_ui::immediate::UIContext;

#[derive(Clone, Debug)]
struct Item {
    name: String,
    id: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = InputState::default();
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 800.0, 600.0));

    let mut left_items = vec![
        Item {
            name: "Apple".to_string(),
            id: 1,
        },
        Item {
            name: "Banana".to_string(),
            id: 2,
        },
        Item {
            name: "Cherry".to_string(),
            id: 3,
        },
    ];
    let mut right_items: Vec<Item> = Vec::new();

    run_app(
        WindowConfig {
            title: "Drag & Drop Demo".to_string(),
            width: 800,
            height: 600,
        },
        move |engine: &mut PlutoniumEngine, frame: &FrameContext, _app| {
            input.update_from_keys(frame.pressed_keys.iter().map(|k| format!("{:?}", k)));
            input.update_mouse(
                frame.mouse_info.mouse_pos.x,
                frame.mouse_info.mouse_pos.y,
                frame.mouse_info.is_lmb_clicked,
            );

            let screen = Rectangle::new(0.0, 0.0, 800.0, 600.0);
            ui.begin_frame(input.clone(), screen);

            ui.label("Drag items between lists");
            ui.add_space(20.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Left List");
                    ui.add_space(10.0);

                    let mut to_remove = None;
                    for (idx, item) in left_items.iter().enumerate() {
                        ui.drag_source(("left", item.id), item.clone(), |ui| {
                            if ui.button(&item.name).clicked() {
                                println!("Clicked: {}", item.name);
                            }
                        });

                        let (_resp, dropped) = ui.drop_target::<Item>(|ui| {
                            ui.add_space(5.0);
                        });

                        if let Some(dropped_item) = dropped {
                            println!("Dropped {} at position {}", dropped_item.name, idx);
                            to_remove = Some(dropped_item.id);
                        }
                    }

                    if let Some(id) = to_remove {
                        left_items.retain(|item| item.id != id);
                    }
                });

                ui.add_space(40.0);

                ui.vertical(|ui| {
                    ui.label("Right List");
                    ui.add_space(10.0);

                    for item in &right_items {
                        ui.button(&item.name);
                    }

                    ui.add_space(10.0);

                    let (_resp, dropped) = ui.drop_target::<Item>(|ui| {
                        ui.panel(|ui| {
                            ui.label("Drop here");
                            ui.add_space(40.0);
                        });
                    });

                    if let Some(item) = dropped {
                        println!("Dropped item: {:?}", item);
                        right_items.push(item);
                        left_items.retain(|i| i.id != right_items.last().unwrap().id);
                    }
                });
            });

            ui.end_frame();

            engine.begin_frame();
            ui.render(engine);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}
