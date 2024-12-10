/*use crate::utils::*;
use wgpu::util::DeviceExt;

#[derive(Debug)]
pub struct TextureAtlas {
    pub tile_size: Size,
    pub atlas_size: Size,
    uv_bind_groups: Vec<wgpu::BindGroup>,
    uv_bind_group_layout: wgpu::BindGroupLayout,
}

impl TextureAtlas {
    pub fn new(device: &wgpu::Device, tile_size: Size, atlas_size: Size) -> Self {
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

        let num_tiles = (atlas_size.width as usize / tile_size.width as usize)
            * (atlas_size.height as usize / tile_size.height as usize);

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

        let uv_bind_groups = (0..num_tiles)
            .map(|i| {
                let offset = (i * aligned_element_size) as u64;

                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &uv_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uv_uniform_buffer,
                            offset,
                            size: wgpu::BufferSize::new(std::mem::size_of::<UVTransform>() as u64),
                        }),
                    }],
                    label: Some("UV Bind Group"),
                })
            })
            .collect();

        Self {
            tile_size,
            atlas_size,
            uv_bind_groups,
            uv_bind_group_layout,
        }
    }

    pub fn get_uv_bind_group(&self, tile_index: usize) -> Option<&wgpu::BindGroup> {
        self.uv_bind_groups.get(tile_index)
    }

    pub fn uv_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.uv_bind_group_layout
    }
}
*/
