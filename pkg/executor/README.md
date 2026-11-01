# Legacy executor package — not a SCUD v3 CLI feature

This package is retained for staged source cleanup and its protocol tests.
It is **not used by the registered v3 task workflow, PRD proposal path, or
experimental ensemble**. Retention is not a promise of supported public
execution commands or a reason to install a coding harness.

For current usage, read [the project README](../../README.md),
[CLI reference](../../docs/CLI_REFERENCE.md), and
[developer guide](../../go.md). Task state goes through `internal/taskapi`;
model inference uses the explicit main/fast defaults. SCUD does not own coding
worker execution, repairs, or Git commits.

## Retained source contracts

- `Runner`, `Request`, `Result` and event types describe the former bounded
  worker interface. See `executor.go` for the precise retained definitions.
- `RhoV1` and `rho.run/v1` tests exercise request/event framing, run identity,
  monotonic sequences and terminal-result constraints.
- `LegacyRho` preserves old behavior for unregistered legacy call sites.

These are implementation-maintenance facts, not current user setup steps.
Do not copy old executor configuration or worker-derived completion rules into
v3 scripts/skills. Current `done` status is user-reported; optional sb command
results do not silently change it. Audit remaining call sites before removing
the package, without restoring execution to the small public API.
