# BMAD-TM Lite Quick Start Guide

**Get started with BMAD-TM Lite in 5 minutes** ⚡

BMAD-TM Lite is a lightweight workflow orchestration system that guides you through building software epics using Task Master for state management and intelligent agent prompting.

---

## Installation

### For Claude Code CLI

```bash
./install-claude-code.sh
```

This will:
- Check prerequisites (Task Master CLI, Node.js)
- Initialize Task Master
- Create workflow state tracker
- Install slash commands
- Set up validator

### For OpenCode

```bash
./install-opencode.sh
```

This will:
- Check prerequisites (Task Master CLI, Node.js)
- Initialize Task Master
- Create workflow state tracker
- Install skills
- Set up validator

---

## The 5-Minute Tour

### 1. Check Your Status

**Claude Code CLI:**
```
/status
```

**OpenCode:**
```
show status
```

You'll see:
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

✨ Available Commands:
  /tm-pm - Ready to create PRD ✅

💡 Next Steps: Run /tm-pm to create your Product Requirements Document
```

### 2. Create a PRD (Product Manager Phase)

**Claude Code CLI:**
```
/tm-pm
```

**OpenCode:**
```
I need to create a product requirements document
```

The PM agent will:
1. Ask discovery questions about your product
2. Create a structured PRD document
3. Help you break it into epics
4. Parse epics into Task Master

**Example interaction:**
```
PM: What are you building?
You: A user authentication system with OAuth and MFA

PM: Who are the users?
You: End users signing up for our SaaS app

PM: What's the primary goal?
You: Secure, user-friendly authentication that meets SOC2 requirements

[PM creates PRD at docs/prd/auth-system-prd.md]
[PM creates epic file at docs/epics/epic-1-authentication.md]

PM: Great! Now let's parse this into Task Master:

$ task-master parse-prd docs/epics/epic-1-authentication.md --tag=epic-1-auth

[8 tasks created]

✅ Epic created! Run /status to see next steps.
```

### 3. Check Status Again

```
/status
```

Now you'll see:
```
📍 Current Phase: architecture

🎯 Active Epic: epic-1-auth (8 tasks pending)

✨ Available Commands:
  /tm-architect - Ready to design architecture ✅

💡 Next Steps: Run /tm-architect to design the technical solution
```

### 4. Design Architecture (Architect Phase)

**Claude Code CLI:**
```
/tm-architect
```

**OpenCode:**
```
design the architecture
```

The Architect agent will:
1. Read your PRD and Task Master epic
2. Ask technical clarifying questions
3. Create comprehensive architecture document
4. Enhance each task with implementation details
5. Set task dependencies

**Example:**
```
Architect: I see 8 tasks for authentication. Let me ask a few questions:

1. What's your current backend stack?
You: Node.js with Express and PostgreSQL

2. Any existing authentication code?
You: No, starting fresh

3. Expected user volume?
You: 10K users initially, growing to 100K

[Architect analyzes and designs]

Architect: I'll design around:
- Passport.js for authentication strategies
- JWT tokens for sessions
- bcrypt for password hashing
- Google OAuth for social login

[Creates docs/architecture/epic-1-auth-architecture.md]
[Enhances all 8 tasks in Task Master with technical details]
[Sets dependencies: Task 3 depends on Tasks 1, 2]

✅ Architecture complete! Run /status to see next steps.
```

### 5. Implement Tasks (Developer Phase)

**Claude Code CLI:**
```
/tm-dev
```

**OpenCode:**
```
start implementation
```

The Developer agent will:
1. Show available tasks (dependencies met)
2. Validate dependencies before starting any task
3. Implement code following architecture
4. Write and run tests
5. Update Task Master status

**Example:**
```
Developer: Let me check available tasks...

Ready to Start (no dependencies):
  ✅ Task 1: Set up user model (pending)
  ✅ Task 2: Create database migration (pending)

Blocked (dependencies not met):
  ❌ Task 3: Implement OAuth (depends on: 1, 2)
  ❌ Task 4: Add MFA (depends on: 1, 3)
  [... 4 more blocked tasks]

Recommendation: Start with Task 1

Should I start Task 1? (Y/N)
You: Y

[Developer implements User model with validation, tests]
[Updates Task Master status to 'done']

✅ Task 1 complete! This unblocks Task 3.

Next: Task 2 (Create database migration)
Continue? (Y/N)
```

### 6. Complete All Tasks

Continue with the Developer agent until all tasks are done:

```
Task 1: done ✅
Task 2: done ✅
Task 3: done ✅ (unblocked after 1, 2)
Task 4: done ✅ (unblocked after 1, 3)
[...]
Task 8: done ✅

🎉 EPIC COMPLETE! All 8 tasks done.
```

### 7. Run Retrospective

**Claude Code CLI:**
```
/tm-retrospective
```

**OpenCode:**
```
run retrospective
```

The Retrospective agent will:
1. Validate all tasks complete
2. Ask reflection questions
3. Analyze epic metrics
4. Create retrospective document
5. Identify action items
6. Reset workflow for next epic

**Example:**
```
Retrospective: Epic 'epic-1-auth' complete! Let me gather data...

What went well during this epic?
You: Architecture planning prevented major issues. Dependencies worked perfectly.

What was challenging?
You: OAuth redirect URLs were tricky. Underestimated Task 3 complexity.

[Analyzes data: 8 tasks, 45 complexity points, 2.5 weeks duration]

[Creates docs/retrospectives/epic-1-auth-retrospective.md]

✅ Retrospective complete!

Success Rating: 8.5/10

Action Items for Next Epic:
  • Add external dependency checklist to architecture phase
  • Estimate API integration tasks at minimum complexity 7
  • Set up external credentials during architecture phase

Workflow reset to ideation. Ready for next epic!
```

---

## Your First Epic: Step-by-Step

Let's build a simple "User Profile" feature from scratch.

### Step 1: Initialize

```bash
/status
```

Expected: Phase is `ideation`, no active epic.

### Step 2: Create PRD

```bash
/tm-pm
```

Tell the PM:
- **Product:** User profile management
- **Users:** Registered users of the app
- **Goal:** Allow users to view and edit their profile information
- **Scope:**
  - View profile
  - Edit profile (name, email, bio)
  - Upload avatar image
  - Update password

PM will create PRD and epic file.

### Step 3: Parse into Task Master

Run the command provided by PM:
```bash
task-master parse-prd docs/epics/epic-2-user-profile.md --tag=epic-2-profile
```

### Step 4: Design Architecture

```bash
/tm-architect
```

Answer questions:
- **Stack:** Your current stack
- **Storage:** Where to store avatars (S3? local?)
- **Validation:** What rules for bio, email, etc.

Architect creates architecture doc and enhances tasks.

### Step 5: Implement Tasks

```bash
/tm-dev
```

Follow dependency order:
1. Create Profile model
2. Add profile API endpoints
3. Implement avatar upload
4. Add validation
5. Create frontend UI
6. Write tests

Mark each task done after tests pass.

### Step 6: Retrospective

```bash
/tm-retrospective
```

Answer reflection questions, get retrospective document.

### Step 7: Start Next Epic

```bash
/status
```

You're back at `ideation` phase. Repeat!

---

## Key Concepts

### Workflow Phases

The system enforces a linear workflow:

1. **Ideation** → Create PRD
2. **Planning** → Parse into Task Master
3. **Architecture** → Design technical solution
4. **Implementation** → Build tasks
5. **Retrospective** → Capture learnings

**Phase gates prevent skipping steps.** You can't jump to implementation without completing architecture first.

### Task Master as Single Source of Truth

All task state lives in `.taskmaster/tasks/tasks.json`:
- Task title, description, details
- Status (pending, in-progress, done, blocked)
- Dependencies
- Complexity scores
- Test strategy

**No story files.** All context is in Task Master's `details` field.

### Dependency Enforcement

The Developer agent **cannot start a task** if its dependencies aren't complete:

```
Task 5 depends on:
  • Task 2 (status: done) ✅
  • Task 3 (status: in-progress) ❌

Cannot start Task 5 until Task 3 is done.
```

This prevents build order issues and ensures prerequisites are met.

### Test-Driven Development

The Developer agent **cannot mark a task done** without passing tests:

```
❌ TESTS FAILED

Cannot mark task done while tests are failing:
  • test_user_creation_with_invalid_email
  • test_password_hashing_with_null_password

Fix tests first, then mark done.
```

This enforces code quality and correctness.

---

## Common Commands

### Check Workflow Status
```bash
/status
```
Shows: current phase, active epic, task progress, available commands, next steps

### List All Tasks in Epic
```bash
task-master list epic-1-auth
```

### Show Task Details
```bash
task-master show epic-1-auth 3
```

### Update Task Status
```bash
task-master update-status epic-1-auth 3 in-progress
task-master update-status epic-1-auth 3 done
```

### Set Task Dependency
```bash
task-master set-dependency epic-1-auth 5 2
```
(Task 5 depends on Task 2)

### Remove Task Dependency
```bash
task-master remove-dependency epic-1-auth 5 2
```

---

## Troubleshooting

### "Task Master CLI not found"
```bash
npm install -g task-master
```

### "Phase gate blocked"
Check current phase:
```bash
/status
```
Complete previous phases before proceeding.

### "Dependencies not met"
Check which tasks are blocking:
```bash
task-master show epic-1-auth 5
```
Look at `dependencies` field. Complete those tasks first.

### "Tests failing"
Fix the code or tests:
```bash
npm test  # or your test command
```
Only mark done when tests pass.

### "Workflow state corrupted"
Reset workflow state:
```bash
cp .taskmaster/workflow-state.json .taskmaster/workflow-state.json.backup
./install-claude-code.sh  # or install-opencode.sh
```

### "Can't find architecture document"
Architect agent should have created it at:
```bash
ls docs/architecture/
```
If missing, run `/tm-architect` again.

---

## Tips for Success

1. **Trust the workflow** - Don't skip phases. Each builds on the previous.

2. **Use /status frequently** - When in doubt, check your status.

3. **Let architecture guide development** - Read the architecture doc before implementing.

4. **Respect dependencies** - They're there to prevent build order issues.

5. **Write tests first** - TDD catches issues early.

6. **Run retrospectives** - Every epic improves the next one.

7. **Keep Task Master updated** - It's your single source of truth.

8. **Ask clarifying questions** - Agents will probe for details. Answer thoroughly.

9. **Read agent boundaries** - Each agent has specific responsibilities. Don't ask Developer to design architecture.

10. **Iterate and improve** - Use retrospective action items in next epic.

---

## What's Next?

- **Read the full workflow guide:** `src/workflows/workflow-plan-and-build.md`
- **Explore slash commands:** `.claude/commands/`
- **Understand the validator:** `src/validators/taskmaster-validator.js`
- **Check integration guide:** `src/INTEGRATION_GUIDE.md`

---

## Getting Help

### Slash Commands
- `/status` - When you're lost, start here
- `/tm-pm` - For planning and requirements
- `/tm-architect` - For technical design questions
- `/tm-dev` - For implementation guidance
- `/tm-retrospective` - For learning capture

### Documentation
- **Workflow Guide:** Comprehensive workflow explanation
- **Integration Guide:** How BMAD-TM Lite works internally
- **Validator README:** Understanding the enforcement layer

### Common Scenarios

**"I don't know what to do next"**
→ Run `/status` - it tells you exactly what to do

**"I want to skip architecture and just code"**
→ Phase gates prevent this. Complete architecture first.

**"My task is blocked"**
→ Check dependencies. Complete prerequisite tasks first.

**"I'm stuck in a phase"**
→ Check exit criteria for that phase in the workflow guide

**"Tests are failing but I want to move on"**
→ Agent blocks this. Fix tests first - it prevents bugs later.

---

## Example Project Timeline

**Week 1:**
- Day 1: Run `/tm-pm`, create PRD, parse into Task Master
- Day 2: Run `/tm-architect`, design architecture, enhance tasks
- Day 3-4: Start `/tm-dev`, implement first few tasks
- Day 5: Continue implementation

**Week 2:**
- Day 1-3: Complete remaining tasks
- Day 4: Run `/tm-retrospective`, capture learnings
- Day 5: Start next epic with improved process

---

Happy building! 🚀

For questions or issues, refer to:
- Full workflow guide: `src/workflows/workflow-plan-and-build.md`
- Integration details: `src/INTEGRATION_GUIDE.md`

**Now run `/status` and start your first epic!**
