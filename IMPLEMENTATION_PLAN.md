# Implementation Plan

Generated: 2026-02-04
Last Updated: 2026-02-04

## Summary

Replace the static Agents view in descartes-gui with an editable configuration + launch panel. Add per-task spawn to Waves view. Pass model through the bridge to headless runners. Six phases progressing from state layer through UI to tests.

## Completed

- [x] Phase 1: State — Add LaunchConfig and discovery fields to `state.rs` (completed 2026-02-04)
- [x] Phase 2: Messages + Handlers — New Message variants and update handlers in `main.rs` (completed 2026-02-04)
- [x] Phase 5: Bridge — Model plumbing + tag/agent loading in `scud_bridge.rs` (completed 2026-02-04)
- [x] Phase 3: Agents View — Config panel rewrite of `views/agents.rs` (completed 2026-02-04)
- [x] Phase 4: Waves View — Spawn button per task in `views/waves.rs` (completed 2026-02-04)
- [x] Phase 6: Tests — Update and add tests (completed 2026-02-04)
- [x] Resolve clippy failures in scud-core/scud-cli/descartes-gui (completed 2026-02-04)
- [x] Review backlog and select next task (completed 2026-02-04)
- [x] Verify gui-config-controls spec fully implemented (completed 2026-02-04)

## In Progress

- [ ] **[CURRENT]** Await next prioritized task (backlog empty)

## Backlog (Prioritized)


## Discovered Issues

- [x] 2026-02-04: `cargo clippy -p descartes-gui -- -D warnings` fails in `scud-core/src/models/task.rs`
  due to `clippy::useless_conversion` (TaskStatus::from(self.status.clone()).into()).
  Resolved 2026-02-04.

## Open Questions

(none — spec is self-contained)
