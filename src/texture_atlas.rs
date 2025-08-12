use crate::text::CharacterInfo;
use crate::utils::*;
use resvg::usvg::{Options, Tree};
use std::collections::HashMap;
use std::{fs, num::NonZeroU64};
use uuid::Uuid;
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy)]
struct BufferDimensions {
    width: u32,
    height: u32,
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
}

impl BufferDimensions {
    fn new(width: u32, height: u32) -> Self {
        let bytes_per_pixel = std::mem::size_of::<u32>() as u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row_padding = (align - unpadded_bytes_per_row % align) % align;
        let padded_bytes_per_row = unpadded_bytes_per_row + padded_bytes_per_row_padding;

        Self {
            width,
            height,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct TextureAtlas {
    texture_key: Uuid,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    transform_uniform: TransformUniform,
    transform_uniform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    vertices: Vec<Vertex>,
    dimensions: Rectangle,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    uv_uniform_buffer: wgpu::Buffer,
    uv_bind_groups: Vec<wgpu::BindGroup>,
    uv_bind_group: wgpu::BindGroup,
    tile_size: Size,
}

impl TextureAtlas {
    /// Sets the position of the texture.
    pub fn set_position(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        position: Position,
        viewport_size: Size,
        camera_position: Position,
    ) {
        self.dimensions.set_pos(position);
        self.update_transform_uniform(device, queue, viewport_size, camera_position);
    }

    fn calculate_required_tiles(char_positions: &HashMap<char, CharacterInfo>) -> usize {
        let mut max_index = 0;
        for info in char_positions.values() {
            max_index = max_index.max(info.tile_index);
        }
        max_index + 1 // Add 1 because indices are 0-based
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_from_texture(
        texture_key: Uuid,
        texture: wgpu::Texture,
        texture_bind_group: wgpu::BindGroup,
        position: Position,
        size: Size,
        tile_size: Size,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        char_positions: &HashMap<char, CharacterInfo>,
    ) -> Option<Self> {
        // Create texture view for rendering
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Initialize default transform matrix
        let transform_uniform = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };

        // Set up vertex and index buffers
        let (vertices, vertex_buffer, index_buffer) = Self::initialize_buffers(device);

        // Create transform uniform buffer
        let transform_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Transform Uniform Buffer"),
                contents: bytemuck::cast_slice(&[transform_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Create transform bind group
        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_uniform_buffer.as_entire_binding(),
            }],
            label: Some("Transform Bind Group"),
        });

        // Create UV bind group layout
        let uv_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<UVTransform>() as u64
                        ),
                    },
                    count: None,
                }],
                label: Some("UV Bind Group Layout"),
            });

        // Calculate how many tiles we need based on character positions
        let num_tiles = Self::calculate_required_tiles(char_positions);

        // Set up memory alignment for UV buffer
        let alignment = 256; // WebGPU buffer alignment requirement
        let element_size = std::mem::size_of::<UVTransform>();
        let aligned_element_size = (element_size + alignment - 1) / alignment * alignment;
        let buffer_size = num_tiles * aligned_element_size;

        // Create single UV uniform buffer for all transforms
        let uv_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UV Uniform Buffer"),
            contents: bytemuck::cast_slice(&vec![
                UVTransform {
                    uv_offset: [0.0, 0.0],
                    uv_scale: [1.0, 1.0],
                };
                buffer_size / element_size
            ]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Set up texture dimensions
        let dimensions = Rectangle::new(position.x, position.y, size.width, size.height);
        let mut uv_bind_groups = Vec::with_capacity(num_tiles);

        // Create bind groups for each tile
        for tile_index in 0..num_tiles {
            let offset = (tile_index * aligned_element_size) as u64;

            // Calculate UV coordinates using grid-based approach
            if let Some(tile_rect) = Self::tile_uv_coordinates(tile_index, tile_size, size) {
                let uv_transform = UVTransform {
                    uv_offset: [tile_rect.x, tile_rect.y],
                    uv_scale: [tile_rect.width, tile_rect.height],
                };

                // Write UV transform to buffer at the correct offset
                queue.write_buffer(
                    &uv_uniform_buffer,
                    offset,
                    bytemuck::cast_slice(&[uv_transform]),
                );

                // Create bind group for this tile
                let uv_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &uv_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uv_uniform_buffer,
                            offset,
                            size: NonZeroU64::new(std::mem::size_of::<UVTransform>() as u64),
                        }),
                    }],
                    label: Some(&format!("UV Bind Group for tile {}", tile_index)),
                });

                // Debug output
                uv_bind_groups.push(uv_bind_group);
            }
        }

        // Create default UV bind group (used as fallback)
        let default_uv_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uv_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uv_uniform_buffer,
                    offset: 0,
                    size: NonZeroU64::new(std::mem::size_of::<UVTransform>() as u64),
                }),
            }],
            label: Some("Default UV Bind Group"),
        });

        Some(TextureAtlas {
            texture_key,
            texture,
            view,
            bind_group: texture_bind_group,
            transform_uniform,
            transform_uniform_buffer,
            transform_bind_group,
            vertices,
            dimensions,
            vertex_buffer,
            index_buffer,
            num_indices: 6,
            uv_uniform_buffer,
            uv_bind_groups,
            uv_bind_group: default_uv_bind_group,
            tile_size,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        texture_key: Uuid,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        file_path: &str,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        screen_pos: Position,
        tile_size: Size,
    ) -> Option<Self> {
        let (texture, pixel_size) = Self::svg_to_texture(file_path, device, queue)?;

        let view: wgpu::TextureView = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = Self::create_sampler(device);
        let bind_group: wgpu::BindGroup =
            Self::create_bind_group(device, &view, &sampler, texture_bind_group_layout);

        let (vertices, vertex_buffer, index_buffer) = Self::initialize_buffers(device);
        let transform_uniform = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };

        let transform_uniform_buffer = Self::create_uniform_buffer(device, &transform_uniform);
        let transform_bind_group = Self::create_bind_group_for_transform(
            device,
            &transform_uniform_buffer,
            transform_bind_group_layout,
        );

        let uv_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<UVTransform>() as u64
                        ),
                    },
                    count: None,
                }],
                label: Some("UV Bind Group Layout"),
            });

        let num_tiles = (pixel_size.width as usize / tile_size.width as usize)
            * (pixel_size.height as usize / tile_size.height as usize);

        let alignment = 256;
        let element_size = std::mem::size_of::<UVTransform>();
        let aligned_element_size = (element_size + alignment - 1) / alignment * alignment;
        let buffer_size = num_tiles * aligned_element_size;

        let uv_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UV Uniform Buffer"),
            contents: bytemuck::cast_slice(&vec![
                UVTransform {
                    uv_offset: [0.0, 0.0],
                    uv_scale: [1.0, 1.0]
                };
                buffer_size / element_size
            ]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let dimensions = Rectangle::new(
            screen_pos.x,
            screen_pos.y,
            pixel_size.width,
            pixel_size.height,
        );

        let uv_bind_groups = (0..num_tiles)
            .filter_map(|i| {
                let offset = (i * aligned_element_size) as u64;
                if offset + aligned_element_size as u64 > buffer_size as u64 {
                    None
                } else {
                    if let Some(tile_rect) =
                        Self::tile_uv_coordinates(i, tile_size, dimensions.size())
                    {
                        let uv_transform = UVTransform {
                            uv_offset: [tile_rect.x, tile_rect.y],
                            uv_scale: [tile_rect.width, tile_rect.height],
                        };
                        queue.write_buffer(
                            &uv_uniform_buffer,
                            offset,
                            bytemuck::bytes_of(&uv_transform),
                        );
                    }

                    Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &uv_bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &uv_uniform_buffer,
                                offset,
                                // Bind only the size of the UVTransform; the offset is already 256-byte aligned
                                size: NonZeroU64::new(std::mem::size_of::<UVTransform>() as u64),
                            }),
                        }],
                        label: Some("UV Bind Group"),
                    }))
                }
            })
            .collect();

        let default_uv_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uv_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uv_uniform_buffer,
                    offset: 0,
                    size: NonZeroU64::new(std::mem::size_of::<UVTransform>() as u64),
                }),
            }],
            label: Some("Default UV Bind Group"),
        });

        Some(Self {
            texture_key,
            texture,
            view,
            bind_group,
            transform_uniform,
            transform_uniform_buffer,
            transform_bind_group,
            vertices,
            dimensions,
            vertex_buffer,
            index_buffer,
            num_indices: 6,
            uv_uniform_buffer,
            uv_bind_groups,
            uv_bind_group: default_uv_bind_group,
            tile_size,
        })
    }

    /// Creates a sampler for texture filtering.
    fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        // Use nearest filtering and clamp to edge for atlas sampling to avoid bleeding between tiles
        device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        })
    }

    /// Creates a bind group for the texture and sampler.
    fn create_bind_group(
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
            label: Some("texture_bind_group"),
        })
    }

    /// Initializes the vertex and index buffers.
    fn initialize_buffers(device: &wgpu::Device) -> (Vec<Vertex>, wgpu::Buffer, wgpu::Buffer) {
        let vertices = vec![
            Vertex {
                position: [-0.5, 0.5],
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [0.5, 0.5],
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [-0.5, -0.5],
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                position: [0.5, -0.5],
                tex_coords: [1.0, 1.0],
            },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices: [u16; 6] = [0, 1, 2, 2, 1, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        (vertices, vertex_buffer, index_buffer)
    }

    /// Creates a uniform buffer for the transform matrix.
    fn create_uniform_buffer(
        device: &wgpu::Device,
        transform_uniform: &TransformUniform,
    ) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Transform Uniform Buffer"),
            contents: bytemuck::bytes_of(transform_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    /// Creates a bind group for the transform uniform buffer.
    fn create_bind_group_for_transform(
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: None,
                }),
            }],
            label: Some("transform_bind_group"),
        })
    }

    /// Returns the position of the texture.
    pub fn pos(&self) -> Position {
        self.dimensions.pos()
    }

    /// Returns the size of the texture.
    pub fn size(&self) -> Size {
        self.dimensions.size()
    }

    /// Returns the dimensions of the texture.
    pub fn dimensions(&self) -> Rectangle {
        self.dimensions
    }

    /// Updates the vertex buffer with the current vertices.
    pub fn update_vertex_buffer(&mut self, device: &wgpu::Device) {
        let new_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        self.vertex_buffer = new_vertex_buffer;
    }

    /// Updates the viewport and transform uniform based on the new viewport size.
    pub fn update_viewport(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport_size: Size,
        camera_position: Position,
    ) {
        self.update_transform_uniform(device, queue, viewport_size, camera_position);
    }

    pub fn update_transform_uniform(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport_size: Size,
        camera_position: Position,
    ) {
        let viewport_width = viewport_size.width;
        let viewport_height = viewport_size.height;

        let tile_size = self.tile_size;
        self.update_vertex_buffer(device);

        // Calculate NDC scaling factors
        let width_ndc = tile_size.width / viewport_width;
        let height_ndc = tile_size.height / viewport_height;

        // Calculate NDC position
        let ndc_x = (2.0 * (self.dimensions.x - camera_position.x)) / viewport_size.width - 1.0;
        let ndc_y = 1.0 - (2.0 * (self.dimensions.y - camera_position.y)) / viewport_size.height;

        // Construct transformation matrix in column-major order
        let transform = [
            [1.0, 0.0, 0.0, ndc_x + width_ndc],
            [0.0, 1.0, 0.0, ndc_y - height_ndc],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];

        self.transform_uniform.transform = transform;
        queue.write_buffer(
            &self.transform_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.transform_uniform]),
        );
    }

    pub fn render<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        render_pipeline: &'a wgpu::RenderPipeline,
        transform_bind_group: &'a wgpu::BindGroup,
    ) {
        rpass.set_pipeline(render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_bind_group(1, transform_bind_group, &[]);
        rpass.set_bind_group(2, &self.uv_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..)); // Add this line
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..self.num_indices, 0, 0..1);
    }

    pub fn render_tile<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        render_pipeline: &'a wgpu::RenderPipeline,
        tile_index: usize,
        tile_bind_group: &'a wgpu::BindGroup,
    ) {
        rpass.set_pipeline(render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_bind_group(1, tile_bind_group, &[]);

        // Add safety check for bind group access
        let uv_bind_group = if tile_index < self.uv_bind_groups.len() {
            &self.uv_bind_groups[tile_index]
        } else {
            println!(
                "Warning: Tile index {} out of bounds (max: {}), using default UV bind group",
                tile_index,
                self.uv_bind_groups.len() - 1
            );
            &self.uv_bind_group
        };

        rpass.set_bind_group(2, uv_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..self.num_indices, 0, 0..1);
    }

    pub fn get_transform_uniform(
        &self,
        viewport_size: Size,
        pos: Position, // Logical top-left position
        camera_pos: Position,
        scale: f32, // Combined user scale * DPI scale
    ) -> TransformUniform {
        // Scaled tile size
        let scaled_tile_w = self.tile_size.width * scale;
        let scaled_tile_h = self.tile_size.height * scale;

        // Logical position -> Final on-screen pixels
        let final_x = (pos.x - camera_pos.x) * scale;
        let final_y = (pos.y - camera_pos.y) * scale;

        // Convert to NDC
        let ndc_left = 2.0 * (final_x / viewport_size.width) - 1.0;
        let ndc_top = -2.0 * (final_y / viewport_size.height) + 1.0;

        // Width and height in NDC
        let tile_w_ndc = 2.0 * (scaled_tile_w / viewport_size.width);
        let tile_h_ndc = 2.0 * (scaled_tile_h / viewport_size.height);

        TransformUniform {
            transform: [
                [tile_w_ndc, 0.0, 0.0, 0.0],  // Scale X
                [0.0, -tile_h_ndc, 0.0, 0.0], // Scale Y (negative to flip)
                [0.0, 0.0, 1.0, 0.0],         // Z remains untouched
                [
                    ndc_left + tile_w_ndc * 0.5, // Translate X
                    ndc_top - tile_h_ndc * 0.5,  // Translate Y
                    0.0,
                    1.0,
                ], // Homogeneous coord
            ],
        }
    }

    // Pure helper for tests: compute transform without accessing self
    pub fn compute_transform_uniform(
        viewport_size: Size,
        pos: Position,
        camera_pos: Position,
        scale: f32,
        tile_size: Size,
    ) -> TransformUniform {
        let scaled_tile_w = tile_size.width * scale;
        let scaled_tile_h = tile_size.height * scale;

        let final_x = (pos.x - camera_pos.x) * scale;
        let final_y = (pos.y - camera_pos.y) * scale;

        let ndc_left = 2.0 * (final_x / viewport_size.width) - 1.0;
        let ndc_top = -2.0 * (final_y / viewport_size.height) + 1.0;

        let tile_w_ndc = 2.0 * (scaled_tile_w / viewport_size.width);
        let tile_h_ndc = 2.0 * (scaled_tile_h / viewport_size.height);

        TransformUniform {
            transform: [
                [tile_w_ndc, 0.0, 0.0, 0.0],
                [0.0, -tile_h_ndc, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [
                    ndc_left + tile_w_ndc * 0.5,
                    ndc_top - tile_h_ndc * 0.5,
                    0.0,
                    1.0,
                ],
            ],
        }
    }

    /// Adjusts the vertex texture coordinates based on the tile size and viewport size.
    pub fn adjust_vertex_texture_coordinates(&mut self, tile_size: Size, viewport_size: Size) {
        let tex_coords = [
            [0.0, 0.0], // Top-left
            [1.0, 0.0], // Top-right
            [0.0, 1.0], // Bottom-left
            [1.0, 1.0], // Bottom-right
        ];

        let width_ndc = tile_size.width / viewport_size.width;
        let height_ndc = tile_size.height / viewport_size.height;

        self.vertices = vec![
            Vertex {
                position: [-width_ndc, height_ndc],
                tex_coords: tex_coords[0],
            },
            Vertex {
                position: [width_ndc, height_ndc],
                tex_coords: tex_coords[1],
            },
            Vertex {
                position: [-width_ndc, -height_ndc],
                tex_coords: tex_coords[2],
            },
            Vertex {
                position: [width_ndc, -height_ndc],
                tex_coords: tex_coords[3],
            },
        ];
    }

    pub fn tile_uv_coordinates(
        tile_index: usize,
        tile_size: Size,
        atlas_size: Size,
    ) -> Option<Rectangle> {
        // Early return if we have invalid dimensions
        if atlas_size.width == 0.0
            || atlas_size.height == 0.0
            || tile_size.width == 0.0
            || tile_size.height == 0.0
        {
            return None;
        }

        // Calculate how many tiles can fit in each row and column
        let tiles_per_row = (atlas_size.width / tile_size.width).floor() as usize;
        if tiles_per_row == 0 {
            return None;
        }

        // Calculate the grid position
        let col = tile_index % tiles_per_row;
        let row = tile_index / tiles_per_row;

        // Calculate exact pixel positions in the texture
        let pixel_x = col as f32 * tile_size.width;
        let pixel_y = row as f32 * tile_size.height;

        // Verify we're not going outside texture bounds
        if pixel_x >= atlas_size.width || pixel_y >= atlas_size.height {
            return None;
        }

        // Convert to normalized UV coordinates (0.0 to 1.0 range) with inset
        let mut uv_x = (pixel_x + 0.5) / atlas_size.width;
        let mut uv_y = (pixel_y + 0.5) / atlas_size.height;

        // Calculate UV size with a 1px shrink to keep sampling fully inside the tile
        let mut uv_width = (tile_size.width - 1.0).max(0.0) / atlas_size.width;
        let mut uv_height = (tile_size.height - 1.0).max(0.0) / atlas_size.height;

        // Clamp to [0,1] to be safe
        uv_x = uv_x.clamp(0.0, 1.0);
        uv_y = uv_y.clamp(0.0, 1.0);
        uv_width = uv_width.clamp(0.0, 1.0 - uv_x);
        uv_height = uv_height.clamp(0.0, 1.0 - uv_y);

        Some(Rectangle::new(uv_x, uv_y, uv_width, uv_height))
    }

    
    /// Converts an SVG file to a wgpu texture.
    fn svg_to_texture(
        file_path: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<(wgpu::Texture, Size)> {
        let svg_data = fs::read_to_string(file_path)
            .unwrap_or_else(|_| panic!("file not found: {}", file_path));
        let opt = Options::default();
        let rtree = Tree::from_str(&svg_data, &opt).ok()?;
        let original_size = rtree.size();
        let scaled_size = Size {
            width: original_size.width(),
            height: original_size.height(),
        };
        let mut pixmap =
            tiny_skia::Pixmap::new(scaled_size.width as u32, scaled_size.height as u32)?;
        pixmap.fill(tiny_skia::Color::TRANSPARENT);

        let transform = tiny_skia::Transform::identity();
        resvg::render(&rtree, transform, &mut pixmap.as_mut());

        let svg_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SVG Texture"),
            size: wgpu::Extent3d {
                width: pixmap.width(),
                height: pixmap.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
        });

        let bytes_per_pixel = 4;
        let unpadded_bytes_per_row = pixmap.width() as usize * bytes_per_pixel;
        const COPY_BYTES_PER_ROW_ALIGNMENT: usize = 256;
        let padded_bytes_per_row = (unpadded_bytes_per_row + COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            / COPY_BYTES_PER_ROW_ALIGNMENT
            * COPY_BYTES_PER_ROW_ALIGNMENT;

        let total_size = padded_bytes_per_row * pixmap.height() as usize;
        let mut padded_buffer = vec![0u8; total_size];

        for y in 0..pixmap.height() as usize {
            let dst_start = y * padded_bytes_per_row;
            let src_start = y * unpadded_bytes_per_row;
            padded_buffer[dst_start..dst_start + unpadded_bytes_per_row]
                .copy_from_slice(&pixmap.data()[src_start..src_start + unpadded_bytes_per_row]);
        }

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SVG Pixel Buffer"),
            contents: &padded_buffer,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Texture Copy Encoder"),
        });
        encoder.copy_buffer_to_texture(
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row as u32),
                    rows_per_image: Some(pixmap.height()),
                },
            },
            wgpu::ImageCopyTexture {
                texture: &svg_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: pixmap.width(),
                height: pixmap.height(),
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));
        Some((
            svg_texture,
            Size {
                width: scaled_size.width,
                height: scaled_size.height,
            },
        ))
    }

    /* higher level functions */

    pub fn contains(&self, pos: &Position) -> bool {
        self.dimensions.contains(*pos)
    }

    pub fn tile_contains(&self, pos: &Position, tile_index: usize) -> bool {
        Rectangle {
            x: tile_index as f32 * self.tile_size.width,
            y: tile_index as f32 * self.tile_size.height,
            width: self.tile_size.width,
            height: self.tile_size.height,
        }
        .contains(*pos)
    }
    pub fn save_debug_png(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
    ) -> Result<(), String> {
        // First verify that the texture has the correct usage flags
        if !self.texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
            return Err("Texture does not have COPY_SRC usage flag".to_string());
        }

        // Create a buffer to copy texture data into
        let buffer_dimensions = BufferDimensions::new(
            self.dimensions.size().width as u32,
            self.dimensions.size().height as u32,
        );

        // Verify dimensions are non-zero
        if buffer_dimensions.width == 0 || buffer_dimensions.height == 0 {
            return Err("Invalid texture dimensions".to_string());
        }

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Texture Atlas Debug Buffer"),
            size: (buffer_dimensions.padded_bytes_per_row as u64 * buffer_dimensions.height as u64),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create command encoder for the copy operation
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Debug PNG Encoder"),
        });

        // Copy texture to buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(buffer_dimensions.padded_bytes_per_row),
                    rows_per_image: Some(buffer_dimensions.height),
                },
            },
            wgpu::Extent3d {
                width: buffer_dimensions.width,
                height: buffer_dimensions.height,
                depth_or_array_layers: 1,
            },
        );

        // Submit command encoder
        queue.submit(std::iter::once(encoder.finish()));

        // Create a mapping for the buffer
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| format!("Failed to receive mapping result: {}", e))?
            .map_err(|e| e.to_string())?;

        // Get the mapped data
        let padded_data = buffer_slice.get_mapped_range();

        // Convert from RGBA to RGB and remove padding
        let mut rgba =
            Vec::with_capacity((buffer_dimensions.width * buffer_dimensions.height * 4) as usize);

        for chunk in padded_data.chunks(buffer_dimensions.padded_bytes_per_row as usize) {
            rgba.extend_from_slice(&chunk[..buffer_dimensions.unpadded_bytes_per_row as usize]);
        }

        // Drop the mapping
        drop(padded_data);
        output_buffer.unmap();

        // Create the image buffer and save to PNG
        let image_buffer = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            buffer_dimensions.width,
            buffer_dimensions.height,
            rgba,
        )
        .ok_or("Failed to create image buffer")?;

        image_buffer
            .save(path)
            .map_err(|e| format!("Failed to save PNG: {}", e))?;

        Ok(())
    }
}
