use crate::traits::PlutoObject;
use crate::utils::{Position, Rectangle, Size};
use crate::PlutoniumEngine;
use std::cell::RefCell;
use std::rc::Rc;
use uuid::Uuid;

// Internal Representation
pub struct TextureAtlas2DInternal {
    id: Uuid,
    texture_key: Uuid,
    scale_factor: f32,
    dimensions: Rectangle,
    tile_size: Size,
    z: i32,
}

impl TextureAtlas2DInternal {
    pub fn new(
        id: Uuid,
        texture_key: Uuid,
        scale_factor: f32,
        dimensions: Rectangle,
        tile_size: Size,
    ) -> Self {
        Self {
            id,
            texture_key,
            scale_factor,
            dimensions,
            tile_size,
            z: 0,
        }
    }

    pub fn set_scale_factor(&mut self, factor: f32) {
        self.scale_factor = factor;
    }
    pub fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.dimensions = new_dimensions;
    }

    pub fn set_pos(&mut self, new_position: Position) {
        self.dimensions.set_pos(new_position);
    }

    pub fn render_tile(&self, engine: &mut PlutoniumEngine, tile_index: usize, position: Position) {
        self.render_tile_with_z(engine, tile_index, position, self.z);
    }

    pub fn render_tile_with_z(
        &self,
        engine: &mut PlutoniumEngine,
        tile_index: usize,
        position: Position,
        z: i32,
    ) {
        engine.queue_tile_with_layer(
            &self.texture_key,
            tile_index,
            position,
            self.scale_factor,
            z,
        );
    }

    pub fn set_z(&mut self, z: i32) {
        self.z = z;
    }

    pub fn get_z(&self) -> i32 {
        self.z
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }
}

impl PlutoObject for TextureAtlas2DInternal {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn texture_key(&self) -> Uuid {
        self.texture_key
    }

    fn dimensions(&self) -> Rectangle {
        self.dimensions
    }

    fn pos(&self) -> Position {
        self.dimensions.pos()
    }

    fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.set_dimensions(new_dimensions);
    }

    fn set_pos(&mut self, new_position: Position) {
        self.set_pos(new_position);
    }
}

// Wrapper Representation
pub struct TextureAtlas2D {
    internal: Rc<RefCell<TextureAtlas2DInternal>>,
}

impl TextureAtlas2D {
    pub fn new(internal: Rc<RefCell<TextureAtlas2DInternal>>) -> Self {
        Self { internal }
    }

    pub fn set_dimensions(&self, new_dimensions: Rectangle) {
        self.internal.borrow_mut().set_dimensions(new_dimensions);
    }

    pub fn set_pos(&self, new_position: Position) {
        self.internal.borrow_mut().set_pos(new_position);
    }

    pub fn render_tile(&self, engine: &mut PlutoniumEngine, tile_index: usize, position: Position) {
        self.internal
            .borrow()
            .render_tile(engine, tile_index, position);
    }

    pub fn render_tile_with_z(
        &self,
        engine: &mut PlutoniumEngine,
        tile_index: usize,
        position: Position,
        z: i32,
    ) {
        self.internal
            .borrow()
            .render_tile_with_z(engine, tile_index, position, z);
    }

    pub fn set_z(&self, z: i32) {
        self.internal.borrow_mut().set_z(z);
    }

    pub fn get_z(&self) -> i32 {
        self.internal.borrow().get_z()
    }

    pub fn with_z(self, z: i32) -> Self {
        self.set_z(z);
        self
    }

    pub fn get_id(&self) -> Uuid {
        self.internal.borrow().get_id()
    }

    pub fn get_dimensions(&self) -> Rectangle {
        self.internal.borrow().dimensions()
    }

    pub fn get_pos(&self) -> Position {
        self.internal.borrow().pos()
    }

    pub fn get_tile_size(&self) -> Size {
        self.internal.borrow().tile_size
    }

    pub fn set_scale_factor(&mut self, factor: f32) {
        self.internal.borrow_mut().set_scale_factor(factor);
    }
}
