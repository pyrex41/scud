# BMAD-TM Lite: Complete Detailed Walkthrough

**A comprehensive, example-driven guide to understanding and using the entire BMAD-TM Lite system**

This document walks you through every aspect of BMAD-TM Lite with real examples, showing exactly how agents work, how validation happens, and how the workflow progresses.

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Initial Setup](#initial-setup)
3. [Understanding the Workflow State](#understanding-the-workflow-state)
4. [The /status Command Explained](#the-status-command-explained)
5. [Phase 1: Ideation (Product Manager)](#phase-1-ideation-product-manager)
6. [Phase 2: Planning (Product Manager)](#phase-2-planning-product-manager)
7. [Phase 3: Architecture (Architect)](#phase-3-architecture-architect)
8. [Phase 4: Implementation (Developer)](#phase-4-implementation-developer)
9. [Phase 5: Retrospective](#phase-5-retrospective)
10. [Validation & Enforcement in Action](#validation--enforcement-in-action)
11. [Agent Deep Dive](#agent-deep-dive)
12. [Complete Example: Building a Todo App](#complete-example-building-a-todo-app)
13. [Troubleshooting Real Scenarios](#troubleshooting-real-scenarios)

---

## System Overview

### What Is BMAD-TM Lite?

BMAD-TM Lite is a **workflow orchestration system** that guides you through building software epics using:

1. **Task Master** - CLI tool for task state management (`.taskmaster/tasks/tasks.json`)
2. **Workflow State Tracker** - Phase progression tracker (`.taskmaster/workflow-state.json`)
3. **AI Agents** - Markdown documents that define agent personas and workflows
4. **Validator** - JavaScript module that enforces workflow rules
5. **Slash Commands** - Entry points to activate agents

### The Mental Model

Think of it as a **guided assembly line** for building software:

```
You start with an idea
    ↓
PM helps you define it (PRD)
    ↓
PM breaks it into tasks (Task Master)
    ↓
Architect designs how to build it
    ↓
Developer implements task by task
    ↓
Retrospective captures what you learned
    ↓
Loop back for next feature
```

**The system enforces this order** - you can't skip ahead, and you can't start tasks until their prerequisites are done.

### Key Files and Their Roles

| File | Purpose | Who Updates It |
|------|---------|----------------|
| `.taskmaster/tasks/tasks.json` | **Single source of truth** for all tasks | Task Master CLI, Agents guide you |
| `.taskmaster/workflow-state.json` | Current workflow phase and history | Agents (via validator) |
| `docs/prd/*.md` | Product Requirements Documents | PM Agent |
| `docs/architecture/*.md` | Technical design documents | Architect Agent |
| `docs/retrospectives/*.md` | Learning and improvement docs | Retrospective Agent |
| `.claude/commands/*.md` | Agent definitions (slash commands) | You (customization) |
| `src/validators/taskmaster-validator.js` | Validation and enforcement logic | You (customization) |

---

## Initial Setup

### Installation Walkthrough

Let's walk through installation step-by-step:

```bash
# Clone or navigate to your project
cd ~/projects/my-app

# Copy BMAD-TM Lite files to your project
# (Or install as shown in README)

# Run installation
./install-claude-code.sh
```

**What the installer does:**

1. **Checks Task Master CLI:**
   ```
   ✓ Task Master CLI found (v1.2.3)
   ```
   If not found, prompts to install: `npm install -g task-master`

2. **Checks Node.js:**
   ```
   ✓ Node.js found (v18.16.0)
   ```

3. **Initializes Task Master:**
   ```
   ✓ Task Master initialized
   ```
   Creates `.taskmaster/tasks/tasks.json` with `{}`

4. **Creates workflow state:**
   ```
   ✓ Workflow state created
   ```
   Creates `.taskmaster/workflow-state.json` (see below)

5. **Creates directory structure:**
   ```
   ✓ Directory structure created
   ```
   - `docs/prd/`
   - `docs/epics/`
   - `docs/architecture/`
   - `docs/retrospectives/`

6. **Installs slash commands:**
   ```
   ✓ Slash commands installed to ~/.config/claude-code/commands
   ```

7. **Sets up validator:**
   ```
   ✓ Validator made executable
   ```

### Post-Installation Check

```bash
# Verify Task Master
task-master --version
# → task-master version 1.2.3

# Verify validator
node src/validators/taskmaster-validator.js get-command-availability
# → JSON output showing command availability

# Check initial workflow state
cat .taskmaster/workflow-state.json
# → Shows ideation phase active
```

---

## Understanding the Workflow State

### The workflow-state.json File

This file is the **brain** of the workflow orchestration. Let's examine it in detail:

```json
{
  "version": "1.0.0",
  "current_phase": "ideation",
  "active_epic": null,
  "phases": {
    "ideation": {
      "status": "active",
      "completed_at": null,
      "agent": "tm-pm",
      "description": "Product definition and PRD creation"
    },
    "planning": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-pm",
      "description": "Parse PRD into Task Master epics and tasks"
    },
    "architecture": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-architect",
      "description": "Technical design and architecture planning"
    },
    "implementation": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-dev",
      "description": "Task execution and development"
    },
    "retrospective": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-retrospective",
      "description": "Post-epic analysis and learning capture"
    }
  },
  "history": [],
  "completed_epics": [],
  "last_updated": null
}
```

### Field Explanations

**`current_phase`**: The phase you're currently in. Determines which agents can run.
- Valid values: `"ideation"`, `"planning"`, `"architecture"`, `"implementation"`, `"retrospective"`

**`active_epic`**: The epic tag being worked on (e.g., `"epic-1-auth"`).
- `null` when no epic is active
- Set during planning phase
- Used by architect, developer, and retrospective agents

**`phases`**: Details for each phase:
- `status`: `"active"` (current), `"completed"` (done), `"pending"` (not started)
- `completed_at`: ISO timestamp when phase finished
- `agent`: Which agent handles this phase
- `description`: What happens in this phase

**`history`**: Log of all workflow events (chronological)
- Task completions
- Phase transitions
- Epic creation
- Architecture completion

**`completed_epics`**: Archive of finished epics with metadata
- Epic tag
- Completion date
- Task count
- Complexity points
- Duration
- Success rating

**`last_updated`**: Last modification timestamp

### How It Changes During Workflow

**Initial State (After Installation):**
```json
{
  "current_phase": "ideation",
  "active_epic": null,
  "phases": {
    "ideation": { "status": "active" }
    // All others: "pending"
  }
}
```

**After Creating PRD (Ideation Complete):**
```json
{
  "current_phase": "planning",
  "active_epic": null,
  "phases": {
    "ideation": {
      "status": "completed",
      "completed_at": "2025-11-04T10:30:00.000Z"
    },
    "planning": { "status": "active" }
  },
  "last_updated": "2025-11-04T10:30:00.000Z"
}
```

**After Parsing Epic (Planning Complete):**
```json
{
  "current_phase": "architecture",
  "active_epic": "epic-1-todo",
  "phases": {
    "planning": {
      "status": "completed",
      "completed_at": "2025-11-04T10:45:00.000Z"
    },
    "architecture": { "status": "active" }
  },
  "history": [
    {
      "action": "epic_created",
      "epic": "epic-1-todo",
      "timestamp": "2025-11-04T10:45:00.000Z",
      "tasks_count": 6
    }
  ]
}
```

**After Architecture Complete:**
```json
{
  "current_phase": "implementation",
  "active_epic": "epic-1-todo",
  "phases": {
    "architecture": {
      "status": "completed",
      "completed_at": "2025-11-04T11:30:00.000Z",
      "artifacts": [
        "docs/architecture/epic-1-todo-architecture.md"
      ]
    },
    "implementation": { "status": "active" }
  },
  "history": [
    {
      "action": "epic_created",
      "epic": "epic-1-todo",
      "timestamp": "2025-11-04T10:45:00.000Z",
      "tasks_count": 6
    },
    {
      "action": "architecture_complete",
      "epic": "epic-1-todo",
      "timestamp": "2025-11-04T11:30:00.000Z",
      "architect_notes": "All tasks have technical details"
    }
  ]
}
```

**After All Tasks Done:**
```json
{
  "current_phase": "implementation",
  "active_epic": "epic-1-todo",
  // ... (implementation still active until retrospective runs)
  "history": [
    // ... previous entries
    {
      "action": "task_completed",
      "epic": "epic-1-todo",
      "task_id": "1",
      "task_title": "Create Todo model",
      "timestamp": "2025-11-04T12:00:00.000Z",
      "tests_passed": true
    },
    {
      "action": "task_completed",
      "epic": "epic-1-todo",
      "task_id": "2",
      "task_title": "Add CRUD endpoints",
      "timestamp": "2025-11-04T13:00:00.000Z",
      "tests_passed": true
    }
    // ... more task completions
  ]
}
```

**After Retrospective (Reset for Next Epic):**
```json
{
  "current_phase": "ideation",
  "active_epic": null,
  "phases": {
    "ideation": { "status": "active" }
    // All others reset to "pending"
  },
  "completed_epics": [
    {
      "epic_tag": "epic-1-todo",
      "completed_at": "2025-11-04T14:00:00.000Z",
      "total_tasks": 6,
      "complexity_points": 28,
      "duration_days": 3,
      "retrospective_doc": "docs/retrospectives/epic-1-todo-retrospective.md",
      "success_rating": 8.5
    }
  ],
  "history": [
    // ... all previous entries (preserved)
    {
      "action": "retrospective_complete",
      "epic": "epic-1-todo",
      "timestamp": "2025-11-04T14:00:00.000Z"
    }
  ]
}
```

---

## The /status Command Explained

The `/status` command is your **workflow compass** - it tells you where you are, what's available, and what to do next.

### What /status Does

When you run `/status`, Claude:

1. Reads `.taskmaster/workflow-state.json`
2. Reads `.taskmaster/tasks/tasks.json`
3. Calls validator: `taskmaster-validator.js get-command-availability`
4. Displays formatted status

### Example 1: Initial State (Just Installed)

**Command:**
```bash
/status
```

**Output:**
```
🔄 BMAD-TM WORKFLOW STATUS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📍 Current Phase: ideation

  Workflow Progress:
  🔵 Ideation       (tm-pm)         [ACTIVE]
  ⚪ Planning       (tm-pm)         [PENDING]
  ⚪ Architecture   (tm-architect)  [PENDING]
  ⚪ Implementation (tm-dev)        [PENDING]
  ⚪ Retrospective  (tm-retrospective) [PENDING]

🎯 Active Epic: None

  No epic is currently being worked on.

✨ Available Commands:

  /tm-pm          - ✅ Ready to create PRD or parse into Task Master
  /tm-architect   - 🔒 No epic in Task Master - run /tm-pm first
  /tm-dev         - 🔒 No epic in Task Master - complete planning first
  /tm-retrospective - 🔒 No active epic

⚠️ Warnings: None - workflow is healthy ✅

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

💡 Next Steps: Run /tm-pm to create your Product Requirements Document
```

**Key Insights:**
- Only `/tm-pm` is available (phase gate enforcement)
- No warnings (clean state)
- Clear next step provided

### Example 2: After Creating Epic (Architecture Phase)

**Command:**
```bash
/status
```

**Output:**
```
🔄 BMAD-TM WORKFLOW STATUS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📍 Current Phase: architecture

  Workflow Progress:
  🟢 Ideation       (tm-pm)         [COMPLETED] ✓
  🟢 Planning       (tm-pm)         [COMPLETED] ✓
  🔵 Architecture   (tm-architect)  [ACTIVE]
  ⚪ Implementation (tm-dev)        [PENDING]
  ⚪ Retrospective  (tm-retrospective) [PENDING]

🎯 Active Epic: epic-1-todo

  Task Progress:
  ✅ Completed: 0 tasks
  🔄 In Progress: 0 tasks
  ⏸️  Blocked: 0 tasks
  ⏳ Pending: 6 tasks
  ━━━━━━━━━━━━━━
  📊 Total: 6 tasks (28 complexity points)

✨ Available Commands:

  /tm-pm          - 🔒 Only available in ideation/planning phases
  /tm-architect   - ✅ Ready to design architecture
  /tm-dev         - 🔒 Only available in implementation phase (current: architecture)
  /tm-retrospective - 🔒 Epic has 6 incomplete tasks

⚠️ Warnings: None - workflow is healthy ✅

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

💡 Next Steps: Run /tm-architect to design the technical solution
```

**Key Insights:**
- Two phases complete (ideation, planning)
- Active epic with 6 pending tasks
- Only `/tm-architect` available
- Workflow enforcing correct order

### Example 3: During Implementation (Some Tasks Done)

**Command:**
```bash
/status
```

**Output:**
```
🔄 BMAD-TM WORKFLOW STATUS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📍 Current Phase: implementation

  Workflow Progress:
  🟢 Ideation       (tm-pm)         [COMPLETED] ✓
  🟢 Planning       (tm-pm)         [COMPLETED] ✓
  🟢 Architecture   (tm-architect)  [COMPLETED] ✓
  🔵 Implementation (tm-dev)        [ACTIVE]
  ⚪ Retrospective  (tm-retrospective) [PENDING]

🎯 Active Epic: epic-1-todo

  Task Progress:
  ✅ Completed: 3 tasks
  🔄 In Progress: 1 task
  ⏸️  Blocked: 1 task (dependencies not met)
  ⏳ Pending: 1 task
  ━━━━━━━━━━━━━━
  📊 Total: 6 tasks (28 complexity points)

  Progress: ████████████░░░░░░░░ 50%

✨ Available Commands:

  /tm-pm          - 🔒 Only available in ideation/planning phases
  /tm-architect   - 🔒 Only available in architecture phase
  /tm-dev         - ✅ Ready to implement tasks
  /tm-retrospective - 🔒 Epic has 3 incomplete tasks

⚠️ Warnings:

  • Task 5 is blocked by incomplete dependencies (Task 3, Task 4)
  • Task 2 is in-progress for 2 days - check if stuck

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

💡 Next Steps: Run /tm-dev to continue implementing tasks. Focus on completing Task 2, then Task 5 will unblock.
```

**Key Insights:**
- Shows detailed task breakdown
- Visual progress bar
- Warnings about blocked tasks and stale in-progress tasks
- Specific guidance on what to do next

### Example 4: Ready for Retrospective

**Command:**
```bash
/status
```

**Output:**
```
🔄 BMAD-TM WORKFLOW STATUS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📍 Current Phase: implementation

  Workflow Progress:
  🟢 Ideation       (tm-pm)         [COMPLETED] ✓
  🟢 Planning       (tm-pm)         [COMPLETED] ✓
  🟢 Architecture   (tm-architect)  [COMPLETED] ✓
  🔵 Implementation (tm-dev)        [ACTIVE]
  ⚪ Retrospective  (tm-retrospective) [PENDING]

🎯 Active Epic: epic-1-todo

  Task Progress:
  ✅ Completed: 6 tasks
  🔄 In Progress: 0 tasks
  ⏸️  Blocked: 0 tasks
  ⏳ Pending: 0 tasks
  ━━━━━━━━━━━━━━
  📊 Total: 6 tasks (28 complexity points)

  Progress: ████████████████████ 100% 🎉

✨ Available Commands:

  /tm-pm          - 🔒 Only available in ideation/planning phases
  /tm-architect   - 🔒 Only available in architecture phase
  /tm-dev         - ⚠️  No more tasks available
  /tm-retrospective - ✅ All tasks complete - ready for retrospective

⚠️ Warnings: None - all tasks complete! ✅

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

💡 Next Steps: 🎉 EPIC COMPLETE! Run /tm-retrospective to capture learnings and prepare for next epic.
```

**Key Insights:**
- All tasks complete
- Celebration message
- Retrospective now available
- Clear transition to next phase

---

## Phase 1: Ideation (Product Manager)

### What Happens in Ideation

The Product Manager agent helps you create a structured **Product Requirements Document (PRD)** that defines:
- What you're building
- Who it's for
- Why it matters
- What's in scope vs out of scope
- How success is measured

### Activating the PM Agent

**Command:**
```bash
/tm-pm
```

**What Claude Does:**

1. **Loads agent persona** from `.claude/commands/tm-pm.md`
2. **Validates phase gate:**
   ```javascript
   validator.validatePhase('tm-pm', ['ideation', 'planning'])
   // current_phase = 'ideation' ✓
   ```
3. **Detects phase** from workflow state: `ideation`
4. **Enters ideation workflow**

### The Ideation Conversation

**PM Agent:**
```
Hello! I'm your Product Manager. I see we're in the ideation phase - let's create a Product Requirements Document for your new feature.

To write a great PRD, I need to understand the product deeply. Let me ask you some discovery questions:

1. **What are you building?** (In one sentence)
```

**You:**
```
A todo list application with user authentication
```

**PM Agent:**
```
Great! A todo app with auth. Let me dig deeper:

2. **Who are the primary users?**
   - End users creating todos?
   - Teams collaborating on tasks?
   - Enterprises with admin dashboards?
```

**You:**
```
Individual end users creating personal todos
```

**PM Agent:**
```
3. **What's the core problem you're solving?**
   - Why do users need another todo app?
   - What's missing from existing solutions?
```

**You:**
```
Existing todo apps are either too complex or lack proper authentication. I want something simple but secure.
```

**PM Agent:**
```
4. **What are the must-have features for v1?**
   - What's the absolute minimum to launch?
```

**You:**
```
- User signup/login with email/password
- Create, read, update, delete todos
- Mark todos as complete/incomplete
- Basic user profile
```

**PM Agent:**
```
5. **What's explicitly OUT of scope for v1?**
   - This helps us avoid scope creep
```

**You:**
```
- No team features
- No recurring tasks
- No mobile apps (web only)
- No integrations (Slack, etc.)
```

**PM Agent:**
```
6. **How will you measure success?**
   - What metrics matter?
```

**You:**
```
- 100 active users in first month
- Users creating average of 10 todos per week
- < 5% bounce rate on signup
```

**PM Agent:**
```
Perfect! I have everything I need. Let me create the PRD...

[Creates PRD document]

✅ PRD created at: docs/prd/todo-app-prd.md

Let me show you the structure:

---

# Product Requirements Document: Simple Secure Todo App

**Date:** 2025-11-04
**Author:** Product Manager
**Version:** 1.0

## Executive Summary

A minimalist todo list application focused on simplicity and security. Targets individual users who want a straightforward way to manage personal tasks without feature bloat, backed by proper authentication.

## Problem Statement

Existing todo applications fall into two camps: overly complex enterprise tools or insecure hobby projects. Users need a middle ground - simple enough for daily use, secure enough to trust with their task data.

## Target Users

**Primary:** Individual professionals and students (ages 20-45) who:
- Manage 10-50 personal tasks at a time
- Value data privacy and security
- Prefer web-based tools (no app installation)
- Need basic task management without learning curve

**Not for:** Enterprise teams, project managers needing advanced features, users requiring mobile-first experience

## Goals & Success Metrics

- **Adoption:** 100 active users in first month
- **Engagement:** Users create average 10+ todos per week
- **Retention:** < 5% bounce rate on signup flow
- **Security:** Zero authentication breaches, passwords properly hashed

## Scope

### In Scope (MVP)
1. **User Authentication**
   - Email/password signup
   - Email/password login
   - Password reset via email
   - Session management
   - Basic user profile

2. **Todo Management**
   - Create todo with title and description
   - View list of all todos
   - Edit todo details
   - Delete todos
   - Mark as complete/incomplete
   - Filter by complete/incomplete status

3. **User Profile**
   - View profile info
   - Update email
   - Update password
   - Delete account

### Out of Scope (Future Versions)
- Team collaboration features
- Recurring tasks / task scheduling
- Mobile native apps (iOS/Android)
- Third-party integrations (Slack, Calendar, etc.)
- File attachments
- Tags or categories
- Task prioritization
- Due dates and reminders

## Epics Overview

### Epic 1: User Authentication
**Goal:** Allow users to securely sign up, log in, and manage their accounts

**User Stories:**
- As a new user, I want to sign up with email/password so I can create an account
- As a returning user, I want to log in with my credentials so I can access my todos
- As a user, I want to reset my password if I forget it
- As a user, I want to update my profile information
- As a user, I want to delete my account if I no longer use the service

**Technical Considerations:**
- Password hashing (bcrypt)
- Email validation
- Session management (JWT or session cookies)
- HTTPS required
- Rate limiting on login attempts

**Success Criteria:**
- Signup flow completes in < 60 seconds
- Login is instant (< 1 second)
- Passwords are never stored in plain text
- Password reset works within 5 minutes

### Epic 2: Todo CRUD Operations
**Goal:** Allow users to create, view, update, and delete their todos

**User Stories:**
- As a user, I want to create a new todo with title and description
- As a user, I want to see all my todos in a list
- As a user, I want to edit a todo's title or description
- As a user, I want to delete a todo I no longer need
- As a user, I want to mark a todo as complete
- As a user, I want to mark a todo as incomplete (undo completion)
- As a user, I want to filter todos by complete/incomplete status

**Technical Considerations:**
- RESTful API design
- Data validation (title required, max lengths)
- User-scoped queries (users only see their own todos)
- Pagination for large todo lists
- Soft delete vs hard delete

**Success Criteria:**
- Todo creation is instant
- List loads in < 1 second
- No user can see another user's todos
- Completed todos are visually distinct

## Dependencies

**External:**
- Email service (SendGrid, AWS SES, etc.) for password resets
- Database (PostgreSQL recommended)
- Hosting platform (Heroku, AWS, etc.)

**Internal:**
- Authentication must be complete before todo features
- User model must exist before todos can be created

## Timeline & Milestones

- **Week 1:** Epic 1 - User Authentication
- **Week 2:** Epic 2 - Todo CRUD
- **Week 3:** Testing, polish, deployment
- **Target Launch:** End of Week 3

## Open Questions

- [ ] Which email service provider?
- [ ] Which hosting platform?
- [ ] Do we need email verification on signup?
- [ ] Should completed todos be auto-archived after X days?

---

Now that we have a clear PRD, we need to break it into epics for Task Master.

I see two main epics:
1. Epic 1: User Authentication
2. Epic 2: Todo CRUD Operations

Let me create the epic files now...

[Creates epic files]

✅ Epic files created:
- docs/epics/epic-1-authentication.md
- docs/epics/epic-2-todo-crud.md

Since this is your first epic, let's start with Epic 1 (Authentication).

🔄 Updating workflow state to 'planning' phase...

✅ Workflow phase updated: ideation → planning

💡 Next Step: Parse Epic 1 into Task Master:

Run this command:
```
task-master parse-prd docs/epics/epic-1-authentication.md --tag=epic-1-auth
```

This will create the epic and tasks in Task Master. After that, run /status to see next steps!
```

### What Just Happened?

1. **PM asked discovery questions** - Thorough requirements gathering
2. **PM created PRD** - Structured document at `docs/prd/todo-app-prd.md`
3. **PM identified epics** - Two logical epic boundaries
4. **PM created epic files** - Ready for parsing into Task Master
5. **PM updated workflow state** - Transitioned from `ideation` → `planning`
6. **PM guided next step** - Told you exactly what command to run

### The PRD File

Let's examine what was created at `docs/prd/todo-app-prd.md`:

**Key Sections:**
- **Executive Summary**: One-paragraph overview
- **Problem Statement**: Why this matters
- **Target Users**: Who it's for (and who it's NOT for)
- **Goals & Success Metrics**: How success is measured
- **Scope**: What's in vs out (prevents scope creep)
- **Epics Overview**: Logical feature groupings
- **Dependencies**: What's needed (external services, internal prerequisites)
- **Timeline**: Rough milestones
- **Open Questions**: Unresolved decisions

**Why This Structure?**
- Forces clear thinking
- Prevents assumptions
- Provides reference during development
- Helps with scope discipline

---

## Phase 2: Planning (Product Manager)

### What Happens in Planning

The PM agent helps you **parse the epic into Task Master tasks**. This converts high-level epic descriptions into concrete, actionable tasks with:
- Task titles
- Descriptions
- Complexity scores
- Dependencies (initial guess, refined by architect)

### Epic File Structure

The PM created `docs/epics/epic-1-authentication.md`:

```markdown
# Epic 1: User Authentication

**Goal:** Allow users to securely sign up, log in, and manage their accounts

**Priority:** High (blocking epic - required before todos can work)

## User Stories

- As a new user, I want to sign up with email/password so I can create an account
- As a returning user, I want to log in with my credentials so I can access my todos
- As a user, I want to reset my password if I forget it
- As a user, I want to update my profile information
- As a user, I want to delete my account if I no longer use the service

## Tasks

### Task 1: Set up User model and database schema
**Description:** Create User model with email, password_hash, created_at, updated_at fields. Set up database migration.
**Complexity:** 3
**Dependencies:** None

### Task 2: Implement password hashing
**Description:** Add bcrypt for secure password hashing and verification. Create utility functions for hash/compare.
**Complexity:** 2
**Dependencies:** Task 1

### Task 3: Build signup endpoint
**Description:** Create POST /api/auth/signup endpoint with email/password validation, duplicate email checking, user creation.
**Complexity:** 5
**Dependencies:** Task 1, Task 2

### Task 4: Build login endpoint
**Description:** Create POST /api/auth/login endpoint with credential verification, session/JWT token generation.
**Complexity:** 5
**Dependencies:** Task 1, Task 2

### Task 5: Implement password reset flow
**Description:** Create forgot password endpoint, generate reset tokens, send reset emails, create reset password endpoint.
**Complexity:** 8
**Dependencies:** Task 1, Task 2, Task 3

### Task 6: Add session management
**Description:** Implement session middleware, protect routes requiring authentication, handle logout.
**Complexity:** 6
**Dependencies:** Task 4

### Task 7: Build profile management endpoints
**Description:** Create GET/PUT /api/profile endpoints for viewing and updating user information.
**Complexity:** 4
**Dependencies:** Task 1, Task 6

### Task 8: Add account deletion
**Description:** Create DELETE /api/profile endpoint with confirmation, cascade delete user data.
**Complexity:** 3
**Dependencies:** Task 7

## Technical Considerations

- Use bcrypt for password hashing (cost factor 10)
- Email validation with regex or validator library
- JWT tokens or session cookies (decide in architecture)
- HTTPS required in production
- Rate limiting on login attempts (10 per minute per IP)
- Email service integration (SendGrid, AWS SES, etc.)

## Success Criteria

- All endpoints return proper HTTP status codes
- Passwords never logged or returned in responses
- Email uniqueness enforced at database level
- Sessions expire after 24 hours of inactivity
- Password reset tokens expire after 1 hour

## Test Strategy

- Unit tests for password hashing utilities
- Integration tests for all API endpoints
- Security tests for common vulnerabilities (SQL injection, XSS)
- Load test login endpoint (1000 req/sec)
```

### Parsing into Task Master

**Command:**
```bash
task-master parse-prd docs/epics/epic-1-authentication.md --tag=epic-1-auth
```

**What Happens:**

1. Task Master reads the epic file
2. Extracts tasks from `## Tasks` section
3. Parses task titles, descriptions, complexity, dependencies
4. Creates epic entry in `.taskmaster/tasks/tasks.json`
5. Validates JSON structure

**Output:**
```
✓ Parsed epic: epic-1-authentication.md
✓ Created epic: epic-1-auth
✓ Added 8 tasks

Task Summary:
  • Task 1: Set up User model and database schema (complexity: 3)
  • Task 2: Implement password hashing (complexity: 2)
  • Task 3: Build signup endpoint (complexity: 5)
  • Task 4: Build login endpoint (complexity: 5)
  • Task 5: Implement password reset flow (complexity: 8)
  • Task 6: Add session management (complexity: 6)
  • Task 7: Build profile management endpoints (complexity: 4)
  • Task 8: Add account deletion (complexity: 3)

Total complexity: 36 points

✓ Saved to: .taskmaster/tasks/tasks.json
```

### The tasks.json File

Let's examine what was created in `.taskmaster/tasks/tasks.json`:

```json
{
  "epic-1-auth": {
    "metadata": {
      "taskCount": 8,
      "created": "2025-11-04T10:45:00.000Z",
      "lastModified": "2025-11-04T10:45:00.000Z"
    },
    "tasks": [
      {
        "id": "1",
        "title": "Set up User model and database schema",
        "description": "Create User model with email, password_hash, created_at, updated_at fields. Set up database migration.",
        "details": "",
        "testStrategy": "",
        "dependencies": [],
        "status": "pending",
        "priority": "high",
        "complexity": 3
      },
      {
        "id": "2",
        "title": "Implement password hashing",
        "description": "Add bcrypt for secure password hashing and verification. Create utility functions for hash/compare.",
        "details": "",
        "testStrategy": "",
        "dependencies": ["1"],
        "status": "pending",
        "priority": "high",
        "complexity": 2
      },
      {
        "id": "3",
        "title": "Build signup endpoint",
        "description": "Create POST /api/auth/signup endpoint with email/password validation, duplicate email checking, user creation.",
        "details": "",
        "testStrategy": "",
        "dependencies": ["1", "2"],
        "status": "pending",
        "priority": "high",
        "complexity": 5
      },
      {
        "id": "4",
        "title": "Build login endpoint",
        "description": "Create POST /api/auth/login endpoint with credential verification, session/JWT token generation.",
        "details": "",
        "testStrategy": "",
        "dependencies": ["1", "2"],
        "status": "pending",
        "priority": "high",
        "complexity": 5
      },
      {
        "id": "5",
        "title": "Implement password reset flow",
        "description": "Create forgot password endpoint, generate reset tokens, send reset emails, create reset password endpoint.",
        "details": "",
        "testStrategy": "",
        "dependencies": ["1", "2", "3"],
        "status": "pending",
        "priority": "medium",
        "complexity": 8
      },
      {
        "id": "6",
        "title": "Add session management",
        "description": "Implement session middleware, protect routes requiring authentication, handle logout.",
        "details": "",
        "testStrategy": "",
        "dependencies": ["4"],
        "status": "pending",
        "priority": "high",
        "complexity": 6
      },
      {
        "id": "7",
        "title": "Build profile management endpoints",
        "description": "Create GET/PUT /api/profile endpoints for viewing and updating user information.",
        "details": "",
        "testStrategy": "",
        "dependencies": ["1", "6"],
        "status": "pending",
        "priority": "medium",
        "complexity": 4
      },
      {
        "id": "8",
        "title": "Add account deletion",
        "description": "Create DELETE /api/profile endpoint with confirmation, cascade delete user data.",
        "details": "",
        "testStrategy": "",
        "dependencies": ["7"],
        "status": "pending",
        "priority": "low",
        "complexity": 3
      }
    ]
  }
}
```

**Key Points:**
- **`details`**: Empty initially, filled by Architect agent
- **`testStrategy`**: Empty initially, filled by Architect agent
- **`dependencies`**: Array of task IDs that must be done first
- **`status`**: All start as `"pending"`
- **`complexity`**: Fibonacci-like scale (1, 2, 3, 5, 8, 13...)

### Continuing the PM Conversation

After you run the parse command:

**You:**
```
[Ran command above, got output]
Done! Epic created.
```

**PM Agent:**
```
✅ Perfect! Epic 'epic-1-auth' is now in Task Master with 8 tasks.

Let me check the state...

[Reads .taskmaster/tasks/tasks.json]

Looks good! All tasks are present with:
  • Task IDs: 1-8
  • Dependencies mapped correctly
  • Total complexity: 36 points

🔄 Updating workflow state...

[Calls validator to update phase]

✅ Workflow phase updated: planning → architecture
✅ Active epic set: epic-1-auth

📊 Current Status:
  • Phase: architecture
  • Active Epic: epic-1-auth (8 pending tasks)
  • Next Agent: tm-architect

💡 Next Step: Run /tm-architect to design the technical solution

The Architect will:
  1. Review your epic and PRD
  2. Make technology decisions
  3. Create detailed architecture document
  4. Add technical implementation details to each task
  5. Refine dependencies based on technical requirements

When you're ready, run: /tm-architect
```

### What Just Happened?

1. **Epic parsed into Task Master** - 8 tasks created
2. **Dependencies captured** - Task relationships defined
3. **Workflow state updated** - `planning` → `architecture`, active epic set
4. **Next step clear** - Run `/tm-architect`

---

## Phase 3: Architecture (Architect)

### What Happens in Architecture

The Architect agent:
1. Reviews the PRD and epic tasks
2. Makes technical decisions (tech stack, patterns, tools)
3. Creates comprehensive architecture document
4. **Enhances each task** with implementation details in `details` field
5. Refines dependencies based on technical requirements
6. Adds test strategy to each task

This phase **transforms vague tasks into actionable implementation plans**.

### Activating the Architect Agent

**Command:**
```bash
/tm-architect
```

**What Claude Does:**

1. **Loads agent persona** from `.claude/commands/tm-architect.md`
2. **Validates phase gate:**
   ```javascript
   validator.validatePhase('tm-architect', ['architecture'])
   // current_phase = 'architecture' ✓
   ```
3. **Validates active epic:**
   ```javascript
   validator.validateActiveEpic()
   // active_epic = 'epic-1-auth' ✓
   ```
4. **Loads epic from Task Master**
5. **Enters architecture workflow**

### The Architecture Conversation

**Architect Agent:**
```
Hello! I'm your Technical Architect. I see we're working on Epic 1: User Authentication.

Let me review what we're building...

[Reads .taskmaster/tasks/tasks.json for epic-1-auth]
[Reads docs/prd/todo-app-prd.md]

I see 8 tasks focused on user authentication:
  • User model and database
  • Password hashing
  • Signup/login endpoints
  • Password reset
  • Session management
  • Profile management
  • Account deletion

Total complexity: 36 points

Before I design the architecture, I need to understand your technical context:

1. **What's your current backend stack?**
   - Language (Node.js, Python, Go, Ruby, etc.)
   - Framework (Express, FastAPI, Rails, etc.)
```

**You:**
```
Node.js with Express
```

**Architect:**
```
Great! Node.js + Express.

2. **What database are you using or planning to use?**
```

**You:**
```
PostgreSQL
```

**Architect:**
```
Perfect, PostgreSQL is a solid choice for this use case.

3. **Do you have any existing authentication code, or are we starting fresh?**
```

**You:**
```
Starting completely fresh
```

**Architect:**
```
Got it. Clean slate.

4. **For sessions, do you prefer:**
   A) JWT tokens (stateless, scales horizontally)
   B) Server-side sessions (stateful, easier to revoke)
```

**You:**
```
JWT tokens
```

**Architect:**
```
JWT it is. Good for scaling.

5. **Email service for password resets - do you have a preference?**
   - SendGrid (generous free tier)
   - AWS SES (cheapest at scale)
   - Nodemailer with SMTP (DIY)
   - None yet / decide later
```

**You:**
```
Let's go with SendGrid for now
```

**Architect:**
```
Excellent. SendGrid is easy to set up.

6. **Expected scale initially?**
   - < 100 users (simple setup, optimize later)
   - 100-1000 users (moderate optimization)
   - 1000+ users (performance-critical from day 1)
```

**You:**
```
Starting small, < 100 users initially
```

**Architect:**
```
Perfect. We can keep it simple and optimize as needed.

Let me design the architecture...

[Thinking about:]
- Database schema
- API structure
- Security considerations
- Error handling
- Testing approach

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ Architecture Design Complete!

I've created a comprehensive architecture document at:
📄 docs/architecture/epic-1-auth-architecture.md

Let me show you the key decisions:

## Technology Stack

**Backend:**
- Node.js 18+ with Express 4.x
- PostgreSQL 15+ for database
- bcrypt for password hashing (cost factor 10)
- jsonwebtoken for JWT generation
- express-validator for input validation
- SendGrid for email delivery

**Security:**
- JWT tokens (24h expiration)
- HTTP-only cookies for token storage
- HTTPS required in production
- Rate limiting: express-rate-limit
- Helmet.js for security headers

## Database Schema

```sql
CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email VARCHAR(255) UNIQUE NOT NULL,
  password_hash VARCHAR(255) NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE password_reset_tokens (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  token VARCHAR(255) UNIQUE NOT NULL,
  expires_at TIMESTAMP NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_reset_tokens_token ON password_reset_tokens(token);
CREATE INDEX idx_reset_tokens_expires ON password_reset_tokens(expires_at);
```

## API Endpoints

- POST /api/auth/signup
- POST /api/auth/login
- POST /api/auth/logout
- POST /api/auth/forgot-password
- POST /api/auth/reset-password
- GET /api/profile (protected)
- PUT /api/profile (protected)
- DELETE /api/profile (protected)

## Key Architectural Decisions

1. **JWT in HTTP-only cookies** - More secure than localStorage
2. **Separate tokens table** - Clean password reset token management
3. **UUID primary keys** - Better for distributed systems, harder to enumerate
4. **Cascade delete** - User deletion automatically cleans up related data
5. **Email index** - Fast login lookups
6. **bcrypt cost 10** - Good balance of security and performance

Now, let me enhance each task in Task Master with detailed implementation guidance...

[Updating .taskmaster/tasks/tasks.json]

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ Enhanced all 8 tasks with technical details!

Let me show you what I added to Task 1 as an example:

**Task 1: Set up User model and database schema**

**Details added:**
```
TECHNICAL DESIGN:

**Approach:** Use node-postgres (pg) with manual migration scripts

**Implementation Steps:**
1. Install dependencies: pg, uuid
2. Create database connection module (db/connection.js)
3. Create migration file: migrations/001_create_users_table.sql
4. Create User model: models/User.js with methods:
   - User.create({ email, password_hash })
   - User.findByEmail(email)
   - User.findById(id)
   - User.update(id, { email, password_hash })
   - User.delete(id)

**Files to Create:**
- db/connection.js (PostgreSQL connection pool)
- migrations/001_create_users_table.sql (CREATE TABLE script)
- migrations/002_create_password_reset_tokens_table.sql
- models/User.js (User model with CRUD methods)
- scripts/migrate.js (Run migrations)

**Database Schema:**
```sql
CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email VARCHAR(255) UNIQUE NOT NULL,
  password_hash VARCHAR(255) NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

**Dependencies:** None (foundational task)

**Testing:**
- Unit tests for User model methods
- Test database connection
- Test migration script runs successfully
- Test email uniqueness constraint

**Risks:**
- Database connection issues (wrong credentials, network)
- Migration script failures (syntax errors)

**Complexity:** 3 (straightforward database setup)
```

**Test Strategy added:**
```
Unit Tests:
- User.create() returns user object with id
- User.findByEmail() returns user or null
- User.findById() returns user or null
- User.update() modifies user fields
- User.delete() removes user from database
- Email uniqueness enforced (duplicate email throws error)

Integration Tests:
- Database connection works
- Migration creates table successfully
- User CRUD operations work end-to-end

Test Tools: Jest, supertest
Test Database: PostgreSQL test instance or Docker container
```

I've added similar detailed guidance for all 8 tasks!

Also updated dependencies based on technical requirements:
  • Task 3 (signup) now depends on Task 1, 2 ✓ (was already correct)
  • Task 6 (sessions) depends on Task 4 ✓ (was already correct)
  • All dependencies are technically sound

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🔄 Updating workflow state...

✅ Workflow phase updated: architecture → implementation

📊 Current Status:
  • Phase: implementation
  • Active Epic: epic-1-auth (8 pending tasks, all with technical details)
  • Architecture Document: docs/architecture/epic-1-auth-architecture.md
  • Next Agent: tm-dev

💡 Next Step: Run /tm-dev to start implementing tasks

The Developer will:
  1. Show you available tasks (dependencies met)
  2. Validate dependencies before each task
  3. Implement following the technical design
  4. Write and run tests
  5. Update Task Master status

When you're ready, run: /tm-dev
```

### The Architecture Document

Let's examine what was created at `docs/architecture/epic-1-auth-architecture.md`:

*(This would be a very long, detailed document with all the sections shown in the slash command documentation. For brevity, showing key highlights)*

```markdown
# Architecture Document: User Authentication

**Epic Tag:** epic-1-auth
**Date:** 2025-11-04
**Architect:** Technical Architect
**Status:** Final

## 1. System Overview

Building a secure authentication system for a todo application using Node.js, Express, and PostgreSQL. The system handles user signup, login, password reset, session management, and profile operations using JWT tokens stored in HTTP-only cookies.

**Architecture Diagram:**
```
┌─────────────┐
│   Client    │
│  (Browser)  │
└──────┬──────┘
       │ HTTPS
       ↓
┌─────────────────────┐
│  Express Server     │
│  ┌────────────────┐ │
│  │ Rate Limiter   │ │
│  └────────────────┘ │
│  ┌────────────────┐ │
│  │ Auth Routes    │ │
│  │ /signup /login │ │
│  └────────────────┘ │
│  ┌────────────────┐ │
│  │ Auth Middleware│ │
│  │ (JWT verify)   │ │
│  └────────────────┘ │
│  ┌────────────────┐ │
│  │ Profile Routes │ │
│  │ (protected)    │ │
│  └────────────────┘ │
└──────────┬──────────┘
           │
           ↓
┌─────────────────────┐      ┌─────────────┐
│   PostgreSQL DB     │      │  SendGrid   │
│  ┌──────────────┐   │      │   (Email)   │
│  │ users table  │   │      └─────────────┘
│  └──────────────┘   │
│  ┌──────────────┐   │
│  │ reset_tokens │   │
│  └──────────────┘   │
└─────────────────────┘
```

## 2. Technology Stack

[Full stack details with rationale for each choice]

## 3. Data Models

[Complete database schema with indexes]

## 4. Component Architecture

[Detailed breakdown of each module]

## 5. Integration Points

[SendGrid configuration, email templates]

## 6. Security Considerations

[Comprehensive security analysis]

## 7. Performance Considerations

[Bottleneck analysis, optimization strategies]

## 8. Testing Strategy

[Unit, integration, security, load testing approach]

## 9. Risks & Mitigation

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Email service downtime | High | Low | Queue failed emails, retry logic |
| JWT secret leaked | Critical | Low | Rotate secrets, short expiration |
| SQL injection | Critical | Low | Parameterized queries, validation |
| Rate limit bypass | Medium | Medium | Multiple layers (IP, user, endpoint) |

## 10. Implementation Plan

### Phase 1: Foundation (Tasks 1-2)
**Tasks:** 1 (User model), 2 (Password hashing)
**Rationale:** Core data layer must exist before any endpoints
**Duration:** 1-2 days

### Phase 2: Core Auth (Tasks 3-4)
**Tasks:** 3 (Signup), 4 (Login)
**Rationale:** Basic auth flow enables testing
**Duration:** 2-3 days

### Phase 3: Advanced Features (Tasks 5-6)
**Tasks:** 5 (Password reset), 6 (Sessions)
**Rationale:** Build on core auth
**Duration:** 3-4 days

### Phase 4: Profile Management (Tasks 7-8)
**Tasks:** 7 (Profile endpoints), 8 (Account deletion)
**Rationale:** Requires session middleware from Phase 3
**Duration:** 1-2 days

**Total Estimated Duration:** 7-11 days
```

### Enhanced Task in Task Master

Let's see what Task 1 looks like now in `.taskmaster/tasks/tasks.json`:

```json
{
  "id": "1",
  "title": "Set up User model and database schema",
  "description": "Create User model with email, password_hash, created_at, updated_at fields. Set up database migration.",
  "details": "TECHNICAL DESIGN:\n\n**Approach:** Use node-postgres (pg) with manual migration scripts\n\n**Implementation Steps:**\n1. Install dependencies: pg, uuid\n2. Create database connection module (db/connection.js)\n3. Create migration file: migrations/001_create_users_table.sql\n4. Create User model: models/User.js with methods:\n   - User.create({ email, password_hash })\n   - User.findByEmail(email)\n   - User.findById(id)\n   - User.update(id, { email, password_hash })\n   - User.delete(id)\n\n**Files to Create:**\n- db/connection.js (PostgreSQL connection pool)\n- migrations/001_create_users_table.sql (CREATE TABLE script)\n- migrations/002_create_password_reset_tokens_table.sql\n- models/User.js (User model with CRUD methods)\n- scripts/migrate.js (Run migrations)\n\n**Database Schema:**\n```sql\nCREATE TABLE users (\n  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n  email VARCHAR(255) UNIQUE NOT NULL,\n  password_hash VARCHAR(255) NOT NULL,\n  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,\n  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP\n);\n```\n\n**Dependencies:** None (foundational task)\n\n**Testing:**\n- Unit tests for User model methods\n- Test database connection\n- Test migration script runs successfully\n- Test email uniqueness constraint\n\n**Risks:**\n- Database connection issues (wrong credentials, network)\n- Migration script failures (syntax errors)\n\n**Complexity:** 3 (straightforward database setup)",
  "testStrategy": "Unit Tests:\n- User.create() returns user object with id\n- User.findByEmail() returns user or null\n- User.findById() returns user or null\n- User.update() modifies user fields\n- User.delete() removes user from database\n- Email uniqueness enforced (duplicate email throws error)\n\nIntegration Tests:\n- Database connection works\n- Migration creates table successfully\n- User CRUD operations work end-to-end\n\nTest Tools: Jest, supertest\nTest Database: PostgreSQL test instance or Docker container",
  "dependencies": [],
  "status": "pending",
  "priority": "high",
  "complexity": 3
}
```

**Notice:**
- **`details`**: Now filled with comprehensive technical guidance
- **`testStrategy`**: Specific tests to write
- Everything a developer needs to implement the task

### What Just Happened?

1. **Architect asked technical questions** - Stack, database, scale
2. **Architect made technical decisions** - JWT, PostgreSQL, SendGrid, etc.
3. **Architect created architecture document** - Comprehensive design
4. **Architect enhanced all tasks** - Added implementation details to `details` field
5. **Architect validated dependencies** - Ensured technical correctness
6. **Architect updated workflow state** - `architecture` → `implementation`

---

## Phase 4: Implementation (Developer)

### What Happens in Implementation

The Developer agent:
1. Shows available tasks (dependencies met)
2. **Validates dependencies** before allowing any task to start
3. Implements code following architecture
4. Writes tests per test strategy
5. Runs tests and verifies they pass
6. **Blocks marking done** if tests fail
7. Updates Task Master status
8. Repeats until all tasks complete

This phase is where **enforcement is strongest**.

### Activating the Developer Agent

**Command:**
```bash
/tm-dev
```

**What Claude Does:**

1. **Loads agent persona** from `.claude/commands/tm-dev.md`
2. **Validates phase gate:**
   ```javascript
   validator.validatePhase('tm-dev', ['implementation'])
   // current_phase = 'implementation' ✓
   ```
3. **Validates active epic:**
   ```javascript
   validator.validateActiveEpic()
   // active_epic = 'epic-1-auth' ✓
   ```
4. **Gets available tasks:**
   ```javascript
   validator.getAvailableTasks('epic-1-auth')
   // Returns tasks with no unmet dependencies
   ```
5. **Enters implementation workflow**

### The Implementation Conversation (Task 1)

**Developer Agent:**
```
Hello! I'm your Senior Developer. Let's implement Epic 1: User Authentication.

Let me check which tasks are ready to start...

[Calls validator to get available tasks]

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 Epic Status: epic-1-auth
Total Tasks: 8
Pending: 8 | In Progress: 0 | Done: 0 | Blocked: 0

**Ready to Start** (no dependencies):
  ✅ Task 1: Set up User model and database schema (complexity: 3, priority: high)
  ✅ Task 2: Implement password hashing (complexity: 2, priority: high)

**Blocked** (dependencies not met):
  ❌ Task 3: Build signup endpoint
     Waiting on: Task 1, Task 2
  ❌ Task 4: Build login endpoint
     Waiting on: Task 1, Task 2
  ❌ Task 5: Implement password reset flow
     Waiting on: Task 1, Task 2, Task 3
  ❌ Task 6: Add session management
     Waiting on: Task 4
  ❌ Task 7: Build profile management endpoints
     Waiting on: Task 1, Task 6
  ❌ Task 8: Add account deletion
     Waiting on: Task 7

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Recommendation:** Start with Task 1 (User model)
Rationale: Foundation for all other tasks, blocks 5 other tasks

Should I start Task 1? (Y/N)
```

**You:**
```
Yes
```

**Developer:**
```
✅ Starting Task 1: Set up User model and database schema

[Updates Task Master status to 'in-progress']
[Reads task details from Task Master]

According to the architecture, I need to:

1. Install dependencies: pg, uuid
2. Create database connection module
3. Create migration scripts
4. Create User model with CRUD methods

Let me implement step by step...

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Step 1: Install Dependencies**

```bash
npm install pg uuid
npm install --save-dev @types/pg
```

[Executes command]
✅ Dependencies installed

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Step 2: Create Database Connection**

Creating: db/connection.js

```javascript
const { Pool } = require('pg');

const pool = new Pool({
  host: process.env.DB_HOST || 'localhost',
  port: process.env.DB_PORT || 5432,
  database: process.env.DB_NAME || 'todo_app',
  user: process.env.DB_USER || 'postgres',
  password: process.env.DB_PASSWORD,
  max: 20, // Maximum number of clients in pool
  idleTimeoutMillis: 30000,
  connectionTimeoutMillis: 2000,
});

// Test connection on startup
pool.query('SELECT NOW()', (err, res) => {
  if (err) {
    console.error('Database connection error:', err);
    process.exit(1);
  }
  console.log('✓ Database connected:', res.rows[0].now);
});

module.exports = pool;
```

[Creates file]
✅ db/connection.js created

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Step 3: Create Migration Scripts**

Creating: migrations/001_create_users_table.sql

```sql
-- Create users table
CREATE TABLE IF NOT EXISTS users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email VARCHAR(255) UNIQUE NOT NULL,
  password_hash VARCHAR(255) NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create index on email for fast lookups
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- Create function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = CURRENT_TIMESTAMP;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger to auto-update updated_at
CREATE TRIGGER update_users_updated_at
  BEFORE UPDATE ON users
  FOR EACH ROW
  EXECUTE FUNCTION update_updated_at_column();
```

[Creates file]
✅ migrations/001_create_users_table.sql created

Creating: migrations/002_create_password_reset_tokens_table.sql

```sql
-- Create password reset tokens table
CREATE TABLE IF NOT EXISTS password_reset_tokens (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token VARCHAR(255) UNIQUE NOT NULL,
  expires_at TIMESTAMP NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for fast lookups
CREATE INDEX IF NOT EXISTS idx_reset_tokens_token ON password_reset_tokens(token);
CREATE INDEX IF NOT EXISTS idx_reset_tokens_user_id ON password_reset_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_reset_tokens_expires ON password_reset_tokens(expires_at);
```

[Creates file]
✅ migrations/002_create_password_reset_tokens_table.sql created

Creating: scripts/migrate.js

```javascript
const fs = require('fs');
const path = require('path');
const pool = require('../db/connection');

async function runMigrations() {
  const migrationsDir = path.join(__dirname, '..', 'migrations');
  const files = fs.readdirSync(migrationsDir).sort();

  console.log('Running migrations...\n');

  for (const file of files) {
    if (!file.endsWith('.sql')) continue;

    console.log(`→ Running ${file}...`);
    const sql = fs.readFileSync(path.join(migrationsDir, file), 'utf8');

    try {
      await pool.query(sql);
      console.log(`✓ ${file} complete\n`);
    } catch (error) {
      console.error(`✗ ${file} failed:`, error.message);
      process.exit(1);
    }
  }

  console.log('✓ All migrations complete!');
  process.exit(0);
}

runMigrations();
```

[Creates file]
✅ scripts/migrate.js created

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Step 4: Create User Model**

Creating: models/User.js

```javascript
const pool = require('../db/connection');

class User {
  /**
   * Create a new user
   * @param {Object} userData - User data
   * @param {string} userData.email - User email
   * @param {string} userData.password_hash - Hashed password
   * @returns {Promise<Object>} Created user object
   */
  static async create({ email, password_hash }) {
    const query = `
      INSERT INTO users (email, password_hash)
      VALUES ($1, $2)
      RETURNING id, email, created_at, updated_at
    `;

    try {
      const result = await pool.query(query, [email, password_hash]);
      return result.rows[0];
    } catch (error) {
      // Check for unique constraint violation (duplicate email)
      if (error.code === '23505') {
        throw new Error('Email already exists');
      }
      throw error;
    }
  }

  /**
   * Find user by email
   * @param {string} email - User email
   * @returns {Promise<Object|null>} User object or null if not found
   */
  static async findByEmail(email) {
    const query = 'SELECT * FROM users WHERE email = $1';
    const result = await pool.query(query, [email]);
    return result.rows[0] || null;
  }

  /**
   * Find user by ID
   * @param {string} id - User UUID
   * @returns {Promise<Object|null>} User object or null if not found
   */
  static async findById(id) {
    const query = 'SELECT * FROM users WHERE id = $1';
    const result = await pool.query(query, [id]);
    return result.rows[0] || null;
  }

  /**
   * Update user fields
   * @param {string} id - User UUID
   * @param {Object} updates - Fields to update
   * @returns {Promise<Object|null>} Updated user object or null if not found
   */
  static async update(id, updates) {
    const allowedFields = ['email', 'password_hash'];
    const fields = [];
    const values = [];
    let paramCount = 1;

    // Build dynamic update query
    Object.keys(updates).forEach(key => {
      if (allowedFields.includes(key)) {
        fields.push(`${key} = $${paramCount}`);
        values.push(updates[key]);
        paramCount++;
      }
    });

    if (fields.length === 0) {
      throw new Error('No valid fields to update');
    }

    values.push(id); // Add ID as last parameter
    const query = `
      UPDATE users
      SET ${fields.join(', ')}
      WHERE id = $${paramCount}
      RETURNING id, email, created_at, updated_at
    `;

    try {
      const result = await pool.query(query, values);
      return result.rows[0] || null;
    } catch (error) {
      if (error.code === '23505') {
        throw new Error('Email already exists');
      }
      throw error;
    }
  }

  /**
   * Delete user
   * @param {string} id - User UUID
   * @returns {Promise<boolean>} True if deleted, false if not found
   */
  static async delete(id) {
    const query = 'DELETE FROM users WHERE id = $1 RETURNING id';
    const result = await pool.query(query, [id]);
    return result.rows.length > 0;
  }
}

module.exports = User;
```

[Creates file]
✅ models/User.js created

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Step 5: Run Migrations**

```bash
node scripts/migrate.js
```

[Executes command]

Output:
```
Running migrations...

→ Running 001_create_users_table.sql...
✓ 001_create_users_table.sql complete

→ Running 002_create_password_reset_tokens_table.sql...
✓ 002_create_password_reset_tokens_table.sql complete

✓ All migrations complete!
```

✅ Database schema created successfully

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Step 6: Write Tests**

According to test strategy, I need to write:
- Unit tests for User model methods
- Integration tests for database operations

Creating: tests/models/User.test.js

```javascript
const User = require('../../models/User');
const pool = require('../../db/connection');

describe('User Model', () => {
  beforeAll(async () => {
    // Clean database before tests
    await pool.query('TRUNCATE users CASCADE');
  });

  afterAll(async () => {
    await pool.end();
  });

  afterEach(async () => {
    // Clean up after each test
    await pool.query('TRUNCATE users CASCADE');
  });

  describe('create()', () => {
    it('should create a new user with valid data', async () => {
      const userData = {
        email: 'test@example.com',
        password_hash: 'hashed_password_123'
      };

      const user = await User.create(userData);

      expect(user).toHaveProperty('id');
      expect(user.email).toBe(userData.email);
      expect(user).not.toHaveProperty('password_hash'); // Should not return password
      expect(user).toHaveProperty('created_at');
      expect(user).toHaveProperty('updated_at');
    });

    it('should throw error for duplicate email', async () => {
      const userData = {
        email: 'duplicate@example.com',
        password_hash: 'hash123'
      };

      await User.create(userData);

      await expect(User.create(userData)).rejects.toThrow('Email already exists');
    });
  });

  describe('findByEmail()', () => {
    it('should find user by email', async () => {
      const userData = {
        email: 'find@example.com',
        password_hash: 'hash123'
      };

      await User.create(userData);
      const found = await User.findByEmail(userData.email);

      expect(found).toBeTruthy();
      expect(found.email).toBe(userData.email);
      expect(found).toHaveProperty('password_hash'); // Full user object
    });

    it('should return null for non-existent email', async () => {
      const found = await User.findByEmail('nonexistent@example.com');
      expect(found).toBeNull();
    });
  });

  describe('findById()', () => {
    it('should find user by ID', async () => {
      const userData = {
        email: 'findid@example.com',
        password_hash: 'hash123'
      };

      const created = await User.create(userData);
      const found = await User.findById(created.id);

      expect(found).toBeTruthy();
      expect(found.id).toBe(created.id);
      expect(found.email).toBe(userData.email);
    });

    it('should return null for non-existent ID', async () => {
      const found = await User.findById('00000000-0000-0000-0000-000000000000');
      expect(found).toBeNull();
    });
  });

  describe('update()', () => {
    it('should update user email', async () => {
      const userData = {
        email: 'original@example.com',
        password_hash: 'hash123'
      };

      const created = await User.create(userData);
      const updated = await User.update(created.id, { email: 'updated@example.com' });

      expect(updated.email).toBe('updated@example.com');
      expect(updated.id).toBe(created.id);
    });

    it('should update user password_hash', async () => {
      const userData = {
        email: 'password@example.com',
        password_hash: 'hash123'
      };

      const created = await User.create(userData);
      const updated = await User.update(created.id, { password_hash: 'newhash456' });

      expect(updated.id).toBe(created.id);

      // Verify password updated
      const found = await User.findById(created.id);
      expect(found.password_hash).toBe('newhash456');
    });

    it('should return null for non-existent user', async () => {
      const updated = await User.update('00000000-0000-0000-0000-000000000000', { email: 'test@example.com' });
      expect(updated).toBeNull();
    });

    it('should throw error for duplicate email on update', async () => {
      await User.create({ email: 'existing@example.com', password_hash: 'hash1' });
      const user2 = await User.create({ email: 'user2@example.com', password_hash: 'hash2' });

      await expect(
        User.update(user2.id, { email: 'existing@example.com' })
      ).rejects.toThrow('Email already exists');
    });
  });

  describe('delete()', () => {
    it('should delete user by ID', async () => {
      const userData = {
        email: 'delete@example.com',
        password_hash: 'hash123'
      };

      const created = await User.create(userData);
      const deleted = await User.delete(created.id);

      expect(deleted).toBe(true);

      // Verify user is gone
      const found = await User.findById(created.id);
      expect(found).toBeNull();
    });

    it('should return false for non-existent user', async () => {
      const deleted = await User.delete('00000000-0000-0000-0000-000000000000');
      expect(deleted).toBe(false);
    });
  });
});
```

[Creates file]
✅ tests/models/User.test.js created

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Step 7: Run Tests**

```bash
npm test tests/models/User.test.js
```

[Executes command]

Output:
```
PASS  tests/models/User.test.js
  User Model
    create()
      ✓ should create a new user with valid data (45ms)
      ✓ should throw error for duplicate email (12ms)
    findByEmail()
      ✓ should find user by email (10ms)
      ✓ should return null for non-existent email (5ms)
    findById()
      ✓ should find user by ID (9ms)
      ✓ should return null for non-existent ID (5ms)
    update()
      ✓ should update user email (15ms)
      ✓ should update user password_hash (14ms)
      ✓ should return null for non-existent user (5ms)
      ✓ should throw error for duplicate email on update (13ms)
    delete()
      ✓ should delete user by ID (11ms)
      ✓ should return false for non-existent user (5ms)

Test Suites: 1 passed, 1 total
Tests:       12 passed, 12 total
Snapshots:   0 total
Time:        2.345s
```

✅ ALL TESTS PASSING! (12/12)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Implementation Complete!**

✅ Database connection module created
✅ Migration scripts created and executed
✅ User model created with all CRUD methods
✅ 12 unit tests written and passing
✅ Email uniqueness enforced
✅ Code follows architecture design

**Files Created:**
- db/connection.js
- migrations/001_create_users_table.sql
- migrations/002_create_password_reset_tokens_table.sql
- scripts/migrate.js
- models/User.js
- tests/models/User.test.js

**Self-Review Checklist:**
- [x] All requirements met per task description
- [x] Code follows architecture document
- [x] Tests written and passing
- [x] Error handling implemented (duplicate email)
- [x] No security issues (parameterized queries)
- [x] Code is readable and well-commented

Ready to mark Task 1 as done!

[Updates Task Master status to 'done']
[Adds history entry to workflow state]

✅ Task 1 marked as DONE

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 Epic Progress:
  ✅ Done: 1 task
  ⏳ Pending: 7 tasks

**Tasks Now Available:**
  ✅ Task 2: Implement password hashing (no dependencies)

**Tasks Now Unblocked:**
  None yet (Task 2 also needed for Tasks 3-4)

Should I continue with Task 2? (Y/N)
```

### The Enforcement in Action

Notice what happened:

1. **Dependency Validation:**
   - Developer showed only Tasks 1 and 2 as available
   - Tasks 3-8 were blocked and showed their dependencies

2. **Implementation Guidance:**
   - Followed architecture `details` field exactly
   - Created all files specified
   - Implemented all methods

3. **Test Enforcement:**
   - Wrote 12 tests per test strategy
   - Ran tests before marking done
   - Would have BLOCKED if tests failed

4. **Status Update:**
   - Marked Task 1 as "done" in Task Master
   - Added history entry to workflow state
   - Showed updated epic progress

5. **Next Steps:**
   - Identified Task 2 as now available
   - Asked if should continue

### Continuing Implementation (Task 2, brief)

**You:**
```
Yes, continue with Task 2
```

**Developer:**
```
✅ Starting Task 2: Implement password hashing

[Implements bcrypt utilities]
[Writes tests]
[Tests pass]
[Marks done]

✅ Task 2 complete!

Now available:
  ✅ Task 3: Build signup endpoint (dependencies met: 1, 2)
  ✅ Task 4: Build login endpoint (dependencies met: 1, 2)

Continue with Task 3? (Y/N)
```

### What If Tests Fail?

Let's simulate a test failure:

**Developer:**
```
✅ Starting Task 3: Build signup endpoint

[Implements signup endpoint]
[Writes tests]
[Runs tests]

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

❌ TESTS FAILED

FAIL  tests/routes/auth.test.js
  POST /api/auth/signup
    ✓ should create user with valid data (45ms)
    ✗ should return 400 for invalid email (15ms)
    ✓ should return 409 for duplicate email (20ms)

  ● POST /api/auth/signup › should return 400 for invalid email

    expect(received).toBe(expected) // Object.is equality

    Expected: 400
    Received: 500

      42 |       .send({ email: 'notanemail', password: 'Password123!' });
      43 |
    > 44 |     expect(response.status).toBe(400);
         |                              ^
      45 |   });

Test Suites: 1 failed, 1 total
Tests:       1 failed, 2 passed, 3 total

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

❌ CANNOT MARK TASK DONE

Tests are failing. I need to fix the code before proceeding.

**Issue:** Email validation is not working correctly - returning 500 instead of 400 for invalid email.

Let me fix the validation...

[Fixes code]
[Reruns tests]

✓ All tests passing now!

✅ Task 3 marked as DONE
```

**Key Point:** Developer agent **blocked marking done** until tests passed. This enforces quality.

### Completing the Epic

After implementing all 8 tasks:

**Developer:**
```
✅ Task 8 complete!

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🎉 EPIC COMPLETE!

All 8 tasks in epic-1-auth are done!

📊 Final Statistics:
  • Total Tasks: 8
  • Completed: 8 (100%)
  • Total Complexity: 36 points
  • Duration: 3 days
  • Tests Written: 87 tests
  • Tests Passing: 87/87 (100%)

**Files Created:**
- db/connection.js
- migrations/ (2 files)
- models/User.js
- routes/auth.js
- middleware/auth.js
- utils/email.js
- tests/ (6 test files)

**Architecture Compliance:** ✅ 100%
All tasks implemented per architecture document

💡 Next Step: Run /tm-retrospective to capture learnings

Ready to move to retrospective? Run: /tm-retrospective
```

---

## Phase 5: Retrospective

### What Happens in Retrospective

The Retrospective agent:
1. Validates all tasks are complete
2. Gathers epic data (tasks, complexity, duration)
3. Asks reflection questions
4. Analyzes the epic across multiple dimensions
5. Creates comprehensive retrospective document
6. Identifies action items for next epic
7. Resets workflow to ideation for next cycle

### Activating the Retrospective Agent

**Command:**
```bash
/tm-retrospective
```

**What Claude Does:**

1. **Loads agent persona** from `.claude/commands/tm-retrospective.md`
2. **Validates epic complete:**
   ```javascript
   validator.validateEpicComplete('epic-1-auth')
   // All 8 tasks status = 'done' ✓
   ```
3. **Gathers data** from Task Master, workflow state, git history
4. **Enters retrospective workflow**

### The Retrospective Conversation

**Retrospective Agent:**
```
Hello! I'm your Retrospective Facilitator. Let's analyze Epic 1: User Authentication.

[Validates epic complete]
✅ All 8 tasks complete!

[Gathers data]

📊 Epic Summary:
  • Epic: epic-1-auth (User Authentication)
  • Tasks: 8
  • Complexity: 36 points
  • Duration: 3 days (Nov 4 - Nov 7)
  • Tests: 87 passing
  • Files Created: 14
  • Lines of Code: ~1,200

Before I create the retrospective document, I'd like your perspective:

1. **What went well during this epic?**
   - What practices or decisions helped?
   - Any wins to celebrate?
```

**You:**
```
- Architecture phase was thorough - no major surprises during implementation
- Dependency mapping worked perfectly - never got blocked
- Tests caught several bugs early
- Task Master kept everything organized
```

**Retrospective:**
```
Excellent! Those are significant wins.

2. **What was challenging or frustrating?**
   - Any unexpected difficulties?
   - Pain points in the process?
```

**You:**
```
- Task 5 (password reset) took longer than expected - underestimated email integration
- Had to debug JWT token expiration logic - wasn't clear in architecture
- Test database setup was tricky initially
```

**Retrospective:**
```
Good feedback. Those are areas to improve.

3. **Were there any surprises or unexpected issues?**
```

**You:**
```
- SendGrid rate limits in development - had to add retry logic
- PostgreSQL UUID generation was slower than expected on old dev machine
- More edge cases in email validation than anticipated
```

**Retrospective:**
```
4. **How accurate were the task estimates?**
   - Which tasks took longer/shorter than complexity suggested?
```

**You:**
```
- Task 1-4: Pretty accurate
- Task 5: Way underestimated (should have been 13, not 8)
- Task 6-8: Accurate
```

**Retrospective:**
```
5. **Did the architecture hold up during implementation?**
   - Any architecture changes needed?
   - Were technology choices correct?
```

**You:**
```
- Architecture was mostly solid
- Had to add retry logic for emails (not in original design)
- JWT expiration needed clarification
- Otherwise no major changes
```

**Retrospective:**
```
Perfect. Let me analyze the data and create the retrospective document...

[Analyzes epic data]
[Creates comprehensive retrospective]

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ Retrospective Complete!

📄 Document created at: docs/retrospectives/epic-1-auth-retrospective.md

Let me share the key insights:

## Success Rating: 8.5/10

**Strengths:**
- Zero scope creep (all 8 planned tasks, no extras)
- Strong architecture prevented major rework
- Dependency management prevented blockers
- High test coverage (87 tests, 100% passing)
- Clean code following architecture design

**Areas for Improvement:**
- Complexity estimation for external API integrations
- Architecture documentation for token management details
- Development environment setup documentation

## Key Learnings

**Technical:**
1. **Email service rate limits are real** - Always add retry logic and queue for production
2. **JWT expiration edge cases need explicit handling** - Document token lifecycle clearly in architecture
3. **UUID generation performance** - Minor but noticeable on older hardware

**Process:**
1. **Thorough architecture saves time** - Upfront design prevented 2-3 days of rework
2. **Dependency mapping is valuable** - Zero blocking situations throughout epic
3. **Test-first approach caught bugs early** - Found 5 bugs in tests before manual testing

**Estimation:**
1. **External API integration complexity underestimated** - Multiply estimated complexity by 1.5x for external APIs
2. **Email-related tasks always have edge cases** - Invalid formats, bounces, rate limits, etc.

## Action Items for Next Epic

### Do More Of:
- ✅ Thorough architecture phase (invest time upfront)
- ✅ Explicit dependency mapping (prevented all blockers)
- ✅ Test-first development (caught bugs early)

### Start Doing:
- 🆕 Add "External Dependencies" checklist to architecture phase
- 🆕 Document token/session lifecycle explicitly
- 🆕 Estimate external API tasks at 1.5x initial guess
- 🆕 Create dev environment setup guide

### Stop Doing:
- ❌ Assuming external API documentation is accurate (verify first)
- ❌ Skipping edge case analysis in architecture

### Specific Improvements:
1. **Architecture template:** Add "External Service Integration" section
2. **Estimation guidelines:** External API = minimum complexity 8
3. **Testing strategy:** Require mock/stub strategy for external services

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🔄 Updating workflow state...

✅ Epic archived as completed
✅ Workflow phase reset: retrospective → ideation
✅ Active epic cleared (ready for next)

📊 Historical Record:
  • Completed Epics: 1
  • Total Tasks Completed: 8
  • Total Complexity: 36 points
  • Average Success Rating: 8.5/10

🎯 You're ready for your next epic!

When ready, run: /tm-pm to start planning Epic 2
```

### The Retrospective Document

Created at `docs/retrospectives/epic-1-auth-retrospective.md`:

*(Full document would be very long, showing summary)*

```markdown
# Retrospective: User Authentication

**Epic Tag:** epic-1-auth
**Completed:** 2025-11-07
**Duration:** Nov 4 - Nov 7 (3 days)
**Facilitator:** Retrospective Agent

---

## Epic Summary

**Goal:** Allow users to securely sign up, log in, and manage their accounts

**Outcome:** Successfully implemented comprehensive authentication system with 8 tasks completed

**Metrics:**
- Total Tasks: 8
- Completed: 8 (100%)
- Complexity Points: 36
- Duration: 3 days
- Tasks Blocked: 0
- Tests Written: 87
- Tests Passing: 87/87 (100%)

---

## 🌟 What Went Well

### Wins & Successes
- **Architecture prevented rework** - Thorough design phase saved 2-3 days of reimplementation
- **Dependency mapping was perfect** - Zero blocking situations, smooth implementation flow
- **Test-first caught bugs early** - Found 5 bugs in tests before they reached production
- **Clean code throughout** - All code followed architecture guidelines

### Effective Practices
- Detailed task enhancement by Architect (implementation became straightforward)
- Rigorous dependency validation (never started task with unmet dependencies)
- Test-driven development (tests written before/alongside code)

---

## 🔥 What Was Challenging

### Obstacles & Frustrations
- **Task 5 complexity underestimated** - Email integration took 2x longer than expected
- **JWT token edge cases** - Architecture didn't document expiration handling clearly
- **SendGrid rate limits** - Development environment hit rate limits, needed retry logic

### Process Issues
- Test database setup not documented - each developer figured it out separately
- No checklist for external service integration - missed edge cases

---

## 📊 Analysis

### Planning Accuracy

| Aspect | Planned | Actual | Variance | Notes |
|--------|---------|--------|----------|-------|
| Tasks | 8 | 8 | 0% | No scope creep ✅ |
| Complexity | 36 | ~54 | +50% | Task 5 significantly underestimated |
| Duration | 2-3 days | 3 days | On target | Within estimate ✅ |

**Complexity Breakdown:**
- Tasks 1-4: Estimated accurately (within 10%)
- Task 5: Severely underestimated (8 → should have been 13)
- Tasks 6-8: Estimated accurately

**Estimation Insight:** External API integrations (Task 5 with SendGrid) need 1.5-2x multiplier

### Architecture Quality

**What Worked:**
- Database schema proved correct (no changes needed)
- Technology choices appropriate (bcrypt, JWT, PostgreSQL)
- API design was clean and RESTful
- Security considerations comprehensive

**What Didn't:**
- Token lifecycle not explicitly documented (caused confusion)
- Email retry logic not in original design (had to add)
- Rate limiting strategy not detailed enough

**Architecture Score:** 8.5/10

Minor gaps but overall strong design

### Process Efficiency

**Workflow Analysis:**
- PM → Architect → Dev flow worked smoothly ✅
- Clear phase gates prevented rushing ahead ✅
- Task Master enforced discipline ✅

**Bottlenecks:**
- None! Dependency mapping prevented all blocking

**Process Score:** 9/10

Nearly flawless execution

---

## 💡 Key Learnings

[... detailed learnings sections ...]

---

## 🚀 Action Items for Next Epic

[... action items ...]

---

## 📈 Metrics & Trends

**Epic Metrics:**
| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| Tasks Completed | 8/8 | 100% | ✅ |
| Test Coverage | 100% | 80%+ | ✅ |
| Blocked Tasks | 0 | 0 | ✅ |
| Scope Creep | 0% | <10% | ✅ |
| Duration Variance | 0% | ±20% | ✅ |

---

## 🎯 Overall Assessment

**Success Rating:** 8.5/10

**Justification:**
- All tasks completed successfully ✅
- Architecture proved solid (minor adjustments) ✅
- High code quality and test coverage ✅
- Process worked smoothly ✅
- Complexity underestimation for Task 5 ⚠️

**Would We Do This Epic Again?**
Yes, with the estimation improvements identified

**Key Takeaway:**
Thorough architecture and dependency planning pays massive dividends during implementation. The disciplined BMAD-TM workflow prevented the chaos typical of unstructured development.

---

Next Steps:
1. ✅ Retrospective complete
2. ✅ Workflow reset to ideation
3. 🔄 Ready for Epic 2: Todo CRUD Operations
4. 📋 Incorporate learnings into next epic

*Ready to start next epic? Run /tm-pm*
```

### What Just Happened?

1. **Validated epic complete** - All 8 tasks checked
2. **Gathered comprehensive data** - Tasks, complexity, duration, tests
3. **Asked reflection questions** - Got user perspective
4. **Analyzed across dimensions** - Planning accuracy, architecture quality, process efficiency
5. **Identified learnings** - Technical, process, estimation insights
6. **Created action items** - Specific improvements for next epic
7. **Archived epic** - Added to `completed_epics`
8. **Reset workflow** - Back to `ideation` phase

---

*(Continuing in next message due to length...)*

---

## Validation & Enforcement in Action

### Example 1: Phase Gate Blocking

**Scenario:** User tries to run `/tm-dev` before architecture is complete

**What Happens:**

```bash
$ /tm-dev
```

**Agent Response:**
```
❌ PHASE GATE BLOCKED

The Developer agent can only run during the implementation phase.

Current phase: architecture

You need to complete architecture first:
  1. Ensure epic exists in Task Master (/tm-pm)
  2. Complete architecture design (/tm-architect)
  3. Then run /tm-dev

Run /status to see your current workflow state.
```

**Behind the Scenes:**
```javascript
// Validator checks phase
validator.validatePhase('tm-dev', ['implementation'])
// Returns: { valid: false, currentPhase: 'architecture', allowedPhases: ['implementation'] }

// Agent blocks activation
```

**Key Point:** Cannot bypass phase gates - workflow enforces correct order

### Example 2: Dependency Blocking

**Scenario:** User tries to start Task 5 when Task 3 is not done

**What Happens:**

```bash
$ /tm-dev
[Developer shows available tasks]

**You:** "Let's do Task 5"
```

**Agent Response:**
```
❌ DEPENDENCY CHECK FAILED

Task 5: Implement password reset flow

Cannot start this task because dependencies are not complete:
  ✅ Task 1: Set up User model (done)
  ✅ Task 2: Implement password hashing (done)
  ❌ Task 3: Build signup endpoint (in-progress)

Complete Task 3 first, then Task 5 will be available.

Available tasks right now:
  None (Task 3 is in-progress, finish it first)
```

**Behind the Scenes:**
```javascript
// Validator checks dependencies
validator.validateDependencies('epic-1-auth', '5')
// Returns: { 
//   valid: false, 
//   unmetDependencies: [{ id: '3', title: 'Build signup endpoint', status: 'in-progress' }]
// }

// Agent blocks task start
```

**Key Point:** Hard block on unmet dependencies - prevents build order issues

### Example 3: Test Failure Blocking

**Scenario:** Developer tries to mark task done with failing tests

**What Happens:**

```bash
$ /tm-dev
[Working on Task 3]
[Writes code]
[Writes tests]
[Tests fail]

**You:** "Mark it done, I'll fix tests later"
```

**Agent Response:**
```
❌ TESTS FAILED

Cannot mark task done while tests are failing.

Failed tests:
  • should return 400 for invalid email
  • should return 409 for duplicate email

Options:
  1. Fix the code to make tests pass
  2. Fix the tests if they're incorrect
  3. Mark task as "blocked" if there's a deeper issue

Task remains: in-progress

I cannot mark this done until tests pass. This prevents bugs from reaching production.
```

**Key Point:** Agent boundary enforcement - Developer CANNOT mark done without passing tests

### Example 4: Epic Incomplete Blocking

**Scenario:** User tries to run retrospective with incomplete tasks

**What Happens:**

```bash
$ /tm-retrospective
```

**Agent Response:**
```
❌ EPIC NOT COMPLETE

Cannot run retrospective while tasks are incomplete.

Epic: epic-1-auth
Status:
  ✅ Done: 5 tasks
  🔄 In Progress: 1 task
  ⏸️  Blocked: 0 tasks
  ⏳ Pending: 2 tasks

Complete all tasks first, then run /tm-retrospective.

Run /status to see current task states.
```

**Behind the Scenes:**
```javascript
// Validator checks epic completion
validator.validateEpicComplete('epic-1-auth')
// Returns: { 
//   valid: false, 
//   incompleteTasks: [
//     { id: '6', title: 'Add session management', status: 'in-progress' },
//     { id: '7', title: 'Build profile endpoints', status: 'pending' },
//     { id: '8', title: 'Add account deletion', status: 'pending' }
//   ]
// }
```

**Key Point:** Cannot skip to retrospective - all tasks must be complete

---

## Agent Deep Dive

### Agent Anatomy

Each agent is a **markdown file** with:
1. **YAML frontmatter** - Description for command palette
2. **Phase gate validation** - Required phase checks
3. **Persona** - Role, style, principles
4. **Workflow** - Step-by-step process
5. **Task Master integration** - How to update state
6. **Agent boundaries** - Can/cannot/must-validate lists
7. **Error handling** - What to do when things fail

### PM Agent Persona

```markdown
**Role:** Product Manager
**Experience:** 8+ years in product management
**Specialty:** Strategic planning, user research, ruthless prioritization

**Communication Style:**
- Direct and analytical
- Ask probing questions
- Focus on "why" before "what"
- Data-driven decision making
- Challenge assumptions

**Core Principles:**
1. Investigative mindset - understand the real problem
2. User-centric - always ask "who benefits and how?"
3. Scope discipline - say no to nice-to-haves
4. Measurable outcomes - define success criteria
5. Clear documentation - PRDs are contracts
```

**What This Means:**
- PM agent will ask **why** repeatedly (gets to root problems)
- PM challenges feature requests (prevents scope creep)
- PM insists on success metrics (measurable outcomes)
- PM documents thoroughly (PRD as contract)

### Architect Agent Persona

```markdown
**Role:** Technical Architect / Engineering Lead
**Experience:** 10+ years in software architecture
**Specialty:** System design, scalability, technical decision-making

**Communication Style:**
- Technical but clear
- Trade-off focused - every decision has pros/cons
- Risk-aware - identify what could go wrong
- Pragmatic - balance ideal vs practical
- Visual - use diagrams and examples

**Core Principles:**
1. YAGNI (You Aren't Gonna Need It) - don't over-engineer
2. Simplicity First - simple solutions are easier to maintain
3. Document Decisions - explain the "why" behind choices
4. Risk Mitigation - anticipate and plan for failure modes
5. Developer Experience - make implementation easier for the team
```

**What This Means:**
- Architect explains trade-offs (no perfect solutions)
- Architect focuses on simplicity (not over-engineering)
- Architect documents *why* (not just *what*)
- Architect thinks about edge cases and failures
- Architect designs for maintainability

### Developer Agent Persona

```markdown
**Role:** Senior Software Engineer
**Experience:** 7+ years full-stack development
**Specialty:** Clean code, TDD, pragmatic problem-solving

**Communication Style:**
- Code-focused - show, don't just tell
- Test-driven - tests prove correctness
- Incremental - small, working iterations
- Practical - ship working code
- Quality-conscious - correct > fast

**Core Principles:**
1. Tests First - write tests, see them fail, make them pass
2. Dependency Discipline - never start without prerequisites
3. Architecture Adherence - follow the design
4. Working Software - always leave code in runnable state
5. Self-Review - catch issues before they become problems
```

**What This Means:**
- Developer writes tests before/with code (TDD)
- Developer validates dependencies rigorously (no shortcuts)
- Developer follows architecture (no freelancing)
- Developer ensures code always works (no broken state)
- Developer self-reviews before marking done

### Retrospective Agent Persona

```markdown
**Role:** Technical Coach / Agile Facilitator
**Experience:** 10+ years facilitating team retrospectives
**Specialty:** Continuous improvement, data-driven analysis, actionable insights

**Communication Style:**
- Reflective - focus on learning, not blame
- Data-driven - use metrics to support insights
- Action-oriented - every learning becomes an action
- Positive - celebrate wins, frame challenges as opportunities
- Forward-looking - how do we improve next time?

**Core Principles:**
1. Blameless - focus on process, not people
2. Specific - vague insights aren't actionable
3. Balanced - celebrate successes AND identify improvements
4. Actionable - every retrospective produces concrete next steps
5. Honest - surface real issues, even if uncomfortable
```

**What This Means:**
- Retro agent never blames individuals (process focus)
- Retro agent celebrates wins (positive reinforcement)
- Retro agent creates action items (not just observations)
- Retro agent uses data (metrics over feelings)
- Retro agent is honest about issues (constructive criticism)

### How Personas Influence Behavior

**Example: Feature Request**

**PM Response:**
> "Interesting idea. Let me ask: Who specifically would use this feature? Can you quantify the impact? What problem does it solve that existing features don't? What's the opportunity cost of building this vs other priorities?"

*(Investigative, challenging assumptions, data-driven)*

**Architect Response:**
> "I see three ways to implement this: A) Simple but doesn't scale, B) Complex but future-proof, C) Pragmatic middle ground. Given your current scale (< 100 users), I recommend C. We can refactor to B later if needed. Here's the trade-off analysis..."

*(Trade-off focused, pragmatic, scalability-aware)*

**Developer Response:**
> "Based on the architecture doc, I'll implement approach C. I'll write tests for the three core use cases first, then implement to make tests pass. Should take ~4 hours. I'll update you when tests are green."

*(Test-driven, architecture adherence, concrete timeline)*

---

## Complete Example: Building a Todo App

Let's walk through building the Todo app from end to end, showing every phase, every command, and every decision.

### Project Setup

```bash
# Create project
mkdir my-todo-app
cd my-todo-app

# Initialize Node.js project
npm init -y

# Install BMAD-TM Lite
# (Assuming you've copied/cloned the files)
./install-claude-code.sh

# Verify setup
/status
```

**Output:**
```
📍 Current Phase: ideation
🎯 Active Epic: None
💡 Next Steps: Run /tm-pm to create your Product Requirements Document
```

### Epic 1: User Authentication

**Step 1: Create PRD (2 hours)**

```bash
/tm-pm
```

[Full PM conversation as shown earlier]

**Created:**
- `docs/prd/todo-app-prd.md`
- `docs/epics/epic-1-authentication.md`

**Step 2: Parse into Task Master (1 minute)**

```bash
task-master parse-prd docs/epics/epic-1-authentication.md --tag=epic-1-auth
```

**Result:** 8 tasks in Task Master

**Step 3: Design Architecture (3 hours)**

```bash
/tm-architect
```

[Full Architect conversation as shown earlier]

**Created:**
- `docs/architecture/epic-1-auth-architecture.md`

**Enhanced:** All 8 tasks with technical details

**Step 4: Implement Tasks (2-3 days)**

```bash
/tm-dev
```

**Task 1 (2 hours):**
- Create User model
- Write migrations
- Write 12 tests
- All tests pass ✅

**Task 2 (1 hour):**
- Implement bcrypt utilities
- Write 8 tests
- All tests pass ✅

**Task 3 (3 hours):**
- Build signup endpoint
- Write 15 tests
- All tests pass ✅

**Task 4 (3 hours):**
- Build login endpoint
- Write 12 tests
- All tests pass ✅

**Task 5 (8 hours - took longer than expected):**
- Implement password reset
- Integrate SendGrid
- Handle rate limits
- Write 18 tests
- All tests pass ✅

**Task 6 (4 hours):**
- Add session middleware
- Write 10 tests
- All tests pass ✅

**Task 7 (3 hours):**
- Build profile endpoints
- Write 10 tests
- All tests pass ✅

**Task 8 (2 hours):**
- Add account deletion
- Write 2 tests
- All tests pass ✅

**Total Implementation Time:** ~26 hours (3.25 days @ 8hr/day)

**Step 5: Retrospective (1 hour)**

```bash
/tm-retrospective
```

[Full Retrospective conversation as shown earlier]

**Created:**
- `docs/retrospectives/epic-1-auth-retrospective.md`

**Insights:**
- Task 5 underestimated (external API integration)
- Architecture was solid (saved ~2 days of rework)
- Dependency mapping perfect (zero blockers)

### Epic 2: Todo CRUD Operations

**Step 1: Create PRD (Already exists from PM first pass)**

```bash
/tm-pm
```

**PM:**
> I see we already have Epic 2 defined in the original PRD. Let me create the detailed epic file...

**Created:**
- `docs/epics/epic-2-todo-crud.md`

**Step 2: Parse into Task Master**

```bash
task-master parse-prd docs/epics/epic-2-todo-crud.md --tag=epic-2-todos
```

**Result:** 7 tasks

**Step 3: Design Architecture**

```bash
/tm-architect
```

**Architect asks:**
- How will todos be organized? (per-user lists)
- Pagination? (yes, 50 per page)
- Soft delete or hard delete? (soft delete)
- Real-time updates? (no, not v1)

**Created:**
- `docs/architecture/epic-2-todos-architecture.md`

**Step 4: Implement Tasks**

```bash
/tm-dev
```

**Task 1:** Create Todo model (2 hours)
**Task 2:** Add CRUD endpoints (4 hours)
**Task 3:** Implement filtering (2 hours)
**Task 4:** Add pagination (2 hours)
**Task 5:** Create frontend UI (6 hours)
**Task 6:** Add client-side validation (2 hours)
**Task 7:** Write E2E tests (3 hours)

**Total:** ~21 hours (2.6 days)

**Step 5: Retrospective**

```bash
/tm-retrospective
```

**Success Rating:** 9/10 (even better than Epic 1)

**Why Better:**
- Applied learnings from Epic 1
- More accurate estimation (used 1.5x for external APIs)
- Better test organization
- Faster implementation

### Project Complete!

**Total Time:** 6 days
**Total Tasks:** 15
**Total Complexity:** 63 points
**Tests Written:** 157
**Tests Passing:** 157/157 (100%)
**Success Rating:** 8.75/10 average

**Files Created:**
- 25 source files
- 12 test files
- 2 PRDs
- 2 architecture docs
- 2 retrospective docs
- 4 migrations

**What Worked:**
- Structured workflow prevented chaos
- Dependency enforcement prevented blockers
- Test enforcement prevented bugs
- Retrospectives improved each epic
- Clear documentation maintained context

---

## Troubleshooting Real Scenarios

### Scenario 1: "I forgot what phase I'm in"

**Problem:** Lost track of workflow state

**Solution:**
```bash
/status
```

Shows exactly where you are and what's next.

### Scenario 2: "Task is marked in-progress but I want to restart it"

**Problem:** Started task, got stuck, want to restart

**Solution:**
```bash
# Option 1: Update status back to pending
task-master update-status epic-1-auth 5 pending

# Option 2: Just continue - status is just for tracking
# Developer agent will show it as available if you want to work on it
```

### Scenario 3: "I need to add a dependency that wasn't there"

**Problem:** Realized Task 7 actually depends on Task 5

**Solution:**
```bash
task-master set-dependency epic-1-auth 7 5

# Now Developer agent will block Task 7 until Task 5 is done
```

### Scenario 4: "I want to remove a dependency that's wrong"

**Problem:** Task 6 doesn't actually depend on Task 3

**Solution:**
```bash
task-master remove-dependency epic-1-auth 6 3

# Now Task 6 can be started without Task 3
```

### Scenario 5: "Tests are failing but it's a flaky test"

**Problem:** Test fails randomly, not a real bug

**Solution:**
```bash
# Fix the flaky test first!
# Flaky tests are tech debt that will cause problems later

# If it's truly external (e.g., network issue):
# 1. Add retry logic to test
# 2. Mock the external dependency
# 3. Run test in isolation

# Developer agent will not let you mark done until tests pass consistently
```

### Scenario 6: "I want to skip architecture and just code"

**Problem:** Feel like architecture is overkill

**Solution:**
```
Phase gate will block you. This is intentional.

Why architecture matters:
- Prevents 2-3 days of rework (proven in metrics)
- Catches design issues before code
- Provides implementation guidance
- Documents decisions for future

Spend the 2-3 hours on architecture. You'll save 2-3 days later.
```

### Scenario 7: "Task Master is out of sync with actual code"

**Problem:** Marked task done but didn't actually implement it

**Solution:**
```bash
# Update status back to in-progress
task-master update-status epic-1-auth 3 in-progress

# Now implement it properly
# Don't mark done until tests pass

# Task Master is source of truth - keep it honest
```

### Scenario 8: "I want to work on multiple epics at once"

**Problem:** Want to switch between Epic 1 and Epic 2

**Solution:**
```
BMAD-TM Lite tracks one active epic for focus.

Recommended approach:
1. Finish Epic 1 completely
2. Run retrospective
3. Start Epic 2

If you MUST work on multiple:
1. Complete Epic 1 to a stable point
2. Commit all changes
3. Manually update workflow-state.json:
   - Set active_epic to epic-2-todos
   - Set current_phase appropriately
4. Work on Epic 2
5. Switch back by reversing

But this defeats the purpose of focused workflow.
```

### Scenario 9: "Workflow state is corrupted"

**Problem:** JSON file is malformed or wrong

**Solution:**
```bash
# Backup current state
cp .taskmaster/workflow-state.json .taskmaster/workflow-state.json.backup

# Check for JSON errors
cat .taskmaster/workflow-state.json | jq .

# If JSON is invalid, restore from backup or reinstall
./install-claude-code.sh

# If JSON is valid but wrong state, manually edit:
# 1. Set current_phase to correct value
# 2. Set active_epic to correct epic tag
# 3. Update phase statuses

# Worst case: delete and reinstall
rm .taskmaster/workflow-state.json
./install-claude-code.sh
```

### Scenario 10: "Agent is hallucinating / ignoring validation"

**Problem:** Claude is not following agent instructions

**Solution:**
```
This means agent documentation needs to be clearer.

Debug steps:
1. Read the agent file: .claude/commands/tm-dev.md
2. Check if validation logic is clear
3. Add explicit reminder:
   "CRITICAL: MUST validate dependencies before allowing task to start"
4. Test with explicit user instruction:
   "Validate dependencies for task 5 before starting"

If Claude continues to ignore:
1. Report issue (agent prompt may need strengthening)
2. Manually run validator:
   node src/validators/taskmaster-validator.js validate-dependencies epic-1-auth 5
3. Use output to inform Claude
```

---

## Summary

BMAD-TM Lite is a **pragmatic workflow orchestration system** that:

1. **Guides** you through structured phases (ideation → planning → architecture → implementation → retrospective)
2. **Enforces** best practices (phase gates, dependencies, tests)
3. **Maintains** single source of truth (Task Master)
4. **Prevents** common mistakes (skipping architecture, ignoring dependencies, skipping tests)
5. **Captures** learnings (retrospectives improve each epic)

**Key Components:**
- **Workflow State Tracker** - Where you are in the workflow
- **Task Master** - What tasks exist and their status
- **Agents** - How to work in each phase
- **Validator** - What rules are enforced
- **Slash Commands** - How to activate agents

**Success Factors:**
- Trust the phases (each builds on previous)
- Use /status frequently (always know where you are)
- Respect dependencies (prevents blockers)
- Write tests first (catch bugs early)
- Run retrospectives (continuous improvement)

**When to Use:**
- Building software epics (authentication, CRUD, etc.)
- Teams needing workflow discipline
- Projects where quality matters
- Situations where dependencies are complex

**When NOT to Use:**
- Trivial changes (fixing typos, etc.)
- Exploratory coding (proof of concepts)
- Solo hobby projects (overkill for small scope)

---

**Ready to build better software?**

```bash
./install-claude-code.sh
/status
/tm-pm
```

Let the workflow guide you! 🚀

