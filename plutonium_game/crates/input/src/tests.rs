#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_detection() {
        let mut input = InputState::default();
        input.update_from_keys(vec!["Space".to_string()]);
        assert!(input.is_just_pressed("Space"));
        assert!(input.is_pressed("Space"));

        // next frame, same key held: not just pressed
        input.update_from_keys(vec!["Space".to_string()]);
        assert!(!input.is_just_pressed("Space"));

        // release
        input.update_from_keys(std::iter::empty());
        assert!(input.just_released.contains("Space"));
        assert!(!input.is_pressed("Space"));
    }

    #[test]
    fn mouse_edges() {
        let mut input = InputState::default();
        input.update_mouse(10.0, 20.0, false);
        assert!(!input.lmb_down);
        assert!(!input.lmb_just_pressed);
        input.update_mouse(10.0, 20.0, true);
        assert!(input.lmb_down);
        assert!(input.lmb_just_pressed);
        input.update_mouse(10.0, 20.0, true);
        assert!(input.lmb_down);
        assert!(!input.lmb_just_pressed);
        input.update_mouse(10.0, 20.0, false);
        assert!(input.lmb_just_released);
    }
}


