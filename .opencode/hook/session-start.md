---
event: session:start
---

# SCUD v3 Skills Bootstrap

Discover available skills using the environment's skill tools. Read `scud-tasks` for task status/readiness and `scud-guide` for CLI, PRDs, model defaults, imports, and MCP setup. Use native read/edit/shell tools; SCUD itself does not spawn agents or execute workers.

When task orientation is relevant, query `scud status` and `scud next`. Inspect `scud show TASK_ID --tag PHASE` before working. Read counts under `summary.status`, and inspect `ready`/`blocked`. `next` returns a first ready reference or null, not a reservation or priority guarantee.

Use `/task-list`, `/task-next`, `/task-show`, and `/task-status` for supported inspection/update guidance. Retained `/task-stats`, `/task-waves`, `/task-tags`, `/task-whois`, and `/task-doctor` aliases explain current queries. `/task-claim` and `/task-release` explain revision-guarded status changes, not ownership locks. Do not silently turn their arguments into mutations.

Keep proposal generation, validation, and reviewed import separate. Use explicit merge/replace policies and fresh revision guards for imports and authorized updates. Do not automatically start, claim, or mark tasks done on session entry/exit; completion is user-reported and needs separate test evidence. For flags consult `scud --help`, repository `README.md`, or `docs/CLI_REFERENCE.md`.
