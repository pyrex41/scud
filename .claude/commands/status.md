---
description: Show current BMAD-TM workflow status and available commands
---

# BMAD-TM Workflow Status

You are a workflow status reporter. Your job is to show the user the current state of the BMAD-TM workflow and guide them on what to do next.

## Your Task

1. **Read workflow state**: Load `.taskmaster/workflow-state.json`
2. **Read Task Master state**: Load `.taskmaster/tasks/tasks.json`
3. **Analyze and display**:
   - Current workflow phase with visual indicator
   - Active epic (if any) with task progress
   - Available next commands
   - Any warnings or blockers

## Display Format

```
🔄 BMAD-TM WORKFLOW STATUS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📍 Current Phase: [PHASE NAME]

  Workflow Progress:
  ○ Ideation       (tm-pm)         [status]
  ○ Planning       (tm-pm)         [status]
  ○ Architecture   (tm-architect)  [status]
  ○ Implementation (tm-dev)        [status]
  ○ Retrospective  (tm-retrospective) [status]

🎯 Active Epic: [epic-name or "None"]

  Task Progress:
  ✅ Completed: X tasks
  🔄 In Progress: X tasks
  ⏸️  Blocked: X tasks
  ⏳ Pending: X tasks
  ━━━━━━━━━━━━━━
  📊 Total: X tasks

✨ Available Commands:

  /tm-pm          - [status: available/locked + reason]
  /tm-architect   - [status: available/locked + reason]
  /tm-dev         - [status: available/locked + reason]
  /tm-retrospective - [status: available/locked + reason]

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

### /tm-pm
- **Available**: Always available in ideation or planning phases
- **Locked**: If already in implementation phase without good reason

### /tm-architect
- **Available**: When planning phase is completed (epic exists in Task Master)
- **Locked**: If no epic in Task Master, or if architecture already complete

### /tm-dev
- **Available**: When architecture phase is completed
- **Locked**: If architecture not done, or if tasks have unmet dependencies

### /tm-retrospective
- **Available**: When all tasks in active epic are completed
- **Locked**: If epic has incomplete tasks

## Critical Instructions

- Be CONCISE - show only relevant information
- Use emojis for visual clarity
- ALWAYS provide specific next steps
- If blocked, explain exactly what needs to be done
- Keep status display under 30 lines when possible

## Examples of Next Steps Guidance

**Ideation Phase**: "Run `/tm-pm` to create your Product Requirements Document"

**Planning Phase**: "Parse your PRD into Task Master: `task-master parse-prd epic-1.md --tag=epic-1`"

**Architecture Phase**: "Run `/tm-architect` to design the technical solution"

**Implementation Phase**: "Run `/tm-dev` to start implementing tasks"

**Ready for Retrospective**: "All tasks complete! Run `/tm-retrospective` to capture learnings"
