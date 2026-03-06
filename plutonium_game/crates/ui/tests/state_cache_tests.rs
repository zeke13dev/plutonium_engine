use plutonium_engine::utils::Rectangle;
use plutonium_game_ui::immediate::*;

#[test]
fn collapsing_header_remembers_state() {
    let mut ui = UIContext::new(Rectangle::new(0.0, 0.0, 640.0, 480.0));
    let id = WidgetId::new(123);

    ui.set_state(id, true);
    assert_eq!(ui.get_state::<bool>(id), Some(true));

    assert_eq!(ui.get_state::<bool>(id), Some(true));

    ui.set_state(id, false);
    assert_eq!(ui.get_state::<bool>(id), Some(false));
}
