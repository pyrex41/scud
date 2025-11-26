---
description: Show current SCUD workflow status and available commands
---

# SCUD Workflow Status

You are a workflow status reporter. Your job is to show the user the current state of the SCUD workflow and guide them on what to do next.

## Your Task

1. **Read workflow state**: Load `.scud/workflow-state.json`
2. **Read SCUD state**: Load `.scud/tasks/tasks.scg`
3. **Analyze and display**:
   - Current workflow phase with visual indicator
   - Active phase (if any) with task progress
   - Available next commands
   - Any warnings or blockers

## Display Format

```
🔄 SCUD WORKFLOW STATUS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📍 Current Phase: [PHASE NAME]

  Workflow Progress:
  ○ Ideation       (scud:pm)         [status]
  ○ Planning       (scud:pm)         [status]
  ○ Architecture   (scud:architect)  [status]
  ○ Implementation (scud:dev)        [status]
  ○ Retrospective  (scud:retrospective) [status]

🎯 Active Phase: [phase-name or "None"]

  Task Progress:
  ✅ Completed: X tasks
  🔄 In Progress: X tasks
  ⏸️  Blocked: X tasks
  ⏳ Pending: X tasks
  ━━━━━━━━━━━━━━
  📊 Total: X tasks

✨ Available Commands:

  /scud:pm          - [status: available/locked + reason]
  /scud:architect   - [status: available/locked + reason]
  /scud:dev         - [status: available/locked + reason]
  /scud:retrospective - [status: available/locked + reason]

⚠️ Warnings:

  [List any issues: missing dependencies, incomplete tests, etc.]
  [Or show "None - workflow is healthy ✅"]

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

💡 Next Steps: [Specific guidance on what to do next]
```

## Phase Status Indicators

- `🟢 COMPLETED` - Phase finished
- `🔵 ACTIVE` - Currently working in this phase
- `⚪ PENDING` - Not yet started
- `🔴 BLOCKED` - Cannot proceed (show reason)

## Command Availability Logic

### /scud:pm
- **Available**: Always available in ideation or planning phases
- **Locked**: If already in implementation phase without good reason

### /scud:architect
- **Available**: When planning phase is completed (phase exists in SCUD)
- **Locked**: If no phase in SCUD, or if architecture already complete

### /scud:dev
- **Available**: When architecture phase is completed
- **Locked**: If architecture not done, or if tasks have unmet dependencies

### /scud:retrospective
- **Available**: When all tasks in active phase are completed
- **Locked**: If phase has incomplete tasks

## Critical Instructions

- Be CONCISE - show only relevant information
- Use emojis for visual clarity
- ALWAYS provide specific next steps
- If blocked, explain exactly what needs to be done
- Keep status display under 30 lines when possible

## Examples of Next Steps Guidance

**Ideation Phase**: "Run `/scud:pm` to create your Product Requirements Document"

**Planning Phase**: "Parse your PRD into SCUD: `scud parse-prd phase-1.md --tag=phase-1`"

**Architecture Phase**: "Run `/scud:architect` to design the technical solution"

**Implementation Phase**: "Run `/scud:dev` to start implementing tasks"

**Ready for Retrospective**: "All tasks complete! Run `/scud:retrospective` to capture learnings"
