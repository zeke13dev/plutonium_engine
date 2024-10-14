use crate::utils::MouseInfo;
use crate::PlutoniumEngine;
use winit::keyboard::Key;

pub trait PlutoObject {
    fn render(&self, engine: &mut PlutoniumEngine);
    fn update(&mut self, mouse_pos: Option<MouseInfo>, key_pressed: &Option<Key>);
}
