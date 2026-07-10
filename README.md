# plutonium_engine

[![CI](https://github.com/zeke13dev/plutonium_engine/actions/workflows/ci.yml/badge.svg)](https://github.com/zeke13dev/plutonium_engine/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/plutonium_engine.svg)](https://crates.io/crates/plutonium_engine)
[![docs.rs](https://docs.rs/plutonium_engine/badge.svg)](https://docs.rs/plutonium_engine)
[![License](https://img.shields.io/crates/l/plutonium_engine.svg)](LICENSE)

A pure-Rust 2D graphics engine built on [`wgpu`](https://wgpu.rs). SVG-first,
DPI-aware, with font-atlas text, retained-mode widgets, animation, and a
WebAssembly target.

![plutonium_engine feature showcase](https://raw.githubusercontent.com/zeke13dev/plutonium_engine/main/docs/media/showcase.png)

*Every tile above is real engine output — see [Examples](#examples).*

## Install

```bash
cargo add plutonium_engine
```

```toml
[dependencies]
plutonium_engine = "0.8"
```

## Hello, sprite

The frame callback runs every frame and receives the engine, a per-frame input
snapshot (`FrameContext`), and the app handle:

```rust
use plutonium_engine::{
    app::{run_app, WindowConfig},
    utils::Position,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Plutonium".to_string(),
        width: 800,
        height: 600,
        ..Default::default()
    };

    let mut sprite = None;
    run_app(config, move |engine, frame, _app| {
        // Load once, on the first frame.
        if sprite.is_none() {
            if let Ok(texture) =
                engine.create_texture_2d("examples/media/player.svg", Position::default(), 1.0)
            {
                sprite = Some(texture);
            }
        }

        // Move with WASD.
        if let Some(sprite) = &sprite {
            if frame.pressed_keys.contains_character_ignore_ascii_case("d") {
                let mut p = sprite.get_pos();
                p.x += 4.0;
                sprite.set_pos(p);
            }

            engine.begin_frame();
            sprite.render(engine);
            engine.end_frame().unwrap();
        }
    })?;

    Ok(())
}
```

See [`docs/getting-started.md`](docs/getting-started.md) for a step-by-step
walk-through.

## Features

| Area | What you get |
| --- | --- |
| **SVG textures** | Vector sprites rasterized via `resvg`/`tiny-skia`, DPI-aware. |
| **Raster textures** (`raster`) | PNG/JPEG loading with `Contain`/`Cover`/`StretchFill` fitting and logical insets. |
| **Texture atlases** | Per-tile UV addressing for sprite sheets and tilemaps. |
| **Text** | Font-atlas rendering with a raster path and an MSDF (crisp-at-any-scale) path, alignment, and auto-sizing. |
| **Widgets** (`widgets`, default) | Retained-mode `Button`, `TextInput`, `Text2D`, plus slider/toggle patterns, with hover/press/focus states. |
| **Shapes** | `Rectangle`, `Circle`, `Polygon` vector primitives. |
| **Effects** | Perimeter glow (`draw_rect_glow`) and tutorial halos (`draw_halo`, `HaloPreset`). |
| **Camera** | Follow-target with boundary clamping and frame-rate-independent smoothing. |
| **Layout** (`layout`) | Anchor/percent/margin helpers for building UIs. |
| **Animation** (`anim`) | `Tween`, `Track::{Sequence,Parallel}`, `Timeline` with labels/callbacks and CSS-like cubic-bezier easing. |
| **Input** | Per-frame key/mouse snapshots, held-key polling, action maps, and scroll deltas. |
| **Determinism** | Seedable RNG streams and per-frame input record/replay. |
| **WebAssembly** (`wasm`) | Runs on `wasm32-unknown-unknown` with a canvas backend and JS entropy. |

Two interchangeable rendering styles share one GPU pipeline:

- **Immediate mode** — `begin_frame()`, `draw_*(...)`, `end_frame()`.
- **Retained mode** — hold objects (`create_texture_2d`, `create_button`, …)
  and call `object.render(engine)`.

`DrawParams` carries `z` (layer), `scale`, `rotation`, and `tint` (RGBA).

### Cargo features

```toml
default = ["widgets"]   # retained-mode widgets (Button, TextInput, ...)
raster  = []            # PNG/JPEG texture + raster-font helpers (opt-in)
layout  = []            # anchor/percent layout helpers (opt-in)
anim    = []            # tweening/animation helpers (opt-in)
wasm    = []            # wasm32-unknown-unknown support (opt-in)
```

RNG and record/replay are always available and are not behind a feature flag.
See [`docs/features-and-modules.md`](docs/features-and-modules.md) for the full
module and feature reference.

## Coordinates

Logical pixels; origin top-left, `+x` right, `+y` down. DPI scaling is handled
internally. Engine objects are single-thread affine — create and use them on the
thread that owns the engine. See
[`docs/coordinates-and-dpi.md`](docs/coordinates-and-dpi.md).

## Examples

Run any example with `cargo run --example <name>`. Examples that use an optional
feature are marked; run those with the listed `--features` (or `--all-features`).

| Example | Shows | Features |
| --- | --- | --- |
| `texture2d` | SVG sprite loading and movement | |
| `texture2d_raster` | PNG/raster sprites | `raster` |
| `texture_atlas2d` | Sprite sheets / tile UVs | |
| `grid` | Tilemap-style atlas grid | |
| `camera` | Camera follow, boundaries, smoothing | |
| `text2d` / `text_alignment` / `text_autosize_demo` | Text rendering, alignment, auto-size | |
| `msdf_visual_test` | Crisp MSDF text at scale | `layout,raster` |
| `ui_primitives` | Buttons, labels, panels | |
| `button_debug` / `slider` / `toggle` | Widget states and interaction | |
| `text_input` | Focusable text field | |
| `actions_demo` | Action-map input (buttons/axes) | |
| `rect_glow_test` | Neon perimeter glow | |
| `halo_showcase` / `halo_text_container` | Tutorial halos/highlights | |
| `anim_demo` | Tweens and timelines | `anim` |
| `layout_basic` / `layout_percent_position` / `layout_top_bar` | Layout helpers | `layout` |
| `wasm_text_smoke` | Text rendering in the browser | `wasm` |

## WebAssembly

Compile-check the whole workspace for wasm:

```bash
cargo check --workspace --target wasm32-unknown-unknown --features wasm
```

Runtime entrypoint (requires an existing `#game-canvas` canvas):

```rust
// cfg(target_arch = "wasm32")
plutonium_engine::app::run_app(config, frame_callback)?;
```

For custom canvas ids or browser-debug tuning, use
`run_app_wasm_with_options(config, canvas_id, WasmAppConfig { .. }, frame_callback).await`.
wasm-safe loaders (`load_font_from_bytes`, `create_texture_svg_from_str`,
`create_texture_raster_from_url`, …) let you load assets without rebaking
bundles. A browser smoke test is provided:

```bash
scripts/run_wasm_text_smoke.sh
```

## Testing and snapshots

- `cargo test --all-features` — unit tests (math, transforms, UVs, easing, RNG).
- `cargo run --bin snapshots` — headless golden-image tests; set
  `UPDATE_SNAPSHOTS=1` to refresh goldens.

## Documentation

- [`docs/getting-started.md`](docs/getting-started.md)
- [`docs/features-and-modules.md`](docs/features-and-modules.md)
- [`docs/api-styles.md`](docs/api-styles.md)
- [`docs/coordinates-and-dpi.md`](docs/coordinates-and-dpi.md)
- [`docs/layout.md`](docs/layout.md)
- [`docs/textures.md`](docs/textures.md)
- [`docs/instancing-and-batching.md`](docs/instancing-and-batching.md)
- [`docs/layering.md`](docs/layering.md)

Full API docs: [docs.rs/plutonium_engine](https://docs.rs/plutonium_engine).

## Versioning and stability

The public API may still evolve ahead of 1.0; see
[`CHANGELOG.md`](CHANGELOG.md). Low-level integration APIs intentionally expose
crate-pinned `wgpu` and `winit` types as part of the semver surface.

## Contributing

Issues and PRs welcome — see [`CONTRIBUTING.md`](CONTRIBUTING.md) and
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## Workspace note

`plutonium_game/` is an in-repository companion workspace used to exercise engine
integrations. It is intentionally excluded from the published crate.

## License

Licensed under the [Apache License 2.0](LICENSE).
