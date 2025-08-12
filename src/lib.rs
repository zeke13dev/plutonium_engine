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
#[cfg(feature = "layout")]
pub mod layout;
pub mod renderer;
pub mod text;
pub mod texture_atlas;
pub mod texture_svg;
pub mod traits;
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
use std::cell::RefCell;
use std::rc::Rc;
use std::{borrow::Cow, collections::HashMap};
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

#[cfg(feature = "raster")]
use image;

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
    // reserved for future instancing bind group
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
        let font = Font::try_from_vec(font_data).ok_or(FontError::InvalidFontData)?;
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
                self.dpi_scale_factor * user_scale,
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
        let chars = self
            .text_renderer
            .calculate_text_layout(text, font_key, position, container);
        for char in chars {
            self.queue_tile_with_layer(&char.atlas_id, char.tile_index, char.position, 1.0, 0);
        }
    }
    pub fn clear_render_queue(&mut self) {
        self.render_queue.clear();
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
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Streaming batcher that preserves z-order and interleaves atlas draws
            let mut current_tex: Option<Uuid> = None;
            let mut batch_indices: Vec<usize> = Vec::new();
            let mut current_atlas: Option<Uuid> = None;
            let mut atlas_instances: Vec<crate::utils::InstanceRaw> = Vec::new();

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

            for q in &self.render_queue {
                match &q.item {
                    RenderItem::Texture {
                        texture_key,
                        transform_index,
                    } => {
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
                }
            }
            // flush any remaining sprite batch
            flush_batch(&mut rpass, current_tex, &mut batch_indices);
            // flush any remaining atlas batch
            if !atlas_instances.is_empty() {
                if let Some(aid) = current_atlas {
                    if let Some(atlas) = self.atlas_map.get(&aid) {
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
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }

    // Frame helpers for an immediate-mode style
    pub fn begin_frame(&mut self) {
        self.clear_render_queue();
        self.transform_pool.reset();
    }

    pub fn end_frame(&mut self) -> Result<(), wgpu::SurfaceError> {
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
        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
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

        // set up render pipeline

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
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
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
            instance_bind_group_layout,
        }
    }
}
