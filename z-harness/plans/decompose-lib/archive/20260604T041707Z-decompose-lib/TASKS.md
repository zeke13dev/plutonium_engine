# TASKS — decompose-lib

Status legend: `[ ]` pending · `[~]` in_progress · `[x]` done.

**Standing gate (every task, in addition to the listed acceptance):** empty `cargo-public-api` diff vs the T001 baseline on host AND wasm32; `cargo test --all` green; `cargo run --bin snapshots` passes at the existing tolerance with NO `UPDATE_SNAPSHOTS` (proves unchanged behavior); `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` clean.

**Standing visibility rule:** any method/helper moved into a child module that is still called from lib.rs or a sibling module MUST be `pub(crate)` (a bare private `fn` in `src/foo.rs` is unreachable from siblings). Public TYPE definitions never move (stay in lib.rs). Public inherent methods may move (their `PlutoniumEngine::method` path is file-independent). `#[cfg]`/`#[inline]`/`#[must_use]`/doc-comments move verbatim with each item.

### [ ] T001 — Stand up the zero-API-change verification harness
- Install/pin `cargo-public-api`; `rustup target add wasm32-unknown-unknown`; capture baseline public-surface snapshots BEFORE any code moves (host + wasm32); add a `scripts/check-api.sh` that regenerates + diffs both targets; add a static auto-trait guard test pinning the engine + public wrapper types' CURRENT `Send`/`Sync` status; add a CI job that runs `check-api.sh`.
- **Files:** new `api-baseline-host.txt`, `api-baseline-wasm.txt`, `scripts/check-api.sh`, `tests/api_autotraits.rs`; `.github/workflows/ci.yml`; `CONTRIBUTING.md` (if present).
- **Depends on:** none.
- **Acceptance:** both baselines committed; `scripts/check-api.sh` exits 0 (empty diff) on the untouched tree for host AND wasm; auto-trait test encodes the pre-refactor trait status and passes; CI job green.
- **VERIFY:** `bash scripts/check-api.sh && cargo test --test api_autotraits`
- **Complexity:** medium

### [ ] T002 — Extract `src/font_raster.rs` (whole raster-font cluster)
- Move the ENTIRE cohesive raster-font cluster — public methods (`load_raster_font*`, warm-cache API) AND every shared associated helper they call (`sanitize_*`, `quantize_*`, `glyphs_from_set`, `choose_loaded_raster_entry`, `build_tiny_raster_fallback_from_font_data`, atlas-build helpers, ~lib.rs:1066-1630) — into an `impl<'a> PlutoniumEngine<'a>` block in `src/font_raster.rs`. Move private helper structs `RasterFontFamily`, `PendingRasterWarmRequest`, `RasterAtlasBuild`, `RasterSizeEntry`, `PendingRasterTextureUrlLoad` there (→ `pub(crate)`; engine fields reference them). Any helper still called from lib.rs/other modules → `pub(crate)`. Move the cluster TOGETHER so no public method is orphaned from a now-private helper.
- **Files:** new `src/font_raster.rs`; `src/lib.rs`.
- **Depends on:** T001.
- **Acceptance:** raster-font cluster fully relocated; lib.rs free of it; cross-called helpers are `pub(crate)`; build + tests + snapshots green; API diff empty.
- **VERIFY:** `cargo build --all-features && cargo test --all && cargo run --bin snapshots`
- **Complexity:** high

### [ ] T003 — Extract `src/font_msdf.rs`
- Move the MSDF font cluster (`load_msdf_font*`, MSDF atlas build, MSDF-only helpers) into `src/font_msdf.rs`. NOTE: `load_msdf_font_with_tiny_raster` calls `build_tiny_raster_fallback_from_font_data` which lives in the raster cluster — that helper must be `pub(crate)` (ensured by T002). Preserve cfg/attributes/docs.
- **Files:** new `src/font_msdf.rs`; `src/lib.rs`.
- **Depends on:** T001, **T002** (shares the tiny-raster fallback helper).
- **Acceptance:** MSDF cluster relocated; cross-cluster calls resolve via `pub(crate)`; build + tests + snapshots green; API diff empty.
- **VERIFY:** `cargo build --all-features && cargo test --all && cargo run --bin snapshots`
- **Complexity:** medium

### [ ] T004 — Extract `src/popup_render.rs`
- Move `render_popup_overlay` + popup-render methods (~3560-3748) into `src/popup_render.rs`; cross-called ones → `pub(crate)`. `PopupRuntimeState`/public popup types stay in popup.rs/lib.rs. Preserve cfg/attributes/docs.
- **Files:** new `src/popup_render.rs`; `src/lib.rs`.
- **Depends on:** T001.
- **Acceptance:** popup-render methods relocated; popup snapshots/tests green; API diff empty.
- **VERIFY:** `cargo build --all-features && cargo test --all && cargo run --bin snapshots`
- **Complexity:** medium

### [ ] T005 — Extract `src/glow.rs`
- Move the glow/halo method cluster — the PUBLIC entry methods (`draw_halo`, `draw_halo_for_object`, `draw_rect_glow`, …) AND their private helpers (`draw_halo_world_rect`, ~3998-4163) — into `src/glow.rs` (public methods keep their `PlutoniumEngine::` path; private helpers → `pub(crate)` only if still called from lib.rs). **Keep the public `HaloStyle/HaloMode/HaloPreset/HaloFalloff` TYPE definitions + their inherent impls in lib.rs** (D5). Move the cluster together so no public glow method is left in lib.rs calling a relocated private helper.
- **Files:** new `src/glow.rs`; `src/lib.rs`.
- **Depends on:** T001.
- **Acceptance:** glow method cluster relocated; public Halo* TYPE defs unmoved; halo snapshots/tests green; API diff empty.
- **VERIFY:** `cargo build --all-features && cargo test --all && cargo run --bin snapshots`
- **Complexity:** medium

### [ ] T006 — Extract `GpuTimer` into `src/gpu_timer.rs` (behavior-preserving)
- Create `pub(crate) struct GpuTimer` owning `timestamp_query/buf/staging/period_ns/count/frame_index` + `gpu_metrics`. Methods take `&Device`/`&mut Encoder`/`&Queue` as params (never `self`) to avoid split-borrow walls. `render()` calls `self.gpu_timer.begin(...)`/`resolve_and_record(...)`/`maybe_report()`. EXACT behavior: identical timestamp-feature gating (adapter-feature-conditional query creation), identical buffer map/unmap lifecycle, identical drop/encoder ordering, NO new per-frame allocation, identical `gpu_metrics` output. Does NOT fix the P4 `poll(Wait)` stall (separate task).
- **Files:** new `src/gpu_timer.rs`; `src/lib.rs` (remove 7 fields, add `gpu_timer`, update `new`/`render`).
- **Depends on:** T001.
- **Acceptance:** 7 timestamp fields collapsed to one `gpu_timer` field; render snapshots pass (no golden update); gpu-metrics report output identical for a fixed seed + frame count; diff review confirms no new per-frame `create_buffer*`/alloc; auto-trait guard still passes (GpuTimer must not change engine Send/Sync); API diff empty.
- **VERIFY:** `cargo test --all && cargo run --bin snapshots`
- **Complexity:** high

### [ ] T007 — Extract core render/draw into `src/render.rs` + `src/draw.rs`
- Move `render()` into `src/render.rs` — its function-local `flush_batch` closure and the three function-local `macro_rules!` (`flush_rect_batch!`/`flush_glow_batch!`/`flush_atlas_batch!`) travel inside the function body (no separate macro module). Move batch helpers `TransformPool`/`RectInstanceBuffer`/`RectStyleKey` + free fns `to_rgba_u8`/`quant_10x` (→ `pub(crate)` as needed). Move `draw_*`/`queue_*` immediate-mode methods into `src/draw.rs`. Keep any `impl Trait for PlutoniumEngine` (Drop/Debug) in lib.rs or render.rs — do not scatter. LAST extraction (widest internal deps).
- **Files:** new `src/render.rs`, `src/draw.rs`; `src/lib.rs`.
- **Depends on:** T002, T003, T004, T005, T006.
- **Acceptance:** render + draw/queue relocated; the local closure/macros compile in their new home; all snapshots pass (no golden update); API diff empty.
- **VERIFY:** `cargo build --all-features && cargo test --all && cargo run --bin snapshots`
- **Complexity:** high

### [ ] T008 — Final cleanup + full gate (host + wasm)
- Relocate any remaining stray private helper to its owning module; trim lib.rs to struct + fields + `new`/`new_async` + public type defs + `mod`/`pub use` decls; confirm `wc -l src/lib.rs` < ~1500; run the complete gate on host AND wasm32.
- **Files:** `src/lib.rs` + any module needing a stray item.
- **Depends on:** T007.
- **Acceptance:** `wc -l src/lib.rs` < ~1500; `scripts/check-api.sh` empty diff on host + wasm32; `cargo test --all` green; `cargo run --bin snapshots` passes (no golden update); `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` clean; auto-trait guard passes (engine still `!Send`).
- **VERIFY:** `bash scripts/check-api.sh && cargo test --all && cargo run --bin snapshots && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- **Complexity:** medium

---
**Ordering:** T001 → (T002 → T003) and (T004, T005, T006 independent) → T007 → T008. 8 tasks, within the 10-20 target. Each leaf task is independently gated against the T001 baseline, so failure isolation is clean.
