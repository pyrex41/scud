---
description: Show one SCUD v3 task and its reference
---

Require an explicit task ID from `$ARGUMENTS`; ask for it if absent. Run `scud show TASK_ID --tag PHASE` with the selected phase when known. Output wraps `ref` and `task`. Explain details, local/cross-phase dependencies, and test strategy without changing status. Do not auto-select or claim a task.
