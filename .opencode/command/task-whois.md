---
description: Legacy whois alias for SCUD v3 assignment inspection
allowed-tools: Bash(scud:*)
---

Explain that the legacy `whois` CLI and worker-lock ownership queries are unavailable. Read `scud list` and inspect task `assigned_to` and status, using `scud show ID --tag PHASE` if an explicit ID is supplied. Assignment and in-progress status are coordination metadata, not evidence of an active worker or exclusive ownership. Do not forward `$ARGUMENTS` into mutations.
