# Changelog

## [0.4.1]
### Added
- Immediate-mode helpers: `begin_frame()` and `end_frame()`
- Z-layered rendering with stable sorting; `queue_*_with_layer` and `draw_*` APIs
- `DrawParams` for consistent draw options (z, scale)
- Cargo features scaffold: `backend-wgpu` (default), `raster`, `widgets`, `layout`
- Raster textures (feature `raster`): load PNG/JPEG via `create_texture_raster_from_path`
- Renderer seam: Introduced `renderer` module with a `Renderer` trait and a `WgpuRenderer` implementation used internally
- Documentation: README updated with features, coordinates, API styles, features, and testing/CI notes

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

