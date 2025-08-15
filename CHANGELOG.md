# Changelog

## [0.7.0] - 2025-08-15

### Release Notes
- Complete Plutonium Game Framework
- Comprehensive visual testing with snapshots
- Production-ready v0.7.0 with full CI/CD pipeline

## [0.6.0] - Released 2025-08-15

### Major Features
- **Plutonium Game Framework**: Complete ECS game development layer
  - 6 modular crates: core, input, assets, ui, audio, gameplay
  - Entity-Component-System with schedules, resources, and events
  - Input action mapping with edge detection
  - Comprehensive UI widget system with themes
  - Demo card game showcasing framework capabilities

### Engine Improvements
- **True instancing** for single-texture sprites with GPU batching
  - Persistent instance bind group layout (group 3)
  - Batching by texture with per-batch storage buffer of `mat4x4`
  - Vertex shader uses `@builtin(instance_index)` for performance
- **Animation System** (feature `anim`): Timeline, Track, Tween with CSS-like easing
- **RNG System** (feature `replay`): Deterministic streams for reproducible simulations
- **UI Primitives**: Nine-slice panels, buttons, sliders, toggles with visual states

### Examples & Testing
- **6 New Examples**: actions_demo, anim_demo, slider, text_alignment, toggle, ui_primitives
- **Comprehensive Snapshot Testing**: 18 visual regression tests with golden images
- **CI/CD Pipeline**: GitHub Actions with formatting, clippy, tests, and snapshots

### API & Documentation
- **Enhanced DrawParams**: Support for `rotation`, `tint`, and `z`-layering
- **Extensive Documentation**: Architecture guides, API references, feature explanations
- **Code Quality**: Clippy-clean, formatted, comprehensive test coverage

## [0.5.0]
- Initial public crate with textures, atlas, text, widgets, camera
# Changelog

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

