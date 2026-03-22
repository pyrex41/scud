package storage

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/reuben/scud/internal/model"
	"github.com/reuben/scud/internal/scg"
)

type Storage struct {
	root string
}

func New(root string) *Storage {
	return &Storage{root: root}
}

// FindRoot walks up from dir looking for .scud/ directory.
func FindRoot(dir string) (string, error) {
	for {
		if _, err := os.Stat(filepath.Join(dir, ".scud")); err == nil {
			return dir, nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return "", fmt.Errorf("no .scud directory found (run 'scud init')")
		}
		dir = parent
	}
}

func (s *Storage) Root() string { return s.root }

func (s *Storage) ScudDir() string {
	return filepath.Join(s.root, ".scud")
}

func (s *Storage) TasksFile() string {
	return filepath.Join(s.root, ".scud", "tasks", "tasks.scg")
}

func (s *Storage) ConfigFile() string {
	return filepath.Join(s.root, ".scud", "config.toml")
}

func (s *Storage) ActiveTagFile() string {
	return filepath.Join(s.root, ".scud", "active-tag")
}

func (s *Storage) GuidanceDir() string {
	return filepath.Join(s.root, ".scud", "guidance")
}

// Initialize creates the .scud directory structure and scaffolds skills.
func (s *Storage) Initialize() error {
	dirs := []string{
		filepath.Join(s.root, ".scud"),
		filepath.Join(s.root, ".scud", "tasks"),
		filepath.Join(s.root, ".scud", "guidance"),
		filepath.Join(s.root, ".scud", "archive"),
	}
	for _, d := range dirs {
		if err := os.MkdirAll(d, 0755); err != nil {
			return fmt.Errorf("creating %s: %w", d, err)
		}
	}
	// Create empty tasks file if not exists
	tf := s.TasksFile()
	if _, err := os.Stat(tf); os.IsNotExist(err) {
		if err := os.WriteFile(tf, []byte(""), 0644); err != nil {
			return err
		}
	}
	// Scaffold skills for Claude Code and OpenCode
	if err := s.scaffoldSkills(); err != nil {
		return fmt.Errorf("scaffolding skills: %w", err)
	}
	return nil
}

func (s *Storage) scaffoldSkills() error {
	skills := map[string]string{
		"scud":       skillScudGuide,
		"scud-tasks": skillScudTasks,
	}
	// Write to both .claude/skills/ and .opencode/skills/
	for _, prefix := range []string{".claude", ".opencode"} {
		for name, content := range skills {
			dir := filepath.Join(s.root, prefix, "skills", name)
			path := filepath.Join(dir, "SKILL.md")
			// Don't overwrite existing skills
			if _, err := os.Stat(path); err == nil {
				continue
			}
			if err := os.MkdirAll(dir, 0755); err != nil {
				return err
			}
			if err := os.WriteFile(path, []byte(content), 0644); err != nil {
				return err
			}
		}
	}
	return nil
}

var skillScudGuide = `---
name: scud-guide
description: SCUD CLI reference and workflow guide. Use when working with scud task management, running scud commands, or when the user mentions tasks, waves, DAG, or project progress.
---

# SCUD CLI Guide

SCUD is a DAG-based task manager for AI-driven development. Tasks have dependencies, priorities, and complexity scores. Work flows through parallel waves.

## Session Workflow

` + "```" + `bash
scud warmup              # Orient: status, git history, next task
scud next                # Find next available task (deps satisfied)
scud set-status ID in-progress
# ... do the work ...
scud commit -m "message" # Auto-prefixes [TASK-ID]
scud set-status ID done
scud stats               # Check progress
` + "```" + `

## Commands

| Category | Command | Description |
|----------|---------|-------------|
| **Session** | ` + "`scud warmup`" + ` | Orient with status + next task |
| **View** | ` + "`scud list [--status pending]`" + ` | List tasks |
| | ` + "`scud show ID`" + ` | Task details |
| | ` + "`scud stats`" + ` | Completion statistics |
| **Work** | ` + "`scud next`" + ` | Next ready task |
| | ` + "`scud waves`" + ` | Parallel execution waves |
| | ` + "`scud set-status ID STATUS`" + ` | Update status |
| | ` + "`scud create --title \"...\"`" + ` | Create a task |
| **Git** | ` + "`scud commit -m \"msg\"`" + ` | [TASK-ID] prefixed commit |
| **AI** | ` + "`scud parse FILE`" + ` | Generate tasks from doc |
| | ` + "`scud expand ID`" + ` | Break into subtasks |
| | ` + "`scud heavy \"query\"`" + ` | Multi-agent reasoning ensemble |
| **Tags** | ` + "`scud tags`" + ` | List/switch phases |
| **Server** | ` + "`scud mcp-server`" + ` | Start MCP server for tool integration |

## Heavy Ensemble

Multi-agent reasoning with per-role model control:

` + "```" + `bash
scud heavy "query" -v                                    # Default
scud heavy "query" --model-agents grok-4.1-fast          # Cheap agents
scud heavy "query" --mode hybrid                         # Local + web research
` + "```" + `

Modes: ensemble (default), native (xAI multi-agent), hybrid (both).

## MCP Server

Expose scud as tools for Cowork/Claude Code:

` + "```" + `json
{"mcpServers": {"scud": {"command": "scud", "args": ["mcp-server"]}}}
` + "```" + `

Tiers via SCUD_TOOLS: core (default, 6 tools), full (9 tools), or custom comma-separated list.

## Task Statuses

pending | in-progress | done | blocked | failed | review | expanded | deferred | cancelled
`

var skillScudTasks = `---
name: scud-tasks
description: SCUD task management - view, update, and track tasks in the SCUD DAG system. Use when the user asks about tasks, wants to see progress, needs the next task, or wants to update task status.
---

# SCUD Task Management

SCUD organizes work as a DAG of tasks with dependencies, priorities, and complexity scores.

## Quick Reference

` + "```" + `bash
scud warmup                        # Session start: status + next task
scud next                          # Next available task (all deps done)
scud show <id>                     # Full task details
scud list                          # All tasks in active tag
scud list --status pending         # Filter by status
scud set-status <id> in-progress   # Start working
scud set-status <id> done          # Mark complete
scud stats                         # Completion statistics
scud waves                         # View parallel execution plan
scud create --title "..."          # Create a new task
` + "```" + `

## Workflow

1. **Orient**: ` + "`scud warmup`" + ` - see project state and what's next
2. **Claim**: ` + "`scud set-status <id> in-progress`" + `
3. **Implement**: do the work
4. **Commit**: ` + "`scud commit -m \"message\"`" + ` - auto-prefixes [TASK-ID]
5. **Complete**: ` + "`scud set-status <id> done`" + ` - unblocks dependent tasks
6. **Repeat**: ` + "`scud next`" + `

## Task Statuses

| Status | Meaning |
|--------|---------|
| pending | Ready to start (or waiting on deps) |
| in-progress | Currently being worked on |
| done | Completed and verified |
| blocked | Cannot proceed (external blocker) |
| failed | Attempted but failed |
| review | Ready for review |
| expanded | Decomposed into subtasks |

## Dependencies & Waves

Tasks depend on other tasks. A task is "ready" when status is pending and all deps are done.
` + "`scud waves`" + ` groups ready tasks into parallel waves. ` + "`scud next`" + ` returns the highest-priority ready task.

## Tags (Phases)

` + "```" + `bash
scud tags              # List all tags
scud tags <name>       # Set active tag
` + "```" + `

## Task IDs

Hierarchical: 1, 1.1, 1.1.1. Subtasks inherit parent dependencies.
`

// LoadPhases reads and parses all phases from the tasks file.
func (s *Storage) LoadPhases() (map[string]*model.Phase, error) {
	f, err := os.OpenFile(s.TasksFile(), os.O_RDONLY|os.O_CREATE, 0644)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	if err := lockShared(f); err != nil {
		return nil, fmt.Errorf("acquiring read lock: %w", err)
	}
	defer unlock(f)

	content, err := io.ReadAll(f)
	if err != nil {
		return nil, err
	}
	return scg.ParseMultiPhase(string(content)), nil
}

// SavePhases writes all phases to the tasks file atomically.
func (s *Storage) SavePhases(phases map[string]*model.Phase) error {
	f, err := os.OpenFile(s.TasksFile(), os.O_RDWR|os.O_CREATE, 0644)
	if err != nil {
		return err
	}
	defer f.Close()

	if err := lockExclusive(f); err != nil {
		return fmt.Errorf("acquiring write lock: %w", err)
	}
	defer unlock(f)

	content := scg.SerializeMultiPhase(phases)
	if err := f.Truncate(0); err != nil {
		return err
	}
	if _, err := f.Seek(0, 0); err != nil {
		return err
	}
	_, err = f.WriteString(content)
	return err
}

// UpdatePhase performs an atomic read-modify-write on a single phase.
func (s *Storage) UpdatePhase(tag string, fn func(*model.Phase) error) error {
	f, err := os.OpenFile(s.TasksFile(), os.O_RDWR|os.O_CREATE, 0644)
	if err != nil {
		return err
	}
	defer f.Close()

	if err := lockExclusive(f); err != nil {
		return fmt.Errorf("acquiring write lock: %w", err)
	}
	defer unlock(f)

	content, err := io.ReadAll(f)
	if err != nil {
		return err
	}

	phases := scg.ParseMultiPhase(string(content))
	phase, ok := phases[tag]
	if !ok {
		phase = &model.Phase{Name: tag, IDFormat: "sequential"}
		phases[tag] = phase
	}

	if err := fn(phase); err != nil {
		return err
	}

	output := scg.SerializeMultiPhase(phases)
	if err := f.Truncate(0); err != nil {
		return err
	}
	if _, err := f.Seek(0, 0); err != nil {
		return err
	}
	_, err = f.WriteString(output)
	return err
}

// ActiveTag returns the current active tag.
func (s *Storage) ActiveTag() string {
	data, err := os.ReadFile(s.ActiveTagFile())
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}

// SetActiveTag sets the current active tag.
func (s *Storage) SetActiveTag(tag string) error {
	return os.WriteFile(s.ActiveTagFile(), []byte(tag+"\n"), 0644)
}

// LoadGuidance concatenates all .md files from the guidance directory.
func (s *Storage) LoadGuidance() string {
	entries, err := os.ReadDir(s.GuidanceDir())
	if err != nil {
		return ""
	}
	var files []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), ".md") {
			files = append(files, e.Name())
		}
	}
	sort.Strings(files)

	var b strings.Builder
	for _, f := range files {
		data, err := os.ReadFile(filepath.Join(s.GuidanceDir(), f))
		if err != nil {
			continue
		}
		if b.Len() > 0 {
			b.WriteString("\n\n")
		}
		b.Write(data)
	}
	return b.String()
}

// ResolveTag returns the given tag, or the active tag, or the sole phase, or an error.
// It also persists the resolved tag as the active tag for future commands.
func (s *Storage) ResolveTag(tag string) (string, error) {
	if tag != "" {
		s.SetActiveTag(tag)
		return tag, nil
	}
	if active := s.ActiveTag(); active != "" {
		return active, nil
	}
	// If only one phase exists, use it automatically
	phases, err := s.LoadPhases()
	if err == nil && len(phases) == 1 {
		for name := range phases {
			s.SetActiveTag(name)
			return name, nil
		}
	}
	return "", fmt.Errorf("no tag specified and no active tag set (use -t or 'scud tags <name>')")
}

