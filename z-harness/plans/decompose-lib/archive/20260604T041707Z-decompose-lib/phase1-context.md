# Phase 1 — Context (direct Read/Grep; 0 Explore agents — under cap)

No docs/llm, no precontext artifacts. Structural facts gathered directly:

- `src/lib.rs` = 5531 lines. Shape: type/enum defs + small impls (lines 73-498), then ONE mega `impl<'a> PlutoniumEngine<'a>` (line 574 → ~end) with **133 methods**, plus 2 free fns (`to_rgba_u8` :471, `quant_10x` :480).
- `PlutoniumEngine` struct (line 499): 50 private fields + `pub size`. Private fields ⇒ child modules can access them ⇒ impl-block split is safe.
- Method buckets (by name keyword over the 133): ~65 font/raster/text/glyph/atlas/warm; 9 popup; 6 halo/glow; GPU-timer is NOT separate methods — it is inline in `render()` at ~3440-3501 (timestamp readback) touching fields `timestamp_query/buf/staging/period_ns/count/frame_index` + `gpu_metrics: FrameTimeMetrics`.
- Helper structs currently private in lib.rs that belong to subsystems: `RasterFontFamily` (:217), `PendingRasterWarmRequest` (:227), `RasterAtlasBuild` (:233), `RasterSizeEntry` (:212), `PendingRasterTextureUrlLoad` (:206), `RasterTextureUrlLoadHandle` (:202 pub), `TransformPool` (:448/484), `RectInstanceBuffer` (:455), `RectStyleKey` (:464), `SlotState` (:440). Halo public types `HaloFalloff/HaloMode/HaloPreset/HaloStyle` (:247-407) + impls.
- Cited regions verified accurate: raster-font ~1066-1630 (`load_raster_font_variant_from_data`, `load_msdf_font`, ...); gpu-timer ~3440-3501 (inline in render); popup ~3560-3748 (`render_popup_overlay`); halo/glow ~3998-4163 (`draw_halo_world_rect`).
- Public types defined in lib.rs that must keep their `plutonium_engine::X` path if moved: `DrawParams, TextureFit, GlyphSet, RasterHintingMode, PrewarmConfig, PrewarmPolicy, FontLoadOptions, WarmStats, RasterTextureLoadError, RasterTextureUrlLoadHandle, HaloFalloff, HaloMode, HaloPreset, HaloStyle, QueuedItem, PlutoniumEngine` — relocation requires `pub use <module>::X;` re-exports from the crate root.
- Verification tooling: `cargo-public-api` NOT installed; no `cargo public-api`/`semver-checks` subcommands present. Verification method is an open decision (D2).
