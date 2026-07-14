use crate::utils::TransformUniform;
use crate::{Rectangle, RenderItem};
use uuid::Uuid;
use wgpu::util::DeviceExt;

pub(crate) struct TransformPool {
    pub(crate) buffers: Vec<wgpu::Buffer>,
    pub(crate) bind_groups: Vec<wgpu::BindGroup>,
    pub(crate) cursor: usize,
    pub(crate) cpu_mats: Vec<[[f32; 4]; 4]>,
}

pub(crate) struct InstanceBufferPoolEntry {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) capacity: u64,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) used_this_frame: bool,
    pub(crate) last_used_frame: u64,
}

pub(crate) fn create_identity_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
) -> wgpu::BindGroup {
    let identity = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("identity-transform-ubo"),
        contents: bytemuck::bytes_of(&identity),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
        label: Some(label),
    })
}

pub(crate) fn upload_instance_batch<T: bytemuck::Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    pool: &mut Vec<InstanceBufferPoolEntry>,
    frame_counter: u64,
    instances: &[T],
    buffer_label: &'static str,
    bind_group_label: &'static str,
) -> usize {
    let bytes_needed = std::mem::size_of_val(instances) as u64;
    let idx = pool
        .iter()
        .position(|entry| !entry.used_this_frame && entry.capacity >= bytes_needed)
        .unwrap_or_else(|| {
            let capacity = bytes_needed.next_power_of_two().max(256);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(buffer_label),
                size: capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(bind_group_label),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            pool.push(InstanceBufferPoolEntry {
                buffer,
                capacity,
                bind_group,
                used_this_frame: false,
                last_used_frame: frame_counter,
            });
            pool.len() - 1
        });
    let entry = &mut pool[idx];
    queue.write_buffer(&entry.buffer, 0, bytemuck::cast_slice(instances));
    entry.used_this_frame = true;
    entry.last_used_frame = frame_counter;
    idx
}

pub(crate) fn reset_instance_pool_usage(pool: &mut [InstanceBufferPoolEntry]) {
    for entry in pool {
        entry.used_this_frame = false;
    }
}

pub(crate) fn evict_instance_pool(
    pool: &mut Vec<InstanceBufferPoolEntry>,
    frame_counter: u64,
    max_pool: usize,
    evict_age: u64,
) {
    if pool.len() > max_pool {
        pool.retain(|entry| frame_counter.saturating_sub(entry.last_used_frame) < evict_age);
        if pool.len() > max_pool {
            pool.sort_by_key(|entry| entry.last_used_frame);
            pool.truncate(max_pool);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RectStyleKey {
    pub(crate) fill_rgba_u8: [u8; 4],
    pub(crate) border_rgba_u8: [u8; 4],
    pub(crate) corner_radius_10x: u16,    // quantized 0.1 px
    pub(crate) border_thickness_10x: u16, // quantized 0.1 px
}

pub(crate) fn to_rgba_u8(c: [f32; 4]) -> [u8; 4] {
    [
        (c[0].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[1].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[2].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (c[3].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
    ]
}

pub(crate) fn quant_10x(v: f32) -> u16 {
    ((v.max(0.0) * 10.0) + 0.5).floor() as u16
}

impl TransformPool {
    pub(crate) fn new() -> Self {
        Self {
            buffers: Vec::new(),
            bind_groups: Vec::new(),
            cursor: 0,
            cpu_mats: Vec::new(),
        }
    }
    pub(crate) fn reset(&mut self) {
        self.cursor = 0;
        self.cpu_mats.clear();
    }
}

impl<'a> super::PlutoniumEngine<'a> {
    /// Queues this object for rendering.
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(());
        }

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
        // GPU timestamp begin — access fields directly to allow field-level borrow splitting
        // (calling &self methods on gpu_timer would lock all of self during the render loop)
        let qcount = self.gpu_timer.count;
        let qindex = if qcount >= 2 {
            self.gpu_timer.frame_index % (qcount / 2)
        } else {
            0
        };
        let q0 = qindex * 2;
        let q1 = q0 + 1;
        // We'll write timestamps via render pass timestamp_writes when supported.

        {
            // Fast path for the common already-layered queue: avoid invoking
            // stable sort (and its scratch allocation) unless z-order changed.
            if !self.render_queue.windows(2).all(|w| w[0].z <= w[1].z) {
                self.render_queue.sort_by_key(|item| item.z);
            }

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
                timestamp_writes: self.gpu_timer.query.as_ref().map(|qs| {
                    wgpu::RenderPassTimestampWrites {
                        query_set: qs,
                        beginning_of_pass_write_index: Some(q0),
                        end_of_pass_write_index: Some(q1),
                    }
                }),
                occlusion_query_set: None,
            });

            // Set default pipeline for texture/atlas draws; rect draws will override temporarily
            rpass.set_pipeline(&self.render_pipeline);

            // Streaming batcher that preserves z-order and interleaves atlas draws
            let mut current_tex: Option<Uuid> = None;
            let mut batch_indices: Vec<usize> = Vec::new();
            let mut current_atlas: Option<Uuid> = None;
            let mut current_atlas_is_msdf = false;
            let mut atlas_instances: Vec<crate::utils::InstanceRaw> = Vec::new();
            // Rect batching
            let mut rect_instances: Vec<crate::utils::RectInstanceRaw> = Vec::new();
            let mut rect_draw_calls: usize = 0;
            // Glow batching
            let mut glow_instances: Vec<crate::utils::GlowInstanceRaw> = Vec::new();
            let mut active_item_clip: Option<Rectangle> = None;
            let mut scissor_initialized = false;

            macro_rules! flush_sprite_batch {
                ($rpass:expr, $tex_id:expr, $indices:expr) => {
                    if !$indices.is_empty() {
                        if let Some(tid) = $tex_id {
                            if self.texture_map.contains_key(&tid) {
                                let instances: Vec<crate::utils::InstanceRaw> = $indices
                                    .iter()
                                    .map(|i| crate::utils::InstanceRaw {
                                        model: self.transform_pool.cpu_mats[*i],
                                        uv_offset: [0.0, 0.0],
                                        uv_scale: [1.0, 1.0],
                                        tint: [1.0, 1.0, 1.0, 1.0],
                                        msdf_px_range: 0.0,
                                        _msdf_pad: [0.0, 0.0, 0.0],
                                    })
                                    .collect();
                                let pool_idx = upload_instance_batch(
                                    &self.device,
                                    &self.queue,
                                    &self.instance_bind_group_layout,
                                    &mut self.sprite_instance_pool,
                                    self.frame_counter,
                                    &instances,
                                    "sprite-instance-buffer",
                                    "sprite-instance-bg",
                                );
                                let texture = &self.texture_map[&tid];
                                $rpass.set_bind_group(0, texture.bind_group(), &[]);
                                $rpass.set_bind_group(1, &self.identity_transform_bg, &[]);
                                $rpass.set_bind_group(2, texture.uv_bind_group(), &[]);
                                $rpass.set_bind_group(
                                    3,
                                    &self.sprite_instance_pool[pool_idx].bind_group,
                                    &[],
                                );
                                $rpass.set_vertex_buffer(0, texture.vertex_buffer_slice());
                                $rpass.set_index_buffer(
                                    texture.index_buffer_slice(),
                                    wgpu::IndexFormat::Uint16,
                                );
                                $rpass.draw_indexed(
                                    0..texture.num_indices(),
                                    0,
                                    0..($indices.len() as u32),
                                );
                            }
                        }
                        $indices.clear();
                    }
                };
            }

            macro_rules! flush_rect_batch {
                ($rpass:expr, $instances:expr) => {
                    if !$instances.is_empty() {
                        let pool_idx = upload_instance_batch(
                            &self.device,
                            &self.queue,
                            &self.instance_bind_group_layout,
                            &mut self.rect_instance_pool,
                            self.frame_counter,
                            &$instances,
                            "rect-instance-buffer",
                            "rect-instance-bg",
                        );

                        $rpass.set_pipeline(&self.rect_pipeline);
                        $rpass.set_bind_group(0, &self.rect_dummy_bg, &[]);
                        $rpass.set_bind_group(1, &self.identity_transform_bg, &[]);
                        $rpass.set_bind_group(2, &self.rect_dummy_bg, &[]);
                        $rpass.set_bind_group(
                            3,
                            &self.rect_instance_pool[pool_idx].bind_group,
                            &[],
                        );
                        $rpass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
                        $rpass.set_index_buffer(
                            self.rect_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        $rpass.draw_indexed(0..6, 0, 0..($instances.len() as u32));
                        rect_draw_calls += 1;
                        $instances.clear();
                    }
                };
            }

            macro_rules! flush_glow_batch {
                ($rpass:expr, $glow_insts:expr) => {
                    if !$glow_insts.is_empty() {
                        let pool_idx = upload_instance_batch(
                            &self.device,
                            &self.queue,
                            &self.glow_instance_bgl,
                            &mut self.glow_instance_pool,
                            self.frame_counter,
                            &$glow_insts,
                            "glow-instance-buffer",
                            "glow-instance-bg",
                        );

                        $rpass.set_pipeline(&self.glow_pipeline);
                        $rpass.set_bind_group(0, &self.rect_dummy_bg, &[]);
                        $rpass.set_bind_group(1, &self.identity_transform_bg, &[]);
                        $rpass.set_bind_group(2, &self.rect_dummy_bg, &[]);
                        $rpass.set_bind_group(
                            3,
                            &self.glow_instance_pool[pool_idx].bind_group,
                            &[],
                        );
                        $rpass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
                        $rpass.set_index_buffer(
                            self.rect_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        $rpass.draw_indexed(0..6, 0, 0..($glow_insts.len() as u32));
                        $glow_insts.clear();
                    }
                };
            }

            macro_rules! flush_atlas_batch {
                ($rpass:expr, $instances:expr, $atlas_id:expr, $is_msdf:expr) => {
                    if !$instances.is_empty() {
                        if let Some(aid) = $atlas_id {
                            if self.atlas_map.contains_key(&aid) {
                                let pool_idx = upload_instance_batch(
                                    &self.device,
                                    &self.queue,
                                    &self.instance_bind_group_layout,
                                    &mut self.atlas_instance_pool,
                                    self.frame_counter,
                                    &$instances,
                                    "atlas-instance-buffer",
                                    "atlas-instance-bg",
                                );
                                let atlas = &self.atlas_map[&aid];
                                if $is_msdf {
                                    $rpass.set_pipeline(&self.msdf_render_pipeline);
                                } else {
                                    $rpass.set_pipeline(&self.render_pipeline);
                                }
                                $rpass.set_bind_group(0, &atlas.bind_group, &[]);
                                $rpass.set_bind_group(1, &self.identity_transform_bg, &[]);
                                $rpass.set_bind_group(2, atlas.default_uv_bind_group(), &[]);
                                $rpass.set_bind_group(
                                    3,
                                    &self.atlas_instance_pool[pool_idx].bind_group,
                                    &[],
                                );
                                $rpass.set_vertex_buffer(0, atlas.vertex_buffer.slice(..));
                                $rpass.set_index_buffer(
                                    atlas.index_buffer.slice(..),
                                    wgpu::IndexFormat::Uint16,
                                );
                                $rpass.draw_indexed(
                                    0..atlas.num_indices,
                                    0,
                                    0..($instances.len() as u32),
                                );
                            }
                        }
                        $instances.clear();
                    }
                };
            }

            for q in &self.render_queue {
                let effective_item_clip = self.effective_item_clip_rect(q.clip_rect);
                if !scissor_initialized || effective_item_clip != active_item_clip {
                    flush_sprite_batch!(rpass, current_tex, batch_indices);
                    current_tex = None;
                    flush_atlas_batch!(
                        rpass,
                        atlas_instances,
                        current_atlas,
                        current_atlas_is_msdf
                    );
                    flush_rect_batch!(rpass, rect_instances);
                    flush_glow_batch!(rpass, glow_instances);
                    self.apply_scissor_logical(&mut rpass, effective_item_clip);
                    active_item_clip = effective_item_clip;
                    scissor_initialized = true;
                    rpass.set_pipeline(&self.render_pipeline);
                }

                match &q.item {
                    RenderItem::Texture {
                        texture_key,
                        transform_index,
                    } => {
                        // Switching away from rects/glows; flush pending batches
                        flush_rect_batch!(rpass, rect_instances);
                        flush_glow_batch!(rpass, glow_instances);
                        // Switch back to texture/atlas pipeline after rects
                        rpass.set_pipeline(&self.render_pipeline);
                        match current_tex {
                            Some(tid) if tid == *texture_key => {
                                batch_indices.push(*transform_index);
                            }
                            _ => {
                                // different texture; flush previous
                                flush_sprite_batch!(rpass, current_tex, batch_indices);
                                flush_atlas_batch!(
                                    rpass,
                                    atlas_instances,
                                    current_atlas,
                                    current_atlas_is_msdf
                                );
                                current_tex = Some(*texture_key);
                                batch_indices.push(*transform_index);
                            }
                        }
                    }
                    RenderItem::AtlasTile {
                        texture_key,
                        transform_index,
                        tile_index,
                        tint,
                    } => {
                        // Switching away from rects/glows; flush pending batches
                        flush_rect_batch!(rpass, rect_instances);
                        flush_glow_batch!(rpass, glow_instances);
                        // Switch back to texture/atlas pipeline after rects
                        rpass.set_pipeline(&self.render_pipeline);
                        // flush any sprite batch first
                        flush_sprite_batch!(rpass, current_tex, batch_indices);
                        current_tex = None;
                        // switch atlas batch if needed
                        if current_atlas != Some(*texture_key) || current_atlas_is_msdf {
                            flush_atlas_batch!(
                                rpass,
                                atlas_instances,
                                current_atlas,
                                current_atlas_is_msdf
                            );
                            current_atlas = Some(*texture_key);
                            current_atlas_is_msdf = false;
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
                                    tint: *tint,
                                    msdf_px_range: 0.0,
                                    _msdf_pad: [0.0, 0.0, 0.0],
                                });
                            }
                        }
                    }
                    RenderItem::AtlasGlyph {
                        texture_key,
                        transform_index,
                        uv_offset,
                        uv_scale,
                        tint,
                        is_msdf,
                        msdf_px_range,
                    } => {
                        flush_rect_batch!(rpass, rect_instances);
                        flush_glow_batch!(rpass, glow_instances);
                        flush_sprite_batch!(rpass, current_tex, batch_indices);
                        current_tex = None;

                        if current_atlas != Some(*texture_key) || current_atlas_is_msdf != *is_msdf
                        {
                            flush_atlas_batch!(
                                rpass,
                                atlas_instances,
                                current_atlas,
                                current_atlas_is_msdf
                            );
                            current_atlas = Some(*texture_key);
                            current_atlas_is_msdf = *is_msdf;
                        }

                        let model = self.transform_pool.cpu_mats[*transform_index];
                        atlas_instances.push(crate::utils::InstanceRaw {
                            model,
                            uv_offset: *uv_offset,
                            uv_scale: *uv_scale,
                            tint: *tint,
                            msdf_px_range: *msdf_px_range,
                            _msdf_pad: [0.0, 0.0, 0.0],
                        });
                    }
                    RenderItem::Rect(cmd) => {
                        // Flush any pending sprite/atlas/glow batches before enqueueing rects
                        flush_sprite_batch!(rpass, current_tex, batch_indices);
                        current_tex = None;
                        flush_glow_batch!(rpass, glow_instances);
                        flush_atlas_batch!(
                            rpass,
                            atlas_instances,
                            current_atlas,
                            current_atlas_is_msdf
                        );

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
                    RenderItem::Glow(cmd) => {
                        // Flush any pending sprite/atlas/rect batches
                        flush_sprite_batch!(rpass, current_tex, batch_indices);
                        current_tex = None;
                        flush_atlas_batch!(
                            rpass,
                            atlas_instances,
                            current_atlas,
                            current_atlas_is_msdf
                        );
                        flush_rect_batch!(rpass, rect_instances);

                        glow_instances.push(crate::utils::GlowInstanceRaw {
                            model: cmd.transform,
                            color: cmd.color,
                            rect_size_px: [cmd.width_px, cmd.height_px],
                            corner_radius_px: cmd.corner_radius_px,
                            glow_radius_px: cmd.glow_radius_px,
                            sigma: cmd.sigma,
                            max_alpha: cmd.max_alpha,
                            mode: cmd.mode,
                            border_width: cmd.border_width,
                            _pad: [0.0, 0.0, 0.0, 0.0],
                        });
                    }
                }
            }
            // flush any remaining sprite batch
            flush_sprite_batch!(rpass, current_tex, batch_indices);
            // flush any remaining atlas batch
            flush_atlas_batch!(rpass, atlas_instances, current_atlas, current_atlas_is_msdf);
            // flush any remaining rects
            flush_rect_batch!(rpass, rect_instances);
            // flush any remaining glows
            flush_glow_batch!(rpass, glow_instances);
            self.rect_draw_calls_count = rect_draw_calls;
        }
        // End timestamp + resolve
        self.gpu_timer.resolve(&mut encoder, q0, q1);
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        // Read back timestamps (synchronously for simplicity)
        self.gpu_timer
            .readback_and_report(&self.device, &self.queue, q0);
        Ok(())
    }

    pub(crate) fn halo_world_rect_from_screen_rect(&self, rect: Rectangle) -> Rectangle {
        use crate::utils::Position;
        let cam_px = self.camera.get_pos(self.dpi_scale_factor);
        let cam_logical = Position {
            x: cam_px.x / self.dpi_scale_factor,
            y: cam_px.y / self.dpi_scale_factor,
        };
        Rectangle::new(
            rect.x + cam_logical.x,
            rect.y + cam_logical.y,
            rect.width,
            rect.height,
        )
    }

    pub(crate) fn halo_screen_rect_from_world_rect(&self, rect: Rectangle) -> Rectangle {
        use crate::utils::Position;
        let cam_px = self.camera.get_pos(self.dpi_scale_factor);
        let cam_logical = Position {
            x: cam_px.x / self.dpi_scale_factor,
            y: cam_px.y / self.dpi_scale_factor,
        };
        Rectangle::new(
            rect.x - cam_logical.x,
            rect.y - cam_logical.y,
            rect.width,
            rect.height,
        )
    }

    pub(crate) fn screen_space_viewport_rect(&self) -> Rectangle {
        Rectangle::new(
            0.0,
            0.0,
            self.viewport_size.width / self.dpi_scale_factor,
            self.viewport_size.height / self.dpi_scale_factor,
        )
    }

    pub(crate) fn rects_intersect(a: Rectangle, b: Rectangle) -> bool {
        if a.width <= 0.0 || a.height <= 0.0 || b.width <= 0.0 || b.height <= 0.0 {
            return false;
        }
        let ax2 = a.x + a.width;
        let ay2 = a.y + a.height;
        let bx2 = b.x + b.width;
        let by2 = b.y + b.height;
        a.x < bx2 && ax2 > b.x && a.y < by2 && ay2 > b.y
    }

    fn rect_intersection(a: Rectangle, b: Rectangle) -> Option<Rectangle> {
        let x1 = a.x.max(b.x);
        let y1 = a.y.max(b.y);
        let x2 = (a.x + a.width).min(b.x + b.width);
        let y2 = (a.y + a.height).min(b.y + b.height);
        let w = (x2 - x1).max(0.0);
        let h = (y2 - y1).max(0.0);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some(Rectangle::new(x1, y1, w, h))
    }

    fn effective_item_clip_rect(&self, item_clip: Option<Rectangle>) -> Option<Rectangle> {
        let global_clip = self.clip_stack.last().copied().or(self.current_scissor);
        match (global_clip, item_clip) {
            (Some(global), Some(item)) => {
                Self::rect_intersection(global, item).or(Some(Rectangle::new(0.0, 0.0, 0.0, 0.0)))
            }
            (Some(global), None) => Some(global),
            (None, Some(item)) => Some(item),
            (None, None) => None,
        }
    }

    fn apply_scissor_logical(
        &self,
        rpass: &mut wgpu::RenderPass<'_>,
        clip_rect: Option<Rectangle>,
    ) {
        if let Some(sc) = clip_rect {
            let x_phys = (sc.x * self.dpi_scale_factor).floor() as i32;
            let y_phys = (sc.y * self.dpi_scale_factor).floor() as i32;
            let w_phys = (sc.width * self.dpi_scale_factor).floor() as i32;
            let h_phys = (sc.height * self.dpi_scale_factor).floor() as i32;

            // Intersect physical rect with render target boundaries [0, 0, width, height]
            let x = x_phys.clamp(0, self.config.width as i32) as u32;
            let y = y_phys.clamp(0, self.config.height as i32) as u32;

            let x2 = (x_phys + w_phys).clamp(0, self.config.width as i32) as u32;
            let y2 = (y_phys + h_phys).clamp(0, self.config.height as i32) as u32;

            let w = x2.saturating_sub(x);
            let h = y2.saturating_sub(y);

            if w > 0 && h > 0 {
                rpass.set_scissor_rect(x, y, w, h);
            } else {
                // If the intersection is empty (off-screen), set a 1x1 rect at [0,0]
                // and we expect the batcher to ideally skip this or just let it draw nothing.
                // wgpu requires w > 0 and h > 0.
                rpass.set_scissor_rect(0, 0, 1, 1);
            }
        } else {
            rpass.set_scissor_rect(0, 0, self.config.width, self.config.height);
        }
    }

    pub(crate) fn allocate_transform_bind_group(
        &mut self,
        transform_uniform: TransformUniform,
    ) -> usize {
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
}
