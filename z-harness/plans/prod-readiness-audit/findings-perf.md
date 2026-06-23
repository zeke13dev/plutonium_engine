# Perf audit findings

**Target:** plutonium_engine v0.8.0 — pure-Rust wgpu 2D graphics/game engine — `/Users/zeke/dev/plutonium_engine/.claude/worktrees/eager-tharp-b3022f`
**Rubric:** generic checklist (no rubric supplied)
**Date (UTC):** 2026-06-02T00:00Z

## Summary

- CRITICAL: `flush_batch` (sprite path) and `flush_atlas_batch!` (glyph path) both call `device.create_buffer_init` and `device.create_bind_group` every time they flush — every draw call every frame — generating unbounded GPU resource allocations on the hot render path.
- CRITICAL: `update_transform_uniform` on every `TextureSVG`/`TextureAtlas` calls `device.create_buffer_init` (via `update_vertex_buffer`) every frame for every loaded texture/atlas, even when nothing moved.
- HIGH: GPU timestamp readback uses `device.poll(wgpu::Maintain::Wait)` synchronously after `frame.present()` every frame, stalling the CPU until the GPU drains.
- HIGH: `calculate_text_layout` allocates `Vec<char>` multiple times per line per call (width-measurement pass + render pass), and this is called every frame for every text draw with no result cache at the call site.
- MED: `render_queue.sort_by` (stable sort) is called every frame on the full render queue even when z-ordering is unchanged.
- Overall verdict: **NEEDS-WORK** — two CRITICAL paths create GPU objects per frame; the engine would degrade sharply above a few dozen visible sprites or atlas glyphs.

## Findings

### [CRITICAL] Per-flush GPU buffer and bind group creation in sprite batch (hot render path)

- **Location:** `src/lib.rs:2946-2961` (`flush_batch` closure, inside `render()`)
- **Evidence:**
  ```rust
  let instance_buffer =
      self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
          label: Some("instance data (sprite)"),
          ...
      });
  let instance_bg =
      self.device.create_bind_group(&wgpu::BindGroupDescriptor { ... });
  ```
- **Recommendation:** Pool or reuse instance storage buffers between frames the same way `flush_rect_batch!` does with `rect_instance_pool`; do not call `create_buffer_init`/`create_bind_group` inside the render loop.

### [CRITICAL] Per-flush GPU buffer and bind group creation in atlas/glyph batch (hot render path)

- **Location:** `src/lib.rs:3187-3226` (`flush_atlas_batch!` macro, inside `render()`)
- **Evidence:**
  ```rust
  let instance_buffer = self.device.create_buffer_init(
      &wgpu::util::BufferInitDescriptor {
          label: Some("instance data (atlas)"),
          ...
      },
  );
  let instance_bg = self.device.create_bind_group(...);
  let id_buf = self.device.create_buffer_init(...);
  let id_bg  = self.device.create_bind_group(...);
  ```
- **Recommendation:** Apply the same pooled-buffer strategy used by `flush_rect_batch!`; for the identity UBO, create it once at startup and reuse the cached `rect_identity_bg` pattern already present in the rect path.

### [CRITICAL] Per-frame vertex buffer recreation for every loaded texture and atlas (update hot path)

- **Location:** `src/texture_svg.rs:638-646` (`update_vertex_buffer`), called from `update_transform_uniform` at line 667; and `src/texture_atlas.rs:515-523` (same pattern); invoked unconditionally for every entry in `texture_map` and `atlas_map` at `src/lib.rs:2199-2214`
- **Evidence:**
  ```rust
  pub fn update_vertex_buffer(&mut self, device: &wgpu::Device) {
      let new_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
          ...
          usage: wgpu::BufferUsages::VERTEX,
      });
      self.vertex_buffer = new_vertex_buffer;
  }
  ```
  `adjust_vertex_texture_coordinates` always writes the same constant four vertices (`[0,0], [1,0], [0,-1], [1,-1]`) regardless of arguments, making this allocation unconditionally wasteful.
- **Recommendation:** Create the vertex buffer once at construction time; since the coordinates are constants, never recreate it. The transform is already updated separately via `queue.write_buffer` on the uniform buffer, which is correct.

### [HIGH] Synchronous GPU stall per frame for timestamp readback

- **Location:** `src/lib.rs:3460-3498` (inside `render()`, after `frame.present()`)
- **Evidence:**
  ```rust
  // Block until mapping completes
  self.device.poll(wgpu::Maintain::Wait);
  ```
  A fresh command encoder is submitted, then `poll(Wait)` blocks the CPU until the GPU completes it — every frame when timestamp queries are active.
- **Recommendation:** Use double-buffered staging (read frame N-2 while frame N is in flight) or move readback to a callback; never call `poll(Wait)` on the render thread between present and the next frame begin.

### [HIGH] Per-frame `Vec<char>` allocations inside `calculate_text_layout` and `measure_text` (hot text path)

- **Location:** `src/text.rs:1430`, `1451`, `1501`, `1591`, `1639`, `1712`, `1757`, `1868`, `1914`, `1953` (multiple `let chars: Vec<char> = line.chars().collect()` inside `calculate_text_layout`); also `src/text.rs:843`, `909` inside `measure_text`
- **Evidence:**
  ```rust
  // In calculate_text_layout — width-measurement pass:
  let chars: Vec<char> = line.chars().collect();
  // then again in the render pass for the same line:
  let chars: Vec<char> = line.chars().collect();
  ```
  For a text element with N lines, the raster path allocates 2 × N `Vec<char>` per `queue_text_with_spacing` call, plus another allocation inside `measure_text` if called separately. This is done every frame for every dynamic text object.
- **Recommendation:** Iterate over `line.chars()` directly without collecting, or hoist a single `Vec<char>` per line across both passes; for static text, cache the glyph layout result and only re-run layout when the content/size/DPI changes.

### [HIGH] `resolve_font_key_for_render` allocates a `String` on every text draw call (hot text path)

- **Location:** `src/lib.rs:1201-1229` (`resolve_font_key_for_render`), called from `queue_text_with_spacing` at line 2764
- **Evidence:**
  ```rust
  // Called once per queue_text_with_spacing invocation:
  let Some(family) = self.raster_font_families.get(font_key) else {
      return (font_key.to_string(), font_size_override);  // alloc
  };
  ...
  let atlas_key = entry.atlas_key.clone();  // alloc
  ```
  Every text render call returns an owned `String` even for the steady-state exact-match case.
- **Recommendation:** Return `Cow<str>` or `&str` for the exact-match path; the atlas key is already stored in the entry and can be borrowed for the duration of `queue_text_with_spacing`.

### [MED] Full render queue stable-sort every frame regardless of z-order changes

- **Location:** `src/lib.rs:2882`
- **Evidence:**
  ```rust
  self.render_queue.sort_by(|a, b| a.z.cmp(&b.z));
  ```
  The render queue is rebuilt from scratch each frame (it is cleared in `begin_frame`), so the items are almost always already in insertion order within each z-layer. The stable sort runs in O(N log N) even when nothing changed.
- **Recommendation:** Use `sort_unstable_by` (items within the same z are equivalent for ordering purposes so stability is unneeded) or insert items into a pre-bucketed structure keyed by z-layer to eliminate the sort entirely.

### [MED] `[FONT DEBUG]` prints inside atlas construction function active in release builds

- **Location:** `src/text.rs:999-1100` (`render_glyphs_to_atlas_for_chars`)
- **Evidence:**
  ```rust
  println!("[FONT DEBUG] Atlas size: {}x{}", atlas_width, atlas_height);
  ...
  println!("[FONT DEBUG] Non-zero bytes in texture: {}/{}", non_zero_pixels, texture_data.len());
  ```
  The last print at line 1096 calls `texture_data.iter().filter(|&&b| b != 0).count()` — an O(atlas_width × atlas_height × 4) scan over the entire atlas pixel buffer — present in every build configuration.
- **Recommendation:** Gate all `[FONT DEBUG]` prints (lines 999–1100) behind `#[cfg(debug_assertions)]` or a feature flag; remove the O(N) pixel scan from non-debug builds entirely.

### [MED] `update` unconditionally calls `measure_text` every frame for non-auto-size, non-wrap text objects

- **Location:** `src/pluto_objects/text2d.rs:740-750`
- **Evidence:**
  ```rust
  if !self.auto_size_enabled && !self.wrap_enabled {
      let (text_width, line_count) = text_renderer.measure_text(...);
      self.dimensions.width = text_width;
      self.dimensions.height = ...;
  }
  ```
  This branch is reached every frame when `needs_recalc` returns `false`, meaning even "no change" frames pay full text measurement cost.
- **Recommendation:** Track a `dimensions_valid` flag and skip `measure_text` when the content, DPI, and font cache version have not changed since the last successful measurement.

### [MED] Per-frame debug prints in font cache warm path (`eprintln!` on error, active in all builds)

- **Location:** `src/lib.rs:1289-1293` (inside `process_runtime_raster_warm_queue`, called every frame from `begin_frame`)
- **Evidence:**
  ```rust
  eprintln!(
      "[FONT CACHE] failed to warm '{}' @ {:.2}px: {:?}",
      req.family_key, logical_size, err
  );
  ```
  While this fires only on rasterization errors, the `process_runtime_raster_warm_queue` function is called unconditionally every `begin_frame`, and the `HashMap::new` for `remaining_budget` at line 1236 is allocated every time the queue is non-empty (including during the transition frames when a new size is being warmed).
- **Recommendation:** Replace the `HashMap::new` budget tracker with a fixed-size array or stack-allocated structure; the error print is acceptable but should be rate-limited or converted to `log::warn!`.

### [MED] `flush_rect_batch!` identity UBO/bind-group rebuilt on first rect per frame

- **Location:** `src/lib.rs:3077-3102` (inside `flush_rect_batch!` macro)
- **Evidence:**
  ```rust
  if self.rect_identity_bg.is_none() {
      let id_buf = self.device.create_buffer_init(...);
      let id_bg  = self.device.create_bind_group(...);
      self.rect_identity_bg = Some(id_bg);
  }
  ```
  `rect_identity_bg` is cleared to `None` in every `begin_frame` (line 3508), so the identity UBO and bind group are recreated from scratch on the first rect/glow flush every frame.
- **Recommendation:** Keep the identity bind group alive across frames; only recreate it on window resize. Remove the `self.rect_identity_bg = None` line from `begin_frame`.

### [LOW] `flush_glow_batch!` always creates a new GPU buffer and bind group (no pooling)

- **Location:** `src/lib.rs:3125-3139` (`flush_glow_batch!` macro)
- **Evidence:**
  ```rust
  let glow_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("glow-instance-buffer"),
      ...
      usage: wgpu::BufferUsages::STORAGE,
  });
  let glow_bg = self.device.create_bind_group(...);
  ```
  Unlike rect instances, glow instances have no pool. Any frame with a glow effect pays one `create_buffer_init` + `create_bind_group`.
- **Recommendation:** Add glow instances to the `rect_instance_pool` pattern or a dedicated glow pool.

### [LOW] SVG rasterization is done at load time only — no per-frame cost confirmed

- **Location:** `src/texture_svg.rs:790-870` (`svg_to_texture`)
- **Evidence:** `TextureSVG::new` / `new_from_data` are creation-time calls; no call site in the hot `render` or `update` loops re-invokes them unconditionally.
- **Recommendation:** No action needed; SVG-to-texture is correctly a one-time operation. Confirm `update_text` (lines 48-148) is only called on user-driven changes and not every frame.

## Cross-dimension notes

- CLEANLINESS: `adjust_vertex_texture_coordinates` in both `TextureSVG` and `TextureAtlas` ignores all its parameters and always writes identical constant vertices; the function signature is misleading and the `device` parameter is passed through `update_transform_uniform` solely because `update_vertex_buffer` needs it, even though the buffer contents never change.
- CORRECTNESS: The stable sort at `lib.rs:2882` preserves submission order within a z-level, which is a documented invariant; switching to `sort_unstable_by` would require confirming no callers depend on intra-layer draw order being preserved.
- DESIGN: The 5531-line `lib.rs` god-module makes it difficult to profile individual subsystems; splitting the render loop into a dedicated `render.rs` would help adopters understand the hot path.

## Verdict

NEEDS-WORK — Two CRITICAL paths (`flush_batch` and `flush_atlas_batch!`) allocate new GPU buffers and bind groups every render call per frame, and a third CRITICAL path recreates vertex buffers for every loaded texture/atlas every frame unconditionally. The synchronous GPU stall for timestamp readback also hard-caps frame throughput. These issues would make the engine feel sluggish to adopters with more than a handful of sprites or text elements, and block any claims of instancing/batching performance in the README.
