#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Default, Debug, Clone)]
pub struct InputState {
    // Keys are identified by string names (e.g., "KeyA", "Space")
    pub pressed: HashSet<String>,
    pub just_pressed: HashSet<String>,
    pub just_released: HashSet<String>,
    prev_pressed: HashSet<String>,
    // Mouse
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub lmb_down: bool,
    pub lmb_just_pressed: bool,
    pub lmb_just_released: bool,
    prev_lmb_down: bool,
}

impl InputState {
    pub fn update_from_keys<I: IntoIterator<Item = String>>(&mut self, keys: I) {
        let current: HashSet<String> = keys.into_iter().collect();
        // compute edges
        self.just_pressed = current.difference(&self.prev_pressed).cloned().collect();
        self.just_released = self.prev_pressed.difference(&current).cloned().collect();
        self.pressed = current.clone();
        self.prev_pressed = current;
    }

    pub fn update_mouse(&mut self, x: f32, y: f32, lmb_down_now: bool) {
        self.mouse_x = x;
        self.mouse_y = y;
        self.lmb_just_pressed = lmb_down_now && !self.prev_lmb_down;
        self.lmb_just_released = !lmb_down_now && self.prev_lmb_down;
        self.lmb_down = lmb_down_now;
        self.prev_lmb_down = lmb_down_now;
    }

    pub fn is_pressed(&self, key: &str) -> bool {
        self.pressed.contains(key)
    }
    pub fn is_just_pressed(&self, key: &str) -> bool {
        self.just_pressed.contains(key)
    }
}

#[derive(Default, Debug, Clone)]
pub struct ActionMap {
    // action name -> keys that trigger it
    pub bindings: std::collections::HashMap<String, Vec<String>>,
}

impl ActionMap {
    pub fn bind(&mut self, action: &str, key: &str) {
        self.bindings
            .entry(action.to_string())
            .or_default()
            .push(key.to_string());
    }
    pub fn action_just_pressed(&self, input: &InputState, action: &str) -> bool {
        if let Some(keys) = self.bindings.get(action) {
            keys.iter().any(|k| input.is_just_pressed(k))
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrameInputRecord {
    pub pressed_keys: Vec<String>,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub lmb_down: bool,
    pub committed_text: Vec<String>,
}

impl FrameInputRecord {
    pub fn from_state_and_commits(input: &InputState, commits: &[String]) -> Self {
        Self {
            pressed_keys: input.pressed.iter().cloned().collect(),
            mouse_x: input.mouse_x,
            mouse_y: input.mouse_y,
            lmb_down: input.lmb_down,
            committed_text: commits.to_vec(),
        }
    }
    pub fn apply_to(&self, input: &mut InputState) {
        input.update_from_keys(self.pressed_keys.clone());
        input.update_mouse(self.mouse_x, self.mouse_y, self.lmb_down);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplayScript {
    pub frames: Vec<FrameInputRecord>,
}

impl ReplayScript {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
    pub fn from_json(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_edge_detection_for_keys() {
        let mut input = InputState::default();
        input.update_from_keys(vec!["Space".to_string()]);
        assert!(input.is_pressed("Space"));
        assert!(input.is_just_pressed("Space"));
        // Next frame: still held, no longer just_pressed
        input.update_from_keys(vec!["Space".to_string()]);
        assert!(input.is_pressed("Space"));
        assert!(!input.is_just_pressed("Space"));
        // Release
        input.update_from_keys(Vec::<String>::new());
        assert!(!input.is_pressed("Space"));
        assert!(input.just_released.contains("Space"));
    }

    #[test]
    fn input_mouse_edges() {
        let mut input = InputState::default();
        input.update_mouse(10.0, 20.0, true);
        assert!(input.lmb_down);
        assert!(input.lmb_just_pressed);
        input.update_mouse(15.0, 25.0, true);
        assert!(input.lmb_down);
        assert!(!input.lmb_just_pressed);
        input.update_mouse(15.0, 25.0, false);
        assert!(!input.lmb_down);
        assert!(input.lmb_just_released);
    }

    #[test]
    fn action_map_works_with_edges() {
        let mut input = InputState::default();
        let mut map = ActionMap::default();
        map.bind("jump", "Space");
        input.update_from_keys(vec!["Space".to_string()]);
        assert!(map.action_just_pressed(&input, "jump"));
        // Held next frame: not just pressed
        input.update_from_keys(vec!["Space".to_string()]);
        assert!(!map.action_just_pressed(&input, "jump"));
    }

    #[test]
    fn frame_record_and_apply_roundtrip() {
        let mut input = InputState::default();
        input.update_from_keys(vec!["Enter".to_string(), "KeyA".to_string()]);
        input.update_mouse(42.0, 24.0, true);
        let commits = vec!["H".to_string(), "i".to_string()];
        let rec = FrameInputRecord::from_state_and_commits(&input, &commits);
        let json = serde_json::to_string(&rec).unwrap();
        let parsed: FrameInputRecord = serde_json::from_str(&json).unwrap();
        let mut input2 = InputState::default();
        parsed.apply_to(&mut input2);
        assert!(input2.is_pressed("Enter") && input2.is_pressed("KeyA"));
        assert_eq!(input2.mouse_x, 42.0);
        assert_eq!(input2.mouse_y, 24.0);
        assert!(input2.lmb_down);
    }
}
