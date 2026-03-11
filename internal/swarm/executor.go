package swarm

import (
	"context"
	"fmt"
	"strings"
	"time"

	"golang.org/x/sync/errgroup"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/model"
	"github.com/reuben/scud/internal/rho"
	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/internal/wave"
)

// RunOpts configures swarm execution.
type RunOpts struct {
	DryRun     bool
	NoValidate bool
	Tag        string
}

// Run executes the swarm: parallel waves with ralph fallback on failure.
func Run(ctx context.Context, cfg *config.Config, store *storage.Storage, opts RunOpts) error {
	tag := opts.Tag
	roundSize := cfg.Swarm.RoundSize
	if roundSize <= 0 {
		roundSize = 5
	}
	maxRalph := cfg.Swarm.MaxRalphAttempts
	if maxRalph <= 0 {
		maxRalph = 3
	}
	taskTimeout := cfg.Swarm.TaskTimeoutSecs
	if taskTimeout <= 0 {
		taskTimeout = 600
	}

	for {
		// Load current state
		phases, err := store.LoadPhases()
		if err != nil {
			return fmt.Errorf("loading phases: %w", err)
		}
		phase, ok := phases[tag]
		if !ok {
			return fmt.Errorf("tag '%s' not found", tag)
		}

		actionable := phase.ActionableTasks()
		if len(actionable) == 0 {
			stats := phase.Stats()
			fmt.Printf("No more actionable tasks. Done=%d, Failed=%d, Total=%d\n",
				stats.Done, stats.Failed, stats.Total)
			return nil
		}

		// Compute waves
		wr := wave.Plan(actionable)
		if len(wr.Waves) == 0 {
			if len(wr.CircularDeps) > 0 {
				return fmt.Errorf("circular dependencies detected: %s", strings.Join(wr.CircularDeps, ", "))
			}
			return nil
		}

		currentWave := wr.Waves[0]
		fmt.Printf("\n=== Wave %d: %d tasks ===\n", currentWave.Number, len(currentWave.Tasks))
		for _, id := range currentWave.Tasks {
			if t := phase.FindTask(id); t != nil {
				tierModel := resolveModel(t, cfg)
				fmt.Printf("  %s: %s [%s/%s]\n", id, t.Title, t.AgentType, tierModel)
			}
		}

		if opts.DryRun {
			fmt.Println("(dry run - not executing)")
			// Show remaining waves
			for _, w := range wr.Waves[1:] {
				fmt.Printf("\n--- Wave %d: %d tasks ---\n", w.Number, len(w.Tasks))
				for _, id := range w.Tasks {
					if t := phase.FindTask(id); t != nil {
						fmt.Printf("  %s: %s\n", id, t.Title)
					}
				}
			}
			return nil
		}

		// Execute wave in parallel rounds
		waveTasks := currentWave.Tasks
		for i := 0; i < len(waveTasks); i += roundSize {
			end := i + roundSize
			if end > len(waveTasks) {
				end = len(waveTasks)
			}
			chunk := waveTasks[i:end]

			fmt.Printf("  Round: %d tasks\n", len(chunk))
			g, gctx := errgroup.WithContext(ctx)
			g.SetLimit(roundSize)

			for _, taskID := range chunk {
				taskID := taskID
				g.Go(func() error {
					return executeTask(gctx, store, cfg, tag, taskID, taskTimeout)
				})
			}

			if err := g.Wait(); err != nil {
				fmt.Printf("  Round error: %v\n", err)
			}
		}

		// Backpressure gate
		if !opts.NoValidate {
			fmt.Println("  Running validation...")
			vr := RunValidation(ctx, store.Root(), cfg)
			if vr.AllPassed {
				fmt.Println("  Validation passed!")
				continue
			}

			// Print failures
			for _, cr := range vr.Results {
				if !cr.Passed {
					fmt.Printf("  FAIL: %s (exit %d, %.1fs)\n", cr.Command, cr.ExitCode, cr.DurationSec)
					if cr.Stderr != "" {
						fmt.Printf("    stderr: %s\n", firstLines(cr.Stderr, 5))
					}
				}
			}

			// Ralph recovery
			fmt.Println("\n  Wave failed backpressure. Switching to sequential recovery (smart model)...")

			// Reset wave tasks to pending for retry
			if err := store.UpdatePhase(tag, func(p *model.Phase) error {
				for _, id := range waveTasks {
					if t := p.FindTask(id); t != nil && t.Status != model.Done {
						t.Status = model.Pending
						t.SetUpdatedNow()
					}
				}
				return nil
			}); err != nil {
				return fmt.Errorf("resetting tasks: %w", err)
			}

			recovered := false
			for attempt := 1; attempt <= maxRalph; attempt++ {
				fmt.Printf("  Recovery attempt %d/%d...\n", attempt, maxRalph)

				for _, taskID := range waveTasks {
					// Reload to check current status
					phases, _ := store.LoadPhases()
					if p, ok := phases[tag]; ok {
						if t := p.FindTask(taskID); t != nil && t.Status != model.Done {
							// Execute with smart model
							if err := executeTaskWithModel(ctx, store, cfg, tag, taskID, cfg.Rho.SmartModel, taskTimeout); err != nil {
								fmt.Printf("    Task %s recovery error: %v\n", taskID, err)
							}

							// Validate after each task in ralph mode
							vr := RunValidation(ctx, store.Root(), cfg)
							if vr.AllPassed {
								// Mark done
								store.UpdatePhase(tag, func(p *model.Phase) error {
									if t := p.FindTask(taskID); t != nil {
										t.Status = model.Done
										t.SetUpdatedNow()
									}
									return nil
								})
							} else {
								store.UpdatePhase(tag, func(p *model.Phase) error {
									if t := p.FindTask(taskID); t != nil {
										t.Status = model.Failed
										t.SetUpdatedNow()
									}
									return nil
								})
							}
						}
					}
				}

				// Check overall validation
				vr := RunValidation(ctx, store.Root(), cfg)
				if vr.AllPassed {
					fmt.Printf("  Recovery succeeded on attempt %d\n", attempt)
					recovered = true
					break
				}
			}

			if !recovered {
				fmt.Printf("  Recovery exhausted after %d attempts. Failed tasks left as-is.\n", maxRalph)
			}
		}
	}
}

func executeTask(ctx context.Context, store *storage.Storage, cfg *config.Config, tag, taskID string, timeoutSecs int) error {
	// Load task to get model
	phases, err := store.LoadPhases()
	if err != nil {
		return err
	}
	phase, ok := phases[tag]
	if !ok {
		return fmt.Errorf("tag not found: %s", tag)
	}
	t := phase.FindTask(taskID)
	if t == nil {
		return fmt.Errorf("task not found: %s", taskID)
	}
	tierModel := resolveModel(t, cfg)
	return executeTaskWithModel(ctx, store, cfg, tag, taskID, tierModel, timeoutSecs)
}

func executeTaskWithModel(ctx context.Context, store *storage.Storage, cfg *config.Config, tag, taskID, taskModel string, timeoutSecs int) error {
	// Mark in-progress
	if err := store.UpdatePhase(tag, func(p *model.Phase) error {
		if t := p.FindTask(taskID); t != nil {
			t.Status = model.InProgress
			t.SetUpdatedNow()
		}
		return nil
	}); err != nil {
		return err
	}

	// Reload task for prompt building
	phases, _ := store.LoadPhases()
	phase := phases[tag]
	t := phase.FindTask(taskID)
	if t == nil {
		return fmt.Errorf("task disappeared: %s", taskID)
	}

	guidance := store.LoadGuidance()
	prompt := buildTaskPrompt(t, tag, guidance, phase)

	fmt.Printf("    [%s] Starting: %s (model=%s)\n", taskID, t.Title, taskModel)

	// Run rho with timeout
	taskCtx, cancel := context.WithTimeout(ctx, time.Duration(timeoutSecs)*time.Second)
	defer cancel()

	result, err := rho.Run(taskCtx, rho.Options{
		Prompt:     prompt,
		Model:      taskModel,
		WorkingDir: store.Root(),
	})

	// Reload task status (agent may have called scud set-status)
	phases, _ = store.LoadPhases()
	if p, ok := phases[tag]; ok {
		if current := p.FindTask(taskID); current != nil {
			if current.Status == model.Done || current.Status == model.Failed || current.Status == model.Review {
				fmt.Printf("    [%s] Agent set status: %s\n", taskID, current.Status)
				return nil
			}
		}
	}

	// Determine status from rho result
	finalStatus := model.Done
	if err != nil || (result != nil && result.ExitCode != 0) {
		finalStatus = model.Failed
		if err != nil {
			fmt.Printf("    [%s] Error: %v\n", taskID, err)
		} else {
			fmt.Printf("    [%s] Exited with code %d\n", taskID, result.ExitCode)
		}
	} else {
		fmt.Printf("    [%s] Completed\n", taskID)
	}

	return store.UpdatePhase(tag, func(p *model.Phase) error {
		if t := p.FindTask(taskID); t != nil {
			t.Status = finalStatus
			t.SetUpdatedNow()
		}
		return nil
	})
}

func buildTaskPrompt(t *model.Task, tag, guidance string, phase *model.Phase) string {
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

	// Include completed dependency context
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

func resolveModel(t *model.Task, cfg *config.Config) string {
	if t.ModelTier == model.TierCustom && t.ModelOverride != "" {
		return t.ModelOverride
	}
	return cfg.ModelForTier(string(t.ModelTier))
}

func firstLines(s string, n int) string {
	lines := strings.Split(s, "\n")
	if len(lines) > n {
		lines = lines[:n]
	}
	return strings.Join(lines, "\n")
}
