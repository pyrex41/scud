package generate

import (
	"fmt"
	"strings"

	"github.com/reuben/scud/internal/model"
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
