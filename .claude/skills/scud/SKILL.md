---
name: scud-guide
description: SCUD v3 CLI guide for turning PRDs into task proposals, validating and importing task JSON, discovering and setting main/fast models, and configuring stateless MCP. Use for SCUD commands and setup; use scud-tasks for current readiness, progress, and task status updates.
---

# SCUD v3 CLI Guide

Use `scud --help` and `scud COMMAND --help` for exact flags. Consult the repository `README.md` and `docs/CLI_REFERENCE.md` for the complete reference. SCUD manages a dependency graph; it does not execute task workers, manage git, or lease tasks.

## Orient without changing tasks

```bash
scud status
scud next
scud show TASK_ID --tag PHASE
scud list
```

- Read `status` as a snapshot: `schema_version`, `revision`, `phases`, `summary.total`, `summary.status`, `ready`, and `blocked`. Counts live under `summary.status`, not a top-level `status` field.
- Read `next` as the first `{ "tag": "…", "id": "…" }` in the canonical ready order, or JSON `null`. It is not a task object, priority-ranking promise, reservation, or claim.
- Read `show` as `{ "ref": {"tag": "…", "id": "…"}, "task": {...} }`.
- Read `list` as an importable `{ "schema_version": 1, "phases": [...] }` document. `list --tag PHASE` includes the transitive closure of whole prerequisite phases, not just that phase. There is no active-tag switch or `list --status` filter.
- Use `scud status --watch --interval 1s` for polling-based JSONL snapshots on changes, not an event log or lease feed.

## Discover main and fast defaults

Use authenticated provider discovery rather than inventing model IDs or editing TOML manually:

```bash
scud models list --search 'QUERY' --json
scud models set main 'QUERY'
scud models set fast 'QUERY'
```

Replace `QUERY` with the user's requested catalog search. Use `--provider PROVIDER` to narrow discovery when needed. `main` and `fast` are the two defaults, not role tiers. These commands may query providers; run them only when model discovery/configuration is requested. `scud init` initializes storage without overwriting existing tasks or model defaults.

## Propose, validate, then import

Treat these as distinct operations. Parsing can call a paid model and never writes project tasks:

```bash
scud parse requirements.md --tag feature --model main > proposal.json
scud validate --input proposal.json
scud status
# After review and explicit approval; use the revision just observed:
scud import proposal.json --policy merge --if-revision REVISION
```

Review task contents and dependencies before importing. Choose `merge` or `replace` explicitly for collisions; replacement can remove existing phase tasks. Re-read and reconcile on revision conflicts rather than retrying blindly. Use `scud validate` to check the stored graph without changing it. To add tasks manually, edit an exported versioned document, validate it, and import it with an explicit policy and revision guard; there is no `create` or `expand` command.

Keep IDs phase-scoped. Use `--tag PHASE` to disambiguate task queries and updates. Use `phase:id` for cross-phase dependency references; unqualified dependencies refer to the current phase. Preserve schema version 1 and phase/task fields from exported JSON rather than guessing a legacy file format. A nonempty legacy `tasks.scg` needs explicit conversion and archival before importing; SCUD does not migrate it automatically.

## Update deliberately

Read `status` for a revision and `show` for the exact task first. Then, only for an authorized change:

```bash
scud update TASK_ID --tag PHASE --status in-progress --if-revision REVISION
```

Patch only requested fields; omitted fields remain unchanged. Use `scud update --help` for patch flags or `--input` JSON. Revision guards prevent stale writes but do not provide worker ownership or an atomic claim. `done` is user-reported workflow state, not test evidence or verification. Do not mark tasks done automatically at session exit. Run project tests separately and report their results; use git separately only when requested.

## MCP integration

Expose five default tools: `scud_tasks`, `scud_import`, `scud_update`, `scud_status`, `scud_parse`. `SCUD_TOOLS=full` adds `scud_sb`; a custom selection such as `SCUD_TOOLS=tasks,status` narrows exposure.

```bash
scud mcp-server
scud mcp-server --transport http --listen 127.0.0.1:8080
```

Require a client compatible with **stateless MCP 2026-07-28**. There is no `initialize` handshake or session. Every request supplies `params._meta` with `io.modelcontextprotocol/protocolVersion` set to `2026-07-28` and object `io.modelcontextprotocol/clientCapabilities`; client info may also be supplied. For HTTP POST `/mcp`, also send `MCP-Protocol-Version: 2026-07-28`, `Content-Type: application/json`, `Accept: application/json, text/event-stream`, and `Mcp-Method` matching the JSON-RPC method. Tool calls also require `Mcp-Name` matching the tool name. Check client compatibility before offering desktop configuration: older session-based MCP clients do not automatically work. HTTP is loopback-only; authenticated remote proxying requires explicit setup.

Use `scud_tasks` with `id` (and optional `tag`) for a task, or `ready: true` for `{revision, ready}`; these selectors are mutually exclusive. With neither selector it returns the importable document. Use `scud_status` for the full snapshot. Import/update arguments accept `if_revision`; require the same review and authorization as CLI writes.

## Verification and legacy guidance

Use `scud sb --help` for optional external sb diagnostics and gates, only when requested. A successful sb command does not itself discharge invariants; `done` does not prove verification. No workers are launched.

Translate old workflow advice to supported queries: `status` replaces progress/statistics orientation; `validate` checks graph validity; inspect `ready`/`blocked` instead of execution waves, and `phases`/task `assigned_to` instead of active tags or lock ownership. Legacy slash aliases may remain as informational helpers, not restored CLI commands. Do not run removed warmup, set-status, stats, waves, tags, claim/release, heavy, execution, or git commands.
