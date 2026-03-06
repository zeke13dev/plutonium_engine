# Plutonium UI

Immediate-mode UI toolkit for Plutonium Engine games.

## Features

- Persistent widget state cache
- Images and image buttons
- Progress bars (sized, colored, labeled)
- Text input with focus, hints, and cursor
- Drag-and-drop with typed payloads
- Tooltips with hover delay
- Style stack for scoped overrides
- Tutorial halos/highlights (`halo_rect`, `halo_response`) with configurable falloff, color, pulse, and intensity

## Halo Notes

- Coordinate space: halo target rects are logical screen-space rectangles (top-left origin).
- Safety clamps:
  - `radius <= 0` draws nothing
  - `ring_count` has an effective minimum of `1`
  - `inner_padding < 0` clamps to `0`
  - alpha values are clamped to `[0, 1]`, and `intensity < 0` clamps to `0`

## Examples

Run with:

```sh
cargo run -p plutonium_game_ui --example hello_world
cargo run -p plutonium_game_ui --example layout_demo
cargo run -p plutonium_game_ui --example image_demo
cargo run -p plutonium_game_ui --example progress_demo
cargo run -p plutonium_game_ui --example text_input_demo
cargo run -p plutonium_game_ui --example drag_drop_demo
cargo run -p plutonium_game_ui --example tooltip_demo
cargo run -p plutonium_game_ui --example style_stack_demo
```
