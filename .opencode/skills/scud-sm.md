# SCUD v3 Task Manager Compatibility Guide

Use `scud status`, `scud next`, `scud show TASK_ID --tag PHASE`, and `scud list` to inspect tasks. `next` returns a ready reference or null, not a claim; `show` returns a ref/task wrapper. Counts are under `summary.status` in status snapshots.

Read `scud-tasks/SKILL.md` for readiness, blockers, and revision-guarded authorized updates. Read `scud/SKILL.md` for models, PRD proposals, validation/import, and stateless MCP 2026-07-28 compatibility. Check `scud --help` for commands.

Do not use removed warmup, set-status, stats, waves, heavy, execution, git, or claim-lock commands. Do not automatically update task status at session exit. `done` is user-reported workflow state, not verification.
