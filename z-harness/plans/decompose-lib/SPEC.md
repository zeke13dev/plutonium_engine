# SPEC — decompose-lib

## Overview

Split `src/lib.rs` (5531 lines: ~30 type defs + one `impl<'a> PlutoniumEngine<'a>` with 133 methods) into topic modules. **Hard invariant: zero public-API change.** Mechanism: move method clusters into `impl<'a> PlutoniumEngine<'a>` blocks in child modules (child modules can read the crate-root struct's private fields — verified). The `PlutoniumEngine` struct, all its fields, and every PUBLIC TYPE DEFINITION stay in `lib.rs`.

**What may relocate vs what must stay (precise):**
- **Public inherent METHODS** (`pub fn` on `PlutoniumEngine`, e.g. `load_msdf_font`, `render`, `draw_halo`, `draw_*`/`queue_*`) MAY move to a child module — their public path is `plutonium_engine::PlutoniumEngine::method` regardless of which file the `impl` block lives in, so the surface is unchanged. (The earlier "only private items relocate" framing was imprecise.)
- **Public TYPE definitions** (`pub struct`/`pub enum`: DrawParams, Halo*, TextureFit, FontLoadOptions, RasterTextureLoadError, …) MUST stay defined in lib.rs (D5) so their canonical `plutonium_engine::X` path does not move.
- **Visibility of moved private methods/helpers:** a private (`fn`/`struct`) item moved into `src/foo.rs` is private to `foo` and is NOT callable from lib.rs or sibling modules. Therefore **any moved item that is still called from outside its new module MUST be bumped to `pub(crate)`.** This is internal-only visibility, invisible to the public surface.

## Planning Inputs

| Artifact | Path | generated_at |
|----------|------|--------------|
| (none) | — | none — fresh /z-plan run (seeded by prod-readiness-audit REPORT.md, not a precontext artifact) |

## Invariants (apply to every task)

1. **Public surface unchanged.** `cargo-public-api` diff vs the T001 baseline is EMPTY, on BOTH `host` and `wasm32-unknown-unknown`. Public type DEFINITIONS never move (no re-export canonical-path drift).
2. **Behavior unchanged.** `cargo test --all` green; `cargo run --bin snapshots` passes at the runner's EXISTING comparison tolerance with NO golden regeneration — i.e. it must pass without ever setting `UPDATE_SNAPSHOTS=1`. (The runner compares with tolerance, not byte equality; "no golden update needed" is the verifiable signal of unchanged behavior.)
3. **cfg/feature gates preserved verbatim.** Every moved method keeps its `#[cfg(...)]` / `#[cfg(feature=...)]` attributes; wasm/native branches do not drift.
4. **Attributes preserved.** `#[inline]`, `#[must_use]`, `#[track_caller]`, doc-comments move with their method (cargo-public-api can false-pass on these — manual check per task).
5. **Docs clean.** `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` builds (catches broken intra-doc links from moved methods).
6. **No new allocation in hot paths.** Especially T007 (GpuTimer) and T008 (render/draw).

## Modules to create (private items only)

| File | Contents (method clusters + private helpers moved from lib.rs; cross-called items → `pub(crate)`) |
|------|----------------------------------------------------------------|
| `src/font_raster.rs` | the WHOLE raster-font cluster (load/atlas/warm-queue methods AND their shared associated helpers `sanitize_*`/`quantize_*`/`glyphs_from_set`/`choose_loaded_raster_entry`/`build_tiny_raster_fallback_from_font_data`/etc., ~lib.rs:1066-1630) + helper structs `RasterFontFamily`, `PendingRasterWarmRequest`, `RasterAtlasBuild`, `RasterSizeEntry`, `PendingRasterTextureUrlLoad` (→ `pub(crate)`). Move the cohesive cluster together so no public method is left in lib.rs calling a now-private helper. |
| `src/font_msdf.rs` | MSDF font methods (`load_msdf_font`, MSDF atlas build). |
| `src/popup_render.rs` | `render_popup_overlay` + popup-render private methods (~3560-3748). |
| `src/glow.rs` | private halo/glow methods (`draw_halo_world_rect`, etc., ~3998-4163). Public `HaloStyle/HaloMode/HaloPreset/HaloFalloff/HaloPreset` definitions STAY in lib.rs. |
| `src/gpu_timer.rs` | `GpuTimer` struct owning `timestamp_query/buf/staging/period_ns/count/frame_index` + `gpu_metrics`; methods `new`, `begin(&Device,&mut Encoder)`, `resolve_and_record(...)`, `maybe_report()`. Behavior-identical to inline code at ~3440-3501. |
| `src/render.rs` | core `render()` (INCLUDING its function-local `flush_batch` closure + the three function-local `macro_rules!` `flush_rect_batch!`/`flush_glow_batch!`/`flush_atlas_batch!` — these are defined inside and used only by `render()`, so they travel with it; NO separate macro module needed) + batch helpers `TransformPool`, `RectInstanceBuffer`, `RectStyleKey`, free fns `to_rgba_u8`/`quant_10x`. |
| `src/draw.rs` | `draw_*` / `queue_*` immediate-mode methods (cross-called ones → `pub(crate)`). |

`src/lib.rs` after the pass: `PlutoniumEngine` struct + fields, `new`/`new_async`, all PUBLIC type defs, `mod` declarations + `pub use` (only where a public type was ALREADY re-exported), top-level small impls. Target < ~1500 lines.

## Verification harness (T001)

- `cargo install cargo-public-api` (document in CONTRIBUTING / CI). `rustup target add wasm32-unknown-unknown`.
- Baseline (exact, committed):
  - host: `cargo public-api --simplified --all-features > api-baseline-host.txt`
  - wasm: `cargo public-api --simplified --target wasm32-unknown-unknown --features wasm > api-baseline-wasm.txt`
- Per task gate (a helper script `scripts/check-api.sh` regenerates + diffs):
  - `diff <(cargo public-api --simplified --all-features) api-baseline-host.txt` → must be empty
  - `diff <(cargo public-api --simplified --target wasm32-unknown-unknown --features wasm) api-baseline-wasm.txt` → must be empty
  - non-empty either diff = task fails.
- Static auto-trait guard in a test: assert the engine and public wrapper types retain their current `Send`/`Sync` status (engine is `!Send` via `Rc`; a compile-fail test or `static_assertions`-style check pins it).
- CI job added (pairs with audit RE2/T017).

## Edge cases / risks (from consult)

- **Macros (corrected):** the flush helpers (`flush_batch` closure + `flush_rect_batch!`/`flush_glow_batch!`/`flush_atlas_batch!` `macro_rules!`) are ALL defined inside `render()` and used only there — they are NOT shared across methods. They move with `render()` into `render.rs`; no separate macro module and no `pub(crate) use` is needed.
- **Extension-trait `use`:** each module re-imports the exact wgpu/bytemuck/font `use` set its moved methods need; a dropped extension-trait import silently changes method resolution → caught by `cargo build`.
- **GpuTimer borrow:** pass `&Device`/`&mut Encoder` as params, never `&PlutoniumApp`/`self`, to avoid split-borrow walls.
- **Trait impls** (`impl Drop/Debug for PlutoniumEngine`): keep in lib.rs or render.rs — do not scatter.
