#![forbid(unsafe_code)]

use crate::immediate::types::{vec2, Color, RectExt, UiRect, UiVec2};
use plutonium_engine::pluto_objects::text2d::TextContainer;
use plutonium_engine::utils::Rectangle;
use plutonium_engine::PlutoniumEngine;
use uuid::Uuid;

pub trait Painter {
    fn rect(&mut self, rect: UiRect, color: Color, corner_radius: f32);
    fn rect_outline(&mut self, rect: UiRect, color: Color, thickness: f32, corner_radius: f32);
    fn text(&mut self, pos: UiVec2, text: &str, color: Color, font_key: &str, size: f32);
    fn text_centered(&mut self, rect: UiRect, text: &str, color: Color, font_key: &str, size: f32);
    fn measure_text(&self, text: &str, font_key: &str, size: f32) -> UiVec2;
    fn push_clip_rect(&mut self, rect: UiRect);
    fn pop_clip_rect(&mut self);
    /// Draw an image/texture at a position with size.
    fn image(&mut self, texture: Uuid, pos: UiVec2, size: UiVec2);
    /// Draw an image/texture tinted with a color.
    fn image_tinted(&mut self, texture: Uuid, pos: UiVec2, size: UiVec2, tint: Color);
}

fn approximate_text_size(text: &str, size: f32) -> UiVec2 {
    let glyph_w = size * 0.6;
    let width = text.chars().count() as f32 * glyph_w;
    vec2(width, size)
}

impl Painter for PlutoniumEngine<'_> {
    fn rect(&mut self, rect: UiRect, color: Color, corner_radius: f32) {
        self.draw_rect(rect, color.to_rgba(), corner_radius, None, 0);
    }

    fn rect_outline(&mut self, rect: UiRect, color: Color, thickness: f32, corner_radius: f32) {
        self.draw_rect(
            rect,
            Color::TRANSPARENT.to_rgba(),
            corner_radius,
            Some((color.to_rgba(), thickness)),
            0,
        );
    }

    fn text(&mut self, pos: UiVec2, text: &str, color: Color, font_key: &str, size: f32) {
        let dims = approximate_text_size(text, size);
        let rect = Rectangle::new(pos.x, pos.y, dims.x, dims.y);
        let container = TextContainer::new(rect);
        self.queue_text_with_spacing(
            text,
            font_key,
            pos,
            &container,
            0.0,
            0.0,
            0,
            color.to_rgba(),
            Some(size),
        );
    }

    fn text_centered(&mut self, rect: UiRect, text: &str, color: Color, font_key: &str, size: f32) {
        let dims = approximate_text_size(text, size);
        let pos = vec2(
            rect.center().x - dims.x * 0.5,
            rect.center().y - dims.y * 0.5,
        );
        self.text(pos, text, color, font_key, size);
    }

    fn measure_text(&self, text: &str, _font_key: &str, size: f32) -> UiVec2 {
        approximate_text_size(text, size)
    }

    fn push_clip_rect(&mut self, rect: UiRect) {
        self.push_clip(rect);
    }

    fn pop_clip_rect(&mut self) {
        self.pop_clip();
    }

    fn image(&mut self, texture: Uuid, pos: UiVec2, size: UiVec2) {
        self.draw_texture(
            &texture,
            pos,
            plutonium_engine::DrawParams {
                z: 0,
                rotation: 0.0,
                scale: 1.0,
                tint: [1.0, 1.0, 1.0, 1.0],
            },
        );
        let _ = size;
    }

    fn image_tinted(&mut self, texture: Uuid, pos: UiVec2, size: UiVec2, tint: Color) {
        self.draw_texture(
            &texture,
            pos,
            plutonium_engine::DrawParams {
                z: 0,
                rotation: 0.0,
                scale: 1.0,
                tint: tint.to_rgba(),
            },
        );
        let _ = size;
    }
}
