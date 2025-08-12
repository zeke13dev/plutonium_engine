# plutonium_engine

A pure Rust 2D graphics engine built on wgpu. SVG-first, DPI-aware, with text and optional widgets.

Features:
- SVG textures rendered via resvg/tiny-skia
- Texture atlases with per-tile UVs
- Text rendering via a font atlas
- Optional retained-mode widgets: Texture2D, TextureAtlas2D, Text2D, Button, TextInput (feature: `widgets`)
- Camera with boundary and tether

Coordinate system:
- Logical pixels; origin top-left, +x right, +y down. DPI scaling handled internally.

API styles:
- Immediate-mode: `begin_frame()`, `draw_*`, `end_frame()`
- Optional retained widgets (feature `widgets`): higher-level objects that render via the same draw path

Cargo features:
```toml
[features]
default = ["backend-wgpu"]
backend-wgpu = []           # WGPU backend
raster = []                 # PNG/JPEG helpers
widgets = []                # Retained-mode widgets
layout = []                 # Simple layout helpers (anchors/percent)
```

Versioning:
- The public API may evolve; see `CHANGELOG.md` for details.

Testing and CI:
- Unit tests (math, transforms, UVs, text measurement) and headless rendering tests are planned.
- GitHub Actions CI with fmt/clippy/test will be added to keep quality high.

Further docs in `docs/`:
- `docs/coordinates-and-dpi.md`
- `docs/api-styles.md`
- `docs/features-and-modules.md`
- `docs/layering.md`
