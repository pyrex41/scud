# SCUD Complete Guide

**Sprint Cycle Unified Development**

A lightweight, AI-powered workflow orchestration system for software development that combines task management with intelligent agent assistance.

---

## Table of Contents

1. [What is SCUD?](#what-is-scud)
2. [Core Concepts](#core-concepts)
3. [Installation & Setup](#installation--setup)
4. [Complete Workflow](#complete-workflow)
5. [Commands Reference](#commands-reference)
6. [Agent Guide](#agent-guide)
7. [Task Management](#task-management)
8. [Examples](#examples)
9. [Best Practices](#best-practices)
10. [Troubleshooting](#troubleshooting)

---

## What is SCUD?

SCUD (Sprint Cycle Unified Development) is a structured workflow system that guides you through software development projects using a proven 5-phase approach:

```
┌─────────────┐
│  Ideation   │  Define what to build (PRD)
└──────┬──────┘
       ↓
┌─────────────┐
│  Planning   │  Break into tasks
└──────┬──────┘
       ↓
┌─────────────┐
│Architecture │  Design technical solution
└──────┬──────┘
       ↓
┌─────────────┐
│Implementation│  Execute tasks
└──────┬──────┘
       ↓
┌─────────────┐
│Retrospective│  Learn and improve
└─────────────┘
```

### Key Features

✅ **AI-Powered** - Leverages Claude for PRD parsing, complexity analysis, and task breakdown
✅ **Structured Workflow** - 5-phase process ensures nothing is missed
✅ **Fast** - Rust-based CLI with 50x faster performance
✅ **Flexible** - Works with any tech stack, language, or framework
✅ **Trackable** - Complete history and metrics for every epic
✅ **Integrated** - Works seamlessly with Claude Code or any AI assistant

### Why SCUD?

Traditional development often lacks structure:
- ❌ Requirements are vague or missing
- ❌ Tasks are poorly defined
- ❌ Dependencies are unclear
- ❌ Progress is hard to track
- ❌ Lessons are forgotten

SCUD solves this by:
- ✅ Forcing clear requirements (PRD)
- ✅ Breaking work into manageable tasks
- ✅ Mapping dependencies automatically
- ✅ Tracking progress with metrics
- ✅ Capturing learnings in retrospectives

---

## Core Concepts

### Epic

An **epic** is a cohesive feature or set of related functionality. Examples:
- User authentication system
- Shopping cart functionality
- Admin dashboard
- Payment processing

Each epic has:
- Unique tag (e.g., `epic-1-auth`)
- Set of tasks
- Workflow state

### Task

A **task** is a discrete unit of work. Each task has:

```json
{
  "id": "1",
  "title": "Create User model",
  "description": "Implement User model with fields...",
  "status": "pending",
  "complexity": 3,
  "priority": "high",
  "dependencies": [],
  "details": "Technical implementation details...",
  "test_strategy": "How to test this task..."
}
```

**Task Statuses:**
- `pending` - Not started
- `in-progress` - Currently being worked on
- `done` - Completed
- `review` - Awaiting review
- `blocked` - Blocked by dependencies or issues
- `deferred` - Postponed to later
- `cancelled` - Not doing this

**Complexity Scale (Fibonacci):**
- `1` - Trivial (~30 min) - Update config, fix typo
- `2` - Simple (30m-1h) - Add basic validation
- `3` - Moderate (1-2h) - Create new API endpoint
- `5` - Complex (2-4h) - Integrate third-party service
- `8` - Very Complex (4-8h) - Build feature with multiple components
- `13` - Extremely Complex (1 day) - **SHOULD BE SPLIT**
- `21` - Too Large - **MUST BE SPLIT**

### Workflow Phases

SCUD enforces a sequential 5-phase workflow:

1. **Ideation** - Define the product (create PRD)
2. **Planning** - Break PRD into epics and tasks
3. **Architecture** - Design technical solution
4. **Implementation** - Build the tasks
5. **Retrospective** - Review and learn

Each phase must complete before moving to the next.

### Agents

SCUD uses **slash commands** to activate AI agents for each phase:

- `/tm-pm` - Product Manager (Ideation & Planning)
- `/tm-sm` - Scrum Master (Planning & Task Breakdown)
- `/tm-architect` - Architect (Technical Design)
- `/tm-dev` - Developer (Implementation)
- `/tm-retrospective` - Facilitator (Retrospective)
- `/status` - Status Reporter (anytime)

---

## Installation & Setup

### Prerequisites

- Node.js 16+ (for the wrapper)
- Rust & Cargo (for building the CLI)
- Anthropic API key (for AI commands)

### Installation

```bash
# Clone or install SCUD
npm install -g @eyaltoledano/scud

# Or for local development
git clone https://github.com/yourusername/scud
cd scud
npm install
npm link

# Set up API key for AI features
export ANTHROPIC_API_KEY=sk-ant-...
```

### Initialize a Project

```bash
# Navigate to your project directory
cd my-project

# Initialize SCUD
scud init

# This creates:
# .taskmaster/
#   ├── tasks/tasks.json
#   └── workflow-state.json
# docs/
#   ├── prd/
#   ├── epics/
#   ├── architecture/
#   └── retrospectives/
# .claude/commands/  (if using Claude Code)
```

---

## Complete Workflow

### Phase 1: Ideation (Product Manager)

**Goal:** Define what you're building with a Product Requirements Document (PRD)

**Command:** `/tm-pm` (in Claude Code)

**Process:**
1. Agent asks discovery questions:
   - What problem are you solving?
   - Who are the users?
   - What are the key features?
   - What's out of scope?
2. Creates PRD: `docs/prd/[product-name]-prd.md`
3. Updates workflow to `planning` phase

**Example PRD Structure:**
```markdown
# Authentication System PRD

## Problem Statement
Users need secure access to the application.

## Goals
- Enable user registration
- Provide secure login
- Support password reset

## Target Users
- End users signing up for accounts
- Returning users logging in

## Features
1. Email/password registration
2. Login with session management
3. Password reset flow

## Out of Scope
- OAuth/social login (future)
- Multi-factor auth (future)
```

**Tips:**
- Be specific about requirements
- Define clear success criteria
- Explicitly state what's NOT included
- Consider user experience

---

### Phase 2: Planning (Product Manager + Scrum Master)

**Goal:** Break the PRD into epics and tasks

#### Part A: Create Epics (Product Manager)

**Command:** `/tm-pm` (again, but in planning mode)

**Process:**
1. Reviews PRD
2. Breaks into logical epics
3. Creates epic files: `docs/epics/[epic-name].md`

**Example Epic:**
```markdown
# Epic 1: User Registration

## User Stories
- As a new user, I want to sign up with email/password
- As a new user, I want email verification
- As a new user, I want clear error messages

## Acceptance Criteria
- Email must be valid format
- Password must meet security requirements (8+ chars, etc.)
- Duplicate emails are rejected
- Verification email is sent

## Technical Considerations
- Need User model in database
- Need email service integration
- Need password hashing
```

#### Part B: Parse into Tasks (Scrum Master)

**Command:** `scud parse-prd docs/epics/epic-1-registration.md --tag epic-1-reg`

Or use `/tm-sm` in Claude Code

**What Happens:**
1. AI reads epic markdown
2. Extracts discrete tasks
3. Assigns initial complexity scores
4. Identifies dependencies
5. Creates tasks in `.taskmaster/tasks/tasks.json`

**Review and Refine:**
```bash
# List tasks
scud list

# Review specific task
scud show 1

# Analyze complexity with AI
scud analyze-complexity

# Expand tasks >13 complexity
scud expand --all
```

**Scrum Master Agent (`/tm-sm`) will:**
- Review all tasks
- Adjust complexity scores
- Break down large tasks (>13)
- Map dependencies
- Set priorities

**Example Task Breakdown:**

Before expansion:
```
Task 3: Implement user authentication [21]
```

After expansion:
```
Task 3: [PARENT] Implement user authentication
Task 8: Create User model [3]
Task 9: Add password hashing [3]
Task 10: Build registration endpoint [5]
Task 11: Build login endpoint [5]
Task 12: Implement session management [5]
Task 13: Add authentication middleware [3]
```

---

### Phase 3: Architecture (Architect)

**Goal:** Design the technical solution

**Command:** `/tm-architect`

**Process:**
1. Reviews all tasks in epic
2. Creates architecture document: `docs/architecture/[epic-tag]-architecture.md`
3. Adds technical details to each task:
   - Implementation approach
   - Files to create/modify
   - Libraries/frameworks needed
   - Data structures
   - API contracts
4. Sets task dependencies based on technical requirements
5. Validates no task has complexity >13

**Architecture Document Example:**
```markdown
# Epic 1: Registration - Architecture

## System Design

### Database Schema
```sql
CREATE TABLE users (
  id UUID PRIMARY KEY,
  email VARCHAR(255) UNIQUE NOT NULL,
  password_hash VARCHAR(255) NOT NULL,
  email_verified BOOLEAN DEFAULT FALSE,
  created_at TIMESTAMP DEFAULT NOW()
);
```

### API Endpoints
- POST /api/auth/register
- POST /api/auth/verify-email
- POST /api/auth/login

### Components
- UserModel (models/User.js)
- AuthService (services/auth.js)
- PasswordUtils (utils/password.js)
- EmailService (services/email.js)

### Tech Stack
- Node.js + Express
- PostgreSQL
- bcrypt for password hashing
- jsonwebtoken for sessions
- nodemailer for emails
```

**Task Enhancement Example:**

Before:
```json
{
  "id": "8",
  "title": "Create User model",
  "description": "Implement User model",
  "complexity": 3
}
```

After Architect:
```json
{
  "id": "8",
  "title": "Create User model",
  "description": "Implement User model with validation",
  "complexity": 3,
  "details": "Create models/User.js with:\n- Schema: id, email, password_hash, email_verified, created_at\n- Validation: email format, password requirements\n- Methods: comparePassword(), generateToken()\n- Use Sequelize ORM\n- Migration: migrations/001-create-users.js",
  "test_strategy": "Unit tests:\n- Valid user creation\n- Email validation\n- Password hashing\n- Duplicate email rejection"
}
```

**Architect Updates Workflow:**
- Marks architecture phase complete
- Advances to `implementation` phase

---

### Phase 4: Implementation (Developer)

**Goal:** Execute the tasks

**Command:** `/tm-dev`

**Process:**

1. **Start Development Session**
   ```bash
   # Developer agent checks:
   # - Workflow phase is 'implementation'
   # - Architecture is complete
   # - Active epic is set
   ```

2. **Find Next Task**
   ```bash
   scud next

   # Returns:
   # Next Available Task:
   #
   # ID:          8
   # Title:       Create User model
   # Complexity:  3
   # Priority:    high
   #
   # Description: Implement User model with validation
   #
   # Technical Details:
   # Create models/User.js with:
   # - Schema: id, email, password_hash...
   #
   # Test Strategy:
   # Unit tests for validation...
   ```

3. **Start Task**
   ```bash
   scud set-status 8 in-progress
   ```

4. **Implement Task**
   - Developer agent implements the code
   - Follows technical details
   - Ensures test strategy is followed

5. **Test Task**
   - Must write and run tests
   - All tests must pass
   - Agent validates this before marking done

6. **Complete Task**
   ```bash
   scud set-status 8 done
   ```

7. **Repeat** until all tasks are done

**Developer Agent Behavior:**

```markdown
## Developer Agent Rules

1. ALWAYS check workflow phase = implementation
2. ALWAYS use `scud next` to find tasks
3. ALWAYS validate dependencies are met
4. MUST write tests (enforced by test_strategy)
5. MUST run tests before marking done
6. CANNOT mark task done if tests fail
7. Shows progress regularly
```

**Example Developer Session:**

```
User: /tm-dev

Agent: Checking workflow status...
       Phase: implementation ✓
       Active epic: epic-1-reg

       Finding next task...

       Next Task: #8 - Create User model
       Complexity: 3 | Priority: high

       I'll start working on this task.

       Setting status to in-progress...

       Creating models/User.js...
       [Implements code following details]

       Writing tests...
       [Implements tests following test_strategy]

       Running tests...
       ✓ All tests pass

       Marking task as done.

       Progress: 1/10 tasks complete (10%)
```

**Track Progress:**
```bash
# See all tasks and their status
scud list

# See statistics
scud stats

# Epic Statistics: epic-1-reg
# =================
# Total Tasks:      10
# Pending:          5
# In Progress:      1
# Done:             4
# Blocked:          0
#
# Total Complexity: 35
# Completion:       40%
#
# [████████████████████                              ]
```

---

### Phase 5: Retrospective (Facilitator)

**Goal:** Review the epic and capture learnings

**Command:** `/tm-retrospective`

**Triggers:**
- All tasks must be in `done` status
- Implementation phase is complete

**Process:**

1. **Gather Metrics**
   - Tasks completed
   - Total complexity
   - Time taken (if tracked)
   - Blockers encountered

2. **Reflection Questions**
   Agent asks:
   - What went well?
   - What could be improved?
   - What was surprising?
   - What did you learn?
   - What would you do differently?

3. **Create Retrospective Document**
   `docs/retrospectives/[epic-tag]-retrospective.md`

**Example Retrospective:**
```markdown
# Epic 1: User Registration - Retrospective

## Metrics
- **Tasks Completed:** 10
- **Total Complexity:** 35 points
- **Duration:** 3 days
- **Tasks Expanded:** 2 (complexity >13)

## What Went Well
- Clear architecture document saved time
- Test-first approach caught bugs early
- Password validation prevented security issues
- Email service integration was smooth

## What Could Be Improved
- Initial complexity estimates were too low
- Task 3 should have been split from the start
- Needed better error handling patterns
- Should have mocked email service for tests

## Learnings
- bcrypt rounds: 10 is good balance of security/speed
- JWT expiry: 24h for web, 30d for mobile
- Email verification: 24h expiry on tokens
- Rate limiting: needed on auth endpoints

## Action Items for Next Epic
- [ ] Create error handling utilities first
- [ ] Set up test mocking infrastructure
- [ ] Be more aggressive splitting complex tasks
- [ ] Add rate limiting to architecture phase

## Pattern Library Additions
- Auth middleware pattern
- Password validation regex
- JWT token generation
- Email template structure
```

4. **Archive Epic**
   - Moves epic to `completed_epics` in workflow-state.json
   - Resets to `ideation` phase for next epic

5. **Ready for Next Epic**
   ```
   ✅ Epic 1 completed!

   Next steps:
   1. Run /tm-pm to start next epic
   2. Or review metrics and patterns
   ```

---

## Commands Reference

### Setup Commands

#### `scud init`
Initialize SCUD in current directory.

```bash
scud init
```

Creates:
- `.taskmaster/` directory structure
- `docs/` directories
- `workflow-state.json`
- `.gitignore` entry

#### `scud status`
Show current workflow status.

```bash
scud status
```

Shows:
- Current workflow phase
- Active epic
- Available slash commands
- Next steps

---

### Task Management Commands

#### `scud tags`
List all epic tags.

```bash
scud tags

# Output:
# Epic Tags:
#   ● epic-1-reg (10 tasks)    # Active
#   ○ epic-2-login (5 tasks)
```

#### `scud use-tag <tag>`
Switch to a different epic.

```bash
scud use-tag epic-2-login

# Output:
# ✓ Active epic set to: epic-2-login
#   Tasks: 5
#   Pending: 5
#   In Progress: 0
#   Done: 0
```

#### `scud list [--status <status>]`
List tasks in active epic.

```bash
# All tasks
scud list

# Output:
# Tasks in epic: epic-1-reg
#
# 1    done            Create User model [3]
# 2    done            Add password hashing [3]
# 3    in-progress     Build registration endpoint [5]
# 4    pending         Build login endpoint [5]
# 5    pending         Implement session management [5]

# Filter by status
scud list --status pending
scud list --status done
scud list --status in-progress
```

#### `scud show <id>`
Show detailed task information.

```bash
scud show 3

# Output:
# Task Details
# =============
# ID:                  3
# Title:               Build registration endpoint
# Status:              in-progress
# Complexity:          5
# Priority:            high
# Dependencies:        [1, 2]
#
# Description:
# Create POST /api/auth/register endpoint...
#
# Technical Details:
# - Route: routes/auth.js
# - Controller: controllers/authController.js
# - Validation: joi schema
# - Response: 201 + JWT token
#
# Test Strategy:
# - Valid registration
# - Duplicate email
# - Invalid email format
# - Weak password
```

#### `scud set-status <id> <status>`
Update task status.

```bash
scud set-status 3 in-progress
scud set-status 3 done
scud set-status 3 blocked

# Valid statuses:
# pending, in-progress, done, review, blocked, deferred, cancelled
```

#### `scud next`
Find next available task with dependencies met.

```bash
scud next

# Finds first pending task where all dependencies are done
# Shows full task details
# Suggests: scud set-status <id> in-progress
```

#### `scud stats`
Show epic statistics.

```bash
scud stats

# Output:
# Epic Statistics: epic-1-reg
# =================
#
# Total Tasks:      10
# Pending:          3
# In Progress:      1
# Done:             6
# Blocked:          0
#
# Total Complexity: 35
# Completion:       60%
#
# [██████████████████████████████                    ]
```

---

### AI-Powered Commands

Require `ANTHROPIC_API_KEY` environment variable.

#### `scud parse-prd <file> --tag <tag>`
Parse PRD/epic markdown into tasks.

```bash
scud parse-prd docs/epics/user-registration.md --tag epic-1-reg

# What it does:
# 1. Reads markdown file
# 2. Sends to Claude for parsing
# 3. Creates structured tasks
# 4. Assigns complexity scores
# 5. Identifies dependencies
# 6. Sets active epic
```

**Example Input** (`docs/epics/user-registration.md`):
```markdown
# User Registration

## Features
- Email/password signup
- Email verification
- Password strength validation

## Requirements
- Unique emails
- Secure password storage
- Send verification email
```

**Example Output:**
```
✓ Parsed 5 tasks

✅ Epic parsed and created successfully!

Tag:                 epic-1-reg
Tasks created:       5

Next steps:
  1. Review tasks: scud list
  2. Analyze complexity: scud analyze-complexity
  3. Use /tm-architect to add technical details
```

#### `scud analyze-complexity [--task <id>]`
Analyze and score task complexity.

```bash
# Analyze all tasks
scud analyze-complexity

# Analyze specific task
scud analyze-complexity --task 5

# What it does:
# 1. Examines task title, description, details
# 2. Considers technical difficulty, unknowns, testing
# 3. Assigns Fibonacci complexity score
# 4. Provides reasoning
# 5. Flags tasks >13 for expansion
```

**Example Output:**
```
Analyzing complexity for 10 task(s)...

✓ Task 1: Create User model → complexity 3
✓ Task 2: Add password hashing → complexity 2
✓ Task 3: Build registration endpoint → complexity 5
⚠ Task 4: Implement full auth system → complexity 21
  ⚠ Task complexity >13. Consider running: scud expand 4
...

✅ Complexity analysis complete!

Total complexity:     45

⚠ 1 task(s) with complexity >13:
  4 Implement full auth system [21]

Run: scud expand --all
```

#### `scud expand <id>` or `scud expand --all`
Break down complex tasks into subtasks.

```bash
# Expand specific task
scud expand 4

# Expand all tasks with complexity >13
scud expand --all

# What it does:
# 1. Identifies tasks with complexity >13
# 2. Sends to Claude for breakdown
# 3. Creates subtasks (each ≤8 complexity)
# 4. Sets dependencies between subtasks
# 5. Marks original as [PARENT]
```

**Example:**

Before:
```
4    pending    Implement full auth system [21]
```

After:
```
4    pending    [PARENT] Implement full auth system [21]
11   pending    Create User model [3]
12   pending    Add password hashing [3]
13   pending    Build registration endpoint [5]
14   pending    Build login endpoint [5]
15   pending    Add session management [5]
```

#### `scud research "<query>"`
AI-powered topic research.

```bash
scud research "OAuth 2.0 best practices"
scud research "PostgreSQL connection pooling"
scud research "JWT vs session tokens"

# What it does:
# 1. Sends query to Claude
# 2. Gets comprehensive response
# 3. Displays formatted result
# 4. Includes best practices, pitfalls, examples
```

**Example Output:**
```
Research Results
================
Query: OAuth 2.0 best practices

## Key Concepts

OAuth 2.0 is an authorization framework that enables applications
to obtain limited access to user accounts...

## Best Practices

1. **Always use HTTPS** - OAuth requires secure communication
2. **Validate redirect URIs** - Prevent authorization code interception
3. **Use PKCE** - Proof Key for Code Exchange for mobile apps
4. **Short-lived access tokens** - 1 hour or less
5. **Rotate refresh tokens** - Issue new refresh token on each use

## Common Pitfalls

- ❌ Storing tokens in localStorage (use httpOnly cookies)
- ❌ Not validating state parameter (CSRF vulnerability)
- ❌ Using implicit flow (deprecated, use auth code + PKCE)

## Code Example

```javascript
// Authorization request
const authUrl = `${OAUTH_ENDPOINT}/authorize?
  client_id=${CLIENT_ID}&
  redirect_uri=${REDIRECT_URI}&
  response_type=code&
  scope=read:user&
  state=${randomState}`;
```

## Resources
- RFC 6749: OAuth 2.0 Framework
- OAuth 2.0 Security Best Current Practice
```

---

## Agent Guide

### When to Use Each Agent

| Agent | Phase | Command | Purpose |
|-------|-------|---------|---------|
| **Product Manager** | Ideation | `/tm-pm` | Create PRD, define product |
| **Product Manager** | Planning | `/tm-pm` | Break PRD into epics |
| **Scrum Master** | Planning | `/tm-sm` | Parse epics into tasks, manage breakdown |
| **Architect** | Architecture | `/tm-architect` | Design technical solution |
| **Developer** | Implementation | `/tm-dev` | Execute tasks |
| **Facilitator** | Retrospective | `/tm-retrospective` | Review and learn |
| **Status Reporter** | Any | `/status` | Check current state |

### Agent Behaviors

#### Product Manager (`/tm-pm`)

**Ideation Phase:**
- Asks discovery questions
- Creates comprehensive PRD
- Defines scope and out-of-scope
- Identifies user personas
- Sets success criteria

**Planning Phase:**
- Reviews PRD
- Breaks into epics (3-7 is ideal)
- Creates epic markdown files
- Ensures each epic is cohesive

**Key Principle:** Clear requirements prevent rework

#### Scrum Master (`/tm-sm`)

**Responsibilities:**
- Parse epics into discrete tasks
- Analyze task complexity
- Break down tasks >13 complexity
- Map task dependencies
- Ensure all tasks are actionable
- Balance workload

**Complexity Guidelines:**
- 1-8: Good to go
- 13: Should split (full day)
- 21: Must split (too large)

**Dependency Rules:**
- Foundational tasks first (models, schemas)
- Then business logic
- Then API/UI layers
- Finally tests and docs

**Key Principle:** Small, testable tasks reduce risk

#### Architect (`/tm-architect`)

**Responsibilities:**
- Create architecture document
- Design system components
- Choose technology stack
- Define data structures
- Specify API contracts
- Add technical details to ALL tasks
- Set technical dependencies
- Validate task complexity

**Architecture Document Sections:**
- System design overview
- Database schema
- API endpoints
- Component structure
- Tech stack decisions
- Security considerations
- Performance considerations

**Task Enhancement:**
- Implementation approach
- Files to create/modify
- Libraries/dependencies needed
- Data structures
- API/interface contracts

**Key Principle:** Good architecture prevents technical debt

#### Developer (`/tm-dev`)

**Responsibilities:**
- Execute tasks in order
- Follow technical details
- Implement test strategy
- Validate dependencies
- Track progress
- Cannot skip tests

**Workflow:**
1. Check phase = implementation
2. Find next task (`scud next`)
3. Validate dependencies met
4. Set status to in-progress
5. Implement following details
6. Write tests per test_strategy
7. Run all tests
8. Mark done (only if tests pass)
9. Repeat

**Hard Rules:**
- ✅ Must follow technical details
- ✅ Must write tests
- ✅ Must run tests
- ✅ Tests must pass before done
- ❌ Cannot mark done without tests
- ❌ Cannot skip dependencies

**Key Principle:** Test-driven implementation ensures quality

#### Facilitator (`/tm-retrospective`)

**Responsibilities:**
- Gather metrics
- Facilitate reflection
- Capture learnings
- Document patterns
- Create action items
- Archive epic

**Reflection Areas:**
- What went well
- What could improve
- Surprises/learnings
- Technical insights
- Process improvements

**Deliverables:**
- Retrospective document
- Pattern library updates
- Action items for next epic
- Archived epic with metrics

**Key Principle:** Continuous improvement through reflection

---

## Task Management

### Task Lifecycle

```
┌─────────┐
│ pending │  Created, not started
└────┬────┘
     │ scud set-status X in-progress
     ↓
┌─────────────┐
│ in-progress │  Currently being worked on
└─────┬───────┘
      │ scud set-status X done
      ↓
┌──────┐
│ done │  Completed and tested
└──────┘
```

**Alternative Paths:**
- `pending` → `blocked` (dependencies not met, issues)
- `in-progress` → `review` (needs code review)
- `pending` → `deferred` (postponed)
- `pending` → `cancelled` (not doing)

### Dependencies

**How They Work:**
```json
{
  "id": "5",
  "title": "Build login endpoint",
  "dependencies": ["1", "2", "3"]
}
```

Task 5 cannot start until tasks 1, 2, and 3 are `done`.

**Finding Next Task:**
```bash
scud next
```

Returns first `pending` task with all dependencies `done`.

**Dependency Graph Example:**
```
Task 1: Create User model
  ↓
Task 2: Add password hashing (depends on 1)
  ↓
Task 3: Build registration endpoint (depends on 1, 2)
  ↓
Task 4: Build login endpoint (depends on 1, 2)
  ↓
Task 5: Add auth middleware (depends on 3, 4)
```

### Complexity Management

**Fibonacci Scale:**
- 1, 2, 3, 5, 8, 13, 21

**Rules:**
- Tasks ≤8: Good to implement
- Tasks = 13: Should split (1 full day)
- Tasks ≥21: Must split (too large)

**Splitting Process:**
```bash
# Identify large tasks
scud analyze-complexity

# Split them
scud expand --all

# Or specific task
scud expand 7
```

**Example Split:**

Original (21 points):
```
Implement OAuth integration
- Set up OAuth provider
- Handle authorization flow
- Store tokens securely
- Refresh token logic
- Error handling
```

Split into subtasks (total: 22 points):
```
1. Configure OAuth provider settings [3]
2. Build authorization redirect endpoint [5]
3. Implement callback handler [5]
4. Add token storage [3]
5. Implement token refresh [5]
6. Add comprehensive error handling [3]
```

Each subtask ≤8 points = manageable

### Priority Management

**Priority Levels:**
- `high` - Critical path, blockers
- `medium` - Normal work
- `low` - Nice to have, improvements

**Not a Substitute for Dependencies:**
- Use dependencies for technical order
- Use priority for business importance

**Example:**
```json
{
  "id": "10",
  "title": "Add rate limiting",
  "priority": "high",      // Important for security
  "dependencies": ["3", "4"], // But must wait for endpoints
  "complexity": 3
}
```

---

## Examples

### Example 1: Building a Todo App

#### Step 1: Initialize
```bash
cd todo-app
scud init
```

#### Step 2: Create PRD
```
Use: /tm-pm

Agent creates: docs/prd/todo-app-prd.md

# Todo App PRD

## Goals
Simple task management app for personal use

## Features
- Create/read/update/delete todos
- Mark complete/incomplete
- Filter by status
- Persist to database

## Out of Scope
- User accounts (v2)
- Sharing (v2)
- Mobile app (v2)
```

#### Step 3: Break into Epics
```
Use: /tm-pm (planning mode)

Agent creates:
- docs/epics/epic-1-backend.md
- docs/epics/epic-2-frontend.md
```

#### Step 4: Parse First Epic
```bash
scud parse-prd docs/epics/epic-1-backend.md --tag epic-1-backend
```

Creates tasks:
```
1. Set up Express server [2]
2. Create Todo model [3]
3. Build CRUD API endpoints [5]
4. Add validation [2]
5. Set up database [3]
6. Add error handling [2]
```

#### Step 5: Analyze Complexity
```bash
scud analyze-complexity
```

All tasks ≤5, good to proceed.

#### Step 6: Architecture
```
Use: /tm-architect

Creates: docs/architecture/epic-1-backend-architecture.md

Adds technical details to all tasks:
- Task 2 gets schema definition
- Task 3 gets API contract
- etc.
```

#### Step 7: Implementation
```
Use: /tm-dev

Agent workflow:
1. scud next → Task 5 (no dependencies)
2. Set up PostgreSQL database
3. Write tests
4. Mark done

5. scud next → Task 1 (depends on 5, now done)
6. Create Express server
7. Write tests
8. Mark done

... continues for all tasks ...
```

#### Step 8: Retrospective
```
Use: /tm-retrospective

Creates: docs/retrospectives/epic-1-backend-retrospective.md

Learnings:
- Validation library choice was good
- Should have added logging earlier
- Test setup took longer than expected
```

#### Step 9: Second Epic
```
Use: /tm-pm to start epic-2-frontend
```

### Example 2: Adding Authentication to Existing App

#### Starting State
App exists, adding auth feature.

#### Step 1: Initialize SCUD
```bash
scud init
```

#### Step 2: Create Auth PRD
```
Use: /tm-pm

Focus: Just authentication feature

PRD covers:
- Login/logout
- JWT tokens
- Password hashing
- Protected routes
```

#### Step 3: Single Epic
```
Use: /tm-pm (planning)

Creates: docs/epics/epic-1-auth.md
```

#### Step 4: Parse and Refine
```bash
scud parse-prd docs/epics/epic-1-auth.md --tag epic-1-auth

scud list
# Review tasks

scud analyze-complexity
# Task 7 is complexity 21!

scud expand 7
# Splits into 5 subtasks
```

#### Step 5: Architecture
```
Use: /tm-architect

Decisions:
- bcrypt for hashing
- jsonwebtoken for JWTs
- Express middleware for auth
- Add auth field to User model

Updates all tasks with implementation details
```

#### Step 6: Implement
```
Use: /tm-dev

20 tasks implemented over 2 days
All with tests
Progress tracked with scud stats
```

#### Step 7: Learn
```
Use: /tm-retrospective

Key learning: Rate limiting needed
Action: Add to next epic
```

---

## Best Practices

### 1. PRD Quality

✅ **Do:**
- Be specific about requirements
- Define clear success criteria
- Explicitly state what's out of scope
- Include user personas
- Describe user flows

❌ **Don't:**
- Be vague ("make it better")
- Mix features from different areas
- Skip the "why"
- Forget edge cases

### 2. Epic Sizing

✅ **Do:**
- Keep epics cohesive (one feature area)
- Aim for 3-7 epics per project
- Size for 1-2 weeks of work
- Ensure epics can be independent

❌ **Don't:**
- Create massive epics (>50 tasks)
- Mix unrelated features
- Create dependencies between epics
- Make epics too granular

### 3. Task Breakdown

✅ **Do:**
- Keep tasks ≤8 complexity
- Make tasks independently testable
- Map clear dependencies
- Write descriptive titles
- Include acceptance criteria

❌ **Don't:**
- Create huge tasks (>13 complexity)
- Make tasks dependent on everything
- Write vague descriptions
- Skip test strategy
- Ignore dependencies

### 4. Architecture Phase

✅ **Do:**
- Create comprehensive architecture doc
- Add technical details to EVERY task
- Specify files to create/modify
- Define data structures
- Document API contracts
- Consider security/performance

❌ **Don't:**
- Skip architecture ("we'll figure it out")
- Leave tasks without details
- Ignore technical dependencies
- Forget about testing approach
- Rush through this phase

### 5. Implementation

✅ **Do:**
- Always use `scud next`
- Validate dependencies
- Follow technical details
- Write tests FIRST
- Run tests before marking done
- Track progress regularly

❌ **Don't:**
- Pick random tasks
- Ignore dependencies
- Skip tests
- Mark done without testing
- Deviate from architecture
- Work on multiple tasks simultaneously

### 6. Retrospectives

✅ **Do:**
- Be honest about what went wrong
- Capture specific learnings
- Document patterns discovered
- Create actionable improvements
- Review metrics

❌ **Don't:**
- Skip retrospectives
- Only focus on positives
- Make vague observations
- Forget to act on learnings
- Ignore the data

### 7. Workflow Discipline

✅ **Do:**
- Complete each phase before moving on
- Follow the agent sequence
- Trust the process
- Update task status promptly
- Keep documentation current

❌ **Don't:**
- Skip phases
- Jump around randomly
- Ignore phase gates
- Let task status get stale
- Forget to update docs

---

## Troubleshooting

### "Task file not found"

**Problem:** `.taskmaster/tasks/tasks.json` doesn't exist

**Solution:**
```bash
scud init
```

### "No active epic"

**Problem:** Haven't set which epic to work on

**Solution:**
```bash
# List available epics
scud tags

# Set active epic
scud use-tag epic-1-auth
```

### "Wrong phase for this command"

**Problem:** Trying to use an agent in the wrong phase

**Solution:**
```bash
# Check current phase
scud status

# Use correct agent:
# ideation → /tm-pm
# planning → /tm-pm then /tm-sm
# architecture → /tm-architect
# implementation → /tm-dev
# retrospective → /tm-retrospective
```

### "Dependencies not met"

**Problem:** Trying to start a task that depends on incomplete tasks

**Solution:**
```bash
# See task dependencies
scud show <task-id>

# Use next to find available tasks
scud next

# Complete dependencies first
```

### "ANTHROPIC_API_KEY not set"

**Problem:** AI commands need API key

**Solution:**
```bash
export ANTHROPIC_API_KEY=sk-ant-...

# Or add to ~/.bashrc or ~/.zshrc:
echo 'export ANTHROPIC_API_KEY=sk-ant-...' >> ~/.bashrc
```

### "Rust binary not found"

**Problem:** Rust CLI not built

**Solution:**
```bash
cd scud-cli
cargo build --release

# Or it will auto-build on first use
```

### "Task complexity too high"

**Problem:** Tasks >13 should be split

**Solution:**
```bash
# Analyze to find them
scud analyze-complexity

# Expand large tasks
scud expand --all

# Or specific task
scud expand <task-id>
```

### "No tasks need expansion"

**Message:** All tasks ≤13 complexity

**This is good!** Proceed with architecture phase.

---

## Advanced Topics

### Multi-Epic Projects

**Scenario:** Large project with multiple feature areas

**Approach:**
1. Create comprehensive PRD covering entire project
2. Break into multiple epics (one per feature area)
3. Parse each epic separately with different tags:
   ```bash
   scud parse-prd docs/epics/auth.md --tag epic-1-auth
   scud parse-prd docs/epics/payments.md --tag epic-2-pay
   scud parse-prd docs/epics/admin.md --tag epic-3-admin
   ```
4. Work on one epic at a time
5. Switch between epics:
   ```bash
   scud use-tag epic-2-pay
   ```

### Custom Workflow

**SCUD is flexible.** You can adapt it:

**Option 1: Skip Architecture**
- Not recommended, but possible
- Manually update workflow-state.json phase
- Add technical details yourself in task descriptions

**Option 2: Combined Phases**
- Do Planning + Architecture together
- Have Architect also refine tasks

**Option 3: Iterative**
- Do Ideation → Planning → Arch → Impl → Retro for each epic
- Treat each epic as mini-project

**Best Practice:** Follow the standard workflow first. Customize only after you understand it.

### Integration with Existing Tools

**GitHub Issues:**
- Export tasks to issues
- Link SCUD tasks to issue numbers
- Use SCUD for planning, GitHub for tracking

**Jira:**
- Similar to GitHub integration
- SCUD for structured breakdown
- Jira for team visibility

**Linear:**
- SCUD tasks → Linear tasks
- Maintain sync

### Team Usage

**SCUD is designed for solo or small teams.**

**For Teams:**
- One person runs SCUD agents
- Export tasks to team tools
- Use retrospectives for team learning
- Share architecture docs

**Not Recommended:**
- Multiple people editing tasks.json simultaneously
- Different team members in different phases

---

## FAQ

### Q: Do I need to use all 5 phases?
**A:** Yes, for best results. Each phase builds on the previous. Skipping phases leads to problems.

### Q: Can I use SCUD with any programming language?
**A:** Yes! SCUD is language-agnostic. It works with any tech stack.

### Q: How long should each phase take?
**A:** Varies by project size:
- Ideation: 30min - 2hr
- Planning: 1-4hr
- Architecture: 2-8hr
- Implementation: Days to weeks
- Retrospective: 30min - 1hr

### Q: What if requirements change mid-epic?
**A:** Options:
1. Finish current epic, create new epic for changes
2. Cancel current epic, start over (if early)
3. Add deferred tasks for later

**Best Practice:** Scope epics small enough that changes are rare.

### Q: Can I work on multiple epics simultaneously?
**A:** Technically yes (use `scud use-tag`), but not recommended. Focus on one epic at a time.

### Q: Do I need Claude Code to use SCUD?
**A:** No! SCUD works with:
- Claude Code (best experience)
- Any AI assistant (manually paste agent prompts)
- No AI (manually create tasks)

### Q: Is SCUD free?
**A:** Yes, SCUD is open source. You only pay for Anthropic API usage (for AI features).

### Q: What about tasks that aren't in any epic?
**A:** Everything should be in an epic. Create a "Maintenance" or "Misc" epic if needed.

### Q: How do I handle bugs found during implementation?
**A:** Add them as tasks in current epic:
```bash
# Manually add to tasks.json, or
# Create mini-epic for bugs if many
```

---

## Summary

SCUD provides a structured, AI-powered workflow for software development:

1. **Define** what to build (PRD)
2. **Plan** the work (epics & tasks)
3. **Design** the solution (architecture)
4. **Build** the features (implementation)
5. **Learn** from the experience (retrospective)

Each phase has dedicated AI agents to guide you. The Rust CLI provides fast, efficient task management.

**Key Principles:**
- 📋 Clear requirements prevent rework
- 🧩 Small tasks reduce risk
- 🏗️ Good architecture prevents technical debt
- 🧪 Test-driven implementation ensures quality
- 📈 Continuous improvement through reflection

**Start Simple:**
```bash
scud init
# Use /tm-pm
# Follow the workflow
# Trust the process
```

**Questions?**
- Check documentation: `README.md`, `DETAILED_WALKTHROUGH.md`
- Review examples in this guide
- Check troubleshooting section

**Happy Building! 🚀**
