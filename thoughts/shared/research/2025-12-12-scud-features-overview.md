---
date: 2025-12-12T16:01:21Z
researcher: Claude
git_commit: 9d5251d97ad9627b57192f337c259a1aed378179
branch: master
repository: scud
topic: "SCUD Features and Usage Overview for Presentation"
tags: [research, codebase, scud, presentation, features, commands, architecture]
status: complete
last_updated: 2025-12-12
last_updated_by: Claude
---

# Research: SCUD Features and Usage Overview

**Date**: 2025-12-12T16:01:21Z
**Researcher**: Claude
**Git Commit**: 9d5251d97ad9627b57192f337c259a1aed378179
**Branch**: master
**Repository**: scud

## Research Question

Review the SCUD codebase and features for a presentation on how to use it.

## Summary

SCUD (Simple Claude-optimized Universal DAG) is a fast, AI-powered task management system designed for software development workflows. It parses PRDs into tasks, tracks dependencies using a DAG (Directed Acyclic Graph), and enables parallel execution through wave-based planning. Built with a Rust core (~50x faster than JS alternatives), it's optimized for AI agent workflows with Claude Code integration.

---

## Core Concepts

### What SCUD Does

1. **Parses PRDs into structured tasks** - AI-powered conversion of natural language requirements
2. **Manages task dependencies as a DAG** - Prevents working on blocked tasks
3. **Computes parallel execution waves** - Identifies which tasks can run simultaneously
4. **Coordinates multiple agents** - Task claiming prevents conflicts
5. **Provides visualization** - Web dashboard with dependency diagrams

### The SCG Format

SCUD stores tasks in a token-efficient text format (~75% smaller than JSON):

```
# SCUD Graph v1
# Phase: auth

@nodes
auth:1 | Design auth system | X | 13 | H
auth:1.1 | Implement JWT tokens | D | 5 | H
auth:1.2 | Add refresh token flow | I | 3 | M
auth:2 | Rate limiting | P | 5 | M

@edges
auth:2 -> auth:1
auth:1.2 -> auth:1.1

@parents
auth:1: auth:1.1, auth:1.2
```

**Status codes**: P=Pending, I=InProgress, D=Done, R=Review, B=Blocked, F=Deferred, C=Cancelled, X=Expanded
**Priority codes**: C=Critical, H=High, M=Medium, L=Low

---

## Key Features

### 1. Tag-Based Organization

Tasks are grouped into "tags" (phases) for different features or workstreams:

```bash
scud tags                    # List all tags
scud tags auth-system        # Switch to auth-system tag
scud list --tag api          # List tasks in specific tag
```

### 2. Wave-Based Parallel Execution

SCUD uses Kahn's algorithm to compute which tasks can execute in parallel:

```bash
scud waves --tag auth-system
```

Output:
```
Wave 1 (3 tasks): auth:1, auth:3, auth:4
Wave 2 (2 tasks): auth:2, auth:5  # depends on wave 1
Wave 3 (1 task):  auth:6          # depends on wave 2
```

### 3. AI-Powered Task Creation

Parse PRDs or feature documents into structured tasks:

```bash
scud parse docs/feature.md --tag new-feature --num-tasks 10
```

The AI extracts:
- Task titles and descriptions
- Dependencies between tasks
- Complexity estimates (Fibonacci: 1,2,3,5,8,13,21...)
- Priority levels

### 4. Task Expansion

Break complex tasks into subtasks:

```bash
scud expand --task auth:1           # Expand specific task
scud expand --all                   # Expand all complex tasks
```

### 5. Orchestrator Support

Enable multiple Claude Code agents working in parallel:

```bash
scud next --tag auth                # Find next ready task
scud claim auth:2 --name agent-1    # Lock task
scud set-status auth:2 in-progress  # Start work
# ... do the work ...
scud set-status auth:2 done         # Complete (auto-releases)
```

### 6. Web Dashboard

Visual task board with dependency diagrams:

```bash
scud view                           # Generate and open HTML viewer
scud serve                          # Start live web server
```

Features:
- Interactive Mermaid diagrams
- Pan/zoom controls
- Per-phase visualizations
- Statistics dashboard

---

## CLI Commands Reference

### Core Commands (Instant, No API Required)

| Command | Description |
|---------|-------------|
| `scud init` | Initialize SCUD in current directory |
| `scud tags [tag]` | List tags or set active tag |
| `scud list [--status X]` | List tasks (filter by status) |
| `scud show <id>` | Show task details |
| `scud next` | Find next ready task |
| `scud set-status <id> <status>` | Update task status |
| `scud stats` | Show completion statistics |
| `scud waves` | Show parallel execution waves |

### AI Commands (Require API Key)

| Command | Description |
|---------|-------------|
| `scud parse <file> --tag <tag>` | Parse PRD into tasks |
| `scud expand [--task <id>]` | Break down complex tasks |
| `scud analyze-complexity` | Analyze task complexity |
| `scud reanalyze-deps` | Suggest cross-tag dependencies |

### Coordination Commands

| Command | Description |
|---------|-------------|
| `scud claim <id> --name <name>` | Lock task for agent |
| `scud release <id>` | Release task lock |
| `scud whois` | Show who's working on what |
| `scud doctor` | Check for stale locks |
| `scud next-batch --limit N` | Get multiple ready tasks |

### Utility Commands

| Command | Description |
|---------|-------------|
| `scud view` | Open web dashboard |
| `scud mermaid` | Generate dependency diagram |
| `scud log <id> "message"` | Add log entry to task |
| `scud commit` | Git commit with task context |
| `scud warmup` | Session orientation |

---

## Claude Code Integration

### Slash Commands

SCUD provides slash commands for Claude Code:

| Command | Description |
|---------|-------------|
| `/scud:list` | List tasks with status filter |
| `/scud:show <id>` | Show task details |
| `/scud:status <id> <status>` | Update task status |
| `/scud:waves` | Show execution waves |
| `/scud:next` | Find next ready task |
| `/scud:stats` | Show statistics |

### Skills System

SCUD registers as a skill that Claude Code can use automatically:

```
Skill: scud-tasks
- View, update, and track tasks in the SCUD graph system
```

---

## Typical Workflows

### 1. Starting a New Feature

```bash
# 1. Initialize (first time only)
scud init

# 2. Create tasks from PRD
scud parse docs/my-feature.md --tag my-feature

# 3. Review the plan
scud waves --tag my-feature
scud view

# 4. Start working
scud next --tag my-feature
```

### 2. Working on Tasks

```bash
# Find what's ready
scud next

# Start work
scud set-status feature:1 in-progress

# ... implement the task ...

# Complete
scud set-status feature:1 done

# Next task automatically unblocks
scud next
```

### 3. Parallel Agent Orchestration

```bash
# Agent 1
TASK=$(scud next --tag feature --spawn)
scud claim $TASK_ID --name agent-1
scud set-status $TASK_ID in-progress
# ... work ...
scud set-status $TASK_ID done

# Agent 2 (simultaneously)
TASK=$(scud next --tag feature --spawn)
scud claim $TASK_ID --name agent-2
# ... etc
```

---

## Architecture Overview

### Dual-Language Design

```
User → bin/scud.js (Node wrapper)
          ↓
       scud binary (Rust)
          ↓
       Command handlers → Storage → .scud/tasks/tasks.scg
```

### Key Directories

```
.scud/
├── tasks/
│   └── tasks.scg          # All tasks in SCG format
├── config.toml            # AI provider settings
└── active-tag             # Currently active tag

scud-cli/                  # Rust CLI source
├── src/
│   ├── main.rs           # Entry point
│   ├── commands/         # Command implementations
│   ├── models/           # Task, Phase data structures
│   ├── storage/          # File I/O with locking
│   ├── formats/          # SCG parser/serializer
│   └── llm/              # AI provider clients
```

### Supported AI Providers

- **xAI** (default) - `XAI_API_KEY`
- **Anthropic** - `ANTHROPIC_API_KEY`
- **OpenAI** - `OPENAI_API_KEY`
- **OpenRouter** - `OPENROUTER_API_KEY`

Configure with: `scud config set-provider <provider> --model <model>`

---

## Code References

- Entry point: `scud-cli/src/main.rs:54-456`
- Task model: `scud-cli/src/models/task.rs:71-110`
- Wave computation: `scud-cli/src/commands/waves.rs:201-278`
- SCG format: `scud-cli/src/formats/scg.rs:120-341`
- Storage layer: `scud-cli/src/storage/mod.rs:140-468`
- Documentation: `docs/reference/SCG_FORMAT_SPEC.md`
- Orchestrator guide: `docs/orchestrator.md`

---

## Open Questions

None - comprehensive documentation gathered for presentation purposes.
