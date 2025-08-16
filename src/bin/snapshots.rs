use image::ImageReader;
use plutonium_engine::texture_atlas::TextureAtlas;
use plutonium_engine::texture_svg::TextureSVG;
use plutonium_engine::utils::{InstanceRaw, Position, Size, TransformUniform};
use std::fs;
use std::path::Path;
use wgpu::util::DeviceExt;

#[cfg(feature = "anim")]
use plutonium_engine::anim::{Ease, Timeline, Track, Tween};

fn asset(name: &str) -> String {
    format!("{}/examples/media/{}", env!("CARGO_MANIFEST_DIR"), name)
}
// Duplicate minimal types to avoid snapshot bin depending on game crate directly
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct FrameInputRecordLocal {
    pressed_keys: Vec<String>,
    mouse_x: f32,
    mouse_y: f32,
    lmb_down: bool,
    committed_text: Vec<String>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct ReplayScriptLocal {
    frames: Vec<FrameInputRecordLocal>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct RecordingMetaLocal {
    seed: Option<u64>,
    window_w: u32,
    window_h: u32,
    dt: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct RecordingFileLocal {
    meta: RecordingMetaLocal,
    frames: Vec<FrameInputRecordLocal>,
}

type ParseResult = (
    Option<u64>,
    Option<String>,
    Option<String>,
    Option<usize>,
    Option<f32>,
);

fn parse_args() -> ParseResult {
    let mut seed: Option<u64> = None;
    let mut record: Option<String> = None;
    let mut replay: Option<String> = None;
    let mut frames: Option<usize> = None;
    let mut dt: Option<f32> = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--seed" => {
                if let Some(s) = it.next() {
                    seed = s.parse::<u64>().ok();
                }
            }
            // Record a minimal script of N frames to <path>.json (defaults to 3 if --frames unspecified)
            "--record" => {
                if let Some(p) = it.next() {
                    record = Some(p);
                }
            }
            // Replay a script at <path>.json and render a verification scene
            "--replay" => {
                if let Some(p) = it.next() {
                    replay = Some(p);
                }
            }
            "--frames" => {
                if let Some(n) = it.next() {
                    frames = n.parse::<usize>().ok();
                }
            }
            "--dt" => {
                if let Some(v) = it.next() {
                    dt = v.parse::<f32>().ok();
                }
            }
            _ => {}
        }
    }
    (seed, record, replay, frames, dt)
}

fn record_minimal_script(
    path: &str,
    frames: usize,
    seed: Option<u64>,
    dt: f32,
) -> anyhow::Result<()> {
    let rec = FrameInputRecordLocal {
        pressed_keys: vec!["Enter".into()],
        mouse_x: 10.0,
        mouse_y: 10.0,
        lmb_down: false,
        committed_text: vec![],
    };
    let file = RecordingFileLocal {
        meta: RecordingMetaLocal {
            seed,
            window_w: 256,
            window_h: 256,
            dt,
        },
        frames: vec![rec; frames],
    };
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(path, serde_json::to_string_pretty(&file)?)?;
    Ok(())
}

fn replay_scene_from(path: &str) -> anyhow::Result<()> {
    let json = std::fs::read_to_string(path)?;
    // Try new format first, fallback to old
    let (frames, _meta_dt): (Vec<FrameInputRecordLocal>, f32) =
        if let Ok(file) = serde_json::from_str::<RecordingFileLocal>(&json) {
            (file.frames, file.meta.dt)
        } else if let Ok(old) = serde_json::from_str::<ReplayScriptLocal>(&json) {
            (old.frames, 0.2)
        } else {
            (Vec::new(), 0.2)
        };
    // Simulate simple state from replay frames
    let mut pos = Position { x: 20.0, y: 20.0 };
    let mut tile_index: usize = 0;
    for f in &frames {
        if f.pressed_keys.iter().any(|k| k == "Enter") {
            tile_index ^= 1;
        }
        if f.lmb_down {
            pos.x += 5.0;
        }
    }
    // Build device and pipeline
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);
    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("replay-scene-target"),
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
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let tile_size = Size {
        width: 512.0,
        height: 512.0,
    };
    let atlas = TextureAtlas::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("map_atlas.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        tile_size,
    )
    .expect("atlas");
    let viewport = Size {
        width: 256.0,
        height: 256.0,
    };
    let tf = atlas.get_transform_uniform(viewport, pos, Position { x: 0.0, y: 0.0 }, 1.0, 1.0);
    let uv = TextureAtlas::tile_uv_coordinates(tile_index, tile_size, atlas.dimensions().size())
        .unwrap_or_else(|| plutonium_engine::utils::Rectangle::new(0.0, 0.0, 1.0, 1.0));
    let raw = InstanceRaw {
        model: tf.transform,
        uv_offset: [uv.x, uv.y],
        uv_scale: [uv.width, uv.height],
    };
    let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("replay-scene-inst2"),
        contents: bytemuck::bytes_of(&raw),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: inst_buf.as_entire_binding(),
        }],
        label: Some("replay-scene-inst-bg2"),
    });
    let id = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("replay-scene-id2"),
        contents: bytemuck::bytes_of(&id),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: id_buf.as_entire_binding(),
        }],
        label: Some("replay-scene-id-bg2"),
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("replay-scene-enc2"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("replay-scene-rpass2"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.04,
                        g: 0.04,
                        b: 0.06,
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
        rpass.set_bind_group(0, atlas.texture_bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, atlas.default_uv_bind_group(), &[]);
        rpass.set_bind_group(3, &inst_bg, &[]);
        rpass.set_vertex_buffer(0, atlas.vertex_buffer_slice());
        rpass.set_index_buffer(atlas.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..atlas.num_indices(), 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    fs::create_dir_all("snapshots/actual").ok();
    fs::create_dir_all("snapshots/golden").ok();
    let out_actual = Path::new("snapshots/actual/replay_scene.png");
    let out_golden = Path::new("snapshots/golden/replay_scene.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "replay_scene") {
            println!("replay_scene snapshot MISMATCH");
        } else {
            println!("replay_scene snapshot OK");
        }
    } else {
        println!("replay_scene snapshot OK");
    }
    Ok(())
}

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
    // Try to get a native adapter first, then fall back to a software adapter
    let mut adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }));
    if adapter.is_none() {
        adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: true,
            compatible_surface: None,
        }));
    }
    let adapter = adapter.expect("no adapter");
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

fn can_acquire_adapter() -> bool {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: true,
        compatible_surface: None,
    }));
    adapter.is_some()
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

fn maybe_update_golden(actual: &Path, golden: &Path, label: &str) -> bool {
    if std::env::var("UPDATE_SNAPSHOTS").ok().as_deref() == Some("1") {
        if let Err(e) = std::fs::copy(actual, golden) {
            eprintln!("failed to update golden for {label}: {e}");
            false
        } else {
            println!("{label} golden updated");
            true
        }
    } else {
        false
    }
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
        &asset("map_atlas.svg"),
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
        if !maybe_update_golden(out_actual, out_golden, "map_atlas") {
            println!("snapshot mismatch for map_atlas.png");
        } else {
            println!("snapshot OK for map_atlas.png");
        }
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

// Rect SDF pipeline for UI primitives in snapshots
fn create_rect_pipeline(
    device: &wgpu::Device,
    transform_bgl: &wgpu::BindGroupLayout,
    inst_bgl: &wgpu::BindGroupLayout,
) -> (
    wgpu::RenderPipeline,
    wgpu::BindGroupLayout,
    wgpu::BindGroup,
    wgpu::Buffer,
    wgpu::Buffer,
) {
    let dummy_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rect-dummy-bgl"),
        entries: &[],
    });
    let dummy_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rect-dummy-bg"),
        layout: &dummy_bgl,
        entries: &[],
    });
    let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rect-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/rect.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rect-pipeline-layout"),
        bind_group_layouts: &[&dummy_bgl, transform_bgl, &dummy_bgl, inst_bgl],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("rect-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &rect_shader,
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
    let (vbuf, ibuf) = plutonium_engine::utils::create_centered_quad_buffers(device);
    (pipeline, dummy_bgl, dummy_bg, vbuf, ibuf)
}

fn snapshot_toggle_states() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (_pipeline, inst_bgl) = create_shader(&device);
    let (_tex_bgl, xform_bgl) = make_layouts(&device);
    let (rect_pipeline, _rect_dummy_bgl, rect_dummy_bg, rect_vbuf, rect_ibuf) =
        create_rect_pipeline(&device, &xform_bgl, &inst_bgl);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("toggle-states-target"),
        size: wgpu::Extent3d {
            width: 480,
            height: 200,
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

    // Background clear via pass
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("toggle-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("toggle-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Draw off + focused and on + unfocused toggles using rect SDF directly
        let view_size = plutonium_engine::utils::Size {
            width: 480.0,
            height: 200.0,
        };

        // Helper to draw a track+thumb
        let mut draw_toggle_rect = |top_left: (f32, f32), on: bool, focused: bool| {
            let track =
                plutonium_engine::utils::Rectangle::new(top_left.0, top_left.1, 200.0, 40.0);
            // Focus ring first
            if focused {
                let ring_rect = plutonium_engine::utils::Rectangle::new(
                    track.x - 4.0,
                    track.y - 4.0,
                    track.width + 8.0,
                    track.height + 8.0,
                );
                let ring_model = rect_model_for(view_size, ring_rect);
                let ring_inst = plutonium_engine::utils::RectInstanceRaw {
                    model: ring_model,
                    color: [0.0, 0.0, 0.0, 0.0],
                    corner_radius_px: 22.0,
                    border_thickness_px: 2.0,
                    _pad0: [0.0, 0.0],
                    border_color: [1.0, 0.9, 0.2, 1.0],
                    rect_size_px: [ring_rect.width, ring_rect.height],
                    _pad1: [0.0, 0.0],
                    _pad2: [0.0, 0.0, 0.0, 0.0],
                };
                let ring_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("toggle-ring"),
                    contents: bytemuck::bytes_of(&ring_inst),
                    usage: wgpu::BufferUsages::STORAGE,
                });
                let ring_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &inst_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ring_buf.as_entire_binding(),
                    }],
                    label: Some("toggle-ring-bg"),
                });

                // Identity world
                let id = plutonium_engine::utils::TransformUniform {
                    transform: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                };
                let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("toggle-id"),
                    contents: bytemuck::bytes_of(&id),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &xform_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: id_buf.as_entire_binding(),
                    }],
                    label: Some("toggle-id-bg"),
                });

                rpass.set_pipeline(&rect_pipeline);
                rpass.set_bind_group(0, &rect_dummy_bg, &[]);
                rpass.set_bind_group(1, &id_bg, &[]);
                rpass.set_bind_group(2, &rect_dummy_bg, &[]);
                rpass.set_bind_group(3, &ring_bg, &[]);
                rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
                rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..6, 0, 0..1);
            }

            // Track
            let track_col = if on {
                [0.25, 0.55, 0.35, 1.0]
            } else {
                [0.25, 0.27, 0.32, 1.0]
            };
            let track_model = rect_model_for(view_size, track);
            let track_inst = plutonium_engine::utils::RectInstanceRaw {
                model: track_model,
                color: track_col,
                corner_radius_px: 20.0,
                border_thickness_px: 1.0,
                _pad0: [0.0, 0.0],
                border_color: [0.15, 0.17, 0.22, 1.0],
                rect_size_px: [track.width, track.height],
                _pad1: [0.0, 0.0],
                _pad2: [0.0, 0.0, 0.0, 0.0],
            };
            let track_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("toggle-track"),
                contents: bytemuck::bytes_of(&track_inst),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let track_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &inst_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: track_buf.as_entire_binding(),
                }],
                label: Some("toggle-track-bg"),
            });

            // Identity world
            let id = plutonium_engine::utils::TransformUniform {
                transform: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            };
            let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("toggle-id2"),
                contents: bytemuck::bytes_of(&id),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &xform_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: id_buf.as_entire_binding(),
                }],
                label: Some("toggle-id-bg2"),
            });

            rpass.set_pipeline(&rect_pipeline);
            rpass.set_bind_group(0, &rect_dummy_bg, &[]);
            rpass.set_bind_group(1, &id_bg, &[]);
            rpass.set_bind_group(2, &rect_dummy_bg, &[]);
            rpass.set_bind_group(3, &track_bg, &[]);
            rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
            rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..6, 0, 0..1);

            // Thumb
            let pad = 2.0f32;
            let d = (track.height - 2.0 * pad).max(0.0);
            let cx = if on {
                track.x + track.width - track.height + pad
            } else {
                track.x + pad
            };
            let thumb = plutonium_engine::utils::Rectangle::new(cx, track.y + pad, d, d);
            let thumb_model = rect_model_for(view_size, thumb);
            let thumb_inst = plutonium_engine::utils::RectInstanceRaw {
                model: thumb_model,
                color: [0.95, 0.95, 0.98, 1.0],
                corner_radius_px: d * 0.5,
                border_thickness_px: 1.0,
                _pad0: [0.0, 0.0],
                border_color: [0.15, 0.17, 0.22, 1.0],
                rect_size_px: [d, d],
                _pad1: [0.0, 0.0],
                _pad2: [0.0, 0.0, 0.0, 0.0],
            };
            let thumb_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("toggle-thumb"),
                contents: bytemuck::bytes_of(&thumb_inst),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let thumb_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &inst_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: thumb_buf.as_entire_binding(),
                }],
                label: Some("toggle-thumb-bg"),
            });

            rpass.set_pipeline(&rect_pipeline);
            rpass.set_bind_group(0, &rect_dummy_bg, &[]);
            rpass.set_bind_group(1, &id_bg, &[]);
            rpass.set_bind_group(2, &rect_dummy_bg, &[]);
            rpass.set_bind_group(3, &thumb_bg, &[]);
            rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
            rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..6, 0, 0..1);
        };

        draw_toggle_rect((40.0, 40.0), false, true); // off, focused
        draw_toggle_rect((240.0, 120.0), true, false); // on, unfocused
    }

    queue.submit(Some(encoder.finish()));
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/toggle_states.png");
    let out_golden = std::path::Path::new("snapshots/golden/toggle_states.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 4);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "toggle_states") {
            println!("toggle_states snapshot MISMATCH");
        } else {
            println!("toggle_states snapshot OK");
        }
    } else {
        println!("toggle_states snapshot OK");
    }
    Ok(())
}

fn rect_model_for(
    view: plutonium_engine::utils::Size,
    rect: plutonium_engine::utils::Rectangle,
) -> [[f32; 4]; 4] {
    let width_ndc = rect.width / view.width;
    let height_ndc = rect.height / view.height;
    let ndc_dx = 2.0 * (rect.x / view.width) - 1.0;
    let ndc_dy = 1.0 - 2.0 * (rect.y / view.height);
    let ndc_x = ndc_dx + width_ndc;
    let ndc_y = ndc_dy - height_ndc;
    [
        [width_ndc, 0.0, 0.0, 0.0],
        [0.0, height_ndc, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [ndc_x, ndc_y, 0.0, 1.0],
    ]
}

fn save_texture_png(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    path: &Path,
) -> anyhow::Result<()> {
    let size = tex.size();
    let bytes_per_row = (size.width * 4).div_ceil(256) * 256; // align to 256
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
        &asset("map_atlas.svg"),
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
        let tf = atlas.get_transform_uniform(viewport, *pos, Position { x: 0.0, y: 0.0 }, 1.0, 1.0);
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
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "checkerboard") {
            println!("checkerboard snapshot MISMATCH");
        } else {
            println!("checkerboard snapshot OK");
        }
    } else {
        println!("checkerboard snapshot OK");
    }
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
        &asset("square.svg"),
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
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "sprite") {
            println!("sprite snapshot MISMATCH");
        } else {
            println!("sprite snapshot OK");
        }
    } else {
        println!("sprite snapshot OK");
    }
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
        &asset("square.svg"),
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
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "many_sprites") {
            println!("many_sprites snapshot MISMATCH");
        } else {
            println!("many_sprites snapshot OK");
        }
    } else {
        println!("many_sprites snapshot OK");
    }
    Ok(())
}

fn snapshot_demo_player() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("demo-player-target"),
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

    // Create texture from SVG (player)
    let texture = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("player.svg"),
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
    let pos = Position { x: 80.0, y: 80.0 };
    let tf = texture.get_transform_uniform(viewport, pos, Position { x: 0.0, y: 0.0 }, 0.0);

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("demo-player-encoder"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("demo-player-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.05,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Instance buffer with single instance (full UVs)
        let raw = InstanceRaw {
            model: tf.transform,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        };
        let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("demo-player-inst"),
            contents: bytemuck::bytes_of(&raw),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: inst_buf.as_entire_binding(),
            }],
            label: Some("demo-player-inst-bg"),
        });

        // Identity world transform BG for group 1
        let identity = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("demo-player-id"),
            contents: bytemuck::bytes_of(&identity),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &xform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: id_buf.as_entire_binding(),
            }],
            label: Some("demo-player-id-bg"),
        });

        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, texture.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, texture.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &inst_bg, &[]);
        rpass.set_vertex_buffer(0, texture.vertex_buffer_slice());
        rpass.set_index_buffer(texture.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..texture.num_indices(), 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    // Save and compare
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/demo_player.png");
    let out_golden = std::path::Path::new("snapshots/golden/demo_player.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "demo_player") {
            println!("demo_player snapshot MISMATCH");
        } else {
            println!("demo_player snapshot OK");
        }
    } else {
        println!("demo_player snapshot OK");
    }
    Ok(())
}

fn snapshot_menu_ui() -> anyhow::Result<()> {
    // Render a button background + label using engine immediate-mode path
    // Create a headless surface-sized texture via a dummy surface is non-trivial; instead reuse offscreen pattern
    let (_i, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("menu-ui-target"),
        size: wgpu::Extent3d {
            width: 320,
            height: 120,
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

    // Build layouts
    let (tex_bgl, xform_bgl) = make_layouts(&device);

    // Load background texture
    let bg = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("bg");
    // Also load font to queue text (skipped here; separate snapshot covers text)

    // Identity transform for world
    let identity = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("menu-id"),
        contents: bytemuck::bytes_of(&identity),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: id_buf.as_entire_binding(),
        }],
        label: Some("menu-id-bg"),
    });

    // Transform for placing bg at 20,60
    let viewport = Size {
        width: 320.0,
        height: 120.0,
    };
    let pos = Position { x: 20.0, y: 60.0 };
    let tf = bg.get_transform_uniform(viewport, pos, Position { x: 0.0, y: 0.0 }, 0.0);
    let inst_raw = InstanceRaw {
        model: tf.transform,
        uv_offset: [0.0, 0.0],
        uv_scale: [1.0, 1.0],
    };
    let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("menu-inst"),
        contents: bytemuck::bytes_of(&inst_raw),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: inst_buf.as_entire_binding(),
        }],
        label: Some("menu-inst-bg"),
    });

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("menu-ui-encoder"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("menu-ui-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
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
        rpass.set_bind_group(0, bg.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, bg.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &inst_bg, &[]);
        rpass.set_vertex_buffer(0, bg.vertex_buffer_slice());
        rpass.set_index_buffer(bg.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..bg.num_indices(), 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    // Save and compare
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/menu_button.png");
    let out_golden = std::path::Path::new("snapshots/golden/menu_button.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "menu_button") {
            println!("menu_button snapshot MISMATCH");
        } else {
            println!("menu_button snapshot OK");
        }
    } else {
        println!("menu_button snapshot OK");
    }
    Ok(())
}

fn snapshot_menu_ui_text() -> anyhow::Result<()> {
    use plutonium_engine::text::TextRenderer;
    use rusttype::{Font, Scale};
    use std::fs::read;

    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("menu-ui-text-target"),
        size: wgpu::Extent3d {
            width: 320,
            height: 120,
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
    // Note: using atlas path; unit quad helper also available via utils::create_unit_quad_buffers if needed

    // Load font and build atlas texture
    let font_data = read(asset("roboto.ttf"))?;
    let font = Font::try_from_vec(font_data).ok_or_else(|| anyhow::anyhow!("font"))?;
    let scale = Scale::uniform(16.0);
    let padding = 2;
    let (atlas_w, atlas_h, char_dims, max_w, max_h) =
        TextRenderer::calculate_atlas_size(&font, scale, padding);
    let (tex_rgba, char_map) =
        TextRenderer::render_glyphs_to_atlas(&font, scale, (atlas_w, atlas_h), &char_dims, padding)
            .ok_or_else(|| anyhow::anyhow!("atlas"))?;

    let texture_size = wgpu::Extent3d {
        width: atlas_w,
        height: atlas_h,
        depth_or_array_layers: 1,
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("font atlas tex"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &tex_rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * atlas_w),
            rows_per_image: Some(atlas_h),
        },
        texture_size,
    );
    let tex_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let texture_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&tex_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
        label: Some("font-atlas-bg"),
    });

    // Wrap in TextureAtlas to reuse buffers and uv helpers
    let atlas = plutonium_engine::texture_atlas::TextureAtlas::new_from_texture(
        uuid::Uuid::new_v4(),
        tex,
        texture_bg,
        Position { x: 0.0, y: 0.0 },
        Size {
            width: atlas_w as f32,
            height: atlas_h as f32,
        },
        Size {
            width: max_w as f32,
            height: max_h as f32,
        },
        &device,
        &queue,
        &xform_bgl,
        &char_map,
    )
    .expect("wrap font atlas");

    // Identity transform for world (group 1)
    let identity = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("menu-text-id"),
        contents: bytemuck::bytes_of(&identity),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: id_buf.as_entire_binding(),
        }],
        label: Some("menu-text-id-bg"),
    });

    // Build instances for a sample text
    let viewport = Size {
        width: 320.0,
        height: 120.0,
    };
    let text = "Hello";
    let mut instances: Vec<InstanceRaw> = Vec::new();
    let mut pen_x: f32 = 20.0;
    let baseline_y: f32 = 40.0;
    for c in text.chars() {
        if let Some(info) = char_map.get(&c) {
            // model matrix via atlas helper
            let tf = atlas.get_transform_uniform(
                viewport,
                Position {
                    x: pen_x + info.bearing.0,
                    y: baseline_y - info.bearing.1,
                },
                Position { x: 0.0, y: 0.0 },
                1.0,
                1.0,
            );
            // per-char UV rectangle from tile index
            if let Some(uv) = plutonium_engine::texture_atlas::TextureAtlas::tile_uv_coordinates(
                info.tile_index,
                Size {
                    width: max_w as f32,
                    height: max_h as f32,
                },
                Size {
                    width: atlas_w as f32,
                    height: atlas_h as f32,
                },
            ) {
                instances.push(InstanceRaw {
                    model: tf.transform,
                    uv_offset: [uv.x, uv.y],
                    uv_scale: [uv.width, uv.height],
                });
            }
            pen_x += info.advance_width;
        }
    }
    let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("menu-text-instances"),
        contents: bytemuck::cast_slice(&instances),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: inst_buf.as_entire_binding(),
        }],
        label: Some("menu-text-inst-bg"),
    });

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("menu-text-encoder"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("menu-text-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
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
        rpass.set_bind_group(0, atlas.texture_bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, atlas.default_uv_bind_group(), &[]);
        rpass.set_bind_group(3, &inst_bg, &[]);
        rpass.set_vertex_buffer(0, atlas.vertex_buffer_slice());
        rpass.set_index_buffer(atlas.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..atlas.num_indices(), 0, 0..(instances.len() as u32));
    }
    queue.submit(Some(encoder.finish()));

    // Save and compare
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/menu_text.png");
    let out_golden = std::path::Path::new("snapshots/golden/menu_text.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 5);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "menu_text") {
            println!("menu_text snapshot MISMATCH");
        } else {
            println!("menu_text snapshot OK");
        }
    } else {
        println!("menu_text snapshot OK");
    }
    Ok(())
}

fn snapshot_menu_panel() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("menu-panel-target"),
        size: wgpu::Extent3d {
            width: 256,
            height: 128,
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

    // Build layouts
    let (tex_bgl, xform_bgl) = make_layouts(&device);

    // Create a dummy 3x3 atlas by slicing square.svg according to tile size
    let atlas = plutonium_engine::texture_atlas::TextureAtlas::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        Size {
            width: 64.0,
            height: 64.0,
        },
    )
    .expect("atlas");

    // Render several tiles to form a panel
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("panel-encoder"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("panel-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // Instance buffer/group (single instance) to satisfy pipeline bind group 3
        let raw = plutonium_engine::utils::InstanceRaw {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        };
        let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("panel-inst"),
            contents: bytemuck::bytes_of(&raw),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: inst_buf.as_entire_binding(),
            }],
            label: Some("panel-inst-bg"),
        });
        // Corners positions; using tile index 0 for all due to demo texture
        let positions = [
            (0usize, Position { x: 16.0, y: 16.0 }),
            (0usize, Position { x: 176.0, y: 16.0 }),
            (0usize, Position { x: 16.0, y: 80.0 }),
            (0usize, Position { x: 176.0, y: 80.0 }),
        ];
        let mut tf_bgs: Vec<wgpu::BindGroup> = Vec::new();
        let mut tf_bufs: Vec<wgpu::Buffer> = Vec::new();
        for (_, pos) in &positions {
            let tf = atlas.get_transform_uniform(
                Size {
                    width: 256.0,
                    height: 128.0,
                },
                *pos,
                Position { x: 0.0, y: 0.0 },
                1.0,
                1.0,
            );
            let tf_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("panel-tf"),
                contents: bytemuck::bytes_of(&tf),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let tf_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &xform_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: tf_buf.as_entire_binding(),
                }],
                label: Some("panel-tf-bg"),
            });
            tf_bufs.push(tf_buf);
            tf_bgs.push(tf_bg);
        }
        for (i, (tile, _)) in positions.iter().enumerate() {
            atlas.render_tile(&mut rpass, &pipeline, *tile, &tf_bgs[i], Some(&inst_bg));
        }
    }
    queue.submit(Some(encoder.finish()));

    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/menu_panel.png");
    let out_golden = std::path::Path::new("snapshots/golden/menu_panel.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 5);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "menu_panel") {
            println!("menu_panel snapshot MISMATCH");
        } else {
            println!("menu_panel snapshot OK");
        }
    } else {
        println!("menu_panel snapshot OK");
    }
    Ok(())
}

fn snapshot_menu_button_focused() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("menu-button-focused-target"),
        size: wgpu::Extent3d {
            width: 320,
            height: 120,
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

    // Button base
    let btn = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 20.0, y: 60.0 },
        1.0,
    )
    .expect("btn");

    // Build rect pipeline for crisp focus ring
    let (rect_pipeline, _rect_dummy_bgl, rect_dummy_bg, rect_vbuf, rect_ibuf) =
        create_rect_pipeline(&device, &xform_bgl, &inst_bgl);

    // Identity world transform (group 1)
    let id = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("btnf-id"),
        contents: bytemuck::bytes_of(&id),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: id_buf.as_entire_binding(),
        }],
        label: Some("btnf-id-bg"),
    });

    // Instances for button and ring
    let viewport = Size {
        width: 320.0,
        height: 120.0,
    };
    let btn_tf = btn.get_transform_uniform(
        viewport,
        Position { x: 20.0, y: 60.0 },
        Position { x: 0.0, y: 0.0 },
        0.0,
    );
    let btn_raw = InstanceRaw {
        model: btn_tf.transform,
        uv_offset: [0.0, 0.0],
        uv_scale: [1.0, 1.0],
    };
    let btn_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("btnf-btn"),
        contents: bytemuck::bytes_of(&btn_raw),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let btn_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: btn_buf.as_entire_binding(),
        }],
        label: Some("btnf-btn-bg"),
    });

    // (removed old sprite-based ring)

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("btnf-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("btnf-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // Draw button
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, btn.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, btn.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &btn_bg, &[]);
        rpass.set_vertex_buffer(0, btn.vertex_buffer_slice());
        rpass.set_index_buffer(btn.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..btn.num_indices(), 0, 0..1);
        // Draw crisp focus ring using rect SDF
        let dims = btn.dimensions();
        let ring_inset = 2.0f32;
        let ring_rect = plutonium_engine::utils::Rectangle::new(
            dims.x - ring_inset,
            dims.y - ring_inset,
            dims.width + ring_inset * 2.0,
            dims.height + ring_inset * 2.0,
        );
        let viewport = plutonium_engine::utils::Size {
            width: 320.0,
            height: 120.0,
        };
        let ring_model = rect_model_for(viewport, ring_rect);
        let ring_inst = plutonium_engine::utils::RectInstanceRaw {
            model: ring_model,
            color: [0.0, 0.0, 0.0, 0.0],
            corner_radius_px: 10.0 + 2.0,
            border_thickness_px: 3.0,
            _pad0: [0.0, 0.0],
            border_color: [1.0, 0.9, 0.2, 1.0],
            rect_size_px: [ring_rect.width, ring_rect.height],
            _pad1: [0.0, 0.0],
            _pad2: [0.0, 0.0, 0.0, 0.0],
        };
        let ring_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("btnf-ring-rect"),
            contents: bytemuck::bytes_of(&ring_inst),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let ring_bg_rect = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ring_buf.as_entire_binding(),
            }],
            label: Some("btnf-ring-rect-bg"),
        });
        rpass.set_pipeline(&rect_pipeline);
        rpass.set_bind_group(0, &rect_dummy_bg, &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, &rect_dummy_bg, &[]);
        rpass.set_bind_group(3, &ring_bg_rect, &[]);
        rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
        rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..6, 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/menu_button_focused.png");
    let out_golden = std::path::Path::new("snapshots/golden/menu_button_focused.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "menu_button_focused") {
            println!("menu_button_focused snapshot MISMATCH");
        } else {
            println!("menu_button_focused snapshot OK");
        }
    } else {
        println!("menu_button_focused snapshot OK");
    }
    Ok(())
}

fn snapshot_menu_button_hovered() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (sprite_pipeline, inst_bgl) = create_shader(&device);
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let (rect_pipeline, _rect_dummy_bgl, rect_dummy_bg, rect_vbuf, rect_ibuf) =
        create_rect_pipeline(&device, &xform_bgl, &inst_bgl);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("menu-button-hovered-target"),
        size: wgpu::Extent3d {
            width: 320,
            height: 120,
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

    // Base button sprite
    let btn = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 20.0, y: 60.0 },
        1.0,
    )
    .expect("btn");

    // Identity world
    let id = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("btnh-id"),
        contents: bytemuck::bytes_of(&id),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: id_buf.as_entire_binding(),
        }],
        label: Some("btnh-id-bg"),
    });

    // Instance for button
    let viewport = Size {
        width: 320.0,
        height: 120.0,
    };
    let btn_tf = btn.get_transform_uniform(
        viewport,
        Position { x: 20.0, y: 60.0 },
        Position { x: 0.0, y: 0.0 },
        0.0,
    );
    let btn_raw = InstanceRaw {
        model: btn_tf.transform,
        uv_offset: [0.0, 0.0],
        uv_scale: [1.0, 1.0],
    };
    let btn_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("btnh-btn"),
        contents: bytemuck::bytes_of(&btn_raw),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let btn_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: btn_buf.as_entire_binding(),
        }],
        label: Some("btnh-btn-bg"),
    });

    // Hover overlay rect (lighten)
    let dims = btn.dimensions();
    let hover_model = rect_model_for(viewport, dims);
    let hover_inst = plutonium_engine::utils::RectInstanceRaw {
        model: hover_model,
        color: [1.0, 1.0, 1.0, 0.06],
        corner_radius_px: 10.0,
        border_thickness_px: 1.0,
        _pad0: [0.0, 0.0],
        border_color: [1.0, 1.0, 1.0, 0.12],
        rect_size_px: [dims.width, dims.height],
        _pad1: [0.0, 0.0],
        _pad2: [0.0, 0.0, 0.0, 0.0],
    };
    let hover_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("btnh-hover"),
        contents: bytemuck::bytes_of(&hover_inst),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let hover_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: hover_buf.as_entire_binding(),
        }],
        label: Some("btnh-hover-bg"),
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("btnh-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("btnh-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // Base button
        rpass.set_pipeline(&sprite_pipeline);
        rpass.set_bind_group(0, btn.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, btn.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &btn_bg, &[]);
        rpass.set_vertex_buffer(0, btn.vertex_buffer_slice());
        rpass.set_index_buffer(btn.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..btn.num_indices(), 0, 0..1);
        // Hover overlay
        rpass.set_pipeline(&rect_pipeline);
        rpass.set_bind_group(0, &rect_dummy_bg, &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, &rect_dummy_bg, &[]);
        rpass.set_bind_group(3, &hover_bg, &[]);
        rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
        rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..6, 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/menu_button_hovered.png");
    let out_golden = std::path::Path::new("snapshots/golden/menu_button_hovered.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "menu_button_hovered") {
            println!("menu_button_hovered snapshot MISMATCH");
        } else {
            println!("menu_button_hovered snapshot OK");
        }
    } else {
        println!("menu_button_hovered snapshot OK");
    }
    Ok(())
}

fn snapshot_menu_button_pressed() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (sprite_pipeline, inst_bgl) = create_shader(&device);
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let (rect_pipeline, _rect_dummy_bgl, rect_dummy_bg, rect_vbuf, rect_ibuf) =
        create_rect_pipeline(&device, &xform_bgl, &inst_bgl);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("menu-button-pressed-target"),
        size: wgpu::Extent3d {
            width: 320,
            height: 120,
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
    let btn = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 20.0, y: 62.0 },
        1.0,
    )
    .expect("btn");
    // Identity world
    let id = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("btnp-id"),
        contents: bytemuck::bytes_of(&id),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: id_buf.as_entire_binding(),
        }],
        label: Some("btnp-id-bg"),
    });
    // Instances
    let viewport = Size {
        width: 320.0,
        height: 120.0,
    };
    let btn_tf = btn.get_transform_uniform(
        viewport,
        Position { x: 20.0, y: 62.0 },
        Position { x: 0.0, y: 0.0 },
        0.0,
    );
    let btn_raw = InstanceRaw {
        model: btn_tf.transform,
        uv_offset: [0.0, 0.0],
        uv_scale: [1.0, 1.0],
    };
    let btn_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("btnp-btn"),
        contents: bytemuck::bytes_of(&btn_raw),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let btn_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: btn_buf.as_entire_binding(),
        }],
        label: Some("btnp-btn-bg"),
    });

    // Darken overlay
    let dims = btn.dimensions();
    let press_model = rect_model_for(viewport, dims);
    let press_inst = plutonium_engine::utils::RectInstanceRaw {
        model: press_model,
        color: [0.0, 0.0, 0.0, 0.12],
        corner_radius_px: 10.0,
        border_thickness_px: 0.0,
        _pad0: [0.0, 0.0],
        border_color: [0.0, 0.0, 0.0, 0.0],
        rect_size_px: [dims.width, dims.height],
        _pad1: [0.0, 0.0],
        _pad2: [0.0, 0.0, 0.0, 0.0],
    };
    let press_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("btnp-press"),
        contents: bytemuck::bytes_of(&press_inst),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let press_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: press_buf.as_entire_binding(),
        }],
        label: Some("btnp-press-bg"),
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("btnp-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("btnp-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // Base button (offset for pressed)
        rpass.set_pipeline(&sprite_pipeline);
        rpass.set_bind_group(0, btn.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, btn.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &btn_bg, &[]);
        rpass.set_vertex_buffer(0, btn.vertex_buffer_slice());
        rpass.set_index_buffer(btn.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..btn.num_indices(), 0, 0..1);
        // Darken overlay
        rpass.set_pipeline(&rect_pipeline);
        rpass.set_bind_group(0, &rect_dummy_bg, &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, &rect_dummy_bg, &[]);
        rpass.set_bind_group(3, &press_bg, &[]);
        rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
        rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..6, 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/menu_button_pressed.png");
    let out_golden = std::path::Path::new("snapshots/golden/menu_button_pressed.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "menu_button_pressed") {
            println!("menu_button_pressed snapshot MISMATCH");
        } else {
            println!("menu_button_pressed snapshot OK");
        }
    } else {
        println!("menu_button_pressed snapshot OK");
    }
    Ok(())
}
fn snapshot_slider_states() -> anyhow::Result<()> {
    // Render slider visuals with rect SDF pipeline: track, fill, thumb, focus ring
    let (_instance, device, queue) = build_device();
    let (_sprite_pipeline, inst_bgl) = create_shader(&device);
    let (_tex_bgl, xform_bgl) = make_layouts(&device);
    let (rect_pipeline, _dummy_bgl, dummy_bg, rect_vbuf, rect_ibuf) =
        create_rect_pipeline(&device, &xform_bgl, &inst_bgl);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("slider-states-target"),
        size: wgpu::Extent3d {
            width: 480,
            height: 180,
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
    // Identity world transform (group 1)
    let identity = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("slider-id"),
        contents: bytemuck::bytes_of(&identity),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: id_buf.as_entire_binding(),
        }],
        label: Some("slider-id-bg"),
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("slider-rect-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("slider-rect-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let viewport = plutonium_engine::utils::Size {
            width: 480.0,
            height: 180.0,
        };
        let origin = plutonium_engine::utils::Position { x: 40.0, y: 70.0 };
        let track_w = 360.0f32;
        let track_h = 10.0f32;
        let thumb_w = 20.0f32;
        let thumb_h = 28.0f32;
        let value = 0.35f32;
        let corner = 6.0f32;
        // Track
        let track_rect =
            plutonium_engine::utils::Rectangle::new(origin.x, origin.y, track_w, track_h);
        let track_model = rect_model_for(viewport, track_rect);
        let track_inst = plutonium_engine::utils::RectInstanceRaw {
            model: track_model,
            color: [0.18, 0.20, 0.24, 1.0],
            corner_radius_px: track_h * 0.5,
            border_thickness_px: 0.0,
            _pad0: [0.0, 0.0],
            border_color: [0.0, 0.0, 0.0, 0.0],
            rect_size_px: [track_w, track_h],
            _pad1: [0.0, 0.0],
            _pad2: [0.0, 0.0, 0.0, 0.0],
        };
        let track_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("slider-track"),
            contents: bytemuck::bytes_of(&track_inst),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let track_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: track_buf.as_entire_binding(),
            }],
            label: Some("slider-track-bg"),
        });
        rpass.set_pipeline(&rect_pipeline);
        rpass.set_bind_group(0, &dummy_bg, &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, &dummy_bg, &[]);
        rpass.set_bind_group(3, &track_bg, &[]);
        rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
        rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..6, 0, 0..1);

        // Filled portion
        let thumb_center_x = origin.x + value * (track_w - thumb_w) + thumb_w * 0.5;
        let filled_w = (thumb_center_x - origin.x).clamp(0.0, track_w);
        if filled_w > 0.0 {
            let fill_rect =
                plutonium_engine::utils::Rectangle::new(origin.x, origin.y, filled_w, track_h);
            let fill_model = rect_model_for(viewport, fill_rect);
            let fill_inst = plutonium_engine::utils::RectInstanceRaw {
                model: fill_model,
                color: [0.36, 0.56, 0.98, 1.0],
                corner_radius_px: track_h * 0.5,
                border_thickness_px: 0.0,
                _pad0: [0.0, 0.0],
                border_color: [0.0, 0.0, 0.0, 0.0],
                rect_size_px: [filled_w, track_h],
                _pad1: [0.0, 0.0],
                _pad2: [0.0, 0.0, 0.0, 0.0],
            };
            let fill_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("slider-fill"),
                contents: bytemuck::bytes_of(&fill_inst),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let fill_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &inst_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: fill_buf.as_entire_binding(),
                }],
                label: Some("slider-fill-bg"),
            });
            rpass.set_bind_group(3, &fill_bg, &[]);
            rpass.draw_indexed(0..6, 0, 0..1);
        }

        // Thumb
        let thumb_x = origin.x + value * (track_w - thumb_w);
        let thumb_y = origin.y + track_h * 0.5 - thumb_h * 0.5;
        let thumb_rect = plutonium_engine::utils::Rectangle::new(
            thumb_x.floor(),
            thumb_y.floor(),
            thumb_w,
            thumb_h,
        );
        let thumb_model = rect_model_for(viewport, thumb_rect);
        let thumb_inst = plutonium_engine::utils::RectInstanceRaw {
            model: thumb_model,
            color: [0.92, 0.94, 0.96, 1.0],
            corner_radius_px: corner,
            border_thickness_px: 1.0,
            _pad0: [0.0, 0.0],
            border_color: [0.12, 0.14, 0.18, 1.0],
            rect_size_px: [thumb_w, thumb_h],
            _pad1: [0.0, 0.0],
            _pad2: [0.0, 0.0, 0.0, 0.0],
        };
        let thumb_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("slider-thumb"),
            contents: bytemuck::bytes_of(&thumb_inst),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let thumb_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: thumb_buf.as_entire_binding(),
            }],
            label: Some("slider-thumb-bg"),
        });
        rpass.set_bind_group(3, &thumb_bg, &[]);
        rpass.draw_indexed(0..6, 0, 0..1);

        // Focus ring around thumb
        let ring_inset = 2.0f32;
        let ring_rect = plutonium_engine::utils::Rectangle::new(
            thumb_rect.x - ring_inset,
            thumb_rect.y - ring_inset,
            thumb_rect.width + ring_inset * 2.0,
            thumb_rect.height + ring_inset * 2.0,
        );
        let ring_model = rect_model_for(viewport, ring_rect);
        let ring_inst = plutonium_engine::utils::RectInstanceRaw {
            model: ring_model,
            color: [0.0, 0.0, 0.0, 0.0],
            corner_radius_px: corner + 2.0,
            border_thickness_px: 3.0,
            _pad0: [0.0, 0.0],
            border_color: [1.0, 0.85, 0.30, 1.0],
            rect_size_px: [ring_rect.width, ring_rect.height],
            _pad1: [0.0, 0.0],
            _pad2: [0.0, 0.0, 0.0, 0.0],
        };
        let ring_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("slider-ring"),
            contents: bytemuck::bytes_of(&ring_inst),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let ring_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ring_buf.as_entire_binding(),
            }],
            label: Some("slider-ring-bg"),
        });
        rpass.set_bind_group(3, &ring_bg, &[]);
        rpass.draw_indexed(0..6, 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/slider_states.png");
    let out_golden = std::path::Path::new("snapshots/golden/slider_states.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 5);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "slider_states") {
            println!("slider_states snapshot MISMATCH");
        } else {
            println!("slider_states snapshot OK");
        }
    } else {
        println!("slider_states snapshot OK");
    }
    Ok(())
}

// Validate button visual helper by drawing default, hovered, pressed, and focused states
fn snapshot_button_states() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (_sprite_pipeline, inst_bgl) = create_shader(&device);
    let (_tex_bgl, xform_bgl) = make_layouts(&device);
    let (rect_pipeline, _dummy_bgl, rect_dummy_bg, rect_vbuf, rect_ibuf) =
        create_rect_pipeline(&device, &xform_bgl, &inst_bgl);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("button-states-target"),
        size: wgpu::Extent3d {
            width: 640,
            height: 240,
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
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("button-states-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("button-states-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let viewport = plutonium_engine::utils::Size {
            width: 640.0,
            height: 240.0,
        };
        // Helper to draw a button via rect SDFs similar to draw_button_background
        let mut draw_btn = |x: f32, y: f32, hovered: bool, pressed: bool, focused: bool| {
            let rect = plutonium_engine::utils::Rectangle::new(x, y, 160.0, 56.0);
            // Optional focus ring
            if focused {
                let fr = plutonium_engine::utils::Rectangle::new(
                    rect.x - 4.0,
                    rect.y - 4.0,
                    rect.width + 8.0,
                    rect.height + 8.0,
                );
                let model = rect_model_for(viewport, fr);
                let inst = plutonium_engine::utils::RectInstanceRaw {
                    model,
                    color: [0.0, 0.0, 0.0, 0.0],
                    corner_radius_px: 12.0,
                    border_thickness_px: 2.0,
                    _pad0: [0.0, 0.0],
                    border_color: [1.0, 0.9, 0.2, 1.0],
                    rect_size_px: [fr.width, fr.height],
                    _pad1: [0.0, 0.0],
                    _pad2: [0.0, 0.0, 0.0, 0.0],
                };
                let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("btns-fr"),
                    contents: bytemuck::bytes_of(&inst),
                    usage: wgpu::BufferUsages::STORAGE,
                });
                let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &inst_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    }],
                    label: Some("btns-fr-bg"),
                });
                let id = plutonium_engine::utils::TransformUniform {
                    transform: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                };
                let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("btns-id"),
                    contents: bytemuck::bytes_of(&id),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &xform_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: id_buf.as_entire_binding(),
                    }],
                    label: Some("btns-id-bg"),
                });
                rpass.set_pipeline(&rect_pipeline);
                rpass.set_bind_group(0, &rect_dummy_bg, &[]);
                rpass.set_bind_group(1, &id_bg, &[]);
                rpass.set_bind_group(2, &rect_dummy_bg, &[]);
                rpass.set_bind_group(3, &bg, &[]);
                rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
                rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..6, 0, 0..1);
            }
            // Base button
            let base = rect_model_for(viewport, rect);
            let base_inst = plutonium_engine::utils::RectInstanceRaw {
                model: base,
                color: [0.20, 0.22, 0.28, 1.0],
                corner_radius_px: 10.0,
                border_thickness_px: 1.0,
                _pad0: [0.0, 0.0],
                border_color: [0.14, 0.16, 0.20, 1.0],
                rect_size_px: [rect.width, rect.height],
                _pad1: [0.0, 0.0],
                _pad2: [0.0, 0.0, 0.0, 0.0],
            };
            let base_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("btns-base"),
                contents: bytemuck::bytes_of(&base_inst),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let base_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &inst_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: base_buf.as_entire_binding(),
                }],
                label: Some("btns-base-bg"),
            });
            let id = plutonium_engine::utils::TransformUniform {
                transform: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            };
            let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("btns-id2"),
                contents: bytemuck::bytes_of(&id),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &xform_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: id_buf.as_entire_binding(),
                }],
                label: Some("btns-id2-bg"),
            });
            rpass.set_pipeline(&rect_pipeline);
            rpass.set_bind_group(0, &rect_dummy_bg, &[]);
            rpass.set_bind_group(1, &id_bg, &[]);
            rpass.set_bind_group(2, &rect_dummy_bg, &[]);
            rpass.set_bind_group(3, &base_bg, &[]);
            rpass.set_vertex_buffer(0, rect_vbuf.slice(..));
            rpass.set_index_buffer(rect_ibuf.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..6, 0, 0..1);
            // Overlay
            if pressed {
                let over_inst = plutonium_engine::utils::RectInstanceRaw {
                    model: base,
                    color: [0.0, 0.0, 0.0, 0.12],
                    corner_radius_px: 10.0,
                    border_thickness_px: 0.0,
                    _pad0: [0.0, 0.0],
                    border_color: [0.0, 0.0, 0.0, 0.0],
                    rect_size_px: [rect.width, rect.height],
                    _pad1: [0.0, 0.0],
                    _pad2: [0.0, 0.0, 0.0, 0.0],
                };
                let obuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("btns-press"),
                    contents: bytemuck::bytes_of(&over_inst),
                    usage: wgpu::BufferUsages::STORAGE,
                });
                let obg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &inst_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: obuf.as_entire_binding(),
                    }],
                    label: Some("btns-press-bg"),
                });
                rpass.set_bind_group(3, &obg, &[]);
                rpass.draw_indexed(0..6, 0, 0..1);
            } else if hovered {
                let over_inst = plutonium_engine::utils::RectInstanceRaw {
                    model: base,
                    color: [1.0, 1.0, 1.0, 0.06],
                    corner_radius_px: 10.0,
                    border_thickness_px: 1.0,
                    _pad0: [0.0, 0.0],
                    border_color: [1.0, 1.0, 1.0, 0.12],
                    rect_size_px: [rect.width, rect.height],
                    _pad1: [0.0, 0.0],
                    _pad2: [0.0, 0.0, 0.0, 0.0],
                };
                let obuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("btns-hover"),
                    contents: bytemuck::bytes_of(&over_inst),
                    usage: wgpu::BufferUsages::STORAGE,
                });
                let obg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &inst_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: obuf.as_entire_binding(),
                    }],
                    label: Some("btns-hover-bg"),
                });
                rpass.set_bind_group(3, &obg, &[]);
                rpass.draw_indexed(0..6, 0, 0..1);
            }
        };
        draw_btn(40.0, 92.0, false, false, false);
        draw_btn(220.0, 92.0, true, false, false);
        draw_btn(400.0, 92.0, false, true, false);
        draw_btn(580.0, 92.0, false, false, true);
    }
    queue.submit(Some(encoder.finish()));
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/button_states.png");
    let out_golden = std::path::Path::new("snapshots/golden/button_states.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 4);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "button_states") {
            println!("button_states snapshot MISMATCH");
        } else {
            println!("button_states snapshot OK");
        }
    } else {
        println!("button_states snapshot OK");
    }
    Ok(())
}
// Replay-driven minimal snapshot: for now this validates that a replay script can be read and applied.
fn snapshot_replay_driven() -> anyhow::Result<()> {
    let script_path = std::path::Path::new("snapshots/replays/minimal.json");
    if !script_path.exists() {
        // create a tiny script
        std::fs::create_dir_all(script_path.parent().unwrap()).ok();
        let rec = FrameInputRecordLocal {
            pressed_keys: vec!["Enter".into()],
            mouse_x: 10.0,
            mouse_y: 10.0,
            lmb_down: false,
            committed_text: vec![],
        };
        let script = ReplayScriptLocal {
            frames: vec![rec; 3],
        };
        std::fs::write(script_path, serde_json::to_string_pretty(&script)?)?;
    }
    let json = std::fs::read_to_string(script_path)?;
    let script: ReplayScriptLocal = serde_json::from_str(&json)?;
    assert!(!script.frames.is_empty());
    println!("replay script frames: {}", script.frames.len());
    Ok(())
}

fn snapshot_transitions() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("transitions-target"),
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

    // Background texture
    let bg = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("map_atlas.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("bg");

    // Foreground overlay: square.svg tinted via separate pass by drawing two sprites with different alpha
    let fg = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("fg");

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("trans-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("trans-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.03,
                        g: 0.04,
                        b: 0.06,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // Draw background tile at origin
        let viewport = Size {
            width: 256.0,
            height: 256.0,
        };
        let bg_tf = bg.get_transform_uniform(
            viewport,
            Position { x: 0.0, y: 0.0 },
            Position { x: 0.0, y: 0.0 },
            0.0,
        );
        let bg_raw = InstanceRaw {
            model: bg_tf.transform,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        };
        let bg_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trans-bg"),
            contents: bytemuck::bytes_of(&bg_raw),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let bg_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bg_buf.as_entire_binding(),
            }],
            label: Some("trans-bg-bg"),
        });
        let id = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trans-id"),
            contents: bytemuck::bytes_of(&id),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &xform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: id_buf.as_entire_binding(),
            }],
            label: Some("trans-id-bg"),
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, bg.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, bg.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &bg_bg, &[]);
        rpass.set_vertex_buffer(0, bg.vertex_buffer_slice());
        rpass.set_index_buffer(bg.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..bg.num_indices(), 0, 0..1);

        // Draw overlay faded and slid to the right-bottom slightly
        let fg_tf = fg.get_transform_uniform(
            viewport,
            Position { x: 20.0, y: 20.0 },
            Position { x: 0.0, y: 0.0 },
            0.0,
        );
        let fg_raw = InstanceRaw {
            model: fg_tf.transform,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        };
        let fg_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trans-fg"),
            contents: bytemuck::bytes_of(&fg_raw),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let fg_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: fg_buf.as_entire_binding(),
            }],
            label: Some("trans-fg-bg"),
        });
        // Reuse id_bg
        rpass.set_bind_group(0, fg.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, fg.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &fg_bg, &[]);
        rpass.set_vertex_buffer(0, fg.vertex_buffer_slice());
        rpass.set_index_buffer(fg.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..fg.num_indices(), 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/transitions.png");
    let out_golden = std::path::Path::new("snapshots/golden/transitions.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "transitions") {
            println!("transitions snapshot MISMATCH");
        } else {
            println!("transitions snapshot OK");
        }
    } else {
        println!("transitions snapshot OK");
    }
    Ok(())
}

fn snapshot_transitions_frame2() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("transitions2-target"),
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
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let bg = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("map_atlas.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("bg");
    let fg = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("fg");

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("trans2-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("trans2-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.03,
                        g: 0.04,
                        b: 0.06,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let viewport = Size {
            width: 256.0,
            height: 256.0,
        };
        // draw background
        let bg_tf = bg.get_transform_uniform(
            viewport,
            Position { x: 0.0, y: 0.0 },
            Position { x: 0.0, y: 0.0 },
            0.0,
        );
        let bg_raw = InstanceRaw {
            model: bg_tf.transform,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        };
        let bg_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trans2-bg"),
            contents: bytemuck::bytes_of(&bg_raw),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let inst_bgl2 = &inst_bgl;
        let bg_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: inst_bgl2,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bg_buf.as_entire_binding(),
            }],
            label: Some("trans2-bg-bg"),
        });
        let id = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trans2-id"),
            contents: bytemuck::bytes_of(&id),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &xform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: id_buf.as_entire_binding(),
            }],
            label: Some("trans2-id-bg"),
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, bg.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, bg.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &bg_bg, &[]);
        rpass.set_vertex_buffer(0, bg.vertex_buffer_slice());
        rpass.set_index_buffer(bg.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..bg.num_indices(), 0, 0..1);
        // draw overlay further slid (simulate later frame)
        let fg_tf = fg.get_transform_uniform(
            viewport,
            Position { x: 60.0, y: 60.0 },
            Position { x: 0.0, y: 0.0 },
            0.0,
        );
        let fg_raw = InstanceRaw {
            model: fg_tf.transform,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        };
        let fg_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trans2-fg"),
            contents: bytemuck::bytes_of(&fg_raw),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let fg_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: fg_buf.as_entire_binding(),
            }],
            label: Some("trans2-fg-bg"),
        });
        rpass.set_bind_group(0, fg.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, fg.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &fg_bg, &[]);
        rpass.set_vertex_buffer(0, fg.vertex_buffer_slice());
        rpass.set_index_buffer(fg.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..fg.num_indices(), 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));
    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/transitions_frame2.png");
    let out_golden = std::path::Path::new("snapshots/golden/transitions_frame2.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "transitions_frame2") {
            println!("transitions_frame2 snapshot MISMATCH");
        } else {
            println!("transitions_frame2 snapshot OK");
        }
    } else {
        println!("transitions_frame2 snapshot OK");
    }
    Ok(())
}
fn snapshot_deal_grid() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("deal-grid-target"),
        size: wgpu::Extent3d {
            width: 300,
            height: 200,
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
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let card = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("card");

    // Build simple grid positions (3x2)
    let positions: Vec<(f32, f32)> = {
        let mut v = Vec::new();
        for r in 0..2 {
            for c in 0..3 {
                v.push((20.0 + c as f32 * 40.0, 20.0 + r as f32 * 60.0));
            }
        }
        v
    };

    let viewport = Size {
        width: 300.0,
        height: 200.0,
    };
    // Build instance buffer of all cards
    let mut raws: Vec<InstanceRaw> = Vec::new();
    for (x, y) in positions {
        let tf = card.get_transform_uniform(
            viewport,
            Position { x, y },
            Position { x: 0.0, y: 0.0 },
            0.0,
        );
        raws.push(InstanceRaw {
            model: tf.transform,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        });
    }
    let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("deal-inst"),
        contents: bytemuck::cast_slice(&raws),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: inst_buf.as_entire_binding(),
        }],
        label: Some("deal-inst-bg"),
    });
    let id = TransformUniform {
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("deal-id"),
        contents: bytemuck::bytes_of(&id),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &xform_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: id_buf.as_entire_binding(),
        }],
        label: Some("deal-id-bg"),
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("deal-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("deal-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.08,
                        g: 0.08,
                        b: 0.1,
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
        rpass.set_bind_group(0, card.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, card.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &inst_bg, &[]);
        rpass.set_vertex_buffer(0, card.vertex_buffer_slice());
        rpass.set_index_buffer(card.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..card.num_indices(), 0, 0..(raws.len() as u32));
    }
    queue.submit(Some(encoder.finish()));

    std::fs::create_dir_all("snapshots/actual").ok();
    std::fs::create_dir_all("snapshots/golden").ok();
    let out_actual = std::path::Path::new("snapshots/actual/deal_grid.png");
    let out_golden = std::path::Path::new("snapshots/golden/deal_grid.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        std::fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "deal_grid") {
            println!("deal_grid snapshot MISMATCH");
        } else {
            println!("deal_grid snapshot OK");
        }
    } else {
        println!("deal_grid snapshot OK");
    }
    Ok(())
}

#[cfg(feature = "anim")]
fn snapshot_timeline_anim() -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("timeline-target"),
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

    // Sprite
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let sprite = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("sprite");

    // Build a simple position timeline and step a fixed dt
    let mut tl: Timeline<Position> = Timeline::new();
    tl.push_track(Track::Sequence(vec![Tween::new(
        Position { x: 20.0, y: 20.0 },
        Position { x: 180.0, y: 140.0 },
        0.5,
        Ease::EaseInOut,
    )]));
    // Step twice by 0.25s to reach mid/late point
    let _ = tl.step(0.25);
    let out = tl.step(0.25);
    let mut pos = Position { x: 20.0, y: 20.0 };
    if let Some(track_vals) = out.first() {
        if let Some(&v) = track_vals.last() {
            pos = v;
        }
    }

    // Render sprite at computed position
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("timeline-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("timeline-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.05,
                        b: 0.06,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let viewport = Size {
            width: 256.0,
            height: 256.0,
        };
        let tf = sprite.get_transform_uniform(viewport, pos, Position { x: 0.0, y: 0.0 }, 0.0);
        let raw = InstanceRaw {
            model: tf.transform,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
        };
        let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("timeline-inst"),
            contents: bytemuck::bytes_of(&raw),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &inst_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: inst_buf.as_entire_binding(),
            }],
            label: Some("timeline-inst-bg"),
        });
        let id = TransformUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("timeline-id"),
            contents: bytemuck::bytes_of(&id),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &xform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: id_buf.as_entire_binding(),
            }],
            label: Some("timeline-id-bg"),
        });

        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, sprite.bind_group(), &[]);
        rpass.set_bind_group(1, &id_bg, &[]);
        rpass.set_bind_group(2, sprite.uv_bind_group(), &[]);
        rpass.set_bind_group(3, &inst_bg, &[]);
        rpass.set_vertex_buffer(0, sprite.vertex_buffer_slice());
        rpass.set_index_buffer(sprite.index_buffer_slice(), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..sprite.num_indices(), 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    fs::create_dir_all("snapshots/actual").ok();
    fs::create_dir_all("snapshots/golden").ok();
    let out_actual = Path::new("snapshots/actual/timeline_frame.png");
    let out_golden = Path::new("snapshots/golden/timeline_frame.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "timeline_frame") {
            println!("timeline_frame snapshot MISMATCH");
        } else {
            println!("timeline_frame snapshot OK");
        }
    } else {
        println!("timeline_frame snapshot OK");
    }
    Ok(())
}

#[cfg(feature = "anim")]
fn snapshot_timeline_anim_multiframe(frames: usize, frame_dt: f32) -> anyhow::Result<()> {
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("timeline-mf-target"),
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

    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let sprite = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("sprite");

    let mut tl: Timeline<Position> = Timeline::new();
    tl.push_track(Track::Sequence(vec![Tween::new(
        Position { x: 20.0, y: 20.0 },
        Position { x: 220.0, y: 180.0 },
        0.6,
        Ease::EaseInOut,
    )]));

    let viewport = Size {
        width: 256.0,
        height: 256.0,
    };
    for i in 0..frames {
        if i == 0 {
            let _ = tl.step(0.0);
        } else {
            let _ = tl.step(frame_dt);
        }
        let out = tl.step(0.0);
        let mut pos = Position { x: 20.0, y: 20.0 };
        if let Some(track_vals) = out.first() {
            if let Some(&v) = track_vals.last() {
                pos = v;
            }
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("timeline-mf-enc"),
        });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("timeline-mf-rpass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.06,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let tf = sprite.get_transform_uniform(viewport, pos, Position { x: 0.0, y: 0.0 }, 0.0);
            let raw = InstanceRaw {
                model: tf.transform,
                uv_offset: [0.0, 0.0],
                uv_scale: [1.0, 1.0],
            };
            let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("timeline-mf-inst"),
                contents: bytemuck::bytes_of(&raw),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &inst_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: inst_buf.as_entire_binding(),
                }],
                label: Some("timeline-mf-inst-bg"),
            });
            let id = TransformUniform {
                transform: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            };
            let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("timeline-mf-id"),
                contents: bytemuck::bytes_of(&id),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &xform_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: id_buf.as_entire_binding(),
                }],
                label: Some("timeline-mf-id-bg"),
            });

            rpass.set_pipeline(&pipeline);
            rpass.set_bind_group(0, sprite.bind_group(), &[]);
            rpass.set_bind_group(1, &id_bg, &[]);
            rpass.set_bind_group(2, sprite.uv_bind_group(), &[]);
            rpass.set_bind_group(3, &inst_bg, &[]);
            rpass.set_vertex_buffer(0, sprite.vertex_buffer_slice());
            rpass.set_index_buffer(sprite.index_buffer_slice(), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..sprite.num_indices(), 0, 0..1);
        }
        queue.submit(Some(encoder.finish()));

        fs::create_dir_all("snapshots/actual").ok();
        fs::create_dir_all("snapshots/golden").ok();
        let actual_str = format!("snapshots/actual/timeline_frame{}.png", i);
        let golden_str = format!("snapshots/golden/timeline_frame{}.png", i);
        let out_actual_path = Path::new(&actual_str);
        let out_golden_path = Path::new(&golden_str);
        save_texture_png(&device, &queue, &target, out_actual_path)?;
        if !out_golden_path.exists() {
            fs::copy(out_actual_path, out_golden_path)?;
        }
        let ok = compare_with_tolerance(out_actual_path, out_golden_path, 3);
        let label = format!("timeline_frame{}", i);
        if !ok {
            if !maybe_update_golden(out_actual_path, out_golden_path, &label) {
                println!("{} snapshot MISMATCH", label);
            } else {
                println!("{} snapshot OK", label);
            }
        } else {
            println!("{} snapshot OK", label);
        }
    }
    Ok(())
}

fn snapshot_rng_pattern(seed: u64) -> anyhow::Result<()> {
    use plutonium_engine::rng::RngService;
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rng-pattern-target"),
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
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let spr = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("spr");

    let svc = RngService::with_seed(seed);
    let mut rng = svc.derive_stream(100);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("rngp-enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rngp-rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.02,
                        b: 0.03,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let viewport = Size {
            width: 256.0,
            height: 256.0,
        };
        // Place 25 sprites in pseudo-random positions
        for _ in 0..25 {
            let px = rng.range_f32(8.0, 220.0);
            let py = rng.range_f32(8.0, 220.0);
            let tf = spr.get_transform_uniform(
                viewport,
                Position { x: px, y: py },
                Position { x: 0.0, y: 0.0 },
                0.0,
            );
            let raw = InstanceRaw {
                model: tf.transform,
                uv_offset: [0.0, 0.0],
                uv_scale: [1.0, 1.0],
            };
            let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rngp-inst"),
                contents: bytemuck::bytes_of(&raw),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &inst_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: inst_buf.as_entire_binding(),
                }],
                label: Some("rngp-inst-bg"),
            });
            let id = TransformUniform {
                transform: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            };
            let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rngp-id"),
                contents: bytemuck::bytes_of(&id),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &xform_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: id_buf.as_entire_binding(),
                }],
                label: Some("rngp-id-bg"),
            });
            rpass.set_pipeline(&pipeline);
            rpass.set_bind_group(0, spr.bind_group(), &[]);
            rpass.set_bind_group(1, &id_bg, &[]);
            rpass.set_bind_group(2, spr.uv_bind_group(), &[]);
            rpass.set_bind_group(3, &inst_bg, &[]);
            rpass.set_vertex_buffer(0, spr.vertex_buffer_slice());
            rpass.set_index_buffer(spr.index_buffer_slice(), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..spr.num_indices(), 0, 0..1);
        }
    }
    queue.submit(Some(encoder.finish()));

    fs::create_dir_all("snapshots/actual").ok();
    fs::create_dir_all("snapshots/golden").ok();
    let out_actual = Path::new("snapshots/actual/rng_pattern.png");
    let out_golden = Path::new("snapshots/golden/rng_pattern.png");
    save_texture_png(&device, &queue, &target, out_actual)?;
    if !out_golden.exists() {
        fs::copy(out_actual, out_golden)?;
    }
    let ok = compare_with_tolerance(out_actual, out_golden, 3);
    if !ok {
        if !maybe_update_golden(out_actual, out_golden, "rng_pattern") {
            println!("rng_pattern snapshot MISMATCH");
        } else {
            println!("rng_pattern snapshot OK");
        }
    } else {
        println!("rng_pattern snapshot OK");
    }
    Ok(())
}

fn snapshot_deal_grid_anim_multiframe(
    seed: u64,
    frames: usize,
    frame_dt: f32,
) -> anyhow::Result<()> {
    use plutonium_engine::rng::RngService;
    let (_instance, device, queue) = build_device();
    let (pipeline, inst_bgl) = create_shader(&device);

    // Offscreen target
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("deal-grid-anim-target"),
        size: wgpu::Extent3d {
            width: 300,
            height: 200,
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
    let (tex_bgl, xform_bgl) = make_layouts(&device);
    let card = TextureSVG::new(
        uuid::Uuid::new_v4(),
        &device,
        &queue,
        &asset("square.svg"),
        &tex_bgl,
        &xform_bgl,
        Position { x: 0.0, y: 0.0 },
        1.0,
    )
    .expect("card");

    // Build grid target positions
    let cols = 3usize;
    let rows = 2usize;
    let start = (20.0f32, 20.0f32);
    let dx = 40.0f32;
    let dy = 60.0f32;
    let mut targets: Vec<(f32, f32)> = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            targets.push((start.0 + c as f32 * dx, start.1 + r as f32 * dy));
        }
    }

    // RNG order and delays
    let svc = RngService::with_seed(seed);
    let mut rng = svc.derive_stream(77);
    let mut order: Vec<usize> = (0..targets.len()).collect();
    rng.shuffle(&mut order);
    #[derive(Clone, Copy)]
    struct Deal {
        delay: f32,
        dur: f32,
        to: (f32, f32),
    }
    let deck_pos = (8.0f32, 8.0f32);
    let mut deals: Vec<Deal> = Vec::new();
    for (i, idx) in order.iter().enumerate() {
        let delay = 0.10f32 * i as f32 + rng.range_f32(0.0, 0.06);
        let dur = 0.32f32 + rng.range_f32(0.0, 0.20);
        deals.push(Deal {
            delay,
            dur,
            to: targets[*idx],
        });
    }

    let viewport = Size {
        width: 300.0,
        height: 200.0,
    };
    for fi in 0..frames {
        let t = frame_dt * (fi as f32);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("deal-anim-enc"),
        });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("deal-anim-rpass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.08,
                            b: 0.10,
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
            // Identity world
            let id = TransformUniform {
                transform: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            };
            let id_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("deal-anim-id"),
                contents: bytemuck::bytes_of(&id),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let id_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &xform_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: id_buf.as_entire_binding(),
                }],
                label: Some("deal-anim-id-bg"),
            });
            rpass.set_bind_group(1, &id_bg, &[]);
            // Draw each card at interpolated position
            for d in &deals {
                let pos = if t <= d.delay {
                    Position {
                        x: deck_pos.0,
                        y: deck_pos.1,
                    }
                } else if t >= d.delay + d.dur {
                    Position {
                        x: d.to.0,
                        y: d.to.1,
                    }
                } else {
                    let k = (t - d.delay) / d.dur;
                    let x = deck_pos.0 + (d.to.0 - deck_pos.0) * k;
                    let y = deck_pos.1 + (d.to.1 - deck_pos.1) * k;
                    Position { x, y }
                };
                let tf =
                    card.get_transform_uniform(viewport, pos, Position { x: 0.0, y: 0.0 }, 0.0);
                let raw = InstanceRaw {
                    model: tf.transform,
                    uv_offset: [0.0, 0.0],
                    uv_scale: [1.0, 1.0],
                };
                let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("deal-anim-inst"),
                    contents: bytemuck::bytes_of(&raw),
                    usage: wgpu::BufferUsages::STORAGE,
                });
                let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &inst_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: inst_buf.as_entire_binding(),
                    }],
                    label: Some("deal-anim-inst-bg"),
                });
                rpass.set_bind_group(0, card.bind_group(), &[]);
                rpass.set_bind_group(2, card.uv_bind_group(), &[]);
                rpass.set_bind_group(3, &inst_bg, &[]);
                rpass.set_vertex_buffer(0, card.vertex_buffer_slice());
                rpass.set_index_buffer(card.index_buffer_slice(), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..card.num_indices(), 0, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));
        fs::create_dir_all("snapshots/actual").ok();
        fs::create_dir_all("snapshots/golden").ok();
        let actual_str = format!("snapshots/actual/deal_grid_anim{}.png", fi);
        let golden_str = format!("snapshots/golden/deal_grid_anim{}.png", fi);
        let out_actual_path = Path::new(&actual_str);
        let out_golden_path = Path::new(&golden_str);
        save_texture_png(&device, &queue, &target, out_actual_path)?;
        if !out_golden_path.exists() {
            fs::copy(out_actual_path, out_golden_path)?;
        }
        let ok = compare_with_tolerance(out_actual_path, out_golden_path, 4);
        let label = format!("deal_grid_anim{}", fi);
        if !ok {
            if !maybe_update_golden(out_actual_path, out_golden_path, &label) {
                println!("{} snapshot MISMATCH", label);
            } else {
                println!("{} snapshot OK", label);
            }
        } else {
            println!("{} snapshot OK", label);
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    if !can_acquire_adapter() {
        eprintln!("no wgpu adapter available; skipping snapshots");
        return Ok(());
    }
    let (seed_opt, record_opt, replay_opt, frames_opt, dt_opt) = parse_args();
    if let Some(seed) = seed_opt {
        println!("seed={}", seed);
    }
    if let Some(rec) = record_opt.as_ref() {
        let n = frames_opt.unwrap_or(3);
        let dt = dt_opt.unwrap_or(0.2);
        let _ = record_minimal_script(rec, n, seed_opt, dt);
        println!("recorded {} frames to {}", n, rec);
    }
    if let Some(rep) = replay_opt.as_ref() {
        let _ = replay_scene_from(rep);
        println!("replayed from {}", rep);
    }

    let mf_frames = frames_opt.unwrap_or(3);
    let mf_dt = dt_opt.unwrap_or(0.2);

    snapshot_map_atlas()?;
    snapshot_checkerboard()?;
    snapshot_single_sprite()?;
    snapshot_many_sprites()?;
    snapshot_demo_player()?;
    snapshot_menu_ui()?;
    let _ = snapshot_menu_ui_text();
    let _ = snapshot_menu_panel();
    let _ = snapshot_menu_button_focused();
    let _ = snapshot_menu_button_hovered();
    let _ = snapshot_menu_button_pressed();
    let _ = snapshot_slider_states();
    let _ = snapshot_button_states();
    #[cfg(feature = "anim")]
    let _ = snapshot_timeline_anim();
    #[cfg(feature = "anim")]
    let _ = snapshot_timeline_anim_multiframe(mf_frames, mf_dt);
    let _ = snapshot_toggle_states();
    let _ = snapshot_replay_driven();
    let _ = snapshot_deal_grid();
    let _ = snapshot_transitions();
    let _ = snapshot_transitions_frame2();
    if let Some(seed) = seed_opt {
        let _ = snapshot_rng_pattern(seed);
        let _ = snapshot_deal_grid_anim_multiframe(seed, mf_frames, mf_dt);
    }
    Ok(())
}
