use crate::{Position, Rectangle, Size};

#[derive(Debug)]
pub struct Camera {
    position: Position,
    boundary: Option<Rectangle>,
    activated: bool,
    pub tether_target: Option<String>,
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

    pub fn get_pos(&self) -> Position {
        if self.activated {
            self.position
        } else {
            Position { x: 0.0, y: 0.0 }
        }
    }

    pub fn set_pos(&mut self, new_pos: Position) {
        if let Some(boundary) = &self.boundary {
            // Calculate the logical boundary taking into account the camera's position
            let logical_boundary: Rectangle = Rectangle::new(
                boundary.x + self.position.x,
                boundary.y + self.position.y,
                boundary.width,
                boundary.height,
            );

            // Handle the x-direction
            let dx_right = if let Some(tether_size) = self.tether_size {
                new_pos.x + tether_size.width - (logical_boundary.x + logical_boundary.width)
            } else {
                new_pos.x - (logical_boundary.x + logical_boundary.width)
            };
            if dx_right > 0.0 {
                self.position.x += dx_right;
            }

            let dx_left = new_pos.x - logical_boundary.x;
            if dx_left < 0.0 {
                self.position.x += dx_left;
            }

            // Handle the y-direction
            let dy_bottom = if let Some(tether_size) = self.tether_size {
                new_pos.y + tether_size.height - (logical_boundary.y + logical_boundary.height)
            } else {
                new_pos.y - (logical_boundary.y + logical_boundary.height)
            };
            if dy_bottom > 0.0 {
                self.position.y += dy_bottom;
            }

            let dy_top = new_pos.y - logical_boundary.y;
            if dy_top < 0.0 {
                self.position.y += dy_top;
            }
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

    pub fn set_tether_target(&mut self, target: Option<String>) {
        self.tether_target = target;
    }

    pub fn set_tether_size(&mut self, size: Option<Size>) {
        self.tether_size = size;
    }
}
