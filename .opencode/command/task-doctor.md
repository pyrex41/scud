---
description: Legacy doctor alias for read-only SCUD v3 graph checks
allowed-tools: Bash(scud:*)
---

Explain that the legacy `doctor` CLI command and automatic fixes are unavailable. Run `scud validate` for graph validity and `scud status` for readiness and blockers. Report errors without repairing or rewriting tasks. Do not forward `$ARGUMENTS` into a mutation. Optional external sb checks are separate and require an explicit request.
