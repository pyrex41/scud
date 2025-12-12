# SCUD CLI Commands

## Tag Management

```bash
scud tags              # List all tags (active marked with *)
scud tags auth         # Set 'auth' as active tag
```

## Task Viewing

```bash
scud list                    # List tasks in active tag
scud list --status pending   # Filter by status
scud list --tag auth         # List tasks in specific tag
scud show 3                  # Show task 3 details
scud show 3 --tag auth       # Show task in specific tag
```

**Status filters**: `pending`, `in-progress`, `done`, `blocked`, `expanded`

## Finding Work

```bash
scud next                    # Find next available task
scud next --tag auth         # Find next in specific tag
scud waves                   # Show parallel execution waves
scud waves --all-tags        # Waves across all tags
scud waves --max-parallel 3  # Limit tasks per wave
```

## Status Updates

```bash
scud set-status 3 pending
scud set-status 3 in-progress
scud set-status 3 done
scud set-status 3 blocked
```

## Progress

```bash
scud stats                   # Completion statistics
scud stats --tag auth        # Stats for specific tag
```

## AI Commands

```bash
scud parse doc.md --tag feat # Parse PRD into tasks
scud expand 5                # Break down complex task
scud analyze-complexity      # Analyze task complexity
```

## Diagnostics

```bash
scud doctor                  # Check for issues
scud doctor --fix            # Auto-fix issues
```

## Example Workflow

```bash
scud tags auth               # Set active tag
scud waves                   # See what can run in parallel
scud next                    # Get next available task
scud set-status 3 in-progress
# ... implement ...
scud set-status 3 done
scud stats                   # Check progress
```
