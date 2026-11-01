---
name: scud-tasks
description: Read SCUD v3 task readiness, dependencies, blockers, assignments, and progress; safely update explicitly requested task fields and status. Use when the user asks what is next, what is blocked, how work is progressing, or to change a task. Use scud-guide for PRD proposals, imports, model defaults, and MCP setup.
---

# SCUD v3 Task Management

## Inspect before acting

```bash
scud status
scud next
scud show TASK_ID --tag PHASE
scud list
```

Read `status` as `{schema_version, revision, phases, summary, ready, blocked}`. Counts are in `summary.total` and `summary.status`; inspect each blocked entry's `ref` and `reasons` for dependency blockers. Status is user-reported workflow state, not verification.

Read `next` as the first ready `{tag, id}` reference or JSON `null`, not a full task. It follows canonical ready order, not a highest-priority guarantee. It neither reserves work nor creates a lease. If null, inspect `status`: no ready work does not necessarily mean everything is done.

Read `show` as `{ref, task}`. Qualify ambiguous IDs using `--tag`. IDs are phase-scoped; dependencies use local IDs or `phase:id` across phases. Read `list` as a schema-versioned importable document, not a flat task array. `list --tag PHASE` also includes whole prerequisite phases transitively. There is no persistent active tag or CLI status filter; inspect the JSON for filtering.

Use `scud status --watch --interval 1s` for polling-based JSONL snapshots on changes. Do not treat it as an event history.

## Make an authorized update

1. Read the task and a fresh `status` revision.
2. Confirm the exact phase, ID, and requested change.
3. Apply only that change using the observed revision:

```bash
scud update TASK_ID --tag PHASE --status in-progress --if-revision REVISION
```

4. Inspect the returned snapshot. On a revision conflict, re-read and reconcile; do not silently remove the guard.

Supported statuses are `pending`, `in-progress`, `done`, `failed`, `blocked`, `review`, `expanded`, `deferred`, and `cancelled`. Consult `scud update --help` for other patch fields and JSON input. Omitted fields remain unchanged; dependency updates replace the dependency list.

Treat assignments and in-progress status as coordination metadata, not an atomic claim or ownership lock. Agree on concurrent work outside SCUD. Do not start or finish tasks merely because a session starts or stops. Mark `done` only for an explicitly authorized workflow update; report tests and verification evidence separately. Use git independently only when requested: SCUD has no commit or execution command.

## Answer common requests

- **Progress/stats:** summarize `summary.status` and totals from `scud status`.
- **Ready work/waves:** show current `ready` refs and `blocked` reasons; do not invent execution waves or launch workers.
- **Tags/phases:** inspect `phases`; use `--tag` on supported list/show/update commands rather than switching an active tag.
- **Who is working:** inspect task `assigned_to` and status; there are no claim/release/whois CLI commands or worker locks.
- **Diagnose a graph:** use `scud validate` and inspect `status`; neither operation repairs the graph automatically.
- **Add or reorganize tasks:** use scud-guide's separate proposal → validation → reviewed import workflow. Parsing does not import. Imports need an explicit merge/replace policy and a revision guard.

For flags, use `scud COMMAND --help` or repository `docs/CLI_REFERENCE.md`. For MCP, use `scud_status` for snapshots and `scud_tasks` for documents, single-task wrappers, or ready refs; use `scud_update` with `if_revision` only for authorized changes. MCP requires compatible stateless 2026-07-28 clients, not a legacy initialization/session handshake; see scud-guide for transport details.
