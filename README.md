# BMAD-TM Lite

**Lightweight workflow orchestration for building software with Task Master and AI agents** 🚀

BMAD-TM Lite combines Task Master's robust state management with intelligent agent prompting to guide you through building software epics. It enforces best practices (dependency management, testing, phase gates) without the overhead of full BMAD XML complexity.

---

## What is BMAD-TM Lite?

A **lightweight alternative to full BMAD** that:

- ✅ Guides you through structured workflow phases
- ✅ Enforces dependencies and prevents build order issues
- ✅ Maintains single source of truth (Task Master)
- ✅ Uses simple markdown agents (no XML)
- ✅ Validates workflow correctness automatically
- ✅ Works with Claude Code CLI or OpenCode

**Not BMAD XML** - We skip the heavy XML agent structure and workflow.yaml complexity in favor of pragmatic markdown-based agents with validation hooks.

---

## Quick Start

### Installation

**Claude Code CLI:**
```bash
./install-claude-code.sh
```

**OpenCode:**
```bash
./install-opencode.sh
```

### First Epic

```bash
/status                    # Check your workflow state
/tm-pm                     # Create PRD and epic files
/tm-sm                     # Parse epics into Task Master (with tags)
/tm-architect              # Design technical solution
/tm-dev                    # Implement tasks
/tm-retrospective          # Capture learnings
```

**Full walkthrough:** See [QUICKSTART.md](QUICKSTART.md)

---

## The Workflow

BMAD-TM Lite enforces a **6-phase linear workflow**:

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐     ┌────────────────┐     ┌───────────────┐
│  Ideation   │────▶│  Planning    │────▶│ Architecture │────▶│ Implementation │────▶│ Retrospective │
│   (PRD)     │     │(PM + SM)     │     │  (Design)    │     │    (Code)      │     │  (Learning)   │
└─────────────┘     └──────────────┘     └──────────────┘     └────────────────┘     └───────────────┘
      ▲                                                                                       │
      └───────────────────────────────────────────────────────────────────────────────────────┘
                                    (Reset for next epic)
```

### Phase 1: Ideation (Product Manager)
- Create Product Requirements Document
- Define goals, users, scope
- Identify epic boundaries
- **Output:** `docs/prd/[name]-prd.md`

### Phase 2a: Planning - Epic Creation (Product Manager)
- Break PRD into epic markdown files
- Define user stories and initial tasks
- **Output:** `docs/epics/[epic-name].md`

### Phase 2b: Planning - Task Master Translation (Scrum Master)
- Parse epic files into Task Master with tags (`--tag=epic-name`)
- Switch between epics using `task-master use-tag`
- Analyze and refine task complexity (Fibonacci scale)
- Break down large tasks (>13 points)
- Map dependencies
- **Output:** Tasks in `.taskmaster/tasks/tasks.json`

### Phase 3: Architecture (Architect)
- Design technical solution
- Create architecture document
- Enhance tasks with implementation details
- Set dependencies
- **Output:** `docs/architecture/[epic]-architecture.md`

### Phase 4: Implementation (Developer)
- Implement tasks in dependency order
- Write and run tests
- Update Task Master status
- **Output:** Working, tested code

### Phase 5: Retrospective (Retrospective Agent)
- Analyze completed epic
- Capture learnings
- Identify improvements
- Reset for next epic
- **Output:** `docs/retrospectives/[epic]-retrospective.md`

---

## Key Features

### 🔒 Phase Gates
**Cannot skip phases.** Architect agent blocks if no epic exists. Developer agent blocks if architecture isn't complete.

### 🔗 Dependency Enforcement
**Cannot start tasks with incomplete dependencies.** Developer agent validates all prerequisites before allowing work to start.

### ✅ Test Enforcement
**Cannot mark tasks done without passing tests.** Developer agent blocks completion until tests pass.

### 📊 Status Visibility
**Always know where you are.** `/status` command shows current phase, task progress, available commands, and next steps.

### 📝 Single Source of Truth
**All task state in Task Master.** No story files, no duplicate metadata, no state drift.

### 🎯 Agent Boundaries
**Each agent has clear responsibilities.** PM doesn't code, Dev doesn't design architecture, Architect doesn't create PRDs.

---

## File Structure

```
bmad-tm/
├── .claude/
│   └── commands/              # Slash commands for Claude Code CLI
│       ├── status.md          # Show workflow status
│       ├── tm-pm.md           # Product Manager agent
│       ├── tm-sm.md           # Scrum Master agent (Task Master operations)
│       ├── tm-architect.md    # Architect agent
│       ├── tm-dev.md          # Developer agent
│       ├── tm-retrospective.md # Retrospective agent
│       └── helpers/
│           └── validation-helper.md # Validation patterns
├── .opencode/
│   └── skills/                # Skills for OpenCode
│       ├── status.md
│       ├── tm-pm.md
│       ├── tm-sm.md           # Scrum Master skill
│       ├── tm-architect.md
│       ├── tm-dev.md
│       └── tm-retrospective.md
├── .taskmaster/
│   ├── tasks/
│   │   └── tasks.json         # Task Master state (single source of truth)
│   └── workflow-state.json    # Workflow phase tracker
├── docs/
│   ├── prd/                   # Product Requirements Documents
│   ├── epics/                 # Epic markdown files (for parsing)
│   ├── architecture/          # Architecture documents
│   └── retrospectives/        # Retrospective documents
├── src/
│   ├── validators/
│   │   └── taskmaster-validator.js  # Validation logic
│   └── workflows/
│       └── workflow-plan-and-build.md # Full workflow guide
├── install-claude-code.sh     # Claude Code CLI installer
├── install-opencode.sh        # OpenCode installer
├── QUICKSTART.md              # 5-minute quick start
├── README.md                  # This file
└── INTEGRATION_GUIDE.md       # System internals (optional)
```

---

## Agents

### Product Manager (`/tm-pm`)
**Phases:** Ideation, Planning
**Creates:** PRD, Epic markdown files
**Updates:** Workflow state

**Responsibilities:**
- Gather requirements through discovery questions
- Create structured PRD
- Break down PRD into epic markdown files
- Hand off to Scrum Master for Task Master operations

### Scrum Master (`/tm-sm`)
**Phase:** Planning
**Creates:** Task Master tasks with tags
**Updates:** Task Master (parse, complexity, dependencies)

**Responsibilities:**
- Parse epic markdown files into Task Master with `--tag` flag
- Switch between epics using `task-master use-tag`
- Analyze and refine task complexity (Fibonacci scale: 1, 2, 3, 5, 8, 13)
- Break down large tasks (>13 points) into subtasks
- Map task dependencies
- Validate dependency chains

### Architect (`/tm-architect`)
**Phase:** Architecture
**Creates:** Architecture document
**Updates:** Task Master (adds technical details, sets dependencies)

**Responsibilities:**
- Design technical solution
- Choose technologies and patterns
- Enhance tasks with implementation guidance
- Set task dependencies based on technical requirements

### Developer (`/tm-dev`)
**Phase:** Implementation
**Creates:** Code, tests
**Updates:** Task Master (status changes)

**Responsibilities:**
- Validate dependencies before starting tasks
- Implement code following architecture
- Write and run tests
- Update task status (in-progress, done)
- BLOCK if dependencies unmet or tests failing

### Retrospective (`/tm-retrospective`)
**Phase:** Retrospective
**Creates:** Retrospective document
**Updates:** Workflow state (reset to ideation)

**Responsibilities:**
- Validate all tasks complete
- Analyze epic metrics (complexity, duration, blockers)
- Capture learnings and insights
- Identify action items for next epic
- Reset workflow for next cycle

---

## Commands

### Workflow Commands

| Command | Purpose | Phase |
|---------|---------|-------|
| `/status` | Show current workflow state | Any |
| `/tm-pm` | Activate Product Manager | Ideation, Planning |
| `/tm-sm` | Activate Scrum Master | Planning |
| `/tm-architect` | Activate Architect | Architecture |
| `/tm-dev` | Activate Developer | Implementation |
| `/tm-retrospective` | Activate Retrospective | Retrospective |

### Task Master Commands

| Command | Purpose |
|---------|---------|
| `task-master parse-prd [file] --tag=[epic]` | Parse epic into tasks (creates tag) |
| `task-master use-tag [epic-tag]` | Switch to work on specific epic |
| `task-master list-tags` | List all epic tags |
| `task-master list` | List all tasks in active epic |
| `task-master show [task-id]` | Show task details |
| `task-master update-status [task-id] [status]` | Update task status |
| `task-master set-dependency [task] [dep]` | Set dependency |
| `task-master remove-dependency [task] [dep]` | Remove dependency |

### Validator Commands

| Command | Purpose |
|---------|---------|
| `taskmaster-validator.js validate-phase [agent] [phases...]` | Check phase gate |
| `taskmaster-validator.js validate-epic` | Check active epic exists |
| `taskmaster-validator.js validate-dependencies [epic] [task]` | Check task dependencies |
| `taskmaster-validator.js validate-epic-complete [epic]` | Check all tasks done |
| `taskmaster-validator.js get-available-tasks [epic]` | Get startable tasks |
| `taskmaster-validator.js get-epic-stats [epic]` | Get epic statistics |
| `taskmaster-validator.js get-command-availability` | Check available commands |
| `taskmaster-validator.js list-epic-tags` | List all epic tags |
| `taskmaster-validator.js get-active-epic-tag` | Get currently active epic |
| `taskmaster-validator.js set-active-epic-tag [epic]` | Set active epic in workflow |

---

## Validation & Enforcement

BMAD-TM Lite enforces correct workflow usage through the Task Master validator:

### Phase Gates
```javascript
// Before activating tm-architect:
validatePhase('tm-architect', ['architecture'])
// ❌ Blocks if current phase is not 'architecture'
```

### Dependency Checks
```javascript
// Before starting task 3:
validateDependencies('epic-1-auth', '3')
// ❌ Blocks if any dependency task is not 'done'
```

### Epic Completion
```javascript
// Before running retrospective:
validateEpicComplete('epic-1-auth')
// ❌ Blocks if any task is not 'done'
```

### Test Validation
```
Developer agent CANNOT mark task done if tests failing.
Manual enforcement via agent boundaries.
```

---

## Example Workflow

### Building User Authentication

```bash
# Phase 1: Ideation
$ /status
→ Phase: ideation
→ Run /tm-pm to create PRD

$ /tm-pm
[PM asks questions, creates PRD at docs/prd/auth-system-prd.md]
[PM creates epic at docs/epics/epic-1-authentication.md]

# Phase 2: Planning
$ task-master parse-prd docs/epics/epic-1-authentication.md --tag=epic-1-auth
→ Epic created: 8 tasks

$ /status
→ Phase: architecture
→ Run /tm-architect to design solution

# Phase 3: Architecture
$ /tm-architect
[Architect asks technical questions]
[Creates docs/architecture/epic-1-auth-architecture.md]
[Enhances all tasks with implementation details]
[Sets dependencies: Task 3 depends on 1, 2]

$ /status
→ Phase: implementation
→ Run /tm-dev to implement tasks

# Phase 4: Implementation
$ /tm-dev
→ Available: Task 1, Task 2 (no dependencies)
→ Blocked: Task 3 (depends on 1, 2)

[Implement Task 1]
[Tests pass ✅]
[Mark Task 1 done]

[Implement Task 2]
[Tests pass ✅]
[Mark Task 2 done]

→ Task 3 now available (dependencies met)

[Continue until all 8 tasks done]

$ /status
→ All tasks complete (8/8)
→ Run /tm-retrospective

# Phase 5: Retrospective
$ /tm-retrospective
[Analyzes epic: 8 tasks, 45 complexity, 2 weeks]
[Creates docs/retrospectives/epic-1-auth-retrospective.md]
[Captures learnings and action items]
[Resets workflow to ideation]

$ /status
→ Phase: ideation
→ Ready for next epic!
```

---

## Differences from Full BMAD

| Feature | Full BMAD | BMAD-TM Lite |
|---------|-----------|--------------|
| Agent Structure | XML with `<agent>` wrapper | Markdown with YAML frontmatter |
| Activation | `<activation>` steps | Natural language invocation |
| Menu System | `<menu>` with numbered commands | No menu, direct agent interaction |
| Workflows | workflow.yaml + instructions.md | Markdown documentation only |
| Config | config.yaml with variables | No config file |
| Templates | template.md with {{variables}} | Direct document creation |
| Validation | `<validation>` blocks | JavaScript validator module |
| State Management | Workflow engine | Task Master + workflow-state.json |
| Complexity | High (XML, YAML, templates) | Low (markdown + validation) |
| Learning Curve | Steep | Gentle |
| Enforcement | Structural | Runtime validation |

**Choose BMAD-TM Lite when:**
- You want simple, readable agents
- You need dependency enforcement
- You prefer Task Master for state
- You want quick setup and gentle learning curve

**Choose Full BMAD when:**
- You need menu-driven interfaces
- You want template-based document generation
- You have complex multi-agent coordination
- You need workflow engine features

---

## Troubleshooting

### Installation Issues

**"Task Master CLI not found"**
```bash
npm install -g task-master
```

**"Node.js not found"**
Install from: https://nodejs.org/

**"Validator not working"**
```bash
chmod +x src/validators/taskmaster-validator.js
node src/validators/taskmaster-validator.js --version
```

### Workflow Issues

**"Phase gate blocked"**
- Run `/status` to see current phase
- Complete previous phases before proceeding
- Check workflow-state.json if corrupted

**"Dependencies not met"**
- Run `task-master show [epic] [task-id]`
- Check `dependencies` field
- Complete dependency tasks first
- Or remove incorrect dependency

**"Tests failing"**
- Fix code or tests
- Developer agent blocks marking done until tests pass
- This is intentional - prevents bugs

**"Can't find epic"**
- Run `/status` to see active epic
- If none, run `/tm-pm` to create one
- Check `.taskmaster/tasks/tasks.json`

### State Issues

**"Workflow state corrupted"**
```bash
# Backup current state
cp .taskmaster/workflow-state.json .taskmaster/workflow-state.json.backup

# Reset to clean state
./install-claude-code.sh  # or install-opencode.sh
```

**"Task Master state corrupted"**
```bash
# Backup
cp .taskmaster/tasks/tasks.json .taskmaster/tasks/tasks.backup.json

# Check JSON validity
cat .taskmaster/tasks/tasks.json | jq .

# Manually fix or restore from backup
```

---

## FAQ

### Q: Can I skip the architecture phase?
**A:** No. Phase gates enforce the workflow. Architecture guides implementation and prevents rework.

### Q: Can I start a task with incomplete dependencies?
**A:** No. The Developer agent validates dependencies and blocks if any are not 'done'.

### Q: What if I disagree with a dependency?
**A:** Remove it: `task-master remove-dependency [epic] [task] [dep]`

### Q: Can I mark a task done without tests?
**A:** No. Agent boundaries enforce test-driven development. Tests must pass first.

### Q: Do I need to use story files?
**A:** No. Story files are eliminated. All context lives in Task Master's `details` field.

### Q: Can I work on multiple epics simultaneously?
**A:** Not recommended. BMAD-TM Lite tracks a single active epic for focus. Finish one before starting another.

### Q: How do I reset the workflow?
**A:** Run `/tm-retrospective` after completing an epic. It automatically resets to ideation.

### Q: Can I customize the agents?
**A:** Yes! Edit the markdown files in `.claude/commands/` or `.opencode/skills/`.

### Q: Do I need full BMAD installed?
**A:** No. BMAD-TM Lite is standalone. No BMAD installation required.

### Q: Can I use this with other AI tools?
**A:** The slash commands are for Claude Code CLI. For other tools, reference the agent markdown directly.

---

## Contributing

Contributions welcome! Please:

1. Read the workflow guide: `src/workflows/workflow-plan-and-build.md`
2. Understand the validator: `src/validators/taskmaster-validator.js`
3. Follow agent boundary patterns
4. Add tests for validation logic
5. Update documentation

---

## License

[Your License Here]

---

## Support

- **Documentation:** See `QUICKSTART.md` and `src/workflows/workflow-plan-and-build.md`
- **Issues:** Check troubleshooting section above
- **Questions:** Review FAQ or integration guide

---

## Credits

- **Task Master:** State management foundation
- **BMAD:** Conceptual inspiration for agent-based workflows
- **Claude Code CLI / OpenCode:** Agent invocation platforms

---

**Ready to build better software?** Run `./install-claude-code.sh` (or `./install-opencode.sh`) and start your first epic!

🚀 **Happy building!**
