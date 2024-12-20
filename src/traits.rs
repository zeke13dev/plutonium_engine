use crate::texture_svg::TextureSVG;
use crate::utils::{MouseInfo, Position, Rectangle, Size};
use crate::PlutoniumEngine;
use std::collections::HashMap;
use uuid::Uuid;
use winit::keyboard::Key;

pub struct UpdateContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub viewport_size: &'a Size,
    pub camera_position: &'a Position,
}

pub trait PlutoObject {
    // getters
    fn texture_key(&self) -> Uuid;
    fn get_id(&self) -> Uuid;
    fn dimensions(&self) -> Rectangle;
    fn pos(&self) -> Position;

    // setters
    fn set_dimensions(&mut self, new_dimensions: Rectangle);
    fn set_pos(&mut self, new_pos: Position);

    fn update(
        &mut self,
        _mouse_info: Option<MouseInfo>,
        _key_pressed: &Option<Key>,
        _texture_map: &mut HashMap<Uuid, TextureSVG>,
        _update_context: Option<UpdateContext>,
        _dpi_scale_factor: f32,
    ) {
        // do i need to do anything default?
        // engine.update_texture(self.texture_key());
    }

    fn render(&self, engine: &mut PlutoniumEngine) {
        engine.queue_texture(&self.texture_key(), Some(self.pos()));
    }

    fn delete(&self, engine: &mut PlutoniumEngine) {
        engine.remove_object(self.get_id());
    }
}

/*
===== DEFAULT IMPLEMENTATIONS FOR SETTERS AND GETTERS ===== (NEEDS TO BE UPDATED as of 11/24/24)
fn texture_key(&self) -> &str {
    &self.texture_key
}

fn dimensions(&self) -> &Rectangle {
    &self.dimensions
}

fn pos(&self) -> &Position {
    &self.position
}

fn set_dimensions(&mut self, &Rectangle new_dimensions) {
    &self.dimensions = new_dimensions;
}

fn set_pos(&mut self, &Position new_position) {
    &self.position = new_position;
}
*/
