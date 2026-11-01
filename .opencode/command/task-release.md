---
description: Legacy release alias explaining SCUD v3 guarded status updates, not unlocks
allowed-tools: Bash(scud:*)
argument-hint: "<task-id> [--tag <phase>]"
---

Explain that SCUD v3 has no release command, worker lock, or lease to unlock. Require an explicit ID and resolve its phase, then inspect `scud show ID --tag PHASE` and a fresh `scud status` revision. Ask which workflow status the user intends if not explicit; releasing does not imply pending or done.

Only after an explicit requested status change, use `scud update ID --tag PHASE --status STATUS --if-revision REVISION`. Re-read and reconcile conflicts. Do not forward arbitrary `$ARGUMENTS` into a mutation. Do not automatically mark done, clear assignment, or claim that ownership was released. `done` remains user-reported, not verified.
