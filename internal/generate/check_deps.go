package generate

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/reuben/scud/internal/llm"
	"github.com/reuben/scud/internal/model"
	"github.com/reuben/scud/internal/storage"
)

// CheckDepsResult holds the results of dependency validation.
type CheckDepsResult struct {
	Cycles     [][]string
	MissingDeps []MissingDep
	SelfRefs   []string
	OK         bool
}

type MissingDep struct {
	TaskID string
	DepID  string
}

// CheckDeps performs structural validation of task dependencies.
func CheckDeps(phases map[string]*model.Phase, tag string) *CheckDepsResult {
	result := &CheckDepsResult{OK: true}

	phase, ok := phases[tag]
	if !ok {
		return result
	}

	taskMap := phase.TaskMap()

	// Also build cross-phase task set
	allTasks := make(map[string]bool)
	for t, p := range phases {
		for _, task := range p.Tasks {
			allTasks[task.ID] = true
			allTasks[fmt.Sprintf("%s:%s", t, task.ID)] = true
		}
	}

	// Check self-references
	for _, t := range phase.Tasks {
		for _, dep := range t.Dependencies {
			if dep == t.ID {
				result.SelfRefs = append(result.SelfRefs, t.ID)
				result.OK = false
			}
		}
	}

	// Check missing dependencies
	for _, t := range phase.Tasks {
		for _, dep := range t.Dependencies {
			if !allTasks[dep] {
				// Check if it's in the same phase
				if _, ok := taskMap[dep]; !ok {
					result.MissingDeps = append(result.MissingDeps, MissingDep{
						TaskID: t.ID,
						DepID:  dep,
					})
					result.OK = false
				}
			}
		}
	}

	// Check cycles via DFS
	cycles := detectCycles(phase.Tasks)
	if len(cycles) > 0 {
		result.Cycles = cycles
		result.OK = false
	}

	return result
}

func detectCycles(tasks []*model.Task) [][]string {
	taskMap := make(map[string]*model.Task)
	for _, t := range tasks {
		taskMap[t.ID] = t
	}

	visited := make(map[string]int) // 0=unvisited, 1=in-path, 2=done
	var cycles [][]string

	var dfs func(id string, path []string)
	dfs = func(id string, path []string) {
		if visited[id] == 2 {
			return
		}
		if visited[id] == 1 {
			// Found cycle - extract it
			start := -1
			for i, p := range path {
				if p == id {
					start = i
					break
				}
			}
			if start >= 0 {
				cycle := make([]string, len(path)-start)
				copy(cycle, path[start:])
				cycles = append(cycles, cycle)
			}
			return
		}
		visited[id] = 1
		if t, ok := taskMap[id]; ok {
			for _, dep := range t.Dependencies {
				dfs(dep, append(path, id))
			}
		}
		visited[id] = 2
	}

	for _, t := range tasks {
		if visited[t.ID] == 0 {
			dfs(t.ID, nil)
		}
	}
	return cycles
}

// CoverageResult holds the results of LLM-powered PRD coverage analysis.
type CoverageResult struct {
	CoverageScore       int      `json:"coverage_score"`
	MissingRequirements []string `json:"missing_requirements"`
	IncompleteCoverage  []string `json:"incomplete_coverage"`
	MisalignedTasks     []string `json:"misaligned_tasks"`
	Summary             string   `json:"summary"`
}

// CheckDepsWithPRD validates tasks against a PRD using an LLM.
func CheckDepsWithPRD(ctx context.Context, caller llm.Caller, store *storage.Storage, tag, prdContent string) (*CoverageResult, error) {
	phases, err := store.LoadPhases()
	if err != nil {
		return nil, fmt.Errorf("loading phases: %w", err)
	}
	phase, ok := phases[tag]
	if !ok {
		return nil, fmt.Errorf("tag '%s' not found", tag)
	}

	tasksJSON, err := json.Marshal(phase.Tasks)
	if err != nil {
		return nil, fmt.Errorf("serializing tasks: %w", err)
	}

	prompt := ValidateTasksAgainstPRD(prdContent, string(tasksJSON))

	var result CoverageResult
	if err := caller.CompleteJSON(ctx, prompt, "", false, &result); err != nil {
		return nil, fmt.Errorf("llm validate: %w", err)
	}

	return &result, nil
}

// fixSuggestion represents LLM-suggested fixes for PRD coverage issues.
type fixSuggestion struct {
	UpdateTasks []struct {
		ID          string `json:"id"`
		Title       string `json:"title,omitempty"`
		Description string `json:"description,omitempty"`
	} `json:"update_tasks"`
	AddTasks []struct {
		Title        string   `json:"title"`
		Description  string   `json:"description"`
		Priority     string   `json:"priority"`
		Complexity   int      `json:"complexity"`
		Dependencies []string `json:"dependencies"`
		AgentType    string   `json:"agent_type"`
	} `json:"add_tasks"`
}

// FixPRDIssues uses an LLM to suggest and apply fixes for PRD coverage issues.
func FixPRDIssues(ctx context.Context, caller llm.Caller, store *storage.Storage, tag, prdContent string, issues *CoverageResult) error {
	phases, err := store.LoadPhases()
	if err != nil {
		return fmt.Errorf("loading phases: %w", err)
	}
	phase, ok := phases[tag]
	if !ok {
		return fmt.Errorf("tag '%s' not found", tag)
	}

	tasksJSON, err := json.Marshal(phase.Tasks)
	if err != nil {
		return fmt.Errorf("serializing tasks: %w", err)
	}

	issuesJSON, err := json.Marshal(issues)
	if err != nil {
		return fmt.Errorf("serializing issues: %w", err)
	}

	prompt := FixPRDIssuesPrompt(prdContent, string(tasksJSON), string(issuesJSON))

	var fixes fixSuggestion
	if err := caller.CompleteJSON(ctx, prompt, "", false, &fixes); err != nil {
		return fmt.Errorf("llm fix: %w", err)
	}

	return store.UpdatePhase(tag, func(p *model.Phase) error {
		// Apply updates to existing tasks
		for _, upd := range fixes.UpdateTasks {
			t := p.FindTask(upd.ID)
			if t == nil {
				continue
			}
			if upd.Title != "" {
				t.Title = upd.Title
			}
			if upd.Description != "" {
				t.Description = upd.Description
			}
			t.SetUpdatedNow()
		}

		// Add new tasks
		for _, add := range fixes.AddTasks {
			id := p.NextTaskID()
			t := &model.Task{
				ID:           id,
				Title:        add.Title,
				Description:  add.Description,
				Status:       model.Pending,
				Complexity:   add.Complexity,
				Priority:     parsePriority(add.Priority),
				Dependencies: add.Dependencies,
			}
			if add.AgentType != "" {
				t.AgentType = model.AgentType(add.AgentType)
			}
			t.AutoAssignAgent()
			p.Tasks = append(p.Tasks, t)
		}
		return nil
	})
}

// FormatCoverageResult returns a human-readable string of coverage results.
func FormatCoverageResult(r *CoverageResult) string {
	var parts []string
	parts = append(parts, fmt.Sprintf("Coverage Score: %d/100", r.CoverageScore))
	if len(r.MissingRequirements) > 0 {
		parts = append(parts, fmt.Sprintf("Missing Requirements:\n  - %s", strings.Join(r.MissingRequirements, "\n  - ")))
	}
	if len(r.IncompleteCoverage) > 0 {
		parts = append(parts, fmt.Sprintf("Incomplete Coverage:\n  - %s", strings.Join(r.IncompleteCoverage, "\n  - ")))
	}
	if len(r.MisalignedTasks) > 0 {
		parts = append(parts, fmt.Sprintf("Misaligned Tasks:\n  - %s", strings.Join(r.MisalignedTasks, "\n  - ")))
	}
	if r.Summary != "" {
		parts = append(parts, fmt.Sprintf("Summary: %s", r.Summary))
	}
	return strings.Join(parts, "\n\n")
}

// FormatCheckResult returns a human-readable string of check results.
func FormatCheckResult(r *CheckDepsResult) string {
	if r.OK {
		return "All dependency checks passed."
	}

	var parts []string
	if len(r.SelfRefs) > 0 {
		parts = append(parts, fmt.Sprintf("Self-references: %s", strings.Join(r.SelfRefs, ", ")))
	}
	for _, m := range r.MissingDeps {
		parts = append(parts, fmt.Sprintf("Missing dep: task %s depends on %s (not found)", m.TaskID, m.DepID))
	}
	for _, c := range r.Cycles {
		parts = append(parts, fmt.Sprintf("Cycle: %s", strings.Join(c, " -> ")))
	}
	return strings.Join(parts, "\n")
}
