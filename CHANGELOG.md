# Changelog

## [0.8.0] - Unreleased

### Added
- **Perimeter Glow API** - New `draw_rect_glow` method for neon-style perimeter effects around rounded rectangles. Uses SDF-based rendering for sharp core lines and exponential soft-glow falloff (both inward and outward).
- **New example: Rect Glow** - Added `examples/rect_glow_test.rs` demonstrating various neon, soft, and lamp-style perimeter effects.
- **Texture: Cover mode** - Added `TextureFit::Cover` which fills the destination rectangle while preserving aspect ratio (cropping the excess).
- **Texture: Logical Insets** - Added `inset` parameter to stretched texture drawing functions to allow for uniform logical padding within a container.
- **Texture: Stretched Draw API** - New convenience method `draw_texture_stretched_with_fit_and_inset`.
- **WASM raster URL loading (raster feature)** - Added `create_texture_raster_from_url(url, position) -> async Result<(Uuid, Size), RasterTextureLoadError>` for runtime image loading from browser URLs (for example `/assets/*.png`) without rebuilding wasm bundles.
- **WASM pollable raster URL loading (raster feature)** - Added `begin_texture_raster_from_url(url, position) -> RasterTextureUrlLoadHandle` and `poll_texture_raster_from_url(handle) -> Option<Result<(Uuid, Size), RasterTextureLoadError>>` for synchronous frame loops that cannot `await`.

### Fixed
- **Engine: Missing self context** - Fixed pre-existing compilation errors in `src/lib.rs` where `dpi_scale_factor` was missing `self.` context in `load_font` and `load_msdf_font_from_ttf`.
- **Snapshots: API alignment** - Fixed `src/bin/snapshots.rs` compilation by adding the missing `scale` argument to `get_transform_uniform` calls.
- **Texture: Aspect Ratio Preservation** - Updated `TextureFit::Contain` to correctly preserve aspect ratios across different DPI scales using the texture's original pixel dimensions.
- **Texture: NDC Math & Standardization** - Standardized on a 0.0 to 1.0 unit quad and fixed NDC transformation math for all texture and atlas rendering, improving placement accuracy and fixing double-scaling bugs.
- **Texture: Sprite Scaling** - `DrawParams::scale` is now correctly applied when using `draw_texture`.

## [0.7.2] - 2026-02-06

### Major Update: Smooth Camera + Movement Validation

### Added
- Opt-in workspace-wide `wasm` Cargo feature for `wasm32-unknown-unknown` compile support (engine + game crates)
- Feature propagation across workspace crates so `cargo check --workspace --target wasm32-unknown-unknown --features wasm` works from the repo root
- WASM runtime entrypoint: `app::run_app_wasm(config, canvas_id, frame_callback) -> async Result<(), JsValue>`
  - Uses an existing canvas id (required)
  - Returns clear `JsValue` errors when the canvas is missing or not a `<canvas>`
  - Keeps the same frame callback shape as native `run_app`
- WASM runtime options API: `app::run_app_wasm_with_options(config, canvas_id, wasm_config, frame_callback)`
  - New `WasmAppConfig { prevent_default, focusable }` for browser input/debug tuning
- WASM-safe asset/font loading hooks:
  - `load_font_from_bytes(font_bytes, logical_font_size, font_key)`
  - `load_msdf_font_from_bytes(atlas_rgba, width, height, metadata_json, font_key)`
  - `create_texture_svg_from_str(svg_source, position, scale_factor)`
- New wasm raster-text smoke example: `examples/wasm_text_smoke.rs`
  - Loads font bytes via `load_font_from_bytes(...)`
  - Draws animated text in a canvas-backed wasm window
- New browser runner script: `scripts/run_wasm_text_smoke.sh`
  - Builds wasm example, generates web bindings, serves locally, and opens browser automatically
- Camera follow smoothing with frame-rate independent interpolation (`set_camera_smoothing`, dt-aware camera updates)
- Held-key input helpers on `PlutoniumApp` for continuous movement polling (`is_key_down`, `is_char_key_down`, `is_named_key_down`)
- `MouseInfo` now exposes per-frame scroll deltas via `scroll_dx` and `scroll_dy` (logical units, includes wheel and trackpad scroll gestures, reset after each frame)
- MSDF text support with offline atlas assets:
  - New API: `load_msdf_font(atlas_image_path, metadata_json_path, font_key)`
  - New API: `load_msdf_font_from_ttf(font_path, logical_font_size, font_key)` for runtime TTF-to-MSDF generation at font load
  - Runtime generator now builds edge-colored, multi-channel distance fields from glyph outlines (not alpha coverage conversion)
  - JSON metadata parsing for atlas/metrics/glyph/kerning data
  - MSDF shader pipeline (`shaders/text_msdf.wgsl`) and per-glyph UV rendering path
  - ASCII `32..=126` phase-1 coverage with `?` fallback enforcement
- New offline MSDF bake binary: `msdf_bake`
  - One-time TTF/OTF -> `*.msdf.png` + `*.msdf.json` asset generation
  - Configurable bake quality knobs (`--font-size`, `--gen-scale`, `--padding`, `--px-range`)
  - Runtime-friendly output for `load_msdf_font(...)`
- Configurable tutorial halo/highlight effect:
  - Engine-level APIs: `HaloStyle`, `HaloFalloff`, `HaloPreset`, `HaloStyle::from_preset`, `draw_halo`, `draw_halo_for_object`, `object_bounds`
  - Immediate UI helpers: `halo_rect` and `halo_response`
  - Supports customizable color, falloff profile, intensity/alpha, radius, pulse, ring count, and corner radius
  - `draw_halo` now explicitly consumes logical screen-space target rects
  - `draw_halo_for_object` returns `false` and draws nothing for missing, hidden (non-positive bounds), or offscreen objects
- New `halo_showcase` example demonstrating screen-space halos, object halos, preset cycling, and offscreen `draw_halo_for_object == false` behavior
- New `halo_text_container` example showing a halo around text rendered inside a `TextContainer`
- New `msdf_text` example for side-by-side raster vs MSDF rendering across multiple font sizes
- New `msdf_visual_test` example for side-by-side raster reference vs baked MSDF quality checks across small-to-large sizes
- New `raster_cache_visual_test` example for raster-only multi-size cache/prewarm validation across 10px..96px text
- `msdf_visual_test` now includes a same-size raster line directly below the 32px MSDF sample for easier spacing/placement comparisons
- `msdf_visual_test` now prints spacing diagnostics to console (once) instead of rendering debug text overlays
- New high-quality MSDF loading API: `load_msdf_font_with_tiny_raster(font_path, atlas_image_path, metadata_json_path, font_key)` for hybrid tiny hinted-raster + MSDF rendering
- New font-atlas debug export API: `debug_dump_font_atlas_png(font_key, output_path)` for one-click PNG dumps
- New text measurement API: `measure_text(text, font_key, letter_spacing, word_spacing, font_size_override)` returning `(width_px, line_count)` from runtime layout math
- New text spacing debug API: `debug_print_text_line_layout(line, font_key, font_size, letter_spacing, word_spacing)` for per-glyph pen/kerning/bounds console diagnostics
- New raster font cache APIs:
  - `load_font_with_options(font_path, logical_font_size, font_key, FontLoadOptions)`
  - `warm_text_cache(font_key, PrewarmConfig) -> Result<WarmStats, String>`
  - New config/types: `FontLoadOptions`, `PrewarmPolicy`, `PrewarmConfig`, `GlyphSet`, `RasterHintingMode`, `WarmStats`
- `msdf_text` example now supports left-click MSDF atlas export (`debug_msdf_font_atlas.png`)
- `load_msdf_font(...)` now reports detailed bake instructions when atlas/metadata files are missing or invalid
- Headless camera behavior tests covering:
  - Activation/deactivation behavior
  - Deadzone/bounding box overflow behavior
  - Smoothing convergence and non-overshoot
- Engine DPI API: `PlutoniumEngine::set_dpi_scale_factor(f64)` with finite/positive sanitization and internal `f32` storage for render/input math
- Texture fitting API for responsive sprite placement:
  - `TextureFit::{Contain, StretchFill}`
  - Queue-first stretched draws: `queue_texture_stretched(...)` (+ fit/layer variants)
  - Immediate convenience wrapper: `draw_texture_stretched(...)`
- Window/DPI query accessors for layout correctness:
  - `logical_window_size() -> Size` (logical px)
  - `dpi_scale_factor() -> f32`
- Layered slot rendering API for deterministic UI composition:
  - `begin_slot(slot_id, rect, z_base)` / `end_slot(slot_id)` frame-local lifecycle
  - `queue_slot_layer_texture(...)` with `TextureFit::{Contain, StretchFill}` and uniform logical inset
  - `queue_slot_layer_rect(...)` with fill + optional border from the same slot definition
  - Slot layer queue calls now return `bool` (`true` queued, `false` ignored for missing/ended/invalid slot or missing texture)
  - `slot_hit_rect(slot_id)` returns logical screen-space rectangle used for hit testing
  - Per-item slot clipping via rectangular scissor intersection (global clip + slot clip)
- Engine modal popup primitive (`ui_popup`) for transient outcomes and confirmations:
  - New types: `PopupConfig`, `PopupAction`, `PopupActionStyle`, `PopupSize`, `PopupEvent`, `PopupDismissReason`
  - New engine APIs: `show_popup`, `close_popup`, `popup_is_open`, `drain_popup_events`
  - Single active popup with replacement semantics (`Dismissed { Replaced }` for the previous popup)
  - Centered modal panel with backdrop scrim and deterministic top-layer rendering in `end_frame`
  - Configurable title, message, 1-2 actions (v1 sanitized), Escape dismissal, backdrop-click dismissal, and auto-dismiss timeout
  - While open, engine-level object updates receive consumed input (`mouse=None`, `key=None`) so background interactions do not trigger
  - Works in native and wasm runtimes through shared engine update/render paths (no platform-specific popup code)
  - Popup Interaction Policy v2:
    - New `PopupConfig` fields: `consume_opening_click`, `click_anywhere_action_id`, `block_input_behind_popup`
    - New popup lifecycle event: `PopupEvent::Opened { popup_id }`
    - Same-frame dismissal guard via internal popup open frame tracking
    - Optional consume-opening-click release gate to prevent open-and-dismiss on one click sequence
    - Optional click-anywhere action routing with precedence after explicit action buttons
    - Configurable background input blocking while a popup is open (default-safe behavior remains blocked)
    - New custom-content popup API: `show_popup_with_objects(config, panel_rect, object_ids)` for manual popup panel layouts backed by retained Pluto object UUIDs
    - Custom popup mode renders only provided objects inside a user-defined logical panel rectangle while preserving popup interaction policy (click-anywhere/backdrop/escape/turn-gating)
    - Custom popup object IDs are auto-cleaned from engine object storage on popup close/replacement/programmatic close
    - Standard popup message rendering now auto-wraps to panel message bounds, preserves explicit `\n`, and force-breaks overlong words to avoid overflow

### Changed
- Native-only dependencies are now target-gated (`freetype-rs`, `arboard`) so wasm builds avoid unsupported host integrations
- FreeType-backed raster generation paths now return clear wasm stub errors instead of failing to compile
- Native runtime API remains unchanged; wasm runtime/path-specific additions are gated to `cfg(target_arch = "wasm32")`
- Camera follow pipeline now updates using `delta_time` so smoothing is stable across frame-rate variation
- Text rendering internals now support dual backends (legacy raster atlas + MSDF atlas) behind existing `queue_text*` and `Text2D` APIs
- Atlas batching now supports direct per-instance UV rectangles for glyphs (used by MSDF text path)
- Hybrid text routing now supports tiny-size hinted-raster fallback for MSDF fonts (quality-first path for very small text)
- `examples/camera.rs` now uses continuous dt-based movement and camera on/off toggling for clearer smoothness verification
- `examples/jitter_test.rs` now supports direct camera-follow comparison with centered deadzone visualization and on/off follow mode
- `examples/jitter_test.rs` control mapping changed:
  - `D` is always movement-right
  - dt smoothing toggle moved to `T`
- `load_font(...)` now defaults to a light raster prewarm profile (`12,14,16,18,24,32px`) while preserving existing call sites
- Raster text rendering now selects the nearest prewarmed size for measurement/layout and queues missing sizes for runtime warm-up under a per-frame glyph budget
- MSDF remains an explicit opt-in path (`load_msdf_font*` only); raster loading APIs never auto-convert to MSDF
- Raster cache loading now uses hinted FreeType glyph rasterization for small/medium sizes (auto up to 48px) and rusttype atlas generation above that threshold
- App runtime now handles `WindowEvent::ScaleFactorChanged` and always runs the DPI update path plus surface resize/reconfigure on scale changes
- WASM canvas startup/resize path now keeps backing dimensions synced to CSS size multiplied by DPR (prefers winit scale factor, falls back to `window.devicePixelRatio`)

### Fixed
- WASM raster font loading now avoids FreeType hinted-atlas paths and falls back to rusttype atlas generation, fixing `load_font_from_bytes(...)` failures in wasm text smoke runs
- `load_font_from_bytes(...)` now uses wasm-friendly defaults (no prewarm, no hinting) to keep browser startup responsive
- WASM smoke example now uses debug-friendly web input options (`prevent_default: false`) and a plain viewport page to make browser/devtools diagnostics easier
- WASM smoke runner now includes an on-page debug status HUD and a frame-heartbeat background for no-console diagnosis
- WASM smoke HTML bootstrap now reports JS/init errors in-page (`js error`, `promise rejection`, `wasm init failed`) so failures before Rust startup are visible without devtools
- WASM smoke loader now uses streaming module init to reduce main-thread stalls during wasm instantiation
- Resolved jitter caused by event-driven movement input in jitter testing by switching to held-key polling
- Fixed deadzone test visualization placement on high-DPI displays by centering using logical window size
- Fixed runtime TTF->MSDF glyph generation producing vertically reflected text (Y-axis sampling orientation)
- Fixed runtime TTF->MSDF sampling origin bug that produced line-artifact glyph atlases
- Fixed runtime TTF->MSDF glyph UV metadata using full tile bounds, which compressed glyphs on X
- Improved MSDF text sharpness by switching to scale-aware `screenPxRange` shader decoding
- Improved runtime TTF->MSDF quality by generating higher-resolution atlases internally (2x scale)
- Improved runtime TTF->MSDF edge coloring to keep channels stable across contour sides (corner-aware assignment)
- Improved runtime MSDF rendering stability by storing SDF in alpha and clamping MSDF decode with alpha fallback
- Improved MSDF crispness via pixel-snapped glyph placement in layout
- Fixed MSDF atlas sampling artifacts by forcing base-mip sampling (`textureSampleLevel(..., 0.0)`) and clamping MSDF samplers to mip 0 with linear (non-sRGB) texture data
- Fixed MSDF metadata UV bounds to include padded glyph tiles (prevents stretched/bitten glyphs when sampling padded distance fields)
- Fixed MSDF decode constant mismatch by piping `px_range` from metadata/runtime into shader sampling
- Fixed tiny-raster fallback atlas packing to match tile UV indexing (prevents sparse/diagonal "few letters visible" glyph sampling)
- Fixed text instance GPU buffer layout mismatch between Rust and WGSL (prevents per-glyph instance corruption after the first few characters)
- Fixed MSDF glyph placement metrics by deriving `plane_bounds` from exact glyph outline bounds (improves kerning/sidebearing and baseline consistency)
- Fixed MSDF layout double-rounding of glyph quads in logical space (improves horizontal spacing and baseline consistency)
- Fixed MSDF `plane_bounds` Y-axis conversion from rusttype to y-up coordinates (descenders now drop below baseline correctly)
- Fixed MSDF per-glyph queue pixel-snapping to avoid quantized pair spacing drift (`j/u`, `m/y`-style inconsistencies)
- Improved MSDF kerning extraction by using OpenType shaping (`rustybuzz`) for ASCII pair advances, with rusttype fallback when shaping data is unavailable
- Fixed tiny-raster fallback X placement rounding in hybrid mode to reduce pair-spacing jitter at very small sizes (notably around 10px)
- Fixed hybrid MSDF tiny-raster spacing to advance with tiny hinted glyph metrics (instead of MSDF em advances), resolving tight/overlap pairs like `j/u` at small sizes
- Adjusted MSDF shader decode to use clamped MSDF distance directly (instead of alpha-envelope min-clamp), reducing pair-specific visual spacing drift from sidebearing bias
- Fixed MSDF glyph horizontal squeeze by generating `plane_bounds` from positioned pixel bounds (matching atlas tile rasterization geometry), reducing apparent width/spacing drift in baked metadata
- Fixed MSDF atlas field sampling anchor to use positioned pixel bounding boxes (instead of exact outline bounds), aligning glyph ink with quad placement and reducing left/right directional spacing bias
- Fixed MSDF kerning metadata generation to use rusttype `pair_kerning` (same model as raster text path), removing pair-direction spacing drift introduced by shaping-offset kerning in a pen-only renderer
- Fixed shaping-derived kerning extraction to account for second-glyph `x_offset` placement (improves pair spacing where GPOS uses offsets instead of width deltas)
- Fixed offline `msdf_bake` glyph `plane_bounds.left` generation to use exact glyph bounds (matching runtime MSDF generation), eliminating baked/runtime spacing drift from mixed metric sources
- Fixed runtime/offline MSDF atlas sampling origin on X to use exact glyph bounds instead of left-side-bearing anchoring (prevents glyph-local horizontal drift that can surface as pair overlap like `j/u`)
- Fixed MSDF metadata unit mismatch by storing advances/kerning/plane-bounds/vertical metrics in a consistent rusttype-space scale (prevents pair-specific spacing drift such as `p/h` vs `j/u` despite similar nominal font size)
- Fixed hybrid tiny-raster vertical alignment to use MSDF atlas ascender/descender metrics for line baseline placement (prevents baseline jumps when switching between tiny-raster and MSDF paths)
- Removed temporary MSDF kerning sign inversion debug logic from runtime layout (restores direct metadata kerning direction)
- Fixed raster atlas glyph-bearing extraction to use pixel bounds (instead of fractional exact bounds) so per-character vertical placement stays stable at small sizes
- Fixed raster text pixel-snapping on HiDPI displays by snapping glyph baseline/top-left to physical-pixel precision (`round(v*dpi)/dpi`) instead of whole logical pixels

## [0.7.1] - 2025-11-07

### Added
- **Layout: Percentage-based positioning** - Added `HAnchor::Percent(f32)` and `VAnchor::Percent(f32)` to position elements at arbitrary percentages within containers
  - Example: `HAnchor::Percent(1.0/3.0)` positions center at 1/3 width from left
  - Works with margins and percent sizing
- **Layout: Window bounds helpers** - Added `engine.window_bounds()` method and `layout::window_bounds()` function for convenient container creation
  - Returns logical pixel coordinates (DPI-aware)
- **Examples** - Added `layout_top_bar.rs` and `layout_percent_position.rs` demonstrating layout features
- **Documentation** - Added guides for:
  - Creating UI library crates (`docs/creating-ui-lib.md`)
  - Text style structs (`docs/text-style-example.md`)
  - Layout positioning (`docs/layout-positioning-example.md`)
  - Updated `docs/layout.md` with percentage positioning examples

### Fixed
- **Layout: DPI scaling** - Fixed `engine.window_bounds()` to return logical pixels instead of physical pixels, preventing double-height rendering issues

### Changed
- `layout` feature is now opt-in (removed from default features) to keep the engine lightweight
  - Enable with: `plutonium_engine = { features = ["layout"] }`

## [0.7.0] - 2025-08-16

### Changed
- Version bump to 0.7.0 (next release after 0.6.0)
- Clippy-clean with `-D warnings`; CI green across fmt, clippy, tests, and snapshots

### Added
- Utility types for WGSL interop in `src/utils.rs` (`UVTransform`, `TransformUniform`, `InstanceRaw`, `RectInstanceRaw`)
- Frame time metrics helper for lightweight runtime performance reporting

## [0.6.0] - 2025-08-15

### Major Features
- Plutonium Game Framework: ECS-based game layer with modular crates (core, input, assets, ui, audio, gameplay)
- Input action mapping with edge detection
- Retained-mode UI widgets with themes; demo card game example

### Engine Improvements
- True instancing for single-texture sprites with GPU batching
- Animation system (feature `anim`): Timeline, Track, Tween with easing
- RNG system (feature `replay`): Deterministic streams for reproducible simulations

### Examples & Testing
- New examples: actions_demo, anim_demo, slider, text_alignment, toggle, ui_primitives
- Snapshot testing with golden images

### API & Documentation
- Enhanced `DrawParams` (`rotation`, `tint`, `z` layering)
- Docs: architecture guides, API references, feature explanations
- Code quality: formatted and comprehensive coverage

## [0.5.0]
### Added
- Immediate-mode helpers: `begin_frame()` and `end_frame()`
- Z-layered rendering with stable sorting; `queue_*_with_layer` and `draw_*` APIs
- `DrawParams` for consistent draw options (z, scale)
- Cargo features scaffold: `backend-wgpu` (default), `raster`, `widgets`, `layout`
- Raster textures (feature `raster`): load PNG/JPEG via `create_texture_raster_from_path`
- Renderer seam: Introduced `renderer` module with a `Renderer` trait and a `WgpuRenderer` implementation used internally
- Documentation: README updated with features, coordinates, API styles, features, and testing/CI notes
- CI: GitHub Actions workflow for fmt, clippy, tests (default and raster features)
- Docs scaffolding under `docs/` (coordinates/DPI, API styles, features, layering)
- Transform pooling to reduce per-draw allocations (batching groundwork)
- DrawParams now includes `rotation` for sprites
- New examples: raster texture example; local snapshot scenes (atlas, checkerboard, sprite)
- Layout v1 (feature `layout`): anchors, percent sizing, margins; basic example and unit test

### Changed
- Input: keys are now forwarded to engine updates so interactive widgets receive keystrokes
- Mouse coordinates are now DPI-correct (`dpi_scale_factor`), removing hard-coded divide
- Surface error handling: recover on `Lost`/`Outdated`, skip frame on `Timeout`
- README expanded with features, coordinate system, API styles, and feature flags

### Removed
- Unused dependencies cleaned up

---

## [0.4.0]
### Changed
- button now has on_click, on_focus, on_unfocus
- updated all dependencies
- added wrapper to refactor api

## [0.3.0]
### Added
- text alignment options using text container
- Shape PlutoObject for drawing primitive shapes (rectangle, circle, polygon)
- text input now works with standard movement (arrow keys, clicking mouse for text position, and delete key)

### Changed
- text input cursor is now texture svg instead of text object

### Removed
-src/text_input.rs (wasn't being used anyways)

### Fixed
- text alignment and positioning
- text_input cursor placement

## [0.2.2] - 2024-12-31
### Fixed
- Fixed window resizing behavior in examples

## [0.2.1] - Earlier Release
Previous version history does not exist at this point as the crate was in alpha before this release.
