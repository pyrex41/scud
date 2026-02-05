# Subagent Patterns and Scaling

How to use parallel subagents effectively in Ralph workflows.

## Scaling Rules

| Task Type | Max Parallel | Model | Rationale |
|-----------|--------------|-------|-----------|
| File search/read | Up to 500 | Sonnet | Read-only, no conflicts |
| Code analysis | Up to 100 | Sonnet | Read-only, compute-bound |
| Implementation | Up to 10 | Sonnet | May touch same files |
| Test execution | **1 only** | Sonnet | Serialized backpressure |
| Architecture decisions | As needed | Opus | Complex reasoning |
| Debugging | 1-3 | Opus | Deep investigation |

## Why Single Subagent for Tests?

**Backpressure requires serialization.**

If 10 subagents run tests in parallel:
- Flaky tests become noise
- Resource contention causes false failures
- Hard to identify which change broke what

With 1 subagent:
- Clear cause-and-effect
- Test failure = implementation problem
- Immediate feedback loop

## Prompt Patterns

### High Parallelism (Search)

```markdown
Study the codebase to understand [topic]. Use up to 500 parallel Sonnet
subagents for file discovery and reading. Map out:
- All files related to [topic]
- Existing patterns and conventions
- Integration points
```

### Medium Parallelism (Implementation)

```markdown
Implement the following tasks. Use up to 5 parallel Sonnet subagents,
ensuring no two subagents modify the same file:
- Task A: [description]
- Task B: [description]
- Task C: [description]
```

### Single Subagent (Validation)

```markdown
Validate the implementation. Use exactly 1 Sonnet subagent to run:
1. Build
2. Lint
3. Tests

If any step fails, stop and report. Do not parallelize validation.
```

### Opus for Complexity

```markdown
This requires architectural reasoning. Use an Opus subagent to:
- Analyze the current architecture
- Identify the best approach for [requirement]
- Document tradeoffs and decision rationale
```

## Subagent Coordination

### Avoiding Conflicts

**Bad**: Multiple subagents editing `src/utils.ts`
```markdown
Use 5 subagents to implement all utility functions.
```

**Good**: Partition by file
```markdown
Use 3 subagents:
- Subagent 1: Implement string utilities in src/utils/string.ts
- Subagent 2: Implement date utilities in src/utils/date.ts
- Subagent 3: Implement validation utilities in src/utils/validation.ts
```

### Sequential Dependencies

```markdown
Execute in order (not parallel):
1. First, generate the schema types
2. Then, implement the API handlers using those types
3. Finally, write tests for the handlers
```

## Model Selection

### Use Sonnet for:
- File operations (read, write, search)
- Straightforward implementation
- Test execution
- Linting and formatting
- Routine code generation

### Use Opus for:
- Architectural decisions
- Complex debugging
- Refactoring strategies
- Performance optimization analysis
- Security review
- Novel problem solving

## Context Efficiency

Each subagent gets fresh context. Don't:
- Pass entire codebase to every subagent
- Repeat full specs in every subagent prompt

Do:
- Give specific file paths
- Reference spec sections by name
- Provide focused, task-specific context

## AGENTS.md Subagent Section

Include in your AGENTS.md:

```markdown
## Subagent Guidelines
- Search/analysis: up to 100 parallel Sonnet subagents
- Implementation: up to 5 parallel Sonnet subagents, partition by file
- Validation: exactly 1 Sonnet subagent, sequential steps
- Architecture/debugging: Opus subagent as needed

Never parallelize test execution. Tests provide backpressure only when serialized.
```
