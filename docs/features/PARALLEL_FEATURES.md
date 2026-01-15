# SCUD Parallel Features (Planned/Experimental)

> **⚠️ IMPORTANT: Most features in this document are PLANNED but NOT YET IMPLEMENTED.**
>
> **Currently implemented:**
> - `scud assign <task-id> <assignee>` - Assign a task to someone
> - `scud who-is` - Show who is assigned to what
>
> **Not yet implemented (planned):**
> - `scud claim` / `scud release` - Task locking
> - Tag groups (`create-group`, `list-groups`, `group-status`, `add-to-group`)
>
> This document describes the vision for parallel development features. Check back for updates.

---

Enable parallel development with tag groups and task assignment for team collaboration.

---

## Overview

SCUD now supports two powerful features for parallel development:

1. **Tag Groups** - Coordinate related tags (e.g., backend/frontend) that share context
2. **Task Assignment** - Lock mechanism for multiple developers working on the same tag

---

## Tag Groups

### Concept

Tag groups allow you to work on multiple related tags simultaneously while maintaining coordination. Perfect for:

- **Backend + Frontend** split
- **Core + UI** feature development
- **API + Client** parallel work
- **Multiple workspaces/worktrees**

### Commands

#### Create a Group
```bash
scud create-group "User Authentication" --tags auth-backend,auth-frontend

# With description
scud create-group "Payment System" \
  --tags pay-backend,pay-frontend \
  --description "Complete payment processing implementation"
```

**Output:**
```
✅ Tag group created!

Group ID:            user-authentication
Name:                User Authentication
Tags:                auth-backend, auth-frontend

Usage:
  scud group-status user-authentication
  scud list --group user-authentication
  scud stats --group user-authentication
```

#### List Groups
```bash
scud list-groups
```

**Output:**
```
Tag Groups:

● User Authentication (user-authentication)
  Tags: auth-backend, auth-frontend
  Complete user auth system with API and UI

✓ Payment System (payment-system)
  Tags: pay-backend, pay-frontend
```

#### Group Status
```bash
scud group-status user-authentication
```

**Output:**
```
Group: User Authentication
==================================================

ID:                  user-authentication
Status:              Active

Tags in Group:
  auth-backend 12 tasks
  auth-frontend 8 tasks

Aggregate Statistics:
Total Tasks:         20
Pending:             10
In Progress:         5
Done:                5
Blocked:             0

Total Complexity:    65
Completion:          25%

[============                                      ]
```

#### Add Tag to Group
```bash
scud add-to-group user-authentication auth-mobile
```

---

## Workflow with Tag Groups

### Scenario: Backend/Frontend Split

```bash
# 1. Plan both features together
/tm-pm  # Create PRD

# 2. Create both feature files
# docs/features/auth-backend.md
# docs/features/auth-frontend.md

# 3. Parse both features
scud parse-prd docs/features/auth-backend.md --tag auth-backend
scud parse-prd docs/features/auth-frontend.md --tag auth-frontend

# 4. Create group
scud create-group "User Auth" --tags auth-backend,auth-frontend

# 5. Architect both together (coordinate API contracts)
/tm-architect  # While on backend tag
scud use-tag auth-frontend
/tm-architect  # While on frontend tag

# 6. Implement in parallel
# Developer A (Backend):
scud use-tag auth-backend
/tm-dev  # Work on backend tasks

# Developer B (Frontend):
scud use-tag auth-frontend
/tm-dev  # Work on frontend tasks

# Or use different worktrees:
git worktree add ../scud-backend auth-backend
git worktree add ../scud-frontend auth-frontend

# 7. Monitor overall progress
scud group-status user-auth
```

### Cross-Tag Coordination

**Backend Task** (API Endpoint):
```
auth-backend:5 | Build POST /api/auth/login endpoint | P | 5 | H
  Returns: { token: string, user: { id, email } }
```

**Frontend Task** (API Integration):
```
auth-frontend:3 | Integrate login API | P | 3 | H
  Expects: { token: string, user: { id, email } }
```

**Key:** Both tasks reference the same API contract, ensuring coordination.

---

## Task Assignment & Locking

### Concept

When multiple developers work on the same tag, task assignment prevents conflicts:

- **Claim tasks** to show you're working on them
- **Lock tasks** to prevent others from claiming
- **Track assignments** to see who's doing what
- **Auto-release** when tasks complete

### Commands

#### Assign a Task
```bash
scud assign 5 alice
```

**Output:**
```
✓ Task 5 assigned to alice
```

#### Claim a Task
```bash
scud claim 7 --name bob
```

**Output:**
```
✅ Task claimed successfully!

Task ID:             7
Title:               Build login endpoint
Claimed by:          bob
Status:              locked

Next steps:
  1. Start working on the task
  2. Run: scud set-status 7 in-progress
  3. When done: scud set-status 7 done
  4. Task will auto-release when marked done
```

**What happens:**
- Task is assigned to bob
- Task is locked by bob
- Lock timestamp recorded
- No one else can claim it

#### Release a Task
```bash
# Release your own task
scud release 7

# Force release (if someone left)
scud release 7 --force
```

**Output:**
```
⚠ Task is locked
Locked by:           bob
Locked:              2.5h ago

To force release: scud release 7 --force
```

#### Who's Working on What
```bash
scud who-is
```

**Output:**
```
Task Assignments
============================================================

● alice
  auth-backend 5 - Build registration endpoint
  auth-backend 8 - Add password hashing

● bob
  auth-backend 7 - Build login endpoint
  auth-frontend 3 - Create login form

⚠ Stale Locks (>24h)
============================================================

  auth-backend 12 locked by charlie (26.3h ago)

Consider releasing stale locks:
  scud release 12 --force
```

---

## Team Workflow

### Scenario: 3 Developers, 1 Tag

```bash
# Alice: Lead developer
cd project
scud use-tag auth
scud next              # Find next available task

Task 5: Build registration endpoint

scud claim 5 --name alice
scud set-status 5 in-progress
# ... implements task ...
scud set-status 5 done  # Auto-releases lock

# Bob: Second developer
cd project
scud use-tag auth
scud next              # Skips task 5 (locked by alice)

Task 7: Build login endpoint

scud claim 7 --name bob
scud set-status 7 in-progress

# Charlie: Third developer
cd project
scud use-tag auth
scud next

Task 9: Add email verification

scud claim 9 --name charlie
scud set-status 9 in-progress

# Team Lead: Check progress
scud who-is

# Shows:
# ● alice - Task 5 (in progress)
# ● bob - Task 7 (in progress)
# ● charlie - Task 9 (in progress)

scud stats  # Overall tag progress
```

---

## Features & Safety

### Task Locking

**Claim Prevention:**
```bash
# Alice claims task 5
scud claim 5 --name alice  # ✓ Success

# Bob tries to claim same task
scud claim 5 --name bob    # ✗ Error: Task is locked by alice
```

**Stale Lock Detection:**
- Locks >24h are flagged as stale
- Shown in `scud who-is` output
- Can be force-released

**Auto-Release:**
```bash
scud set-status 5 done    # Auto-releases lock when done
```

### Tag Groups

**Aggregate Stats:**
- See total progress across all tags in group
- Identify bottlenecks
- Balance workload

**Coordinated Planning:**
- Architect can see all tags in group
- Ensure API contracts match
- Share dependencies

---

## File Structure

```
.scud/
├── tasks/
│   └── tasks.scg               # Tasks with assigned_to, locked_by
├── config.toml                 # Active tag and settings
└── tag-groups.json             # Tag groups (NEW)
```

### tag-groups.json
```json
{
  "groups": [
    {
      "id": "user-authentication",
      "name": "User Authentication",
      "tags": [
        "auth-backend",
        "auth-frontend"
      ],
      "description": "Complete user auth system",
      "created_at": "2025-01-15T10:00:00Z",
      "status": "active"
    }
  ]
}
```

### Task with Assignment (in SCG format)
```
@nodes
auth:5 | Build registration endpoint | I | 5 | H

@meta
auth:5 | assigned_to | alice
auth:5 | locked_by | alice
auth:5 | locked_at | 2025-01-15T14:30:00Z
```

---

## Use Cases

### Use Case 1: Backend/Frontend Teams

```
Project: E-commerce checkout

Tag Group: "Checkout Flow"
- checkout-backend (Cart API, Payment API)
- checkout-frontend (Cart UI, Payment UI)

Team Backend: 2 devs
Team Frontend: 2 devs

Workflow:
1. PM creates single PRD
2. SM creates 2 feature files (backend, frontend)
3. Create group linking both
4. Architect designs both (API contracts)
5. Teams work in parallel
6. Monitor with group-status
```

### Use Case 2: Distributed Team

```
Project: User management system

Tag: "user-crud"
Developers:
- Alice (US, timezone UTC-8)
- Bob (Europe, timezone UTC+1)
- Charlie (Asia, timezone UTC+9)

Workflow:
1. All work on same tag
2. Each developer claims tasks
3. Use scud who-is to avoid conflicts
4. Work asynchronously across timezones
5. Lock prevents accidental overlaps
```

### Use Case 3: Multiple Worktrees

```
Project: Mobile + Web app

Tag Group: "Dashboard Feature"
- dashboard-web
- dashboard-mobile

Setup:
git worktree add ../project-web dashboard-web
git worktree add ../project-mobile dashboard-mobile

Developer workflow:
# Terminal 1 (Web)
cd ../project-web
scud use-tag dashboard-web
/tm-dev

# Terminal 2 (Mobile)
cd ../project-mobile
scud use-tag dashboard-mobile
/tm-dev

# Monitor both
scud group-status dashboard-feature
```

---

## Best Practices

### Tag Groups

✅ **Do:**
- Group tags that share context (API contracts, data models)
- Coordinate architecture phase across all tags in group
- Use group-status for overall progress monitoring
- Keep groups focused (2-4 tags max)

❌ **Don't:**
- Create groups for unrelated tags
- Skip architecture coordination
- Ignore API contract mismatches
- Make huge groups (>5 tags)

### Task Assignment

✅ **Do:**
- Claim tasks before starting work
- Release tasks if you step away
- Use `scud next` to find available tasks
- Check `scud who-is` before claiming
- Set status to done when complete (auto-releases)

❌ **Don't:**
- Force-release active locks (unless truly stale)
- Work on tasks without claiming
- Leave tasks locked overnight
- Claim multiple tasks simultaneously
- Forget to update task status

### Team Coordination

✅ **Do:**
- Communicate with team about task choices
- Use task dependencies to sequence work
- Monitor stale locks regularly
- Review group-status in standups
- Document API contracts in architecture phase

❌ **Don't:**
- Work in isolation without coordination
- Ignore dependencies
- Skip architecture phase
- Forget to update team on blockers

---

## Advanced: Git Worktrees

Perfect companion to tag groups!

### Setup
```bash
# Main repo
cd my-project
scud create-group "Feature X" --tags feature-x-backend,feature-x-frontend

# Create worktrees
git worktree add ../my-project-backend
git worktree add ../my-project-frontend

# Backend worktree
cd ../my-project-backend
git checkout -b feature-x-backend
scud use-tag feature-x-backend
/tm-dev  # Work on backend

# Frontend worktree
cd ../my-project-frontend
git checkout -b feature-x-frontend
scud use-tag feature-x-frontend
/tm-dev  # Work on frontend

# Monitor from anywhere
scud group-status feature-x
```

### Benefits
- Separate file trees
- No constant branch switching
- Parallel builds/tests
- IDE can run both
- Each worktree has own tag

---

## Limitations & Future

### Current Limitations

- **No real-time sync** - Tasks are locked in local files, not server-side
- **Manual conflict resolution** - If two devs claim same task offline
- **No notifications** - Won't alert when someone claims your task
- **Single active tag** - Each worktree can only have one active tag

### Planned Enhancements

- [ ] Lock server for real-time coordination
- [ ] Task notifications/webhooks
- [ ] Multi-tag view (work on multiple tags simultaneously)
- [ ] Cross-tag dependencies (task in tag A depends on task in tag B)
- [ ] Assignment rotation suggestions
- [ ] Workload balancing
- [ ] Time tracking integration
- [ ] Slack/Discord integration for whois

---

## Troubleshooting

### "Task is locked by someone else"

```bash
# Check who has it
scud who-is

# If they're done, ask them to release
# Or force release if stale
scud release <task-id> --force
```

### "Tag not found in group"

```bash
# List all groups
scud list-groups

# Add tag to group
scud add-to-group <group-id> <tag>
```

### "Stale locks everywhere"

```bash
# See all assignments
scud who-is

# Release stale locks
scud release <task-id> --force

# Or clean all (future feature)
# scud clean-locks --older-than 24h
```

---

## Summary

**Tag Groups:**
- Coordinate related tags
- Aggregate progress
- Perfect for backend/frontend splits
- Use with git worktrees

**Task Assignment:**
- Claim tasks to show you're working
- Lock prevents conflicts
- Auto-release on completion
- Monitor with `scud who-is`

**Together:**
Enable teams to work in parallel efficiently while maintaining coordination and preventing conflicts.

**Experimental Status:**
Most features described above are planned but not yet implemented. See the note at the top of this document.

---

**Quick Reference (Currently Implemented):**

```bash
# Task Assignment (implemented)
scud assign <task-id> <assignee>    # Assign a task
scud who-is                          # Show assignments

# Planned (not yet implemented)
# scud claim <task-id> --name <your-name>
# scud release <task-id> [--force]
# scud create-group "Name" --tags tag1,tag2
# scud list-groups
# scud group-status <group-id>
# scud add-to-group <group-id> <tag>
```

**Happy parallel development!**
