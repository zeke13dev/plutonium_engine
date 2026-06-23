# Audit SPEC — prod-readiness (plutonium_engine)

This SPEC was produced by `/z-audit`, not `/z-plan`. Each task in `TASKS.md` references a finding in `REPORT.md`; the finding's "Recommendation" line is the per-task spec. The implementer should:

1. Read the task's Files + Acceptance.
2. Read the cited `REPORT.md` finding (e.g. `C1`, `D2`, `RE2`) for full context.
3. Apply the Recommendation surgically — no scope expansion beyond Files.

Read-only invariant of the audit does not bind the implementer; the implementer edits the target. But each task must stay within its cited finding's scope.
