package generate

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/rho"
	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/internal/taskapi"
	"github.com/reuben/scud/pkg/llm"
	"github.com/reuben/scud/pkg/model"
)

// Proposal is a persistence-free, importable PRD breakdown.
type Proposal struct {
	SchemaVersion int             `json:"schema_version"`
	Phases        []ProposalPhase `json:"phases"`
}
type ProposalPhase struct {
	Tag      string        `json:"tag"`
	Name     string        `json:"name"`
	IDFormat string        `json:"id_format"`
	Tasks    []*model.Task `json:"tasks"`
}
type ProposalOptions struct {
	Tag      string
	NumTasks int
}
type TargetCaller = llm.JSONCaller

type proposalPayload struct {
	Tasks []struct {
		Title        string   `json:"title"`
		Description  string   `json:"description"`
		Priority     string   `json:"priority"`
		Complexity   int      `json:"complexity"`
		Dependencies []string `json:"dependencies"`
	} `json:"tasks"`
}

// Propose converts PRD text to a validated task graph without touching storage.
func Propose(ctx context.Context, caller TargetCaller, content string, opts ProposalOptions) (*Proposal, error) {
	if opts.NumTasks < 0 {
		return nil, fmt.Errorf("task count cannot be negative")
	}
	if len(content) > 4<<20 {
		return nil, fmt.Errorf("PRD exceeds 4 MiB input limit")
	}
	if strings.TrimSpace(content) == "" {
		return nil, fmt.Errorf("PRD content is empty")
	}
	if caller == nil {
		return nil, fmt.Errorf("model caller is required")
	}
	tag := opts.Tag
	if tag == "" {
		tag = "main"
	}
	if _, err := taskapi.ValidateDocument(taskapi.Document{SchemaVersion: 1, Phases: []taskapi.Phase{{Tag: tag, Name: tag, IDFormat: "sequential", Tasks: []*model.Task{}}}}); err != nil {
		return nil, fmt.Errorf("invalid phase tag %q: %w", tag, err)
	}
	prompt := `Create an actionable implementation plan from this PRD. Size each task around a clear goal and observable verification. Do not assign agents or model tiers. Do not force a task count unless requested; if requested, aim for that number. Return only JSON: {"tasks":[{"title":"...","description":"...","priority":"medium","complexity":3,"dependencies":[]}]} Dependencies refer to 1-based task indices as strings.`
	if opts.NumTasks > 0 {
		prompt += fmt.Sprintf(" Requested task count: %d.", opts.NumTasks)
	}
	prompt += "\n\nPRD:\n" + content
	raw, err := llm.RequestJSON[proposalPayload](ctx, caller, prompt, "Return valid JSON only.")
	if err != nil {
		return nil, err
	}
	if len(raw.Tasks) == 0 {
		return nil, fmt.Errorf("proposal contains no tasks")
	}
	tasks := make([]*model.Task, len(raw.Tasks))
	for i, r := range raw.Tasks {
		if r.Title == "" || r.Description == "" {
			return nil, fmt.Errorf("task %d requires title and description", i+1)
		}
		pri, ok := model.ParsePriority(r.Priority)
		if !ok {
			return nil, fmt.Errorf("task %d has invalid priority %q", i+1, r.Priority)
		}
		if r.Complexity < 1 || r.Complexity > 21 {
			return nil, fmt.Errorf("task %d complexity must be 1..21", i+1)
		}
		tasks[i] = &model.Task{ID: fmt.Sprint(i + 1), Title: r.Title, Description: r.Description, Priority: pri, Complexity: r.Complexity, Status: model.Pending, Dependencies: r.Dependencies}
	}
	if _, err := taskapi.ValidateDocument(taskapi.Document{SchemaVersion: 1, Phases: []taskapi.Phase{{Tag: tag, Name: tag, IDFormat: "sequential", Tasks: tasks}}}); err != nil {
		return nil, fmt.Errorf("invalid proposed task document: %w", err)
	}
	return &Proposal{SchemaVersion: 1, Phases: []ProposalPhase{{Tag: tag, Name: tag, IDFormat: "sequential", Tasks: tasks}}}, nil
}

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
