# PLAN — decompose-lib

## Goals
- Make `src/lib.rs` navigable for contributors by splitting it from 5531 lines into ~8 coherent topic modules, lib.rs ending < ~1500 lines.
- Resolve audit findings CL1 + D4 (god module) with provably zero public-API change and zero behavior change.

## Decisions (with rationale)
- **D1 impl-block split, not owning sub-structs** — child modules can access the crate-root struct's private fields (verified by compile test), so methods relocate with no borrow restructuring and no visibility change. Owning sub-structs would fight the borrow checker for near-zero added value here.
- **D2 cargo-public-api gate on host + wasm32** — the only mechanical guarantee of "zero API change"; wasm32 included because wasm-only methods would be invisible to a host-only snapshot. Backed by manual attribute checks + a static auto-trait assertion (tool false-passes on those).
- **D4 GpuTimer extracted, behavior-preserving** — the timestamp_* fields are the one genuinely cohesive cluster; extraction drops 7 fields off the engine. Strictly no behavior change (does NOT fix the P4 poll-stall — that's a separate perf task).
- **D5 public type definitions stay in lib.rs** — moving only private items sidesteps re-export canonical-path false-fails and docs.rs private-module issues. Public surface text is provably identical.
- **D6 full split incl. render/draw** (user-chosen) — render/draw extracted LAST because it has the widest dependency on other internals.

## Non-goals
- No public-API redesign (that's audit tasks T004/T007/T009/T010 — separate).
- No perf fixes (P1-P15 are separate tasks); behavior stays byte-identical, including the existing GPU-timer `poll(Wait)`.
- No owning sub-structs beyond GpuTimer.
- No hard line-count target chase — stop when each module is coherent.

## Approved shortcuts
- None. (Consult introduced no shortcuts; all refinements harden the robust path.)

## Ordered phases
1. **Verification infra** (T001) — cargo-public-api baseline + auto-trait guard + CI. Must be first so every later task asserts an empty diff.
2. **Macros** (T002) — relocate flush macros with `pub(crate) use` before any macro-using method moves.
3. **Leaf subsystems** (T003 font_raster, T004 font_msdf, T005 popup_render, T006 glow) — independent method clusters; each asserts empty API diff + green tests/snapshots.
4. **GpuTimer** (T007) — isolated behavior refactor; extra scrutiny on render hot path + gpu-metrics output.
5. **Core render/draw** (T008) — last; depends on macros + leaf modules being out.
6. **Cleanup + final gate** (T009) — relocate remaining private helpers, trim lib.rs, run the full gate (build host+wasm, test, snapshots, doc -D warnings, empty API diff).

## DRY / KISS / SOLID
- **DRY:** the flush macros are centralized once (T002) instead of being duplicated when render/draw move; shared batch helpers live in one render module.
- **KISS:** the conservative impl-block split is the simplest mechanism that satisfies the constraint; no abstraction is introduced (owning sub-structs deliberately avoided).
- **SOLID:** each module gains a single responsibility (font raster, msdf, popup, glow, timing, render, draw); the engine's public interface is untouched (interface segregation preserved by D5). GpuTimer is the one new single-responsibility type, with a minimal parameter-passing interface (no `self` coupling).
