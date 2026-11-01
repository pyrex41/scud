---
description: Inspect SCUD v3 status or make an explicitly requested guarded task update
allowed-tools: Bash(scud:*)
argument-hint: "[<task-id> <status> --tag <phase>]"
---

With no explicit change requested, run `scud status`; read counts in `summary.status` and inspect `ready`/`blocked`. Treat `$ARGUMENTS` as user intent, not shell text to pass into a mutation. For an authorized change, require an explicit ID and status, resolve the phase, read `scud show ID --tag PHASE` and a fresh status revision, then use `scud update ID --tag PHASE --status STATUS --if-revision REVISION`. Re-read and reconcile conflicts. Status/assignment are not locks; `done` is user-reported, not verified. Never auto-complete on exit or commit through SCUD.
