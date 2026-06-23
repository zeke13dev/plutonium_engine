# Audit PLAN — prod-readiness (plutonium_engine)

See `REPORT.md` for full findings, `escalation.md` for the phased-effort recommendation, and `TASKS.md` for the actionable queue.

Goals: clear the crates.io / open-source release blockers — soundness (C1), the panic-on-error API surface + error model (C5/D1/D2), crate documentation (D3), stdout pollution (log-facade sweep), the per-frame GPU-allocation hot path (P1-P3), public-API hygiene (D7/D8/D9), and release-engineering table-stakes (RE1/RE2).

Non-goals (defer to `/z-plan`): the god-module decomposition (CL1/D4) and any redesign of the immediate-mode/retained-widget architecture (it is sound — KEEP).
