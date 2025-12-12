---
description: Show parallel execution waves for SCUD tasks
---

Compute and display parallel execution waves based on task dependencies.

Run: `scud waves $ARGUMENTS`

Arguments: `[--tag <tag>] [--max-parallel <n>] [--all-tags]`

Explain the output:
1. Wave 1 tasks have no dependencies and can start immediately
2. Each subsequent wave depends on prior waves completing
3. Tasks within a wave can run in parallel
4. The speedup ratio shows efficiency vs sequential execution
