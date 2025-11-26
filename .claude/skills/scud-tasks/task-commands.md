# SCUD Task Commands Reference

Complete reference for all SCUD CLI commands related to task management.

## Tag Management

### `scud tags [tag]`
List or set the active task tag.

```bash
scud tags              # List all tags
scud tags auth         # Set 'auth' as active tag
```

**Output:**
```
Tags:
  * auth (active)
    api
    ui
```

## Task Viewing

### `scud list`
List tasks in a tag.

```bash
scud list                              # List active tag tasks
scud list --tag auth                   # List tasks in 'auth'
scud list --status pending             # Filter by status
scud list --status in-progress --tag api
```

**Status filters:** `pending`, `in-progress`, `done`, `blocked`, `review`, `deferred`, `cancelled`, `expanded`

**Output:**
```
Tasks for tag: auth

  ID   | Status | Title                    | Complexity
  -----+--------+--------------------------+------------
  1    | P      | Create user model        | 3
  2    | I      | Auth middleware          | 5
  3    | D      | Login endpoint           | 5
```

### `scud show <task-id>`
Show detailed information about a task.

```bash
scud show 1                    # Show task 1 in active tag
scud show 1 --tag auth         # Show task 1 in 'auth' tag
```

**Output:**
```
Task: 1
Title: Create user model
Status: Pending
Complexity: 3 (Small)
Priority: High

Description:
  Create the User model with email, password_hash, and timestamps.

Dependencies: none

Test Strategy:
  - Unit tests for validation
  - Integration tests for database operations

Assigned to: (none)
Locked by: (none)
```

## Task Status Updates

### `scud set-status <task-id> <status>`
Update a task's status.

```bash
scud set-status 1 in-progress          # Start working
scud set-status 1 done                 # Mark complete
scud set-status 1 blocked              # Mark blocked
scud set-status 1 review               # Submit for review
scud set-status 1 pending --tag auth   # Reset to pending
```

**Valid statuses:** `pending`, `in-progress`, `done`, `blocked`, `review`, `deferred`, `cancelled`

## Finding Work

### `scud next`
Find the next available task based on dependencies and status.

```bash
scud next                              # Find next in active tag
scud next --tag auth                   # Find next in 'auth'
scud next --claim --name alice         # Find and claim in one step
```

**Output:**
```
Next available task:

  ID: 2
  Title: Auth middleware
  Complexity: 5
  Dependencies: 1 (done)

  scud claim 2 --name <your-name> --tag auth
```

### `scud waves`
Compute and display parallel execution waves based on dependencies.

```bash
scud waves                             # Waves for active tag
scud waves --tag auth                  # Waves for 'auth'
scud waves --max-parallel 3            # Limit parallel tasks
scud waves --all-tags                  # Show all tags
```

**Output:**
```
Waves for tag: auth

Wave 1: (2 tasks, can run in parallel)
  P 1 | Create user model [3]
  P 2 | Auth config [2]

Wave 2: (3 tasks, depends on Wave 1)
  P 3 | Auth middleware <- 1 [5]
  P 4 | Registration <- 1,2 [5]
  P 5 | Password reset <- 1 [3]

Wave 3: (2 tasks, depends on Wave 2)
  P 6 | Login flow <- 3,4 [5]
  P 7 | Session mgmt <- 3 [3]

Summary:
  Tasks: 7
  Waves: 3
  Speedup: 2.3x (vs sequential)
```

**Understanding waves:**
- Wave 1 tasks have no dependencies - start here
- Each subsequent wave depends on previous waves completing
- Tasks within a wave can run in parallel
- `--max-parallel N` splits large waves into batches

## Task Assignment & Locking

### `scud claim <task-id> --name <name>`
Claim a task to prevent concurrent work conflicts.

```bash
scud claim 3 --name alice              # Claim task 3
scud claim 3 --name alice --tag auth   # Claim in specific tag
```

**Output:**
```
Claimed task 3 for alice

  Title: Auth middleware
  Status: Pending -> In Progress
  Locked by: alice
  Locked at: 2025-11-26T10:30:00Z
```

### `scud release <task-id>`
Release a claimed task.

```bash
scud release 3                         # Release task 3
scud release 3 --force                 # Force release (any lock)
scud release 3 --tag auth              # Release in specific tag
```

**Output:**
```
Released task 3

  Previously locked by: alice
  Duration: 2h 15m
```

### `scud whois`
Show who is working on what tasks.

```bash
scud whois                             # Active tag assignments
scud whois --tag auth                  # Specific tag
```

**Output:**
```
Task Assignments (auth):

  Task | Assigned To | Locked By | Lock Age
  -----+-------------+-----------+---------
  3    | alice       | alice     | 2h 15m
  5    | bob         | (none)    | -
  7    | alice       | (none)    | -
```

### `scud assign <task-id> <assignee>`
Assign a task to someone (informational, doesn't lock).

```bash
scud assign 5 bob                      # Assign task 5 to bob
scud assign 5 bob --tag auth           # Assign in specific tag
```

## Progress & Statistics

### `scud stats`
Show completion statistics for a tag.

```bash
scud stats                             # Active tag stats
scud stats --tag auth                  # Specific tag stats
```

**Output:**
```
Statistics for tag: auth

  Total Tasks:    12
  Pending:        5  (42%)
  In Progress:    2  (17%)
  Done:           4  (33%)
  Blocked:        1  (8%)
  Expanded:       0

  Total Complexity: 47 points
  Completed:        18 points (38%)
  Remaining:        29 points

  Progress: [=========>          ] 38%
```

## Maintenance

### `scud doctor`
Diagnose and fix issues with tasks.

```bash
scud doctor                            # Check active tag
scud doctor --tag auth                 # Check specific tag
scud doctor --stale-hours 12           # Custom stale threshold
scud doctor --fix                      # Auto-fix issues
```

**Checks:**
- Stale locks (default: >24 hours old)
- Orphaned subtasks
- Circular dependencies
- Missing dependencies

**Output:**
```
Diagnosis for tag: auth

Issues found:
  WARN: Task 3 has stale lock (36 hours old, locked by alice)
  WARN: Task 7 depends on non-existent task 99

Run with --fix to auto-repair:
  scud doctor --tag auth --fix
```

## Task Expansion

### `scud expand <task-id>`
Break down a complex task into subtasks.

```bash
scud expand 4                          # Expand task 4
scud expand 4 --tag auth               # Expand in specific tag
scud expand --all                      # Expand all tasks >= 13 points
```

**Output:**
```
Expanding task 4: Implement login/logout [13 points]

Created subtasks:
  4.1 | Login form component [3]
  4.2 | Login API endpoint [3]
  4.3 | Token generation [2]
  4.4 | Logout endpoint [2]
  4.5 | Auth state management [3]

Task 4 status: Expanded
Total subtask points: 13 (matches parent)
```

### `scud analyze-complexity`
AI-powered complexity analysis for tasks.

```bash
scud analyze-complexity                # Analyze all tasks
scud analyze-complexity --task 5       # Analyze specific task
scud analyze-complexity --tag auth     # Analyze specific tag
```

**Output:**
```
Complexity Analysis for tag: auth

  Task 5: Password reset
    Current: 3 points
    Suggested: 5 points
    Reason: Involves email integration, token management, security considerations

  Task 7: Session management
    Current: 8 points
    Suggested: 13 points (NEEDS EXPANSION)
    Reason: Multiple storage backends, invalidation logic, concurrent session handling
```

## Examples

### Complete Task Workflow
```bash
# 1. Set active tag
scud tags auth

# 2. See what needs doing
scud waves

# 3. Find next available task
scud next

# 4. Claim it
scud claim 3 --name dev

# 5. Start working
scud set-status 3 in-progress

# 6. View details while working
scud show 3

# 7. Mark complete
scud set-status 3 done

# 8. Release lock
scud release 3

# 9. Check progress
scud stats
```

### Parallel Team Work
```bash
# Developer 1
scud next --claim --name alice --tag auth
# Works on task...
scud set-status <id> done
scud release <id>

# Developer 2 (concurrent)
scud next --claim --name bob --tag auth
# Works on different task...
scud set-status <id> done
scud release <id>

# See who's working on what
scud whois --tag auth
```
