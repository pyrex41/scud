---
tags: [skill-creator]
max_turns: 6
timeout_seconds: 120
allowed_tools: [Read, Skill]
---

Advice only: do not run commands or change anything. Using the SCUD task skill, tell me how to inspect progress and get the next task, then safely mark a selected task in progress. Does next claim a lease or guarantee highest priority? What should I do if it returns null? Explain the show/status JSON shapes, concurrency guard, and what marking done actually proves. Can a session-exit hook mark it done and commit automatically? Use TASK_ID, PHASE, and REVISION placeholders rather than inventing a real task.
