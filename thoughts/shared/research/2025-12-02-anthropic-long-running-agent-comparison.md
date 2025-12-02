---
date: 2025-12-02T12:00:00-08:00
researcher: reuben
git_commit: bcc34bc4e943403c57e69ae27f30ddbbc1f0b075
branch: master
repository: bmad-tm
topic: "How Anthropic's long-running agent harness intersects with SCUD implementation"
tags: [research, agent-architecture, scud, anthropic, long-running-agents]
status: complete
last_updated: 2025-12-02
last_updated_by: reuben
---

# Research: Anthropic Long-Running Agent Harness vs SCUD Implementation

**Date**: 2025-12-02T12:00:00-08:00
**Researcher**: reuben
**Git Commit**: bcc34bc4e943403c57e69ae27f30ddbbc1f0b075
**Branch**: master
**Repository**: bmad-tm

## Research Question

How does the Anthropic blog "Effective harnesses for long-running agents" intersect with our SCUD implementation? What improvements or simplifications could be incorporated?

## Summary

SCUD already implements many of Anthropic's recommended patterns, but approaches them differently through a **DAG-driven task graph** rather than a **linear feature list**. The core overlap is in the philosophy of incremental progress with structured handoffs between sessions. Key gaps include: no narrative progress file, no automatic git commit workflow, and no explicit "getting up to speed" routine. The biggest opportunity is adding a **progress journal** to complement the existing task status tracking.

## Detailed Findings

### Concept Mapping: Anthropic vs SCUD

| Anthropic Concept | SCUD Equivalent | Match Quality |
|-------------------|-----------------|---------------|
| **Initializer Agent** | `scud init` command | Partial - creates structure but not feature list |
| **Feature List (JSON)** | Task graph in `.scud/tasks/tasks.scg` | Different approach - DAG > linear list |
| **Progress File (claude-progress.txt)** | No equivalent | Gap |
| **Git Commits** | Manual (no automation) | Gap |
| **init.sh** | No equivalent | Gap |
| **Incremental Progress** | `scud next --claim` + single task focus | Strong match |
| **Clean State** | Hooks auto-mark tasks Done | Strong match |
| **Getting Up to Speed** | `.opencode/hook/session-start.md` (informational only) | Partial |

### Deep Dive: Key Intersections

#### 1. Initialization System

**Anthropic's Approach:**
- Initializer agent creates: init.sh, claude-progress.txt, initial git commit
- Feature list JSON with all features marked "failing"

**SCUD's Approach (`scud-cli/src/commands/init.rs:10-108`):**
- Creates `.scud/` directory structure
- Creates `config.toml` for LLM settings
- Creates empty `tasks.scg` and `tasks.json` files
- Creates docs directories (prd, phases, architecture, retrospectives)
- No feature list generation - tasks added later via `scud parse-prd` or manually

**Assessment:** SCUD's initialization is more minimal. Anthropic's approach front-loads work definition, while SCUD defers it.

#### 2. Progress Tracking

**Anthropic's Approach:**
- `claude-progress.txt` - narrative log of what agents have done
- Git commit messages - structured record of changes
- Together provide context for fresh sessions

**SCUD's Approach:**
- Task status in `.scud/tasks/tasks.scg` - structured but not narrative
- No automatic git integration
- `scud sessions` shows who is working on what (live state, not history)
- No narrative "what happened" log

**Assessment:** SCUD tracks current state well but lacks historical narrative. An agent starting fresh can see WHAT is done but not HOW or WHY decisions were made.

#### 3. Incremental Progress

**Anthropic's Approach:**
- Work on one feature at a time
- Mark feature as "passing" only after testing
- Commit progress frequently

**SCUD's Approach (`scud-cli/src/commands/next.rs:22-56`):**
- `scud next` finds single next available task by dependency resolution
- `--claim` flag prevents concurrent work
- Status progression: Pending → InProgress → Done
- Hooks auto-complete task when session ends

**Assessment:** Strong alignment. SCUD's DAG-driven approach is more sophisticated - it automatically determines what's ready based on dependency completion, vs Anthropic's linear feature list.

#### 4. Session Handoff

**Anthropic's Approach:**
- Each session reads progress file and git logs first
- Explicit "getting up to speed" steps in prompt
- Run basic test before starting new work

**SCUD's Approach:**
- `.opencode/hook/session-start.md` displays available commands (informational)
- No automatic execution of orientation steps
- No standard "run tests first" pattern

**Assessment:** SCUD could benefit from a more structured session start routine that reads recent commits, checks for stale locks, and verifies baseline health.

#### 5. Victory Prevention

**Anthropic's Problem:**
> "A later agent instance would look around, see that progress had been made, and declare the job done."

**Anthropic's Solution:**
- Feature list with explicit pass/fail status
- Strongly-worded instructions: "It is unacceptable to remove or edit tests"

**SCUD's Approach:**
- Task graph with explicit status per task
- `scud stats` shows completion percentage
- `scud next` automatically finds remaining work
- No agent can "declare victory" - incomplete tasks remain visible

**Assessment:** SCUD handles this well through structure. The DAG ensures uncompleted work is visible. However, no equivalent to the "don't edit tests" instruction exists.

### Gap Analysis

#### Critical Gaps

**1. No Progress Journal**
- Anthropic: `claude-progress.txt` provides narrative context
- SCUD: Only structured task status exists
- Impact: Fresh agents lack "what happened" context

**2. No Automatic Git Workflow**
- Anthropic: Commit after each feature
- SCUD: No git integration
- Impact: Can't use git history for context or rollback

**3. No Explicit Session Start Routine**
- Anthropic: pwd → read progress → read git logs → choose task
- SCUD: Informational hook only, no automatic execution
- Impact: Each agent must figure out orientation independently

#### Minor Gaps

**4. No init.sh Script**
- Anthropic: Generated script to start dev server
- SCUD: Expects project-specific setup
- Impact: Minimal - SCUD is task management, not dev environment

**5. No Browser Testing Integration**
- Anthropic: Puppeteer MCP for E2E verification
- SCUD: No testing integration
- Impact: Outside SCUD's scope (task tracking vs execution)

### SCUD Advantages Over Anthropic's Approach

1. **DAG-Driven Execution** - Dependencies automatically determine task readiness, vs manual feature list ordering

2. **Automatic Completion via Hooks** - Stop hook marks tasks Done automatically (`scud-cli/src/commands/hook_complete.rs:26-60`)

3. **Parallel Execution Support** - Wave computation, claiming, lock management for multiple agents (`scud-cli/src/commands/next.rs:133-289`)

4. **Stale Lock Detection** - `scud doctor` finds and fixes orphaned work (`scud-cli/src/commands/doctor.rs:88-296`)

5. **Structured Task Expansion** - Complex tasks automatically suggest breakdown via Fibonacci complexity (`scud-cli/src/models/task.rs:307-332`)

## Recommended Improvements

### High Value / Low Effort

**1. Add Progress Journal File**

Create `.scud/progress.md` (or `.scud/journal.md`) that agents append to:

```markdown
## 2025-12-02 10:30 - phase1:5 - alice
Implemented user authentication flow. Added JWT token handling
in auth.rs. Hit issue with refresh tokens, documented workaround
in code comments. Next agent should review token expiry logic.
```

- Append-only (agents don't read/modify existing entries)
- Provides narrative context Anthropic recommends
- Complements structured task status

**2. Add Session Start Command**

Create `scud warmup` command that executes Anthropic's "getting up to speed" routine:

```bash
scud warmup [--tag TAG]
# Output:
# Working directory: /path/to/project
# Recent commits:
#   abc123 Fix auth bug (2h ago)
#   def456 Add user model (4h ago)
# Progress summary:
#   Last entry: "Implemented auth flow..."
# Current sessions:
#   phase1:7 | bob | 0.5h
# Next available task:
#   phase1:8 | Add password reset
```

**3. Add Git Commit Wrapper**

Create `scud commit` that:
- Stages SCUD-related files
- Generates commit message from current task
- Optionally appends to progress journal

### Medium Value / Medium Effort

**4. Progress Journal in Hook**

Modify `hook_complete` to append brief entry to progress journal:
- Task ID, title, completion time
- Optionally prompt for summary before closing

**5. Feature List Mode**

Add `scud features` command that generates Anthropic-style feature list from task graph:
- Useful for projects where DAG is overkill
- Provides familiar interface for teams from Anthropic's approach

### Lower Priority

**6. init.sh Generation**

Add `scud init --generate-script` that creates project startup script based on detected patterns (package.json → npm start, Cargo.toml → cargo run, etc.)

## Recommended Simplifications

### Remove or Simplify

**1. Workflow State Phases**

The `workflow-state.json` with ideation/planning/architecture/implementation/retrospective phases adds complexity but may not be used. Consider:
- Making phases optional (default to no workflow phases)
- Or removing entirely if tag-based organization is sufficient

**2. Dual Format Storage**

Both `.scud/tasks/tasks.scg` and `.scud/tasks/tasks.json` are maintained. Consider:
- SCG-only with on-demand JSON export
- Or JSON-only for simplicity (more familiar format)

**3. Agent Definitions**

Multiple agent roles (pm, sm, architect, dev, retrospective) in `.claude/agents/` and `.opencode/skills/` may be over-specified. Consider:
- Single "developer" agent with task context
- Let task metadata (complexity, type) drive behavior

## Architecture Documentation

### Current SCUD Session Lifecycle

```
┌─────────────────────────────────────────────────────────────────┐
│ Session Start                                                   │
│ - .opencode/hook/session-start.md displayed (informational)     │
│ - Agent manually runs: scud next --claim --name alice           │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│ Task Execution                                                  │
│ - Task locked (locked_by: alice, locked_at: timestamp)          │
│ - Status: in-progress                                           │
│ - SCUD_TASK_ID env var set for hook                            │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│ Session End (Stop Hook)                                         │
│ - scud _hook-complete fires automatically                       │
│ - Task status → Done                                            │
│ - Lock released                                                 │
│ - .scud/current-task deleted                                    │
└─────────────────────────────────────────────────────────────────┘
```

### Proposed Enhanced Lifecycle (with Anthropic patterns)

```
┌─────────────────────────────────────────────────────────────────┐
│ Session Start                                                   │
│ 1. Run: scud warmup (NEW)                                       │
│    - Shows working directory                                    │
│    - Shows recent git commits                                   │
│    - Shows progress journal tail                                │
│    - Shows active sessions                                      │
│    - Suggests next task                                         │
│ 2. Agent claims task: scud next --claim --name alice            │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│ Task Execution                                                  │
│ - Same as current                                               │
│ - Agent periodically runs: scud commit (NEW)                    │
│   - Creates git commit with task context                        │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│ Session End (Enhanced Hook)                                     │
│ - scud _hook-complete fires                                     │
│ - Prompts for brief progress note (NEW)                         │
│ - Appends to .scud/progress.md (NEW)                            │
│ - Task status → Done                                            │
│ - Lock released                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Code References

### SCUD Initialization
- `scud-cli/src/commands/init.rs:10-108` - CLI init command
- `scud-cli/src/storage/mod.rs:149-189` - Storage initialization
- `bin/install.js:154-437` - NPM installation wrapper

### Session Management
- `scud-cli/src/commands/hook_complete.rs:26-60` - Hook completion flow
- `scud-cli/src/commands/hooks.rs:28-68` - Hook installation
- `scud-cli/src/commands/sessions.rs:7-58` - Session listing
- `.opencode/hook/session-start.md:1-53` - Session start hook

### Task Discovery
- `scud-cli/src/commands/next.rs:22-56` - Find next available task
- `scud-cli/src/commands/next.rs:133-289` - Claim mode with atomic locking
- `scud-cli/src/models/task.rs:294-302` - Dependency checking

### Progress Tracking
- `scud-cli/src/models/task.rs:110-114` - Lock fields (locked_by, locked_at)
- `scud-cli/src/commands/doctor.rs:88-296` - Stale lock detection
- `.scud/workflow-state.json` - Workflow phases (may be unused)

## Open Questions

1. **Progress Journal Format**: Should it be markdown (human-readable) or JSON (machine-parseable)? Markdown aligns with Anthropic's approach.

2. **Git Integration Scope**: Full automation (auto-commit on task completion) vs wrapper command (manual trigger)?

3. **Workflow Phases Usage**: Are the ideation/planning/architecture/implementation/retrospective phases actively used? If not, simplify.

4. **Hook Prompt Feasibility**: Can the Stop hook prompt for input before completing, or must it be non-interactive?
