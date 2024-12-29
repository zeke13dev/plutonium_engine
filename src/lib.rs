extern crate image;
pub mod camera;
pub mod pluto_objects {
    pub mod button;
    pub mod text2d;
    pub mod text_input;
    pub mod texture_2d;
    pub mod texture_atlas_2d;
}
pub mod text;
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
use rusttype::{point, Font, PositionedGlyph, Scale};

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

enum RenderItem {
    Texture {
        texture_key: Uuid,
        transform_bind_group: wgpu::BindGroup,
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
    text_renderer: TextRenderer,
    loaded_fonts: HashMap<String, bool>,
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
        font_size: f32,
        font_key: &str,
    ) -> Result<(), FontError> {
        if self.loaded_fonts.contains_key(font_key) {
            return Ok(());
        }

        let font_data = std::fs::read(font_path).map_err(|e| FontError::IoError(e))?;
        let font = Font::try_from_vec(font_data).ok_or(FontError::InvalidFontData)?;
        let scale = Scale::uniform(font_size);
        let padding = 2;

        // Get atlas dimensions and max tile sizes
        let (atlas_width, atlas_height, char_dimensions, max_tile_width, max_tile_height) =
            TextRenderer::calculate_atlas_size(&font, scale, padding);

        let tile_size = Size::new(max_tile_width as f32, max_tile_height as f32);

        let (texture_data, char_map) = TextRenderer::render_glyphs_to_atlas(
            &font,
            scale,
            (atlas_width, atlas_height),
            &char_dimensions,
            padding,
        )
        .ok_or(FontError::AtlasRenderError)?;

        let atlas_id = Uuid::new_v4();
        let atlas = self.create_font_texture_atlas(
            atlas_id,
            &texture_data,
            atlas_width,
            atlas_height,
            tile_size,
            &char_map,
        );

        // Pass max dimensions to store_font_atlas
        self.text_renderer.store_font_atlas(
            font_key,
            atlas,
            char_map,
            font_size,
            (atlas_width, atlas_height),
            padding,
            max_tile_width,
            max_tile_height,
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
                    &self.text_renderer,
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
                &self.device,
                &self.queue,
                self.viewport_size,
                self.camera.get_pos(),
            );
        }
        for atlas in self.atlas_map.values_mut() {
            atlas.update_transform_uniform(
                &self.device,
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
                transform_bind_group,
            });
        }
    }

    pub fn queue_tile(&mut self, texture_key: &Uuid, tile_index: usize, position: Position) {
        let position = position * self.dpi_scale_factor;
        if let Some(atlas) = self.atlas_map.get(texture_key) {
            // Get transform from TextureAtlas
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

    pub fn queue_text(&mut self, text: &str, font_key: &str, position: Position) {
        let chars = self.text_renderer.calculate_text_layout(
            text,
            font_key,
            position,
            self.dpi_scale_factor,
        );
        for char in chars {
            // Scale position here instead
            // let scaled_position = char.position * self.dpi_scale_factor;
            let scaled_position = char.position;
            self.queue_tile(&char.atlas_id, char.tile_index, scaled_position);
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
                    RenderItem::Texture {
                        texture_key,
                        transform_bind_group,
                    } => {
                        // Render the texture, using the precomputed transform
                        if let Some(texture) = self.texture_map.get(texture_key) {
                            texture.render(&mut rpass, &self.render_pipeline, transform_bind_group);
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
        file_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> (Uuid, Rectangle) {
        let scale_factor = scale_factor * self.dpi_scale_factor;
        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new(
            texture_key,
            &self.device,
            &self.queue,
            file_path,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            scale_factor,
        );

        let texture = svg_texture.expect("texture should always be created properly");
        let dimensions = texture.dimensions();
        self.texture_map.insert(texture_key, texture);
        (texture_key, dimensions)
    }

    pub fn create_texture_atlas(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
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
            scale_factor * self.dpi_scale_factor, // Apply DPI scaling
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
            view_formats: &[],
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
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
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
        scale_factor: f32,
    ) -> Text2D {
        let id = Uuid::new_v4();

        // Ensure font is loaded, now with proper error handling
        if !self.loaded_fonts.contains_key(font_key) {
            panic!("Failed to load font");
        }

        // Create text dimensions based on measurement - now needs font_key
        let width = self.text_renderer.measure_text(text, font_key);
        let dimensions = Rectangle::new(position.x, position.y, width, font_size);

        let internal = Text2DInternal::new(
            id,
            font_key.to_string(), // Changed from font_path to font_key
            dimensions,
            font_size,
            text,
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
        let (texture_key, dimensions) =
            self.create_texture_atlas(svg_path, position, scale_factor, tile_size);

        // Create the internal representation
        let internal = TextureAtlas2DInternal::new(id, texture_key, dimensions, tile_size);
        let rc_internal = Rc::new(RefCell::new(internal));

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        TextureAtlas2D::new(rc_internal)
    }

    pub fn create_button(
        &mut self,
        svg_path: &str,
        text: &str,
        font_key: &str,
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
        let text_object =
            self.create_text2d(text, font_key, font_size, text_position, scale_factor);

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
        font_key: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> TextInput {
        let input_id = Uuid::new_v4();

        // Create button
        let button = self.create_button(
            svg_path,
            "",
            font_key,
            font_size,
            position,
            scale_factor,
            None,
        );

        // Create text object
        let text_position = Position {
            x: button.get_dimensions().x + (button.get_dimensions().width * 0.01),
            y: button.get_dimensions().y + (button.get_dimensions().height * 0.05),
        };
        let text = self.create_text2d("", font_key, font_size, text_position, scale_factor);

        // Create cursor
        let cursor = self.create_text2d("|", font_key, font_size, position, scale_factor);

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

        let text_renderer = TextRenderer::new();
        let loaded_fonts = HashMap::new();

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
            text_renderer,
            loaded_fonts,
        }
    }
}
