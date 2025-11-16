# Validation Helper for Slash Commands

This document provides Claude with validation patterns to use in slash commands.

## How to Use the Validator

The Task Master validator is available at `src/validators/taskmaster-validator.js`

You can invoke it directly or load it as a Node.js module.

## Common Validation Patterns

### Pattern 1: Validate Workflow Phase

Before activating an agent, check if the current phase allows it:

```javascript
const validator = new (require('./src/validators/taskmaster-validator.js'))();
const result = validator.validatePhase('tm-architect', ['architecture']);

if (!result.valid) {
  console.error('❌ PHASE GATE BLOCKED');
  console.error(`Current phase: ${result.currentPhase}`);
  console.error(`Allowed phases: ${result.allowedPhases.join(', ')}`);
  console.error('\nRun /status to see your current workflow state.');
  return;
}

// Proceed with agent activation
console.log('✅ Phase validation passed');
```

### Pattern 2: Validate Active Epic Exists

Before running architect or developer agents:

```javascript
const epicResult = validator.validateActiveEpic();

if (!epicResult.valid) {
  console.error('❌ NO ACTIVE EPIC');
  console.error(epicResult.error);
  console.error('\nRun /scud-pm to create an epic first.');
  return;
}

const epicTag = epicResult.epic;
const tasks = epicResult.tasks;
console.log(`✅ Active epic: ${epicTag} (${tasks.length} tasks)`);
```

### Pattern 3: Validate Task Dependencies

Before starting a task in the Developer agent:

```javascript
const depResult = validator.validateDependencies('epic-1-auth', '3');

if (!depResult.valid) {
  console.error('❌ DEPENDENCY CHECK FAILED');
  console.error(`\nTask ${depResult.task.id}: ${depResult.task.title}`);
  console.error('\nIncomplete dependencies:');

  for (const dep of depResult.unmetDependencies) {
    console.error(`  ❌ Task ${dep.id}: ${dep.title} (status: ${dep.status})`);
  }

  console.error('\nComplete these tasks first, or remove incorrect dependencies.');
  return;
}

console.log('✅ All dependencies met');
// Proceed with task implementation
```

### Pattern 4: Validate Epic Complete

Before running retrospective:

```javascript
const completeResult = validator.validateEpicComplete('epic-1-auth');

if (!completeResult.valid) {
  console.error('❌ EPIC NOT COMPLETE');
  console.error(`\nEpic has ${completeResult.incompleteTasks.length} incomplete tasks:`);

  for (const task of completeResult.incompleteTasks) {
    const statusEmoji = task.status === 'in-progress' ? '🔄' :
                       task.status === 'blocked' ? '⏸️' : '⏳';
    console.error(`  ${statusEmoji} Task ${task.id}: ${task.title} (${task.status})`);
  }

  console.error('\nComplete all tasks before running retrospective.');
  return;
}

console.log(`✅ Epic complete (${completeResult.totalTasks} tasks)`);
// Proceed with retrospective
```

### Pattern 5: Get Available Tasks

Show user which tasks can be started (no unmet dependencies):

```javascript
const availResult = validator.getAvailableTasks('epic-1-auth');

if (!availResult.valid) {
  console.error('❌ ERROR:', availResult.error);
  return;
}

console.log('**Ready to Start** (dependencies met):');
for (const task of availResult.availableTasks) {
  const statusEmoji = task.status === 'in-progress' ? '🔄' : '⏳';
  console.log(`  ✅ Task ${task.id}: ${task.title} (${task.status}, complexity: ${task.complexity})`);
}

console.log('\n**Blocked** (dependencies not met):');
for (const task of availResult.blockedTasks) {
  console.log(`  ❌ Task ${task.id}: ${task.title}`);
  console.log(`     Waiting on: ${task.unmetDependencies.map(d => `Task ${d.id}`).join(', ')}`);
}
```

### Pattern 6: Update Workflow Phase

After completing a phase (e.g., architecture done):

```javascript
const updateResult = validator.updatePhase('implementation', {
  active_epic: 'epic-1-auth'
});

if (updateResult.success) {
  console.log('✅ Workflow phase updated to: implementation');
  console.log('Run /status to see available commands.');
}
```

### Pattern 7: Add History Entry

Log important events:

```javascript
validator.addHistoryEntry({
  action: 'task_completed',
  epic: 'epic-1-auth',
  task_id: '3',
  task_title: 'Implement OAuth integration',
  tests_passed: true
});

console.log('✅ History updated');
```

### Pattern 8: Get Epic Statistics

Show progress summary:

```javascript
const stats = validator.getEpicStats('epic-1-auth');

if (stats.valid) {
  console.log(`Epic: ${stats.epic}`);
  console.log(`Total Tasks: ${stats.totalTasks}`);
  console.log(`Complexity: ${stats.totalComplexity} points`);
  console.log(`\nStatus Breakdown:`);
  console.log(`  ✅ Done: ${stats.byStatus.done}`);
  console.log(`  🔄 In Progress: ${stats.byStatus.inProgress}`);
  console.log(`  ⏸️  Blocked: ${stats.byStatus.blocked}`);
  console.log(`  ⏳ Pending: ${stats.byStatus.pending}`);
}
```

### Pattern 9: Check Command Availability

Used by /status command:

```javascript
const commands = validator.getCommandAvailability();

console.log('✨ Available Commands:');
console.log(`  /scud-pm          - ${commands['tm-pm'].available ? '✅' : '🔒'} ${commands['tm-pm'].reason}`);
console.log(`  /scud-architect   - ${commands['tm-architect'].available ? '✅' : '🔒'} ${commands['tm-architect'].reason}`);
console.log(`  /scud-dev         - ${commands['tm-dev'].available ? '✅' : '🔒'} ${commands['tm-dev'].reason}`);
console.log(`  /scud-retrospective - ${commands['tm-retrospective'].available ? '✅' : '🔒'} ${commands['tm-retrospective'].reason}`);
```

## CLI Usage Examples

You can also call the validator from the command line:

### Validate Phase
```bash
node src/validators/taskmaster-validator.js validate-phase tm-architect architecture
```

### Validate Epic
```bash
node src/validators/taskmaster-validator.js validate-epic
```

### Validate Dependencies
```bash
node src/validators/taskmaster-validator.js validate-dependencies epic-1-auth 3
```

### Validate Epic Complete
```bash
node src/validators/taskmaster-validator.js validate-epic-complete epic-1-auth
```

### Get Available Tasks
```bash
node src/validators/taskmaster-validator.js get-available-tasks epic-1-auth
```

### Get Epic Stats
```bash
node src/validators/taskmaster-validator.js get-epic-stats epic-1-auth
```

### Get Command Availability
```bash
node src/validators/taskmaster-validator.js get-command-availability
```

### Update Phase
```bash
node src/validators/taskmaster-validator.js update-phase implementation '{"active_epic":"epic-1-auth"}'
```

### Add History Entry
```bash
node src/validators/taskmaster-validator.js add-history '{"action":"task_completed","epic":"epic-1-auth","task_id":"3"}'
```

## Integration in Slash Commands

Each slash command should follow this pattern:

```markdown
---
description: Agent description
---

# Agent Name

## Phase Gate Validation

**CRITICAL: Before proceeding, validate workflow phase**

[Use validation patterns from above]

## Your Role

[Agent persona and instructions]

## Workflow

[Step-by-step workflow]

## Task Master Integration

[How to update Task Master]

## Agent Boundaries

### ✅ I CAN:
[Allowed actions]

### ❌ I CANNOT:
[Forbidden actions]

### 🔒 MUST VALIDATE BEFORE PROCEEDING:
[Checklist of validations]

## Error Handling

[Error message templates]
```

## Best Practices

1. **Always validate before proceeding** - Never skip phase gates
2. **Show clear error messages** - Tell user exactly what's wrong and how to fix it
3. **Log important events** - Add history entries for major state changes
4. **Check dependencies rigorously** - Prevent build order issues
5. **Update workflow state** - Keep state synchronized with reality
6. **Provide next steps** - Always tell user what to do next
7. **Handle errors gracefully** - Catch validation failures and guide user

## Error Handling Pattern

```javascript
try {
  const result = validator.validatePhase('tm-dev', ['implementation']);

  if (!result.valid) {
    // Show user-friendly error
    showPhaseGateError(result);
    return;
  }

  // Proceed with agent workflow
  activateAgent('tm-dev');

} catch (error) {
  console.error('❌ VALIDATION ERROR');
  console.error(error.message);
  console.error('\nIf this persists, check:');
  console.error('  • .taskmaster/workflow-state.json exists');
  console.error('  • Task Master is initialized');
  console.error('  • Run installation script again');
}
```

## Validation Cheat Sheet

| Validation | Command | When to Use |
|------------|---------|-------------|
| Phase Gate | `validate-phase` | Before activating any agent |
| Active Epic | `validate-epic` | Before architect, dev, or retrospective |
| Dependencies | `validate-dependencies` | Before starting any task |
| Epic Complete | `validate-epic-complete` | Before retrospective |
| Available Tasks | `get-available-tasks` | In developer agent to show options |
| Epic Stats | `get-epic-stats` | In /status or retrospective |
| Command Availability | `get-command-availability` | In /status command |

---

**Remember:** Validation is not optional. It's what makes SCUD enforce correct workflow usage and prevent common mistakes.
