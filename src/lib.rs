pub mod camera;
pub mod pluto_objects {
    #[cfg(feature = "widgets")]
    pub mod button;
    pub mod shapes;
    pub mod text2d;
    #[cfg(feature = "widgets")]
    pub mod text_input;
    pub mod texture_2d;
    pub mod texture_atlas_2d;
}
pub mod app;
pub use app::{FrameContext, PlutoniumApp, WindowConfig};
#[cfg(feature = "anim")]
pub mod anim;
pub mod input;
#[cfg(feature = "layout")]
pub mod layout;
pub mod renderer;
pub mod rng;
pub mod text;
pub mod texture_atlas;
pub mod texture_svg;
pub mod traits;
pub mod ui;
pub mod utils;

use crate::traits::UpdateContext;
use camera::Camera;
#[cfg(feature = "widgets")]
use pluto_objects::button::{Button, ButtonInternal};
#[cfg(feature = "widgets")]
use pluto_objects::text_input::{TextInput, TextInputInternal};
use pluto_objects::{
    shapes::{Shape, ShapeInternal, ShapeType},
    text2d::{Text2D, Text2DInternal, TextContainer},
    texture_2d::{Texture2D, Texture2DInternal},
    texture_atlas_2d::{TextureAtlas2D, TextureAtlas2DInternal},
};
use rusttype::{Font, Scale};

use pollster::block_on;
use renderer::RectCommand;
use std::cell::RefCell;
use std::rc::Rc;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};
use text::*;
use texture_atlas::TextureAtlas;
use texture_svg::*;
use traits::PlutoObject;
use utils::*;
use uuid::Uuid;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::keyboard::Key;

// renderer seam reserved for future use


#[derive(Debug, Clone, Copy, Default)]
pub struct DrawParams {
    pub z: i32,
    pub scale: f32,
    pub rotation: f32,
    pub tint: [f32; 4],
}

pub(crate) enum RenderItem {
    Texture {
        texture_key: Uuid,
        transform_index: usize,
    },
    AtlasTile {
        texture_key: Uuid,
        transform_index: usize,
        tile_index: usize,
    },
    Rect(RectCommand),
}

pub struct QueuedItem {
    z: i32,
    item: RenderItem,
}

struct TransformPool {
    buffers: Vec<wgpu::Buffer>,
    bind_groups: Vec<wgpu::BindGroup>,
    cursor: usize,
    cpu_mats: Vec<[[f32; 4]; 4]>,
}

struct RectInstanceBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    bind_group: wgpu::BindGroup,
    used_this_frame: bool,
    last_used_frame: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RectStyleKey {
    fill_rgba_u8: [u8; 4],
    border_rgba_u8: [u8; 4],
    corner_radius_10x: u16,    // quantized 0.1 px
    border_thickness_10x: u16, // quantized 0.1 px
}

fn to_rgba_u8(c: [f32; 4]) -> [u8; 4] {
    [
        (c[0].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[1].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[2].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[3].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
    ]
}

fn quant_10x(v: f32) -> u16 {
    ((v.max(0.0) * 10.0) + 0.5).floor() as u16
}

impl TransformPool {
    fn new() -> Self {
        Self {
            buffers: Vec::new(),
            bind_groups: Vec::new(),
            cursor: 0,
            cpu_mats: Vec::new(),
        }
    }
    fn reset(&mut self) {
        self.cursor = 0;
        self.cpu_mats.clear();
    }
}

pub struct PlutoniumEngine<'a> {
    pub size: PhysicalSize<u32>,
    dpi_scale_factor: f32,
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    #[allow(dead_code)]
    render_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    rect_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    rect_dummy_bgl: wgpu::BindGroupLayout,
    rect_dummy_bg: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    transform_bind_group_layout: wgpu::BindGroupLayout,
    instance_bind_group_layout: wgpu::BindGroupLayout,
    texture_map: HashMap<Uuid, TextureSVG>,
    atlas_map: HashMap<Uuid, TextureAtlas>,
    pluto_objects: HashMap<Uuid, Rc<RefCell<dyn PlutoObject>>>,
    update_queue: Vec<Uuid>,
    render_queue: Vec<QueuedItem>,
    viewport_size: Size,
    camera: Camera,
    text_renderer: TextRenderer,
    loaded_fonts: HashMap<String, bool>,
    transform_pool: TransformPool,
    // Static geometry for rects
    rect_vertex_buffer: wgpu::Buffer,
    rect_index_buffer: wgpu::Buffer,
    // Per-frame cached identity UBO bind group
    rect_identity_bg: Option<wgpu::BindGroup>,
    // Rect instance buffer pool
    rect_instance_pool: Vec<RectInstanceBuffer>,
    rect_pool_cursor: usize,
    frame_counter: u64,
    // GPU timing instrumentation (optional)
    #[allow(dead_code)]
    timestamp_query: Option<wgpu::QuerySet>,
    #[allow(dead_code)]
    timestamp_buf: Option<wgpu::Buffer>,
    #[allow(dead_code)]
    timestamp_staging: Option<wgpu::Buffer>,
    #[allow(dead_code)]
    timestamp_period_ns: f32,
    #[allow(dead_code)]
    timestamp_count: u32,
    #[allow(dead_code)]
    timestamp_frame_index: u32,
    #[allow(dead_code)]
    gpu_metrics: FrameTimeMetrics,
    // Global UI clip rectangle (logical coords)
    current_scissor: Option<Rectangle>,
    // Nested clip stack (logical coords); top-most is applied. Each push intersects with previous.
    clip_stack: Vec<Rectangle>,
    // Rect batching metrics (style diversity and counts) — preserved-order strategy
    rect_style_keys: HashSet<RectStyleKey>,
    rect_instances_count: usize,
    rect_draw_calls_count: usize,
}

impl<'a> PlutoniumEngine<'a> {
    /* CAMERA STUFF */
    pub fn set_boundary(&mut self, boundary: Rectangle) {
        self.camera.set_boundary(boundary);
    }
    pub fn clear_boundary(&mut self) {
        self.camera.clear_boundary();
    }

    pub fn activate_camera(&mut self) {
        self.camera.activate();
    }

    pub fn deactivate_camera(&mut self) {
        self.camera.deactivate();
    }

    pub fn load_font(
        &mut self,
        font_path: &str,
        logical_font_size: f32,
        font_key: &str,
    ) -> Result<(), FontError> {
        if self.loaded_fonts.contains_key(font_key) {
            return Ok(());
        }

        // Generate the physical-sized atlas for actual texture rendering
        let physical_font_size = logical_font_size * self.dpi_scale_factor;
        let font_data = std::fs::read(font_path).map_err(FontError::IoError)?;
        let font = Font::try_from_vec(font_data.clone()).ok_or(FontError::InvalidFontData)?;
        let scale = Scale::uniform(physical_font_size);
        let padding = 2;

        // Calculate atlas dimensions in physical space
        let (atlas_width, atlas_height, char_dimensions, max_tile_width, max_tile_height) =
            TextRenderer::calculate_atlas_size(&font, scale, padding);

        // Generate the physical texture atlas
        let (texture_data, physical_char_map) = TextRenderer::render_glyphs_to_atlas(
            &font,
            scale,
            (atlas_width, atlas_height),
            &char_dimensions,
            padding,
        )
        .ok_or(FontError::AtlasRenderError)?;

        // Create a new char_map with logical coordinates
        let mut logical_char_map = HashMap::new();
        for (c, physical_info) in physical_char_map {
            logical_char_map.insert(
                c,
                CharacterInfo {
                    tile_index: physical_info.tile_index, // Texture index stays the same
                    advance_width: physical_info.advance_width / self.dpi_scale_factor,
                    bearing: (
                        physical_info.bearing.0 / self.dpi_scale_factor,
                        physical_info.bearing.1 / self.dpi_scale_factor,
                    ),
                    size: physical_info.size, // Keep physical size for texture coordinates
                },
            );
        }

        let atlas_id = Uuid::new_v4();
        // Create texture atlas keeping physical dimensions for the actual texture
        let atlas = self.create_font_texture_atlas(
            atlas_id,
            &texture_data,
            atlas_width,
            atlas_height,
            Size {
                width: max_tile_width as f32,
                height: max_tile_height as f32,
            },
            &logical_char_map, // Use logical char map for consistency
        );

        // Store everything with logical coordinates except the texture dimensions
        let ascent = font.v_metrics(scale).ascent / self.dpi_scale_factor;
        let descent = font.v_metrics(scale).descent / self.dpi_scale_factor;
        // Store font for kerning and future measurement
        // SAFETY: we keep the Vec alive by leaking it into 'static; acceptable for demo/app lifetime
        let leaked: &'static [u8] = Box::leak(font_data.into_boxed_slice());
        let font_static = Font::try_from_bytes(leaked).ok_or(FontError::InvalidFontData)?;
        self.text_renderer
            .fonts
            .insert(font_key.to_string(), font_static);

        self.text_renderer.store_font_atlas(
            font_key,
            atlas,
            logical_char_map,
            logical_font_size, // Store logical size
            ascent,
            descent,
            Size {
                width: max_tile_width as f32, // Keep physical size for texture coordinates
                height: max_tile_height as f32,
            },
            self.dpi_scale_factor,
            padding,
        );

        self.loaded_fonts.insert(font_key.to_string(), true);
        Ok(())
    }
    pub fn set_texture_position(&mut self, key: &Uuid, position: Position) {
        if let Some(texture) = self.texture_map.get_mut(key) {
            texture.set_position(
                &self.device,
                &self.queue,
                position,
                self.viewport_size,
                self.camera.get_pos(self.dpi_scale_factor),
            );
        }
    }

    pub fn resize(&mut self, new_size: &PhysicalSize<u32>) {
        // MAYBE NEEDS TO TAKE INTO ACCOUNT NEW SCALE FACTOR IF RESIZE CHANGES DEVICE
        self.size = *new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.viewport_size = Size {
            width: self.size.width as f32,
            height: self.size.height as f32,
        };
    }

    pub fn update(&mut self, mouse_info: Option<MouseInfo>, key: &Option<Key>) {
        // text doesn't seem to be getting updated
        let scaled_mouse_info = mouse_info.map(|info| MouseInfo {
            is_rmb_clicked: info.is_rmb_clicked,
            is_lmb_clicked: info.is_lmb_clicked,
            is_mmb_clicked: info.is_mmb_clicked,
            mouse_pos: info.mouse_pos / self.dpi_scale_factor,
        });

        for id in &self.update_queue {
            if let Some(obj) = self.pluto_objects.get(id) {
                obj.borrow_mut().update(
                    scaled_mouse_info,
                    key,
                    &mut self.texture_map,
                    Some(UpdateContext {
                        device: &self.device,
                        queue: &self.queue,
                        viewport_size: &self.viewport_size,
                        camera_position: &self.camera.get_pos(self.dpi_scale_factor),
                    }),
                    self.dpi_scale_factor,
                    &self.text_renderer,
                );
            }
        }

        // Handle camera tethering with DPI scaling
        let (camera_position, tether_size) = if let Some(tether_target) = &self.camera.tether_target
        {
            if let Some(tether) = self.pluto_objects.get(tether_target) {
                let tether_ref = tether.borrow();
                let tether_dimensions = tether_ref.dimensions();
                (tether_dimensions.pos(), Some(tether_dimensions.size()))
            } else {
                (self.camera.get_pos(self.dpi_scale_factor), None)
            }
        } else {
            (self.camera.get_pos(self.dpi_scale_factor), None)
        };

        self.camera.set_pos(camera_position);
        self.camera.set_tether_size(tether_size);

        // update actual location of where object buffers are
        for texture in self.texture_map.values_mut() {
            texture.update_transform_uniform(
                &self.device,
                &self.queue,
                self.viewport_size,
                self.camera.get_pos(self.dpi_scale_factor),
            );
        }
        for atlas in self.atlas_map.values_mut() {
            atlas.update_transform_uniform(
                &self.device,
                &self.queue,
                self.viewport_size,
                self.camera.get_pos(self.dpi_scale_factor),
            );
        }
    }

    pub fn set_camera_target(&mut self, texture_key: Uuid) {
        self.camera.tether_target = Some(texture_key);
    }

    pub fn queue_texture(&mut self, texture_key: &Uuid, position: Option<Position>) {
        self.queue_texture_with_layer(texture_key, position, 0);
    }

    pub fn queue_texture_with_layer(
        &mut self,
        texture_key: &Uuid,
        position: Option<Position>,
        z: i32,
    ) {
        if let Some(texture) = self.texture_map.get(texture_key) {
            // Generate the transformation matrix based on the position and camera
            let position = position.unwrap_or_default() * self.dpi_scale_factor;
            let transform_uniform = texture.get_transform_uniform(
                self.viewport_size,
                position,
                self.camera.get_pos(self.dpi_scale_factor),
                0.0,
            );
            let transform_index = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z,
                item: RenderItem::Texture {
                    texture_key: *texture_key,
                    transform_index,
                },
            });
        }
    }

    pub fn queue_tile(
        &mut self,
        texture_key: &Uuid,
        tile_index: usize,
        position: Position,
        user_scale: f32,
    ) {
        self.queue_tile_with_layer(texture_key, tile_index, position, user_scale, 0);
    }

    pub fn queue_tile_with_layer(
        &mut self,
        texture_key: &Uuid,
        tile_index: usize,
        position: Position,
        user_scale: f32,
        z: i32,
    ) {
        if let Some(atlas) = self.atlas_map.get(texture_key) {
            let transform_uniform = atlas.get_transform_uniform(
                self.viewport_size,
                position,
                self.camera.get_pos(self.dpi_scale_factor),
                self.dpi_scale_factor, // position scale (DPI)
                user_scale,            // tile size scale (already physical px)
            );
            let transform_index = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z,
                item: RenderItem::AtlasTile {
                    texture_key: *texture_key,
                    transform_index,
                    tile_index,
                },
            });
        }
    }

    pub fn queue_text(
        &mut self,
        text: &str,
        font_key: &str,
        position: Position,
        container: &TextContainer,
    ) {
        self.queue_text_with_spacing(text, font_key, position, container, 0.0, 0.0);
    }

    pub fn queue_text_with_spacing(
        &mut self,
        text: &str,
        font_key: &str,
        position: Position,
        container: &TextContainer,
        letter_spacing: f32,
        word_spacing: f32,
    ) {
        let chars = self.text_renderer.calculate_text_layout(
            text,
            font_key,
            position,
            container,
            letter_spacing,
            word_spacing,
        );
        for char in chars {
            // Draw full tile size; glyph alpha handles actual shape
            self.queue_tile_with_layer(&char.atlas_id, char.tile_index, char.position, 1.0, 0);
        }
    }
    pub fn clear_render_queue(&mut self) {
        self.render_queue.clear();
    }

    pub fn unload_texture(&mut self, texture_key: &Uuid) -> bool {
        self.texture_map.remove(texture_key).is_some()
    }

    pub fn unload_atlas(&mut self, atlas_key: &Uuid) -> bool {
        self.atlas_map.remove(atlas_key).is_some()
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                match e {
                    wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                        // Reconfigure the surface and skip this frame
                        self.surface.configure(&self.device, &self.config);
                        return Ok(());
                    }
                    wgpu::SurfaceError::OutOfMemory => {
                        return Err(e);
                    }
                    wgpu::SurfaceError::Timeout => {
                        // Skip this frame and try again on the next one
                        return Ok(());
                    }
                }
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        // GPU timestamp begin
        let (qset, qbuf, qcount) = (
            self.timestamp_query.as_ref(),
            self.timestamp_buf.as_ref(),
            self.timestamp_count,
        );
        let qindex = if qcount >= 2 {
            self.timestamp_frame_index % (qcount / 2)
        } else {
            0
        };
        let q0 = qindex * 2;
        let q1 = q0 + 1;
        // We'll write timestamps via render pass timestamp_writes when supported.

        {
            // sort by z, stable to preserve submission order within same layer
            self.render_queue.sort_by(|a, b| a.z.cmp(&b.z));

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: qset.map(|qs| wgpu::RenderPassTimestampWrites {
                    query_set: qs,
                    beginning_of_pass_write_index: Some(q0),
                    end_of_pass_write_index: Some(q1),
                }),
                occlusion_query_set: None,
            });

            // Apply scissor rect if set
            // Determine effective scissor: prefer top of stack; if none, fall back to current_scissor
            let effective = self.clip_stack.last().copied().or(self.current_scissor);
            if let Some(sc) = effective {
                // Convert logical to physical, clamp to surface bounds
                let x = (sc.x * self.dpi_scale_factor).max(0.0).floor() as u32;
                let y = (sc.y * self.dpi_scale_factor).max(0.0).floor() as u32;
                let w = ((sc.width * self.dpi_scale_factor).max(0.0).floor() as u32)
                    .min(self.config.width.saturating_sub(x));
                let h = ((sc.height * self.dpi_scale_factor).max(0.0).floor() as u32)
                    .min(self.config.height.saturating_sub(y));
                rpass.set_scissor_rect(x, y, w, h);
            }

            // Set default pipeline for texture/atlas draws; rect draws will override temporarily
            rpass.set_pipeline(&self.render_pipeline);

            // Streaming batcher that preserves z-order and interleaves atlas draws
            let mut current_tex: Option<Uuid> = None;
            let mut batch_indices: Vec<usize> = Vec::new();
            let mut current_atlas: Option<Uuid> = None;
            let mut atlas_instances: Vec<crate::utils::InstanceRaw> = Vec::new();
            // Rect batching
            let mut rect_instances: Vec<crate::utils::RectInstanceRaw> = Vec::new();
            let mut rect_draw_calls: usize = 0;

            // Helper to flush a pending sprite batch
            let flush_batch = |rpass: &mut wgpu::RenderPass<'_>,
                               tex_id: Option<Uuid>,
                               indices: &mut Vec<usize>| {
                if indices.is_empty() {
                    return;
                }
                if let Some(tid) = tex_id {
                    if let Some(texture) = self.texture_map.get(&tid) {
                        // Build per-instance data: model + uv (full sprite)
                        let instances: Vec<crate::utils::InstanceRaw> = indices
                            .iter()
                            .map(|i| crate::utils::InstanceRaw {
                                model: self.transform_pool.cpu_mats[*i],
                                uv_offset: [0.0, 0.0],
                                uv_scale: [1.0, 1.0],
                            })
                            .collect();
                        let instance_buffer =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("instance data (sprite)"),
                                    contents: bytemuck::cast_slice(&instances),
                                    usage: wgpu::BufferUsages::STORAGE,
                                });
                        let instance_bg =
                            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                layout: &self.instance_bind_group_layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: instance_buffer.as_entire_binding(),
                                }],
                                label: Some("instance_bind_group"),
                            });

                        // Bind texture, identity world, uv and instance buffer
                        rpass.set_bind_group(0, texture.bind_group(), &[]);
                        rpass.set_bind_group(3, &instance_bg, &[]);

                        let identity = TransformUniform {
                            transform: [
                                [1.0, 0.0, 0.0, 0.0],
                                [0.0, 1.0, 0.0, 0.0],
                                [0.0, 0.0, 1.0, 0.0],
                                [0.0, 0.0, 0.0, 1.0],
                            ],
                        };
                        let id_buf =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("id-ubo"),
                                    contents: bytemuck::bytes_of(&identity),
                                    usage: wgpu::BufferUsages::UNIFORM,
                                });
                        let id_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &self.transform_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: id_buf.as_entire_binding(),
                            }],
                            label: Some("id-bg"),
                        });
                        rpass.set_bind_group(1, &id_bg, &[]);
                        rpass.set_bind_group(2, texture.uv_bind_group(), &[]);
                        rpass.set_vertex_buffer(0, texture.vertex_buffer_slice());
                        rpass.set_index_buffer(
                            texture.index_buffer_slice(),
                            wgpu::IndexFormat::Uint16,
                        );
                        rpass.draw_indexed(0..texture.num_indices(), 0, 0..(indices.len() as u32));
                    }
                }
                indices.clear();
            };

            // Helper to flush a pending rect batch
            let mut flush_rects =
                |rpass: &mut wgpu::RenderPass<'_>,
                 instances: &mut Vec<crate::utils::RectInstanceRaw>| {
                    if instances.is_empty() {
                        return;
                    }
                    let bytes_needed = (instances.len()
                        * std::mem::size_of::<crate::utils::RectInstanceRaw>())
                        as u64;
                    // Find a pool entry with sufficient capacity not used yet; else allocate
                    let mut chosen: Option<usize> = None;
                    for (i, entry) in self.rect_instance_pool.iter().enumerate() {
                        if !entry.used_this_frame && entry.capacity >= bytes_needed {
                            chosen = Some(i);
                            break;
                        }
                    }
                    let idx = if let Some(i) = chosen {
                        i
                    } else {
                        let cap = bytes_needed.next_power_of_two().max(256);
                        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("rect-instance-buffer"),
                            size: cap,
                            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        let bind_group =
                            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("rect-instance-bg"),
                                layout: &self.instance_bind_group_layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: buffer.as_entire_binding(),
                                }],
                            });
                        self.rect_instance_pool.push(RectInstanceBuffer {
                            buffer,
                            capacity: cap,
                            bind_group,
                            used_this_frame: false,
                            last_used_frame: self.frame_counter,
                        });
                        self.rect_instance_pool.len() - 1
                    };
                    // Grow if needed, write, and mark used
                    {
                        let entry = &mut self.rect_instance_pool[idx];
                        if entry.capacity < bytes_needed {
                            let cap = bytes_needed.next_power_of_two();
                            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("rect-instance-buffer"),
                                size: cap,
                                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            });
                            entry.buffer = buffer;
                            entry.capacity = cap;
                            entry.bind_group =
                                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                    label: Some("rect-instance-bg"),
                                    layout: &self.instance_bind_group_layout,
                                    entries: &[wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: entry.buffer.as_entire_binding(),
                                    }],
                                });
                        }
                        self.queue
                            .write_buffer(&entry.buffer, 0, bytemuck::cast_slice(instances));
                        entry.used_this_frame = true;
                        entry.last_used_frame = self.frame_counter;
                    }

                    // Cache identity transform BG for rects per frame
                    if self.rect_identity_bg.is_none() {
                        let identity = TransformUniform {
                            transform: [
                                [1.0, 0.0, 0.0, 0.0],
                                [0.0, 1.0, 0.0, 0.0],
                                [0.0, 0.0, 1.0, 0.0],
                                [0.0, 0.0, 0.0, 1.0],
                            ],
                        };
                        let id_buf =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("rect-id-ubo"),
                                    contents: bytemuck::bytes_of(&identity),
                                    usage: wgpu::BufferUsages::UNIFORM,
                                });
                        let id_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &self.transform_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: id_buf.as_entire_binding(),
                            }],
                            label: Some("rect-id-bg"),
                        });
                        self.rect_identity_bg = Some(id_bg);
                    }

                    rpass.set_pipeline(&self.rect_pipeline);
                    // Bind dummy groups for slots 0 and 2 to satisfy layout
                    rpass.set_bind_group(0, &self.rect_dummy_bg, &[]);
                    rpass.set_bind_group(1, self.rect_identity_bg.as_ref().unwrap(), &[]);
                    rpass.set_bind_group(2, &self.rect_dummy_bg, &[]);
                    rpass.set_bind_group(3, &self.rect_instance_pool[idx].bind_group, &[]);
                    rpass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
                    rpass.set_index_buffer(
                        self.rect_index_buffer.slice(..),
                        wgpu::IndexFormat::Uint16,
                    );
                    rpass.draw_indexed(0..6, 0, 0..(instances.len() as u32));
                    rect_draw_calls += 1;
                    instances.clear();
                };

            for q in &self.render_queue {
                match &q.item {
                    RenderItem::Texture {
                        texture_key,
                        transform_index,
                    } => {
                        // Switching away from rects; flush any pending rect batch
                        flush_rects(&mut rpass, &mut rect_instances);
                        match current_tex {
                            Some(tid) if tid == *texture_key => {
                                batch_indices.push(*transform_index);
                            }
                            _ => {
                                // different texture; flush previous
                                flush_batch(&mut rpass, current_tex, &mut batch_indices);
                                // also flush any atlas batch
                                if !atlas_instances.is_empty() {
                                    let _ = ();
                                }
                                // flush atlas using helper
                                // (inline since closures can't borrow self twice safely here)
                                if !atlas_instances.is_empty() {
                                    if let Some(aid) = current_atlas {
                                        if let Some(atlas) = self.atlas_map.get(&aid) {
                                            let instance_buffer = self.device.create_buffer_init(
                                                &wgpu::util::BufferInitDescriptor {
                                                    label: Some("instance data (atlas)"),
                                                    contents: bytemuck::cast_slice(
                                                        &atlas_instances,
                                                    ),
                                                    usage: wgpu::BufferUsages::STORAGE,
                                                },
                                            );
                                            let instance_bg = self.device.create_bind_group(
                                                &wgpu::BindGroupDescriptor {
                                                    layout: &self.instance_bind_group_layout,
                                                    entries: &[wgpu::BindGroupEntry {
                                                        binding: 0,
                                                        resource: instance_buffer
                                                            .as_entire_binding(),
                                                    }],
                                                    label: Some("atlas-instance-bg"),
                                                },
                                            );
                                            let identity = TransformUniform {
                                                transform: [
                                                    [1.0, 0.0, 0.0, 0.0],
                                                    [0.0, 1.0, 0.0, 0.0],
                                                    [0.0, 0.0, 1.0, 0.0],
                                                    [0.0, 0.0, 0.0, 1.0],
                                                ],
                                            };
                                            let id_buf = self.device.create_buffer_init(
                                                &wgpu::util::BufferInitDescriptor {
                                                    label: Some("id-ubo"),
                                                    contents: bytemuck::bytes_of(&identity),
                                                    usage: wgpu::BufferUsages::UNIFORM,
                                                },
                                            );
                                            let id_bg = self.device.create_bind_group(
                                                &wgpu::BindGroupDescriptor {
                                                    layout: &self.transform_bind_group_layout,
                                                    entries: &[wgpu::BindGroupEntry {
                                                        binding: 0,
                                                        resource: id_buf.as_entire_binding(),
                                                    }],
                                                    label: Some("id-bg"),
                                                },
                                            );
                                            rpass.set_bind_group(0, &atlas.bind_group, &[]);
                                            rpass.set_bind_group(1, &id_bg, &[]);
                                            rpass.set_bind_group(
                                                2,
                                                atlas.default_uv_bind_group(),
                                                &[],
                                            );
                                            rpass.set_bind_group(3, &instance_bg, &[]);
                                            rpass.set_vertex_buffer(
                                                0,
                                                atlas.vertex_buffer.slice(..),
                                            );
                                            rpass.set_index_buffer(
                                                atlas.index_buffer.slice(..),
                                                wgpu::IndexFormat::Uint16,
                                            );
                                            rpass.draw_indexed(
                                                0..atlas.num_indices,
                                                0,
                                                0..(atlas_instances.len() as u32),
                                            );
                                        }
                                    }
                                    atlas_instances.clear();
                                    current_atlas = None;
                                }
                                current_tex = Some(*texture_key);
                                batch_indices.push(*transform_index);
                            }
                        }
                    }
                    RenderItem::AtlasTile {
                        texture_key,
                        transform_index,
                        tile_index,
                    } => {
                        // Switching away from rects; flush any pending rect batch
                        flush_rects(&mut rpass, &mut rect_instances);
                        // flush any sprite batch first
                        flush_batch(&mut rpass, current_tex, &mut batch_indices);
                        current_tex = None;
                        // switch atlas batch if needed
                        if current_atlas != Some(*texture_key) {
                            // flush previous atlas
                            if !atlas_instances.is_empty() {
                                if let Some(aid) = current_atlas {
                                    if let Some(atlas) = self.atlas_map.get(&aid) {
                                        let instance_buffer = self.device.create_buffer_init(
                                            &wgpu::util::BufferInitDescriptor {
                                                label: Some("instance data (atlas)"),
                                                contents: bytemuck::cast_slice(&atlas_instances),
                                                usage: wgpu::BufferUsages::STORAGE,
                                            },
                                        );
                                        let instance_bg = self.device.create_bind_group(
                                            &wgpu::BindGroupDescriptor {
                                                layout: &self.instance_bind_group_layout,
                                                entries: &[wgpu::BindGroupEntry {
                                                    binding: 0,
                                                    resource: instance_buffer.as_entire_binding(),
                                                }],
                                                label: Some("atlas-instance-bg"),
                                            },
                                        );
                                        let identity = TransformUniform {
                                            transform: [
                                                [1.0, 0.0, 0.0, 0.0],
                                                [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0],
                                                [0.0, 0.0, 0.0, 1.0],
                                            ],
                                        };
                                        let id_buf = self.device.create_buffer_init(
                                            &wgpu::util::BufferInitDescriptor {
                                                label: Some("id-ubo"),
                                                contents: bytemuck::bytes_of(&identity),
                                                usage: wgpu::BufferUsages::UNIFORM,
                                            },
                                        );
                                        let id_bg = self.device.create_bind_group(
                                            &wgpu::BindGroupDescriptor {
                                                layout: &self.transform_bind_group_layout,
                                                entries: &[wgpu::BindGroupEntry {
                                                    binding: 0,
                                                    resource: id_buf.as_entire_binding(),
                                                }],
                                                label: Some("id-bg"),
                                            },
                                        );
                                        rpass.set_bind_group(0, &atlas.bind_group, &[]);
                                        rpass.set_bind_group(1, &id_bg, &[]);
                                        rpass.set_bind_group(2, atlas.default_uv_bind_group(), &[]);
                                        rpass.set_bind_group(3, &instance_bg, &[]);
                                        rpass.set_vertex_buffer(0, atlas.vertex_buffer.slice(..));
                                        rpass.set_index_buffer(
                                            atlas.index_buffer.slice(..),
                                            wgpu::IndexFormat::Uint16,
                                        );
                                        rpass.draw_indexed(
                                            0..atlas.num_indices,
                                            0,
                                            0..(atlas_instances.len() as u32),
                                        );
                                    }
                                }
                                atlas_instances.clear();
                            }
                            current_atlas = Some(*texture_key);
                        }
                        if let Some(atlas) = self.atlas_map.get(texture_key) {
                            let model = self.transform_pool.cpu_mats[*transform_index];
                            if let Some(uv_rect) =
                                crate::texture_atlas::TextureAtlas::tile_uv_coordinates(
                                    *tile_index,
                                    atlas.tile_size,
                                    atlas.dimensions.size(),
                                )
                            {
                                atlas_instances.push(crate::utils::InstanceRaw {
                                    model,
                                    uv_offset: [uv_rect.x, uv_rect.y],
                                    uv_scale: [uv_rect.width, uv_rect.height],
                                });
                            }
                        }
                    }
                    RenderItem::Rect(cmd) => {
                        // Flush any pending sprite/atlas batches before enqueueing rects
                        flush_batch(&mut rpass, current_tex, &mut batch_indices);
                        current_tex = None;
                        if !atlas_instances.is_empty() {
                            if let Some(aid) = current_atlas {
                                if let Some(atlas) = self.atlas_map.get(&aid) {
                                    let instance_buffer = self.device.create_buffer_init(
                                        &wgpu::util::BufferInitDescriptor {
                                            label: Some("instance data (atlas)"),
                                            contents: bytemuck::cast_slice(&atlas_instances),
                                            usage: wgpu::BufferUsages::STORAGE,
                                        },
                                    );
                                    let instance_bg =
                                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                            layout: &self.instance_bind_group_layout,
                                            entries: &[wgpu::BindGroupEntry {
                                                binding: 0,
                                                resource: instance_buffer.as_entire_binding(),
                                            }],
                                            label: Some("atlas-instance-bg"),
                                        });
                                    let identity = TransformUniform {
                                        transform: [
                                            [1.0, 0.0, 0.0, 0.0],
                                            [0.0, 1.0, 0.0, 0.0],
                                            [0.0, 0.0, 1.0, 0.0],
                                            [0.0, 0.0, 0.0, 1.0],
                                        ],
                                    };
                                    let id_buf = self.device.create_buffer_init(
                                        &wgpu::util::BufferInitDescriptor {
                                            label: Some("id-ubo"),
                                            contents: bytemuck::bytes_of(&identity),
                                            usage: wgpu::BufferUsages::UNIFORM,
                                        },
                                    );
                                    let id_bg =
                                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                            layout: &self.transform_bind_group_layout,
                                            entries: &[wgpu::BindGroupEntry {
                                                binding: 0,
                                                resource: id_buf.as_entire_binding(),
                                            }],
                                            label: Some("id-bg"),
                                        });
                                    rpass.set_bind_group(0, &atlas.bind_group, &[]);
                                    rpass.set_bind_group(1, &id_bg, &[]);
                                    rpass.set_bind_group(2, atlas.default_uv_bind_group(), &[]);
                                    rpass.set_bind_group(3, &instance_bg, &[]);
                                    rpass.set_vertex_buffer(0, atlas.vertex_buffer.slice(..));
                                    rpass.set_index_buffer(
                                        atlas.index_buffer.slice(..),
                                        wgpu::IndexFormat::Uint16,
                                    );
                                    rpass.draw_indexed(
                                        0..atlas.num_indices,
                                        0,
                                        0..(atlas_instances.len() as u32),
                                    );
                                }
                            }
                            atlas_instances.clear();
                            current_atlas = None;
                        }

                        // Enqueue rect instance for batching
                        rect_instances.push(crate::utils::RectInstanceRaw {
                            model: cmd.transform,
                            color: cmd.color,
                            corner_radius_px: cmd.corner_radius_px,
                            border_thickness_px: cmd.border_thickness_px,
                            _pad0: [0.0, 0.0],
                            border_color: cmd.border_color,
                            rect_size_px: [cmd.width_px, cmd.height_px],
                            _pad1: [0.0, 0.0],
                            _pad2: [0.0, 0.0, 0.0, 0.0],
                        });
                        // Metrics: track style diversity and counts (no reordering/grouping)
                        self.rect_instances_count = self.rect_instances_count.saturating_add(1);
                        let key = RectStyleKey {
                            fill_rgba_u8: to_rgba_u8(cmd.color),
                            border_rgba_u8: to_rgba_u8(cmd.border_color),
                            corner_radius_10x: quant_10x(cmd.corner_radius_px),
                            border_thickness_10x: quant_10x(cmd.border_thickness_px),
                        };
                        self.rect_style_keys.insert(key);
                    }
                }
            }
            // flush any remaining sprite batch
            flush_batch(&mut rpass, current_tex, &mut batch_indices);
            // flush any remaining atlas batch
            if !atlas_instances.is_empty() {
                if let Some(aid) = current_atlas {
                    if self.atlas_map.contains_key(&aid) {
                        let instance_buffer =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("instance data (atlas)"),
                                    contents: bytemuck::cast_slice(&atlas_instances),
                                    usage: wgpu::BufferUsages::STORAGE,
                                });
                        let instance_bg =
                            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                layout: &self.instance_bind_group_layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: instance_buffer.as_entire_binding(),
                                }],
                                label: Some("atlas-instance-bg"),
                            });
                        let identity = TransformUniform {
                            transform: [
                                [1.0, 0.0, 0.0, 0.0],
                                [0.0, 1.0, 0.0, 0.0],
                                [0.0, 0.0, 1.0, 0.0],
                                [0.0, 0.0, 0.0, 1.0],
                            ],
                        };
                        let id_buf =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("id-ubo"),
                                    contents: bytemuck::bytes_of(&identity),
                                    usage: wgpu::BufferUsages::UNIFORM,
                                });
                        let id_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &self.transform_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: id_buf.as_entire_binding(),
                            }],
                            label: Some("id-bg"),
                        });
                        if let Some(atlas) = self.atlas_map.get(&aid) {
                            rpass.set_bind_group(0, &atlas.bind_group, &[]);
                            rpass.set_bind_group(1, &id_bg, &[]);
                            rpass.set_bind_group(2, atlas.default_uv_bind_group(), &[]);
                            rpass.set_bind_group(3, &instance_bg, &[]);
                            rpass.set_vertex_buffer(0, atlas.vertex_buffer.slice(..));
                            rpass.set_index_buffer(
                                atlas.index_buffer.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            rpass.draw_indexed(
                                0..atlas.num_indices,
                                0,
                                0..(atlas_instances.len() as u32),
                            );
                        }
                    }
                }
            }
            // flush any remaining rects
            flush_rects(&mut rpass, &mut rect_instances);
            self.rect_draw_calls_count = rect_draw_calls;
        }
        // End timestamp + resolve
        if let (Some(qs), Some(buf)) = (qset, qbuf) {
            let base = (((q0 as u64) * 8) / wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
                * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            encoder.resolve_query_set(qs, q0..(q1 + 1), buf, base);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        // Read back timestamps (synchronously for simplicity)
        if let (Some(src), Some(dst)) = (&self.timestamp_buf, &self.timestamp_staging) {
            // Copy resolved results into MAP_READ staging
            let mut enc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("copy ts"),
                });
            let base = (((q0 as u64) * 8) / wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
                * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            enc.copy_buffer_to_buffer(
                src,
                base,
                dst,
                base,
                wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT,
            );
            self.queue.submit(Some(enc.finish()));
            let start = base;
            let end = start + 16;
            let slice = dst.slice(start..end);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res.is_ok());
            });
            // Block until mapping completes
            self.device.poll(wgpu::Maintain::Wait);
            if rx.recv().unwrap_or(false) {
                let data = slice.get_mapped_range();
                if data.len() >= 16 {
                    let t0 = u64::from_le_bytes(data[0..8].try_into().unwrap());
                    let t1 = u64::from_le_bytes(data[8..16].try_into().unwrap());
                    if t1 > t0 {
                        let dt_ns = (t1 - t0) as f64 * (self.timestamp_period_ns as f64);
                        let dt_s = (dt_ns / 1_000_000_000.0) as f32;
                        self.gpu_metrics.record(dt_s);
                        if let Some(line) = self.gpu_metrics.maybe_report() {
                            println!("gpu_{}", line);
                        }
                    }
                }
                drop(data);
                dst.unmap();
            }
        }
        self.timestamp_frame_index = self.timestamp_frame_index.wrapping_add(1);
        Ok(())
    }

    // Frame helpers for an immediate-mode style
    pub fn begin_frame(&mut self) {
        self.clear_render_queue();
        self.transform_pool.reset();
        self.rect_identity_bg = None;
        self.rect_pool_cursor = 0;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        for entry in &mut self.rect_instance_pool {
            entry.used_this_frame = false;
        }
        self.current_scissor = None;
        self.clip_stack.clear();
        self.rect_style_keys.clear();
        self.rect_instances_count = 0;
        self.rect_draw_calls_count = 0;
    }

    pub fn end_frame(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Periodically evict least recently used rect instance buffers to cap memory
        const MAX_POOL: usize = 32;
        const EVICT_AGE: u64 = 600; // frames
        if self.rect_instance_pool.len() > MAX_POOL {
            // Retain entries that are either recently used or needed
            self.rect_instance_pool
                .retain(|e| self.frame_counter.saturating_sub(e.last_used_frame) < EVICT_AGE);
            if self.rect_instance_pool.len() > MAX_POOL {
                // Sort by last_used_frame ascending and truncate
                self.rect_instance_pool.sort_by_key(|e| e.last_used_frame);
                self.rect_instance_pool.truncate(MAX_POOL);
            }
        }
        self.render()
    }

    // Convenience immediate-mode draws for consistent naming
    pub fn draw_texture(&mut self, texture_key: &Uuid, position: Position, params: DrawParams) {
        // rotation only supported by direct draw path for sprites; augment transform
        if let Some(texture) = self.texture_map.get(texture_key) {
            let transform_uniform = texture.get_transform_uniform(
                self.viewport_size,
                position * self.dpi_scale_factor,
                self.camera.get_pos(self.dpi_scale_factor),
                params.rotation,
            );
            let idx = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z: params.z,
                item: RenderItem::Texture {
                    texture_key: *texture_key,
                    transform_index: idx,
                },
            });
        }
    }

    pub fn draw_tile(
        &mut self,
        atlas_key: &Uuid,
        tile_index: usize,
        position: Position,
        params: DrawParams,
    ) {
        let user_scale = if params.scale == 0.0 {
            1.0
        } else {
            params.scale
        };
        self.queue_tile_with_layer(atlas_key, tile_index, position, user_scale, params.z);
    }

    // Draw an atlas tile stretched to an arbitrary destination rectangle (non-uniform scale)
    pub fn draw_atlas_tile_stretched(
        &mut self,
        atlas_key: &Uuid,
        tile_index: usize,
        dst: Rectangle,
        z: i32,
    ) {
        if !self.atlas_map.contains_key(atlas_key) {
            return;
        }
        // Convert logical to physical pixels via DPI scale
        let cam = self.camera.get_pos(self.dpi_scale_factor);
        let left_px = ((dst.x * self.dpi_scale_factor) - cam.x).round();
        let top_px = ((dst.y * self.dpi_scale_factor) - cam.y).round();
        let right_px = (((dst.x + dst.width) * self.dpi_scale_factor) - cam.x).round();
        let bottom_px = (((dst.y + dst.height) * self.dpi_scale_factor) - cam.y).round();
        let px_w = (right_px - left_px).max(0.0);
        let px_h = (bottom_px - top_px).max(0.0);
        // NDC scale
        let width_ndc = 2.0 * (px_w / self.viewport_size.width);
        let height_ndc = 2.0 * (px_h / self.viewport_size.height);
        // Center translation from snapped edges
        let ndc_left = 2.0 * (left_px / self.viewport_size.width) - 1.0;
        let ndc_top = -2.0 * (top_px / self.viewport_size.height) + 1.0;
        let ndc_x = ndc_left + width_ndc * 0.5;
        let ndc_y = ndc_top - height_ndc * 0.5;
        let transform_uniform = TransformUniform {
            transform: [
                [width_ndc, 0.0, 0.0, 0.0],
                [0.0, height_ndc, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ndc_x, ndc_y, 0.0, 1.0],
            ],
        };
        let transform_index = self.allocate_transform_bind_group(transform_uniform);
        self.render_queue.push(QueuedItem {
            z,
            item: RenderItem::AtlasTile {
                texture_key: *atlas_key,
                transform_index,
                tile_index,
            },
        });
    }

    // Immediate-mode rect draw (UI primitive)
    pub fn draw_rect(
        &mut self,
        bounds: Rectangle,
        color: [f32; 4],
        corner_radius_px: f32,
        border: Option<([f32; 4], f32)>,
        z: i32,
    ) {
        // Convert to a model matrix similar to textures, accounting for dpi and camera
        let pos = Position {
            x: bounds.x,
            y: bounds.y,
        } * self.dpi_scale_factor;
        let size = bounds.size() * self.dpi_scale_factor;
        let width_ndc = size.width / self.viewport_size.width;
        let height_ndc = size.height / self.viewport_size.height;
        let ndc_dx = (2.0 * (pos.x - self.camera.get_pos(self.dpi_scale_factor).x))
            / self.viewport_size.width
            - 1.0;
        let ndc_dy = 1.0
            - (2.0 * (pos.y - self.camera.get_pos(self.dpi_scale_factor).y))
                / self.viewport_size.height;
        let ndc_x = ndc_dx + width_ndc;
        let ndc_y = ndc_dy - height_ndc;
        let model = [
            [width_ndc, 0.0, 0.0, 0.0],
            [0.0, height_ndc, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [ndc_x, ndc_y, 0.0, 1.0],
        ];
        let (border_color, border_thickness) = border.unwrap_or(([0.0, 0.0, 0.0, 0.0], 0.0));
        let cmd = RectCommand {
            width_px: size.width,
            height_px: size.height,
            color,
            corner_radius_px,
            border_thickness_px: border_thickness,
            border_color,
            transform: model,
            z,
        };
        self.render_queue.push(QueuedItem {
            z,
            item: RenderItem::Rect(cmd),
        });
    }

    fn allocate_transform_bind_group(&mut self, transform_uniform: TransformUniform) -> usize {
        // Reuse existing entry if available, else create new
        if self.transform_pool.cursor < self.transform_pool.buffers.len() {
            let idx = self.transform_pool.cursor;
            self.queue.write_buffer(
                &self.transform_pool.buffers[idx],
                0,
                bytemuck::bytes_of(&transform_uniform),
            );
            self.transform_pool.cursor += 1;
            self.transform_pool
                .cpu_mats
                .push(transform_uniform.transform);
            idx
        } else {
            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Transform UBO (pooled)"),
                    contents: bytemuck::bytes_of(&transform_uniform),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.transform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
                label: Some("Transform BG (pooled)"),
            });
            self.transform_pool.buffers.push(buffer);
            self.transform_pool.bind_groups.push(bind_group);
            let idx = self.transform_pool.cursor;
            self.transform_pool.cursor += 1;
            self.transform_pool
                .cpu_mats
                .push(transform_uniform.transform);
            idx
        }
    }

    pub fn create_texture_svg(
        &mut self,
        file_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> (Uuid, Rectangle) {
        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new(
            texture_key,
            &self.device,
            &self.queue,
            file_path,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            scale_factor * self.dpi_scale_factor,
        );

        let texture = svg_texture.expect("texture should always be created properly");
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        (texture_key, dimensions)
    }

    pub fn create_texture_svg_from_data(
        &mut self,
        svg_data: &str,
        position: Position,
        scale_factor: f32,
    ) -> (Uuid, Rectangle) {
        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new_from_data(
            texture_key,
            &self.device,
            &self.queue,
            svg_data,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            scale_factor * self.dpi_scale_factor,
        );

        let texture = svg_texture.expect("texture should always be created properly");
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        (texture_key, dimensions)
    }

    #[cfg(feature = "raster")]
    pub fn create_texture_raster_from_path(
        &mut self,
        path: &str,
        position: Position,
    ) -> (Uuid, Rectangle) {
        let img = image::open(path).expect("failed to open image").to_rgba8();
        let (width, height) = img.dimensions();
        let rgba = img.as_raw();

        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new_from_rgba(
            texture_key,
            &self.device,
            &self.queue,
            width,
            height,
            rgba,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
        );

        let texture = svg_texture.expect("texture should always be created properly");
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        (texture_key, dimensions)
    }

    pub fn create_texture_atlas(
        &mut self,
        svg_path: &str,
        position: Position,
        tile_size: Size,
    ) -> (Uuid, Rectangle) {
        let texture_key = Uuid::new_v4();

        // Update to match new TextureAtlas interface
        if let Some(atlas) = TextureAtlas::new(
            texture_key,
            &self.device,
            &self.queue,
            svg_path,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            tile_size,
        ) {
            let dimensions = atlas.dimensions();

            let positioned_dimensions =
                Rectangle::new(position.x, position.y, dimensions.width, dimensions.height);

            self.atlas_map.insert(texture_key, atlas);
            (texture_key, positioned_dimensions)
        } else {
            panic!("Failed to create texture atlas")
        }
    }

    pub fn create_font_texture_atlas(
        &mut self,
        atlas_id: Uuid,
        texture_data: &[u8],
        width: u32,
        height: u32,
        tile_size: Size,
        char_positions: &HashMap<char, CharacterInfo>,
    ) -> TextureAtlas2D {
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Font Atlas Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            texture_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        // Create texture view and sampler
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create the texture bind group
        let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("Font Atlas Bind Group"),
        });

        // Create TextureAtlas and add it to the atlas_map
        if let Some(atlas) = TextureAtlas::new_from_texture(
            atlas_id,
            texture,
            texture_bind_group,
            Position { x: 0.0, y: 0.0 },
            Size::new(width as f32, height as f32),
            tile_size,
            &self.device,
            &self.queue,
            &self.transform_bind_group_layout,
            char_positions,
        ) {
            atlas
                .save_debug_png(&self.device, &self.queue, "debug_atlas.png")
                .unwrap();
            // Add to atlas_map
            self.atlas_map.insert(atlas_id, atlas);

            // Create the internal representation
            let internal = TextureAtlas2DInternal::new(
                atlas_id,
                atlas_id,
                1.0,
                Rectangle::new(0.0, 0.0, width as f32, height as f32),
                tile_size,
            );
            let rc_internal = Rc::new(RefCell::new(internal));

            self.pluto_objects.insert(atlas_id, rc_internal.clone());
            self.update_queue.push(atlas_id);

            TextureAtlas2D::new(rc_internal)
        } else {
            panic!("Failed to create font texture atlas");
        }
    }
    pub fn remove_object(&mut self, id: Uuid) {
        self.pluto_objects.remove(&id);
    }

    /* OBJECT CREATION FUNCTIONS */
    pub fn create_texture_2d(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> Texture2D {
        let id = Uuid::new_v4();

        // Create the underlying texture
        let (texture_key, dimensions) = self.create_texture_svg(svg_path, position, scale_factor);

        // Create the internal representation
        let internal = Texture2DInternal::new(id, texture_key, dimensions);
        let rc_internal = Rc::new(RefCell::new(internal));

        // Add to pluto objects and update queue
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        // Return the wrapper
        Texture2D::new(rc_internal)
    }
    pub fn create_text2d(
        &mut self,
        text: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
    ) -> Text2D {
        let id = Uuid::new_v4();
        // Ensure font is loaded, now with proper error handling
        if !self.loaded_fonts.contains_key(font_key) {
            panic!("Failed to load font");
        }

        // Create text dimensions based on measurement - now needs font_key
        let text_size = self.text_renderer.measure_text(text, font_key);
        let dimensions = Rectangle::new(
            position.x,
            position.y,
            text_size.0,
            text_size.1 as f32 * font_size,
        );

        let internal = Text2DInternal::new(
            id,
            font_key.to_string(), // Changed from font_path to font_key
            dimensions,
            font_size,
            text,
            None,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Text2D::new(rc_internal)
    }

    pub fn create_texture_atlas_2d(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
        tile_size: Size,
    ) -> TextureAtlas2D {
        let id = Uuid::new_v4();

        // Create texture atlas instead of regular texture
        let (texture_key, dimensions) = self.create_texture_atlas(svg_path, position, tile_size);

        // Create the internal representation
        let internal =
            TextureAtlas2DInternal::new(id, texture_key, scale_factor, dimensions, tile_size);
        let rc_internal = Rc::new(RefCell::new(internal));

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        TextureAtlas2D::new(rc_internal)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_button(
        &mut self,
        svg_path: &str,
        text: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> Button {
        let id = Uuid::new_v4();

        // Create button texture
        let (button_texture_key, button_dimensions) =
            self.create_texture_svg(svg_path, position, scale_factor);

        // Create text object
        let text_position = Position {
            x: button_dimensions.x + (button_dimensions.width * 0.1),
            y: button_dimensions.y + (button_dimensions.height / 2.0),
        };
        let text_object = self.create_text2d(text, font_key, font_size, text_position);

        text_object.set_pos(Position { x: 0.0, y: 0.0 });
        // Create internal representation
        let internal = ButtonInternal::new(id, button_texture_key, button_dimensions, text_object);

        // Wrap in Rc<RefCell> and store
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        // Return the wrapper
        Button::new(rc_internal)
    }

    pub fn create_text_input(
        &mut self,
        svg_path: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> TextInput {
        let input_id = Uuid::new_v4();

        // Create button
        let button = self.create_button(svg_path, "", font_key, font_size, position, scale_factor);

        // Create text object
        let text_position = Position {
            x: button.get_dimensions().x + (button.get_dimensions().width * 0.01),
            y: button.get_dimensions().y + (button.get_dimensions().height * 0.05),
        };
        let text = self.create_text2d("", font_key, font_size, text_position);

        // Create cursor using Texture2D with embedded SVG data
        let cursor_height = font_size * 1.05 / self.dpi_scale_factor;
        let cursor_width = scale_factor / self.dpi_scale_factor;
        let cursor_svg_data = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">
        <rect width=\"{}\" height=\"{}\" fill=\"#000\">
            <animate 
                attributeName=\"opacity\"
                values=\"1;0;1\" 
                dur=\"1s\"
                repeatCount=\"indefinite\"/>
        </rect>
    </svg>",
            cursor_width, cursor_height, cursor_width, cursor_height
        );

        let cursor_position = Position {
            x: text_position.x,
            y: button.get_dimensions().y + (button.get_dimensions().height * 0.1),
        };

        let cursor_id = Uuid::new_v4();
        let (texture_key, dimensions) =
            self.create_texture_svg_from_data(&cursor_svg_data, cursor_position, scale_factor);

        // Create the internal representation for cursor
        let cursor_internal = Texture2DInternal::new(cursor_id, texture_key, dimensions);
        let rc_cursor_internal = Rc::new(RefCell::new(cursor_internal));

        // Add cursor to pluto objects and update queue
        self.pluto_objects
            .insert(cursor_id, rc_cursor_internal.clone());
        self.update_queue.push(cursor_id);

        let cursor = Texture2D::new(rc_cursor_internal);

        // Create internal representation for text input
        let dimensions = button.get_dimensions();
        let internal = TextInputInternal::new(input_id, button, text, cursor, dimensions);

        // Wrap in Rc<RefCell> and store
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(input_id, rc_internal.clone());
        self.update_queue.push(input_id);

        // Return the wrapper
        TextInput::new(rc_internal)
    }

    pub fn create_rect(
        &mut self,
        bounds: Rectangle,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Shape {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Rectangle,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        // Create the texture using svg data directly
        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0);

        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Shape::new(rc_internal)
    }

    pub fn create_circle(
        &mut self,
        radius: f32,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Shape {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();
        let bounds = Rectangle::new(0.0, 0.0, radius * 2.0, radius * 2.0);

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Circle,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0);
        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Shape::new(rc_internal)
    }

    pub fn create_polygon(
        &mut self,
        radius: f32,
        points: u32,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Shape {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();
        let bounds = Rectangle::new(0.0, 0.0, radius * 2.0, radius * 2.0);

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Polygon(points),
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0);
        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Shape::new(rc_internal)
    }

    pub fn new(
        surface: wgpu::Surface<'a>,
        instance: wgpu::Instance,
        size: PhysicalSize<u32>,
        dpi_scale_factor: f32,
    ) -> Self {
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        }))
        .expect("Failed to find an appropriate adapter");

        // create the logical device and command queue
        let mut required_features = wgpu::Features::empty();
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            required_features |= wgpu::Features::TIMESTAMP_QUERY;
        }
        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                memory_hints: Default::default(),
            },
            None,
        ))
        .expect("Failed to create device"); // Handle the Result

        let config = wgpu::SurfaceConfiguration {
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("transform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX, // Transformation matrix is used in the vertex shader
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<TransformUniform>() as _,
                        ),
                    },
                    count: None,
                }],
            });

        let uv_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uv_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT, // UV offsets and scales are used in the fragment shader
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        // The size must match the UVUniform structure defined in the shader
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<UVTransform>() as _
                        ),
                    },
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT, // Texture is used in the fragment shader
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT, // Sampler is used in the fragment shader
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // shader and related devices
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/shader.wgsl"))),
        });

        // Now update the pipeline layout to include all four bind group layouts

        // Persistent instance bind group layout (group 3)
        let instance_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("instance_bind_group_layout"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Texture Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &transform_bind_group_layout,
                &uv_bind_group_layout,
                &instance_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // set up render pipeline (textured quads)

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    // Use standard non-premultiplied alpha blending so glyph quads don't appear as solid white
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Rect pipeline (SDF rects with optional border)
        // Create an empty bind group layout for slots we don't use (group 0 and 2)
        let rect_dummy_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rect-dummy-bgl"),
            entries: &[],
        });
        let rect_dummy_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect-dummy-bg"),
            layout: &rect_dummy_bgl,
            entries: &[],
        });
        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/rect.wgsl"))),
        });
        let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Rect Pipeline Layout"),
            bind_group_layouts: &[
                // group(0) unused (no texture) — use empty layout
                &rect_dummy_bgl,
                &transform_bind_group_layout,
                // group(2) unused — use empty layout
                &rect_dummy_bgl,
                &instance_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect-pipeline"),
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &rect_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Static centered quad for rects
        let (rect_vertex_buffer, rect_index_buffer) = create_centered_quad_buffers(&device);

        let texture_map: HashMap<Uuid, TextureSVG> = HashMap::new();
        let atlas_map: HashMap<Uuid, TextureAtlas> = HashMap::new();
        let pluto_objects = HashMap::new();
        let viewport_size = Size {
            width: config.width as f32,
            height: config.height as f32,
        };
        let render_queue = Vec::new();
        let update_queue = Vec::new();
        let camera = Camera::new(Position { x: 0.0, y: 0.0 });

        let text_renderer = TextRenderer::new();
        let loaded_fonts = HashMap::new();
        let transform_pool = TransformPool::new();

        // Optional GPU timestamp query setup
        let mut timestamp_query: Option<wgpu::QuerySet> = None;
        let mut timestamp_buf: Option<wgpu::Buffer> = None;
        let mut timestamp_count: u32 = 0;
        let timestamp_period_ns: f32 = queue.get_timestamp_period();
        let mut timestamp_staging_buf: Option<wgpu::Buffer> = None;
        if device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            // 2 queries per frame across a small ring buffer
            timestamp_count = 128;
            timestamp_query = Some(device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("gpu-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: timestamp_count,
            }));
            timestamp_buf = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu-timestamps-buffer"),
                size: (timestamp_count as u64) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            // Staging buffer for CPU readback
            let staging = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu-timestamps-staging"),
                size: (timestamp_count as u64) * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            timestamp_staging_buf = Some(staging);
        }

        Self {
            size,
            surface,
            device,
            dpi_scale_factor,
            queue,
            config,
            render_pipeline,
            texture_bind_group_layout,
            transform_bind_group_layout,
            texture_map,
            atlas_map,
            pluto_objects,
            render_queue,
            update_queue,
            viewport_size,
            camera,
            text_renderer,
            loaded_fonts,
            transform_pool,
            rect_pipeline,
            rect_dummy_bgl,
            rect_dummy_bg,
            rect_vertex_buffer,
            rect_index_buffer,
            rect_identity_bg: None,
            rect_instance_pool: Vec::new(),
            rect_pool_cursor: 0,
            frame_counter: 0,
            instance_bind_group_layout,
            timestamp_query,
            timestamp_buf,
            timestamp_staging: timestamp_staging_buf,
            timestamp_period_ns,
            timestamp_count,
            timestamp_frame_index: 0,
            gpu_metrics: FrameTimeMetrics::new(600, 5.0),
            current_scissor: None,
            clip_stack: Vec::new(),
            rect_style_keys: HashSet::new(),
            rect_instances_count: 0,
            rect_draw_calls_count: 0,
        }
    }

    // UI clipping (logical coordinates); applies a scissor rect for the render pass of this frame
    pub fn set_clip(&mut self, rect: Rectangle) {
        self.current_scissor = Some(rect);
    }
    pub fn clear_clip(&mut self) {
        self.current_scissor = None;
    }

    // Push a clip rectangle (intersect with prior top if present)
    pub fn push_clip(&mut self, rect: Rectangle) {
        if let Some(&prev) = self.clip_stack.last() {
            let x1 = prev.x.max(rect.x);
            let y1 = prev.y.max(rect.y);
            let x2 = (prev.x + prev.width).min(rect.x + rect.width);
            let y2 = (prev.y + prev.height).min(rect.y + rect.height);
            let w = (x2 - x1).max(0.0);
            let h = (y2 - y1).max(0.0);
            self.clip_stack.push(Rectangle::new(x1, y1, w, h));
        } else {
            self.clip_stack.push(rect);
        }
    }
    pub fn pop_clip(&mut self) {
        let _ = self.clip_stack.pop();
    }
}
