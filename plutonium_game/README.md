# Plutonium Game

A modular 2D game layer built on top of `plutonium_engine`.

## Crates
- `core`: bespoke ECS (entities, components, resources), schedules, time, events, RNG.
- `input`: `InputState` + `ActionMap` with edge detection.
- `assets`: `Handle` registry; helpers to load textures via `plutonium_engine`.
- `ui`: `RenderCommands` that collect sprite draw intents.
- `audio`: placeholder for mixer API.
- `gameplay`: placeholder for demo systems.

## Run the Demo
- `cargo run -p plutonium_demo_card_game`
- Controls:
  - **Enter**: toggle Menu/Game scene
  - **Space**: trigger action "toggle" (tint toggle)
  - **Mouse**: hover/click button; click to focus text input
  - **Tab**: cycle focus between button/toggle/slider
  - **ArrowLeft/ArrowRight**: adjust slider when focused

## ECS Quickstart
```rust
use plutonium_game_core::{App, Entity};

#[derive(Clone, Copy)]
struct Position { x: f32, y: f32 }

let mut app = App::new();
app.startup.add_system(|world| {
    let e: Entity = world.spawn();
    world.insert_component(e, Position { x: 0.0, y: 0.0 });
});
app.run_startup();
```

## Input
```rust
use plutonium_game_input::{InputState, ActionMap};

world.insert_resource(InputState::default());
let mut actions = ActionMap::default();
actions.bind("toggle", "Space");
world.insert_resource(actions);
// each frame: input.update_from_keys(frame_keys)
```

## Render Commands
```rust
use plutonium_game_ui::{RenderCommands, DrawParams};
world.insert_resource(RenderCommands::default());
// per frame: cmds.draw_sprite(texture_uuid, position, DrawParams::default())
// labels and buttons
use plutonium_game_ui::{Label, Button};
let mut label = Label::new(20.0, 20.0, 200.0, 40.0, "Hello", "roboto");
let mut button = Button::new(20.0, 60.0, 200.0, 36.0, "Start", "roboto");
label.draw(&mut cmds);
let clicked = button.update(&world.get_resource::<plutonium_game_input::InputState>().unwrap());
button.draw(&mut cmds);
```

See `examples/demo_card_game` for usage.

## Tests and Snapshots
- Run all tests: `cargo test --workspace`
- Run GPU snapshots: `cargo run --bin snapshots`
  - First run creates goldens in `snapshots/golden/` if missing.
  - Subsequent runs compare `snapshots/actual/` against goldens with a small tolerance and print OK/MISMATCH.


