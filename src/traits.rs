use crate::texture_svg::TextureSVG;
use crate::utils::{MouseInfo, Position};
use crate::PlutoniumEngine;
use winit::keyboard::Key;

pub trait PlutoObject {
    fn render(&self, engine: &mut PlutoniumEngine);
    fn update(
        &mut self,
        texture: &TextureSVG,
        mouse_pos: Option<MouseInfo>,
        key_pressed: &Option<Key>,
    );
}
