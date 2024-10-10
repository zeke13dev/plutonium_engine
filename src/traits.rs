use crate::texture_svg::TextureSVG;
use crate::utils::Position;
use crate::PlutoniumEngine;
use winit::keyboard::Key;

pub trait PlutoObject {
    fn render(&self, engine: &mut PlutoniumEngine);
    fn update(
        &mut self,
        texture: &TextureSVG,
        mouse_pos: Option<Position>,
        key_pressed: Option<Key>,
    );
}
