# Phase 0 — Premise check

**Premise accepted, with one scoping nuance surfaced.**

Goal as I read it: split the 5531-line `src/lib.rs` (one giant `impl<'a> PlutoniumEngine<'a>` + ~30 small type defs) into navigable topic modules, with ZERO public-API change and all tests/snapshots green. This is a sound, well-bounded refactor — the audit (CL1/D4) flagged the god module as the top cleanliness/design item, and the "zero API change" constraint keeps it safe.

Key enabling fact (verified): `PlutoniumEngine` has 50 private fields (+1 `pub size`). In Rust, a CHILD module can access private fields of a struct defined in an ANCESTOR module. Therefore method clusters can move into `impl<'a> PlutoniumEngine<'a>` blocks in new child modules (`src/font_raster.rs`, `src/glow.rs`, ...) and still touch `self.device`, `self.queue`, etc. with NO borrow restructuring and NO visibility change to the public surface. This makes the conservative path very low risk.

**Nuance to decide (not a flawed premise, a fork):**
- The "move methods into per-file impl blocks" approach is near-zero-risk and achieves navigability + the audit's intent.
- The "extract owning sub-structs (FontCache/TextureRegistry/PopupController/ClipStack)" approach is a deeper design win but fights the borrow checker (methods needing device + cache simultaneously) and risks behavior change. The task marks it "optional."
- Recommendation: default to impl-block-split + free-item relocation; reserve owning-sub-struct extraction only where state is genuinely cohesive and self-contained. The GPU-timer is the one natural sub-struct (its timestamp_* fields are touched only by the timer path).

Proceeding to plan the conservative path with per-subsystem opt-in for sub-structs.
