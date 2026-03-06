# Text Style Struct Example

Based on the engine's text rendering API, here's what a `TextStyle` struct should look like for a UI library:

## Basic TextStyle

```rust
use plutonium_engine::pluto_objects::text2d::{HorizontalAlignment, VerticalAlignment};

#[derive(Clone, Debug)]
pub struct TextStyle {
    /// Font key identifier (must be loaded via engine.load_font())
    pub font_key: String,
    
    /// Font size in logical pixels
    pub font_size: f32,
    
    /// Text color (RGBA, 0.0-1.0 range)
    pub color: [f32; 4],
    
    /// Horizontal text alignment
    pub h_align: HorizontalAlignment,
    
    /// Vertical text alignment
    pub v_align: VerticalAlignment,
    
    /// Line height multiplier (1.0 = normal, >1.0 = more spacing)
    pub line_height_mul: f32,
    
    /// Letter spacing in logical pixels
    pub letter_spacing: f32,
    
    /// Word spacing in logical pixels
    pub word_spacing: f32,
    
    /// Padding around text container in logical pixels
    pub padding: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_key: "default".to_string(),
            font_size: 16.0,
            color: [1.0, 1.0, 1.0, 1.0], // White
            h_align: HorizontalAlignment::Left,
            v_align: VerticalAlignment::Top,
            line_height_mul: 1.0,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            padding: 0.0,
        }
    }
}
```

## Enhanced Version with Builder Pattern

```rust
use plutonium_engine::pluto_objects::text2d::{HorizontalAlignment, VerticalAlignment};

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub font_key: String,
    pub font_size: f32,
    pub color: [f32; 4],
    pub h_align: HorizontalAlignment,
    pub v_align: VerticalAlignment,
    pub line_height_mul: f32,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub padding: f32,
}

impl TextStyle {
    pub fn new(font_key: impl Into<String>, font_size: f32) -> Self {
        Self {
            font_key: font_key.into(),
            font_size,
            ..Default::default()
        }
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a];
        self
    }

    pub fn with_color_rgba(mut self, rgba: [f32; 4]) -> Self {
        self.color = rgba;
        self
    }

    pub fn with_alignment(mut self, h: HorizontalAlignment, v: VerticalAlignment) -> Self {
        self.h_align = h;
        self.v_align = v;
        self
    }

    pub fn with_line_height(mut self, mul: f32) -> Self {
        self.line_height_mul = mul;
        self
    }

    pub fn with_spacing(mut self, letter: f32, word: f32) -> Self {
        self.letter_spacing = letter;
        self.word_spacing = word;
        self
    }

    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_key: "default".to_string(),
            font_size: 16.0,
            color: [1.0, 1.0, 1.0, 1.0],
            h_align: HorizontalAlignment::Left,
            v_align: VerticalAlignment::Top,
            line_height_mul: 1.0,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            padding: 0.0,
        }
    }
}
```

## Usage with Engine API

```rust
use plutonium_engine::{
    PlutoniumEngine,
    pluto_objects::text2d::TextContainer,
    utils::{Position, Rectangle},
};

pub fn draw_text(
    engine: &mut PlutoniumEngine,
    text: &str,
    position: Position,
    container_size: (f32, f32),
    style: &TextStyle,
) {
    // Create text container from style
    let container = TextContainer::new(Rectangle::new(
        position.x,
        position.y,
        container_size.0,
        container_size.1,
    ))
    .with_alignment(style.h_align, style.v_align)
    .with_padding(style.padding)
    .with_line_height_mul(style.line_height_mul);

    // Queue text with spacing and z-order
    engine.queue_text_with_spacing(
        text,
        &style.font_key,
        position,
        &container,
        style.letter_spacing,
        style.word_spacing,
        0, // z-order (0 = default, higher values render on top)
    );
    
    // Note: Color/tint support may need to be added to the engine
    // Currently text is rendered as-is from the font atlas
}
```

## Predefined Styles

```rust
impl TextStyle {
    pub fn heading_large(font_key: impl Into<String>) -> Self {
        Self::new(font_key, 32.0)
            .with_line_height(1.2)
    }

    pub fn heading_medium(font_key: impl Into<String>) -> Self {
        Self::new(font_key, 24.0)
            .with_line_height(1.2)
    }

    pub fn body(font_key: impl Into<String>) -> Self {
        Self::new(font_key, 16.0)
            .with_line_height(1.5)
    }

    pub fn caption(font_key: impl Into<String>) -> Self {
        Self::new(font_key, 12.0)
            .with_line_height(1.4)
    }

    pub fn button_text(font_key: impl Into<String>) -> Self {
        Self::new(font_key, 16.0)
            .with_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
    }
}
```

## Notes

1. **Font Loading**: Fonts must be loaded separately before use:
   ```rust
   engine.load_font("path/to/font.ttf", style.font_size, &style.font_key)?;
   ```

2. **Color Support**: The engine's current text rendering doesn't explicitly support tinting. If you need colored text, you may need to:
   - Use pre-colored font atlases
   - Request color/tint support in the engine
   - Use a workaround if the engine supports it

3. **Container vs Style**: 
   - `TextStyle` defines how text looks and behaves
   - `TextContainer` defines the layout bounds and positioning
   - They work together to render text

4. **DPI Awareness**: Font sizes are in logical pixels, so they automatically scale with DPI.

