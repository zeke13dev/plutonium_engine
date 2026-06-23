# Audit — prod-readiness (plutonium_engine v0.8.0)

- **Date (UTC):** 2026-06-03T04:55Z
- **Target:** `/Users/zeke/dev/plutonium_engine` — pure-Rust wgpu 2D graphics/game engine, intended for crates.io publication + open-sourcing (`license = Apache-2.0`, `repository = github.com/zeke13dev/plutonium`, `documentation = docs.rs/plutonium_engine`).
- **Question:** what remains to make this engine production-ready / ready for a public release or open-sourcing.
- **Dimensions audited:** correctness, perf, cleanliness, design — plus an orchestrator-level release-engineering lens.
- **Rubric:** generic.
- **Mode:** MEDIUM (whole-crate; 4 parallel dimension auditors).

## Summary

The engine is functionally capable and the *architecture is sound* — the immediate-mode `begin_frame`/`draw_*`/`end_frame` loop with optional retained widgets is a reasonable public model that does **not** need a redesign. What's missing is almost entirely **library-hygiene and release-engineering**, not algorithmic rework. The headline gaps:

1. **It is undocumented as a library.** Zero `//!` crate docs, ~80% of ~422 `pub fn` undocumented, `documentation = docs.rs/...` would point at a near-empty page. This alone makes a crates.io release indefensible.
2. **It panics where a library must return `Result`.** Resource-creation (`create_texture_svg/atlas`, `create_text2d`, font/file load) `panic!`/`expect()` on foreseeable IO/font failure; `FontError` implements neither `Display` nor `std::error::Error`, so it can't even be used with `?`.
3. **It owns stdout.** 34+ `println!`/`eprintln!` in library paths (always-on `[FONT DEBUG]` block, a `gpu_…` line every 5s, frame-metrics every 5s, 17 WASM console logs). No `log` facade is used at all.
4. **One soundness bug.** `src/app.rs:782` uses `ptr::read` to bitwise-duplicate the frame callback `Box<dyn FnMut>` — double-drop on panic + aliasing UB; the single highest-risk correctness item.
5. **The "batching/instancing" claim is not real on the hot path.** Sprite and atlas/glyph flushes call `create_buffer_init` + `create_bind_group` *every flush every frame*; vertex buffers for every loaded texture/atlas are recreated every frame for constants that never change.
6. **It leaks its dependencies' types and internals into the public API.** `wgpu::SurfaceError`, `winit::keyboard::Key`, `winit::dpi::PhysicalSize` in public signatures; all `*Internal` structs and GPU-wire structs (`Vertex`, `RectInstanceRaw`, `DrawingContext`, …) are `pub`. Every wgpu/winit major bump breaks downstream.
7. **Open-source table-stakes files are missing.** No `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`; README ends with a literal `# CI Test Comment` junk line; CI has no wasm build, no doc build, no MSRV, and masks 6 clippy lint categories.

**Finding volume:** 55 findings (5 CRITICAL, 16 HIGH, 21 MED, 13 LOW across 4 dimensions) + 7 release-engineering items. This exceeds the audit auto-bail thresholds (>30 total / >10 CRITICAL+HIGH) — see `escalation.md`. The volume reflects *breadth of small hygiene fixes*, not deep structural rot; remediation is best run as a phased release-prep effort (`/z-plan`), with the surgical blockers also captured in `TASKS.md`.

## Verdicts

- correctness: **NEEDS-WORK** (1 soundness bug + panic-on-error API surface)
- perf:        **NEEDS-WORK** (per-frame GPU allocation on the hot path)
- cleanliness: **NEEDS-WORK** (god module, stdout pollution, dead code/features)
- design:      **REFACTOR (API hygiene only — keep the architecture)**
- release-eng: **NEEDS-WORK** (docs, CI, community files, packaging)

---

## Findings — correctness

### [CRITICAL] C1 — `ptr::read` of `Box<dyn FnMut>` → double-drop + aliasing UB
- **Location:** `src/app.rs:781-787`
- **Evidence:** `let mut callback = unsafe { std::ptr::read(&self.frame_callback) };` then `let frame_context = unsafe { &*frame_context_ptr };` then `callback(engine, frame_context, self)` (self passed `&mut`), then `ptr::write` restores. After the `ptr::read`, both `callback` and `self.frame_callback` own the same heap allocation → **double-drop if the callback panics** before the `ptr::write`. Separately, `&FrameContext` (derived from raw ptr to `self.frame_context`) is live while `&mut PlutoniumApp` (`self`) is passed to the callback → **aliasing UB** if the callback touches `frame_context` via any `&mut self` method. miri would flag this.
- **Recommendation:** Replace with `Option<FrameCallback>`: `take()` the callback, call it, restore it; or restructure the signature so `self` and `frame_context` aren't both needed.

### [HIGH] C2 — `padded_contains` inverts padding polarity (live click-region bug)
- **Location:** `src/utils.rs:333-338`, called from `src/button.rs:76`
- **Evidence:** left/top bound uses `self.x - padding` (expands outward) while right/bottom shrinks — region is asymmetrically grown on the near side. For `{x:10,w:100}, pad:5` left bound = 5. Buttons register clicks to the left/above their visible bounds.
- **Recommendation:** `position.x >= self.x + padding && position.x <= self.x + self.width - padding` (same for y).

### [HIGH] C3 — `Box::leak` permanently loses font bytes on error / accumulates over instances
- **Location:** `src/lib.rs:918`, `950-957`
- **Evidence:** `Box::leak(font_data.into_boxed_slice())` then a `?`-propagating call; on error the leaked slice is unrecoverable. Even on success the `'static` leak persists across `PlutoniumEngine` create/drop cycles → unbounded heap growth in long-running processes that reload fonts.
- **Recommendation:** Wrap in a `Drop`-cleanup newtype, or use `Arc<[u8]>` (rusttype supports it) for cleanup on failure.

### [HIGH] C4 — `uv_bind_groups.len() - 1` usize underflow when empty
- **Location:** `src/texture_atlas.rs:606`
- **Evidence:** `self.uv_bind_groups.len() - 1` inside a warning `println!`; when empty → `usize::MAX` (release) or panic (debug).
- **Recommendation:** `len().saturating_sub(1)`.

### [HIGH] C5 — Public API panics on user-recoverable conditions
- **Location:** `src/texture_atlas.rs:832`; `src/lib.rs:4377, 4532, 4614, 4226, 4251, 4274, 4291`; `src/texture_svg.rs:800`
- **Evidence:** e.g. `.unwrap_or_else(|_| panic!("file not found: {}", file_path))`, `panic!("Failed to load font")`, `fs::read_to_string(path).expect("file should exist")` — all inside public `create_*` paths reachable with bad-but-legitimate input (missing asset, wrong font key, invalid SVG path).
- **Recommendation:** Return `Result<T, E>` with a domain error type per Rust API guideline C-FAILURE. (Overlaps design D2.)

### [MED] C6 — `[FONT DEBUG]` println! fires unconditionally on every atlas build *(see also Perf P7, Cleanliness CL2)*
- **Location:** `src/text.rs:999-1097`
- **Recommendation:** gate behind `#[cfg(debug_assertions)]` / `log::debug!`.

### [MED] C7 — NDC/transform tests too weak to catch the regressions the CHANGELOG describes
- **Location:** `tests/transform_tests.rs:23-25`
- **Evidence:** asserts only `transform[0][0].abs() > 0.0` — passes through sign errors, double-scaling, wrong translation. CHANGELOG 0.8.0 explicitly fixed "NDC Math" / "double-scaling bugs"; no value-exact regression test guards them.
- **Recommendation:** assert exact NDC outputs (e.g. tile==viewport → `transform[0][0]==2.0`; known centering offset).

### [LOW] C8 — `Rectangle::pad` has translate+asymmetric-grow semantics (currently unused)
- **Location:** `src/utils.rs:366-373`. **Recommendation:** define shrink-or-expand intent and implement symmetrically.

### [LOW] C9 — Replay does not reconstruct `pressed_keys`
- **Location:** `src/app.rs:737` (`pressed_keys.clear(); // skip key reconstruction`). Replay silently drops keyboard input. **Recommendation:** parse key names back, or document the limitation.

---

## Findings — perf

### [CRITICAL] P1 — Per-flush GPU buffer + bind group creation in sprite batch (hot path)
- **Location:** `src/lib.rs:2946-2961` (`flush_batch`). Calls `create_buffer_init` + `create_bind_group` on every flush every frame. Only the rect path pools.
- **Recommendation:** pool instance buffers like `flush_rect_batch!`/`rect_instance_pool`.

### [CRITICAL] P2 — Per-flush GPU buffer + bind group creation in atlas/glyph batch (hot path)
- **Location:** `src/lib.rs:3187-3226` (`flush_atlas_batch!`). Creates instance buffer+bg and an id buffer+bg per flush.
- **Recommendation:** same pooled-buffer strategy; create the identity UBO once at startup.

### [CRITICAL] P3 — Per-frame vertex-buffer recreation for every loaded texture/atlas
- **Location:** `src/texture_svg.rs:638-646` + `src/texture_atlas.rs:515-523` (`update_vertex_buffer`), called from `update_transform_uniform` for every entry of `texture_map`/`atlas_map` at `src/lib.rs:2199-2214`. The written vertices are hardcoded constants (`[0,0],[1,0],[0,-1],[1,-1]`) — the recreation is unconditional and pointless.
- **Recommendation:** create the vertex buffer once at construction; never recreate (transform already goes through `queue.write_buffer`).

### [HIGH] P4 — Synchronous `device.poll(Wait)` stall per frame for timestamp readback
- **Location:** `src/lib.rs:3460-3498` (after `frame.present()`). Blocks CPU until GPU drains, every frame timestamps are active.
- **Recommendation:** double-buffer the staging readback (read frame N-2); never `poll(Wait)` on the render thread between present and next begin.

### [HIGH] P5 — Per-frame `Vec<char>` allocations in `calculate_text_layout`/`measure_text`
- **Location:** `src/text.rs:1430,1451,1501,1591,1639,1712,1757,1868,1914,1953`; `843,909`. ~2×N `Vec<char>` per text draw per frame.
- **Recommendation:** iterate `line.chars()` without collecting; cache layout for static text.

### [HIGH] P6 — `resolve_font_key_for_render` allocates a `String` per text draw
- **Location:** `src/lib.rs:1201-1229`, called from `queue_text_with_spacing` (`:2764`). Returns owned `String` even on steady-state exact match.
- **Recommendation:** return `Cow<str>`/`&str` on the exact-match path.

### [MED] P7 — `[FONT DEBUG]` block includes an O(atlas_w×h×4) pixel scan, active in release
- **Location:** `src/text.rs:999-1100` (non-zero-byte count at `:1096`). *(Overlaps C6/CL2.)* **Recommendation:** gate behind `#[cfg(debug_assertions)]`; drop the O(N) scan from non-debug builds.

### [MED] P8 — Full render queue stable-sort every frame
- **Location:** `src/lib.rs:2882` (`sort_by`). Queue rebuilt each frame; items usually already ordered. **Recommendation:** `sort_unstable_by` (confirm no intra-z order dependency) or bucket by z-layer.

### [MED] P9 — `Text2DInternal::update` calls `measure_text` every frame for static text
- **Location:** `src/pluto_objects/text2d.rs:740-750`. The `!auto_size && !wrap` branch re-measures even when nothing changed. **Recommendation:** `dimensions_valid` flag.

### [MED] P10 — `process_runtime_raster_warm_queue` allocates a `HashMap` budget tracker per call
- **Location:** `src/lib.rs:1236, 1289-1293` (called every `begin_frame`). **Recommendation:** fixed-size/stack structure; rate-limit/route the error print.

### [MED] P11 — Rect identity UBO/bind-group rebuilt on first rect each frame
- **Location:** `src/lib.rs:3077-3102`, cleared to `None` in `begin_frame` (`:3508`). **Recommendation:** keep alive across frames; recreate only on resize.

### [LOW] P12 — `flush_glow_batch!` always creates a new buffer+bg (no pool)
- **Location:** `src/lib.rs:3125-3139`. **Recommendation:** add a glow pool.

### [LOW] P13 — SVG rasterization confirmed load-time only (no action)
- **Location:** `src/texture_svg.rs:790-870`. Correctly one-time; confirm `update_text` only runs on user change.

---

## Findings — cleanliness

### [HIGH] CL1 — `src/lib.rs` is a 5531-line god module
- **Location:** `src/lib.rs:1-5531` (140 methods, 12+ internal structs, font pipeline + popup + halo/glow + GPU-timer all inline).
- **Recommendation:** extract raster-font (~1066-1630)→`font_raster.rs`, GPU timer→`gpu_timer.rs`, popup render (~3560-3748)→`popup_render.rs`, halo/glow (~3998-4163)→`glow.rs`. *(Overlaps design D4.)*

### [HIGH] CL2 — 10 always-on `[FONT DEBUG]` println! per font atlas build
- **Location:** `src/text.rs:999-1097`. **Recommendation:** delete or gate behind `debug-text` feature / `log::debug!`.

### [HIGH] CL3 — Live `println!("gpu_{}", line)` every ~5s in production
- **Location:** `src/lib.rs:3491` (period `report_period_secs = 5.0`, `:5494`). **Recommendation:** remove or gate.

### [HIGH] CL4 — Two hollow feature flags (`backend-wgpu`, `replay`) gate zero code
- **Location:** `Cargo.toml:57-62`. No `#[cfg(feature=...)]` guards exist for either. **Recommendation:** either wire the gates (esp. `replay` around `src/app.rs:268-845`) or delete the features from Cargo.toml + README. *(Overlaps design D5.)*

### [HIGH] CL5 — `src/button.rs` is a 99-line orphan that never compiles
- **Location:** `src/button.rs:1-99` (no `mod button;` at crate root; canonical Button is `pluto_objects/button.rs`). **Recommendation:** delete.

### [MED] CL6 — `snapshots/actual/` (664 KB, 27 PNGs) committed and not gitignored
- **Location:** `.gitignore`, `snapshots/actual/`. **Recommendation:** `git rm -r --cached snapshots/actual/` + add to `.gitignore`.

### [MED] CL7 — 50 per-field `#[allow(dead_code)]` on GPU bytemuck structs
- **Location:** `src/utils.rs:38-132`. **Recommendation:** single module-level `#![allow(dead_code)]` or remove once confirmed live via bytemuck.

### [MED] CL8 — README ends with junk line `# CI Test Comment`
- **Location:** `README.md` last line. **Recommendation:** delete.

### [MED] CL9 — `draw_*` are undocumented thin wrappers over `queue_*`
- **Location:** `src/lib.rs:3770-3870` (7 one-line forwarders). **Recommendation:** document the intent or collapse and `#[doc(hidden)]`/`pub(crate)` one side.

### [MED] CL10 — `src/app.rs` frame-metrics `println!` every 5s in live apps
- **Location:** `src/app.rs:817-819`. **Recommendation:** route through `log`.

### [MED] CL11 — Public `debug_print_text_line_layout` prints unconditionally
- **Location:** `src/lib.rs:801-848`. **Recommendation:** return `Vec<String>` instead of printing.

### [MED] CL12 — `pluto_objects` Internal/Wrapper boilerplate duplicated 4×
- **Location:** `texture_2d.rs`, `texture_atlas_2d.rs`, `shapes.rs`, `text2d.rs`. **Recommendation:** macro/blanket impl, or document why the pattern exists.

### [MED] CL13 — `env_logger` is a hard dependency but only used by an example
- **Location:** `Cargo.toml:28`, `examples/jitter_test.rs:110`. **Recommendation:** move to `[dev-dependencies]`.

### [LOW] CL14 — Non-debug production `println!` in tiny-raster fallback
- **Location:** `src/text.rs:514-522`. **Recommendation:** `log::debug!` or remove.

### [LOW] CL15 — `save_debug_png("debug_atlas.png")` called unconditionally on atlas build
- **Location:** `src/lib.rs:4511-4512` (`let _ =` hides errors). **Recommendation:** delete or gate behind `debug_assertions`.

### [LOW] CL16 — `Cargo.toml` excludes non-existent `NEXTREADMENEXT.md`
- **Location:** `Cargo.toml:18`. **Recommendation:** remove the stale exclude entry.

### [LOW] CL17 — Bare debug `println!` in `Text2D` public path
- **Location:** `src/pluto_objects/text2d.rs:961`. **Recommendation:** delete.

### [LOW] CL18 — Unconditional `println!` on atlas pack
- **Location:** `src/texture_atlas.rs:603`. **Recommendation:** delete / `log::debug!`.

### [LOW] CL19 — 17 `wasm_log`/`console::log_1` progress strings on every WASM init
- **Location:** `src/lib.rs:4929-5447`. **Recommendation:** gate behind `debug_assertions` / `wasm-debug` feature.

### [LOW] CL20 — `button.rs` (orphan) carries a `// TODO: Render text properly` + silent label drop
- **Location:** `src/pluto_objects/button.rs:167-173`. Button label text silently dropped via texture-only fallback. **Recommendation:** implement label render or document the gap (resolves with CL5 cleanup scope).

---

## Findings — design

### [CRITICAL] D1 — `FontError` implements neither `Display` nor `std::error::Error`
- **Location:** `src/text.rs:69-79`. Unusable with `?`; violates the Rust error contract.
- **Recommendation:** add `impl Display` + `impl std::error::Error { source }`. Consider storing `io::ErrorKind`+`String` so it can derive `Clone` (D-extra, `src/text.rs:70-79`).

### [CRITICAL] D2 — Resource-creation panics instead of returning `Result`
- **Location:** `src/lib.rs:4226,4251,4274,4291,4377,4532,4614`. *(Same surface as correctness C5; tracked once.)*
- **Recommendation:** `create_texture_svg/atlas/atlas_2d`, `create_text2d`, `create_texture_raster_from_path` → `Result<…, EngineError>`.

### [CRITICAL] D3 — Zero crate-level docs; `documentation` URL points at a near-empty page
- **Location:** `src/lib.rs:1-70`; `Cargo.toml:9`. ~95 `pub fn` on `PlutoniumEngine`, vast majority undocumented; no `//!`.
- **Recommendation:** add `//!` overview + quick-start + feature list; add `#![warn(missing_docs)]`; document every public method on the engine.

### [HIGH] D4 — `PlutoniumEngine<'a>` is a 5531-line god object
- **Location:** `src/lib.rs:499-5531`. *(Overlaps cleanliness CL1.)* **Recommendation:** extract `FontCache`, `TextureRegistry`, `PopupController`, `ClipStack` sub-structs the engine delegates to (need not change public API).

### [HIGH] D5 — `backend-wgpu` is an empty, unenforceable feature seam
- **Location:** `Cargo.toml:57`. *(Overlaps CL4.)* **Recommendation:** remove or actually gate wgpu code behind it.

### [HIGH] D6 — Inconsistent creation contract: raw `(Uuid, Rectangle)` vs typed wrappers
- **Location:** `src/lib.rs:4213` (`create_texture_svg`→tuple) vs `:4576/4599/4676` (`create_texture_2d/text2d/button`→objects). Two silently-parallel idioms.
- **Recommendation:** converge on the typed-wrapper idiom; deprecate/doc the raw tuple form.

### [HIGH] D7 — All `*Internal` structs are `pub`
- **Location:** `pluto_objects/{texture_2d,shapes,text2d,button,texture_atlas_2d}.rs`. Implementation detail leaked into docs/contract. **Recommendation:** `pub(crate)`.

### [HIGH] D8 — GPU-wire structs exported via `pub mod utils` + `use utils::*`
- **Location:** `src/utils.rs:31-131` (`DrawingContext`, `UVTransform`, `Vertex`, `TransformUniform`, `InstanceRaw`, `RectInstanceRaw`, `GlowInstanceRaw`); `src/lib.rs:60`. **Recommendation:** `pub(crate)` + `#[doc(hidden)]` where they must stay `pub`; drop the glob.

### [HIGH] D9 — `wgpu`/`winit` types in public signatures (dependency-bump treadmill)
- **Location:** `src/lib.rs:2832, 3522` (`wgpu::SurfaceError`), `:2121` (`&PhysicalSize<u32>`), `:2136` (`&Option<winit::keyboard::Key>`), `:500` (`pub size: PhysicalSize<u32>`). **Recommendation:** crate-owned `EngineError` + geometry/key newtypes (or `#[doc(hidden)]` re-exports with a stability note).

### [MED] D10 — No MSRV (`rust-version`) declared
- **Location:** `Cargo.toml`. **Recommendation:** set `rust-version` (verify with `cargo-msrv`) + CI job.

### [MED] D11 — `warm_text_cache` returns stringly-typed error
- **Location:** `src/lib.rs:994-998` (`Result<WarmStats, String>`). **Recommendation:** `Result<WarmStats, FontError>`.

### [MED] D12 — WASM `new` compiles then panics at runtime
- **Location:** `src/lib.rs:4912-4920`. **Recommendation:** `compile_error!` or remove from the wasm target so misuse is caught at compile time.

### [MED] D13 — Split WASM/native API is incoherent at the boundary
- **Location:** `src/app.rs:862,928`; `src/lib.rs:4883,4922`. `FrameContext::pressed_keys: Vec<winit::keyboard::Key>` leaks winit. **Recommendation:** platform-agnostic `run_app` that branches internally; `Keys` newtype.

### [MED] D14 — `CharacterInfo`/`MsdfGlyphInfo` public but internal atlas-packing types
- **Location:** `src/text.rs:17-31`, in `create_font_texture_atlas` signature. **Recommendation:** `pub(crate)` (and make the method `pub(crate)`).

### [LOW] D15 — Record/replay types public with no stability signal
- **Location:** `src/app.rs:27-39`. **Recommendation:** `#[doc(hidden)]` or gate behind the `replay` feature.

### [LOW] D16 — `resize`/`update` are `pub` and lock in winit types
- **Location:** `src/lib.rs:2121,2136`. **Recommendation:** `pub(crate)` unless a documented custom-event-loop surface is intended.

---

## Findings — release-engineering (orchestrator lens)

### [HIGH] RE1 — Open-source table-stakes files missing
- **Evidence:** no `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md` (checked repo root). A public repo without these signals an unmaintained/closed project and gives contributors no on-ramp.
- **Recommendation:** add all three (CONTRIBUTING: build/test/snapshot/wasm workflow + PR expectations; standard CoC; SECURITY: disclosure contact). An issue/PR template under `.github/` is a plus.

### [HIGH] RE2 — CI does not cover the surfaces the crate advertises
- **Evidence:** `.github/workflows/ci.yml` runs fmt/clippy/test/snapshots on `ubuntu-latest` only. **No wasm32 build** (despite a large documented WASM surface + `wasm` feature), **no `cargo doc` build** (would catch the missing-docs/broken-intra-doc-links problem), **no MSRV job**, **no feature-matrix build** (features claim to be additive but are never tested in combination or in isolation), **no `cargo package`/publish dry-run**. Clippy masks 6 lint categories via `-A` (`too_many_arguments`, `type_complexity`, `explicit_auto_deref`, `manual_clamp`, `collapsible_else_if`, `derivable_impls`).
- **Recommendation:** add jobs: `cargo check --target wasm32-unknown-unknown --features wasm`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`; an MSRV job; a small feature-matrix (`--no-default-features`, each feature alone, `--all-features`); `cargo package` dry-run. Reduce blanket `-A` allows once the underlying lints are addressed.

### [MED] RE3 — `examples/media/` (992 KB of ttf/png/svg) ships in the published crate
- **Evidence:** `Cargo.toml` `exclude` lists `.github/**`, `snapshots/**`, `plutonium_game/**` but **not** `examples/media/**`; `cargo package` includes `examples/` by default, so ~1 MB of binary assets ship in every download.
- **Recommendation:** either exclude `examples/media/**` (and gate heavy examples) or switch to an `include = [...]` allowlist so the package ships only `src/`, `shaders/`, `README`, `LICENSE`. Verify with `cargo package --list`.

### [MED] RE4 — README has no crates.io install path or badges
- **Evidence:** README only shows a path dependency (`{ path = "../path/to/..." }`); no `cargo add plutonium_engine` / version line, no CI/crates.io/docs.rs badges.
- **Recommendation:** add a crates.io install snippet and standard badges once published.

### [MED] RE5 — `Cargo.lock` is both tracked and gitignored
- **Evidence:** `.gitignore` contains `Cargo.lock` yet `Cargo.lock` is a tracked file. For a library, the convention is to not commit it; the current state is contradictory.
- **Recommendation:** decide one policy (untrack for a pure library) and make `.gitignore` consistent.

### [MED] RE6 — `plutonium_game/` (85 files) lives in the engine repo
- **Evidence:** a separate demo-game workspace, excluded from the package but present in the repo and workspace. Muddies the "this is an engine" open-source story and roughly doubles repo file count.
- **Recommendation:** decide whether the demo game belongs in this repo (as `examples/` or a sibling repo) before going public; if it stays, document its relationship in the README.

### [LOW] RE7 — `unsafe` usage undocumented at the policy level
- **Evidence:** 4 `unsafe` blocks (all `src/app.rs:779-790`) with terse inline comments; no crate-level statement of the unsafe policy. (C1 is the substantive fix; this is about signaling.)
- **Recommendation:** after fixing C1, document remaining `unsafe` with `// SAFETY:` invariants and state the policy in CONTRIBUTING.

---

## Cross-dimension findings

- **stdout pollution** is flagged by correctness (C6), perf (P7), and cleanliness (CL2/CL3/CL10/CL11/CL14-CL19) — the *systemic* fix is "adopt the `log` facade, delete/gate every `println!`/`eprintln!` in `src/`," which resolves ~12 individual findings at once. Highest leverage cleanup.
- **god module** is flagged by cleanliness (CL1) and design (D4) — one decomposition effort.
- **panic-on-error / error-type** is flagged by correctness (C5) and design (D1/D2/D11) — one coherent `EngineError`/`FontError` redesign resolves the cluster.
- **dead `replay`/`backend-wgpu` features** flagged by cleanliness (CL4) and design (D5).
- **`Box::leak` font lifetime** flagged by correctness (C3) and design (cross-note).

## Findings — consult additions (Gemini + Codex, verified)

Cross-LLM consult ran directly against the `gemini` and `codex` CLIs (the z-harness provider registry was unbound, so the formal consultant subagents were bypassed and the CLIs invoked directly; transcripts in `archive/<run>/transcripts/consult-{gemini,codex}.txt`). Both models independently proposed the same ~6 "missing" items. Each was then checked against the code ("one reason this might be wrong"). Results:

**DROPPED after verification (consult was wrong — already implemented):**
- *Surface/device-loss recovery* — both models flagged it as missing; it is **already handled** at `src/lib.rs:2841-2849` (`Lost | Outdated => surface.configure(...)`, plus `OutOfMemory`/`Timeout` arms). Not a finding.
- *HiDPI / scale-factor-change handling* — both flagged; **already handled** at `src/app.rs:394 apply_dpi_change` / `:405` ("Always run resize/reconfigure on DPI changes"). Not a finding.

**ACCEPTED after verification (verified-real, added below):**

### [MED] CS1 — sRGB / color-space inconsistency between sprite textures and the font atlas
- **Location:** sprite/SVG/atlas textures created as `Rgba8Unorm` (`src/lib.rs:1697,1753,1810,1870,2044`) vs font atlas `Rgba8UnormSrgb` (`src/lib.rs:4397`); surface format prefers sRGB (`src/lib.rs:5019-5028`).
- **Evidence:** with an sRGB surface, a `Rgba8Unorm` (linear) sprite texture is sampled without the sRGB→linear decode that the `Rgba8UnormSrgb` font atlas gets — so sprite color and alpha blending can be mathematically inconsistent with text (washed-out / over-dark sprites, or vice versa). Classic wgpu 2D gotcha.
- **Recommendation:** pick one color-space convention; verify on-screen output against a known-color reference (a sprite and text of the same RGBA should match). Likely upload color textures as `Rgba8UnormSrgb` (or decode consistently). Confirm with a snapshot diff before/after.

### [MED] CS2 — Bundled font/media assets ship without their licenses
- **Location:** `examples/media/` (`roboto.ttf`, `roboto.msdf.*`, several SVGs/PNG); no `LICENSE`/`OFL`/`NOTICE` alongside them. Combined with RE3 (`examples/` ships in `cargo package`), the crate would redistribute Roboto without its license text.
- **Evidence:** `ls examples/media/` shows no license files; Roboto is Apache-2.0 and requires the license/NOTICE to be carried with redistribution.
- **Recommendation:** add the asset licenses (e.g. `examples/media/Roboto-LICENSE.txt`) and a credits note in README; OR exclude `examples/media/**` from the package (see RE3) and document asset provenance in the repo. Required for a clean open-source/crates.io release.

### [MED] CS3 — No `[package.metadata.docs.rs]`; docs.rs build may fail for a wgpu crate
- **Location:** `Cargo.toml` (absent).
- **Evidence:** wgpu/native-GPU crates frequently fail to build on docs.rs without explicit docs.rs target/feature configuration; combined with D3 (`documentation = docs.rs/...`), a failed docs build leaves the advertised docs URL broken.
- **Recommendation:** add `[package.metadata.docs.rs]` (e.g. `all-features = true`, appropriate `targets`/`rustdoc-args`) and confirm a local `cargo doc --all-features` plus a docs.rs dry-run succeeds. Pairs with D3/T009 and the CI doc job (RE2/T017).

### [LOW] CS4 — `Send`/`Sync` thread-affinity contract undocumented
- **Location:** wrapper objects use `Rc<RefCell<…>>` (18 sites), so the public objects are `!Send`/`!Sync`; the engine holds wgpu/winit handles.
- **Evidence:** users integrating with ECS/async/multi-thread setups will hit `!Send` immediately with no documentation explaining the single-thread affinity.
- **Recommendation:** document the thread-affinity contract in the crate docs (D3); if a `Send` path is ever intended, it is a larger design item, not part of this release.

## Consult drops / demotions (applied)

Both models converged on demoting these; applied to severities above and to `TASKS.md`:
- **RE1** (CONTRIBUTING/CoC/SECURITY): demoted HIGH→MED — expected for *open-sourcing* (the stated goal) but **not** a crates.io blocker. Keep, lower priority.
- **RE4** (badges): badge portion demoted to cosmetic; the README crates.io install/quick-start portion is the part that matters (kept in T019).
- **RE6** (`plutonium_game` in repo): demoted to LOW/informational — a dogfood demo in-workspace is normal (cf. Bevy); only matters if it bloats the package or breaks CI (it is already package-excluded).
- **P5/P6/P9/P8/P10** (text/queue micro-allocations): kept, but tagged "benchmark to confirm hot-path impact" — the CRITICAL GPU-allocation findings (P1-P3) were *not* challenged by either model.
- **C7** (weak transform tests): both called it vague; kept because the 0.8.0 CHANGELOG names the exact bug class (NDC/double-scaling) to pin — see T020.
- **CL14-CL19** (scattered prints): both recommended merging into one logging finding — already done (`TASKS.md` T006 bundles them).

Net: consult removed 2 false "missing" items, added 4 verified findings (CS1-CS4), and validated the report's clustering. Final tally: **59 findings** (5 CRITICAL, 16 HIGH, 24 MED, 14 LOW).
