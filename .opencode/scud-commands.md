# SCUD CLI Commands - Quick Reference

**IMPORTANT: This reference should be included in all agent contexts for SCUD operations.**

---

## Tag Management (Epic Organization)

```bash
# List all tags (epics)
scud tags

# Create new tag/epic and parse PRD into it
scud parse-prd --input=docs/epics/epic-1-auth.md --tag=epic-1-auth

# Switch to work on specific tag/epic
scud use-tag epic-1-auth

# Add a new empty tag
scud add-tag epic-2-todos --d="Todo CRUD operations"

# Copy existing tag to new tag
scud copy-tag epic-1-auth epic-1-auth-v2

# Rename a tag
scud rename-tag old-name new-name

# Delete a tag (with confirmation)
scud delete-tag epic-old
```

**Critical Note:** All task operations apply to the **currently active tag** only. Always verify which tag is active before operations.

---

## Task Viewing & Navigation

```bash
# List all tasks in active tag
scud list

# List tasks by status
scud list --status=pending
scud list --status=done
scud list --status=in-progress

# List tasks with subtasks
scud list --with-subtasks

# Show detailed task information
scud show 3

# Find next task to work on (considers dependencies)
scud next
```

---

## Task Status Management

```bash
# Update task status
scud set-status --id=3 --status=in-progress
scud set-status --id=3 --status=done
scud set-status --id=3 --status=review
scud set-status --id=3 --status=blocked

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
scud add-dependency --id=3 --depends-on=1

# Remove dependency
scud remove-dependency --id=3 --depends-on=1

# Validate all dependencies (check for issues)
scud validate-dependencies

# Fix invalid dependencies automatically
scud fix-dependencies
```

**Dependency Rules:**
- Cannot start task if dependencies not done
- Circular dependencies are invalid
- Subtask dependencies inherit from parent

---

## Task Creation & Modification

```bash
# Add new task using AI
scud add-task --prompt="Create login API endpoint" --priority=high

# Add task with dependencies
scud add-task --prompt="Add JWT middleware" --dependencies=3,4

# Remove a task
scud remove-task --id=5 -y

# Update task with new context
scud update-task --id=3 --prompt="Also needs rate limiting"

# Update multiple tasks from specific ID onwards
scud update --from=5 --prompt="All endpoints need CORS headers"
```

---

## Subtask Management

```bash
# Add subtask to parent task
scud add-subtask --parent=3 --title="Write unit tests" --description="Test all edge cases"

# Convert existing task to subtask
scud add-subtask --parent=3 --task-id=7

# Remove subtask
scud remove-subtask --id=3.1

# Remove subtask and convert to standalone task
scud remove-subtask --id=3.1 --convert

# Clear all subtasks from a task
scud clear-subtasks --id=3

# Clear all subtasks from all tasks
scud clear-subtasks --all
```

---

## Complexity Analysis & Task Breakdown

```bash
# Analyze all tasks for complexity
scud analyze-complexity

# Analyze with higher threshold (default: 5)
scud analyze-complexity --threshold=8

# Use research mode for deeper analysis
scud analyze-complexity --research

# View complexity report
scud complexity-report

# Expand single task into subtasks
scud expand --id=3 --num=5

# Expand with specific context
scud expand --id=3 --prompt="Focus on security concerns"

# Expand with research mode
scud expand --id=3 --research

# Expand all pending tasks
scud expand --all

# Force expand even if already has subtasks
scud expand --all --force
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
scud research "What is the best way to implement JWT auth?"

# Research with specific task context
scud research "Security best practices" -i=3,4,5

# Research with file context
scud research "How does this work?" -f=src/auth.js,src/middleware.js

# Research with additional context
scud research "Optimization strategies" -c="Focus on database queries"

# Save research output to file
scud research "API design patterns" -s=docs/research-api-patterns.md

# Display research as tree
scud research "System architecture" --tree

# Set detail level (1-5)
scud research "Implementation details" -d=3
```

---

## PRD Parsing & Task Generation

```bash
# Parse PRD into tasks (creates or updates tag)
scud parse-prd --input=docs/epics/epic-1-auth.md --tag=epic-1-auth

# Generate with specific number of tasks
scud parse-prd --input=docs/prd/product.md --num-tasks=15 --tag=main-product

# Generate individual task files from tasks.json
scud generate
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
scud sync-readme

# Export with subtasks
scud sync-readme --with-subtasks

# Export only specific status
scud sync-readme --status=pending
```

---

## Project Setup & Configuration

```bash
# Initialize new SCUD project
scud init

# Initialize with project details
scud init --name="My App" --description="Todo application" -y

# View AI model configuration
scud models

# Setup AI models interactively
scud models --setup

# Set main model
scud models --set-main claude-sonnet-4

# Set research model
scud models --set-research claude-opus-4

# Set fallback model
scud models --set-fallback gpt-4
```

---

## Common Workflows

### Starting New Epic
```bash
# 1. Parse PRD with tag
scud parse-prd --input=docs/epics/epic-1-auth.md --tag=epic-1-auth

# 2. Verify it's active
scud tags

# 3. List tasks
scud list

# 4. Analyze complexity
scud analyze-complexity

# 5. Expand complex tasks (>13 points)
scud expand --id=5
```

### Working on Tasks
```bash
# 1. Find next available task
scud next

# 2. Start the task
scud set-status --id=3 --status=in-progress

# 3. View task details
scud show 3

# 4. Complete the task
scud set-status --id=3 --status=done
```

### Switching Between Epics
```bash
# 1. List all epics
scud tags

# 2. Switch to different epic
scud use-tag epic-2-todos

# 3. Verify switch worked
scud list

# 4. Switch back
scud use-tag epic-1-auth
```

### Breaking Down Complex Tasks
```bash
# 1. Identify complex tasks
scud analyze-complexity --threshold=13

# 2. View report
scud complexity-report

# 3. Expand the complex task
scud expand --id=5 --num=5

# 4. Verify subtasks created
scud show 5

# 5. Update dependencies if needed
scud add-dependency --id=5.2 --depends-on=5.1
```

---

## File Locations

```
.scud/
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
   scud tags  # Shows active tag with indicator
   ```

2. **Use `scud next`** to find tasks with met dependencies:
   ```bash
   scud next  # Returns task ID or "No tasks available"
   ```

3. **Check dependencies before starting** work:
   ```bash
   scud show 3  # Shows dependencies and their status
   ```

4. **Break down tasks >13 complexity**:
   ```bash
   scud expand --id=5 --num=5
   ```

5. **Use research mode** for complex planning:
   ```bash
   scud research "Best approach for..." -i=3
   ```

6. **Validate dependencies** before marking epic complete:
   ```bash
   scud validate-dependencies
   ```

---

## Error Prevention

- Start task without checking dependencies
- Change task status without verifying work complete
- Parse PRD without `--tag` flag
- Forget which tag is active
- Create tasks with complexity >13 without breaking down

- Always use tags for epic organization
- Validate dependencies regularly
- Check `scud next` for available tasks
- Expand complex tasks into subtasks
- Use research mode for complex decisions

---

**Last Updated:** 2025-11-26
**Version:** SCUD v1.15.0
