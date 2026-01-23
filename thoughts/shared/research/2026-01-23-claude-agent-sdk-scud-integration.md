---
date: 2026-01-23T14:49:58-06:00
researcher: reuben
git_commit: 948eb8faffaa1e8891e03ced4ad40f25a549e6b4
branch: master
repository: scud
topic: "Claude Agent SDK and Tasks Integration with SCUD"
tags: [research, codebase, claude-agent-sdk, tasks, integration, swarm, subagents]
status: complete
last_updated: 2026-01-23
last_updated_by: reuben
---

# Research: Claude Agent SDK and Tasks Integration with SCUD

**Date**: 2026-01-23T14:49:58-06:00
**Researcher**: reuben
**Git Commit**: 948eb8faffaa1e8891e03ced4ad40f25a549e6b4
**Branch**: master
**Repository**: scud

## Research Question

How can SCUD integrate more deeply with Claude Code and the new Claude Agent SDK, particularly leveraging the new Tasks feature for cross-session coordination? What are the integration points and architectural patterns that would enable SCUD to be more than just a "separate terminal" experience?

## Summary

The Claude Agent SDK provides a programmatic interface to Claude Code's agentic capabilities, with built-in tools, hooks, sessions, and subagent coordination. The new **Tasks** feature (`~/.claude/tasks/`) enables Claude Code to coordinate work across multiple sessions and subagents via file-based task lists. This research identifies several integration opportunities:

1. **SCUD as a Task Backend**: Replace Claude Code's simple task storage with SCUD's DAG-based system
2. **Agent SDK Integration**: Use SCUD programmatically via the Agent SDK's custom tools and hooks
3. **Bidirectional Sync**: Mirror SCUD tasks to Claude Tasks for cross-tool coordination
4. **Subagent Orchestration**: Leverage Agent SDK's subagent primitives alongside SCUD's wave-based execution

## Detailed Findings

### Claude Agent SDK Architecture

The Claude Agent SDK (formerly Claude Code SDK) exposes Claude Code as a library for Python and TypeScript. Key capabilities:

#### Core API (`scud-cli/src/commands/spawn/terminal.rs:48-71` for comparison)

```python
from claude_agent_sdk import query, ClaudeAgentOptions

async for message in query(
    prompt="Fix the bug in auth.py",
    options=ClaudeAgentOptions(allowed_tools=["Read", "Edit", "Bash"])
):
    print(message)
```

SCUD currently spawns Claude Code via CLI command:
```bash
'claude' "$(cat prompt.txt)" --dangerously-skip-permissions --model <model>
```

**Integration Opportunity**: Replace CLI spawning with Agent SDK `query()` calls for tighter integration.

#### Built-in Tools

| Tool | Description | SCUD Equivalent |
|------|-------------|-----------------|
| Read | Read files | N/A (uses shell) |
| Write | Create files | N/A |
| Edit | Modify files | N/A |
| Bash | Run commands | N/A |
| Glob | Find files | N/A |
| Grep | Search content | N/A |
| WebSearch | Web queries | N/A |
| WebFetch | Fetch URLs | N/A |
| Task | Spawn subagents | `scud spawn` / `scud swarm` |
| AskUserQuestion | User input | N/A |
| **TaskCreate/TaskUpdate/TaskList** | Manage tasks | `scud` CLI commands |

The Task tools (TaskCreate, TaskUpdate, TaskList, TaskGet) are new primitives for Claude Code's task management.

### Claude Code Tasks System

The new Tasks feature stores tasks in `~/.claude/tasks/` and enables:

1. **Cross-session coordination**: Multiple Claude Code sessions can share a task list
2. **Subagent collaboration**: Subagents spawned via Task tool can read/update shared tasks
3. **Dependency tracking**: Tasks have `blockedBy` and `blocks` fields for dependencies
4. **File-based persistence**: JSON files allow external tools to read/write tasks

**Key Environment Variable**:
```bash
CLAUDE_CODE_TASK_LIST_ID=groceries claude
```

This sets a shared task list ID that multiple sessions use.

### SCUD's Current Architecture

SCUD stores tasks in `.scud/tasks/tasks.scg` using a custom format with:

- **Phases/Groups**: Logical groupings of tasks (like Claude's task lists)
- **DAG Dependencies**: Full dependency graph with cycle detection
- **Status Workflow**: Pending → InProgress → Done/Failed
- **File Locking**: fs2-based exclusive/shared locks for concurrent access
- **Wave Computation**: Kahn's algorithm for parallel execution waves

**Key Files**:
- `scud-core/src/storage.rs` - Task persistence
- `scud-core/src/models/task.rs` - Task data model
- `scud-core/src/waves.rs` - Wave computation
- `scud-cli/src/commands/swarm/mod.rs` - Swarm orchestration

### Integration Strategies

#### Strategy 1: SCUD as Claude Task Backend (Recommended)

**Concept**: Implement a bidirectional sync layer that maps SCUD tasks to Claude Tasks format.

**Implementation**:

1. Create a sync service that:
   - Watches `.scud/tasks/tasks.scg` for changes
   - Writes corresponding entries to `~/.claude/tasks/<tag>.json`
   - Maps SCUD statuses to Claude Task statuses

2. Hook into Claude Code's Task tools via SDK hooks:
   ```python
   async def scud_task_hook(input_data, tool_use_id, context):
       if input_data['tool_name'] == 'TaskUpdate':
           task_id = input_data['tool_input']['taskId']
           status = input_data['tool_input']['status']
           # Update SCUD task
           subprocess.run(['scud', 'set-status', task_id, status])
       return {}
   ```

**Benefits**:
- Claude Code agents see SCUD tasks natively
- SCUD's DAG and wave computation still drives execution
- No changes to SCUD's core task model

**Files to Modify**:
- New: `scud-cli/src/sync/claude_tasks.rs` - Sync service
- Modify: `scud-cli/src/commands/spawn/mod.rs` - Start sync when spawning

#### Strategy 2: Agent SDK Custom Tool

**Concept**: Register SCUD as a custom MCP server or tool provider for the Agent SDK.

**Implementation**:

```python
from claude_agent_sdk import query, ClaudeAgentOptions

options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Bash", "scud"],
    mcp_servers={
        "scud": {
            "command": "scud",
            "args": ["serve", "--mcp"]
        }
    }
)
```

SCUD would implement MCP endpoints:
- `scud_list` - List tasks
- `scud_show` - Show task details
- `scud_next` - Get next available task
- `scud_status` - Update task status
- `scud_log` - Add discovery log

**Benefits**:
- Claude Code agents call SCUD directly via MCP
- No sync layer needed
- SCUD becomes a first-class tool

**Files to Modify**:
- New: `scud-cli/src/commands/serve_mcp.rs` - MCP server mode
- Modify: `scud-cli/src/main.rs` - Add `serve --mcp` subcommand

#### Strategy 3: SDK-Native Swarm

**Concept**: Replace tmux-based spawning with Agent SDK's native subagent system.

**Current Flow** (`scud-cli/src/commands/swarm/mod.rs`):
1. Compute waves via Kahn's algorithm
2. For each wave, spawn tmux windows with Claude CLI
3. Poll task statuses until wave completes
4. Run validation (backpressure checks)
5. Continue to next wave

**SDK-Native Flow**:
```python
from claude_agent_sdk import query, ClaudeAgentOptions, AgentDefinition

async def run_wave(tasks):
    agents = {
        f"task-{task.id}": AgentDefinition(
            description=f"Complete task: {task.title}",
            prompt=generate_prompt(task),
            tools=["Read", "Edit", "Bash", "Grep", "Glob"]
        )
        for task in tasks
    }

    # All agents spawn in parallel via SDK
    async for message in query(
        prompt=f"Execute these {len(tasks)} tasks in parallel using the task-* agents",
        options=ClaudeAgentOptions(
            allowed_tools=["Task"],
            agents=agents
        )
    ):
        # Handle messages, detect completion
        pass
```

**Benefits**:
- Native parallelization via SDK
- Built-in context management
- No tmux dependency
- Session persistence/resume

**Challenges**:
- SDK doesn't expose wave-level control
- Subagents can't spawn subagents (max depth = 1)
- Less visibility into individual agent progress

#### Strategy 4: Hybrid Approach (Most Practical)

**Concept**: Keep SCUD's orchestration (swarm, waves) but enhance agent spawning with SDK features.

**Implementation**:

1. **SDK Spawning Mode**: Add `--sdk` flag to `scud spawn` and `scud swarm`:
   ```bash
   scud swarm --tag auth --sdk
   ```

2. **Hook Integration**: Install SCUD-aware hooks when spawning:
   ```python
   hooks = {
       'Stop': [HookMatcher(hooks=[scud_task_completion_hook])],
       'PostToolUse': [HookMatcher(matcher='Edit|Write', hooks=[scud_file_change_hook])]
   }
   ```

3. **Session Management**: Store session IDs for resumption:
   - Save session ID to `.scud/sessions/<task_id>.json`
   - Support `scud resume <task_id>` to continue work

4. **Discovery Logging**: Use SDK hooks to auto-log discoveries:
   ```python
   async def discovery_hook(input_data, tool_use_id, context):
       if input_data['tool_name'] == 'Read':
           file_path = input_data['tool_input']['file_path']
           subprocess.run(['scud', 'log', task_id, f'Read {file_path}'])
       return {}
   ```

**Files to Modify**:
- Modify: `scud-cli/src/commands/spawn/mod.rs` - Add SDK spawning path
- New: `scud-cli/src/sdk/mod.rs` - Agent SDK integration
- New: `scud-cli/src/sdk/hooks.rs` - SCUD-aware hooks
- Modify: `scud-cli/src/commands/swarm/mod.rs` - SDK execution mode

### Task Model Mapping

| SCUD Field | Claude Tasks Field | Notes |
|------------|-------------------|-------|
| `id` | `id` | Direct mapping |
| `title` | `subject` | Different names |
| `description` | `description` | Direct mapping |
| `status` (Pending) | `status` (pending) | Case difference |
| `status` (InProgress) | `status` (in_progress) | Underscore difference |
| `status` (Done) | `status` (completed) | Different terminology |
| `dependencies` | `blockedBy` | Array of task IDs |
| `parent_id` | N/A | SCUD subtask system |
| `complexity` | N/A | SCUD-specific |
| `agent_type` | N/A | SCUD-specific |
| `details` | N/A | SCUD extended info |

### Environment Variable Alignment

Claude Code uses:
```bash
CLAUDE_CODE_TASK_LIST_ID=<list-id>
```

SCUD could add compatibility:
```bash
SCUD_TAG=<tag>  # Existing
CLAUDE_CODE_TASK_LIST_ID=$(scud get-task-list-id)  # Bridge
```

When both are set, SCUD sync service maintains consistency.

### Session Coordination

Claude Agent SDK sessions support:
- `session_id` capture from init message
- `resume` option to continue sessions
- `forkSession` to branch conversations

SCUD could track sessions per task:
```
.scud/sessions/
├── auth:1.json    # { session_id: "...", task_id: "auth:1", started_at: "..." }
├── auth:2.json
└── ...
```

New commands:
- `scud resume <task_id>` - Resume a task's Claude session
- `scud sessions` - List active sessions
- `scud sessions --cleanup` - Remove stale sessions

### Hook Installation Enhancement

Currently SCUD installs a Stop hook via `.claude/settings.local.json`:
```json
{
  "hooks": {
    "Stop": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "bash -c 'scud set-status \"$SCUD_TASK_ID\" done'"
      }]
    }]
  }
}
```

With SDK hooks, this becomes programmatic:
```python
async def scud_stop_hook(input_data, tool_use_id, context):
    task_id = os.environ.get('SCUD_TASK_ID')
    if task_id:
        subprocess.run(['scud', 'set-status', task_id, 'done'])
    return {}
```

**Benefits**:
- Richer context (full input_data)
- Can make decisions based on agent output
- Async operations supported

### Subagent Patterns

The SDK's subagent system maps well to SCUD's task hierarchy:

**SCUD Subtasks** (`scud-core/src/models/task.rs:91`):
```rust
pub subtasks: Vec<String>,  // Child task IDs
pub parent_id: Option<String>,  // Parent task ID
```

**SDK Subagents**:
```python
agents = {
    "subtask-1.1": AgentDefinition(
        description="Subtask 1.1 of task 1",
        prompt=subtask_prompt,
        tools=["Read", "Edit"]
    )
}
```

When a parent task needs expansion, SCUD could:
1. Create subtasks in SCUD (`ai::expand`)
2. Register subtasks as SDK subagents
3. Let the parent agent delegate via Task tool
4. Track completion via SubagentStop hooks

## Code References

### SCUD Core
- `scud-core/src/models/task.rs:74-118` - Task struct
- `scud-core/src/models/task.rs:5-16` - TaskStatus enum
- `scud-core/src/storage.rs:21-40` - Storage struct
- `scud-core/src/waves.rs:30-80` - Wave computation

### SCUD Spawn/Swarm
- `scud-cli/src/commands/spawn/mod.rs:36-298` - Spawn command
- `scud-cli/src/commands/spawn/terminal.rs:214-343` - Tmux spawning
- `scud-cli/src/commands/swarm/mod.rs:53-586` - Swarm orchestration
- `scud-cli/src/commands/spawn/hooks.rs:53-100` - Hook installation

### Agent Definition
- `scud-cli/src/agents/mod.rs:14-48` - AgentDef struct
- `scud-cli/src/commands/spawn/agent.rs:38-90` - Agent resolution

## Architecture Documentation

### Current SCUD → Claude Code Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ scud swarm  │────>│ tmux spawn  │────>│ claude CLI  │
└─────────────┘     └─────────────┘     └─────────────┘
      │                                        │
      │  Status polling                        │  Stop hook
      │<───────────────────────────────────────│
      │                                        │
      v                                        v
┌─────────────┐                         ┌─────────────┐
│ .scud/tasks │                         │ File system │
└─────────────┘                         └─────────────┘
```

### Proposed SDK-Integrated Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ scud swarm  │────>│ Agent SDK   │────>│ Claude API  │
│  --sdk      │     │   query()   │     │             │
└─────────────┘     └─────────────┘     └─────────────┘
      │                   │                    │
      │                   │  SDK Hooks         │
      │<──────────────────│<───────────────────│
      │                   │                    │
      v                   v                    v
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ .scud/tasks │<───>│ ~/.claude/  │     │ File system │
│             │     │   tasks/    │     │             │
└─────────────┘     └─────────────┘     └─────────────┘
         Bidirectional sync
```

### Task Storage Comparison

**SCUD (.scud/tasks/tasks.scg)**:
```
# SCUD Graph v1
# Phase: auth

@meta {
  name auth
  id_format sequential
}

@nodes
# id | title | status | complexity | priority
1 | Implement login | I | 5 | M
2 | Add logout | P | 3 | M

@edges
2 -> 1
```

**Claude Tasks (~/.claude/tasks/<list-id>.json)**:
```json
{
  "tasks": [
    {
      "id": "1",
      "subject": "Implement login",
      "description": "...",
      "status": "in_progress",
      "blockedBy": []
    },
    {
      "id": "2",
      "subject": "Add logout",
      "description": "...",
      "status": "pending",
      "blockedBy": ["1"]
    }
  ]
}
```

## Related Research

- [thoughts/shared/research/2025-12-02-anthropic-long-running-agent-comparison.md](./2025-12-02-anthropic-long-running-agent-comparison.md) - Earlier comparison of agent frameworks
- [thoughts/shared/research/2026-01-23-opencode-sdk-deep-integration.md](./2026-01-23-opencode-sdk-deep-integration.md) - OpenCode integration research
- [thoughts/shared/research/2025-11-27-scud-vs-beads-comparison.md](./2025-11-27-scud-vs-beads-comparison.md) - Comparison with Beads (mentioned in Claude Tasks announcement)

## Open Questions

1. **Task List ID Generation**: Should SCUD tags map 1:1 to Claude task list IDs, or should there be a namespace prefix?

2. **Conflict Resolution**: When both SCUD and Claude Code modify the same task, which wins? Options:
   - Last-write-wins (simple)
   - SCUD as source of truth (for DAG integrity)
   - Merge with conflict markers

3. **Session Persistence**: Should SCUD store Claude session IDs for resumption? This enables:
   - `scud resume auth:1` - Continue a task's session
   - `scud fork auth:1` - Branch from a task's state

4. **MCP vs Direct Integration**: Is MCP server mode worth implementing, or is direct SDK integration sufficient?

5. **Wave-Level Coordination**: Can SDK subagents be used for wave execution, or is the 1-level nesting limit too restrictive?

6. **Backpressure Integration**: Should validation failures be surfaced to Claude via hooks (systemMessage) so it can self-correct?

## Recommendations

1. **Start with Hybrid Approach (Strategy 4)**: Add `--sdk` flag to spawn/swarm without breaking existing tmux flow

2. **Implement Task Sync Service**: Bidirectional sync between `.scud/tasks/` and `~/.claude/tasks/` enables Claude to see SCUD tasks natively

3. **Enhance Hook Integration**: Move from shell-based hooks to SDK programmatic hooks for richer context

4. **Track Sessions**: Store Claude session IDs per task for resumption support

5. **Consider MCP Server**: Long-term, implementing SCUD as an MCP server gives maximum flexibility
