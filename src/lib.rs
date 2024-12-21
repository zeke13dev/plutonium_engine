extern crate image;
pub mod camera;
pub mod pluto_objects {
    pub mod button;
    pub mod text2d;
    pub mod text_input;
    pub mod texture_2d;
    pub mod texture_atlas_2d;
}
pub mod texture_atlas;
pub mod texture_svg;
pub mod traits;
pub mod utils;

use crate::traits::UpdateContext;
use camera::Camera;
use pluto_objects::{
    button::{Button, ButtonInternal},
    text2d::{Text2D, Text2DInternal},
    text_input::{TextInput, TextInputInternal},
    texture_2d::{Texture2D, Texture2DInternal},
    texture_atlas_2d::{TextureAtlas2D, TextureAtlas2DInternal},
};
use pollster::block_on;
use std::cell::RefCell;
use std::rc::Rc;
use std::{borrow::Cow, collections::HashMap};
use texture_atlas::TextureAtlas;
use texture_svg::*;
use traits::PlutoObject;
use utils::*;
use uuid::Uuid;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::keyboard::Key;

enum RenderItem {
    Texture {
        texture_key: Uuid,
    },
    AtlasTile {
        texture_key: Uuid,
        transform_bind_group: wgpu::BindGroup,
        tile_index: usize,
    },
}

pub struct PlutoniumEngine<'a> {
    pub size: PhysicalSize<u32>,
    dpi_scale_factor: f32,
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    transform_bind_group_layout: wgpu::BindGroupLayout,
    uv_bind_group_layout: wgpu::BindGroupLayout,
    texture_map: HashMap<Uuid, TextureSVG>,
    atlas_map: HashMap<Uuid, TextureAtlas>,
    pluto_objects: HashMap<Uuid, Rc<RefCell<dyn PlutoObject>>>,
    update_queue: Vec<Uuid>,
    render_queue: Vec<RenderItem>,
    viewport_size: Size,
    camera: Camera,
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

    pub fn set_texture_position(&mut self, key: &Uuid, position: Position) {
        if let Some(texture) = self.texture_map.get_mut(key) {
            texture.set_position(
                &self.device,
                &self.queue,
                position,
                self.viewport_size,
                self.camera.get_pos(),
            );
        }
    }

    pub fn resize(&mut self, new_size: &PhysicalSize<u32>, scale_factor: f32) {
        self.size = *new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.viewport_size = Size {
            width: self.size.width as f32 / scale_factor,
            height: self.size.height as f32 / scale_factor,
        };
    }

    pub fn update(&mut self, mouse_info: Option<MouseInfo>, key: &Option<Key>) {
        // text doesn't seem to be getting updated
        for id in &self.update_queue {
            if let Some(obj) = self.pluto_objects.get(id) {
                obj.borrow_mut().update(
                    mouse_info,
                    key,
                    &mut self.texture_map,
                    Some(UpdateContext {
                        device: &self.device,
                        queue: &self.queue,
                        viewport_size: &self.viewport_size,
                        camera_position: &self.camera.get_pos(),
                    }),
                    self.dpi_scale_factor,
                );
            }
        }
        let (camera_position, tether_size) = if let Some(tether_target) = &self.camera.tether_target
        {
            if let Some(tether) = self.texture_map.get(tether_target) {
                let tether_size = Some(tether.size()); // Wrap in `Some`
                (tether.pos(), tether_size)
            } else {
                (self.camera.get_pos(), None)
            }
        } else {
            (self.camera.get_pos(), None)
        };

        self.camera.set_pos(camera_position);
        self.camera.set_tether_size(tether_size);

        // update actual location of where object buffers are
        for texture in self.texture_map.values_mut() {
            texture.update_transform_uniform(
                &self.queue,
                self.viewport_size,
                self.camera.get_pos(),
            );
        }
    }

    pub fn set_camera_target(&mut self, texture_key: Uuid) {
        self.camera.tether_target = Some(texture_key);
    }

    pub fn queue_texture(&mut self, texture_key: &Uuid, position: Option<Position>) {
        if let Some(texture) = self.texture_map.get(&texture_key) {
            // Generate the transformation matrix based on the position and camera
            let position = position.unwrap_or(Position::default()) * self.dpi_scale_factor;
            let transform_uniform =
                texture.get_transform_uniform(self.viewport_size, position, self.camera.get_pos());

            let transform_uniform_buffer =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Transform Uniform Buffer"),
                        contents: bytemuck::cast_slice(&[transform_uniform]),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    });

            let transform_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.transform_bind_group_layout,
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

            self.render_queue.push(RenderItem::Texture {
                texture_key: *texture_key,
            });
        }
    }

    pub fn queue_tile(&mut self, texture_key: &Uuid, tile_index: usize, position: Position) {
        let position = position * self.dpi_scale_factor;
        if let Some(atlas) = self.atlas_map.get(&texture_key) {
            // Use the existing get_transform_uniform method
            let transform_uniform =
                atlas.get_transform_uniform(self.viewport_size, position, self.camera.get_pos());

            let transform_uniform_buffer =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Transform Uniform Buffer"),
                        contents: bytemuck::cast_slice(&[transform_uniform]),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    });

            let transform_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.transform_bind_group_layout,
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

            self.render_queue.push(RenderItem::AtlasTile {
                texture_key: *texture_key,
                transform_bind_group,
                tile_index,
            });
        }
    }

    pub fn queue_text(&mut self, key: &Uuid) {
        if let Some(texture) = self.texture_map.get(&key) {
            // Generate the transformation matrix based on the texture's position

            // NEED TO MULTIPLY BY SCALE FACTOR DPI
            let transform_uniform = texture.get_transform_uniform(
                self.viewport_size,
                texture.pos() * self.dpi_scale_factor,
                self.camera.get_pos(),
            );

            let transform_uniform_buffer =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Transform Uniform Buffer"),
                        contents: bytemuck::cast_slice(&[transform_uniform]),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    });

            let transform_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.transform_bind_group_layout,
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

            self.render_queue
                .push(RenderItem::Texture { texture_key: *key });
        } else {
            eprintln!("Text texture with key '{}' not found.", key);
        }
    }

    pub fn clear_render_queue(&mut self) {
        self.render_queue.clear();
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
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

            for item in &self.render_queue {
                match item {
                    RenderItem::Texture { texture_key } => {
                        if let Some(texture) = self.texture_map.get(texture_key) {
                            texture.render(&mut rpass, &self.render_pipeline);
                        }
                    }
                    RenderItem::AtlasTile {
                        texture_key,
                        transform_bind_group,
                        tile_index,
                    } => {
                        if let Some(atlas) = self.atlas_map.get(texture_key) {
                            atlas.render_tile(
                                &mut rpass,
                                &self.render_pipeline,
                                *tile_index,
                                transform_bind_group,
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

    pub fn create_texture_svg(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> (Uuid, Rectangle) {
        let texture_key = Uuid::new_v4();

        if let Some(texture) = TextureSVG::new(
            texture_key,
            &self.device,
            &self.queue,
            svg_path,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            scale_factor,
        ) {
            let dimensions = texture.dimensions();
            self.texture_map.insert(texture_key, texture);
            (texture_key, dimensions)
        } else {
            panic!("Failed to create texture")
        }
    }

    pub fn create_texture_atlas(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
        tile_size: Size,
    ) -> (Uuid, Rectangle) {
        // Generate a unique identifier for the new texture atlas
        let texture_key = Uuid::new_v4();

        // Read the SVG file and convert it to a string for proper SVG processing
        let svg_data = std::fs::read_to_string(svg_path).expect("Failed to read SVG file");

        // Create the texture atlas with our updated interface
        if let Some(atlas) = TextureAtlas::new(
            texture_key,
            &self.device,
            &self.queue, // Now required for texture upload
            &svg_data,   // Now expects string data for SVG parsing
            tile_size,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            &self.uv_bind_group_layout, // New requirement for UV coordinate handling
            scale_factor,
        ) {
            // Create a rectangle representing the atlas dimensions at the specified position
            let dimensions = Rectangle::new(
                position.x,
                position.y,
                atlas.dimensions().width,
                atlas.dimensions().height,
            );

            // Store the atlas and return its identifier and dimensions
            self.atlas_map.insert(texture_key, atlas);
            (texture_key, dimensions)
        } else {
            panic!("Failed to create texture atlas")
        }
    }

    pub fn create_text_texture(
        &mut self,
        text: &str,
        font_size: f32,
        scale_factor: f32,
        position: Position,
    ) -> (Uuid, Rectangle) {
        let texture_key = Uuid::new_v4();
        let texture_svg = TextureSVG::from_text(
            texture_key,
            &self.device,
            &self.queue,
            text,
            font_size * scale_factor,
            position,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            scale_factor,
        );

        let texture = texture_svg.expect("texture should vacously be created properly");
        let dimensions = texture.dimensions();
        self.texture_map.insert(texture_key, texture);
        (texture_key, dimensions)
    }

    pub fn remove_object(&mut self, id: Uuid) {
        self.pluto_objects.remove(&id);
    }

    /* OBJECT CREATION FUNCTIONS */
    pub fn create_texture2d(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> Texture2D {
        let id = Uuid::new_v4();

        // Create the texture data
        let (texture_key, dimensions) = self.create_texture_svg(svg_path, position, scale_factor);

        // Create the internal representation
        let internal = Texture2DInternal::new(id, texture_key, dimensions);

        // Wrap it in Rc<RefCell> and store it in the engine
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);
        // Return the user-facing wrapper
        Texture2D::new(rc_internal)
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
        let (texture_key, dimensions) =
            self.create_texture_atlas(svg_path, position, scale_factor, tile_size);

        // Create the internal representation
        let internal = TextureAtlas2DInternal::new(id, texture_key, dimensions, tile_size);
        let rc_internal = Rc::new(RefCell::new(internal));

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        TextureAtlas2D::new(rc_internal)
    }

    pub fn create_text2d(
        &mut self,
        text: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> Text2D {
        let id = Uuid::new_v4();
        let (texture_key, dimensions) =
            self.create_text_texture(text, font_size, scale_factor, position);

        // Create the internal representation
        let internal = Text2DInternal::new(id, texture_key, dimensions, font_size, text);

        // Wrap it in Rc<RefCell> and store it
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        // Return the user-facing wrapper
        Text2D::new(rc_internal)
    }

    pub fn create_button(
        &mut self,
        svg_path: &str,
        text: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
        callback: Option<Box<dyn Fn()>>,
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
        let (_text_texture_key, _text_dimensions) =
            self.create_text_texture(text, font_size, scale_factor, text_position);
        let text_object = self.create_text2d(text, font_size, text_position, scale_factor);
        text_object.set_pos(Position { x: 0.0, y: 0.0 });
        // Create internal representation
        let internal = ButtonInternal::new(
            id,
            button_texture_key,
            button_dimensions,
            text_object,
            callback,
        );

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
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> TextInput {
        let input_id = Uuid::new_v4();

        // Create button
        let button = self.create_button(svg_path, "", font_size, position, scale_factor, None);

        // Create text object
        let text_position = Position {
            x: button.get_dimensions().x + (button.get_dimensions().width * 0.01),
            y: button.get_dimensions().y + (button.get_dimensions().height * 0.05),
        };
        let text = self.create_text2d("", font_size, text_position, scale_factor);

        // Create cursor
        let cursor = self.create_text2d("|", font_size, position, scale_factor);

        // Create internal representation
        let dimensions = button.get_dimensions();
        let internal = TextInputInternal::new(input_id, button, text, cursor, dimensions);

        // Wrap in Rc<RefCell> and store
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(input_id, rc_internal.clone());
        self.update_queue.push(input_id);

        // Return the wrapper
        TextInput::new(rc_internal)
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
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                required_limits:
                    wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            },
            None,
        ))
        .expect("Failed to create device");

        let config = wgpu::SurfaceConfiguration {
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb, // Assume `surface` and `adapter` are already defined
            width: size.width,                           // Set to your window's initial width
            height: size.height,                         // Set to your window's initial height
            present_mode: wgpu::PresentMode::Fifo,       // This enables V-Sync
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Texture Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &transform_bind_group_layout,
                &uv_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // set up render pipeline

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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
            uv_bind_group_layout,
            texture_map,
            atlas_map,
            pluto_objects,
            render_queue,
            update_queue,
            viewport_size,
            camera,
        }
    }
}
