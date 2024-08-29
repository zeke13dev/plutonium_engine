use std::{
    hash::{Hash, Hasher},
    ops::Mul,
};

pub struct DrawingContext<'a> {
    pub rpass: &'a mut wgpu::RenderPass<'a>,
    pub pipeline: &'a wgpu::RenderPipeline,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct UVTransform {
    pub uv_offset: [f32; 2],
    pub uv_scale: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct Vertex {
    pub position: [f32; 3],   // x, y, z coordinates
    pub tex_coords: [f32; 2], // u, v texture coordinates
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TransformUniform {
    pub transform: [[f32; 4]; 4], // 4x4 transformation matrix
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Mul<f32> for Size {
    type Output = Size;

    fn mul(self, rhs: f32) -> Self::Output {
        Size {
            width: self.width * rhs,
            height: self.height * rhs,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl Eq for Position {}

impl Hash for Position {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Convert the floating-point numbers to a fixed precision before hashing
        // This example rounds the numbers to a precision of two decimal places
        let precision = 100.0; // Adjust the precision as needed
        let x = (self.x * precision).round() as i32;
        let y = (self.y * precision).round() as i32;

        x.hash(state);
        y.hash(state);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    pub fn contains(&self, position: Position) -> bool {
        position.x >= self.x
            && position.x <= self.x + self.width
            && position.y >= self.y
            && position.y <= self.y + self.height
    }

    pub fn pos(&self) -> Position {
        return Position {
            x: self.x,
            y: self.y,
        };
    }

    pub fn size(&self) -> Size {
        return Size {
            width: self.width,
            height: self.height,
        };
    }

    pub fn set_pos(&mut self, pos: Position) {
        self.x = pos.x;
        self.y = pos.y;
    }

    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn new_square(x: f32, y: f32, side_length: f32) -> Self {
        Self {
            x,
            y,
            width: side_length,
            height: side_length,
        }
    }
}
