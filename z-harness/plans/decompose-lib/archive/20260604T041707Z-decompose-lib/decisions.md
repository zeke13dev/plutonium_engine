# Decisions — decompose-lib

## D1 — Decomposition mechanism: impl-block split vs owning sub-structs
- **Options:**
  - (a) **Impl-block split** — move method clusters into `impl<'a> PlutoniumEngine<'a>` blocks in new child modules; struct + fields stay in lib.rs. Zero borrow change, zero API change. Achieves navigability.
  - (b) **Owning sub-structs** — extract `FontCache`/`TextureRegistry`/`PopupController`/`ClipStack` that own their fields; engine delegates. Better SOLID, but borrow-checker fights when a method needs `device` + a cache at once; risk of behavior change.
- **Tentative call:** (a) as the default for this pass; (b) only where state is genuinely cohesive (see D4 — GPU timer is the sole natural case). Keeps the "zero API change + green tests" constraint cheap to guarantee.
- **Consult? yes** — Trigger: changes module boundaries; affects >1 module; design-level.

## D2 — Public-API verification method
- **Options:**
  - (a) **Install `cargo-public-api`** — de-facto standard; exact public-surface diff; add as a CI gate. Cost: a dev tool + nightly for full fidelity (works on stable with reduced detail).
  - (b) **rustdoc JSON diff** — `cargo +nightly rustdoc -- -Z unstable-options --output-format json`, diff the `paths` set before/after. No extra crate, needs nightly.
  - (c) **cargo doc symbol grep** — crude diff of generated HTML/`pub` items. No deps, lowest fidelity.
- **Tentative call:** (a) install `cargo-public-api`, capture a baseline snapshot BEFORE any move, and assert an empty diff after each task; wire it into CI (pairs with audit task RE2/T017). Fall back to (b) if avoiding the dev-dep.
- **Consult? yes** — Trigger: defines the correctness gate for the whole refactor; CI surface.

## D3 — Helper-struct & free-fn relocation + internal visibility
- **Options:** move each private helper struct/free fn to its subsystem module and bump to `pub(crate)` so lib.rs can still name it; OR keep them in lib.rs and only move methods.
- **Tentative call:** move helpers WITH their subsystem (RasterFont* → font_raster; TransformPool/RectInstanceBuffer/RectStyleKey → a `render_internal`/batch module or kept in lib core; SlotState → core; Halo* → glow). Bump moved-but-cross-referenced types to `pub(crate)`. This is internal-only visibility (private → pub(crate)), NOT a public-API change.
- **Consult? no** — mechanical; internal visibility only; covered by the D2 gate.

## D4 — GPU-timer extraction shape
- **Options:**
  - (a) Extract a `GpuTimer` struct owning `timestamp_query/buf/staging/period_ns/count/frame_index` + `gpu_metrics`, with `begin(encoder)`/`resolve_and_record()`/`maybe_report()`; `render()` calls it. Cohesive; the fields are touched ONLY by the timer path.
  - (b) Leave timer state as engine fields; move only the readback logic to free functions in `gpu_timer.rs`.
- **Tentative call:** (a) — the timestamp fields are genuinely self-contained, so this is the one place an owning sub-struct is clean and reduces the engine field count by 7. Guard: it sits in the render hot path, so verify snapshots + the existing GPU-metrics behavior are byte-identical.
- **Consult? yes** — Trigger: touches the render hot path + field ownership/borrows.

## D5 — Re-export strategy to preserve public paths
- **Options:** `pub use <module>::Type;` from crate root for every public type that moves (e.g. Halo* → glow); OR don't move public types at all (keep them in lib.rs, move only private methods).
- **Tentative call:** keep PUBLIC type definitions in lib.rs (or a `types.rs` re-exported via `pub use`) to minimize re-export churn; move only private methods + private helpers into subsystem modules. Halo* public enums: keep their definitions re-exported so `plutonium_engine::HaloStyle` is unchanged. The D2 gate proves no path moved.
- **Consult? no** — mechanical; fully verified by D2.

## D6 — Scope of this pass
- **Options:**
  - (a) **4 named modules only** (font_raster, gpu_timer, popup_render, glow). lib.rs drops from 5531 to ~3500 (font is 65 methods — the big chunk lands in font_raster). 
  - (b) **4 modules + split the font cluster** (font_raster + font_msdf) and pull the core draw/render path into `render.rs`. Targets lib.rs < ~2000.
  - (c) Everything in one mega-pass.
- **Tentative call:** (b) — do the 4 named modules AND split font into raster vs msdf (65 methods is too big for one file), but DO NOT chase a hard line target. Stop when each module is coherent. Leave a further `render.rs`/`draw.rs` split as a documented follow-up if lib.rs is still large. One task per module so failure isolation is clean and the D2 gate runs per task.
- **Consult? yes** — Trigger: scope/sequencing; determines task count.

---
**Consult-flagged:** D1, D2, D4, D6 (4 — within the cap of 5). D3, D5 are obvious/mechanical.

**Sequencing note (from audit):** ideally lands after error-model (T004/T004B) and visibility (T007) tasks; planned standalone here. If those land first, D3/D5 visibility churn shrinks.
