#![allow(dead_code)]

use std::{
    cmp::Ordering,
    collections::VecDeque,
    hash::{Hash, Hasher},
    ops::Add,
    ops::Div,
    ops::DivAssign,
    ops::Mul,
};
use wgpu::util::DeviceExt;

pub struct DrawingContext<'a> {
    pub rpass: &'a mut wgpu::RenderPass<'a>,
    pub pipeline: &'a wgpu::RenderPipeline,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
#[allow(dead_code)]
pub struct UVTransform {
    #[allow(dead_code)]
    pub uv_offset: [f32; 2],
    #[allow(dead_code)]
    pub uv_scale: [f32; 2],
    #[allow(dead_code)]
    pub tint: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
#[allow(dead_code)]
pub struct Vertex {
    #[allow(dead_code)]
    pub position: [f32; 2],
    #[allow(dead_code)]
    pub tex_coords: [f32; 2], // u, v texture coordinates
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[allow(dead_code)]
pub struct TransformUniform {
    #[allow(dead_code)]
    pub transform: [[f32; 4]; 4], // 4x4 transformation matrix
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[allow(dead_code)]
pub struct InstanceRaw {
    #[allow(dead_code)]
    pub model: [[f32; 4]; 4],
    #[allow(dead_code)]
    pub uv_offset: [f32; 2],
    #[allow(dead_code)]
    pub uv_scale: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[allow(dead_code)]
pub struct RectInstanceRaw {
    #[allow(dead_code)]
    pub model: [[f32; 4]; 4],
    #[allow(dead_code)]
    pub color: [f32; 4],
    #[allow(dead_code)]
    pub corner_radius_px: f32,
    #[allow(dead_code)]
    pub border_thickness_px: f32,
    #[allow(dead_code)]
    pub _pad0: [f32; 2],
    #[allow(dead_code)]
    pub border_color: [f32; 4],
    #[allow(dead_code)]
    pub rect_size_px: [f32; 2],
    #[allow(dead_code)]
    pub _pad1: [f32; 2],
    #[allow(dead_code)]
    pub _pad2: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

pub fn create_unit_quad_buffers(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer) {
    // A simple unit quad (0,0) to (1,1) with UVs matching positions
    let vertices: [Vertex; 4] = [
        Vertex {
            position: [0.0, 0.0],
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [1.0, 0.0],
            tex_coords: [1.0, 0.0],
        },
        Vertex {
            position: [1.0, 1.0],
            tex_coords: [1.0, 1.0],
        },
        Vertex {
            position: [0.0, 1.0],
            tex_coords: [0.0, 1.0],
        },
    ];
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("unit-quad-vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("unit-quad-indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    (vbuf, ibuf)
}

pub fn create_centered_quad_buffers(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer) {
    // Quad from (-1,-1) to (1,1) for SDF-based rects; tex_coords unused
    let vertices: [Vertex; 4] = [
        Vertex {
            position: [-1.0, -1.0],
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [1.0, -1.0],
            tex_coords: [1.0, 0.0],
        },
        Vertex {
            position: [1.0, 1.0],
            tex_coords: [1.0, 1.0],
        },
        Vertex {
            position: [-1.0, 1.0],
            tex_coords: [0.0, 1.0],
        },
    ];
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("centered-quad-vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("centered-quad-indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    (vbuf, ibuf)
}
impl Add<f32> for Size {
    type Output = Size;
    fn add(self, rhs: f32) -> Self::Output {
        Size {
            width: self.width + rhs,
            height: self.height + rhs,
        }
    }
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

impl Default for Position {
    fn default() -> Self {
        Position { x: 0.0, y: 0.0 }
    }
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl DivAssign<f32> for Position {
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl Div<f32> for Position {
    type Output = Position;
    fn div(self, factor: f32) -> Self::Output {
        Position {
            x: self.x / factor,
            y: self.y / factor,
        }
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

impl Mul<f32> for Position {
    type Output = Position;
    fn mul(self, factor: f32) -> Self::Output {
        Position {
            x: self.x * factor,
            y: self.y * factor,
        }
    }
}

impl Add<f32> for Position {
    type Output = Position;
    fn add(self, other: f32) -> Self::Output {
        Position {
            x: self.x + other,
            y: self.y + other,
        }
    }
}

impl Add<Position> for Position {
    type Output = Position;
    fn add(self, rhs: Position) -> Self::Output {
        Position {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub<Position> for Position {
    type Output = Position;
    fn sub(self, rhs: Position) -> Self::Output {
        Position {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
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
    pub fn padded_contains(&self, position: Position, padding: f32) -> bool {
        position.x >= self.x - padding
            && position.x <= self.x - padding + self.width - (2.0 * padding)
            && position.y >= self.y - padding
            && position.y <= self.y - padding + self.height - (2.0 * padding)
    }

    pub fn contains(&self, position: Position) -> bool {
        position.x >= self.x
            && position.x <= self.x + self.width
            && position.y >= self.y
            && position.y <= self.y + self.height
    }

    pub fn pos(&self) -> Position {
        Position {
            x: self.x,
            y: self.y,
        }
    }

    pub fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    pub fn set_pos(&mut self, pos: Position) {
        self.x = pos.x;
        self.y = pos.y;
    }

    pub fn pad(rec: &Rectangle, padding: f32) -> Rectangle {
        Rectangle::new(
            rec.x + padding,
            rec.y + padding,
            rec.width + padding,
            rec.height + padding,
        )
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

impl Add<f32> for Rectangle {
    type Output = Rectangle;
    fn add(self, other: f32) -> Self::Output {
        Rectangle::new(self.x, self.y, self.width + other, self.height + other)
    }
}

impl Mul<f32> for Rectangle {
    type Output = Rectangle;
    fn mul(self, factor: f32) -> Self::Output {
        Rectangle::new(self.x, self.y, self.width * factor, self.height * factor)
    }
}

impl Div<f32> for Rectangle {
    type Output = Rectangle;
    fn div(self, factor: f32) -> Self::Output {
        Rectangle::new(self.x, self.y, self.width / factor, self.height / factor)
    }
}

impl PartialEq for Rectangle {
    fn eq(&self, other: &Self) -> bool {
        const EPSILON: f32 = 1e-6;

        (self.x - other.x).abs() < EPSILON
            && (self.y - other.y).abs() < EPSILON
            && (self.width - other.width).abs() < EPSILON
            && (self.height - other.height).abs() < EPSILON
    }
}

#[derive(Copy, Clone, Debug)]
pub struct MouseInfo {
    pub is_rmb_clicked: bool,
    pub is_lmb_clicked: bool,
    pub is_mmb_clicked: bool,
    pub mouse_pos: Position,
}

// Simple sliding-window frame time metrics for real-time reporting
#[derive(Debug)]
pub struct FrameTimeMetrics {
    buffer: VecDeque<f32>, // seconds
    capacity: usize,
    last_report: std::time::Instant,
    report_period_secs: f32,
}

impl FrameTimeMetrics {
    pub fn new(capacity: usize, report_period_secs: f32) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            last_report: std::time::Instant::now(),
            report_period_secs,
        }
    }

    pub fn record(&mut self, delta_seconds: f32) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(delta_seconds);
    }

    fn percentile(sorted_ms: &[f32], p: f32) -> f32 {
        if sorted_ms.is_empty() {
            return 0.0;
        }
        let clamped = p.clamp(0.0, 100.0);
        let rank = (clamped / 100.0) * ((sorted_ms.len() - 1) as f32);
        let lower = rank.floor() as usize;
        let upper = rank.ceil() as usize;
        match lower.cmp(&upper) {
            Ordering::Equal => sorted_ms[lower],
            _ => {
                let w = rank - (lower as f32);
                sorted_ms[lower] * (1.0 - w) + sorted_ms[upper] * w
            }
        }
    }

    pub fn stats(&self) -> Option<(f32, f32, f32, f32)> {
        // returns (p50_ms, p95_ms, p99_ms, avg_fps)
        if self.buffer.is_empty() {
            return None;
        }
        let mut ms: Vec<f32> = self.buffer.iter().map(|s| s * 1000.0).collect();
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let p50 = Self::percentile(&ms, 50.0);
        let p95 = Self::percentile(&ms, 95.0);
        let p99 = Self::percentile(&ms, 99.0);
        let avg_dt = self.buffer.iter().copied().sum::<f32>() / (self.buffer.len() as f32);
        let avg_fps = if avg_dt > 0.0 { 1.0 / avg_dt } else { 0.0 };
        Some((p50, p95, p99, avg_fps))
    }

    pub fn maybe_report(&mut self) -> Option<String> {
        let elapsed = self.last_report.elapsed().as_secs_f32();
        if elapsed < self.report_period_secs {
            return None;
        }
        self.last_report = std::time::Instant::now();
        self.stats().map(|(p50, p95, p99, fps)| {
            format!(
				"frame_metrics p50_ms={:.2} p95_ms={:.2} p99_ms={:.2} avg_fps={:.1} window_frames={}",
				p50, p95, p99, fps, self.buffer.len()
			)
        })
    }
}
