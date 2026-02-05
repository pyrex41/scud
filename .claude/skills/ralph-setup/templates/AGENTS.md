# AGENTS.md - Project Operations Guide

<!--
CUSTOMIZE THIS FILE for your specific project.
Keep it brief - this is loaded every iteration.
-->

## Project Overview

<!-- One paragraph max -->
[Brief description of what this project does]

## Tech Stack

- Language: [TypeScript/Rust/Python/Go/etc.]
- Framework: [Next.js/Actix/FastAPI/etc.]
- Database: [PostgreSQL/SQLite/etc.]
- Key dependencies: [list main libraries]

## Directory Structure

```
src/
├── [main directories]
└── [explain organization]
tests/
└── [test organization]
```

## Validation Commands

<!-- CRITICAL: Ralph uses these for backpressure -->

- **Build**: `[your build command]`
- **Test**: `[your test command]`
- **Lint**: `[your lint command]`
- **Type check**: `[your type check command]`
- **Full check**: `[combined command that runs all above]`

Run full check before every commit. All must pass.

## Conventions

<!-- Project-specific patterns Ralph should follow -->

### Code Style
- [Key style rules]
- [Naming conventions]
- [File organization rules]

### Patterns
- [Architecture patterns used]
- [Error handling approach]
- [Logging conventions]

### Testing
- [Test naming conventions]
- [What to test]
- [Test file locations]

## Subagent Guidelines

- Search/analysis: up to 100 parallel Sonnet subagents
- Implementation: up to 5 parallel Sonnet subagents, partition by file
- Validation: exactly 1 Sonnet subagent, sequential steps
- Architecture/debugging: Opus subagent as needed

Never parallelize test execution.

## Common Operations

### Adding a new [feature type]
1. [Step 1]
2. [Step 2]
3. [Step 3]

### Modifying [common thing]
1. [Step 1]
2. [Step 2]

## Environment

<!-- Only include if Ralph needs to know -->
- Required env vars: [list]
- Local setup: [brief notes]

## Guardrails

<!-- Things Ralph must never do -->
- Never modify [protected files]
- Never commit [sensitive patterns]
- Always [critical requirement]
