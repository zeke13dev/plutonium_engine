use crate::text::TextRenderer;
use crate::texture_svg::TextureSVG;
use crate::utils::{MouseInfo, Position, Rectangle, Size};
use crate::PlutoniumEngine;
use std::collections::HashMap;
use uuid::Uuid;
use winit::keyboard::Key;
/// GPU and camera state supplied to object update hooks.
///
/// This context intentionally exposes `wgpu` device and queue references because
/// custom objects that allocate or update GPU resources must use the same backend
/// instances owned by the engine.
pub struct UpdateContext<'a> {
    /// Engine-owned `wgpu` device.
    pub device: &'a wgpu::Device,
    /// Engine-owned `wgpu` queue.
    pub queue: &'a wgpu::Queue,
    /// Current logical viewport size.
    pub viewport_size: &'a Size,
    /// Current logical camera position.
    pub camera_position: &'a Position,
    /// Monotonic font-cache version for invalidating text-dependent caches.
    pub font_cache_version: u32,
}

/// Behavior required by pluto object implementations.
pub trait PlutoObject {
    // getters
    /// Item.
    fn texture_key(&self) -> Uuid;
    /// Item.
    fn get_id(&self) -> Uuid;
    /// Item.
    fn dimensions(&self) -> Rectangle;
    /// Item.
    fn pos(&self) -> Position;

    // setters
    /// Item.
    fn set_dimensions(&mut self, new_dimensions: Rectangle);
    /// Item.
    fn set_pos(&mut self, new_pos: Position);

    /// Updates object state for the current frame.
    ///
    /// The key argument intentionally uses `winit::keyboard::Key` because object
    /// updates consume raw logical keys from the engine's event loop.
    fn update(
        &mut self,
        _mouse_info: Option<MouseInfo>,
        _key_pressed: &Option<Key>,
        _texture_map: &mut HashMap<Uuid, TextureSVG>,
        _update_context: Option<UpdateContext>,
        _dpi_scale_factor: f32,
        _text_renderer: &TextRenderer,
    ) {
        // do i need to do anything default?
        // engine.update_texture(self.texture_key());
    }

    /// Item.
    fn render(&self, engine: &mut PlutoniumEngine) {
        engine.queue_texture(&self.texture_key(), Some(self.pos()));
    }

    /// Item.
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
