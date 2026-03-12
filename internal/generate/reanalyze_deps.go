package generate

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/reuben/scud/internal/llm"
	"github.com/reuben/scud/internal/model"
	"github.com/reuben/scud/internal/storage"
)

// DepSuggestion represents an LLM-suggested dependency change.
type DepSuggestion struct {
	TaskID     string   `json:"task_id"`
	AddDeps    []string `json:"add_deps"`
	RemoveDeps []string `json:"remove_deps"`
	Reason     string   `json:"reason"`
}

// ReanalyzeDeps uses an LLM to analyze and suggest dependency changes.
func ReanalyzeDeps(ctx context.Context, caller llm.Caller, store *storage.Storage, tag string) ([]DepSuggestion, error) {
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

	prompt := ReanalyzeDependenciesPrompt(string(tasksJSON))

	var suggestions []DepSuggestion
	if err := caller.CompleteJSON(ctx, prompt, "", false, &suggestions); err != nil {
		return nil, fmt.Errorf("llm reanalyze-deps: %w", err)
	}

	return suggestions, nil
}

// ApplyDepSuggestions applies dependency suggestions to tasks in storage.
func ApplyDepSuggestions(store *storage.Storage, tag string, suggestions []DepSuggestion) error {
	return store.UpdatePhase(tag, func(p *model.Phase) error {
		taskMap := p.TaskMap()
		for _, s := range suggestions {
			t, ok := taskMap[s.TaskID]
			if !ok {
				continue
			}

			// Remove deps
			if len(s.RemoveDeps) > 0 {
				removeSet := make(map[string]bool)
				for _, d := range s.RemoveDeps {
					removeSet[d] = true
				}
				var kept []string
				for _, d := range t.Dependencies {
					if !removeSet[d] {
						kept = append(kept, d)
					}
				}
				t.Dependencies = kept
			}

			// Add deps (avoid duplicates)
			existing := make(map[string]bool)
			for _, d := range t.Dependencies {
				existing[d] = true
			}
			for _, d := range s.AddDeps {
				if !existing[d] {
					t.Dependencies = append(t.Dependencies, d)
				}
			}

			t.SetUpdatedNow()
		}
		return nil
	})
}
