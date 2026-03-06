#![forbid(unsafe_code)]

use plutonium_engine::utils::Position;
use plutonium_game_input::InputState;

pub type UiInputState = InputState;

pub trait InputStateExt {
    fn pointer_pos(&self) -> Position;
}

impl InputStateExt for InputState {
    fn pointer_pos(&self) -> Position {
        Position {
            x: self.mouse_x,
            y: self.mouse_y,
        }
    }
}
