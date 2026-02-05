# Building Mode

You are in **building mode**. Your job is to implement the highest-priority task from the plan.

## Context Files

Study these files:
- `AGENTS.md` - Project-specific operations, commands, conventions
- `IMPLEMENTATION_PLAN.md` - Your task list (pick the [CURRENT] task or highest priority)
- `specs/*.md` - Requirements for context (reference as needed)

## Your Task (One Iteration)

1. **Read IMPLEMENTATION_PLAN.md** - identify the current task
2. **Study relevant specs** for that task
3. **Investigate existing code** - don't assume not implemented
4. **Implement the task** using subagents
5. **Validate** with tests (single subagent, serialized)
6. **Update the plan** - mark complete, select next task
7. **Commit and exit** for context refresh

## Implementation Rules

999. **Don't assume not implemented**: Always search codebase first
998. **Complete implementations only**: No stubs, no TODOs, no placeholders
997. **Fix bugs you find**: Even if tangential to current task
996. **Follow project conventions**: See AGENTS.md
995. **One task per iteration**: Don't try to do everything

## Subagent Usage

**For investigation/search:**
- Up to 100 parallel Sonnet subagents
- "Study the codebase for [topic]"

**For implementation:**
- Up to 5 parallel Sonnet subagents
- Partition by file to avoid conflicts
- "Implement [specific thing] in [specific file]"

**For validation (CRITICAL):**
- Exactly 1 Sonnet subagent
- Sequential: build → lint → test
- "Validate: run build, lint, then tests. Stop on first failure."

**For complex problems:**
- Opus subagent for debugging or architecture
- Ultrathink for novel solutions

## Validation Commands

Run these before committing (see AGENTS.md for project-specific commands):
```bash
# Example - customize per project
npm run check  # or: cargo check && cargo test, etc.
```

If validation fails:
1. Fix the issue
2. Re-run validation
3. Only commit when all checks pass

## Commit Pattern

```bash
git add -A
git commit -m "[task-id] Brief description of what was implemented

- Detail 1
- Detail 2

Closes: [spec reference if completing a requirement]"
```

## Plan Update Pattern

After completing a task:

```markdown
## Completed
- [x] The task you just finished (completed YYYY-MM-DD)

## In Progress
- [ ] **[CURRENT]** Next highest priority task
```

## Exit Conditions

Exit the iteration after:
- Successfully completing one task and updating the plan
- Encountering a blocker that needs human input
- Validation repeatedly failing (document in plan, exit)
- Discovering the task is already implemented

Always commit your progress before exiting, even partial progress.
