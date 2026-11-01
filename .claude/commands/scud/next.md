---
description: Read the first ready SCUD v3 task reference without claiming it
allowed-tools: Bash(scud:*)
---

Run `scud next`. It returns the first ready `{tag,id}` in canonical order or `null`, not a task object, priority guarantee, claim, or lease. Inspect a returned task using `scud show ID --tag PHASE`. On null, inspect `scud status` for blockers and counts. Do not start workers, change status, or forward unsupported `$ARGUMENTS`.
