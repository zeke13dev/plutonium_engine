# Features and Modules

A reference for the public modules, object factories, rendering paths, and Cargo
feature flags exposed by `plutonium_engine`. For a runnable introduction see
[`getting-started.md`](getting-started.md); for the coordinate model see
[`coordinates-and-dpi.md`](coordinates-and-dpi.md).

## Public modules

| Module | Role |
| --- | --- |
| `app` | Window/event-loop runner (`run_app`, `PlutoniumApp`, `WindowConfig`, `FrameContext`) plus record/replay, fixed-timestep, and held-key polling. |
| `camera` | 2D camera with follow-target, boundary clamping, and frame-rate independent smoothing. |
| `input` | Per-frame keyboard/mouse state, action maps, and axis bindings. |
| `renderer` | Low-level GPU renderer and command types (`RectCommand`, `GlowCommand`). |
| `texture_svg` | SVG texture loading/rasterization via `resvg`/`tiny-skia`. |
| `texture_atlas` | Texture atlases with per-tile UVs. |
| `text` | Font-atlas text rendering (raster and MSDF paths). |
| `popup` | Modal popups with actions, sizing, and dismiss reasons. |
| `traits` | Shared object traits (`PlutoObject`, update/transform contracts). |
| `utils` | Core value types: `Position`, `Size`, `Rectangle`, `DrawParams`, color helpers. |
| `error` | `EngineError` and result types. |
| `rng` | Deterministic, seedable RNG streams for reproducible scenes and tests. |
| `ui` | Immediate-mode UI helpers (`halo_rect`, `halo_response`, and friends). |
| `pluto_objects` | Retained-mode objects — see below. |
| `anim` *(feature `anim`)* | Tweening/animation: `Tween`, `Track`, `Timeline`, easing. |
| `layout` *(feature `layout`)* | Anchor/percent layout helpers. |

### `pluto_objects` (retained-mode objects)

| Type | Feature | Purpose |
| --- | --- | --- |
| `Texture2D` | always | A positioned SVG/raster sprite. |
| `TextureAtlas2D` | always | An atlas-backed sprite addressing tiles by index/UV. |
| `Text2D` | always | A retained text object. |
| Shapes (`Rectangle`, `Circle`, `Polygon`) | always | Vector primitives. |
| `Button` | `widgets` | Clickable button with hover/press/focus states. |
| `TextInput` | `widgets` | Focusable single-line text field. |

## Object factories

Retained objects are created from the engine and render through the same draw
path as immediate-mode calls:

```rust
let sprite  = engine.create_texture_2d("assets/player.svg", Position::default(), 1.0)?;
let atlas   = engine.create_texture_atlas_2d(/* ... */)?;
let label   = engine.create_text2d(/* ... */)?;
let button  = engine.create_button(/* ... */)?;   // feature = "widgets"
let input   = engine.create_text_input(/* ... */)?; // feature = "widgets"
let rect    = engine.create_rect(/* ... */);
let circle  = engine.create_circle(/* ... */);
let poly    = engine.create_polygon(/* ... */);
```

Lower-level texture/font loaders are also available:
`create_texture_svg`, `create_texture_svg_from_str`,
`create_texture_raster_from_path` (feature `raster`), `create_texture_atlas`,
`load_font`, `load_font_from_bytes`, `load_msdf_font`, `load_msdf_font_from_ttf`.

## Rendering paths

The engine supports two interchangeable styles that share one GPU pipeline:

- **Immediate mode** — clear the queue, issue `draw_*` calls, present:
  ```rust
  engine.begin_frame();
  engine.draw_texture(&texture_key, position, DrawParams::default());
  engine.draw_rect(/* ... */);
  engine.end_frame()?;
  ```
  `DrawParams` carries `z` (layer), `scale`, `rotation`, and `tint` (RGBA).

- **Retained mode** — hold objects and call `render`:
  ```rust
  engine.clear_render_queue();
  sprite.render(engine);
  button.render(engine);
  engine.end_frame()?;
  ```

Batching-oriented `queue_*` methods (`queue_texture`, `queue_tile`,
`queue_text`, slot/layer helpers) collect intents that are flushed together; see
[`instancing-and-batching.md`](instancing-and-batching.md).

### Effects

- **Perimeter glow** — `draw_rect_glow` renders neon-style SDF outlines with
  inward/outward falloff. See [`../examples/rect_glow_test.rs`](../examples/rect_glow_test.rs).
- **Halo/highlight** — `draw_halo`, `draw_halo_for_object`, `HaloStyle`,
  `HaloPreset` for tutorial-style spotlights. See
  [`../examples/halo_showcase.rs`](../examples/halo_showcase.rs).

## Cargo features

| Feature | Default | Description |
| --- | --- | --- |
| `widgets` | ✅ | Retained-mode widgets: `Button`, `TextInput` (and their `pluto_objects` modules). |
| `layout` | | Anchor/percent/margin layout helpers (`layout` module). |
| `anim` | | Tweening/animation: `Tween`, `Track::{Sequence,Parallel}`, `Timeline`, cubic-bezier easing (`anim` module). |
| `raster` | | PNG/JPEG texture and raster-font helpers. On wasm this includes URL-based image loading. |
| `wasm` | | `wasm32-unknown-unknown` support, including a JavaScript entropy backend for RNG. |

RNG and record/replay plumbing (`rng` module, `PlutoniumApp::start_recording`)
are always available and are **not** gated behind a feature flag.

### Enabling features

```toml
[dependencies]
plutonium_engine = { version = "0.8", features = ["layout", "anim"] }
```

To opt out of the default widgets:

```toml
[dependencies]
plutonium_engine = { version = "0.8", default-features = false, features = ["layout"] }
```

WASM builds are compile-checked from the workspace root with:

```bash
cargo check --workspace --target wasm32-unknown-unknown --features wasm
```
