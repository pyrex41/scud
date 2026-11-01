# Project Instructions

## SCUD v3 Task Management

Use `scud status` for current counts, revision, ready refs, and dependency blockers. Use `scud next` to read the first ready `{tag, id}` or `null`; this is not a reservation, atomic claim, or highest-priority guarantee. Inspect `scud show TASK_ID --tag PHASE` before working; its JSON contains `ref` and `task`.

Update tasks only when authorized, using `scud update TASK_ID --tag PHASE --status STATUS --if-revision REVISION` with a fresh revision from `status`. Re-read and reconcile conflicts. Do not automatically mark tasks in progress or done on session start/exit. `done` is user-reported workflow state, not verification; report tests separately. SCUD does not execute workers, manage git, or provide task leases. Use git only when requested.

Use `scud list` for an importable versioned document and `scud validate` for graph checks. Read counts under `summary.status`; inspect `ready`/`blocked` rather than invoking legacy stats/waves commands. `list --tag PHASE` includes whole prerequisite phases transitively. Cross-phase dependencies use `phase:id`.

For PRDs, keep `scud parse` → `scud validate --input proposal.json` → reviewed `scud import` distinct. Parsing can call a model but does not write tasks. Choose an explicit merge/replace import policy and use `--if-revision`. Discover and set the `main`/`fast` defaults using `scud models list` and `scud models set`; do not invent model IDs or edit TOML to configure roles.

Read `.claude/skills/scud/SKILL.md` for CLI/model/import/MCP setup and `.claude/skills/scud-tasks/SKILL.md` for readiness and status workflows. Check `scud --help`, `README.md`, and `docs/CLI_REFERENCE.md` for supported commands. Legacy slash aliases are informational translations, not restored v1 commands.

### Progress Journaling

Keep a brief progress log during complex tasks:

```
## Progress Log

### Session: 2025-01-15
- Investigated auth module, found issue in token refresh
- Updated refresh logic to handle edge case
- Tests passing, ready for review
```

This helps maintain continuity across sessions and provides context for future work.
