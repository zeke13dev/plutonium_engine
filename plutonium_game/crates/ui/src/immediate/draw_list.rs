#![forbid(unsafe_code)]

use crate::immediate::painter::Painter;
use crate::immediate::types::{vec2, Color, RectExt, UiRect, UiVec2};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum DrawCommand {
    Rect {
        rect: UiRect,
        color: Color,
        corner_radius: f32,
    },
    RectOutline {
        rect: UiRect,
        color: Color,
        thickness: f32,
        corner_radius: f32,
    },
    Text {
        pos: UiVec2,
        rect: Option<UiRect>,
        text: String,
        color: Color,
        font_key: String,
        size: f32,
    },
    Image {
        texture: Uuid,
        pos: UiVec2,
        size: UiVec2,
        tint: Color,
    },
    PushClip {
        rect: UiRect,
    },
    PopClip,
}

#[derive(Default)]
pub struct DrawList {
    commands: Vec<DrawCommand>,
}

impl DrawList {
    pub fn new() -> Self {
        DrawList {
            commands: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn insert_rect(&mut self, index: usize, rect: UiRect, color: Color, corner_radius: f32) {
        let cmd = DrawCommand::Rect {
            rect,
            color,
            corner_radius,
        };
        let index = index.min(self.commands.len());
        self.commands.insert(index, cmd);
    }

    pub fn push_command(&mut self, cmd: DrawCommand) {
        self.commands.push(cmd);
    }

    pub fn render(&self, painter: &mut impl Painter) {
        for cmd in &self.commands {
            match cmd {
                DrawCommand::Rect {
                    rect,
                    color,
                    corner_radius,
                } => painter.rect(*rect, *color, *corner_radius),
                DrawCommand::RectOutline {
                    rect,
                    color,
                    thickness,
                    corner_radius,
                } => painter.rect_outline(*rect, *color, *thickness, *corner_radius),
                DrawCommand::Text {
                    pos,
                    rect,
                    text,
                    color,
                    font_key,
                    size,
                } => {
                    if let Some(rect) = rect {
                        painter.text_centered(*rect, text, *color, font_key, *size);
                    } else {
                        painter.text(*pos, text, *color, font_key, *size);
                    }
                }
                DrawCommand::PushClip { rect } => painter.push_clip_rect(*rect),
                DrawCommand::PopClip => painter.pop_clip_rect(),
                DrawCommand::Image {
                    texture,
                    pos,
                    size,
                    tint,
                } => painter.image_tinted(*texture, *pos, *size, *tint),
            }
        }
    }
}

fn approximate_text_size(text: &str, size: f32) -> UiVec2 {
    let glyph_w = size * 0.6;
    let width = text.chars().count() as f32 * glyph_w;
    vec2(width, size)
}

impl Painter for DrawList {
    fn rect(&mut self, rect: UiRect, color: Color, corner_radius: f32) {
        self.commands.push(DrawCommand::Rect {
            rect,
            color,
            corner_radius,
        });
    }

    fn rect_outline(&mut self, rect: UiRect, color: Color, thickness: f32, corner_radius: f32) {
        self.commands.push(DrawCommand::RectOutline {
            rect,
            color,
            thickness,
            corner_radius,
        });
    }

    fn text(&mut self, pos: UiVec2, text: &str, color: Color, font_key: &str, size: f32) {
        self.commands.push(DrawCommand::Text {
            pos,
            rect: None,
            text: text.to_string(),
            color,
            font_key: font_key.to_string(),
            size,
        });
    }

    fn text_centered(&mut self, rect: UiRect, text: &str, color: Color, font_key: &str, size: f32) {
        self.commands.push(DrawCommand::Text {
            pos: rect.center(),
            rect: Some(rect),
            text: text.to_string(),
            color,
            font_key: font_key.to_string(),
            size,
        });
    }

    fn measure_text(&self, text: &str, _font_key: &str, size: f32) -> UiVec2 {
        approximate_text_size(text, size)
    }

    fn push_clip_rect(&mut self, rect: UiRect) {
        self.commands.push(DrawCommand::PushClip { rect });
    }

    fn pop_clip_rect(&mut self) {
        self.commands.push(DrawCommand::PopClip);
    }

    fn image(&mut self, texture: Uuid, pos: UiVec2, size: UiVec2) {
        self.commands.push(DrawCommand::Image {
            texture,
            pos,
            size,
            tint: Color::WHITE,
        });
    }

    fn image_tinted(&mut self, texture: Uuid, pos: UiVec2, size: UiVec2, tint: Color) {
        self.commands.push(DrawCommand::Image {
            texture,
            pos,
            size,
            tint,
        });
    }
}
