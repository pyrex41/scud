# SCUD Parallel Features (Experimental)

Enable parallel development with epic groups and task assignment for team collaboration.

---

## Overview

SCUD now supports two powerful features for parallel development:

1. **Epic Groups** - Coordinate related epics (e.g., backend/frontend) that share context
2. **Task Assignment** - Lock mechanism for multiple developers working on the same epic

---

## Epic Groups

### Concept

Epic groups allow you to work on multiple related epics simultaneously while maintaining coordination. Perfect for:

- **Backend + Frontend** split
- **Core + UI** feature development
- **API + Client** parallel work
- **Multiple workspaces/worktrees**

### Commands

#### Create a Group
```bash
scud create-group "User Authentication" --epics epic-1-auth-backend,epic-1-auth-frontend

# With description
scud create-group "Payment System" \
  --epics epic-2-pay-backend,epic-2-pay-frontend \
  --description "Complete payment processing implementation"
```

**Output:**
```
✅ Epic group created!

Group ID:            user-authentication
Name:                User Authentication
Epics:               epic-1-auth-backend, epic-1-auth-frontend

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
Epic Groups:

● User Authentication (user-authentication)
  Epics: epic-1-auth-backend, epic-1-auth-frontend
  Complete user auth system with API and UI

✓ Payment System (payment-system)
  Epics: epic-2-pay-backend, epic-2-pay-frontend
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

Epics in Group:
  epic-1-auth-backend 12 tasks
  epic-1-auth-frontend 8 tasks

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

#### Add Epic to Group
```bash
scud add-to-group user-authentication epic-1-auth-mobile
```

---

## Workflow with Epic Groups

### Scenario: Backend/Frontend Split

```bash
# 1. Plan both epics together
/tm-pm  # Create PRD

# 2. Create both epic files
# docs/epics/auth-backend.md
# docs/epics/auth-frontend.md

# 3. Parse both epics
scud parse-prd docs/epics/auth-backend.md --tag epic-1-auth-backend
scud parse-prd docs/epics/auth-frontend.md --tag epic-1-auth-frontend

# 4. Create group
scud create-group "User Auth" --epics epic-1-auth-backend,epic-1-auth-frontend

# 5. Architect both together (coordinate API contracts)
/tm-architect  # While on backend epic
scud use-tag epic-1-auth-frontend
/tm-architect  # While on frontend epic

# 6. Implement in parallel
# Developer A (Backend):
scud use-tag epic-1-auth-backend
/tm-dev  # Work on backend tasks

# Developer B (Frontend):
scud use-tag epic-1-auth-frontend
/tm-dev  # Work on frontend tasks

# Or use different worktrees:
git worktree add ../scud-backend epic-1-auth-backend
git worktree add ../scud-frontend epic-1-auth-frontend

# 7. Monitor overall progress
scud group-status user-auth
```

### Cross-Epic Coordination

**Backend Task** (API Endpoint):
```json
{
  "id": "5",
  "title": "Build POST /api/auth/login endpoint",
  "description": "Create login endpoint with JWT response",
  "details": "Returns: { token: string, user: { id, email } }"
}
```

**Frontend Task** (API Integration):
```json
{
  "id": "3",
  "title": "Integrate login API",
  "description": "Call POST /api/auth/login from login form",
  "details": "Expects: { token: string, user: { id, email } }"
}
```

**Key:** Both tasks reference the same API contract, ensuring coordination.

---

## Task Assignment & Locking

### Concept

When multiple developers work on the same epic, task assignment prevents conflicts:

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
scud whois
```

**Output:**
```
Task Assignments
============================================================

● alice
  epic-1-auth-backend 5 - Build registration endpoint
  epic-1-auth-backend 8 - Add password hashing

● bob
  epic-1-auth-backend 7 - Build login endpoint
  epic-1-auth-frontend 3 - Create login form

⚠ Stale Locks (>24h)
============================================================

  epic-1-auth-backend 12 locked by charlie (26.3h ago)

Consider releasing stale locks:
  scud release 12 --force
```

---

## Team Workflow

### Scenario: 3 Developers, 1 Epic

```bash
# Alice: Lead developer
cd project
scud use-tag epic-1-auth
scud next              # Find next available task

Task 5: Build registration endpoint

scud claim 5 --name alice
scud set-status 5 in-progress
# ... implements task ...
scud set-status 5 done  # Auto-releases lock

# Bob: Second developer
cd project
scud use-tag epic-1-auth
scud next              # Skips task 5 (locked by alice)

Task 7: Build login endpoint

scud claim 7 --name bob
scud set-status 7 in-progress

# Charlie: Third developer
cd project
scud use-tag epic-1-auth
scud next

Task 9: Add email verification

scud claim 9 --name charlie
scud set-status 9 in-progress

# Team Lead: Check progress
scud whois

# Shows:
# ● alice - Task 5 (in progress)
# ● bob - Task 7 (in progress)
# ● charlie - Task 9 (in progress)

scud stats  # Overall epic progress
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
- Shown in `scud whois` output
- Can be force-released

**Auto-Release:**
```bash
scud set-status 5 done    # Auto-releases lock when done
```

### Epic Groups

**Aggregate Stats:**
- See total progress across all epics in group
- Identify bottlenecks
- Balance workload

**Coordinated Planning:**
- Architect can see all epics in group
- Ensure API contracts match
- Share dependencies

---

## File Structure

```
.taskmaster/
├── tasks/
│   └── tasks.json          # Tasks with assigned_to, locked_by
├── workflow-state.json     # Workflow state
└── epic-groups.json        # Epic groups (NEW)
```

### epic-groups.json
```json
{
  "groups": [
    {
      "id": "user-authentication",
      "name": "User Authentication",
      "epic_tags": [
        "epic-1-auth-backend",
        "epic-1-auth-frontend"
      ],
      "description": "Complete user auth system",
      "created_at": "2025-01-15T10:00:00Z",
      "status": "active"
    }
  ]
}
```

### Task with Assignment (tasks.json)
```json
{
  "id": "5",
  "title": "Build registration endpoint",
  "status": "in-progress",
  "assigned_to": "alice",
  "locked_by": "alice",
  "locked_at": "2025-01-15T14:30:00Z"
}
```

---

## Use Cases

### Use Case 1: Backend/Frontend Teams

```
Project: E-commerce checkout

Epic Group: "Checkout Flow"
- epic-3-checkout-backend (Cart API, Payment API)
- epic-3-checkout-frontend (Cart UI, Payment UI)

Team Backend: 2 devs
Team Frontend: 2 devs

Workflow:
1. PM creates single PRD
2. SM creates 2 epic files (backend, frontend)
3. Create group linking both
4. Architect designs both (API contracts)
5. Teams work in parallel
6. Monitor with group-status
```

### Use Case 2: Distributed Team

```
Project: User management system

Epic: "User CRUD"
Developers:
- Alice (US, timezone UTC-8)
- Bob (Europe, timezone UTC+1)
- Charlie (Asia, timezone UTC+9)

Workflow:
1. All work on same epic
2. Each developer claims tasks
3. Use scud whois to avoid conflicts
4. Work asynchronously across timezones
5. Lock prevents accidental overlaps
```

### Use Case 3: Multiple Worktrees

```
Project: Mobile + Web app

Epic Group: "Dashboard Feature"
- epic-4-dashboard-web
- epic-4-dashboard-mobile

Setup:
git worktree add ../project-web epic-4-dashboard-web
git worktree add ../project-mobile epic-4-dashboard-mobile

Developer workflow:
# Terminal 1 (Web)
cd ../project-web
scud use-tag epic-4-dashboard-web
/tm-dev

# Terminal 2 (Mobile)
cd ../project-mobile
scud use-tag epic-4-dashboard-mobile
/tm-dev

# Monitor both
scud group-status dashboard-feature
```

---

## Best Practices

### Epic Groups

✅ **Do:**
- Group epics that share context (API contracts, data models)
- Coordinate architecture phase across all epics in group
- Use group-status for overall progress monitoring
- Keep groups focused (2-4 epics max)

❌ **Don't:**
- Create groups for unrelated epics
- Skip architecture coordination
- Ignore API contract mismatches
- Make huge groups (>5 epics)

### Task Assignment

✅ **Do:**
- Claim tasks before starting work
- Release tasks if you step away
- Use `scud next` to find available tasks
- Check `scud whois` before claiming
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

Perfect companion to epic groups!

### Setup
```bash
# Main repo
cd my-project
scud create-group "Feature X" --epics epic-x-backend,epic-x-frontend

# Create worktrees
git worktree add ../my-project-backend
git worktree add ../my-project-frontend

# Backend worktree
cd ../my-project-backend
git checkout -b feature-x-backend
scud use-tag epic-x-backend
/tm-dev  # Work on backend

# Frontend worktree
cd ../my-project-frontend
git checkout -b feature-x-frontend
scud use-tag epic-x-frontend
/tm-dev  # Work on frontend

# Monitor from anywhere
scud group-status feature-x
```

### Benefits
- Separate file trees
- No constant branch switching
- Parallel builds/tests
- IDE can run both
- Each worktree has own epic

---

## Limitations & Future

### Current Limitations

- **No real-time sync** - Tasks are locked in local files, not server-side
- **Manual conflict resolution** - If two devs claim same task offline
- **No notifications** - Won't alert when someone claims your task
- **Single active epic** - Each worktree can only have one active epic

### Planned Enhancements

- [ ] Lock server for real-time coordination
- [ ] Task notifications/webhooks
- [ ] Multi-epic view (work on multiple epics simultaneously)
- [ ] Cross-epic dependencies (task in epic A depends on task in epic B)
- [ ] Assignment rotation suggestions
- [ ] Workload balancing
- [ ] Time tracking integration
- [ ] Slack/Discord integration for whois

---

## Troubleshooting

### "Task is locked by someone else"

```bash
# Check who has it
scud whois

# If they're done, ask them to release
# Or force release if stale
scud release <task-id> --force
```

### "Epic not found in group"

```bash
# List all groups
scud list-groups

# Add epic to group
scud add-to-group <group-id> <epic-tag>
```

### "Stale locks everywhere"

```bash
# See all assignments
scud whois

# Release stale locks
scud release <task-id> --force

# Or clean all (future feature)
# scud clean-locks --older-than 24h
```

---

## Summary

**Epic Groups:**
- Coordinate related epics
- Aggregate progress
- Perfect for backend/frontend splits
- Use with git worktrees

**Task Assignment:**
- Claim tasks to show you're working
- Lock prevents conflicts
- Auto-release on completion
- Monitor with `scud whois`

**Together:**
Enable teams to work in parallel efficiently while maintaining coordination and preventing conflicts.

**Experimental Status:**
These features are stable but marked experimental. Feedback welcome!

---

**Quick Reference:**

```bash
# Epic Groups
scud create-group "Name" --epics tag1,tag2
scud list-groups
scud group-status <group-id>
scud add-to-group <group-id> <epic-tag>

# Task Assignment
scud assign <task-id> <assignee>
scud claim <task-id> --name <your-name>
scud release <task-id> [--force]
scud whois
```

**Happy parallel development! 🚀**
