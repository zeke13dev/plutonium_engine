use resvg::usvg::{Options, Tree};
use std::num::NonZeroU64;
use tiny_skia::{Pixmap, Transform as SkiaTransform};
use uuid::Uuid;
use wgpu::util::DeviceExt;

use crate::utils::*;

/// Represents a UV transformation for a tile in the texture atlas
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UVTransform {
    pub uv_offset: [f32; 2],
    pub uv_scale: [f32; 2],
}

/// Main structure for managing a texture atlas
pub struct TextureAtlas {
    // Core identification
    texture_key: Uuid,

    // Base texture components
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,

    // Transform management
    transform_uniform: TransformUniform,
    transform_uniform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,

    // Vertex and rendering data
    vertices: Vec<Vertex>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,

    // Atlas-specific data
    dimensions: Rectangle,
    tile_size: Size,
    uv_bind_groups: Vec<wgpu::BindGroup>,
    uv_uniform_buffer: wgpu::Buffer,
}

impl TextureAtlas {
    /// Creates a new TextureAtlas instance from an SVG
    pub fn new(
        texture_key: Uuid,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        svg_data: &str, // Raw SVG string data
        tile_size: Size,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        uv_bind_group_layout: &wgpu::BindGroupLayout,
        scale_factor: f32,
    ) -> Option<Self> {
        // First, we need to process the SVG data using resvg
        let opt = Options::default();
        let mut fontdb = resvg::usvg::fontdb::Database::new();
        fontdb.load_system_fonts();

        // Parse the SVG tree
        let rtree = Tree::from_str(svg_data, &opt, &fontdb).ok()?;
        let original_size = rtree.size();

        // Calculate scaled dimensions
        let scaled_size = Size {
            width: original_size.width() * scale_factor,
            height: original_size.height() * scale_factor,
        };

        // Create a pixmap to render the SVG
        let mut pixmap = Pixmap::new(scaled_size.width as u32, scaled_size.height as u32)?;
        pixmap.fill(tiny_skia::Color::TRANSPARENT);

        // Render the SVG into the pixmap
        let transform = SkiaTransform::from_scale(scale_factor, scale_factor);
        resvg::render(&rtree, transform, &mut pixmap.as_mut());

        // Now create the texture with the rendered pixmap
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas Texture"),
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

        // Handle the texture data upload with proper padding
        let bytes_per_pixel = 4;
        let unpadded_bytes_per_row = pixmap.width() as usize * bytes_per_pixel;
        const COPY_BYTES_PER_ROW_ALIGNMENT: usize = 256;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            / COPY_BYTES_PER_ROW_ALIGNMENT)
            * COPY_BYTES_PER_ROW_ALIGNMENT;

        // Create padded buffer and copy data
        let total_size = padded_bytes_per_row * pixmap.height() as usize;
        let mut padded_buffer = vec![0u8; total_size];

        // Copy rows with padding
        for y in 0..pixmap.height() as usize {
            let dst_start = y * padded_bytes_per_row;
            let src_start = y * unpadded_bytes_per_row;
            padded_buffer[dst_start..dst_start + unpadded_bytes_per_row]
                .copy_from_slice(&pixmap.data()[src_start..src_start + unpadded_bytes_per_row]);
        }

        // Create and upload the buffer
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SVG Pixel Buffer"),
            contents: &padded_buffer,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        // Create command encoder and copy buffer to texture
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
                texture: &texture,
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

        // Submit the transfer command
        queue.submit(std::iter::once(encoder.finish()));

        // Create the texture view
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler for texture filtering
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create the main texture bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("Atlas Bind Group"),
        });

        // Initialize transform uniform and its buffer
        let transform_uniform = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };

        let transform_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Transform Uniform Buffer"),
                contents: bytemuck::bytes_of(&transform_uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Create transform bind group
        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &transform_uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
            label: Some("Transform Bind Group"),
        });

        // Initialize vertex data
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

        // Create vertex buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Atlas Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create index buffer
        let indices: [u16; 6] = [0, 1, 2, 2, 1, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Atlas Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Calculate atlas dimensions and create UV bindings
        let atlas_size = Rectangle {
            x: 0.0,
            y: 0.0,
            width: pixmap.width() as f32,
            height: pixmap.height() as f32,
        };
        let num_tiles = ((atlas_size.width / tile_size.width) as usize)
            * ((atlas_size.height / tile_size.height) as usize);

        // Create UV uniforms and bind groups
        let (uv_uniform_buffer, uv_bind_groups) = Self::create_uv_bindings(
            device,
            num_tiles,
            tile_size,
            atlas_size.size(),
            uv_bind_group_layout,
        );

        let atlas = Self {
            texture_key,
            texture,
            view,
            bind_group,
            transform_uniform,
            transform_uniform_buffer,
            transform_bind_group,
            vertices,
            vertex_buffer,
            index_buffer,
            num_indices: 6,
            dimensions: atlas_size,
            tile_size,
            uv_bind_groups,
            uv_uniform_buffer,
        };

        atlas.update_uv_transforms(queue);

        Some(atlas)
    }

    /// Renders a specific tile from the atlas
    pub fn render_tile<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        render_pipeline: &'a wgpu::RenderPipeline,
        tile_index: usize,
        transform_bind_group: &'a wgpu::BindGroup,
    ) {
        // Use the pre-computed UV bind group for this tile
        if tile_index < self.uv_bind_groups.len() {
            rpass.set_pipeline(render_pipeline);
            rpass.set_bind_group(0, &self.bind_group, &[]); // Texture
            rpass.set_bind_group(1, transform_bind_group, &[]); // Position
            rpass.set_bind_group(2, &self.uv_bind_groups[tile_index], &[]); // Pre-computed UV
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..self.num_indices, 0, 0..1);
        }
    }

    // Helper function for UV bindings creation - implementation remains the same
    fn create_uv_bindings(
        device: &wgpu::Device,
        num_tiles: usize,
        tile_size: Size,
        atlas_size: Size,
        uv_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::Buffer, Vec<wgpu::BindGroup>) {
        // Implementation remains the same as before...
        let alignment = 256;
        let element_size = std::mem::size_of::<UVTransform>();
        let aligned_size = (element_size + alignment - 1) / alignment * alignment;

        let uv_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UV Uniform Buffer"),
            size: (num_tiles * aligned_size) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut uv_bind_groups = Vec::with_capacity(num_tiles);
        let tiles_per_row = (atlas_size.width / tile_size.width) as usize;

        for i in 0..num_tiles {
            let row = i / tiles_per_row;
            let col = i % tiles_per_row;

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: uv_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uv_uniform_buffer,
                        offset: (i * aligned_size) as u64,
                        size: NonZeroU64::new(std::mem::size_of::<UVTransform>() as u64),
                    }),
                }],
                label: Some(&format!("Tile UV Bind Group {}", i)),
            });

            uv_bind_groups.push(bind_group);
        }

        (uv_uniform_buffer, uv_bind_groups)
    }

    pub fn get_transform_uniform(
        &self,
        viewport_size: Size,
        position: Position,
        camera_position: Position,
    ) -> TransformUniform {
        // Use tile_size for the transformation calculations instead of full texture size
        // This ensures each tile is rendered at the correct size regardless of atlas dimensions
        let width_ndc = self.tile_size.width / viewport_size.width;
        let height_ndc = self.tile_size.height / viewport_size.height;

        // Calculate the normalized device coordinates (NDC)
        // NDC space goes from -1 to 1, so we need to transform our coordinates
        let ndc_x = (2.0 * (position.x - camera_position.x)) / viewport_size.width - 1.0;
        let ndc_y = 1.0 - (2.0 * (position.y - camera_position.y)) / viewport_size.height;

        // Create the transformation matrix
        // This is in column-major order as required by WGPU
        // The matrix combines scaling and translation in a single transform
        TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, ndc_x + width_ndc], // First column: x-axis transform
                [0.0, 1.0, 0.0, ndc_y - height_ndc], // Second column: y-axis transform
                [0.0, 0.0, 1.0, 0.0],               // Third column: z-axis transform
                [0.0, 0.0, 0.0, 1.0],               // Fourth column: homogeneous coordinate
            ],
        }
    }

    pub fn update_uv_transforms(&self, queue: &wgpu::Queue) {
        let tiles_per_row = (self.dimensions.width / self.tile_size.width) as usize;
        let tile_uv_width = self.tile_size.width / self.dimensions.width;
        let tile_uv_height = self.tile_size.height / self.dimensions.height;

        // Create a vector to store all UV transforms
        let mut uv_transforms = Vec::with_capacity(self.uv_bind_groups.len());

        for i in 0..self.uv_bind_groups.len() {
            let row = i / tiles_per_row;
            let col = i % tiles_per_row;

            let uv_transform = UVTransform {
                uv_offset: [col as f32 * tile_uv_width, row as f32 * tile_uv_height],
                uv_scale: [tile_uv_width, tile_uv_height],
            };
            uv_transforms.push(uv_transform);
        }

        // Update the UV uniform buffer
        queue.write_buffer(
            &self.uv_uniform_buffer,
            0,
            bytemuck::cast_slice(&uv_transforms),
        );
    }

    pub fn dimensions(&self) -> Rectangle {
        self.dimensions
    }
}
