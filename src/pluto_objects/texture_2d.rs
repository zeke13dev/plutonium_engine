use crate::traits::PlutoObject;
use crate::utils::{Position, Rectangle};
use crate::PlutoniumEngine;
use std::cell::RefCell;
use std::rc::Rc;
use uuid::Uuid;

pub(crate) struct Texture2DInternal {
    id: Uuid,
    texture_key: Uuid,
    dimensions: Rectangle,
    z: i32,
}

impl Texture2DInternal {
    pub fn new(id: Uuid, texture_key: Uuid, dimensions: Rectangle) -> Self {
        Self {
            id,
            texture_key,
            dimensions,
            z: 0,
        }
    }

    pub fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.dimensions = new_dimensions;
    }

    pub fn set_pos(&mut self, new_position: Position) {
        self.dimensions.set_pos(new_position);
    }

    pub fn set_z(&mut self, z: i32) {
        self.z = z;
    }

    pub fn get_z(&self) -> i32 {
        self.z
    }

    pub fn render_with_z(&self, engine: &mut PlutoniumEngine, z: i32) {
        engine.queue_texture_with_layer(&self.texture_key, Some(self.pos()), z);
    }
}

impl PlutoObject for Texture2DInternal {
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

    fn render(&self, engine: &mut PlutoniumEngine) {
        self.render_with_z(engine, self.z);
    }
}

/// Texture2D data.
pub struct Texture2D {
    internal: Rc<RefCell<Texture2DInternal>>,
}

impl Texture2D {
    impl_wrapper_new!(Texture2D, Texture2DInternal);

    /// Sets the dimensions.
    pub fn set_dimensions(&self, new_dimensions: Rectangle) {
        self.internal.borrow_mut().set_dimensions(new_dimensions);
    }

    /// Sets the pos.
    pub fn set_pos(&self, new_position: Position) {
        self.internal.borrow_mut().set_pos(new_position);
    }

    /// Returns the id.
    pub fn get_id(&self) -> Uuid {
        self.internal.borrow().get_id()
    }

    /// Returns the dimensions.
    pub fn get_dimensions(&self) -> Rectangle {
        self.internal.borrow().dimensions()
    }

    /// Returns the pos.
    pub fn get_pos(&self) -> Position {
        self.internal.borrow().pos()
    }

    /// Queues this object for rendering.
    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.internal.borrow().render(engine);
    }

    /// Render with z.
    pub fn render_with_z(&self, engine: &mut PlutoniumEngine, z: i32) {
        self.internal.borrow().render_with_z(engine, z);
    }

    /// Sets the z.
    pub fn set_z(&self, z: i32) {
        self.internal.borrow_mut().set_z(z);
    }

    /// Returns the z.
    pub fn get_z(&self) -> i32 {
        self.internal.borrow().get_z()
    }

    /// Returns this value with z configured.
    pub fn with_z(self, z: i32) -> Self {
        self.set_z(z);
        self
    }
}
