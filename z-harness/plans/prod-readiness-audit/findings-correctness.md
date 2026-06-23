# Correctness audit findings

**Target:** plutonium_engine v0.8.0, pure-Rust wgpu 2D graphics/game engine — `/Users/zeke/dev/plutonium_engine/.claude/worktrees/eager-tharp-b3022f`
**Rubric:** generic checklist (no rubric_path supplied)
**Date (UTC):** 2026-06-02T00:00Z

## Summary

- The single most critical issue is the `ptr::read` pattern used to "split borrows" around the per-frame callback in `src/app.rs:782-786`. It produces a **bitwise-duplicate `Box<dyn FnMut>`**: both `callback` and `self.frame_callback` alias the same heap allocation, creating two independent ownership claims that double-drop on panic and allow the callback (which receives `&mut PlutoniumApp`) to trigger mutation of `frame_context` while a `&FrameContext` alias is live — textbook aliasing UB.
- `padded_contains` in `utils.rs` has its padding polarity inverted: it expands the hit region leftward and upward rather than shrinking it, causing `Button` to fire outside its visible area.
- `Box::leak` in the font-load path permanently leaks font data when a subsequent `?` propagation error occurs, and can accumulate across multiple load calls if the same key is loaded to a fresh engine instance.
- Library-visible APIs panic on recoverable conditions (missing file, unknown font key, atlas-build failure) rather than returning `Result`, which is the dominant correctness/robustness concern for downstream crate users.
- Verdict: **NEEDS-WORK** — one soundness bug (UB in unsafe), one active rendering/UX bug (padded_contains), two memory-safety concerns (leak + usize underflow), and an API-level panic surface that is unacceptable for crates.io publication.

## Findings

### [CRITICAL] `ptr::read` of `Box<dyn FnMut>` creates two independent owners, enabling double-drop and aliasing UB

- **Location:** `src/app.rs:781-787`
- **Evidence:**
  ```rust
  let frame_context_ptr = &self.frame_context as *const FrameContext;
  let mut callback = unsafe { std::ptr::read(&self.frame_callback) };
  let frame_context = unsafe { &*frame_context_ptr };
  callback(engine, frame_context, self);            // self passed &mut
  unsafe { std::ptr::write(&mut self.frame_callback, callback); }
  ```
- **Recommendation:** Replace with an `Option<FrameCallback>` field: `take()` the callback, call it, then restore it; or restructure the callback signature to remove the need for `self`.

**Two independent UB vectors:**

1. **Double-drop on panic.** `ptr::read` copies the fat-pointer of the `Box<dyn FnMut>`. After line 782, both `callback` (stack) and `self.frame_callback` (struct field) are full owners of the same heap allocation. If the callback panics before `ptr::write` restores the value, both owners try to drop the same allocation during stack unwinding.

2. **Aliasing `&mut` through `&*frame_context_ptr`.** `frame_context` is a shared reference derived from a raw pointer to `self.frame_context`. The callback simultaneously receives `&mut PlutoniumApp` (`self`), whose type includes `frame_context`. The Rust aliasing rules forbid a live `&T` coexisting with any `&mut` that covers the same memory. If the user's callback calls any `&mut self` method on `PlutoniumApp` that touches `frame_context` (e.g., `start_recording`, `stop_replay`, or even `pressed_keys` mutation paths), the compiler cannot enforce XOR mutability through the raw pointer; miri would flag this immediately.

---

### [HIGH] `padded_contains` inverts the padding sense — hit region expands rather than shrinks

- **Location:** `src/utils.rs:333-338`
- **Evidence:**
  ```rust
  pub fn padded_contains(&self, position: Position, padding: f32) -> bool {
      position.x >= self.x - padding                               // expands left edge outward
          && position.x <= self.x - padding + self.width - (2.0 * padding)
          // right bound = self.x + self.width - 3*padding  (not the inner edge)
  ```
  For `rect = {x:10, w:100}`, `padding=5`: left bound = 5 (expands), right bound = 95 (correct shrink). The left/top edge moves *outward* by `padding` while the right/bottom edge shrinks by `padding` — the region is asymmetrically grown by `padding` on the near side.
- **Recommendation:** Change to `position.x >= self.x + padding && position.x <= self.x + self.width - padding` (and same for y) to uniformly shrink the hit region.

This bug is live: `src/button.rs:76` calls `padded_contains(mouse.mouse_pos, self.padding)` for button click detection, meaning clicks to the left of (and above) the button are registered as hits.

---

### [HIGH] `Box::leak` permanently loses font bytes on any `?` error after the leak

- **Location:** `src/lib.rs:918`, `src/lib.rs:950-957`
- **Evidence:**
  ```rust
  let leaked: &'static [u8] = Box::leak(font_data.into_boxed_slice());
  // ...
  self.load_raster_font_variant_from_data(leaked, ...)?;   // if this Err-propagates,
  // `leaked` is irretrievably gone; no Drop path can recover it
  ```
- **Recommendation:** Wrap the leaked slice in a newtype that implements `Drop` via `unsafe { Box::from_raw(...) }`, or use `Arc<[u8]>` which rusttype now supports, to allow cleanup on failure.

Additionally, the `'static` leak is permanent even on success: if a downstream embedder creates and drops multiple `PlutoniumEngine` instances over the lifetime of a process, each `load_font` call accumulates the font bytes in heap memory forever.

---

### [HIGH] `uv_bind_groups.len() - 1` panics via integer underflow when the slice is empty

- **Location:** `src/texture_atlas.rs:606`
- **Evidence:**
  ```rust
  println!(
      "Warning: Tile index {} out of bounds (max: {}), using default UV bind group",
      tile_index,
      self.uv_bind_groups.len() - 1   // usize subtraction: wraps/panics if len == 0
  );
  ```
  When `uv_bind_groups` is empty, `len() - 1` is `usize::MAX` in release mode (wrapping) or a panic in debug mode (overflow check).
- **Recommendation:** Replace with `self.uv_bind_groups.len().saturating_sub(1)` or guard with `if self.uv_bind_groups.is_empty() { 0 } else { self.uv_bind_groups.len() - 1 }`.

---

### [HIGH] Public API panics on user-recoverable error conditions

- **Location:** `src/texture_atlas.rs:832`, `src/lib.rs:4377`, `src/lib.rs:4532`, `src/lib.rs:4614`, `src/texture_svg.rs:800`, `src/lib.rs:4226`, `src/lib.rs:4251`, `src/lib.rs:4274`, `src/lib.rs:4291`
- **Evidence (representative):**
  ```rust
  // texture_atlas.rs:832 — called inside public create_texture_atlas
  .unwrap_or_else(|_| panic!("file not found: {}", file_path));
  // lib.rs:4614 — inside pub create_text2d
  panic!("Failed to load font");
  // lib.rs:4377 — inside pub create_texture_atlas
  panic!("Failed to create texture atlas")
  // texture_svg.rs:800 — inside pub new() for SVG textures
  fs::read_to_string(path).expect("file should exist")
  ```
- **Recommendation:** Convert each of these to return `Result<T, E>` with a domain error type; the caller is the one who can decide whether a missing file or bad font key is fatal.

These are all conditions that a downstream embedder encounters legitimately (missing asset, wrong font key, invalid SVG path) and for which `panic!` in a library is an API correctness violation per Rust API guidelines (C-FAILURE).

---

### [MED] `[FONT DEBUG]` println! statements fire unconditionally on every raster atlas build

- **Location:** `src/text.rs:999-1097`
- **Evidence:**
  ```rust
  println!("[FONT DEBUG] Atlas size: {}x{}", atlas_width, atlas_height);
  println!("[FONT DEBUG] Scale: {:?}", scale);
  println!("[FONT DEBUG] Max tile size: {}x{}", max_width, max_height);
  // ... 7 more println! calls in render_glyphs_to_atlas_for_chars
  ```
  These fire on every font load (including runtime cache warm requests) and write to stdout with no feature flag or log level guard.
- **Recommendation:** Gate behind `#[cfg(debug_assertions)]` or a `log::debug!` call so they are silent in release builds and don't pollute downstream crate consumers' stdout.

---

### [MED] NDC/transform test coverage too weak to catch regressions after recent math churn

- **Location:** `tests/transform_tests.rs:23-25`
- **Evidence:**
  ```rust
  assert!(tf.transform[0][0].abs() > 0.0);
  assert!(tf.transform[1][1].abs() > 0.0);
  ```
  These assertions verify only that the scale diagonal is non-zero. They will pass regardless of sign errors, double-scaling bugs, or incorrect translation terms — precisely the class of regressions the CHANGELOG says were present in 0.8.0 ("NDC Math & Standardization", "double-scaling bugs").
- **Recommendation:** Add tests that check exact NDC output values: e.g. a `tile_size == viewport_size` case should produce `transform[0][0] == 2.0`, and a tile at position `(0,0)` with a zero-size camera should produce `transform[3][0] == -1.0 + tile_w_ndc * 0.5` (the centering offset per lines 699-703).

---

### [LOW] `Rectangle::pad` static method has asymmetric wrong semantics

- **Location:** `src/utils.rs:366-373`
- **Evidence:**
  ```rust
  pub fn pad(rec: &Rectangle, padding: f32) -> Rectangle {
      Rectangle::new(
          rec.x + padding, rec.y + padding,  // shifts top-left inward
          rec.width + padding,                // adds only once to width (not 2×)
          rec.height + padding,
      )
  }
  ```
  A symmetric outward expansion would be `(rec.x - padding, rec.y - padding, rec.width + 2*padding, rec.height + 2*padding)`. The current implementation shifts the origin but expands the size by only one `padding`, which is neither shrink nor expand — it is a translate + asymmetric grow.
- **Recommendation:** Decide intended semantics (shrink for inner inset, or expand for outer border) and implement symmetrically. Currently unused in `src/`, so impact is zero, but it will surprise the first caller.

---

### [LOW] Replay does not reconstruct `pressed_keys` from the stored key-name strings

- **Location:** `src/app.rs:737`
- **Evidence:**
  ```rust
  self.frame_context.pressed_keys.clear(); // skip key reconstruction
  ```
  The recording path serializes key names as `format!("{:?}", k)` strings, but replay simply clears keys and does not parse them back. Any game logic that reads `pressed_keys` during replay will see no key input even if the original session had keyboard activity.
- **Recommendation:** Either implement key-name parsing to restore `pressed_keys` during replay, or document clearly that the replay feature only reproduces mouse/scroll/text-commit input (not keyboard input), so embedders are not surprised.

## Cross-dimension notes

- CROSS_DIMENSION (perf): `[FONT DEBUG]` `println!` calls at text.rs:999-1097 fire on every raster atlas build, including the hot runtime warm queue path; this is also a performance issue.
- CROSS_DIMENSION (cleanliness): `Box::leak` with no reclamation and `Debug` println! left in production paths are also cleanliness concerns.
- CROSS_DIMENSION (design): The `FrameCallback = Box<dyn FnMut(&mut PlutoniumEngine, &FrameContext, &mut PlutoniumApp)>` signature forces the unsafe borrow-splitting because `PlutoniumApp` owns both `frame_context` and the engine. Redesigning `PlutoniumApp` to separate the engine from the app-lifecycle state would eliminate the need for the unsafe block entirely.

## Verdict

NEEDS-WORK — one CRITICAL soundness bug (double-drop/aliasing UB in unsafe block), one HIGH active behavioral bug (padded_contains wrong polarity), two HIGH memory-safety concerns (leak on error, usize underflow), and a pattern of library panics on user-recoverable conditions that violates crates.io/Rust API guidelines expectations. The engine cannot be considered production-ready for open-source publication until at minimum the CRITICAL and HIGH issues are resolved.
