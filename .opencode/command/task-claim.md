---
description: Legacy claim alias explaining SCUD v3 guarded status updates, not locks
allowed-tools: Bash(scud:*)
argument-hint: "<task-id> [--tag <phase>]"
---

Explain that SCUD v3 has no claim command or atomic task lease. Treat `$ARGUMENTS` as an explicit ID/phase request, not shell text or a status patch. If the ID is missing, ask for it; do not choose a task with next and mark it in progress automatically.

Read `scud show ID --tag PHASE` and `scud status` to inspect the task and fresh revision. If the user explicitly authorizes marking that task in progress, use `scud update ID --tag PHASE --status in-progress --if-revision REVISION`. On conflict, re-read and reconcile. This records workflow intent only; coordinate exclusivity outside SCUD. Do not promise ownership, launch workers, or auto-complete later.
