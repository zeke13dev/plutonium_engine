use crate::utils::{Rectangle, Size};
use crate::PlutoniumEngine;

pub struct NinePatch {
    pub atlas_id: uuid::Uuid,
    pub insets_px: [f32; 4], // left, top, right, bottom
    pub tile_size: Size,
}

#[derive(Clone)]
pub struct FocusRingStyle {
    pub thickness_px: f32,
    pub color: [f32; 4],
    pub corner_radius_px: f32,
    pub inset_px: f32,
}

impl NinePatch {
    pub fn new(atlas_id: uuid::Uuid, tile_size: Size, insets_px: [f32; 4]) -> Self {
        Self {
            atlas_id,
            insets_px,
            tile_size,
        }
    }

    pub fn draw(&self, engine: &mut PlutoniumEngine, dst: Rectangle, z: i32) {
        let [l, t, r, b] = self.insets_px;
        let x = dst.x;
        let y = dst.y;
        let w = dst.width;
        let h = dst.height;
        let mid_w = (w - l - r).max(0.0);
        let mid_h = (h - t - b).max(0.0);

        // Tile indices layout assumed as:
        // 0: top-left, 1: top, 2: top-right, 3: left, 4: center, 5: right, 6: bottom-left, 7: bottom, 8: bottom-right
        let tiles = [
            (0usize, Rectangle::new(x, y, l, t)),
            (1, Rectangle::new(x + l, y, mid_w, t)),
            (2, Rectangle::new(x + l + mid_w, y, r, t)),
            (3, Rectangle::new(x, y + t, l, mid_h)),
            (4, Rectangle::new(x + l, y + t, mid_w, mid_h)),
            (5, Rectangle::new(x + l + mid_w, y + t, r, mid_h)),
            (6, Rectangle::new(x, y + t + mid_h, l, b)),
            (7, Rectangle::new(x + l, y + t + mid_h, mid_w, b)),
            (8, Rectangle::new(x + l + mid_w, y + t + mid_h, r, b)),
        ];

        for (tile, rect) in tiles.iter() {
            engine.draw_atlas_tile_stretched(&self.atlas_id, *tile, *rect, z);
        }
    }
}

pub fn draw_focus_ring(
    engine: &mut crate::PlutoniumEngine,
    dst: Rectangle,
    style: FocusRingStyle,
    z: i32,
) {
    let outer = dst;
    let color = style.color;
    let corner_radius_px = style.corner_radius_px;
    let border = Some((color, style.thickness_px));
    engine.draw_rect(outer, [0.0, 0.0, 0.0, 0.0], corner_radius_px, border, z);
}

// Toggle control (track + thumb) rendered with rect SDFs
pub struct ToggleStyle {
    pub track_off_rgba: [f32; 4],
    pub track_on_rgba: [f32; 4],
    pub border_rgba: [f32; 4],
    pub border_thickness_px: f32,
    pub thumb_rgba: [f32; 4],
    pub focus_ring: Option<FocusRingStyle>,
    pub corner_radius_px: f32, // for track; circle thumb derived from height
}

impl Default for ToggleStyle {
    fn default() -> Self {
        Self {
            track_off_rgba: [0.25, 0.27, 0.32, 1.0],
            track_on_rgba: [0.25, 0.55, 0.35, 1.0],
            border_rgba: [0.15, 0.17, 0.22, 1.0],
            border_thickness_px: 1.0,
            thumb_rgba: [0.95, 0.95, 0.98, 1.0],
            focus_ring: Some(FocusRingStyle {
                thickness_px: 2.0,
                color: [1.0, 0.9, 0.2, 1.0],
                corner_radius_px: 0.0,
                inset_px: 0.0,
            }),
            corner_radius_px: 999.0, // pill by default
        }
    }
}

/// Draw a toggle at `track` rectangle. If `focused` and style has a focus ring, draws it first.
/// The thumb is a circle whose diameter equals the track height, inset by 2 px by default.
pub fn draw_toggle(
    engine: &mut PlutoniumEngine,
    track: Rectangle,
    on: bool,
    focused: bool,
    style: &ToggleStyle,
    z: i32,
) {
    // Focus ring (optional)
    if focused {
        if let Some(mut ring) = style.focus_ring.clone() {
            // If corner radius not provided, derive from track height
            if ring.corner_radius_px <= 0.0 {
                ring.corner_radius_px = style.corner_radius_px.max(track.height * 0.5);
            }
            // Expand by ring thickness as an outer ring
            let pad = ring.thickness_px + ring.inset_px;
            let ring_rect = Rectangle::new(
                track.x - pad,
                track.y - pad,
                track.width + pad * 2.0,
                track.height + pad * 2.0,
            );
            draw_focus_ring(engine, ring_rect, ring, z);
        }
    }

    // Track
    let fill = if on {
        style.track_on_rgba
    } else {
        style.track_off_rgba
    };
    engine.draw_rect(
        track,
        fill,
        style.corner_radius_px.max(track.height * 0.5),
        Some((style.border_rgba, style.border_thickness_px)),
        z + 1,
    );

    // Thumb: circle with diameter equal to track height, with a small padding
    let pad = 2.0f32;
    let d = (track.height - pad * 2.0).max(0.0);
    let cx_off = if on { track.width - track.height } else { 0.0 };
    let thumb = Rectangle::new(track.x + cx_off + pad, track.y + pad, d, d);
    engine.draw_rect(
        thumb,
        style.thumb_rgba,
        d * 0.5,
        Some((style.border_rgba, 1.0)),
        z + 2,
    );
}

// Reusable button background helper with hover/pressed/focused overlays
pub struct ButtonStyle {
    pub base_fill_rgba: [f32; 4],
    pub base_border_rgba: [f32; 4],
    pub base_border_thickness_px: f32,
    pub corner_radius_px: f32,
    pub hover_fill_overlay_rgba: [f32; 4], // additive/lighten overlay
    pub hover_border_rgba: [f32; 4],       // subtle outline on hover
    pub hover_border_thickness_px: f32,
    pub pressed_fill_overlay_rgba: [f32; 4], // darken overlay
    pub focus_ring: Option<FocusRingStyle>,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            base_fill_rgba: [0.20, 0.22, 0.28, 1.0],
            base_border_rgba: [0.14, 0.16, 0.20, 1.0],
            base_border_thickness_px: 1.0,
            corner_radius_px: 10.0,
            hover_fill_overlay_rgba: [1.0, 1.0, 1.0, 0.06],
            hover_border_rgba: [1.0, 1.0, 1.0, 0.12],
            hover_border_thickness_px: 1.0,
            pressed_fill_overlay_rgba: [0.0, 0.0, 0.0, 0.12],
            focus_ring: Some(FocusRingStyle {
                thickness_px: 3.0,
                color: [1.0, 0.9, 0.2, 1.0],
                corner_radius_px: 12.0,
                inset_px: 0.0,
            }),
        }
    }
}

pub struct ButtonVisualState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
}

/// Draws the button background with overlays based on the visual state.
pub fn draw_button_background(
    engine: &mut PlutoniumEngine,
    rect: Rectangle,
    state: &ButtonVisualState,
    style: &ButtonStyle,
    z: i32,
) {
    // Focus ring first
    if state.focused {
        if let Some(ring) = style.focus_ring.clone() {
            let pad = ring.thickness_px + ring.inset_px;
            let ring_rect = Rectangle::new(
                rect.x - pad,
                rect.y - pad,
                rect.width + pad * 2.0,
                rect.height + pad * 2.0,
            );
            draw_focus_ring(engine, ring_rect, ring, z);
        }
    }
    // Base
    engine.draw_rect(
        rect,
        style.base_fill_rgba,
        style.corner_radius_px,
        Some((style.base_border_rgba, style.base_border_thickness_px)),
        z + 1,
    );
    // Overlays
    if state.pressed {
        engine.draw_rect(
            rect,
            style.pressed_fill_overlay_rgba,
            style.corner_radius_px,
            None,
            z + 2,
        );
    } else if state.hovered {
        engine.draw_rect(
            rect,
            style.hover_fill_overlay_rgba,
            style.corner_radius_px,
            Some((style.hover_border_rgba, style.hover_border_thickness_px)),
            z + 2,
        );
    }
}
