//! Popup overlay rendering — draws the active popup scrim, panel, buttons, and text.
//!
//! All items in this module are either `pub(crate)` (cross-called from lib.rs) or
//! private (only called within this module by `render_popup_overlay`).

use crate::pluto_objects::text2d::{HorizontalAlignment, TextContainer, VerticalAlignment};
use crate::popup::{popup_layout_for_active, PopupActionStyle};
use crate::PlutoniumEngine;

impl<'a> PlutoniumEngine<'a> {
    pub(crate) fn render_popup_overlay(&mut self) {
        let Some(active) = self.popup_state.active().cloned() else {
            return;
        };
        let layout = popup_layout_for_active(&active, self.screen_space_viewport_rect().size());
        let is_custom = active.custom_panel_rect.is_some();

        let z_scrim = 900_000;
        let z_panel = z_scrim + 10;
        let z_text = z_scrim + 20;
        let z_button = z_scrim + 30;
        let z_button_text = z_scrim + 40;

        let world_viewport = self.halo_world_rect_from_screen_rect(layout.viewport);
        let world_panel = self.halo_world_rect_from_screen_rect(layout.panel);
        let world_title_rect = self.halo_world_rect_from_screen_rect(layout.title_rect);
        let world_message_rect = self.halo_world_rect_from_screen_rect(layout.message_rect);

        self.draw_rect(world_viewport, [0.0, 0.0, 0.0, 0.62], 0.0, None, z_scrim);
        self.draw_rect(
            world_panel,
            [0.11, 0.13, 0.17, 1.0],
            14.0,
            Some(([0.30, 0.34, 0.42, 1.0], 1.0)),
            z_panel,
        );

        if is_custom {
            for object_id in &active.custom_object_ids {
                if let Some(object) = self.pluto_objects.get(object_id).cloned() {
                    object.borrow().render(self);
                }
            }
            return;
        }

        for (idx, action_rect) in layout.action_rects.iter().enumerate() {
            let Some(action) = active.config.actions.get(idx) else {
                continue;
            };
            let world_action_rect = self.halo_world_rect_from_screen_rect(*action_rect);
            let (mut fill, border) = Self::popup_button_colors(action.style);
            if active.hovered_action == Some(idx) {
                fill = [
                    (fill[0] + 0.08).min(1.0),
                    (fill[1] + 0.08).min(1.0),
                    (fill[2] + 0.08).min(1.0),
                    1.0,
                ];
            }
            if active.pressed_action == Some(idx) {
                fill = [fill[0] * 0.85, fill[1] * 0.85, fill[2] * 0.85, 1.0];
            }
            self.draw_rect(world_action_rect, fill, 10.0, Some((border, 1.0)), z_button);
        }

        if let Some(font_key) = self.popup_font_key() {
            let title_container = TextContainer::new(world_title_rect)
                .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                .with_padding(0.0);
            self.queue_text_with_spacing(
                &active.config.title,
                &font_key,
                world_title_rect.pos(),
                &title_container,
                0.0,
                0.0,
                z_text,
                [0.96, 0.97, 1.0, 1.0],
                Some(26.0),
            );

            let message_container = TextContainer::new(world_message_rect)
                .with_alignment(HorizontalAlignment::Left, VerticalAlignment::Top)
                .with_padding(0.0);
            let wrapped_message = self.wrap_popup_message_text(
                &active.config.message,
                &font_key,
                18.0,
                world_message_rect.width,
            );
            self.queue_text_with_spacing(
                &wrapped_message,
                &font_key,
                world_message_rect.pos(),
                &message_container,
                0.0,
                0.0,
                z_text,
                [0.86, 0.88, 0.92, 1.0],
                Some(18.0),
            );

            for (idx, action_rect) in layout.action_rects.iter().enumerate() {
                let Some(action) = active.config.actions.get(idx) else {
                    continue;
                };
                let world_action_rect = self.halo_world_rect_from_screen_rect(*action_rect);
                let action_container = TextContainer::new(world_action_rect)
                    .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                    .with_padding(0.0);
                self.queue_text_with_spacing(
                    &action.label,
                    &font_key,
                    world_action_rect.pos(),
                    &action_container,
                    0.0,
                    0.0,
                    z_button_text,
                    [0.98, 0.98, 1.0, 1.0],
                    Some(16.0),
                );
            }
        }
    }

    fn popup_font_key(&self) -> Option<String> {
        if self.loaded_fonts.contains_key("roboto") {
            return Some("roboto".to_string());
        }
        if self.loaded_fonts.contains_key("default") {
            return Some("default".to_string());
        }
        let mut keys: Vec<&str> = self.loaded_fonts.keys().map(String::as_str).collect();
        keys.sort_unstable();
        keys.first().map(|k| (*k).to_string())
    }

    fn popup_button_colors(style: PopupActionStyle) -> ([f32; 4], [f32; 4]) {
        match style {
            PopupActionStyle::Primary => ([0.20, 0.45, 0.88, 1.0], [0.14, 0.33, 0.70, 1.0]),
            PopupActionStyle::Secondary => ([0.24, 0.27, 0.32, 1.0], [0.16, 0.18, 0.22, 1.0]),
            PopupActionStyle::Danger => ([0.70, 0.22, 0.18, 1.0], [0.55, 0.16, 0.14, 1.0]),
        }
    }

    fn wrap_popup_message_text(
        &self,
        message: &str,
        font_key: &str,
        font_size: f32,
        max_width: f32,
    ) -> String {
        if message.is_empty() || max_width <= 0.0 {
            return String::new();
        }

        let mut wrapped_lines: Vec<String> = Vec::new();
        for segment in message.split('\n') {
            if segment.trim().is_empty() {
                wrapped_lines.push(String::new());
                continue;
            }

            let mut current_line = String::new();
            for word in segment.split_whitespace() {
                let mut remaining_word = word.to_string();
                loop {
                    let candidate = if current_line.is_empty() {
                        remaining_word.clone()
                    } else {
                        format!("{} {}", current_line, remaining_word)
                    };
                    let candidate_width = self
                        .measure_text(&candidate, font_key, 0.0, 0.0, Some(font_size))
                        .0;
                    if candidate_width <= max_width {
                        current_line = candidate;
                        break;
                    }

                    if !current_line.is_empty() {
                        wrapped_lines.push(current_line);
                        current_line = String::new();
                        continue;
                    }

                    let chunk = Self::largest_fitting_prefix(&remaining_word, max_width, |s| {
                        self.measure_text(s, font_key, 0.0, 0.0, Some(font_size)).0
                    });
                    if chunk.is_empty() {
                        if let Some(first_char) = remaining_word.chars().next() {
                            wrapped_lines.push(first_char.to_string());
                            remaining_word = remaining_word[first_char.len_utf8()..].to_string();
                            if remaining_word.is_empty() {
                                break;
                            }
                            continue;
                        }
                        break;
                    }

                    wrapped_lines.push(chunk.clone());
                    remaining_word = remaining_word[chunk.len()..].to_string();
                    if remaining_word.is_empty() {
                        break;
                    }
                }
            }

            if !current_line.is_empty() {
                wrapped_lines.push(current_line);
            }
        }

        wrapped_lines.join("\n")
    }

    fn largest_fitting_prefix<F>(text: &str, max_width: f32, width_fn: F) -> String
    where
        F: Fn(&str) -> f32,
    {
        let mut best_end = 0usize;
        let mut ends: Vec<usize> = text.char_indices().skip(1).map(|(idx, _)| idx).collect();
        ends.push(text.len());
        for end in ends {
            let candidate = &text[..end];
            if width_fn(candidate) <= max_width {
                best_end = end;
            } else {
                break;
            }
        }
        if best_end == 0 {
            return String::new();
        }
        text[..best_end].to_string()
    }
}
