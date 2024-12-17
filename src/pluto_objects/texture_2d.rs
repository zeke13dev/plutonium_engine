use crate::traits::PlutoObject;
use crate::utils::{Position, Rectangle};
use crate::PlutoniumEngine;
use std::cell::RefCell;
use std::rc::Rc;
use uuid::Uuid;

pub struct Texture2DInternal {
    id: Uuid,
    texture_key: Uuid,
    dimensions: Rectangle,
}

impl Texture2DInternal {
    pub fn new(id: Uuid, texture_key: Uuid, dimensions: Rectangle) -> Self {
        Self {
            id,
            texture_key,
            dimensions,
        }
    }

    pub fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.dimensions = new_dimensions;
    }

    pub fn set_pos(&mut self, new_position: Position) {
        self.dimensions.set_pos(new_position);
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
}

pub struct Texture2D {
    internal: Rc<RefCell<Texture2DInternal>>,
}

impl Texture2D {
    pub fn new(internal: Rc<RefCell<Texture2DInternal>>) -> Self {
        Self { internal }
    }

    pub fn set_dimensions(&self, new_dimensions: Rectangle) {
        self.internal.borrow_mut().set_dimensions(new_dimensions);
    }

    pub fn set_pos(&self, new_position: Position) {
        self.internal.borrow_mut().set_pos(new_position);
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

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.internal.borrow().render(engine);
    }
}
