# Getting Started

Add the crate:

```toml
[dependencies]
plutonium_engine = "0.8"
```

Create a window and draw a sprite. The frame callback runs every frame and
receives the engine, a per-frame `FrameContext` (input snapshot), and the app
handle:

```rust
use plutonium_engine::{
    app::{run_app, WindowConfig},
    utils::Position,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Hello".to_string(),
        width: 800,
        height: 600,
        ..Default::default()
    };

    let mut sprite = None;
    run_app(config, move |engine, _frame, _app| {
        if sprite.is_none() {
            match engine.create_texture_2d(
                "examples/media/player.svg",
                Position { x: 100.0, y: 100.0 },
                1.0,
            ) {
                Ok(texture) => sprite = Some(texture),
                Err(err) => {
                    log::warn!("failed to load sprite: {err}");
                    return;
                }
            }
        }

        engine.begin_frame();
        if let Some(texture) = &sprite {
            texture.render(engine);
        }
        engine.end_frame().unwrap();
    })?;

    Ok(())
}
```

Read input from the `FrameContext`:

```rust
run_app(config, move |engine, frame, _app| {
    if frame.pressed_keys.contains_character_ignore_ascii_case("w") {
        // move up
    }
    engine.begin_frame();
    // draw...
    engine.end_frame().unwrap();
})?;
```

Next steps:

- [`features-and-modules.md`](features-and-modules.md) — modules, object
  factories, rendering paths, and Cargo features.
- [`api-styles.md`](api-styles.md) — immediate vs. retained rendering.
- [`instancing-and-batching.md`](instancing-and-batching.md) — batching and
  performance.
