# Design audit findings

**Target:** plutonium_engine v0.8.0 — pure-Rust wgpu 2D graphics/game engine for crates.io publication — `/Users/zeke/dev/plutonium_engine/.claude/worktrees/eager-tharp-b3022f`
**Rubric:** generic checklist (no rubric file supplied); framed as production/open-source-readiness
**Date (UTC):** 2026-06-02T00:00Z

---

## Summary

- The public API is completely undocumented at the crate level (`src/lib.rs` has zero `//!` doc lines) and approximately 80% of the ~95 public methods on `PlutoniumEngine` carry no `///` doc comment; `FontError` implements neither `Display` nor `std::error::Error`, making it unusable with `?` in downstream code.
- The "god object" problem is real: 5 531 lines in `src/lib.rs`, a single `PlutoniumEngine` struct mixing GPU wiring, font caching, object registries, popup state, clip stacks, slot management, raster warm queues, and all draw calls — there is no internal decomposition and no documented separation of concern.
- Error handling is ad hoc and partially broken: resource-creation functions (`create_texture_svg`, `create_texture_atlas`, `create_texture_raster_from_path`) `panic!` or `.expect()` instead of returning `Result`, while font-loading functions do return `Result<(), FontError>` — but `FontError` is not a well-formed error type (no `Display`/`Error` impls).
- Downstream-coupling risk is severe: `wgpu::SurfaceError`, `winit::keyboard::Key`, and `winit::dpi::PhysicalSize<u32>` appear directly in public function signatures, meaning any wgpu or winit major bump immediately breaks downstream crates.
- The feature-gate design has structural flaws: `backend-wgpu = []` is documented as "always enabled" but is an empty, un-enforced feature flag; `backend-wgpu` types appear outside the feature gate; internal GPU helper types (`Vertex`, `DrawingContext`, `UVTransform`, `RectInstanceRaw`, etc.) are `pub` in `src/utils.rs` and leak into docs via `use utils::*`.

---

## Findings

### [CRITICAL] `FontError` does not implement `Display` or `std::error::Error`

- **Location:** `src/text.rs:69-79`
- **Evidence:**
  ```rust
  #[derive(Debug)]
  pub enum FontError { IoError(std::io::Error), InvalidFontData, ... }
  // no impl Display, no impl std::error::Error
  ```
- **Recommendation:** Add `impl std::fmt::Display for FontError` and `impl std::error::Error for FontError { source }` so downstream code can use `?`-propagation and the type satisfies the idiomatic Rust error contract.

---

### [CRITICAL] Resource-creation panic instead of returning `Result`

- **Location:** `src/lib.rs:4226`, `src/lib.rs:4251`, `src/lib.rs:4274`, `src/lib.rs:4291`, `src/lib.rs:4377`, `src/lib.rs:4532`, `src/lib.rs:4614`
- **Evidence (representative set):**
  ```rust
  // line 4226
  let texture = svg_texture.expect("texture should always be created properly");
  // line 4377
  } else { panic!("Failed to create texture atlas") }
  // line 4614 (create_text2d)
  panic!("Failed to load font");
  ```
- **Recommendation:** Change `create_texture_svg`, `create_texture_atlas`, `create_texture_atlas_2d`, `create_text2d`, and `create_texture_raster_from_path` to return `Result<…, SomePublicError>` so errors can be handled without crashing the process; a library crate must never `panic!` on foreseeable IO/font failures.

---

### [CRITICAL] Zero crate-level documentation; `documentation` URL points to near-empty docs

- **Location:** `src/lib.rs:1-70` (entire file has no `//!` module doc); `Cargo.toml:9`
- **Evidence:**
  ```toml
  documentation = "https://docs.rs/plutonium_engine"
  ```
  `src/lib.rs` opens directly at module declarations with no `//!` lines. Of ~95 `pub fn` on `PlutoniumEngine` only a small minority carry `///` comments (the bulk of the draw/queue/create API is undocumented).
- **Recommendation:** Add `//!` crate-level docs to `src/lib.rs` (overview, quick-start snippet, feature flags list), add `#![warn(missing_docs)]`, and document at minimum every `pub fn` on `PlutoniumEngine`; otherwise the `documentation` URL leads to a near-empty page which actively misleads downstream users.

---

### [HIGH] `wgpu` and `winit` types exposed directly in public function signatures

- **Location:** `src/lib.rs:2832` (`-> Result<(), wgpu::SurfaceError>`), `src/lib.rs:3522` (`end_frame -> Result<(), wgpu::SurfaceError>`), `src/lib.rs:2121` (param `&PhysicalSize<u32>`), `src/lib.rs:2136` (param `key: &Option<winit::keyboard::Key>`), `src/lib.rs:500` (`pub size: PhysicalSize<u32>`)
- **Evidence:**
  ```rust
  pub fn end_frame(&mut self) -> Result<(), wgpu::SurfaceError>
  pub fn resize(&mut self, new_size: &PhysicalSize<u32>)
  pub fn update(&mut self, mouse_info: Option<MouseInfo>, key: &Option<Key>, delta_time: f32)
  pub size: PhysicalSize<u32>,   // public struct field
  ```
- **Recommendation:** Wrap these in a crate-owned `EngineError` enum and crate-owned geometry types (or newtype the winit/wgpu types); every wgpu or winit major-version bump otherwise breaks every downstream consumer at the type-signature level.

---

### [HIGH] `PlutoniumEngine<'a>` is a 5531-line god object with no internal decomposition

- **Location:** `src/lib.rs:499-5531`
- **Evidence:** The struct holds simultaneously: `wgpu::Device/Queue/Surface`, pipeline handles, texture/atlas maps, font families, raster warm queues, popup state, slot states, clip stacks, GPU timer state, rect batching metrics, and a retained-object registry — all mixed in one struct with 95 `pub fn` methods ranging from low-level (`resize`, `render`) to high-level (`show_popup`, `draw_halo`).
- **Recommendation:** Extract coherent subsystems into sub-structs or sub-modules (e.g., `FontCache`, `TextureRegistry`, `PopupController`, `ClipStack`) that `PlutoniumEngine` owns and delegates to; this does not have to change the public API surface but makes the codebase navigable and testable.

---

### [HIGH] `backend-wgpu` feature is an empty, unenforceable seam

- **Location:** `Cargo.toml:57`; `src/lib.rs` everywhere
- **Evidence:**
  ```toml
  backend-wgpu = []   # always enabled by default; described as "always enabled"
  ```
  No code is gated behind `#[cfg(feature = "backend-wgpu")]` in `src/lib.rs`; the feature exists purely as a naming stub. If a downstream user removes it, they get the same binary.
- **Recommendation:** Either remove the feature and simplify to a direct wgpu dependency, or gate the wgpu-specific code behind it so the flag is meaningful; a vestigial feature that the README says is "always enabled" misleads contributors about architecture.

---

### [HIGH] Inconsistent return-type contract: `(Uuid, Rectangle)` vs typed wrapper objects

- **Location:** `src/lib.rs:4213` (`create_texture_svg` returns `(Uuid, Rectangle)`), `src/lib.rs:4576` (`create_texture_2d` returns `Texture2D`), `src/lib.rs:4599` (`create_text2d` returns `Text2D`), `src/lib.rs:4676` (`create_button` returns `Button`)
- **Evidence:**
  ```rust
  pub fn create_texture_svg(..) -> (Uuid, Rectangle)       // raw handle
  pub fn create_texture_2d(..) -> Texture2D                 // wrapper object
  pub fn create_text2d(..)      -> Text2D                   // wrapper object
  pub fn create_button(..)      -> Button                   // wrapper object
  ```
  Both approaches are present; lower-level SVG/atlas creators return raw `(Uuid, Rectangle)` pairs while higher-level "2D" creators return wrapper objects. The two systems are silently parallel and a new user has no guidance on which to use.
- **Recommendation:** Converge on one creation idiom (the typed-wrapper `Texture2D` style is idiomatic) and deprecate the raw-tuple form, or at minimum document the design intent clearly so users understand when to use each.

---

### [HIGH] `Texture2DInternal`, `ShapeInternal`, `Text2DInternal`, `ButtonInternal`, `TextureAtlas2DInternal` are `pub` structs

- **Location:** `src/pluto_objects/texture_2d.rs:8`, `src/pluto_objects/shapes.rs:16`, `src/pluto_objects/text2d.rs` (similar), `src/pluto_objects/button.rs` (similar)
- **Evidence:**
  ```rust
  pub struct Texture2DInternal { ... }   // implementation detail of Texture2D
  pub struct ShapeInternal { ... }       // implementation detail of Shape
  ```
- **Recommendation:** Change all `*Internal` structs to `pub(crate)` (or move them into a private submodule); they are leaked into docs because the parent modules are `pub mod`, signalling an unstable implementation as part of the public contract.

---

### [HIGH] GPU-internals exported from `pub mod utils` via glob import

- **Location:** `src/utils.rs:31-131` (`DrawingContext`, `UVTransform`, `Vertex`, `TransformUniform`, `InstanceRaw`, `RectInstanceRaw`, `GlowInstanceRaw`); `src/lib.rs:60` (`use utils::*`)
- **Evidence:**
  ```rust
  // src/utils.rs
  pub struct DrawingContext<'a> { pub rpass: &'a mut wgpu::RenderPass<'a>, ... }
  pub struct Vertex { pub position: [f32; 2], pub tex_coords: [f32; 2] }
  pub struct RectInstanceRaw { pub model: [[f32; 4]; 4], ... }
  // src/lib.rs
  use utils::*;
  ```
  These are GPU-pipeline internals with no semantic value to downstream consumers; they appear in docs.rs output and consume namespace.
- **Recommendation:** Change all GPU-wire structs in `utils.rs` to `pub(crate)` and add `#[doc(hidden)]` to any that must remain `pub`; remove the `use utils::*` glob and use explicit imports.

---

### [MED] No MSRV declared; no `rust-version` in Cargo.toml

- **Location:** `Cargo.toml:1-83` (no `rust-version` field)
- **Evidence:** The crate uses `wgpu 23`, `winit 0.30`, FreeType bindings, WASM futures, and recent Rust 2021-edition features; there is no `rust-version` key nor any README statement about minimum supported Rust.
- **Recommendation:** Add `rust-version = "1.77"` (or determine the actual MSRV via `cargo-msrv`) to `Cargo.toml`; this is required for CI reliability and signals stability to downstream users evaluating dependency fitness.

---

### [MED] `warm_text_cache` returns `Result<WarmStats, String>` (stringly-typed error)

- **Location:** `src/lib.rs:994-998`
- **Evidence:**
  ```rust
  pub fn warm_text_cache(..) -> Result<WarmStats, String>
  ```
- **Recommendation:** Return `Result<WarmStats, FontError>` to remain consistent with the rest of the font API and allow error matching without string parsing.

---

### [MED] `PlutoniumEngine::new` panics on WASM with a misleading error message

- **Location:** `src/lib.rs:4912-4920`
- **Evidence:**
  ```rust
  #[cfg(target_arch = "wasm32")]
  pub fn new(...) -> Self {
      panic!("PlutoniumEngine::new is not available on wasm32. Use PlutoniumEngine::new_async.");
  }
  ```
  The WASM `new` compiles but panics at runtime; downstream crates compiled for `wasm32` will get a runtime crash rather than a compile-time error.
- **Recommendation:** Make the WASM `new` a compile-error via `compile_error!()` or remove it from the WASM target entirely; the `new_async` path returning `Result<Self, String>` is the correct API for WASM and should be the only one visible on that target.

---

### [MED] Split WASM / native API is incoherent at the module boundary

- **Location:** `src/app.rs:862` (`run_app`), `src/app.rs:928` (`run_app_wasm`), `src/lib.rs:4883` (`new`), `src/lib.rs:4922` (`new_async`)
- **Evidence:** On native: `run_app` + synchronous `PlutoniumEngine::new`; on WASM: `run_app_wasm` (async) + `PlutoniumEngine::new_async`. These are the same conceptual operation with entirely different calling conventions. The `FrameContext::pressed_keys: Vec<winit::keyboard::Key>` field leaks a winit type into the frame callback, preventing portability.
- **Recommendation:** Define a platform-agnostic `run_app` entry that branches internally; expose a `Keys` newtype wrapping winit key values so downstream code does not import winit directly.

---

### [MED] `CharacterInfo` and `MsdfGlyphInfo` are public but serve as internal atlas-packing types

- **Location:** `src/text.rs:17-31`
- **Evidence:**
  ```rust
  pub struct CharacterInfo { pub tile_index: usize, pub advance_width: f32, ... }
  pub struct MsdfGlyphInfo { pub advance_width: f32, pub plane_bounds: Bounds, ... }
  ```
  These appear in the signature of `pub fn create_font_texture_atlas(…, char_positions: &HashMap<char, CharacterInfo>)` — a method that is itself a low-level GPU call not needed by end users.
- **Recommendation:** Make `CharacterInfo` and `MsdfGlyphInfo` `pub(crate)` and make `create_font_texture_atlas` `pub(crate)` as well; end users have no reason to build font atlases manually.

---

### [MED] `FontError::IoError` wraps `std::io::Error` which is not `Clone`, blocking `#[derive(Clone)]`

- **Location:** `src/text.rs:70-79`
- **Evidence:**
  ```rust
  pub enum FontError { IoError(std::io::Error), ... }
  // std::io::Error: !Clone — prevents FontError from ever being Clone
  ```
- **Recommendation:** Store `std::io::ErrorKind` plus a `String` message in the `IoError` variant so `FontError` can derive `Clone` when needed downstream.

---

### [LOW] `FrameInputRecordLocal` and record/replay infrastructure are public with no stability signal

- **Location:** `src/app.rs:27-39`
- **Evidence:**
  ```rust
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
  pub struct FrameInputRecordLocal { pub pressed_keys: Vec<String>, ... }
  ```
  This struct and `start_recording`/`stop_recording`/`start_replay`/`stop_replay` are part of the public API despite being developer-tooling plumbing, not user-facing features.
- **Recommendation:** Mark replay/record types `#[doc(hidden)]` or gate them behind the `replay` feature; they carry serde derives that add binary size for users who do not need them.

---

### [LOW] `pub fn resize` and `pub fn update` are on `PlutoniumEngine` rather than internal to the app loop

- **Location:** `src/lib.rs:2121` (`resize`), `src/lib.rs:2136` (`update`)
- **Evidence:**
  ```rust
  pub fn resize(&mut self, new_size: &PhysicalSize<u32>)
  pub fn update(&mut self, mouse_info: Option<MouseInfo>, key: &Option<Key>, delta_time: f32)
  ```
  Both accept raw winit/internal types. The winit event loop in `src/app.rs` calls these internally; they have no reason to be `pub` except for users writing custom event loops (which is not a documented use case).
- **Recommendation:** Make `resize` and `update` `pub(crate)` and document any official "custom event loop" surface separately; exposing them as `pub` invites misuse and locks in the winit `PhysicalSize` / `Key` types as permanent public API.

---

## Cross-dimension notes

- CORRECTNESS: `Box::leak` at `src/lib.rs:918` ("SAFETY: cached for the engine lifetime") leaks a heap allocation per font load with no reclamation path; this is a memory-growth correctness issue in long-running processes that reload fonts.
- PERF: `create_text2d` calls `measure_text` unconditionally at creation time (`src/lib.rs:4618`), which may trigger atlas lookups before the first render frame.

---

## Verdict

NEEDS-WORK — The architecture is coherent enough for a pre-release game prototype (the immediate-mode `begin_frame`/`draw_*`/`end_frame` loop is a reasonable public surface), but multiple blockers prevent a credible public library release: `FontError` is not a well-formed Rust error type, several resource-creation paths panic instead of returning `Result`, the entire API is effectively undocumented, wgpu and winit types leak directly into public signatures, and internal implementation types (`*Internal`, GPU structs) are unnecessarily `pub`. None of these require rethinking the overall design — they are correctable API-hygiene issues — but all must be resolved before a 0.x crates.io publication is defensible.
