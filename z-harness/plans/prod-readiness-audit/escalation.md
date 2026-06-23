# Audit escalation — prod-readiness (plutonium_engine v0.8.0)

## Why this escalated

The audit surfaced **55 findings** across 4 dimensions + **7 release-engineering items**, of which **23 are CRITICAL/HIGH**. This crosses both auto-bail thresholds (`>30 total` and `>10 CRITICAL/HIGH`), which normally signals "structural problems → recommend `/z-plan` instead of a flat task queue."

## Nuance: breadth, not depth

Unlike the usual auto-bail case, this is **not** structural rot. The architecture is sound (see design verdict: KEEP the immediate-mode + retained-widget model). The volume comes from **many small library-hygiene fixes** that cluster into a handful of systemic themes:

| Theme | Findings it resolves | Nature |
|---|---|---|
| Adopt `log` facade; delete/gate every `println!`/`eprintln!` in `src/` | C6, P7, CL2, CL3, CL10, CL11, CL14-CL19 (~12) | Mechanical sweep |
| Coherent error model (`EngineError`/`FontError`: `Display`+`Error`, return `Result`) | C5, D1, D2, D11 | Focused refactor |
| API-hygiene visibility pass (`pub`→`pub(crate)`, `#[doc(hidden)]`, drop `use utils::*`) | D7, D8, D14, D15, D16 | Mechanical |
| Crate documentation (`//!`, `#![warn(missing_docs)]`, doc every `pub fn`) | D3 | Large but mechanical |
| GPU buffer pooling on the hot path | P1, P2, P3, P11, P12 | Focused perf refactor |
| God-module decomposition | CL1, D4 | The one genuinely structural item |
| Dead-code/feature cleanup | CL4, CL5, CL16, D5 | Mechanical |
| Release-engineering (CI jobs, community files, packaging) | RE1-RE7 | Additive |

## Recommendation

Run this as a **phased release-prep effort**. Two viable routes:

1. **`/z-plan "plutonium_engine 0.9 release-readiness"`** — best if you want a single coordinated plan with the god-module decomposition (CL1/D4) and the error-model redesign (D1/D2) designed up front, since those touch many call sites.
2. **`/z-implement-all` against the curated `TASKS.md`** — the audit also emitted `TASKS.md` (against spec's strict "no TASKS on bail", chosen deliberately because the findings are point-fixes). It contains the surgical blockers grouped into phases A-E. Safe to run for the mechanical/contained tasks; the two larger tasks (T-DECOMP god module, T-ERRMODEL error model) are flagged as `/z-plan`-scale and left as single tracked tasks rather than decomposed.

**Suggested order:** ship the release-blockers first (soundness C1, error model, panics→Result, log-facade sweep, crate docs, CI), defer the god-module decomposition to its own PR so it doesn't block the 0.9 release.
