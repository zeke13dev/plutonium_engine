# Phase 3 — Final decisions (post cross-LLM consult)

Consult: Gemini + Codex (direct CLI; registry unbound). Transcripts in `transcripts/consult-{gemini,codex}.txt`. Each accepted point passed a "one reason it might be wrong" check; the linchpin claim was settled by an empirical compile test.

## Linchpin settled empirically (consultants disagreed)
- Gemini: "FATAL — child modules CANNOT access parent private fields; you MUST make all 50 fields `pub(crate)`."
- Codex: "descendants CAN access ancestor private items, as long as the struct stays defined in an ancestor module."
- **Test (`/tmp/privtest`): a `mod child;` impl block reading `self.secret` of a crate-root struct with a private field — BUILDS CLEAN.** → Codex right, Gemini wrong. **No field-visibility change needed.** Task 1 is NOT "bump 50 fields"; the struct + its fields stay in lib.rs untouched.

## Final decisions (D1/D2/D4/D6 user-approved in Phase 2.5; refined by consult)

- **D1 (mechanism):** impl-block split — move method clusters into `impl<'a> PlutoniumEngine<'a>` blocks in child modules. Confirmed safe (private fields accessible). No owning sub-structs except D4.
- **D2 (verification):** `cargo-public-api`. **Refined:** snapshot must be generated on BOTH host AND `wasm32-unknown-unknown` (consult: target-specific public API — wasm-only methods would be missed by a host-only snapshot). Also a manual attribute-preservation check (`#[inline]`, `#[must_use]`, `#[track_caller]`) and a static auto-trait assertion (engine is `!Send`/`!Sync` via `Rc`; assert the CURRENT state is preserved) because cargo-public-api can false-PASS on auto-trait/attribute changes.
- **D4 (GpuTimer):** extract, but **behavior-preserving only** — preserve exact timestamp-feature gating, buffer map/unmap lifecycle, drop order, encoder ordering, and introduce NO new per-frame allocation. Do NOT also fix the perf-audit P4 `poll(Wait)` stall here (separate task). Verify via snapshots + the existing gpu-metrics output being byte-identical. `GpuTimer` takes `&Device`/`&mut Encoder` as params (never `self`) to avoid split-borrow walls.
- **D5 (re-exports):** **FIRMED to the conservative pole** (consult-driven). **Keep all PUBLIC type definitions in `lib.rs`** (DrawParams, Halo*, TextureFit, FontLoadOptions, RasterTextureLoadError, etc.). Move ONLY private methods + private helper types into child modules. This sidesteps two consult risks at once: (a) cargo-public-api false-FAIL from canonical-path drift (`crate::glow::HaloStyle` vs `crate::HaloStyle`), and (b) docs.rs showing public items under a private module needing `#[doc(inline)]`. Net: public surface text is provably identical.
- **D6 (scope):** **user chose "go further"** — full split incl. render/draw; target lib.rs <~1500; ~9 tasks, one module per task.

## Consult refinements folded into PLAN/TASKS

1. **Macro visibility/order** (both): `flush_batch!`/`flush_atlas_batch!`/`flush_rect_batch!`/`flush_glow_batch!` are `macro_rules!` in lib.rs used by render/draw. Moving macro-using methods breaks textual macro scope. → Move macros into `src/render_macros.rs` (or top of the render module) with `macro_rules!` + `pub(crate) use name;` so child modules can import them. This is an early task, before the render/draw split.
2. **cfg/wasm drift** (both): preserve `#[cfg(target_arch="wasm32")]` / `#[cfg(not(...))]` / feature gates VERBATIM on every moved method; the wasm-only `new`/load paths must keep their gates. Verification runs `cargo check` + cargo-public-api on host AND wasm32.
3. **Extension-trait imports** (Codex): moving methods can drop `use` of extension traits (wgpu/bytemuck/font) and silently change method resolution. → each module re-imports the exact `use` set its methods need; rely on `cargo build` + tests.
4. **Intra-doc links** (both): `[Self::x]` / `[crate::HaloStyle]` in moved doc-comments can break. → run `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` as part of each task's gate.
5. **Task ordering** (both, converged): (1) helper types/structs/free-fns + macros FIRST; (2) leaf subsystem impl clusters (font, popup, glow) NEXT; (3) GpuTimer ISOLATED (behavior refactor); (4) render/draw LAST (widest deps). render/draw early would fail to compile against not-yet-moved internals.
6. **Trait impls** (both): inherent-impl splits are fine (local type). Keep any `impl Trait for PlutoniumEngine` (Drop/Debug) with the core or a clearly-labelled file — do not scatter.

## Rejected / not-applied
- Gemini "make 50 fields pub(crate)": rejected — empirically unnecessary; would needlessly widen internal visibility.
- Any owning-sub-struct beyond GpuTimer: out of scope per D1.
