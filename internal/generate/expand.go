package generate

import (
	"context"
	"fmt"
	"time"

	"golang.org/x/sync/errgroup"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/model"
	"github.com/reuben/scud/internal/rho"
	"github.com/reuben/scud/internal/storage"
)

type expandedSubtask struct {
	Title        string   `json:"title"`
	Description  string   `json:"description"`
	Priority     string   `json:"priority"`
	Dependencies []any    `json:"dependencies"`
}

// Expand expands complex tasks into subtasks.
// If taskID is empty, expands all tasks that need expansion.
func Expand(ctx context.Context, cfg *config.Config, store *storage.Storage, tag, taskID string) error {
	phases, err := store.LoadPhases()
	if err != nil {
		return err
	}
	phase, ok := phases[tag]
	if !ok {
		return fmt.Errorf("tag '%s' not found", tag)
	}

	var toExpand []*model.Task
	if taskID != "" {
		t := phase.FindTask(taskID)
		if t == nil {
			return fmt.Errorf("task '%s' not found in tag '%s'", taskID, tag)
		}
		if !t.NeedsExpansion() {
			return fmt.Errorf("task '%s' does not need expansion (complexity=%d, status=%s)", taskID, t.Complexity, t.Status)
		}
		toExpand = []*model.Task{t}
	} else {
		for _, t := range phase.Tasks {
			if t.NeedsExpansion() {
				toExpand = append(toExpand, t)
			}
		}
	}

	if len(toExpand) == 0 {
		fmt.Println("No tasks need expansion.")
		return nil
	}

	fmt.Printf("Expanding %d task(s)...\n", len(toExpand))

	guidance := store.LoadGuidance()

	// Process up to 5 concurrent expansions
	type expandResult struct {
		parentID string
		subtasks []expandedSubtask
	}
	results := make([]expandResult, len(toExpand))

	g, gctx := errgroup.WithContext(ctx)
	g.SetLimit(5)

	for i, t := range toExpand {
		i, t := i, t
		g.Go(func() error {
			recommended := t.RecommendedSubtasks()
			prompt := ExpandTaskPrompt(t.Title, t.Description, t.Complexity, recommended, t.Details, guidance)
			subs, err := rho.RunJSON[[]expandedSubtask](gctx, rho.Options{
				Prompt: prompt,
				Model:  cfg.Rho.FastModel,
			})
			if err != nil {
				return fmt.Errorf("expanding task %s: %w", t.ID, err)
			}
			results[i] = expandResult{parentID: t.ID, subtasks: subs}
			fmt.Printf("  Expanded task %s into %d subtasks\n", t.ID, len(subs))
			return nil
		})
	}

	if err := g.Wait(); err != nil {
		return err
	}

	// Apply all expansions
	return store.UpdatePhase(tag, func(p *model.Phase) error {
		now := time.Now().UTC().Format(time.RFC3339)
		for _, r := range results {
			if r.parentID == "" {
				continue
			}
			parent := p.FindTask(r.parentID)
			if parent == nil {
				continue
			}

			var subtaskIDs []string
			for j, sub := range r.subtasks {
				subID := fmt.Sprintf("%s.%d", r.parentID, j+1)
				subtaskIDs = append(subtaskIDs, subID)

				st := &model.Task{
					ID:          subID,
					Title:       sub.Title,
					Description: sub.Description,
					Status:      model.Pending,
					Complexity:  0,
					Priority:    parsePriority(sub.Priority),
					ParentID:    r.parentID,
					AgentType:   parent.AgentType,
					ModelTier:   parent.ModelTier,
					CreatedAt:   now,
					UpdatedAt:   now,
				}

				// Remap subtask dependencies
				for _, dep := range sub.Dependencies {
					depStr := fmt.Sprintf("%v", dep)
					// 1-indexed subtask ref -> parentID.N
					if isNumeric(depStr) {
						depStr = fmt.Sprintf("%s.%s", r.parentID, depStr)
					}
					st.Dependencies = append(st.Dependencies, depStr)
				}

				p.Tasks = append(p.Tasks, st)
			}

			parent.Status = model.Expanded
			parent.Subtasks = subtaskIDs
			parent.UpdatedAt = now
		}
		return nil
	})
}

func isNumeric(s string) bool {
	for _, c := range s {
		if c < '0' || c > '9' {
			return false
		}
	}
	return len(s) > 0
}
