---
description: Legacy waves alias for SCUD v3 readiness inspection
---

Explain that the legacy `waves` CLI command was removed. Run `scud status` and report current `ready` refs and `blocked` reasons. Readiness is not an execution-wave schedule or reservation; do not launch workers or promise priority ordering. Do not forward `$ARGUMENTS` to mutations.
