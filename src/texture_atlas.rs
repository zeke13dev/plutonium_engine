use crate::text::CharacterInfo;
use crate::traits::UpdateContext;
use crate::{traits::PlutoObject, utils::*, PlutoniumEngine};
use resvg::usvg::{Options, Tree};
use std::collections::HashMap;
use std::{fs, num::NonZeroU64};
use tiny_skia::{Color, Pixmap};
use uuid::Uuid;
use wgpu::util::DeviceExt;
use winit::keyboard::Key;
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
        for (_, info) in char_positions {
            max_index = max_index.max(info.tile_index);
        }
        max_index + 1 // Add 1 because indices are 0-based
    }

    pub fn new_from_texture(
        texture_key: Uuid,
        texture: wgpu::Texture,
        texture_bind_group: wgpu::BindGroup,
        position: Position,
        size: Size,
        tile_size: Size,
        device: &wgpu::Device,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        char_positions: &HashMap<char, CharacterInfo>,
    ) -> Option<Self> {
        // Create texture view
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create transformation matrix
        let transform_uniform = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };

        // Initialize vertices and buffers
        let (vertices, vertex_buffer, index_buffer) = Self::initialize_buffers(device);

        // Create transform buffer and bind group
        let transform_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Transform Uniform Buffer"),
                contents: bytemuck::cast_slice(&[transform_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_uniform_buffer.as_entire_binding(),
            }],
            label: Some("Transform Bind Group"),
        });

        // Create UV bind groups
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

        let num_tiles = (size.width as usize / tile_size.width as usize)
            * (size.height as usize / tile_size.height as usize);

        let alignment = 256;
        let element_size = std::mem::size_of::<UVTransform>();
        let aligned_element_size = (element_size + alignment - 1) / alignment * alignment;
        let buffer_size = num_tiles * aligned_element_size;

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

        // Create UV bind groups for all tiles
        let dimensions = Rectangle::new(position.x, position.y, size.width, size.height);
        // Keep the actual atlas grid layout for reference
        // Get the grid layout for reference
        let grid_tiles_per_row = (size.width / tile_size.width).floor() as usize;
        let grid_tiles_per_column = (size.height / tile_size.height).floor() as usize;

        let num_tiles = Self::calculate_required_tiles(char_positions);
        println!("Creating {} UV bind groups for all characters", num_tiles);

        let mut uv_bind_groups = Vec::with_capacity(num_tiles);

        // Create bind groups for all possible tile indices
        for tile_index in 0..num_tiles {
            // Find the character that corresponds to this tile index
            let mut uv_transform = UVTransform {
                uv_offset: [0.0, 0.0],
                uv_scale: [0.0, 0.0],
            };

            // Look for character with this tile index
            for (c, info) in char_positions {
    if info.tile_index == tile_index {
        // Get the actual position and size from the CharacterInfo
        let position = Position {
            x: info.bearing.0,
            y: info.bearing.1
        };
        
        // Calculate UV coordinates using actual character position and size
        let uv_x = position.x / size.width;
        let uv_y = position.y / size.height;
        let uv_width = info.size.0 as f32 / size.width;
        let uv_height = info.size.1 as f32 / size.height;

        println!(
            "Character '{}' (index {}) UV coords: x={}, y={}, w={}, h={}",
            c, tile_index, uv_x, uv_y, uv_width, uv_height
        );

        uv_transform = UVTransform {
            uv_offset: [uv_x, uv_y],
            uv_scale: [uv_width, uv_height],
        };
        break;
    }
}
            let uv_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("UV Transform Buffer for tile {}", tile_index)),
                contents: bytemuck::cast_slice(&[uv_transform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let uv_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &uv_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uv_buffer.as_entire_binding(),
                }],
                label: Some(&format!("UV Bind Group for tile {}", tile_index)),
            });

            uv_bind_groups.push(uv_bind_group);
        }
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

        // Return owned TextureAtlas
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

    pub fn new(
        texture_key: Uuid,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        file_path: &str,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        screen_pos: Position,
        scale_factor: f32,
        tile_size: Size,
    ) -> Option<Self> {
        let (texture, pixel_size) = Self::svg_to_texture(file_path, device, queue, scale_factor)?;

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
                            offset as u64,
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
                                size: NonZeroU64::new(aligned_element_size as u64),
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
        device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
                position: [-0.5, 0.5, 0.0],
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [0.5, 0.5, 0.0],
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [-0.5, -0.5, 0.0],
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                position: [0.5, -0.5, 0.0],
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
        self.adjust_vertex_texture_coordinates(tile_size, viewport_size);
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
    /// gets the transform uniform based on the viewport size and adjusts for position.
    pub fn get_transform_uniform(
        &self,
        viewport_size: Size,
        pos: Position,
        camera_position: Position,
    ) -> TransformUniform {
        let tile_width = self.tile_size.width;
        let tile_height = self.tile_size.height;
        let width_ndc = tile_width / viewport_size.width;
        let height_ndc = tile_height / viewport_size.height;

        // Calculate NDC position
        let ndc_dx = (2.0 * (pos.x - camera_position.x)) / viewport_size.width - 1.0;
        let ndc_dy = 1.0 - (2.0 * (pos.y - camera_position.y)) / viewport_size.height;

        let ndc_x = ndc_dx + width_ndc;
        let ndc_y = ndc_dy - height_ndc;

        TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ndc_x, ndc_y, 0.0, 1.0],
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
                position: [-width_ndc, height_ndc, 0.0],
                tex_coords: tex_coords[0],
            },
            Vertex {
                position: [width_ndc, height_ndc, 0.0],
                tex_coords: tex_coords[1],
            },
            Vertex {
                position: [-width_ndc, -height_ndc, 0.0],
                tex_coords: tex_coords[2],
            },
            Vertex {
                position: [width_ndc, -height_ndc, 0.0],
                tex_coords: tex_coords[3],
            },
        ];
    }

    /// Calculates and returns the UV coordinates of a tile by index.
    fn tile_uv_coordinates(
        tile_index: usize,
        tile_size: Size,
        atlas_size: Size,
    ) -> Option<Rectangle> {
        if atlas_size.width == 0.0
            || atlas_size.height == 0.0
            || tile_size.width == 0.0
            || tile_size.height == 0.0
        {
            return None;
        }

        // Calculate maximum tiles that could fit in a row
        let tiles_per_row = (atlas_size.width / tile_size.width).floor() as usize;
        if tiles_per_row == 0 {
            return None;
        }

        let row = tile_index / tiles_per_row;
        let col = tile_index % tiles_per_row;

        // Calculate exact pixel positions
        let pixel_x = col as f32 * tile_size.width;
        let pixel_y = row as f32 * tile_size.height;

        // Ensure we're not going outside the atlas bounds
        if pixel_x >= atlas_size.width || pixel_y >= atlas_size.height {
            return None;
        }

        // Convert to normalized UV coordinates
        let uv_x = pixel_x / atlas_size.width;
        let uv_y = pixel_y / atlas_size.height;

        // Calculate UV size, ensuring we don't go outside the texture bounds
        let uv_width = (tile_size.width).min(atlas_size.width - pixel_x) / atlas_size.width;
        let uv_height = (tile_size.height).min(atlas_size.height - pixel_y) / atlas_size.height;

        Some(Rectangle::new(uv_x, uv_y, uv_width, uv_height))
    }
    /// Converts an SVG file to a wgpu texture.
    fn svg_to_texture(
        file_path: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scale_factor: f32,
    ) -> Option<(wgpu::Texture, Size)> {
        let svg_data = fs::read_to_string(file_path)
            .unwrap_or_else(|_| panic!("file not found: {}", file_path));
        let opt = Options::default();
        let fontdb = resvg::usvg::fontdb::Database::new();
        let rtree = Tree::from_str(&svg_data, &opt, &fontdb).ok()?;
        let original_size = rtree.size();
        let scaled_size = Size {
            width: original_size.width() * scale_factor,
            height: original_size.height() * scale_factor,
        };
        let mut pixmap =
            tiny_skia::Pixmap::new(scaled_size.width as u32, scaled_size.height as u32)?;
        pixmap.fill(tiny_skia::Color::TRANSPARENT);

        let transform = tiny_skia::Transform::from_scale(scale_factor, scale_factor);
        resvg::render(&rtree, transform, &mut pixmap.as_mut());

        let svg_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SVG Texture"),
            size: wgpu::Extent3d {
                width: pixmap.width() as u32,
                height: pixmap.height() as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
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
                    rows_per_image: Some(pixmap.height() as u32),
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
                width: scaled_size.width as f32,
                height: scaled_size.height as f32,
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
}
