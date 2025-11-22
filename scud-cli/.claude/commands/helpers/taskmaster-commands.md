# Task Master CLI Commands - Quick Reference

**IMPORTANT: This reference should be included in all agent contexts for Task Master operations.**

---

## Tag Management (Epic Organization)

```bash
# List all tags (epics)
task-master tags

# Create new tag/epic and parse PRD into it
task-master parse-prd --input=docs/epics/epic-1-auth.md --tag=epic-1-auth

# Switch to work on specific tag/epic
task-master use-tag epic-1-auth

# Add a new empty tag
task-master add-tag epic-2-todos --d="Todo CRUD operations"

# Copy existing tag to new tag
task-master copy-tag epic-1-auth epic-1-auth-v2

# Rename a tag
task-master rename-tag old-name new-name

# Delete a tag (with confirmation)
task-master delete-tag epic-old
```

**Critical Note:** All task operations apply to the **currently active tag** only. Always verify which tag is active before operations.

---

## Task Viewing & Navigation

```bash
# List all tasks in active tag
task-master list

# List tasks by status
task-master list --status=pending
task-master list --status=done
task-master list --status=in-progress

# List tasks with subtasks
task-master list --with-subtasks

# Show detailed task information
task-master show 3

# Find next task to work on (considers dependencies)
task-master next
```

---

## Task Status Management

```bash
# Update task status
task-master set-status --id=3 --status=in-progress
task-master set-status --id=3 --status=done
task-master set-status --id=3 --status=review
task-master set-status --id=3 --status=blocked

# Valid status values:
# - pending
# - in-progress
# - done
# - review
# - deferred
# - cancelled
# - blocked
```

---

## Dependency Management

```bash
# Add dependency (task 3 depends on task 1)
task-master add-dependency --id=3 --depends-on=1

# Remove dependency
task-master remove-dependency --id=3 --depends-on=1

# Validate all dependencies (check for issues)
task-master validate-dependencies

# Fix invalid dependencies automatically
task-master fix-dependencies
```

**Dependency Rules:**
- Cannot start task if dependencies not done
- Circular dependencies are invalid
- Subtask dependencies inherit from parent

---

## Task Creation & Modification

```bash
# Add new task using AI
task-master add-task --prompt="Create login API endpoint" --priority=high

# Add task with dependencies
task-master add-task --prompt="Add JWT middleware" --dependencies=3,4

# Remove a task
task-master remove-task --id=5 -y

# Update task with new context
task-master update-task --id=3 --prompt="Also needs rate limiting"

# Update multiple tasks from specific ID onwards
task-master update --from=5 --prompt="All endpoints need CORS headers"
```

---

## Subtask Management

```bash
# Add subtask to parent task
task-master add-subtask --parent=3 --title="Write unit tests" --description="Test all edge cases"

# Convert existing task to subtask
task-master add-subtask --parent=3 --task-id=7

# Remove subtask
task-master remove-subtask --id=3.1

# Remove subtask and convert to standalone task
task-master remove-subtask --id=3.1 --convert

# Clear all subtasks from a task
task-master clear-subtasks --id=3

# Clear all subtasks from all tasks
task-master clear-subtasks --all
```

---

## Complexity Analysis & Task Breakdown

```bash
# Analyze all tasks for complexity
task-master analyze-complexity

# Analyze with higher threshold (default: 5)
task-master analyze-complexity --threshold=8

# Use research mode for deeper analysis
task-master analyze-complexity --research

# View complexity report
task-master complexity-report

# Expand single task into subtasks
task-master expand --id=3 --num=5

# Expand with specific context
task-master expand --id=3 --prompt="Focus on security concerns"

# Expand with research mode
task-master expand --id=3 --research

# Expand all pending tasks
task-master expand --all

# Force expand even if already has subtasks
task-master expand --all --force
```

**Fibonacci Complexity Scale:**
- 1: Trivial (< 30 min)
- 2: Simple (30 min - 1 hour)
- 3: Moderate (1-2 hours)
- 5: Complex (2-4 hours)
- 8: Very Complex (4-8 hours)
- 13: Extremely Complex (1 day) - **SPLIT INTO SUBTASKS**

---

## AI Research & Context

```bash
# Perform research query
task-master research "What is the best way to implement JWT auth?"

# Research with specific task context
task-master research "Security best practices" -i=3,4,5

# Research with file context
task-master research "How does this work?" -f=src/auth.js,src/middleware.js

# Research with additional context
task-master research "Optimization strategies" -c="Focus on database queries"

# Save research output to file
task-master research "API design patterns" -s=docs/research-api-patterns.md

# Display research as tree
task-master research "System architecture" --tree

# Set detail level (1-5)
task-master research "Implementation details" -d=3
```

---

## PRD Parsing & Task Generation

```bash
# Parse PRD into tasks (creates or updates tag)
task-master parse-prd --input=docs/epics/epic-1-auth.md --tag=epic-1-auth

# Generate with specific number of tasks
task-master parse-prd --input=docs/prd/product.md --num-tasks=15 --tag=main-product

# Generate individual task files from tasks.json
task-master generate
```

**PRD Format Requirements:**
- Use markdown with clear sections
- Tasks should be under `## Tasks` heading
- Format: `### Task N: Title`
- Include Description, Complexity, Dependencies

---

## Export & Documentation

```bash
# Export tasks to README.md
task-master sync-readme

# Export with subtasks
task-master sync-readme --with-subtasks

# Export only specific status
task-master sync-readme --status=pending
```

---

## Project Setup & Configuration

```bash
# Initialize new Task Master project
task-master init

# Initialize with project details
task-master init --name="My App" --description="Todo application" -y

# View AI model configuration
task-master models

# Setup AI models interactively
task-master models --setup

# Set main model
task-master models --set-main claude-sonnet-4

# Set research model
task-master models --set-research claude-opus-4

# Set fallback model
task-master models --set-fallback gpt-4
```

---

## Common Workflows

### Starting New Epic
```bash
# 1. Parse PRD with tag
task-master parse-prd --input=docs/epics/epic-1-auth.md --tag=epic-1-auth

# 2. Verify it's active
task-master tags

# 3. List tasks
task-master list

# 4. Analyze complexity
task-master analyze-complexity

# 5. Expand complex tasks (>13 points)
task-master expand --id=5
```

### Working on Tasks
```bash
# 1. Find next available task
task-master next

# 2. Start the task
task-master set-status --id=3 --status=in-progress

# 3. View task details
task-master show 3

# 4. Complete the task
task-master set-status --id=3 --status=done
```

### Switching Between Epics
```bash
# 1. List all epics
task-master tags

# 2. Switch to different epic
task-master use-tag epic-2-todos

# 3. Verify switch worked
task-master list

# 4. Switch back
task-master use-tag epic-1-auth
```

### Breaking Down Complex Tasks
```bash
# 1. Identify complex tasks
task-master analyze-complexity --threshold=13

# 2. View report
task-master complexity-report

# 3. Expand the complex task
task-master expand --id=5 --num=5

# 4. Verify subtasks created
task-master show 5

# 5. Update dependencies if needed
task-master add-dependency --id=5.2 --depends-on=5.1
```

---

## File Locations

```
.taskmaster/
├── tasks/
│   └── tasks.json          # All tasks (organized by tags)
├── config.json             # AI model configuration
└── task-files/             # Individual task files (if using generate)
```

---

## Environment Variables

Required in `.env`:
```bash
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
# Add other provider keys as needed
```

---

## Tips for Agents

1. **Always verify active tag** before task operations:
   ```bash
   task-master tags  # Shows active tag with indicator
   ```

2. **Use `task-master next`** to find tasks with met dependencies:
   ```bash
   task-master next  # Returns task ID or "No tasks available"
   ```

3. **Check dependencies before starting** work:
   ```bash
   task-master show 3  # Shows dependencies and their status
   ```

4. **Break down tasks >13 complexity**:
   ```bash
   task-master expand --id=5 --num=5
   ```

5. **Use research mode** for complex planning:
   ```bash
   task-master research "Best approach for..." -i=3
   ```

6. **Validate dependencies** before marking epic complete:
   ```bash
   task-master validate-dependencies
   ```

---

## Error Prevention

❌ **Don't:**
- Start task without checking dependencies
- Change task status without verifying work complete
- Parse PRD without `--tag` flag
- Forget which tag is active
- Create tasks with complexity >13 without breaking down

✅ **Do:**
- Always use tags for epic organization
- Validate dependencies regularly
- Check `task-master next` for available tasks
- Expand complex tasks into subtasks
- Use research mode for complex decisions

---

**Last Updated:** 2025-11-04
**Version:** SCUD v1.0
