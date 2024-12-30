use crate::{Position, Rectangle, Size};
use uuid::Uuid;

#[derive(Debug)]
pub struct Camera {
    position: Position,
    boundary: Option<Rectangle>,
    activated: bool,
    pub tether_target: Option<Uuid>,
    tether_size: Option<Size>,
}

impl Camera {
    pub fn set_boundary(&mut self, boundary: Rectangle) {
        self.boundary = Some(boundary);
    }

    pub fn clear_boundary(&mut self) {
        self.boundary = None;
    }

    pub fn activate(&mut self) {
        self.activated = true;
    }

    pub fn deactivate(&mut self) {
        self.activated = false;
    }

pub fn get_pos(&self, scale_factor: f32) -> Position {
    if self.activated {
        Position {
            x: self.position.x * scale_factor,
            y: self.position.y * scale_factor,
        }
    } else {
        Position { x: 0.0, y: 0.0 }
    }
}
    pub fn set_pos(&mut self, new_pos: Position) {
        if let Some(boundary) = &self.boundary {
            // Calculate the logical boundary taking into account both camera position and tether size
            let logical_boundary = if let Some(tether_size) = self.tether_size {
                Rectangle::new(
                    boundary.x + self.position.x,
                    boundary.y + self.position.y,
                    boundary.width - tether_size.width,
                    boundary.height - tether_size.height,
                )
            } else {
                Rectangle::new(
                    boundary.x + self.position.x,
                    boundary.y + self.position.y,
                    boundary.width,
                    boundary.height,
                )
            };

            // Handle the x-direction
            let dx = {
                let right_overflow = new_pos.x - (logical_boundary.x + logical_boundary.width);
                let left_overflow = new_pos.x - logical_boundary.x;
                if right_overflow > 0.0 {
                    right_overflow
                } else if left_overflow < 0.0 {
                    left_overflow
                } else {
                    0.0
                }
            };
            self.position.x += dx;

            // Handle the y-direction
            let dy = {
                let bottom_overflow = new_pos.y - (logical_boundary.y + logical_boundary.height);
                let top_overflow = new_pos.y - logical_boundary.y;
                if bottom_overflow > 0.0 {
                    bottom_overflow
                } else if top_overflow < 0.0 {
                    top_overflow
                } else {
                    0.0
                }
            };
            self.position.y += dy;
        } else {
            // If no boundary is set, simply update the position
            self.position = new_pos;
        }
    }
    pub fn new(position: Position) -> Self {
        Self {
            position,
            tether_target: None,
            activated: false,
            boundary: None,
            tether_size: None,
        }
    }

    pub fn set_tether_target(&mut self, target: Option<Uuid>) {
        self.tether_target = target;
    }

    pub fn set_tether_size(&mut self, size: Option<Size>) {
        self.tether_size = size;
    }
}
