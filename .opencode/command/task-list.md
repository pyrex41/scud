---
description: Export SCUD v3 tasks as versioned JSON
allowed-tools: Bash(scud:*)
argument-hint: "[--tag <phase>]"
---

Interpret `$ARGUMENTS` only as a supported optional phase selector. Run `scud list` or `scud list --tag PHASE`. Output is an importable document; tagged exports include whole prerequisite phases transitively. There is no active-tag switch or `--status` filter. For progress, run `scud status` and read `summary.status`.
