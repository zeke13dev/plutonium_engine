#![forbid(unsafe_code)]

use plutonium_engine::utils::{Position, Rectangle};
use std::hash::{Hash, Hasher};

pub type UiVec2 = Position;
pub type UiRect = Rectangle;

pub fn vec2(x: f32, y: f32) -> UiVec2 {
    UiVec2 { x, y }
}

pub fn rect_from_min_max(min: UiVec2, max: UiVec2) -> UiRect {
    UiRect::new(min.x, min.y, max.x - min.x, max.y - min.y)
}

pub fn rect_from_center_size(center: UiVec2, size: UiVec2) -> UiRect {
    UiRect::new(
        center.x - size.x * 0.5,
        center.y - size.y * 0.5,
        size.x,
        size.y,
    )
}

pub trait Vec2Ext {
    fn length(&self) -> f32;
    fn distance(&self, other: UiVec2) -> f32;
    fn normalize(&self) -> UiVec2;
}

impl Vec2Ext for UiVec2 {
    fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn distance(&self, other: UiVec2) -> f32 {
        (*self - other).length()
    }

    fn normalize(&self) -> UiVec2 {
        let len = self.length();
        if len <= f32::EPSILON {
            UiVec2 { x: 0.0, y: 0.0 }
        } else {
            *self / len
        }
    }
}

pub trait RectExt {
    fn min(&self) -> UiVec2;
    fn max(&self) -> UiVec2;
    fn center(&self) -> UiVec2;
    fn width(&self) -> f32;
    fn height(&self) -> f32;
    fn left(&self) -> f32;
    fn right(&self) -> f32;
    fn top(&self) -> f32;
    fn bottom(&self) -> f32;
    fn contains(&self, point: UiVec2) -> bool;
    fn expand(&self, padding: f32) -> UiRect;
    fn shrink(&self, padding: f32) -> UiRect;
}

impl RectExt for UiRect {
    fn min(&self) -> UiVec2 {
        UiVec2 {
            x: self.x,
            y: self.y,
        }
    }

    fn max(&self) -> UiVec2 {
        UiVec2 {
            x: self.x + self.width,
            y: self.y + self.height,
        }
    }

    fn center(&self) -> UiVec2 {
        UiVec2 {
            x: self.x + self.width * 0.5,
            y: self.y + self.height * 0.5,
        }
    }

    fn width(&self) -> f32 {
        self.width
    }

    fn height(&self) -> f32 {
        self.height
    }

    fn left(&self) -> f32 {
        self.x
    }

    fn right(&self) -> f32 {
        self.x + self.width
    }

    fn top(&self) -> f32 {
        self.y
    }

    fn bottom(&self) -> f32 {
        self.y + self.height
    }

    fn contains(&self, point: UiVec2) -> bool {
        point.x >= self.left()
            && point.x <= self.right()
            && point.y >= self.top()
            && point.y <= self.bottom()
    }

    fn expand(&self, padding: f32) -> UiRect {
        UiRect::new(
            self.x - padding,
            self.y - padding,
            self.width + padding * 2.0,
            self.height + padding * 2.0,
        )
    }

    fn shrink(&self, padding: f32) -> UiRect {
        self.expand(-padding)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub fn with_alpha(self, alpha: f32) -> Self {
        Color { a: alpha, ..self }
    }

    pub fn to_rgba(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn lighten(self, amount: f32) -> Self {
        Color {
            r: (self.r + amount).clamp(0.0, 1.0),
            g: (self.g + amount).clamp(0.0, 1.0),
            b: (self.b + amount).clamp(0.0, 1.0),
            a: self.a,
        }
    }

    /// Linear interpolation between two colors.
    pub fn lerp(self, other: Color, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Darken color by amount (0.0 to 1.0).
    pub fn darken(self, amount: f32) -> Self {
        self.lighten(-amount)
    }

    /// Primary red color.
    pub const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    /// Primary green color.
    pub const GREEN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    /// Primary blue color.
    pub const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    /// Bright yellow color.
    pub const YELLOW: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    /// Bright orange color.
    pub const ORANGE: Color = Color {
        r: 1.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    };
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct WidgetId(u64);

impl WidgetId {
    pub fn new(id: u64) -> Self {
        WidgetId(id)
    }

    pub fn from_hash(hash: impl Hash) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hash.hash(&mut hasher);
        WidgetId(hasher.finish())
    }
}
