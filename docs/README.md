# SCUD 3.x documentation

## Current user and maintainer references

| Reference | Purpose |
|---|---|
| [Project README](../README.md) | Setup, main/fast models, PRD proposals, task workflows, JSON shapes and optional sb/MCP |
| [CLI reference](CLI_REFERENCE.md) | Generated exact help for every registered command and subcommand, including completion |
| [Developer guide](../go.md) | Code boundaries, Go toolchain, tests and help regeneration |
| [Work specification](V2_SPEC.md) | Current reduced task API contracts and explicit follow-up scope; filename retained from design history |
| [Redesign rationale](REDESIGN.md) | Why v3 keeps task state/inference/verification separate and removes coding execution |
| [Review handoff](REVIEW_HANDOFF.md) | Read-only review checklist and regression expectations |

Use `scud --help` and `scud COMMAND --help` for the binary actually installed.
The source-generated reference describes this checkout; a released binary does
not acquire new help text until rebuilt or upgraded.

## Agent-facing instructions

- [Project instructions](../CLAUDE.md).
- [SCUD guide](../.claude/skills/scud/SKILL.md): proposals/imports, model discovery/defaults and stateless MCP.
- [Task workflow skill](../.claude/skills/scud-tasks/SKILL.md): readiness/progress, JSON shapes and authorized guarded updates.
- Identical skill bodies in [OpenCode's guide](../.opencode/skills/scud/SKILL.md)
  and [task workflow skill](../.opencode/skills/scud-tasks/SKILL.md).
- [OpenCode quick reference](../.opencode/scud-commands.md) and
  [session bootstrap](../.opencode/hook/session-start.md).

Legacy slash-alias names may remain for familiarity. They are informational
translations to v3 queries/guarded updates, not restored CLI commands or an
automatic claim/completion mechanism. Hooks do not mark tasks done on exit.

Both skills have advice-only regression prompts/graders under their canonical
`.claude/skills/*/evals/` directories. Command help/reference sync, registered
examples, skill mirrors, live guidance, hook safety and installer behavior are
covered by `go test ./cmd/scud`.

## Historical material

[March research](../thoughts/shared/research) is explicitly marked **pre-v3**
and preserved as evidence of the old system, not current setup instructions.
The retained [executor package note](../pkg/executor/README.md) explains legacy
source only; it is not a supported worker/CLI integration. Release notes are
point-in-time records. Do not copy old execution, role/tier or session-based
MCP examples back into live guidance.
