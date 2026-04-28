package generate

import (
	"context"
	"fmt"
	"os"
	"time"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/rho"
	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/pkg/llm"
	"github.com/reuben/scud/pkg/model"
)

type parsedTask struct {
	Title        string `json:"title"`
	Description  string `json:"description"`
	Priority     string `json:"priority"`
	Complexity   int    `json:"complexity"`
	Dependencies []any  `json:"dependencies"` // can be string or int from LLM
	AgentType    string `json:"agent_type"`
	ModelTier    string `json:"model_tier"`
}

// ParsePRD reads a PRD file and generates tasks via LLM (or rho fallback).
func ParsePRD(ctx context.Context, cfg *config.Config, store *storage.Storage, file, tag string, numTasks int, caller ...llm.Caller) error {
	content, err := os.ReadFile(file)
	if err != nil {
		return fmt.Errorf("reading PRD: %w", err)
	}

	guidance := store.LoadGuidance()
	prompt := ParsePRDPrompt(string(content), numTasks, guidance)

	var parsed []parsedTask
	if len(caller) > 0 && caller[0] != nil {
		if err := caller[0].CompleteJSON(ctx, prompt, "", true, &parsed); err != nil {
			return fmt.Errorf("llm parse-prd: %w", err)
		}
	} else {
		var err error
		parsed, err = rho.RunJSON[[]parsedTask](ctx, rho.Options{
			Prompt: prompt,
			Model:  cfg.Rho.FastModel,
		})
		if err != nil {
			return fmt.Errorf("rho parse-prd: %w", err)
		}
	}

	return store.UpdatePhase(tag, func(p *model.Phase) error {
		now := time.Now().UTC().Format(time.RFC3339)
		for i, pt := range parsed {
			id := fmt.Sprintf("%d", i+1)
			t := &model.Task{
				ID:          id,
				Title:       pt.Title,
				Description: pt.Description,
				Status:      model.Pending,
				Complexity:  pt.Complexity,
				Priority:    parsePriority(pt.Priority),
				CreatedAt:   now,
				UpdatedAt:   now,
			}

			// Remap dependencies from 1-indexed to task IDs
			for _, dep := range pt.Dependencies {
				depStr := fmt.Sprintf("%v", dep)
				// Cross-phase deps (contain ":") kept as-is
				// Otherwise, it's a 1-indexed reference
				t.Dependencies = append(t.Dependencies, depStr)
			}

			// Set agent type
			if pt.AgentType != "" {
				t.AgentType = model.AgentType(pt.AgentType)
			}
			if pt.ModelTier != "" {
				t.ModelTier = model.ModelTier(pt.ModelTier)
			}
			t.AutoAssignAgent()

			p.Tasks = append(p.Tasks, t)
		}
		return nil
	})
}

func parsePriority(s string) model.Priority {
	switch s {
	case "critical":
		return model.Critical
	case "high":
		return model.High
	case "low":
		return model.Low
	default:
		return model.Medium
	}
}
