# Implementation Plan

Generated: 2026-02-04
Last Updated: 2026-02-04

## Summary

Replace the static Agents view in descartes-gui with an editable configuration + launch panel. Add per-task spawn to Waves view. Pass model through the bridge to headless runners. Six phases progressing from state layer through UI to tests.

## Completed

- [x] Phase 1: State — Add LaunchConfig and discovery fields to `state.rs` (completed 2026-02-04)
- [x] Phase 2: Messages + Handlers — New Message variants and update handlers in `main.rs` (completed 2026-02-04)
- [x] Phase 5: Bridge — Model plumbing + tag/agent loading in `scud_bridge.rs` (completed 2026-02-04)

## In Progress

- [ ] **[CURRENT]** Phase 3: Agents View — Config panel rewrite of `views/agents.rs`
  - Why: Primary UI deliverable; requires state and bridge to be ready
  - Rewrite as config panel with pick_lists and text_input
  - Show status, current task, launch buttons
  - Spec: FR-8 through FR-12

## Backlog (Prioritized)

1. [ ] Phase 4: Waves View — Spawn button per task in `views/waves.rs`
   - Why: Secondary UI feature; depends on SpawnTask message and bridge support
   - Add Spawn button next to non-done tasks
   - Uses current launch_config settings
   - Spec: FR-13, FR-14

2. [ ] Phase 6: Tests — Update and add tests
   - Why: Final validation; requires all implementation to be stable
   - Update test_app() for new AppState fields
   - Update test_ui_swarm_controls for new agents view signature
   - Add config message handler tests
   - Spec: FR-20 through FR-22

## Discovered Issues

- [ ] 2026-02-04: `cargo clippy -p descartes-gui -- -D warnings` fails in `scud-core/src/models/task.rs`
  due to `clippy::useless_conversion` (TaskStatus::from(self.status.clone()).into()).
  Requires scud-core change, which is blocked without explicit instruction.

## Open Questions

(none — spec is self-contained)
