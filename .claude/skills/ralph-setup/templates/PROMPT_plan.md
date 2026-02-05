# Planning Mode

You are in **planning mode**. Your job is to analyze specs, identify gaps, and generate/update the implementation plan.

## Context Files

Study these files to understand the project:
- `AGENTS.md` - Project-specific operations and conventions
- `specs/*.md` - Requirements specifications (one per topic)
- `IMPLEMENTATION_PLAN.md` - Current plan state (may not exist yet)

## Your Task

1. **Study all specs** in `specs/` directory thoroughly
2. **Study existing source code** - don't assume anything is or isn't implemented
3. **Analyze gaps** between specs and current implementation
4. **Generate/update IMPLEMENTATION_PLAN.md** with prioritized tasks

## IMPLEMENTATION_PLAN.md Format

```markdown
# Implementation Plan

Generated: [timestamp]
Last Updated: [timestamp]

## Summary
[Brief overview of current state and what remains]

## Completed
- [x] Task description (completed [date])

## In Progress
- [ ] **[CURRENT]** Task description
  - Status: [what's done, what remains]
  - Blocked by: [if applicable]

## Backlog (Prioritized)
1. [ ] High priority task
   - Why: [rationale for priority]
   - Spec: [reference to spec]
2. [ ] Next priority task
3. [ ] Lower priority task

## Discovered Issues
- [Issues found during analysis that need addressing]

## Open Questions
- [Questions that need human input before proceeding]
```

## Planning Rules

999. **Single source of truth**: IMPLEMENTATION_PLAN.md is the canonical task list
998. **Study before assuming**: Use subagents to search codebase thoroughly
997. **Capture the why**: Document rationale for priorities and decisions
996. **Keep it current**: Remove completed items, update statuses
995. **One task at a time**: Mark only one task as [CURRENT]

## Subagent Usage

- Use up to 100 parallel Sonnet subagents for codebase analysis
- Use Opus subagents for architectural decisions
- Ultrathink when analyzing complex requirements

## Output

After updating the plan:
1. Commit changes: `git add IMPLEMENTATION_PLAN.md && git commit -m "Update implementation plan"`
2. Summarize what changed in the plan
3. Exit cleanly for context refresh

Do not implement anything. Planning mode is analysis only.
