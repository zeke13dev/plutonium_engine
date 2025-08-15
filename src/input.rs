use crate::app::FrameContext;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ButtonSource {
    Key(String), // Debug-format of winit::keyboard::Key (e.g., "Character(\"A\")", "Named(Enter)")
    MouseLeft,
    MouseRight,
    MouseMiddle,
    GamepadButton(String), // Placeholder for future gamepad support
}

#[derive(Debug, Clone)]
pub struct ButtonBinding {
    pub source: ButtonSource,
}

#[derive(Debug, Clone)]
pub enum AxisSource {
    KeyPair { negative: String, positive: String }, // keys by Debug-format
    GamepadAxis { name: String },                   // Placeholder
}

#[derive(Debug, Clone)]
pub struct AxisBinding {
    pub source: AxisSource,
    pub scale: f32,
    pub deadzone: f32,
}

#[derive(Debug, Default, Clone)]
pub struct ActionMap {
    button_bindings: HashMap<String, Vec<ButtonBinding>>, // action name -> bindings
    axis_bindings: HashMap<String, Vec<AxisBinding>>,     // axis name -> bindings
}

impl ActionMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind_button(&mut self, action: impl Into<String>, source: ButtonSource) {
        self.button_bindings
            .entry(action.into())
            .or_default()
            .push(ButtonBinding { source });
    }

    pub fn bind_axis(
        &mut self,
        axis: impl Into<String>,
        source: AxisSource,
        scale: f32,
        deadzone: f32,
    ) {
        self.axis_bindings
            .entry(axis.into())
            .or_default()
            .push(AxisBinding {
                source,
                scale,
                deadzone,
            });
    }

    // Resolve pressed actions and axis values for this frame using FrameContext events
    // Buttons are edge-triggered: pressed if any binding fired this frame
    // Axes from KeyPair: -1 for negative key, +1 for positive key when present in this frame
    pub fn resolve(&self, frame: &FrameContext) -> (HashSet<String>, HashMap<String, f32>) {
        let mut pressed: HashSet<String> = HashSet::new();
        let mut axes: HashMap<String, f32> = HashMap::new();

        // Precompute key strings for this frame
        let key_strs: Vec<String> = frame
            .pressed_keys
            .iter()
            .map(|k| format!("{:?}", k))
            .collect();

        // Buttons
        for (action, binds) in &self.button_bindings {
            for b in binds {
                let fired = match &b.source {
                    ButtonSource::Key(s) => key_strs.iter().any(|ks| ks == s),
                    ButtonSource::MouseLeft => frame.mouse_info.is_lmb_clicked,
                    ButtonSource::MouseRight => frame.mouse_info.is_rmb_clicked,
                    ButtonSource::MouseMiddle => frame.mouse_info.is_mmb_clicked,
                    ButtonSource::GamepadButton(_name) => false, // not implemented yet
                };
                if fired {
                    pressed.insert(action.clone());
                    break;
                }
            }
        }

        // Axes
        for (axis, binds) in &self.axis_bindings {
            let mut value = 0.0f32;
            for b in binds {
                match &b.source {
                    AxisSource::KeyPair { negative, positive } => {
                        let neg = key_strs.iter().any(|ks| ks == negative);
                        let pos = key_strs.iter().any(|ks| ks == positive);
                        let v = match (neg, pos) {
                            (true, false) => -1.0,
                            (false, true) => 1.0,
                            _ => 0.0,
                        };
                        value += v * b.scale;
                    }
                    AxisSource::GamepadAxis { .. } => {
                        // not implemented; placeholder
                    }
                }
            }
            // Deadzone
            if value.abs() < binds.iter().map(|b| b.deadzone).fold(0.0, f32::max) {
                value = 0.0;
            }
            axes.insert(axis.clone(), value.clamp(-1.0, 1.0));
        }

        (pressed, axes)
    }
}
