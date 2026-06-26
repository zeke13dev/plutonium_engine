//! Object-factory methods for `PlutoniumEngine`.
//!
//! Contains all `create_*` methods that construct textures, atlases, and Pluto objects.

#[cfg(feature = "widgets")]
use crate::pluto_objects::{
    button::{Button, ButtonInternal},
    text_input::{TextInput, TextInputInternal},
};
use crate::pluto_objects::{
    shapes::{Shape, ShapeInternal, ShapeType},
    text2d::{Text2D, Text2DInternal},
    texture_2d::{Texture2D, Texture2DInternal},
    texture_atlas_2d::{TextureAtlas2D, TextureAtlas2DInternal},
};
use crate::text::{CharacterInfo, FontError};
use crate::texture_atlas::TextureAtlas;
use crate::texture_svg::TextureSVG;
use crate::utils::{Position, Rectangle, Size};
use crate::{EngineError, PlutoniumEngine};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;

impl<'a> PlutoniumEngine<'a> {
    /// Creates texture svg.
    pub fn create_texture_svg(
        &mut self,
        file_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> Result<(Uuid, Rectangle), EngineError> {
        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new(
            texture_key,
            &self.device,
            &self.queue,
            file_path,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            scale_factor * self.dpi_scale_factor,
        );

        let texture = svg_texture.ok_or_else(|| {
            EngineError::TextureCreationError(format!(
                "failed to create SVG texture from '{file_path}'"
            ))
        })?;
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        Ok((texture_key, dimensions))
    }

    /// Creates texture svg from data.
    pub fn create_texture_svg_from_data(
        &mut self,
        svg_data: &str,
        position: Position,
        scale_factor: f32,
    ) -> Result<(Uuid, Rectangle), EngineError> {
        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new_from_data(
            texture_key,
            &self.device,
            &self.queue,
            svg_data,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            scale_factor * self.dpi_scale_factor,
        );

        let texture = svg_texture.ok_or_else(|| {
            EngineError::TextureCreationError(
                "failed to create SVG texture from in-memory data".to_string(),
            )
        })?;
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        Ok((texture_key, dimensions))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn create_texture_svg_from_str(
        &mut self,
        svg_source: &str,
        position: Position,
        scale_factor: f32,
    ) -> Result<(Uuid, Rectangle), EngineError> {
        self.create_texture_svg_from_data(svg_source, position, scale_factor)
    }

    #[cfg(feature = "raster")]
    /// Creates texture raster from path.
    pub fn create_texture_raster_from_path(
        &mut self,
        path: &str,
        position: Position,
    ) -> Result<(Uuid, Rectangle), EngineError> {
        let img = image::open(path)
            .map_err(|err| {
                EngineError::ImageDecodeError(format!(
                    "failed to open raster image '{path}': {err}"
                ))
            })?
            .to_rgba8();
        let (width, height) = img.dimensions();
        let rgba = img.as_raw();

        let texture_key = Uuid::new_v4();
        let svg_texture = TextureSVG::new_from_rgba(
            texture_key,
            &self.device,
            &self.queue,
            width,
            height,
            rgba,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
        );

        let texture = svg_texture.ok_or_else(|| {
            EngineError::TextureCreationError(format!(
                "failed to create raster texture from '{path}'"
            ))
        })?;
        let dimensions = texture.dimensions() / self.dpi_scale_factor;

        self.texture_map.insert(texture_key, texture);
        Ok((texture_key, dimensions))
    }

    /// Creates texture atlas.
    pub fn create_texture_atlas(
        &mut self,
        svg_path: &str,
        position: Position,
        tile_size: Size,
    ) -> Result<(Uuid, Rectangle), EngineError> {
        let texture_key = Uuid::new_v4();

        // Update to match new TextureAtlas interface
        let atlas = TextureAtlas::new(
            texture_key,
            &self.device,
            &self.queue,
            svg_path,
            &self.texture_bind_group_layout,
            &self.transform_bind_group_layout,
            position,
            tile_size,
        )
        .ok_or_else(|| {
            EngineError::TextureCreationError(format!(
                "failed to create texture atlas from '{svg_path}'"
            ))
        })?;
        let dimensions = atlas.dimensions();

        let positioned_dimensions =
            Rectangle::new(position.x, position.y, dimensions.width, dimensions.height);

        self.atlas_map.insert(texture_key, atlas);
        Ok((texture_key, positioned_dimensions))
    }

    pub(crate) fn create_font_texture_atlas(
        &mut self,
        atlas_id: Uuid,
        texture_data: &[u8],
        width: u32,
        height: u32,
        tile_size: Size,
        char_positions: &HashMap<char, CharacterInfo>,
    ) -> Result<TextureAtlas2D, FontError> {
        self.create_font_texture_atlas_with_options(
            atlas_id,
            texture_data,
            width,
            height,
            tile_size,
            char_positions,
            crate::COLOR_TEXTURE_FORMAT,
            wgpu::FilterMode::Nearest,
            wgpu::FilterMode::Nearest,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_font_texture_atlas_with_options(
        &mut self,
        atlas_id: Uuid,
        texture_data: &[u8],
        width: u32,
        height: u32,
        tile_size: Size,
        char_positions: &HashMap<char, CharacterInfo>,
        texture_format: wgpu::TextureFormat,
        mag_filter: wgpu::FilterMode,
        min_filter: wgpu::FilterMode,
        force_base_mip_level: bool,
    ) -> Result<TextureAtlas2D, FontError> {
        if force_base_mip_level {
            debug_assert!(
                !matches!(
                    texture_format,
                    wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb
                ),
                "MSDF atlases must use linear (non-sRGB) texture formats"
            );
        }

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
            format: texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[texture_format],
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
            mag_filter,
            min_filter,
            // MSDF textures are encoded distance data, not colors. Keep sampling at mip 0.
            mipmap_filter: if force_base_mip_level {
                wgpu::FilterMode::Nearest
            } else {
                min_filter
            },
            lod_min_clamp: 0.0,
            lod_max_clamp: if force_base_mip_level { 0.0 } else { 32.0 },
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
            #[cfg(not(target_arch = "wasm32"))]
            if texture_format == crate::COLOR_TEXTURE_FORMAT {
                let _ = atlas.save_debug_png(&self.device, &self.queue, "debug_atlas.png");
            }
            // Add to atlas_map
            self.atlas_map.insert(atlas_id, atlas);

            // Create the internal representation
            let internal = TextureAtlas2DInternal::new(
                atlas_id,
                atlas_id,
                1.0,
                Rectangle::new(0.0, 0.0, width as f32, height as f32),
                tile_size,
            );
            let rc_internal = Rc::new(RefCell::new(internal));

            self.pluto_objects.insert(atlas_id, rc_internal.clone());
            self.update_queue.push(atlas_id);

            Ok(TextureAtlas2D::new(rc_internal))
        } else {
            Err(FontError::AtlasRenderError)
        }
    }

    /* OBJECT CREATION FUNCTIONS */
    /// Creates texture 2d.
    pub fn create_texture_2d(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
    ) -> Result<Texture2D, EngineError> {
        let id = Uuid::new_v4();

        // Create the underlying texture
        let (texture_key, dimensions) =
            self.create_texture_svg(svg_path, position, scale_factor)?;

        // Create the internal representation
        let internal = Texture2DInternal::new(id, texture_key, dimensions);
        let rc_internal = Rc::new(RefCell::new(internal));

        // Add to pluto objects and update queue
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        // Return the wrapper
        Ok(Texture2D::new(rc_internal))
    }

    /// Creates text2d.
    pub fn create_text2d(
        &mut self,
        text: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
    ) -> Result<Text2D, EngineError> {
        self.create_text2d_with_z(text, font_key, font_size, position, 0)
    }

    /// Creates text2d with z.
    pub fn create_text2d_with_z(
        &mut self,
        text: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
        z: i32,
    ) -> Result<Text2D, EngineError> {
        let id = Uuid::new_v4();
        if !self.loaded_fonts.contains_key(font_key) {
            return Err(EngineError::FontError(FontError::InvalidFontData));
        }
        // Create text dimensions based on measurement - now needs font_key
        let text_size = self.measure_text(text, font_key, 0.0, 0.0, Some(font_size));
        let dimensions = Rectangle::new(
            position.x,
            position.y,
            text_size.0,
            text_size.1 as f32 * font_size,
        );

        let internal = Text2DInternal::new(
            id,
            font_key.to_string(), // Changed from font_path to font_key
            dimensions,
            font_size,
            text,
            None,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        // Set z after creation
        rc_internal.borrow_mut().set_z(z);
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Ok(Text2D::new(rc_internal))
    }

    /// Creates texture atlas 2d.
    pub fn create_texture_atlas_2d(
        &mut self,
        svg_path: &str,
        position: Position,
        scale_factor: f32,
        tile_size: Size,
    ) -> Result<TextureAtlas2D, EngineError> {
        let id = Uuid::new_v4();

        // Create texture atlas instead of regular texture
        let (texture_key, dimensions) = self.create_texture_atlas(svg_path, position, tile_size)?;

        // Create the internal representation
        let internal =
            TextureAtlas2DInternal::new(id, texture_key, scale_factor, dimensions, tile_size);
        let rc_internal = Rc::new(RefCell::new(internal));

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Ok(TextureAtlas2D::new(rc_internal))
    }

    #[cfg(feature = "widgets")]
    #[allow(clippy::too_many_arguments)]
    /// Creates button.
    pub fn create_button(
        &mut self,
        svg_path: &str,
        text: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> Result<Button, EngineError> {
        let id = Uuid::new_v4();

        // Create button texture
        let (button_texture_key, button_dimensions) =
            self.create_texture_svg(svg_path, position, scale_factor)?;

        // Create text object
        let text_position = Position {
            x: button_dimensions.x + (button_dimensions.width * 0.1),
            y: button_dimensions.y + (button_dimensions.height / 2.0),
        };
        let text_object = self.create_text2d(text, font_key, font_size, text_position)?;
        text_object.set_z(10000);

        // Create internal representation
        let internal = ButtonInternal::new(id, button_texture_key, button_dimensions, text_object);

        // Wrap in Rc<RefCell> and store
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        // Return the wrapper
        Ok(Button::new(rc_internal))
    }

    #[cfg(feature = "widgets")]
    /// Creates text input.
    pub fn create_text_input(
        &mut self,
        svg_path: &str,
        font_key: &str,
        font_size: f32,
        position: Position,
        scale_factor: f32,
    ) -> Result<TextInput, EngineError> {
        let input_id = Uuid::new_v4();

        // Create button
        let button =
            self.create_button(svg_path, "", font_key, font_size, position, scale_factor)?;

        // Create text object
        let text_position = Position {
            x: button.get_dimensions().x + (button.get_dimensions().width * 0.01),
            y: button.get_dimensions().y + (button.get_dimensions().height * 0.05),
        };
        let text = self.create_text2d("", font_key, font_size, text_position)?;

        // Create cursor using Texture2D with embedded SVG data
        let cursor_height = font_size * 1.05 / self.dpi_scale_factor;
        let cursor_width = scale_factor / self.dpi_scale_factor;
        let cursor_svg_data = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">
        <rect width=\"{}\" height=\"{}\" fill=\"#000\">
            <animate
                attributeName=\"opacity\"
                values=\"1;0;1\"
                dur=\"1s\"
                repeatCount=\"indefinite\"/>
        </rect>
    </svg>",
            cursor_width, cursor_height, cursor_width, cursor_height
        );

        let cursor_position = Position {
            x: text_position.x,
            y: button.get_dimensions().y + (button.get_dimensions().height * 0.1),
        };

        let cursor_id = Uuid::new_v4();
        let (texture_key, dimensions) =
            self.create_texture_svg_from_data(&cursor_svg_data, cursor_position, scale_factor)?;

        // Create the internal representation for cursor
        let cursor_internal = Texture2DInternal::new(cursor_id, texture_key, dimensions);
        let rc_cursor_internal = Rc::new(RefCell::new(cursor_internal));

        // Add cursor to pluto objects and update queue
        self.pluto_objects
            .insert(cursor_id, rc_cursor_internal.clone());
        self.update_queue.push(cursor_id);

        let cursor = Texture2D::new(rc_cursor_internal);

        // Create internal representation for text input
        let dimensions = button.get_dimensions();
        let internal = TextInputInternal::new(input_id, button, text, cursor, dimensions);

        // Wrap in Rc<RefCell> and store
        let rc_internal = Rc::new(RefCell::new(internal));
        self.pluto_objects.insert(input_id, rc_internal.clone());
        self.update_queue.push(input_id);

        // Return the wrapper
        Ok(TextInput::new(rc_internal))
    }

    /// Creates rect.
    pub fn create_rect(
        &mut self,
        bounds: Rectangle,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Result<Shape, EngineError> {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Rectangle,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        // Create the texture using svg data directly
        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0)?;

        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Ok(Shape::new(rc_internal))
    }

    /// Creates circle.
    pub fn create_circle(
        &mut self,
        radius: f32,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Result<Shape, EngineError> {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();
        let bounds = Rectangle::new(0.0, 0.0, radius * 2.0, radius * 2.0);

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Circle,
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0)?;
        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Ok(Shape::new(rc_internal))
    }

    /// Creates polygon.
    pub fn create_polygon(
        &mut self,
        radius: f32,
        points: u32,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
    ) -> Result<Shape, EngineError> {
        let id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();
        let bounds = Rectangle::new(0.0, 0.0, radius * 2.0, radius * 2.0);

        let internal = ShapeInternal::new(
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            ShapeType::Polygon(points),
        );

        let rc_internal = Rc::new(RefCell::new(internal));
        let svg_data = rc_internal.borrow().generate_svg_data();

        let (texture_key, _dimensions) =
            self.create_texture_svg_from_data(&svg_data, position, 1.0)?;
        rc_internal.borrow_mut().set_ids(id, texture_key);

        self.pluto_objects.insert(id, rc_internal.clone());
        self.update_queue.push(id);

        Ok(Shape::new(rc_internal))
    }
}
