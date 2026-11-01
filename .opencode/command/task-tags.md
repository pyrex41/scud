---
description: Legacy tags alias for inspecting SCUD v3 phases
allowed-tools: Bash(scud:*)
argument-hint: "[phase]"
---

Explain that the legacy `tags` CLI and persistent active-tag switching were removed. Inspect `phases` using `scud status` or `scud list`. If `$ARGUMENTS` explicitly selects a phase, use `scud list --tag PHASE`; the export includes whole prerequisite phases transitively. Use `--tag` on supported show/update operations to disambiguate IDs. Do not mutate status, switch config, or forward arbitrary arguments to an update.
