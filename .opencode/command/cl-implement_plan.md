---
description: Implement technical plans with verification at each phase
---

# Implement Plan

You are tasked with implementing an approved technical plan. These plans contain phases with specific changes and success criteria.

## Getting Started

When given a plan path:
1. **Read the plan completely** and check for any existing checkmarks (`- [x]`)
2. **Read the original ticket** and all files mentioned in the plan
3. **Read files fully** - never use limit/offset parameters, you need complete context
4. **Think deeply** about how the pieces fit together
5. **Create a todo list** to track your progress
6. **Start implementing** if you understand what needs to be done

If no plan path provided, ask for one.

## Implementation Philosophy

Plans are carefully designed, but reality can be messy. Your job is to:
- Follow the plan's intent while adapting to what you find
- Implement each phase fully before moving to the next
- Verify your work makes sense in the broader codebase context
- Update checkboxes in the plan as you complete sections

When things don't match the plan exactly, think about why and communicate clearly.

## Handling Mismatches

If you encounter something that doesn't match the plan:
1. **STOP** and think deeply about why
2. **Present the issue clearly**:

```
Issue in Phase [N]:
Expected: [what the plan says]
Found: [actual situation]
Why this matters: [explanation]

How should I proceed?
```

## Verification Approach

After implementing a phase:

1. **Run automated checks** (usually `make check test` or similar)
2. **Fix any issues** before proceeding
3. **Update progress** in both the plan and your todos
4. **Check off completed items** in the plan file using Edit

5. **Pause for manual verification**:

```
Phase [N] Complete - Ready for Manual Verification

Automated verification passed:
- [List automated checks that passed]

Please perform the manual verification steps listed in the plan:
- [List manual verification items from the plan]

Let me know when manual testing is complete so I can proceed to Phase [N+1].
```

**Note**: If instructed to execute multiple phases consecutively, skip the pause until the last phase.

**Important**: Do not check off manual testing items until confirmed by the user.

## If You Get Stuck

When something isn't working as expected:
1. Make sure you've read and understood all relevant code
2. Consider if the codebase has evolved since the plan was written
3. Present the mismatch clearly and ask for guidance

Use sub-tasks sparingly - mainly for targeted debugging or exploring unfamiliar territory.

## Resuming Work

If the plan has existing checkmarks:
- Trust that completed work is done
- Pick up from the first unchecked item
- Verify previous work only if something seems off

## Remember

You're implementing a solution, not just checking boxes. Keep the end goal in mind and maintain forward momentum.
