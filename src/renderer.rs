use std::collections::HashMap;
use wgpu::util::DeviceExt;

use uuid::Uuid;

use crate::texture_atlas::TextureAtlas;
use crate::texture_svg::TextureSVG;
use crate::utils::{RectInstanceRaw, Size};
// use crate::PlutoniumEngine; // not needed right now

// Expose the queued items to the renderer backend
// Reserved for future use

pub trait Renderer {
    fn submit<'a>(
        &mut self,
        _rpass: &mut wgpu::RenderPass<'a>,
        _pipeline: &'a wgpu::RenderPipeline,
        _items: &'a [crate::QueuedItem],
        _texture_map: &'a HashMap<Uuid, TextureSVG>,
        _atlas_map: &'a HashMap<Uuid, TextureAtlas>,
    );
}

pub struct WgpuRenderer;

impl WgpuRenderer {
    pub fn new() -> Self {
        Self
    }
}

// UI Rect primitives API (to be used by engine front-end)
#[derive(Debug, Clone, Copy)]
pub struct RectCommand {
    pub width_px: f32,
    pub height_px: f32,
    pub color: [f32; 4],
    pub corner_radius_px: f32,
    pub border_thickness_px: f32,
    pub border_color: [f32; 4],
    pub transform: [[f32; 4]; 4],
    pub z: i32,
}

pub struct RectBatch {
    pub commands: Vec<RectCommand>,
}

impl RectBatch {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
    pub fn clear(&mut self) {
        self.commands.clear();
    }
    pub fn push(&mut self, cmd: RectCommand) {
        self.commands.push(cmd);
    }
    pub fn draw(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'_>,
        rect_pipeline: &wgpu::RenderPipeline,
        transform_bg_layout: &wgpu::BindGroupLayout,
        _viewport_size: Size,
    ) {
        if self.commands.is_empty() {
            return;
        }

        // Build instance buffer
        let instances: Vec<RectInstanceRaw> = self
            .commands
            .iter()
            .map(|c| RectInstanceRaw {
                model: c.transform,
                color: c.color,
                corner_radius_px: c.corner_radius_px,
                border_thickness_px: c.border_thickness_px,
                _pad0: [0.0, 0.0],
                border_color: c.border_color,
                rect_size_px: [c.width_px, c.height_px],
                _pad1: [0.0, 0.0],
                _pad2: [0.0, 0.0, 0.0, 0.0],
            })
            .collect();

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rect-instance-buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let instance_bg_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("rect-instance-bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let instance_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect-instance-bg"),
            layout: &instance_bg_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: instance_buffer.as_entire_binding(),
            }],
        });

        // Create a trivial transform (identity). Engine should set group(1) separately when integrating
        let transform_uniform = crate::utils::TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let transform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rect-transform-ubo"),
            contents: bytemuck::bytes_of(&transform_uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let transform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect-transform-bg"),
            layout: transform_bg_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_buf.as_entire_binding(),
            }],
        });

        rpass.set_pipeline(rect_pipeline);
        rpass.set_bind_group(1, &transform_bg, &[]);
        rpass.set_bind_group(3, &instance_bg, &[]);

        // Fullscreen-ish centered quad in NDC is owned by engine. We just rely on vertex buffer 0 already set
        rpass.draw_indexed(0..6, 0, 0..(self.commands.len() as u32));
    }
}

impl Renderer for WgpuRenderer {
    fn submit<'a>(
        &mut self,
        _rpass: &mut wgpu::RenderPass<'a>,
        _pipeline: &'a wgpu::RenderPipeline,
        _items: &'a [crate::QueuedItem],
        _texture_map: &'a HashMap<Uuid, TextureSVG>,
        _atlas_map: &'a HashMap<Uuid, TextureAtlas>,
    ) {
        // Engine currently issues the actual bind groups during submission;
        // this backend seam is reserved for future full refactor.
        // No-op here to keep behavior unchanged while indices are introduced in the queue.
    }
}
