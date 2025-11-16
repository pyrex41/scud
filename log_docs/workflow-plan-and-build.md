# Workflow: Plan and Build with BMAD-TM Lite

This document outlines the end-to-end workflow for taking a product idea from conception to completion using BMAD-TM Lite.

**Goal:** Complete all tasks for a single epic using lightweight workflow orchestration

**Source of Truth:** `.taskmaster/tasks/tasks.json` (managed by Task Master CLI)

**Workflow Orchestration:** `.taskmaster/workflow-state.json` (phase tracking and enforcement)

---

## Quick Start

1. Check your current status: `/status`
2. Start with Product Manager: `/tm-pm` (creates PRD and epic files)
3. Run Scrum Master: `/tm-sm` (parses into Task Master with tags)
4. Follow the phase gates - the system will guide you

---

## The Six Phases

### Phase 1: Ideation (Product Definition)

**Agent:** `/tm-pm`
**Goal:** Create Product Requirements Document (PRD)
**Output:** `docs/prd/[product-name]-prd.md`

**What Happens:**
1. PM agent asks discovery questions
2. Creates structured PRD document
3. Identifies logical epic boundaries
4. Updates workflow state to 'planning'

**Exit Criteria:**
- ✅ PRD created with clear requirements
- ✅ Epic boundaries identified
- ✅ Workflow phase: planning

---

### Phase 2a: Planning (Epic Creation)

**Agent:** `/tm-pm`
**Goal:** Create epic markdown files
**Output:** Epic files in `docs/epics/`

**What Happens:**
1. PM agent reads PRD
2. Creates epic markdown file(s) in `docs/epics/`
3. Structures epics with user stories and tasks
4. Hands off to Scrum Master

**Exit Criteria:**
- ✅ Epic markdown files created
- ✅ User stories defined
- ✅ Ready for Scrum Master

---

### Phase 2b: Planning (Task Breakdown)

**Agent:** `/tm-sm` (Scrum Master)
**Goal:** Translate epics into Task Master with proper estimation
**Output:** Tasks in `.taskmaster/tasks/tasks.json`

**What Happens:**
1. SM reads epic markdown files
2. **Parses into Task Master with tag:** `task-master parse-prd [file] --tag=[epic-tag]`
3. **Switches to new epic:** `task-master use-tag [epic-tag]`
4. Analyzes task complexity
5. Breaks down large tasks (>13 points) into subtasks
6. Maps dependencies
7. Refines complexity estimates
8. Updates workflow state to 'architecture'

**Task Master Tag Operations:**
```bash
# Create epic with tag
task-master parse-prd docs/epics/epic-1-auth.md --tag=epic-1-auth

# Switch to work on that epic
task-master use-tag epic-1-auth

# View tasks in active epic
task-master list

# Switch to different epic
task-master use-tag epic-2-todos
```

**Exit Criteria:**
- ✅ Epic parsed into Task Master with unique tag
- ✅ All tasks ≤ 13 complexity points
- ✅ Dependencies mapped
- ✅ Workflow phase: architecture
- ✅ Active epic set in workflow state

**IMPORTANT:** Each epic gets its own tag. Use `task-master use-tag [tag]` to switch between epics.

---

### Phase 3: Architecture (Technical Design)

**Agent:** `/tm-architect`
**Goal:** Design technical solution and enhance tasks with implementation details
**Output:**
- `docs/architecture/[epic-tag]-architecture.md`
- Enhanced task details in Task Master

**What Happens:**
1. Architect loads epic from Task Master
2. Asks technical clarifying questions
3. Creates comprehensive architecture document
4. **Enhances each task** with technical details in `details` field:
   - Implementation approach
   - Files to modify
   - Dependencies (technical requirements)
   - Testing strategy
   - Complexity justification
5. Sets task dependencies in Task Master
6. Updates workflow state to 'implementation'

**Exit Criteria:**
- ✅ Architecture document complete
- ✅ All tasks have technical details
- ✅ Dependencies set correctly
- ✅ Workflow phase: implementation

**IMPORTANT:** No story files are created. All task context lives in Task Master's `details` field.

---

### Phase 4: Implementation (Build)

**Agent:** `/tm-dev`
**Goal:** Implement tasks one by one
**Output:** Working, tested code

**What Happens:**
1. Developer loads epic from Task Master
2. Shows available tasks (dependencies met, status: pending)
3. **Validates dependencies** before starting any task
4. Implements task following architecture plan
5. Writes tests per test strategy
6. Verifies tests pass
7. Updates task status to 'done'
8. Repeats until all tasks complete

**Dependency Enforcement:**
- ❌ CANNOT start task if dependencies not complete
- ❌ CANNOT mark task done if tests failing
- ✅ Only shows tasks with all dependencies met

**Exit Criteria (per task):**
- ✅ Code implemented per architecture
- ✅ Tests written and passing
- ✅ Task Master status: done

**Exit Criteria (epic):**
- ✅ All tasks status: done
- ✅ Workflow ready for retrospective

---

### Phase 5: Retrospective (Learning Capture)

**Agent:** `/tm-retrospective`
**Goal:** Analyze completed epic and capture learnings
**Output:** `docs/retrospectives/[epic-tag]-retrospective.md`

**What Happens:**
1. Validates all tasks complete
2. Gathers epic data (complexity, duration, blockers)
3. Asks reflection questions
4. Creates comprehensive retrospective document
5. Identifies action items for next epic
6. Resets workflow to 'ideation' for next cycle

**Exit Criteria:**
- ✅ Retrospective document created
- ✅ Action items identified
- ✅ Workflow reset to ideation
- ✅ Ready for next epic

---

## Workflow State Management

The workflow state file `.taskmaster/workflow-state.json` tracks:

- **Current Phase:** Where you are in the workflow
- **Active Epic:** Which epic is currently being worked on
- **Phase Status:** Which phases are complete
- **History:** Log of all actions (task completed, phase transitions, etc.)
- **Completed Epics:** Archive of finished work

**Check Status Anytime:**
```bash
/status
```

Shows:
- Current phase with visual indicator
- Active epic and task progress
- Available commands
- Any blockers or warnings
- Recommended next steps

---

## Key Differences from Traditional BMAD

### What BMAD-TM Lite Does:

✅ **Lightweight orchestration** - Phase gates guide workflow, don't enforce XML structure
✅ **Markdown agents** - Easy to read, edit, and understand
✅ **Task Master integration** - Single source of truth for tasks
✅ **Dependency enforcement** - Cannot start tasks with incomplete dependencies
✅ **Status visibility** - `/status` command shows current state
✅ **Slash commands** - Simple invocation (no XML menus)

### What BMAD-TM Lite Does NOT Do:

❌ **No BMAD XML agents** - Not using full BMAD agent structure
❌ **No workflow.yaml** - Not using BMAD workflow engine
❌ **No story files** - All context in Task Master details field
❌ **No templates** - Architecture and retrospective docs created directly

---

## Story Files: ELIMINATED

**Previous Approach (REMOVED):**
- Scrum Master created separate story files
- Duplicated metadata (task_id, complexity, status)
- Risk of state drift

**Current Approach:**
- **All task context lives in Task Master** `details` field
- Architect adds technical details during architecture phase
- No separate story files needed
- Single source of truth maintained

**Task Details Field Structure:**

```
TECHNICAL DESIGN:

**Approach:** [High-level approach]

**Implementation Steps:**
1. [Step 1]
2. [Step 2]

**Files to Modify:**
- file.js (what to change)

**Dependencies:**
- Task X (why)

**Testing:**
- Unit: [what to test]
- Integration: [what to test]

**Risks:**
- [Known risks]

**Complexity:** [score] ([justification])
```

---

## Example Workflow Run

```bash
# Start new epic
$ /status
Current Phase: ideation
→ Run /tm-pm to create PRD

$ /tm-pm
[PM creates PRD, then guides to parse]

$ task-master parse-prd epic-1-auth.md --tag=epic-1-auth
Epic created: 8 tasks

$ /status
Current Phase: architecture
Active Epic: epic-1-auth (8 tasks pending)
→ Run /tm-architect to design solution

$ /tm-architect
[Architect creates architecture doc, enhances tasks]

$ /status
Current Phase: implementation
Active Epic: epic-1-auth
  Available: Task 1, Task 2 (no dependencies)
  Blocked: Task 3 (depends on 1, 2)
→ Run /tm-dev to implement tasks

$ /tm-dev
[Implements Task 1, tests pass, marks done]
[Implements Task 2, tests pass, marks done]
[Task 3 now available]
[Continue until all done]

$ /status
All tasks complete! (8/8 done)
→ Run /tm-retrospective

$ /tm-retrospective
[Creates retrospective doc, captures learnings, resets workflow]

$ /status
Current Phase: ideation
Ready for next epic!
```

---

## Common Pitfalls & Solutions

### Pitfall: Jumping phases
**Problem:** Trying to run /tm-dev before architecture complete
**Solution:** Phase gate blocks activation, shows error
**Fix:** Complete architecture phase first

### Pitfall: Ignoring dependencies
**Problem:** Wanting to start Task 5 when Task 2 isn't done
**Solution:** Dependency validation blocks task start
**Fix:** Complete dependency tasks first, or remove incorrect dependency

### Pitfall: Skipping tests
**Problem:** Marking task done without tests passing
**Solution:** Agent boundary - developer CANNOT mark done without passing tests
**Fix:** Write tests, make them pass, then mark done

### Pitfall: Lost context
**Problem:** Can't remember current state
**Solution:** Run `/status` anytime
**Shows:** Current phase, available commands, task progress, blockers

---

## Tips for Success

1. **Trust the phases** - Each phase builds on the previous one
2. **Use /status liberally** - Check state frequently
3. **Let architecture guide implementation** - Don't deviate without reason
4. **Respect dependencies** - They're there for a reason
5. **Write tests first** - Catch issues early
6. **Run retrospectives** - Every epic makes the next one better
7. **Keep Task Master updated** - It's your single source of truth

---

## Next Steps

Ready to start? Run:

```bash
/status
```

Then follow the guidance!
