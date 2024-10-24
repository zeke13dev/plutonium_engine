use crate::texture_svg::TextureSVG;
use crate::utils::{MouseInfo, Position, Size};
use crate::PlutoniumEngine;
use std::collections::HashMap;
use winit::keyboard::Key;

pub struct UpdateContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub viewport_size: &'a Size,
    pub camera_position: &'a Position,
}

pub trait PlutoObject {
    fn render(&self, engine: &mut PlutoniumEngine);
    fn update(
        &mut self,
        mouse_pos: Option<MouseInfo>,
        key_pressed: &Option<Key>,
        texture_map: &mut HashMap<String, TextureSVG>,
        update_context: Option<UpdateContext>,
    );
}
