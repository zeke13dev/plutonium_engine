use image::ImageReader;
use plutonium_engine::texture_atlas::TextureAtlas;
use plutonium_engine::texture_svg::TextureSVG;
use plutonium_engine::utils::{InstanceRaw, Position, Size, TransformUniform};
use std::fs;
use std::path::Path;
use wgpu::util::DeviceExt;

fn make_layouts(device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
    let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("texture_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let transform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("transform_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                    plutonium_engine::utils::TransformUniform,
                >() as _),
            },
            count: None,
        }],
    });

    (texture_bgl, transform_bgl)
}

fn build_device() -> (wgpu::Instance, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("no adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            memory_hints: Default::default(),
        },
        None,
    ))
    .expect("device");
    (instance, device, queue)
}

fn compare_with_tolerance(a_path: &Path, b_path: &Path, tolerance: u8) -> bool {
    let a = ImageReader::open(a_path)
        .expect("read A")
        .decode()
        .expect("decode A")
        .to_rgba8();
    let b = ImageReader::open(b_path)
        .expect("read B")
        .decode()
        .expect("decode B")
        .to_rgba8();

    if a.dimensions() != b.dimensions() {
        return false;
    }
    a.pixels().zip(b.pixels()).all(|(pa, pb)| {
        let da = pa.0;
        let db = pb.0;
        (0..4).all(|i| da[i].abs_diff(db[i]) <= tolerance)
    })
}

fn snapshot_map_atlas() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (tex_bgl, xform_bgl) = make_layouts(&device);

    let tile_size = Size {
        width: 512.0,
        height: 512.0,
    };
    let pos = Position { x: 0.0, y: 0.0 };
    let atlas = TextureAtlas::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        "examples/media/map_atlas.svg",
        &tex_bgl,
        &xform_bgl,
        pos,
        tile_size,
    )
    .expect("atlas");

    fs::create_dir_all("snapshots/actual").ok();
    fs::create_dir_all("snapshots/golden").ok();
    let out_actual = Path::new("snapshots/actual/map_atlas.png");
    let out_golden = Path::new("snapshots/golden/map_atlas.png");
    atlas
        .save_debug_png(&device, &queue, out_actual.to_str().unwrap())
        .map_err(|e| anyhow::anyhow!(e))?;

    if !out_golden.exists() {
        fs::copy(out_actual, out_golden)?;
        println!("golden created at {}", out_golden.display());
        return Ok(());
    }

    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        println!("snapshot mismatch for map_atlas.png");
    } else {
        println!("snapshot OK for map_atlas.png");
    }
    Ok(())
}

fn create_shader(device: &wgpu::Device) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("snapshot-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/shader.wgsl").into()),
    });

    let transform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("transform_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    std::mem::size_of::<TransformUniform>() as _
                ),
            },
            count: None,
        }],
    });

    let uv_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("uv_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                    plutonium_engine::utils::UVTransform,
                >() as _),
            },
            count: None,
        }],
    });

    let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("texture_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    // Instance layout for snapshots (group 3)
    let inst_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("snapshot-instance-bgl"),
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

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("snapshot-pipeline-layout"),
        bind_group_layouts: &[&texture_bgl, &transform_bgl, &uv_bgl, &inst_bgl],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("snapshot-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<plutonium_engine::utils::Vertex>()
                    as wgpu::BufferAddress,
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
    (pipeline, inst_bgl)
}

fn save_texture_png(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    path: &Path,
) -> anyhow::Result<()> {
    let size = tex.size();
    let bytes_per_row = ((size.width * 4 + 255) / 256) * 256; // align to 256
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snapshot-output"),
        size: (bytes_per_row as u64) * (size.height as u64),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("snapshot-encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(size.height),
            },
        },
        wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    let slice = output.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).ok();
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let view = slice.get_mapped_range();
    let mut rgba = Vec::with_capacity((size.width * size.height * 4) as usize);
    for row in view.chunks(bytes_per_row as usize) {
        rgba.extend_from_slice(&row[..(size.width * 4) as usize]);
    }
    drop(view);
    output.unmap();
    let img = image::RgbaImage::from_raw(size.width, size.height, rgba).unwrap();
    img.save(path).unwrap();
    Ok(())
}

fn snapshot_checkerboard() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Create offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("checkerboard-target"),
        size: wgpu::Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Bgra8UnormSrgb],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // Build layouts needed to construct TextureAtlas
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let tile_size = Size {
        width: 512.0,
        height: 512.0,
    };
    let atlas = TextureAtlas::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        "examples/media/map_atlas.svg",
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        tile_size,
    )
    .expect("atlas");

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("checker-encoder"),
    });
    let viewport = Size {
        width: 1024.0,
        height: 1024.0,
    };
    let positions = [
        Position { x: 0.0, y: 0.0 },
        Position {
            x: tile_size.width,
            y: 0.0,
        },
        Position {
            x: 0.0,
            y: tile_size.height,
        },
        Position {
            x: tile_size.width,
            y: tile_size.height,
        },
    ];
    let tile_indices = [0usize, 1usize, 1usize, 0usize];

    // Build 4 instances (2x2) with uv for alternating tiles
    let mut instances: Vec<InstanceRaw> = Vec::new();
    for (i, pos) in positions.iter().enumerate() {
        let tf = atlas.get_transform_uniform(viewport, *pos, Position { x: 0.0, y: 0.0 }, 1.0);
        let model = tf.transform;
        let tile = tile_indices[i];
        let uv =
            TextureAtlas::tile_uv_coordinates(tile, tile_size, atlas.dimensions().size()).unwrap();
        instances.push(InstanceRaw {
            model,
            uv_offset: [uv.x, uv.y],
            uv_scale: [uv.width, uv.height],
        });
    }

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("checker-rpass"),
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

        // Create instance buffer and identity world transform
        let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("checker-inst"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: inst_buf.as_entire_binding(),
            }],
            label: Some("checker-inst-bg"),
        });

        let identity = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("checker-id"),
            contents: bytemuck::bytes_of(&identity),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &xform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: id_buf.as_entire_binding(),
            }],
            label: Some("checker-id-bg"),
        });

        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, atlas.texture_bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, atlas.default_uv_bind_group(), &[]);
        rpass.set_bind_group(3, &inst_bg, &[]);
        rpass.set_vertex_buffer(0, atlas.vertex_buffer_slice());
        rpass.set_index_buffer(atlas.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..atlas.num_indices(), 0, 0..(instances.len() as u32));
    }
    queue.submit(Some(encoder.finish()));

    fs::create_dir_all("snapshots/actual").ok();
    fs::create_dir_all("snapshots/golden").ok();
    let out_actual = Path::new("snapshots/actual/checkerboard.png");
    let out_golden = Path::new("snapshots/golden/checkerboard.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    println!(
        "checkerboard snapshot {}",
        if ok { "OK" } else { "MISMATCH" }
    );
    Ok(())
}

fn snapshot_single_sprite() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite-target"),
        size: wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Bgra8UnormSrgb],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // Layouts
    let (tex_bgl, xform_bgl) = make_layouts(&device);

    // Create texture from SVG
    let texture_key = uuid::Uuid::new_v4();
    let texture = TextureSVG::new(
        texture_key,
        &device,
        &queue,
        "examples/media/square.svg",
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("texture svg");

    // Transform for position
    let viewport = Size {
        width: 256.0,
        height: 256.0,
    };
    let pos = Position { x: 50.0, y: 50.0 };
    let tf = texture.get_transform_uniform(viewport, pos, Position { x: 0.0, y: 0.0 }, 0.0);
    let tf_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sprite-tf-ubo"),
        contents: bytemuck::cast_slice(&[tf]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let tf_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: tf_buf.as_entire_binding(),
        }],
        label: Some("sprite-tf-bg"),
    });

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("sprite-encoder"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprite-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.05,
                        b: 0.05,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // Provide a single-instance InstanceRaw with full-UV to match pipeline layout
        let raw = InstanceRaw {
            model: tf.transform,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        };
        let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("snap-single-instance"),
            contents: bytemuck::bytes_of(&raw),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: inst_buf.as_entire_binding(),
            }],
            label: Some("snap-inst-bg"),
        });
        texture.render(&mut rpass, &pipeline, &tf_bg, Some(&inst_bg));
    }
    queue.submit(Some(encoder.finish()));

    // Save and compare
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/sprite.png");
    let out_golden = std::path::Path::new("snapshots/golden/sprite.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    println!("sprite snapshot {}", if ok { "OK" } else { "MISMATCH" });
    Ok(())
}

fn snapshot_many_sprites() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("many-sprites-target"),
        size: wgpu::Extent3d {
            width: 512,
            height: 512,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Bgra8UnormSrgb],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // Layouts needed to construct TextureSVG
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let texture = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        "examples/media/square.svg",
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("texture svg");

    // Build many instance transforms laid out in a grid
    let viewport = Size {
        width: 512.0,
        height: 512.0,
    };
    let mut instances: Vec<InstanceRaw> = Vec::new();
    let cols = 16u32;
    let rows = 16u32; // 256 sprites
    let spacing = 24.0f32;
    for r in 0..rows {
        for c in 0..cols {
            let pos = Position {
                x: 16.0 + c as f32 * spacing,
                y: 16.0 + r as f32 * spacing,
            };
            let tf = texture.get_transform_uniform(viewport, pos, Position { x: 0.0, y: 0.0 }, 0.0);
            instances.push(InstanceRaw {
                model: tf.transform,
                uv_offset: [0.0, 0.0],
                uv_scale: [1.0, 1.0],
            });
        }
    }

    // Instance buffer with per-instance data
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("many-sprites-instance-buffer"),
        contents: bytemuck::cast_slice(&instances),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let instance_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: instance_buffer.as_entire_binding(),
        }],
        label: Some("many-sprites-instance-bg"),
    });

    // Identity world transform for group(1)
    let identity = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let tf_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("many-sprites-id-ubo"),
        contents: bytemuck::bytes_of(&identity),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let tf_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: tf_buf.as_entire_binding(),
        }],
        label: Some("many-sprites-id-bg"),
    });

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("many-sprites-encoder"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("many-sprites-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.02,
                        b: 0.02,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, texture.bind_group(), &[]);
        rpass.set_bind_group(1, &tf_bg, &[]);
        rpass.set_bind_group(2, texture.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &instance_bg, &[]);
        rpass.set_vertex_buffer(0, texture.vertex_buffer_slice());
        rpass.set_index_buffer(texture.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..texture.num_indices(), 0, 0..(instances.len() as u32));
    }
    queue.submit(Some(encoder.finish()));

    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/many_sprites.png");
    let out_golden = std::path::Path::new("snapshots/golden/many_sprites.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    println!(
        "many_sprites snapshot {}",
        if ok { "OK" } else { "MISMATCH" }
    );
    Ok(())
}

fn main() -> anyhow::Result<()> {
    snapshot_map_atlas()?;
    snapshot_checkerboard()?;
    snapshot_single_sprite()?;
    snapshot_many_sprites()?;
    Ok(())
}
