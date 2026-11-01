package cmd

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/ui"
	agentexec "github.com/reuben/scud/pkg/executor"
	"github.com/reuben/scud/pkg/model"
	"github.com/spf13/cobra"
)

func NewRunCmd() *cobra.Command {
	var tag, modelName, provider, executorKind string

	cmd := &cobra.Command{
		Use:   "run <id>",
		Short: "Run a single task with rho",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}
			tag, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}

			cfg, err := config.Load(store.ScudDir())
			if err != nil {
				return err
			}

			phases, err := store.LoadPhases()
			if err != nil {
				return err
			}
			phase, ok := phases[tag]
			if !ok {
				return fmt.Errorf("tag '%s' not found", tag)
			}
			t := phase.FindTask(args[0])
			if t == nil {
				return fmt.Errorf("task '%s' not found in tag '%s'", args[0], tag)
			}

			// Determine model
			if modelName == "" {
				modelName = cfg.ModelForTier(string(t.ModelTier))
			}
			if provider == "" {
				provider = cfg.ProviderForTier(string(t.ModelTier))
			}
			runner, err := configuredExecutor(cfg, executorKind, provider, modelName, store.Root())
			if err != nil {
				return err
			}

			// Mark in-progress
			if err := store.UpdatePhase(tag, func(p *model.Phase) error {
				if task := p.FindTask(args[0]); task != nil {
					task.Status = model.InProgress
					task.SetUpdatedNow()
				}
				return nil
			}); err != nil {
				return err
			}

			// Build prompt
			guidance := store.LoadGuidance()
			prompt := buildRunPrompt(t, tag, guidance, phase)

			ui.Info(fmt.Sprintf("[%s] %s (model=%s)", t.ID, t.Title, modelName))

			// Run rho
			timeout := time.Duration(cfg.Swarm.TaskTimeoutSecs) * time.Second
			if timeout == 0 {
				timeout = 600 * time.Second
			}
			ctx, cancel := context.WithTimeout(context.Background(), timeout)
			defer cancel()

			result, err := runner.Run(ctx, agentexec.Request{
				RunID:      fmt.Sprintf("scud:%s:%s:run:%d", tag, t.ID, time.Now().UnixNano()),
				Prompt:     prompt,
				Model:      agentexec.ModelRef{Provider: provider, ID: modelName},
				WorkingDir: store.Root(),
				Context: map[string]any{
					"scud.tag": tag, "scud.task_id": t.ID, "scud.mode": "run",
				},
			}, nil)

			// Check if agent already set status
			phases, _ = store.LoadPhases()
			if p, ok := phases[tag]; ok {
				if current := p.FindTask(args[0]); current != nil {
					if current.Status == model.Done || current.Status == model.Failed || current.Status == model.Review {
						ui.Success(fmt.Sprintf("[%s] Agent set status: %s", args[0], current.Status))
						return nil
					}
				}
			}

			// Set final status
			finalStatus := model.Done
			if err != nil || result.Failed() {
				finalStatus = model.Failed
				if err != nil {
					ui.Fail(fmt.Sprintf("[%s] %v", args[0], err))
				} else {
					ui.Fail(fmt.Sprintf("[%s] exit code %d", args[0], result.ExitCode))
				}
			} else {
				ui.Success(fmt.Sprintf("[%s] Completed", args[0]))
			}

			return store.UpdatePhase(tag, func(p *model.Phase) error {
				if task := p.FindTask(args[0]); task != nil {
					task.Status = finalStatus
					task.SetUpdatedNow()
				}
				return nil
			})
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().StringVar(&modelName, "model", "", "Model to use (overrides tier)")
	cmd.Flags().StringVar(&provider, "provider", "", "Provider override for rho-v1 (anthropic, openai, xai)")
	cmd.Flags().StringVar(&executorKind, "executor", "", "Agent executor override: legacy or rho-v1")
	return cmd
}

func buildRunPrompt(t *model.Task, tag, guidance string, phase *model.Phase) string {
	var b strings.Builder

	b.WriteString(fmt.Sprintf("# Task: %s\n\n", t.Title))
	b.WriteString(fmt.Sprintf("**Task ID:** %s\n", t.ID))
	b.WriteString(fmt.Sprintf("**Tag:** %s\n\n", tag))

	if t.Description != "" {
		b.WriteString(fmt.Sprintf("## Description\n%s\n\n", t.Description))
	}
	if t.Details != "" {
		b.WriteString(fmt.Sprintf("## Implementation Details\n%s\n\n", t.Details))
	}
	if t.TestStrategy != "" {
		b.WriteString(fmt.Sprintf("## Test Strategy\n%s\n\n", t.TestStrategy))
	}

	if len(t.Dependencies) > 0 {
		b.WriteString("## Completed Dependencies\n")
		for _, depID := range t.Dependencies {
			if dep := phase.FindTask(depID); dep != nil && dep.Status == model.Done {
				b.WriteString(fmt.Sprintf("- **%s**: %s\n", dep.ID, dep.Title))
			}
		}
		b.WriteString("\n")
	}

	if guidance != "" {
		b.WriteString(fmt.Sprintf("## Project Guidance\n%s\n\n", guidance))
	}

	b.WriteString("## Instructions\n")
	b.WriteString("Implement this task completely. When you are done, run:\n")
	b.WriteString(fmt.Sprintf("  scud set-status %s done -t %s\n\n", t.ID, tag))
	b.WriteString("If you encounter a blocking issue, run:\n")
	b.WriteString(fmt.Sprintf("  scud set-status %s failed -t %s\n", t.ID, tag))

	return b.String()
}
