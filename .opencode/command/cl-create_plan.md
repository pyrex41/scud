---
description: Create detailed implementation plans through interactive research and iteration
---

# Create Implementation Plan

You are tasked with creating a detailed, actionable implementation plan through collaborative research and iteration.

## Philosophy

Be skeptical and thorough. Plans should:
- Eliminate all open questions before finalization
- Be based on deep understanding of the existing codebase
- Include both automated and manual success criteria
- Be iterative - seek feedback at each phase

## Process

### Phase 1: Context Gathering

1. **Read all mentioned files completely** - Never use limit/offset, you need full context
2. **Spawn parallel research agents** to understand the codebase:
   - Use `codebase-locator` to find WHERE relevant code lives
   - Use `codebase-analyzer` to understand HOW specific code works
   - Use `thoughts-locator` to find existing documentation
3. **Present your understanding** with focused questions for clarification

### Phase 2: Research & Discovery

1. **Verify any user corrections** through new research
2. **Create a todo list** to track exploration
3. **Spawn concurrent sub-tasks** for comprehensive investigation
4. **Present findings** with design options when multiple approaches exist

### Phase 3: Plan Structure Development

1. **Propose phasing** before detailed writing
2. **Seek feedback** on organization and granularity
3. **Identify dependencies** between phases

### Phase 4: Detailed Plan Writing

Create a markdown document with this structure:

```markdown
# Plan: [Feature/Task Name]

## Overview
[2-3 sentence summary of what this plan accomplishes]

## Current State Analysis
[What exists today, relevant code locations]

## Desired End State
[Clear description of the goal]

## Implementation Approach
[High-level strategy]

## Phases

### Phase 1: [Name]
**Goal**: [What this phase accomplishes]

**Changes**:
- [ ] Change 1 (`file:line`)
- [ ] Change 2 (`file:line`)

**Success Criteria - Automated**:
- [ ] `make check` passes
- [ ] `make test` passes
- [ ] Specific test case passes

**Success Criteria - Manual**:
- [ ] UI shows expected behavior
- [ ] Edge case X works correctly

### Phase 2: [Name]
[Continue pattern...]

## Open Questions
[MUST be empty before plan is finalized]

## Risks and Mitigations
[Known risks and how to handle them]
```

### Phase 5: Sync and Review

1. **Save the plan** to appropriate location
2. **Present for feedback**
3. **Iterate based on corrections**

## Critical Requirements

### Success Criteria Distinction

Always separate into two categories:
- **Automated**: Commands that can be run (make, npm test, type checking)
- **Manual**: Requires human judgment (UI functionality, real-world performance)

### Zero Open Questions

Plans must have NO open questions before finalization. If there are unresolved decisions:
- Research more
- Ask the user
- Make a recommendation with rationale

### File References

Always include specific file:line references for:
- Code that needs to change
- Code that serves as examples
- Code that might be affected

## Important Notes

- **Interactivity over monolithic output** - Check in frequently
- **Thoroughness** - Read files completely, research comprehensively
- **Practicality** - Incremental, testable changes
- **Skepticism** - Don't assume, verify through research
