# plutonium_engine

A pure Rust 2D graphics engine built on wgpu. SVG-first, DPI-aware, with text and optional widgets.

Features:
- SVG textures rendered via resvg/tiny-skia
- Texture atlases with per-tile UVs
- Text rendering via a font atlas
- Optional retained-mode widgets: Texture2D, TextureAtlas2D, Text2D, Button, TextInput (feature: `widgets`)
- Camera with boundary and tether
- Tweening helpers (feature: `anim`): `Tween`, `Track::{Sequence,Parallel}`, `Timeline` with labels/callbacks and CSS-like cubic-bezier easing
- Deterministic RNG streams and basic record/replay plumbing (feature: `replay`)

Coordinate system:
- Logical pixels; origin top-left, +x right, +y down. DPI scaling handled internally.

API styles:
- Immediate-mode: `begin_frame()`, `draw_*`, `end_frame()`
- Optional retained widgets (feature `widgets`): higher-level objects that render via the same draw path
- `DrawParams` supports `z`, `scale`, `rotation`, and `tint` (RGBA) for sprites

Cargo features:
```toml
[features]
default = ["backend-wgpu", "widgets"]
backend-wgpu = []           # WGPU backend (always enabled by default)
widgets = []                # Retained-mode widgets (enabled by default)
raster = []                 # PNG/JPEG helpers (opt-in)
layout = []                 # Simple layout helpers (anchors/percent) (opt-in)
anim = []                   # Tweening/animation helpers (opt-in)
replay = []                 # RNG streams and record/replay helpers (opt-in)
wasm = []                   # Opt-in wasm32 compile support (target wasm32-unknown-unknown)
```

To enable optional features in your project:
```toml
[dependencies]
plutonium_engine = { path = "../path/to/plutonium_engine", features = ["layout", "anim"] }
```

See `docs/features-and-modules.md` for details.

WASM compile check:
```bash
cargo check --workspace --target wasm32-unknown-unknown --features wasm
```

WASM runtime entrypoint (existing canvas required):
```rust
// cfg(target_arch = "wasm32")
plutonium_engine::app::run_app_wasm(config, "game-canvas", frame_callback).await?;
```
For browser-debug workflows, use:
`run_app_wasm_with_options(config, canvas_id, WasmAppConfig { prevent_default: false, ..Default::default() }, frame_callback)`.

WASM-safe loading hooks (wasm32 only):
- `load_font_from_bytes(...)`
- `load_msdf_font_from_bytes(...)`
- `create_texture_svg_from_str(...)`
- `create_texture_raster_from_url(...)` (requires `raster` feature)
- `begin_texture_raster_from_url(...)` + `poll_texture_raster_from_url(...)` (sync-frame friendly, requires `raster`)

WASM text smoke test (opens browser and serves local page):
```bash
scripts/run_wasm_text_smoke.sh
```
It builds with `PROFILE=release` by default for browser responsiveness.
Set `PROFILE=debug` only when you need debug symbols.

Versioning:
- The public API may evolve; see `CHANGELOG.md` for details.

Testing and CI:
- Unit tests (math, transforms, UVs) and headless snapshots (checkerboard, atlas, sprite, many sprites) are provided.
- Snapshots are intended to run locally; keep large perf snapshots out of CI.

Runtime and stepping:
- `PlutoniumApp` exposes `set_fixed_timestep(dt_seconds)` to run a fixed-dt update loop (useful for deterministic sims/tests).

CLI flags:
- App (`run_app`-based examples):
  - `--record <path>`: record per-frame inputs to JSON at `<path>`.
  - `--replay <path>`: replay inputs from `<path>`.
  - `--dt <seconds>`: set fixed timestep for the update loop.
  - `--fps <hz>`: alternative to `--dt`; sets fixed timestep to `1/fps`.
- Snapshot runner (`cargo run --bin snapshots`):
  - `--seed <u64>`: seed for RNG-driven snapshots.
  - `--record <path>`: write a minimal script (`--frames` frames) to `<path>`.
  - `--replay <path>`: load the script at `<path>` and render a verification scene.
  - `--frames <n>`: number of frames for multiframe/timeline snapshots (default 3).
  - `--dt <seconds>`: per-frame delta for multiframe/timeline snapshots (default 0.2).
  - Set `UPDATE_SNAPSHOTS=1` to update golden images on mismatch.

Further docs in `docs/`:
- `docs/coordinates-and-dpi.md`
- `docs/api-styles.md`
- `docs/features-and-modules.md`
- `docs/layering.md`
- `docs/getting-started.md`
- `docs/layout.md`
- `docs/textures.md`
- `docs/instancing-and-batching.md`

Examples:
- `actions_demo`: input action map (buttons/axes) and button hover/press/focus visuals.
- `halo_showcase`: demonstrates `draw_halo` (screen-space) and `draw_halo_for_object` (including offscreen false behavior), plus preset cycling.
- `halo_text_container`: minimal halo/glow example around text rendered inside a `TextContainer`.
# CI Test Comment
