# SCUD v3 Quick Reference

Read `scud --help` and `scud COMMAND --help` for supported flags, or repository `README.md` and `docs/CLI_REFERENCE.md` for details.

| Need | Supported operation |
| --- | --- |
| Progress and blockers | `scud status` (`summary.status`, `ready`, `blocked`, `revision`) |
| Poll changes as JSONL | `scud status --watch --interval 1s` |
| First ready reference | `scud next` → `{tag,id}` or `null`; not a lease |
| Task details | `scud show TASK_ID --tag PHASE` → `{ref,task}` |
| Export | `scud list` or `scud list --tag PHASE` (includes prerequisite phase closure) |
| Validate | `scud validate` or `scud validate --input proposal.json` |
| Propose from PRD | `scud parse requirements.md --tag feature --model main > proposal.json` (may call a model; no task writes) |
| Import reviewed proposal | `scud import proposal.json --policy merge --if-revision REVISION` |
| Authorized status change | `scud update TASK_ID --tag PHASE --status in-progress --if-revision REVISION` |
| Discover defaults | `scud models list --search 'QUERY' --json`; `scud models set main 'QUERY'` or `fast` |

Read a fresh revision before writes. Choose merge/replace explicitly for imports; reconcile revision conflicts. Parsing, validation, and importing are distinct. Use phase-scoped IDs and `phase:id` for cross-phase dependencies. No active-tag switch exists. `next` uses canonical ready order, not a highest-priority promise. Assignments/status are coordination metadata, not claim locks. `done` is user-reported, not verified; do not auto-complete on exit.

SCUD does not execute tasks, manage git, or expose legacy warmup/stats/waves/tags/doctor/claim/release commands. Retained slash aliases translate to queries or explain supported updates; do not pass arbitrary alias arguments into mutations.

MCP exposes `scud_tasks`, `scud_import`, `scud_update`, `scud_status`, and `scud_parse`; `SCUD_TOOLS=full` adds `scud_sb`. Require stateless MCP 2026-07-28 client compatibility, request metadata on every request, and HTTP version/content/accept headers. No initialize handshake or session exists. See `skills/scud/SKILL.md` for full transport guidance and `skills/scud-tasks/SKILL.md` for task workflows. Optional sb commands do not execute workers or make done a verification claim.
