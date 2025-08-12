# Getting Started

- Add the crate with the default backend:
```toml
plutonium_engine = "0.5"
```
- Create a window and draw a sprite using immediate mode:
```rust
use plutonium_engine::{app::run_app, utils::Position, DrawParams, WindowConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let config = WindowConfig { title: "Hello".into(), width: 800, height: 600 };
  let mut sprite = None;
  run_app(config, move |engine, _| {
    if sprite.is_none() {
      sprite = Some(engine.create_texture_2d("examples/media/square.svg", Position { x: 100.0, y: 100.0 }, 1.0));
    }
    engine.begin_frame();
    if let Some(s) = &sprite { s.render(engine); }
    engine.end_frame().unwrap();
  })?;
  Ok(())
}
```

For performance and batching details, see `docs/instancing-and-batching.md`.
