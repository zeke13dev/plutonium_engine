use crate::utils::*;
use resvg::usvg::{Options, Tree};
use std::{fs, num::NonZeroU64};
use tiny_skia::{Color, Pixmap};
use uuid::Uuid;
use wgpu::util::DeviceExt;

#[allow(dead_code)]
#[derive(Debug)]
pub struct TextureSVG {
    texture_key: Uuid,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    active_buffer_index: usize,
    transform_uniform: TransformUniform,
    transform_uniform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    vertices: Vec<Vertex>,
    dimensions: Rectangle,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    uv_uniform_buffer: wgpu::Buffer,
    uv_bind_group: wgpu::BindGroup,
}

impl TextureSVG {
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

    /// Updates the text content of the existing texture without recreating it.
    pub fn update_text(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        new_text: &str,
        font_size: f32,
        viewport_size: Size,
        camera_position: Position,
    ) -> Result<(), String> {
        // Define padding
        let padding = font_size * 0.1;

        // Create SVG data with predefined dimensions
        let svg_data = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{}" height="{}">
                    <text x="{}" y="{}" font-family="Verdana" font-size="{}" fill="black">{}</text>
                </svg>"#,
            self.dimensions.width as u32,  // Use existing texture width
            self.dimensions.height as u32, // Use existing texture height
            padding.ceil() as u32,         // Adjusted padding for X position
            (padding + font_size * 0.8).ceil() as u32, // Adjusted Y position
            font_size,                     // Font size
            new_text                       // New text content
        );

        // Parse SVG
        let opt = Options::default();
        let mut fontdb = resvg::usvg::fontdb::Database::new();
        fontdb.load_system_fonts(); // Ensure system fonts are loaded
        let rtree = Tree::from_str(&svg_data, &opt, &fontdb)
            .map_err(|e| format!("Failed to parse SVG: {}", e))?;
        let svg_size = rtree.size();
        let svg_width = svg_size.width().ceil() as u32;
        let svg_height = svg_size.height().ceil() as u32;

        // Check if the SVG fits within the preallocated texture
        if svg_width > self.dimensions.width as u32 || svg_height > self.dimensions.height as u32 {
            return Err("New text size exceeds the preallocated texture dimensions.".to_string());
        }

        // Render SVG into pixmap
        let pixmap = {
            let mut pixmap =
                Pixmap::new(svg_width, svg_height).ok_or("Failed to create pixmap.")?;
            pixmap.fill(Color::TRANSPARENT);
            let transform = tiny_skia::Transform::identity();
            resvg::render(&rtree, transform, &mut pixmap.as_mut());
            pixmap
        };

        // Upload pixmap data to existing texture
        let bytes_per_pixel = 4;
        let unpadded_bytes_per_row = pixmap.width() as usize * bytes_per_pixel;
        const COPY_BYTES_PER_ROW_ALIGNMENT: usize = 256;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            / COPY_BYTES_PER_ROW_ALIGNMENT)
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
            label: Some("Texture Update Encoder"),
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
                texture: &self.texture,
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

        // Optionally, update UV coordinates or other related data here
        // For example, if the actual rendered size is different, adjust accordingly
        self.update_transform_uniform(device, queue, viewport_size, camera_position);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_from_data(
        texture_key: Uuid,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        svg_data: &str,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        screen_pos: Position,
        scale_factor: f32,
    ) -> Option<Self> {
        let (texture, pixel_size) =
            Self::svg_to_texture(Some(svg_data), None, device, queue, scale_factor)?;

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = Self::create_sampler(device);
        let bind_group =
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

        let num_tiles = 1;
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

        let dimensions = Rectangle::new(
            screen_pos.x,
            screen_pos.y,
            pixel_size.width,
            pixel_size.height,
        );

        Some(Self {
            texture_key,
            texture,
            view,
            bind_group,
            active_buffer_index: 0,
            transform_uniform,
            transform_uniform_buffer,
            transform_bind_group,
            vertices,
            dimensions,
            vertex_buffer,
            index_buffer,
            num_indices: 6,
            uv_uniform_buffer,
            uv_bind_group: default_uv_bind_group,
        })
    }

    /// Creates a new `TextureSVG` instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        texture_key: Uuid,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        file_path: &str,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        screen_pos: Position,
        scale_factor: f32,
    ) -> Option<Self> {
        let (texture, pixel_size) =
            Self::svg_to_texture(None, Some(file_path), device, queue, scale_factor)?;

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

        let num_tiles = 1;
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
        let dimensions = Rectangle::new(
            screen_pos.x,
            screen_pos.y,
            pixel_size.width,
            pixel_size.height,
        );

        Some(Self {
            texture_key,
            texture,
            view,
            bind_group,
            active_buffer_index: 0,
            transform_uniform,
            transform_uniform_buffer,
            transform_bind_group,
            vertices,
            dimensions,
            vertex_buffer,
            index_buffer,
            num_indices: 6,
            uv_uniform_buffer,
            uv_bind_group: default_uv_bind_group,
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

        let size = self.dimensions.size();
        self.adjust_vertex_texture_coordinates(size, viewport_size);
        self.update_vertex_buffer(device);

        // Calculate NDC scaling factors
        let width_ndc = size.width / viewport_width;
        let height_ndc = size.height / viewport_height;

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

    /// gets the transform uniform based on the viewport size and adjusts for position.
    pub fn get_transform_uniform(
        &self,
        viewport_size: Size,
        pos: Position,
        camera_position: Position,
    ) -> TransformUniform {
        let width = self.dimensions.width;
        let height = self.dimensions.height;

        let width_ndc = width / viewport_size.width;
        let height_ndc = height / viewport_size.height;

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

    /// Converts an SVG file to a wgpu texture.
    fn svg_to_texture(
        svg_data: Option<&str>,
        file_path: Option<&str>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scale_factor: f32,
    ) -> Option<(wgpu::Texture, Size)> {
        let svg_content = if let Some(data) = svg_data {
            data.to_string()
        } else if let Some(path) = file_path {
            fs::read_to_string(path).expect("file should exist")
        } else {
            return None;
        };
        let opt = Options::default();
        let fontdb = resvg::usvg::fontdb::Database::new();
        let rtree = Tree::from_str(&svg_content, &opt, &fontdb).ok()?;
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
                width: pixmap.width(),
                height: pixmap.height(),
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

    /// Swaps the active texture buffer.
    pub fn swap_buffers(&mut self) {
        self.active_buffer_index = 1 - self.active_buffer_index;
    }

    /* higher level functions */

    pub fn contains(&self, pos: &Position) -> bool {
        self.dimensions.contains(*pos)
    }
}
