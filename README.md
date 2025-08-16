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
default = ["backend-wgpu"]
backend-wgpu = []           # WGPU backend
raster = []                 # PNG/JPEG helpers
widgets = []                # Retained-mode widgets
layout = []                 # Simple layout helpers (anchors/percent)
anim = []                   # Tweening/animation helpers
replay = []                 # RNG streams and record/replay helpers
```

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
- `docs/instancing-and-batching.md`

Examples:
- `actions_demo`: input action map (buttons/axes) and button hover/press/focus visuals.
# CI Test Comment
