//! Immediate-mode draw and queue methods for `PlutoniumEngine`.
//!
//! `queue_*` methods are the canonical render-queue API: they append a draw
//! item with explicit coordinates, layer, fit, or clipping choices. `draw_*`
//! methods are compatibility/convenience wrappers for immediate-mode examples;
//! they still append to the same render queue and are flushed by `render()`.

use crate::pluto_objects::text2d::TextContainer;
use crate::renderer::RectCommand;
use crate::text::GlyphRenderMode;
use crate::utils::{Position, Size, TransformUniform};
use crate::{
    DrawParams, PlutoniumEngine, QueuedItem, Rectangle, RenderItem, SlotState, TextureFit,
};
use uuid::Uuid;

impl<'a> PlutoniumEngine<'a> {
    /// Queues texture for rendering.
    pub fn queue_texture(&mut self, texture_key: &Uuid, position: Option<Position>) {
        self.queue_texture_with_layer(texture_key, position, 0);
    }

    /// Queues texture with layer for rendering.
    pub fn queue_texture_with_layer(
        &mut self,
        texture_key: &Uuid,
        position: Option<Position>,
        z: i32,
    ) {
        if let Some(texture) = self.texture_map.get(texture_key) {
            // Generate the transformation matrix based on the position and camera
            let position = position.unwrap_or_default() * self.dpi_scale_factor;
            let transform_uniform = texture.get_transform_uniform(
                self.viewport_size,
                position,
                self.camera.get_pos(self.dpi_scale_factor),
                0.0,
                1.0,
            );
            let transform_index = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z,
                clip_rect: None,
                item: RenderItem::Texture {
                    texture_key: *texture_key,
                    transform_index,
                },
            });
        }
    }

    fn inset_rect_uniform(rect: Rectangle, inset: f32) -> Option<Rectangle> {
        let inset = inset.max(0.0);
        let width = rect.width - (inset * 2.0);
        let height = rect.height - (inset * 2.0);
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        Some(Rectangle::new(
            rect.x + inset,
            rect.y + inset,
            width,
            height,
        ))
    }

    fn queue_texture_stretched_internal(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        z: i32,
        fit: TextureFit,
        clip_rect: Option<Rectangle>,
        camera_position_px: Position,
        inset: f32,
    ) {
        if dst.width <= 0.0 || dst.height <= 0.0 {
            return;
        }
        let Some(texture) = self.texture_map.get(texture_key) else {
            return;
        };

        // Apply inset to the destination rectangle
        let dst = Self::inset_rect_uniform(dst, inset).unwrap_or(dst);

        let dst_left_px = (dst.x * self.dpi_scale_factor) - camera_position_px.x;
        let dst_top_px = (dst.y * self.dpi_scale_factor) - camera_position_px.y;
        let dst_w_px = (dst.width * self.dpi_scale_factor).max(0.0);
        let dst_h_px = (dst.height * self.dpi_scale_factor).max(0.0);
        if dst_w_px <= 0.0 || dst_h_px <= 0.0 {
            return;
        }

        let source_size_px = texture.original_size();
        let (draw_w_px, draw_h_px) = match fit {
            TextureFit::Contain => {
                if source_size_px.width <= 0.0 || source_size_px.height <= 0.0 {
                    return;
                }
                let scale = (dst_w_px / source_size_px.width).min(dst_h_px / source_size_px.height);
                (source_size_px.width * scale, source_size_px.height * scale)
            }
            TextureFit::StretchFill => (dst_w_px, dst_h_px),
            TextureFit::Cover => {
                if source_size_px.width <= 0.0 || source_size_px.height <= 0.0 {
                    return;
                }
                let scale = (dst_w_px / source_size_px.width).max(dst_h_px / source_size_px.height);
                (source_size_px.width * scale, source_size_px.height * scale)
            }
        };
        if draw_w_px <= 0.0 || draw_h_px <= 0.0 {
            return;
        }

        // Contain mode letterboxes by centering inside the destination rect.
        let left_px = (dst_left_px + (dst_w_px - draw_w_px) * 0.5).round();
        let top_px = (dst_top_px + (dst_h_px - draw_h_px) * 0.5).round();

        let width_ndc = 2.0 * (draw_w_px / self.viewport_size.width);
        let height_ndc = 2.0 * (draw_h_px / self.viewport_size.height);
        let ndc_left = 2.0 * (left_px / self.viewport_size.width) - 1.0;
        let ndc_top = 1.0 - 2.0 * (top_px / self.viewport_size.height);

        let transform_uniform = TransformUniform {
            transform: [
                [width_ndc, 0.0, 0.0, 0.0],
                [0.0, height_ndc, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ndc_left, ndc_top, 0.0, 1.0],
            ],
        };
        let transform_index = self.allocate_transform_bind_group(transform_uniform);
        self.render_queue.push(QueuedItem {
            z,
            clip_rect,
            item: RenderItem::Texture {
                texture_key: *texture_key,
                transform_index,
            },
        });
    }

    /// Queues texture stretched for rendering.
    pub fn queue_texture_stretched(&mut self, texture_key: &Uuid, dst: Rectangle) {
        self.queue_texture_stretched_with_layer_and_fit(
            texture_key,
            dst,
            0,
            TextureFit::Contain,
            0.0,
        );
    }

    /// Queues texture stretched with fit for rendering.
    pub fn queue_texture_stretched_with_fit(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        fit: TextureFit,
    ) {
        self.queue_texture_stretched_with_layer_and_fit(texture_key, dst, 0, fit, 0.0);
    }

    /// Queues texture stretched with layer for rendering.
    pub fn queue_texture_stretched_with_layer(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        z: i32,
    ) {
        self.queue_texture_stretched_with_layer_and_fit(
            texture_key,
            dst,
            z,
            TextureFit::Contain,
            0.0,
        );
    }

    /// Queues texture stretched with layer and fit for rendering.
    pub fn queue_texture_stretched_with_layer_and_fit(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        z: i32,
        fit: TextureFit,
        inset: f32,
    ) {
        self.queue_texture_stretched_internal(
            texture_key,
            dst,
            z,
            fit,
            None,
            self.camera.get_pos(self.dpi_scale_factor),
            inset,
        );
    }

    /// Begin a frame-local slot for layered UI composition.
    ///
    /// The slot rectangle is defined in logical screen-space coordinates and is
    /// reused for both rendering and hit geometry.
    pub fn begin_slot(&mut self, slot_id: Uuid, rect: Rectangle, z_base: i32) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            self.slot_states.remove(&slot_id);
            return;
        }
        self.slot_states.insert(
            slot_id,
            SlotState {
                rect,
                z_base,
                is_open: true,
                clip_radius: None,
            },
        );
    }

    /// End a slot declaration for this frame.
    ///
    /// Slot hit geometry remains queryable until `begin_frame()` clears frame-local state.
    pub fn end_slot(&mut self, slot_id: &Uuid) {
        if let Some(slot) = self.slot_states.get_mut(slot_id) {
            slot.is_open = false;
        }
    }

    /// Returns the slot's logical hit rectangle in screen-space.
    pub fn slot_hit_rect(&self, slot_id: &Uuid) -> Option<Rectangle> {
        self.slot_states.get(slot_id).map(|slot| slot.rect)
    }

    /// Placeholder for future rounded-corner slot clipping.
    ///
    /// v1 uses rectangular scissor clipping; this value is stored but not applied yet.
    pub fn set_slot_clip_radius(&mut self, slot_id: &Uuid, radius: Option<f32>) {
        if let Some(slot) = self.slot_states.get_mut(slot_id) {
            slot.clip_radius = radius.map(|r| r.max(0.0));
        }
    }

    /// Queues slot layer texture for rendering.
    pub fn queue_slot_layer_texture(
        &mut self,
        slot_id: &Uuid,
        texture_key: &Uuid,
        fit: TextureFit,
        z_offset: i32,
        inset: Option<f32>,
    ) -> bool {
        let Some(slot) = self.slot_states.get(slot_id).copied() else {
            return false;
        };
        if !slot.is_open {
            return false;
        }
        let Some(layer_rect) = Self::inset_rect_uniform(slot.rect, inset.unwrap_or(0.0)) else {
            return false;
        };
        if !self.texture_map.contains_key(texture_key) {
            return false;
        }
        self.queue_texture_stretched_internal(
            texture_key,
            layer_rect,
            slot.z_base + z_offset,
            fit,
            Some(slot.rect),
            Position::default(),
            0.0,
        );
        true
    }

    pub(crate) fn queue_rect_internal(
        &mut self,
        bounds: Rectangle,
        color: [f32; 4],
        corner_radius_px: f32,
        border: Option<([f32; 4], f32)>,
        z: i32,
        clip_rect: Option<Rectangle>,
        camera_position_px: Position,
    ) {
        let pos = Position {
            x: bounds.x,
            y: bounds.y,
        } * self.dpi_scale_factor;
        let size = bounds.size() * self.dpi_scale_factor;
        let width_ndc = size.width / self.viewport_size.width;
        let height_ndc = size.height / self.viewport_size.height;
        let ndc_dx = (2.0 * (pos.x - camera_position_px.x)) / self.viewport_size.width - 1.0;
        let ndc_dy = 1.0 - (2.0 * (pos.y - camera_position_px.y)) / self.viewport_size.height;
        let ndc_x = ndc_dx + width_ndc;
        let ndc_y = ndc_dy - height_ndc;
        let model = [
            [width_ndc, 0.0, 0.0, 0.0],
            [0.0, height_ndc, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [ndc_x, ndc_y, 0.0, 1.0],
        ];
        let (border_color, border_thickness) = border.unwrap_or(([0.0, 0.0, 0.0, 0.0], 0.0));
        let cmd = RectCommand {
            width_px: size.width,
            height_px: size.height,
            color,
            corner_radius_px,
            border_thickness_px: border_thickness,
            border_color,
            transform: model,
            z,
        };
        self.render_queue.push(QueuedItem {
            z,
            clip_rect,
            item: RenderItem::Rect(cmd),
        });
    }

    /// Queues slot layer rect for rendering.
    pub fn queue_slot_layer_rect(
        &mut self,
        slot_id: &Uuid,
        color: [f32; 4],
        border: Option<([f32; 4], f32)>,
        z_offset: i32,
        inset: Option<f32>,
    ) -> bool {
        let Some(slot) = self.slot_states.get(slot_id).copied() else {
            return false;
        };
        if !slot.is_open {
            return false;
        }
        let Some(layer_rect) = Self::inset_rect_uniform(slot.rect, inset.unwrap_or(0.0)) else {
            return false;
        };
        self.queue_rect_internal(
            layer_rect,
            color,
            0.0,
            border,
            slot.z_base + z_offset,
            Some(slot.rect),
            Position::default(),
        );
        true
    }

    /// Queues tile for rendering.
    pub fn queue_tile(
        &mut self,
        texture_key: &Uuid,
        tile_index: usize,
        position: Position,
        user_scale: f32,
    ) {
        self.queue_tile_with_layer(texture_key, tile_index, position, user_scale, 0);
    }

    /// Queues tile with layer for rendering.
    pub fn queue_tile_with_layer(
        &mut self,
        texture_key: &Uuid,
        tile_index: usize,
        position: Position,
        user_scale: f32,
        z: i32,
    ) {
        self.queue_tile_with_tint(
            texture_key,
            tile_index,
            position,
            user_scale,
            z,
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    /// Queues tile with tint for rendering.
    pub fn queue_tile_with_tint(
        &mut self,
        texture_key: &Uuid,
        tile_index: usize,
        position: Position,
        user_scale: f32,
        z: i32,
        tint: [f32; 4],
    ) {
        if let Some(atlas) = self.atlas_map.get(texture_key) {
            let transform_uniform = atlas.get_transform_uniform(
                self.viewport_size,
                position,
                self.camera.get_pos(self.dpi_scale_factor),
                self.dpi_scale_factor, // position scale (DPI)
                user_scale,            // tile size scale (already physical px)
            );
            let transform_index = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z,
                clip_rect: None,
                item: RenderItem::AtlasTile {
                    texture_key: *texture_key,
                    transform_index,
                    tile_index,
                    tint,
                },
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// Queues atlas uv with tint for rendering.
    pub fn queue_atlas_uv_with_tint(
        &mut self,
        texture_key: &Uuid,
        position: Position,
        size: Size,
        uv_offset: [f32; 2],
        uv_scale: [f32; 2],
        z: i32,
        tint: [f32; 4],
        is_msdf: bool,
        msdf_px_range: f32,
    ) {
        self.queue_atlas_uv_with_tint_internal(
            texture_key,
            position,
            size,
            uv_offset,
            uv_scale,
            z,
            tint,
            is_msdf,
            msdf_px_range,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn queue_atlas_uv_with_tint_internal(
        &mut self,
        texture_key: &Uuid,
        position: Position,
        size: Size,
        uv_offset: [f32; 2],
        uv_scale: [f32; 2],
        z: i32,
        tint: [f32; 4],
        is_msdf: bool,
        msdf_px_range: f32,
    ) {
        if !self.atlas_map.contains_key(texture_key) {
            return;
        }
        if size.width <= 0.0 || size.height <= 0.0 {
            return;
        }

        let cam = self.camera.get_pos(self.dpi_scale_factor);
        let left_raw = (position.x * self.dpi_scale_factor) - cam.x;
        let top_raw = (position.y * self.dpi_scale_factor) - cam.y;
        let right_raw = ((position.x + size.width) * self.dpi_scale_factor) - cam.x;
        let bottom_raw = ((position.y + size.height) * self.dpi_scale_factor) - cam.y;
        // Keep MSDF quads in continuous space to avoid per-glyph quantization errors.
        let (left_px, top_px, right_px, bottom_px) = if is_msdf {
            (left_raw, top_raw, right_raw, bottom_raw)
        } else {
            (
                left_raw.round(),
                top_raw.round(),
                right_raw.round(),
                bottom_raw.round(),
            )
        };
        let px_w = (right_px - left_px).max(0.0);
        let px_h = (bottom_px - top_px).max(0.0);
        if px_w <= 0.0 || px_h <= 0.0 {
            return;
        }

        let width_ndc = 2.0 * (px_w / self.viewport_size.width);
        let height_ndc = 2.0 * (px_h / self.viewport_size.height);
        let ndc_left = 2.0 * (left_px / self.viewport_size.width) - 1.0;
        let ndc_top = -2.0 * (top_px / self.viewport_size.height) + 1.0;
        let ndc_x = ndc_left + width_ndc * 0.5;
        let ndc_y = ndc_top - height_ndc * 0.5;

        let transform_uniform = TransformUniform {
            transform: [
                [width_ndc, 0.0, 0.0, 0.0],
                [0.0, height_ndc, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ndc_x, ndc_y, 0.0, 1.0],
            ],
        };
        let transform_index = self.allocate_transform_bind_group(transform_uniform);
        self.render_queue.push(QueuedItem {
            z,
            clip_rect: None,
            item: RenderItem::AtlasGlyph {
                texture_key: *texture_key,
                transform_index,
                uv_offset,
                uv_scale,
                tint,
                is_msdf,
                msdf_px_range,
            },
        });
    }

    /// Queues text for rendering.
    pub fn queue_text(
        &mut self,
        text: &str,
        font_key: &str,
        position: Position,
        container: &TextContainer,
    ) {
        self.queue_text_with_spacing(
            text,
            font_key,
            position,
            container,
            0.0,
            0.0,
            0,
            [1.0, 1.0, 1.0, 1.0],
            None,
        );
    }

    /// Queues text with spacing for rendering.
    pub fn queue_text_with_spacing(
        &mut self,
        text: &str,
        font_key: &str,
        position: Position,
        container: &TextContainer,
        letter_spacing: f32,
        word_spacing: f32,
        z: i32,
        color: [f32; 4],
        font_size_override: Option<f32>,
    ) {
        let (resolved_font_key, resolved_size_override) =
            self.resolve_font_key_for_render(font_key, font_size_override);
        let chars = self.text_renderer.calculate_text_layout(
            text,
            &resolved_font_key,
            position,
            container,
            letter_spacing,
            word_spacing,
            resolved_size_override,
        );
        for char in chars {
            match char.mode {
                GlyphRenderMode::AtlasTile { tile_index, scale } => {
                    self.queue_tile_with_tint(
                        &char.atlas_id,
                        tile_index,
                        char.position,
                        scale,
                        z,
                        color,
                    );
                }
                GlyphRenderMode::AtlasUv {
                    uv_offset,
                    uv_scale,
                    is_msdf,
                    msdf_px_range,
                } => {
                    self.queue_atlas_uv_with_tint_internal(
                        &char.atlas_id,
                        char.position,
                        char.size,
                        uv_offset,
                        uv_scale,
                        z,
                        color,
                        is_msdf,
                        msdf_px_range,
                    );
                }
            }
        }
    }

    /// Clear render queue.
    pub fn clear_render_queue(&mut self) {
        self.render_queue.clear();
    }

    /// Debug render queue len.
    pub fn debug_render_queue_len(&self) -> usize {
        self.render_queue.len()
    }

    /// Debug surface size.
    pub fn debug_surface_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Debug viewport size.
    pub fn debug_viewport_size(&self) -> Size {
        self.viewport_size
    }

    /// Unload texture.
    pub fn unload_texture(&mut self, texture_key: &Uuid) -> bool {
        self.texture_map.remove(texture_key).is_some()
    }

    /// Unload atlas.
    pub fn unload_atlas(&mut self, atlas_key: &Uuid) -> bool {
        self.atlas_map.remove(atlas_key).is_some()
    }

    /// Convenience immediate-mode sprite draw.
    ///
    /// This appends the same render-queue item as the `queue_*` methods, while
    /// accepting [`DrawParams`] for rotation, scale, and z ordering.
    pub fn draw_texture(&mut self, texture_key: &Uuid, position: Position, params: DrawParams) {
        // rotation only supported by direct draw path for sprites; augment transform
        if let Some(texture) = self.texture_map.get(texture_key) {
            let transform_uniform = texture.get_transform_uniform(
                self.viewport_size,
                position * self.dpi_scale_factor,
                self.camera.get_pos(self.dpi_scale_factor),
                params.rotation,
                params.scale,
            );
            let idx = self.allocate_transform_bind_group(transform_uniform);
            self.render_queue.push(QueuedItem {
                z: params.z,
                clip_rect: None,
                item: RenderItem::Texture {
                    texture_key: *texture_key,
                    transform_index: idx,
                },
            });
        }
    }

    /// Convenience wrapper for [`Self::queue_texture_stretched`].
    pub fn draw_texture_stretched(&mut self, texture_key: &Uuid, dst: Rectangle) {
        self.queue_texture_stretched(texture_key, dst);
    }

    /// Convenience wrapper for [`Self::queue_texture_stretched_with_layer_and_fit`].
    pub fn draw_texture_stretched_with_fit_and_inset(
        &mut self,
        texture_key: &Uuid,
        dst: Rectangle,
        fit: TextureFit,
        inset: f32,
        z: i32,
    ) {
        self.queue_texture_stretched_with_layer_and_fit(texture_key, dst, z, fit, inset);
    }

    /// Convenience wrapper for [`Self::queue_tile_with_layer`] using [`DrawParams`].
    pub fn draw_tile(
        &mut self,
        atlas_key: &Uuid,
        tile_index: usize,
        position: Position,
        params: DrawParams,
    ) {
        let user_scale = if params.scale == 0.0 {
            1.0
        } else {
            params.scale
        };
        self.queue_tile_with_layer(atlas_key, tile_index, position, user_scale, params.z);
    }

    /// Convenience wrapper that queues an atlas tile stretched to `dst`.
    pub fn draw_atlas_tile_stretched(
        &mut self,
        atlas_key: &Uuid,
        tile_index: usize,
        dst: Rectangle,
        z: i32,
    ) {
        if !self.atlas_map.contains_key(atlas_key) {
            return;
        }
        // Convert logical to physical pixels via DPI scale
        let cam = self.camera.get_pos(self.dpi_scale_factor);
        let left_px = ((dst.x * self.dpi_scale_factor) - cam.x).round();
        let top_px = ((dst.y * self.dpi_scale_factor) - cam.y).round();
        let right_px = (((dst.x + dst.width) * self.dpi_scale_factor) - cam.x).round();
        let bottom_px = (((dst.y + dst.height) * self.dpi_scale_factor) - cam.y).round();
        let px_w = (right_px - left_px).max(0.0);
        let px_h = (bottom_px - top_px).max(0.0);

        // NDC scale for 0..1 unit quad
        let width_ndc = 2.0 * (px_w / self.viewport_size.width);
        let height_ndc = 2.0 * (px_h / self.viewport_size.height);

        // NDC translation for top-left (0,0) of unit quad
        let ndc_left = 2.0 * (left_px / self.viewport_size.width) - 1.0;
        let ndc_top = 1.0 - 2.0 * (top_px / self.viewport_size.height);

        let transform_uniform = TransformUniform {
            transform: [
                [width_ndc, 0.0, 0.0, 0.0],
                [0.0, height_ndc, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ndc_left, ndc_top, 0.0, 1.0],
            ],
        };
        let transform_index = self.allocate_transform_bind_group(transform_uniform);
        self.render_queue.push(QueuedItem {
            z,
            clip_rect: None,
            item: RenderItem::AtlasTile {
                texture_key: *atlas_key,
                transform_index,
                tile_index,
                tint: [1.0, 1.0, 1.0, 1.0],
            },
        });
    }

    /// Convenience wrapper that queues an immediate-mode rectangle primitive.
    pub fn draw_rect(
        &mut self,
        bounds: Rectangle,
        color: [f32; 4],
        corner_radius_px: f32,
        border: Option<([f32; 4], f32)>,
        z: i32,
    ) {
        self.queue_rect_internal(
            bounds,
            color,
            corner_radius_px,
            border,
            z,
            None,
            self.camera.get_pos(self.dpi_scale_factor),
        );
    }
}
