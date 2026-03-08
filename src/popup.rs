use crate::utils::{MouseInfo, Rectangle, Size};
use uuid::Uuid;
use winit::keyboard::{Key, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupActionStyle {
    Primary,
    Secondary,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupAction {
    pub id: String,
    pub label: String,
    pub style: PopupActionStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupConfig {
    pub id: String,
    pub title: String,
    pub message: String,
    pub actions: Vec<PopupAction>,
    pub size: PopupSize,
    pub dismiss_on_escape: bool,
    pub dismiss_on_backdrop_click: bool,
    pub auto_dismiss_ms: Option<u64>,
    pub consume_opening_click: bool,
    pub click_anywhere_action_id: Option<String>,
    pub block_input_behind_popup: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupDismissReason {
    Escape,
    BackdropClick,
    Replaced,
    Programmatic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupEvent {
    Opened {
        popup_id: String,
    },
    ActionSelected {
        popup_id: String,
        action_id: String,
    },
    Dismissed {
        popup_id: String,
        reason: PopupDismissReason,
    },
    AutoDismissed {
        popup_id: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PopupLayout {
    pub(crate) viewport: Rectangle,
    pub(crate) panel: Rectangle,
    pub(crate) title_rect: Rectangle,
    pub(crate) message_rect: Rectangle,
    pub(crate) action_rects: Vec<Rectangle>,
}

#[derive(Debug, Clone)]
pub(crate) struct ActivePopup {
    pub(crate) config: PopupConfig,
    elapsed_ms: u64,
    opened_frame: u64,
    awaiting_opening_release: bool,
    pub(crate) custom_panel_rect: Option<Rectangle>,
    pub(crate) custom_object_ids: Vec<Uuid>,
    pub(crate) hovered_action: Option<usize>,
    pub(crate) pressed_action: Option<usize>,
}

pub(crate) struct PopupRuntimeState {
    active: Option<ActivePopup>,
    events: Vec<PopupEvent>,
    pending_object_cleanup: Vec<Uuid>,
    frame_counter: u64,
    prev_lmb_down: bool,
}

impl PopupRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            active: None,
            events: Vec::new(),
            pending_object_cleanup: Vec::new(),
            frame_counter: 0,
            prev_lmb_down: false,
        }
    }

    pub(crate) fn show_popup(&mut self, config: PopupConfig) {
        let config = sanitize_config(config);
        if let Some(existing) = self.active.take() {
            self.pending_object_cleanup
                .extend(existing.custom_object_ids.iter().copied());
            self.events.push(PopupEvent::Dismissed {
                popup_id: existing.config.id,
                reason: PopupDismissReason::Replaced,
            });
        }
        self.active = Some(ActivePopup {
            awaiting_opening_release: config.consume_opening_click,
            config,
            elapsed_ms: 0,
            opened_frame: self.frame_counter,
            custom_panel_rect: None,
            custom_object_ids: Vec::new(),
            hovered_action: None,
            pressed_action: None,
        });
        let popup_id = self.active.as_ref().map(|p| p.config.id.clone()).unwrap();
        self.events.push(PopupEvent::Opened { popup_id });
    }

    pub(crate) fn show_popup_with_objects(
        &mut self,
        config: PopupConfig,
        panel_rect: Rectangle,
        object_ids: Vec<Uuid>,
    ) {
        let config = sanitize_config(config);
        if let Some(existing) = self.active.take() {
            self.pending_object_cleanup
                .extend(existing.custom_object_ids.iter().copied());
            self.events.push(PopupEvent::Dismissed {
                popup_id: existing.config.id,
                reason: PopupDismissReason::Replaced,
            });
        }
        self.active = Some(ActivePopup {
            awaiting_opening_release: config.consume_opening_click,
            config,
            elapsed_ms: 0,
            opened_frame: self.frame_counter,
            custom_panel_rect: Some(panel_rect),
            custom_object_ids: object_ids,
            hovered_action: None,
            pressed_action: None,
        });
        let popup_id = self.active.as_ref().map(|p| p.config.id.clone()).unwrap();
        self.events.push(PopupEvent::Opened { popup_id });
    }

    pub(crate) fn close_popup(&mut self, popup_id: &str) -> bool {
        let should_close = self
            .active
            .as_ref()
            .map(|p| p.config.id == popup_id)
            .unwrap_or(false);
        if !should_close {
            return false;
        }
        let closed = self.active.take().unwrap();
        self.pending_object_cleanup
            .extend(closed.custom_object_ids.iter().copied());
        self.events.push(PopupEvent::Dismissed {
            popup_id: closed.config.id,
            reason: PopupDismissReason::Programmatic,
        });
        true
    }

    pub(crate) fn is_open(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn blocks_input_behind_popup(&self) -> bool {
        self.active
            .as_ref()
            .map(|p| p.config.block_input_behind_popup)
            .unwrap_or(false)
    }

    pub(crate) fn active(&self) -> Option<&ActivePopup> {
        self.active.as_ref()
    }

    pub(crate) fn drain_popup_object_cleanup(&mut self) -> Vec<Uuid> {
        std::mem::take(&mut self.pending_object_cleanup)
    }

    pub(crate) fn drain_events(&mut self) -> Vec<PopupEvent> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn update(
        &mut self,
        mouse_info: Option<MouseInfo>,
        key: &Option<Key>,
        delta_time_s: f32,
        window_size: Size,
    ) {
        let frame = self.frame_counter;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        let lmb_down = mouse_info.map(|m| m.is_lmb_clicked).unwrap_or(false);
        let lmb_pressed = lmb_down && !self.prev_lmb_down;
        let lmb_released = !lmb_down && self.prev_lmb_down;
        self.prev_lmb_down = lmb_down;
        let mouse_pos = mouse_info.map(|m| m.mouse_pos).unwrap_or_default();

        let (
            layout,
            clicked_action_id,
            click_anywhere_action_id,
            dismiss_on_backdrop_click,
            click_interactions_allowed,
            dismiss_on_escape,
        ) = {
            let Some(active) = self.active.as_mut() else {
                return;
            };
            let layout = popup_layout_for_active(active, window_size);
            let hovered_idx = layout
                .action_rects
                .iter()
                .position(|rect| rect.contains(mouse_pos));
            active.hovered_action = hovered_idx;
            if lmb_down {
                active.pressed_action = hovered_idx;
            } else {
                active.pressed_action = None;
            }

            if active.awaiting_opening_release && lmb_released {
                active.awaiting_opening_release = false;
            }

            let click_interactions_allowed =
                frame != active.opened_frame && !active.awaiting_opening_release;
            let clicked_action_id = hovered_idx
                .and_then(|idx| active.config.actions.get(idx))
                .map(|action| action.id.clone());
            (
                layout,
                clicked_action_id,
                active.config.click_anywhere_action_id.clone(),
                active.config.dismiss_on_backdrop_click,
                click_interactions_allowed,
                active.config.dismiss_on_escape,
            )
        };

        if dismiss_on_escape && is_escape_pressed(key) {
            let closed = self.active.take().unwrap();
            self.pending_object_cleanup
                .extend(closed.custom_object_ids.iter().copied());
            self.events.push(PopupEvent::Dismissed {
                popup_id: closed.config.id,
                reason: PopupDismissReason::Escape,
            });
            return;
        }

        if click_interactions_allowed && lmb_pressed {
            if let Some(action_id) = clicked_action_id {
                let closed = self.active.take().unwrap();
                self.pending_object_cleanup
                    .extend(closed.custom_object_ids.iter().copied());
                self.events.push(PopupEvent::ActionSelected {
                    popup_id: closed.config.id,
                    action_id,
                });
                return;
            }

            if let Some(action_id) = click_anywhere_action_id {
                if layout.viewport.contains(mouse_pos) {
                    let closed = self.active.take().unwrap();
                    self.pending_object_cleanup
                        .extend(closed.custom_object_ids.iter().copied());
                    self.events.push(PopupEvent::ActionSelected {
                        popup_id: closed.config.id,
                        action_id,
                    });
                    return;
                }
            }

            if dismiss_on_backdrop_click && !layout.panel.contains(mouse_pos) {
                let closed = self.active.take().unwrap();
                self.pending_object_cleanup
                    .extend(closed.custom_object_ids.iter().copied());
                self.events.push(PopupEvent::Dismissed {
                    popup_id: closed.config.id,
                    reason: PopupDismissReason::BackdropClick,
                });
                return;
            }
        }

        let timeout_ms = self
            .active
            .as_ref()
            .and_then(|popup| popup.config.auto_dismiss_ms);
        if let Some(timeout_ms) = timeout_ms {
            if timeout_ms == 0 {
                let closed = self.active.take().unwrap();
                self.pending_object_cleanup
                    .extend(closed.custom_object_ids.iter().copied());
                self.events.push(PopupEvent::AutoDismissed {
                    popup_id: closed.config.id,
                });
                return;
            }
            let elapsed_ms = (delta_time_s.max(0.0) * 1000.0) as u64;
            if let Some(active) = self.active.as_mut() {
                active.elapsed_ms = active.elapsed_ms.saturating_add(elapsed_ms);
            }
            let timeout_reached = self
                .active
                .as_ref()
                .map(|popup| popup.elapsed_ms >= timeout_ms)
                .unwrap_or(false);
            if timeout_reached {
                let closed = self.active.take().unwrap();
                self.pending_object_cleanup
                    .extend(closed.custom_object_ids.iter().copied());
                self.events.push(PopupEvent::AutoDismissed {
                    popup_id: closed.config.id,
                });
            }
        }
    }
}

fn sanitize_config(mut config: PopupConfig) -> PopupConfig {
    if config.actions.len() > 2 {
        config.actions.truncate(2);
    }
    if config.actions.is_empty() {
        config.actions.push(PopupAction {
            id: "ok".to_string(),
            label: "OK".to_string(),
            style: PopupActionStyle::Primary,
        });
    }
    config
}

fn is_escape_pressed(key: &Option<Key>) -> bool {
    matches!(key, Some(Key::Named(NamedKey::Escape)))
}

fn estimated_message_line_count(message: &str, content_width: f32) -> usize {
    let chars_per_line = ((content_width / 9.0).floor() as usize).max(16);
    let mut total = 0usize;
    for line in message.lines() {
        let len = line.chars().count();
        total += len.div_ceil(chars_per_line).max(1);
    }
    total.max(1).min(12)
}

pub(crate) fn popup_layout_for(config: &PopupConfig, window_size: Size) -> PopupLayout {
    let viewport = Rectangle::new(0.0, 0.0, window_size.width, window_size.height);
    let margin = 24.0f32;
    let max_w = (window_size.width - margin * 2.0).max(220.0);
    let preset_w: f32 = match config.size {
        PopupSize::Small => 360.0,
        PopupSize::Medium => 520.0,
        PopupSize::Large => 720.0,
    };
    let panel_w = preset_w.min(max_w);
    let panel_padding = 20.0f32;
    let title_h = 34.0f32;
    let body_line_h = 22.0f32;
    let body_content_w = (panel_w - panel_padding * 2.0).max(160.0);
    let body_lines = estimated_message_line_count(&config.message, body_content_w);
    let mut body_h = (body_lines as f32) * body_line_h + 8.0;
    let actions_h = 44.0f32;

    let max_panel_h = (window_size.height - margin * 2.0).max(180.0);
    let mut panel_h = panel_padding + title_h + 12.0 + body_h + 20.0 + actions_h + panel_padding;
    if panel_h > max_panel_h {
        let overflow = panel_h - max_panel_h;
        body_h = (body_h - overflow).max(body_line_h * 2.0);
        panel_h = panel_padding + title_h + 12.0 + body_h + 20.0 + actions_h + panel_padding;
    }

    let panel = Rectangle::new(
        ((window_size.width - panel_w) * 0.5).floor(),
        ((window_size.height - panel_h) * 0.5).floor(),
        panel_w,
        panel_h,
    );

    let title_rect = Rectangle::new(
        panel.x + panel_padding,
        panel.y + panel_padding,
        panel.width - panel_padding * 2.0,
        title_h,
    );
    let message_rect = Rectangle::new(
        panel.x + panel_padding,
        title_rect.y + title_rect.height + 12.0,
        panel.width - panel_padding * 2.0,
        body_h,
    );

    let action_count = config.actions.len().clamp(1, 2);
    let action_gap = 12.0f32;
    let actions_y = panel.y + panel.height - panel_padding - actions_h;
    let action_rects = if action_count == 1 {
        let w = (panel.width - panel_padding * 2.0).min(180.0);
        vec![Rectangle::new(
            panel.x + (panel.width - w) * 0.5,
            actions_y,
            w,
            actions_h,
        )]
    } else {
        let w = ((panel.width - panel_padding * 2.0 - action_gap) * 0.5).max(80.0);
        vec![
            Rectangle::new(panel.x + panel_padding, actions_y, w, actions_h),
            Rectangle::new(
                panel.x + panel_padding + w + action_gap,
                actions_y,
                w,
                actions_h,
            ),
        ]
    };

    PopupLayout {
        viewport,
        panel,
        title_rect,
        message_rect,
        action_rects,
    }
}

pub(crate) fn popup_layout_for_active(active: &ActivePopup, window_size: Size) -> PopupLayout {
    if let Some(panel) = active.custom_panel_rect {
        return PopupLayout {
            viewport: Rectangle::new(0.0, 0.0, window_size.width, window_size.height),
            panel,
            title_rect: panel,
            message_rect: panel,
            action_rects: Vec::new(),
        };
    }
    popup_layout_for(&active.config, window_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Position;

    fn base_config() -> PopupConfig {
        PopupConfig {
            id: "p1".to_string(),
            title: "Title".to_string(),
            message: "Message".to_string(),
            actions: vec![
                PopupAction {
                    id: "ok".to_string(),
                    label: "OK".to_string(),
                    style: PopupActionStyle::Primary,
                },
                PopupAction {
                    id: "cancel".to_string(),
                    label: "Cancel".to_string(),
                    style: PopupActionStyle::Secondary,
                },
            ],
            size: PopupSize::Medium,
            dismiss_on_escape: true,
            dismiss_on_backdrop_click: true,
            auto_dismiss_ms: None,
            consume_opening_click: false,
            click_anywhere_action_id: None,
            block_input_behind_popup: true,
        }
    }

    fn mouse(x: f32, y: f32, down: bool) -> MouseInfo {
        MouseInfo {
            is_lmb_clicked: down,
            is_rmb_clicked: false,
            is_mmb_clicked: false,
            mouse_pos: Position { x, y },
            scroll_dx: 0.0,
            scroll_dy: 0.0,
        }
    }

    #[test]
    fn replacing_popup_emits_replaced_event() {
        let mut state = PopupRuntimeState::new();
        state.show_popup(base_config());
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::Opened {
                popup_id: "p1".to_string()
            }]
        );
        let mut next = base_config();
        next.id = "p2".to_string();
        state.show_popup(next);

        let events = state.drain_events();
        assert_eq!(
            events,
            vec![
                PopupEvent::Dismissed {
                    popup_id: "p1".to_string(),
                    reason: PopupDismissReason::Replaced
                },
                PopupEvent::Opened {
                    popup_id: "p2".to_string()
                }
            ]
        );
        assert!(state.is_open());
        assert_eq!(state.active().unwrap().config.id, "p2");
    }

    #[test]
    fn close_popup_is_programmatic_and_id_scoped() {
        let mut state = PopupRuntimeState::new();
        state.show_popup(base_config());
        let _ = state.drain_events();
        assert!(!state.close_popup("wrong"));
        assert!(state.close_popup("p1"));
        assert!(!state.is_open());
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::Dismissed {
                popup_id: "p1".to_string(),
                reason: PopupDismissReason::Programmatic
            }]
        );
    }

    #[test]
    fn action_click_emits_action_selected() {
        let mut state = PopupRuntimeState::new();
        let cfg = base_config();
        let layout = popup_layout_for(
            &cfg,
            Size {
                width: 1280.0,
                height: 720.0,
            },
        );
        let button = layout.action_rects[0];

        state.show_popup(cfg);
        let _ = state.drain_events();
        state.update(
            Some(mouse(button.x + 4.0, button.y + 4.0, false)),
            &None,
            0.016,
            Size {
                width: 1280.0,
                height: 720.0,
            },
        );
        state.update(
            Some(mouse(button.x + 4.0, button.y + 4.0, true)),
            &None,
            0.016,
            Size {
                width: 1280.0,
                height: 720.0,
            },
        );

        assert!(!state.is_open());
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::ActionSelected {
                popup_id: "p1".to_string(),
                action_id: "ok".to_string()
            }]
        );
    }

    #[test]
    fn escape_dismiss_respects_flag() {
        let mut state = PopupRuntimeState::new();
        let mut cfg = base_config();
        cfg.dismiss_on_escape = false;
        state.show_popup(cfg);
        let _ = state.drain_events();
        state.update(
            None,
            &Some(Key::Named(NamedKey::Escape)),
            0.016,
            Size {
                width: 800.0,
                height: 600.0,
            },
        );
        assert!(state.is_open());
        assert!(state.drain_events().is_empty());

        let mut cfg2 = base_config();
        cfg2.id = "p2".to_string();
        cfg2.dismiss_on_escape = true;
        state.show_popup(cfg2);
        let _ = state.drain_events();
        state.update(
            None,
            &Some(Key::Named(NamedKey::Escape)),
            0.016,
            Size {
                width: 800.0,
                height: 600.0,
            },
        );
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::Dismissed {
                popup_id: "p2".to_string(),
                reason: PopupDismissReason::Escape
            }]
        );
    }

    #[test]
    fn backdrop_click_dismiss_respects_flag() {
        let mut state = PopupRuntimeState::new();
        let mut cfg = base_config();
        cfg.dismiss_on_backdrop_click = false;
        state.show_popup(cfg);
        let _ = state.drain_events();
        state.update(
            Some(mouse(2.0, 2.0, true)),
            &None,
            0.016,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );
        assert!(state.is_open());
        assert!(state.drain_events().is_empty());

        let mut cfg2 = base_config();
        cfg2.id = "p2".to_string();
        cfg2.dismiss_on_backdrop_click = true;
        state.show_popup(cfg2);
        let _ = state.drain_events();
        state.update(
            Some(mouse(2.0, 2.0, false)),
            &None,
            0.016,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );
        state.update(
            Some(mouse(2.0, 2.0, true)),
            &None,
            0.016,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::Dismissed {
                popup_id: "p2".to_string(),
                reason: PopupDismissReason::BackdropClick
            }]
        );
    }

    #[test]
    fn auto_dismiss_fires_when_timeout_reached() {
        let mut state = PopupRuntimeState::new();
        let mut cfg = base_config();
        cfg.auto_dismiss_ms = Some(20);
        state.show_popup(cfg);
        let _ = state.drain_events();
        state.update(
            None,
            &None,
            0.010,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );
        assert!(state.is_open());
        state.update(
            None,
            &None,
            0.011,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );
        assert!(!state.is_open());
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::AutoDismissed {
                popup_id: "p1".to_string()
            }]
        );
    }

    #[test]
    fn actions_are_sanitized_to_v1_limit() {
        let mut state = PopupRuntimeState::new();
        let mut cfg = base_config();
        cfg.actions.push(PopupAction {
            id: "extra".to_string(),
            label: "Extra".to_string(),
            style: PopupActionStyle::Danger,
        });
        state.show_popup(cfg);
        let _ = state.drain_events();
        assert_eq!(state.active().unwrap().config.actions.len(), 2);

        let mut cfg2 = base_config();
        cfg2.id = "p2".to_string();
        cfg2.actions.clear();
        state.show_popup(cfg2);
        let _ = state.drain_events();
        assert_eq!(state.active().unwrap().config.actions.len(), 1);
        assert_eq!(state.active().unwrap().config.actions[0].id, "ok");
    }

    #[test]
    fn opening_frame_click_cannot_dismiss_popup() {
        let mut state = PopupRuntimeState::new();
        let mut cfg = base_config();
        cfg.dismiss_on_backdrop_click = true;
        state.show_popup(cfg);
        let _ = state.drain_events();

        state.update(
            Some(mouse(2.0, 2.0, true)),
            &None,
            0.016,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );

        assert!(state.is_open());
        assert!(state.drain_events().is_empty());
    }

    #[test]
    fn consume_opening_click_waits_for_release_before_click_actions() {
        let mut state = PopupRuntimeState::new();
        let mut cfg = base_config();
        cfg.consume_opening_click = true;
        state.show_popup(cfg);
        let _ = state.drain_events();

        state.update(
            Some(mouse(2.0, 2.0, true)),
            &None,
            0.016,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );
        assert!(state.is_open());

        state.update(
            Some(mouse(2.0, 2.0, false)),
            &None,
            0.016,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );
        assert!(state.is_open());

        state.update(
            Some(mouse(2.0, 2.0, true)),
            &None,
            0.016,
            Size {
                width: 1000.0,
                height: 700.0,
            },
        );
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::Dismissed {
                popup_id: "p1".to_string(),
                reason: PopupDismissReason::BackdropClick
            }]
        );
    }

    #[test]
    fn click_anywhere_action_fires_on_backdrop_and_panel() {
        let mut state = PopupRuntimeState::new();
        let mut cfg = base_config();
        cfg.click_anywhere_action_id = Some("ok".to_string());
        state.show_popup(cfg.clone());
        let _ = state.drain_events();
        state.update(
            Some(mouse(2.0, 2.0, false)),
            &None,
            0.016,
            Size {
                width: 1200.0,
                height: 800.0,
            },
        );

        state.update(
            Some(mouse(2.0, 2.0, true)),
            &None,
            0.016,
            Size {
                width: 1200.0,
                height: 800.0,
            },
        );
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::ActionSelected {
                popup_id: "p1".to_string(),
                action_id: "ok".to_string()
            }]
        );

        cfg.id = "p2".to_string();
        state.show_popup(cfg);
        let _ = state.drain_events();
        let layout = popup_layout_for(
            state.active().as_ref().map(|a| &a.config).unwrap(),
            Size {
                width: 1200.0,
                height: 800.0,
            },
        );
        state.update(
            Some(mouse(layout.panel.x + 4.0, layout.panel.y + 4.0, false)),
            &None,
            0.016,
            Size {
                width: 1200.0,
                height: 800.0,
            },
        );
        state.update(
            Some(mouse(layout.panel.x + 4.0, layout.panel.y + 4.0, true)),
            &None,
            0.016,
            Size {
                width: 1200.0,
                height: 800.0,
            },
        );
        assert_eq!(
            state.drain_events(),
            vec![PopupEvent::ActionSelected {
                popup_id: "p2".to_string(),
                action_id: "ok".to_string()
            }]
        );
    }

    #[test]
    fn popup_can_disable_background_input_blocking() {
        let mut state = PopupRuntimeState::new();
        let mut cfg = base_config();
        cfg.block_input_behind_popup = false;
        state.show_popup(cfg);
        assert!(!state.blocks_input_behind_popup());
    }

    #[test]
    fn custom_popup_replacement_enqueues_object_cleanup() {
        let mut state = PopupRuntimeState::new();
        state.show_popup_with_objects(
            base_config(),
            Rectangle::new(100.0, 100.0, 300.0, 200.0),
            vec![Uuid::nil()],
        );
        let _ = state.drain_events();
        let mut next = base_config();
        next.id = "p2".to_string();
        state.show_popup(next);

        assert_eq!(state.drain_popup_object_cleanup(), vec![Uuid::nil()]);
    }

    #[test]
    fn custom_popup_uses_manual_panel_bounds() {
        let mut state = PopupRuntimeState::new();
        let panel = Rectangle::new(120.0, 140.0, 420.0, 260.0);
        state.show_popup_with_objects(base_config(), panel, vec![]);
        let active = state.active().unwrap();
        let layout = popup_layout_for_active(
            active,
            Size {
                width: 1280.0,
                height: 720.0,
            },
        );
        assert_eq!(layout.panel, panel);
        assert!(layout.action_rects.is_empty());
    }
}
