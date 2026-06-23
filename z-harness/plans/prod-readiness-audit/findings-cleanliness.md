# Cleanliness audit findings

**Target:** plutonium_engine v0.8.0 — pure-Rust wgpu 2D graphics/game engine; `/Users/zeke/dev/plutonium_engine/.claude/worktrees/eager-tharp-b3022f`
**Rubric:** generic checklist (production-readiness / open-source-readiness framing)
**Date (UTC):** 2026-06-02T00:00Z

## Summary

- `src/lib.rs` at 5531 lines is a true god-module: 140 methods, 28 top-level items, and a dozen internal structs all in one file — a first-time contributor has no map to navigate it.
- 34 `println!`/`eprintln!` calls survive in library code, including a dense `[FONT DEBUG]` block (10 lines, always-on), a per-character layout trace that fires on every `debug_print_text_line_layout` call, and a live `println!("gpu_{}", line)` that fires every ~5 s in production GPU timer code. A library must not own stdout.
- Two hollow feature flags (`backend-wgpu`, `replay`) are declared in Cargo.toml and documented in README but gate exactly zero code — enabling or disabling them has no effect.
- `src/button.rs` (99 lines) is an unreachable orphan: the crate-root file is never declared as a module; a contributor who finds it will be confused about the canonical Button type.
- `snapshots/actual/` (664 KB, 27 PNGs) is committed and not gitignored — this is test-run output, not source, and will accumulate noise with each CI run.
- README ends with `# CI Test Comment` — an accidental commit artifact on the last line of the primary documentation.

## Findings

### [HIGH] `src/lib.rs` is a 5531-line god module with no internal decomposition

- **Location:** `src/lib.rs:1-5531`
- **Evidence:** 140 methods on `PlutoniumEngine`, 12+ internal structs (`TransformPool`, `RectInstanceBuffer`, `RectStyleKey`, `RasterFontFamily`, `PendingRasterWarmRequest`, etc.), font-loading logic (lines 875–1630), popup rendering (lines 3560–3748), halo/glow rendering (lines 3998–4163), and GPU timer code (lines 3440–3501) all coexist in one file. Sub-modules `camera`, `text`, `renderer`, `popup` exist but the engine's core is not split.
- **Recommendation:** Extract the raster-font pipeline (~lines 1066–1630) into `src/font_raster.rs`, GPU-timer logic into `src/gpu_timer.rs`, popup rendering (~lines 3560–3748) into `src/popup_render.rs`, and halo/glow (~lines 3998–4163) into `src/glow.rs`; leave `PlutoniumEngine` struct + core draw dispatch in `lib.rs`.

### [HIGH] 10 unconditional `[FONT DEBUG]` `println!` calls fire on every font atlas build

- **Location:** `src/text.rs:999-1097`
- **Evidence:**
  ```rust
  println!("[FONT DEBUG] Atlas size: {}x{}", atlas_width, atlas_height);
  println!("[FONT DEBUG] Scale: {:?}", scale);
  println!("[FONT DEBUG] Max tile size: {}x{}", max_width, max_height);
  // ... 7 more, including per-character prints for first 5 glyphs
  println!("[FONT DEBUG] Non-zero bytes in texture: {}/{}", non_zero_pixels, texture_data.len());
  ```
- **Recommendation:** Delete all `[FONT DEBUG]` prints; if introspection is needed, gate the block behind `#[cfg(feature = "debug-text")]` or route through `log::debug!`.

### [HIGH] Live `println!("gpu_{}", line)` fires every ~5 seconds in production GPU timer path

- **Location:** `src/lib.rs:3491`
- **Evidence:**
  ```rust
  if let Some(line) = self.gpu_metrics.maybe_report() {
      println!("gpu_{}", line);
  }
  ```
  `maybe_report` fires every 5 s (`report_period_secs = 5.0`, line 5494). This is always-on stdout pollution from a library crate.
- **Recommendation:** Remove the `println!` or gate it behind a `debug` feature; route through `log::debug!` if retained.

### [HIGH] Two hollow feature flags (`backend-wgpu`, `replay`) gate zero code

- **Location:** `Cargo.toml:57-62`
- **Evidence:**
  - `backend-wgpu = []` — zero `#[cfg(feature = "backend-wgpu")]` guards exist anywhere in `src/`.
  - `replay = []` — zero `#[cfg(feature = "replay")]` guards exist anywhere in `src/`; the full record/replay machinery in `src/app.rs:268-845` compiles unconditionally.
  - Confirmed by: `grep -rn "feature = \"replay\"\|feature = \"backend-wgpu\"" src/` returns empty.
- **Recommendation:** Either add the `#[cfg(feature = "replay")]` gates around `start_recording`, `stop_recording`, `replay_frames`, etc. in `src/app.rs`, or remove the `replay` and `backend-wgpu` feature declarations from Cargo.toml and README.

### [HIGH] `src/button.rs` is a 99-line orphan file that never compiles

- **Location:** `src/button.rs:1-99`
- **Evidence:** `pub mod pluto_objects { pub mod button; ... }` at `src/lib.rs:2-4` resolves to `src/pluto_objects/button.rs`. No `mod button;` declaration exists at crate root, so `src/button.rs` is never compiled. It defines a different `Button` struct with a `String`-keyed texture.
- **Recommendation:** Delete `src/button.rs`; it is dead code that will confuse contributors looking for the canonical `Button` implementation.

### [MED] `snapshots/actual/` (664 KB, 27 PNGs) is committed and not gitignored

- **Location:** `.gitignore`, `snapshots/actual/`
- **Evidence:** `git ls-files snapshots/actual/` lists 27 tracked PNG files totalling 664 KB. `.gitignore` does not contain `snapshots/actual/`. These are test-run artifacts that change on every local or CI snapshot run.
- **Recommendation:** Add `snapshots/actual/` to `.gitignore` and remove the tracked files with `git rm -r --cached snapshots/actual/`.

### [MED] 50 `#[allow(dead_code)]` suppressions blanketing entire GPU struct fields in `src/utils.rs`

- **Location:** `src/utils.rs:38-132`
- **Evidence:** Every field of `UVTransform`, `Vertex`, `TransformUniform`, `InstanceRaw`, `RectInstanceRaw`, `GlowInstanceRaw` is individually annotated `#[allow(dead_code)]`. These are `pub` `repr(C)` bytemuck structs — the fields are dead only because `use utils::*` in `lib.rs` pulls them in but the compiler sees no direct Rust reads.
- **Recommendation:** Replace per-field `#[allow(dead_code)]` with a single `#![allow(dead_code)]` at the top of `utils.rs`, or (preferred) remove the attribute entirely once the GPU structs are confirmed live via bytemuck casts.

### [MED] README ends with bare junk line `# CI Test Comment`

- **Location:** `README.md:last line`
- **Evidence:** Final line of README is literally `# CI Test Comment` with no content following it — an accidental leftover from a CI test commit.
- **Recommendation:** Delete the final line of `README.md`.

### [MED] `draw_*` API is a thin alias layer over `queue_*` with no documented rationale

- **Location:** `src/lib.rs:3770-3870`
- **Evidence:**
  ```rust
  pub fn draw_texture_stretched(&mut self, texture_key: &Uuid, dst: Rectangle) {
      self.queue_texture_stretched(texture_key, dst);
  }
  pub fn draw_texture_stretched_with_fit_and_inset(...) {
      self.queue_texture_stretched_with_layer_and_fit(...);
  }
  ```
  Seven `draw_*` methods are single-line wrappers around same-named `queue_*` methods with no added behaviour. No comment explains whether `draw_*` is the stable API and `queue_*` is internal, or vice versa.
- **Recommendation:** Document the intent (e.g., `queue_*` for retained-mode objects, `draw_*` for immediate-mode callers), or collapse the wrappers and expose only one name; mark the other `#[doc(hidden)]` or `pub(crate)`.

### [MED] `src/app.rs` frame-metrics `println!` fires to stdout every 5 s in live apps

- **Location:** `src/app.rs:817-819`
- **Evidence:**
  ```rust
  if let Some(line) = self.metrics.maybe_report() {
      println!("{}", line);
  }
  ```
  `FrameTimeMetrics::new(600, 5.0)` at line 205. This produces `frame_metrics p50_ms=... avg_fps=...` on every user's terminal.
- **Recommendation:** Route through `log::info!` (gated by adding `log` to `[dependencies]`) or remove; do not leave bare `println!` in a library's application loop.

### [MED] `[TEXT LAYOUT DEBUG]` per-glyph trace is a public `debug_print_text_line_layout` method that calls `println!` unconditionally

- **Location:** `src/lib.rs:801-848`
- **Evidence:**
  ```rust
  pub fn debug_print_text_line_layout(...) {
      println!("[TEXT LAYOUT DEBUG] font='{}' ...", ...);
      for rec in records {
          println!("  #{:02} {} '{}'->'{}' ...", ...);
      }
  }
  ```
  Public method on the engine; prints directly to stdout with no opt-out.
- **Recommendation:** Return the formatted `String`s as a `Vec<String>` (or a structured type) instead of printing; the caller decides what to do with the output.

### [MED] `pluto_objects` Internal/Wrapper split is duplicated across four types with identical boilerplate

- **Location:** `src/pluto_objects/texture_2d.rs:8-125`, `src/pluto_objects/texture_atlas_2d.rs:9-174`, `src/pluto_objects/shapes.rs:16-254`, `src/pluto_objects/text2d.rs:132-1029`
- **Evidence:** Each type has an `XInternal` struct, an `impl XInternal`, an `impl PlutoObject for XInternal`, and a wrapper `X { internal: Rc<RefCell<XInternal>> }` with pass-through methods `set_pos`, `set_dimensions`, `get_id`, `render`, `get_z`, `set_z`. The pattern repeats verbatim four times with no shared macro or trait blanket.
- **Recommendation:** Introduce a `#[derive]` proc-macro or a blanket `impl<T: PlutoObject> ObjectWrapper<T>` to collapse the boilerplate; or at minimum add a comment explaining why the pattern exists so contributors do not invent a fifth variant.

### [MED] `env_logger` is a hard `[dependencies]` (not `[dev-dependencies]`) but is only called from an example

- **Location:** `Cargo.toml:28`, `examples/jitter_test.rs:110`
- **Evidence:** `env_logger = "0.11.6"` in `[dependencies]`; only use is `env_logger::init()` in `examples/jitter_test.rs`. Every downstream crate that depends on `plutonium_engine` will pull in `env_logger`.
- **Recommendation:** Move `env_logger` to `[dev-dependencies]`.

### [LOW] `println!` calls in `src/text.rs:514-522` are non-debug production paths

- **Location:** `src/text.rs:514-522`
- **Evidence:**
  ```rust
  println!("Using fallback tiny-raster for '{}' ...", font_key);
  println!("Raster fallback selected for ...");
  ```
  These fire on every tiny-raster fallback selection, which is a normal runtime path, not an error.
- **Recommendation:** Convert to `log::debug!` or remove.

### [LOW] `save_debug_png("debug_atlas.png")` is called unconditionally inside `create_font_texture_atlas`

- **Location:** `src/lib.rs:4511-4512`
- **Evidence:**
  ```rust
  let _ = atlas.save_debug_png(&self.device, &self.queue, "debug_atlas.png");
  ```
  Writes a file to the working directory every time a font atlas is built on non-WASM debug builds. The `let _` discards errors, hiding failures silently.
- **Recommendation:** Delete this line or gate it behind a compile-time `#[cfg(debug_assertions)]` and a `debug_atlas` feature.

### [LOW] `NEXTREADMENEXT.md` is excluded from the published crate but does not exist

- **Location:** `Cargo.toml:18`
- **Evidence:** `exclude = [..., "NEXTREADMENEXT.md", ...]` — the file does not exist in the repo. The entry is noise left over from a draft workflow.
- **Recommendation:** Remove the `"NEXTREADMENEXT.md"` entry from `Cargo.toml`'s `exclude` list.

### [LOW] `pluto_objects/text2d.rs:961` has a `println!("{:?}", self.internal.borrow().get_container())`

- **Location:** `src/pluto_objects/text2d.rs:961`
- **Evidence:**
  ```rust
  println!("{:?}", self.internal.borrow().get_container());
  ```
  Bare debug print in a public method on `Text2D`; fires whenever `get_container()` is called through that code path.
- **Recommendation:** Delete the `println!`.

### [LOW] `texture_atlas.rs:603` has an unconditional `println!` on atlas pack

- **Location:** `src/texture_atlas.rs:603`
- **Evidence:**
  ```rust
  println!("Atlas packed {} tiles into {}x{}", ...);
  ```
- **Recommendation:** Delete or convert to `log::debug!`.

### [LOW] `wasm_log` / `web_sys::console::log_1` debug strings fire on every engine init (17 calls)

- **Location:** `src/lib.rs:4929-5447` (17 `wasm_log`/`console::log_1` calls)
- **Evidence:** `wasm_log("pluto new_with: start")`, `wasm_log("pluto new_async: requesting adapter ...")`, etc. are hardcoded progress messages that appear in every browser console for every WASM user.
- **Recommendation:** Guard behind `#[cfg(debug_assertions)]` or a `wasm-debug` feature; production WASM should not litter the browser console with internal state strings.

## Cross-dimension notes

- PERF: `#[allow(dead_code)]` on the GPU timestamp query fields (lib.rs:545-557) suggests the GPU profiling infrastructure was scaffolded but never wired; the dead buffers still allocate at startup.
- CORRECTNESS: `PlutoObject::render(&self)` at `pluto_objects/button.rs:167-173` has a `// TODO: Render text properly here` comment and falls back to texture-only rendering, silently dropping button label text — a behavioral gap masked by the silent fallback.

## Verdict

NEEDS-WORK — The codebase has several issues that would embarrass an open-source release: a god module with no navigability, pervasive stdout pollution from a library crate (no `log` facade used at all), dead feature flags advertised in README, an orphan source file, and committed test-output artifacts. None are blockers individually, but the sum makes the project hard to contribute to and unsuitable for crates.io as-is.
