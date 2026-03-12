package generate

import (
	"context"
	"fmt"
	"time"

	"golang.org/x/sync/errgroup"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/llm"
	"github.com/reuben/scud/internal/model"
	"github.com/reuben/scud/internal/rho"
	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/internal/ui"
)

type expandedSubtask struct {
	Title        string   `json:"title"`
	Description  string   `json:"description"`
	Priority     string   `json:"priority"`
	Dependencies []any    `json:"dependencies"`
}

// Expand expands complex tasks into subtasks.
// If taskID is empty, expands all tasks that need expansion.
// An optional llm.Caller can be passed to use direct LLM calls instead of rho.
func Expand(ctx context.Context, cfg *config.Config, store *storage.Storage, tag, taskID string, caller ...llm.Caller) error {
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
		ui.Info("No tasks need expansion.")
		return nil
	}

	guidance := store.LoadGuidance()

	pb := ui.NewProgressBar(len(toExpand), "tasks expanded")

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

			var subs []expandedSubtask
			if len(caller) > 0 && caller[0] != nil {
				if err := caller[0].CompleteJSON(gctx, prompt, "", true, &subs); err != nil {
					pb.Increment(fmt.Sprintf("Task %s: %s", t.ID, t.Title), false)
					return fmt.Errorf("expanding task %s: %w", t.ID, err)
				}
			} else {
				var err error
				subs, err = rho.RunJSON[[]expandedSubtask](gctx, rho.Options{
					Prompt: prompt,
					Model:  cfg.Rho.FastModel,
				})
				if err != nil {
					pb.Increment(fmt.Sprintf("Task %s: %s", t.ID, t.Title), false)
					return fmt.Errorf("expanding task %s: %w", t.ID, err)
				}
			}
			results[i] = expandResult{parentID: t.ID, subtasks: subs}
			pb.Increment(fmt.Sprintf("Task %s → %d subtasks: %s", t.ID, len(subs), t.Title), true)
			return nil
		})
	}

	if err := g.Wait(); err != nil {
		return err
	}
	pb.Finish()

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
