---
name: ralph-setup
description: Set up the Ralph Wiggum autonomous development workflow for a project. Use when initializing Ralph loops, creating IMPLEMENTATION_PLAN.md, or configuring backpressure for autonomous AI development.
argument-hint: [language/framework]
---

# Ralph Wiggum Setup

Set up autonomous Claude loops with persistent state via `IMPLEMENTATION_PLAN.md`.

## Core Architecture

```
┌─────────────────────────────────────────────────────────┐
│  OUTER LOOP (loop.sh)                                   │
│  while true; do cat PROMPT.md | claude; git push; done  │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  INNER LOOP (single execution)                          │
│  1. Orient (study specs and source)                     │
│  2. Read IMPLEMENTATION_PLAN.md                         │
│  3. Select highest-priority task                        │
│  4. Investigate existing code (don't assume missing)    │
│  5. Implement via parallel subagents                    │
│  6. Validate (tests - 1 subagent only for backpressure) │
│  7. Update plan and commit                              │
│  8. Exit for context refresh                            │
└─────────────────────────────────────────────────────────┘
```

## Quick Setup

1. **Copy templates** to your project:
```bash
cp templates/* your-project/
cp templates/ralph.conf your-project/
```

2. **Run interactive setup:**
```bash
cd your-project
./setup.sh
```

3. **File structure created:**
```
project/
├── loop.sh              # Outer loop orchestrator
├── setup.sh             # Interactive configuration
├── ralph.conf           # Harness & loop settings
├── PROMPT_plan.md       # Gap analysis mode
├── PROMPT_build.md      # Implementation mode
├── AGENTS.md            # Project-specific operations
├── IMPLEMENTATION_PLAN.md  # Persistent state (generated)
└── specs/               # One file per JTBD topic
    └── *.md
```

4. **Configure backpressure** for your language - see [references/backpressure.md](references/backpressure.md)

5. **Write specs** using JTBD format - see [references/specs.md](references/specs.md)

6. **Run the loop:**
```bash
./loop.sh --plan              # Planning mode (analysis)
./loop.sh --build             # Building mode (implementation)
./loop.sh --build --allow-subtasks  # With parallel subagents
```

## Harness Configuration

Ralph supports multiple AI coding CLIs:

| Harness | Command | Best For |
|---------|---------|----------|
| `claude` | Claude Code CLI | Best agentic capabilities (recommended) |
| `opencode` | OpenCode CLI | Multi-model flexibility |
| `codex` | Codex CLI | OpenAI o1 reasoning |
| `custom` | Your command | Specialized setups |

```bash
# Claude Code (default)
./loop.sh --harness claude

# OpenCode with GPT-4
./loop.sh --harness opencode --model gpt-4-turbo

# Codex with o1
./loop.sh --harness codex --model o1

# Custom command (set CUSTOM_CMD in ralph.conf)
./loop.sh --harness custom
```

See [references/harnesses.md](references/harnesses.md) for detailed comparison.

## Tool Permissions (Claude Code)

| Mode | Allowed Tools |
|------|---------------|
| `--plan` | Read, Glob, Grep, Task, WebFetch, WebSearch |
| `--build` | Edit, Write, Bash, Read, Glob, Grep |
| `--build --allow-subtasks` | Edit, Write, Bash, Read, Glob, Grep, Task |
| `--dangerous` | All tools, no permission prompts |

Plan mode is read-only for analysis. Build mode enables file modification.
Add `--allow-subtasks` to enable parallel subagent execution.

## What Needs Project-Specific Customization

| Component | What to customize |
|-----------|-------------------|
| `AGENTS.md` | Build commands, test commands, lint commands, project conventions |
| `PROMPT_*.md` | Subagent limits, tool permissions, project-specific guardrails |
| Backpressure | Test framework, build system, linter configuration |
| Specs | Your actual requirements in JTBD format |

## Critical Language Patterns

Use these exact phrases in prompts - Claude responds to them:

- **"study"** not "read" (implies deeper analysis)
- **"don't assume not implemented"** (forces code investigation)
- **"up to N parallel subagents"** (controls parallelism)
- **"Ultrathink"** (enables extended thinking)
- **"capture the why"** (documents reasoning)
- **"keep it up to date"** (maintains plan currency)

## Steering Mechanisms

**Upstream (deterministic setup):**
- Consistent context files loaded every iteration
- Clear specs in `specs/` directory
- Explicit guardrails in prompts

**Downstream (backpressure):**
- Tests reject invalid implementations
- Build fails on compilation errors
- Linter enforces style/safety
- Type checker catches errors early

## Subagent Scaling

| Task Type | Parallelism | Model |
|-----------|-------------|-------|
| Search/read | Up to 500 | Sonnet |
| Build/test | 1 only | Sonnet |
| Architecture | As needed | Opus |

Single subagent for tests = serialized validation = backpressure.

## When to Regenerate the Plan

- Ralph going off-track
- Specs changed significantly
- Plan accumulated clutter
- Starting fresh is cheaper than fixing

The plan is disposable. One Planning loop < looping on wrong tasks.

## Safety

Run Ralph in sandboxes with:
- Minimal API keys
- No unrelated credentials
- Contained blast radius

## Templates

- [templates/loop.sh](templates/loop.sh) - Outer loop script with harness support
- [templates/setup.sh](templates/setup.sh) - Interactive configuration wizard
- [templates/ralph.conf](templates/ralph.conf) - Configuration file template
- [templates/PROMPT_plan.md](templates/PROMPT_plan.md) - Planning phase prompt
- [templates/PROMPT_build.md](templates/PROMPT_build.md) - Building phase prompt
- [templates/AGENTS.md](templates/AGENTS.md) - Operations guide template

## References

- [references/harnesses.md](references/harnesses.md) - AI CLI comparison and configuration
- [references/backpressure.md](references/backpressure.md) - Language-specific backpressure patterns
- [references/specs.md](references/specs.md) - Writing JTBD specifications
- [references/subagents.md](references/subagents.md) - Subagent patterns and scaling
