use plutonium_engine::app::run_app;
use plutonium_engine::input::{ActionMap, AxisSource, ButtonSource};
use plutonium_engine::ui::{draw_button_background, ButtonStyle, ButtonVisualState};
use plutonium_engine::utils::Rectangle;
use plutonium_engine::WindowConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Actions + Button States".into(),
        width: 640,
        height: 240,
    };

    let mut actions = ActionMap::new();
    actions.bind_button("toggle_hover", ButtonSource::Key("Character(\"H\")".into()));
    actions.bind_button("toggle_press", ButtonSource::Key("Character(\"P\")".into()));
    actions.bind_button("toggle_focus", ButtonSource::Key("Character(\"F\")".into()));
    actions.bind_button("click", ButtonSource::MouseLeft);
    actions.bind_axis(
        "move_x",
        AxisSource::KeyPair {
            negative: "Named(ArrowLeft)".into(),
            positive: "Named(ArrowRight)".into(),
        },
        1.0,
        0.0,
    );

    let mut state = ButtonVisualState {
        hovered: false,
        pressed: false,
        focused: false,
    };
    let style = ButtonStyle::default();

    run_app(config, move |engine, frame| {
        engine.begin_frame();
        // Background
        let bg = Rectangle::new(0.0, 0.0, 640.0, 240.0);
        engine.draw_rect(bg, [0.10, 0.11, 0.14, 1.0], 0.0, None, 0);

        let (pressed_actions, axes) = actions.resolve(frame);
        if pressed_actions.contains("toggle_hover") {
            state.hovered = !state.hovered;
        }
        if pressed_actions.contains("toggle_press") || pressed_actions.contains("click") {
            state.pressed = !state.pressed;
        }
        if pressed_actions.contains("toggle_focus") {
            state.focused = !state.focused;
        }
        let dx = axes.get("move_x").copied().unwrap_or(0.0) * 3.0;

        let rect = Rectangle::new(80.0 + dx, 80.0, 200.0, 64.0);
        // Base panel behind
        engine.draw_rect(
            Rectangle::new(
                rect.x - 16.0,
                rect.y - 16.0,
                rect.width + 32.0,
                rect.height + 32.0,
            ),
            [0.16, 0.17, 0.22, 1.0],
            8.0,
            None,
            1,
        );
        draw_button_background(engine, rect, &state, &style, 2);

        engine.end_frame().unwrap();
    })
}
