#![forbid(unsafe_code)]

use crate::immediate::input::UiInputState;
use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum Action {
    MenuUp,
    MenuDown,
    MenuLeft,
    MenuRight,
    Select,
    Back,
    NextTab,
    PrevTab,
    Custom(u32),
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum MouseButton {
    Left,
}

#[derive(Debug, Clone)]
pub enum InputBinding {
    Key(&'static str),
    MouseButton(MouseButton),
}

#[derive(Debug, Clone)]
pub struct InputMap {
    bindings: HashMap<Action, Vec<InputBinding>>,
}

impl InputMap {
    pub fn new() -> Self {
        InputMap {
            bindings: HashMap::new(),
        }
    }

    pub fn bind(&mut self, action: Action, binding: InputBinding) {
        self.bindings.entry(action).or_default().push(binding);
    }

    pub fn is_action_pressed(&self, action: Action, input: &UiInputState) -> bool {
        self.bindings
            .get(&action)
            .map(|bindings| bindings.iter().any(|b| binding_pressed(b, input)))
            .unwrap_or(false)
    }

    pub fn is_action_just_pressed(&self, action: Action, input: &UiInputState) -> bool {
        self.bindings
            .get(&action)
            .map(|bindings| bindings.iter().any(|b| binding_just_pressed(b, input)))
            .unwrap_or(false)
    }

    pub fn default_bindings() -> Self {
        let mut map = InputMap::new();
        map.bind(Action::MenuUp, InputBinding::Key("ArrowUp"));
        map.bind(Action::MenuUp, InputBinding::Key("KeyW"));
        map.bind(Action::MenuDown, InputBinding::Key("ArrowDown"));
        map.bind(Action::MenuDown, InputBinding::Key("KeyS"));
        map.bind(Action::MenuLeft, InputBinding::Key("ArrowLeft"));
        map.bind(Action::MenuLeft, InputBinding::Key("KeyA"));
        map.bind(Action::MenuRight, InputBinding::Key("ArrowRight"));
        map.bind(Action::MenuRight, InputBinding::Key("KeyD"));
        map.bind(Action::Select, InputBinding::Key("Enter"));
        map.bind(Action::Select, InputBinding::Key("Space"));
        map.bind(Action::Select, InputBinding::MouseButton(MouseButton::Left));
        map.bind(Action::Back, InputBinding::Key("Escape"));
        map.bind(Action::NextTab, InputBinding::Key("Tab"));
        map
    }
}

fn binding_pressed(binding: &InputBinding, input: &UiInputState) -> bool {
    match binding {
        InputBinding::Key(key) => input.is_pressed(key),
        InputBinding::MouseButton(MouseButton::Left) => input.lmb_down,
    }
}

fn binding_just_pressed(binding: &InputBinding, input: &UiInputState) -> bool {
    match binding {
        InputBinding::Key(key) => input.is_just_pressed(key),
        InputBinding::MouseButton(MouseButton::Left) => input.lmb_just_pressed,
    }
}
