package swarm

import (
	"context"
	"fmt"
	"os"
	"sort"
	"strings"
	"time"

	"golang.org/x/sync/errgroup"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/rho"
	"github.com/reuben/scud/internal/ui"
	"github.com/reuben/scud/pkg/model"
	"github.com/reuben/scud/pkg/wave"
)

// RunOpts configures swarm execution.
type RunOpts struct {
	DryRun     bool
	NoValidate bool
	NoRepair   bool
	Tag        string
	AllTags    bool
	RoundSize  int // 0 = use config
}

// TaskStore provides the task state and project context needed by the swarm.
type TaskStore interface {
	LoadPhases() (map[string]*model.Phase, error)
	UpdatePhase(tag string, fn func(*model.Phase) error) error
	Root() string
	LoadGuidance() string
}

// Run executes the swarm: parallel waves with ralph fallback on failure.
func Run(ctx context.Context, cfg *config.Config, store TaskStore, opts RunOpts) error {
	if opts.AllTags {
		return runAllTags(ctx, cfg, store, opts)
	}
	return runTag(ctx, cfg, store, opts)
}

// runAllTags iterates all phases/tags in order and runs swarm on each.
func runAllTags(ctx context.Context, cfg *config.Config, store TaskStore, opts RunOpts) error {
	phases, err := store.LoadPhases()
	if err != nil {
		return fmt.Errorf("loading phases: %w", err)
	}
	// Collect and sort tags for deterministic ordering
	var tags []string
	for tag := range phases {
		tags = append(tags, tag)
	}
	sort.Strings(tags)

	for _, tag := range tags {
		ui.Header("Phase", tag)
		tagOpts := opts
		tagOpts.Tag = tag
		tagOpts.AllTags = false
		if err := runTag(ctx, cfg, store, tagOpts); err != nil {
			return fmt.Errorf("phase '%s': %w", tag, err)
		}
	}
	return nil
}

func runTag(ctx context.Context, cfg *config.Config, store TaskStore, opts RunOpts) error {
	tag := opts.Tag
	roundSize := opts.RoundSize
	if roundSize <= 0 {
		roundSize = cfg.Swarm.RoundSize
		if roundSize <= 0 {
			roundSize = 5
		}
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
			ui.Complete(fmt.Sprintf("Swarm finished — Done=%d, Failed=%d, Total=%d", stats.Done, stats.Failed, stats.Total))
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
		fmt.Fprintln(os.Stderr)
		ui.Header(fmt.Sprintf("Wave %d", currentWave.Number), fmt.Sprintf("(%d tasks)", len(currentWave.Tasks)))
		for _, id := range currentWave.Tasks {
			if t := phase.FindTask(id); t != nil {
				tierModel := resolveModel(t, cfg)
				ui.Info(fmt.Sprintf("%s: %s %s", id, t.Title, ui.Dim(fmt.Sprintf("[%s/%s]", t.AgentType, tierModel))))
			}
		}

		if opts.DryRun {
			ui.Info(ui.Dim("(dry run — not executing)"))
			for _, w := range wr.Waves[1:] {
				fmt.Fprintln(os.Stderr)
				fmt.Fprintf(os.Stderr, "  %s %s\n", ui.BoldYellow(fmt.Sprintf("Wave %d:", w.Number)), fmt.Sprintf("%d tasks", len(w.Tasks)))
				for _, id := range w.Tasks {
					if t := phase.FindTask(id); t != nil {
						ui.Info(fmt.Sprintf("%s: %s", id, t.Title))
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

			ui.Info(fmt.Sprintf("Round: %d tasks", len(chunk)))
			g, gctx := errgroup.WithContext(ctx)
			g.SetLimit(roundSize)

			for _, taskID := range chunk {
				taskID := taskID
				g.Go(func() error {
					return executeTask(gctx, store, cfg, tag, taskID, taskTimeout)
				})
			}

			if err := g.Wait(); err != nil {
				ui.Warn(fmt.Sprintf("Round error: %v", err))
			}
		}

		// Backpressure gate
		if !opts.NoValidate {
			spin := ui.NewSpinner("Running validation...")
			vr := RunValidation(ctx, store.Root(), cfg)
			if vr.AllPassed {
				spin.Stop(true, "Validation passed")
				continue
			}

			spin.Stop(false, "Validation failed")
			for _, cr := range vr.Results {
				if !cr.Passed {
					ui.Fail(fmt.Sprintf("%s (exit %d, %.1fs)", cr.Command, cr.ExitCode, cr.DurationSec))
					if cr.Stderr != "" {
						fmt.Fprintf(os.Stderr, "    %s\n", ui.Dim(firstLines(cr.Stderr, 5)))
					}
				}
			}

			// Attribute failures to specific tasks
			attributions := AttributeFailure(ctx, vr, store.Root(), waveTasks)
			if len(attributions) > 0 {
				ui.Info(fmt.Sprintf("Attributed failures to %d task(s):", len(attributions)))
				for _, a := range attributions {
					ui.Info(fmt.Sprintf("  %s [%s]: %s", a.TaskID, a.Confidence, a.Reason))
				}
			}

			if opts.NoRepair {
				ui.Warn("Skipping recovery (--no-repair)")
				continue
			}

			fmt.Fprintln(os.Stderr)
			ui.Warn("Switching to sequential recovery (smart model)...")

			// Determine which tasks to repair: attributed tasks if available, otherwise all wave tasks
			repairTasks := waveTasks
			if len(attributions) > 0 {
				seen := make(map[string]bool)
				repairTasks = nil
				for _, a := range attributions {
					if !seen[a.TaskID] {
						seen[a.TaskID] = true
						repairTasks = append(repairTasks, a.TaskID)
					}
				}
				ui.Info(fmt.Sprintf("Repairing %d attributed task(s) instead of %d total", len(repairTasks), len(waveTasks)))
			}

			// Build attribution map for repair prompts
			attrMap := make(map[string][]Attribution)
			for _, a := range attributions {
				attrMap[a.TaskID] = append(attrMap[a.TaskID], a)
			}

			// Reset repair tasks to pending for retry
			if err := store.UpdatePhase(tag, func(p *model.Phase) error {
				for _, id := range repairTasks {
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
				ui.Info(fmt.Sprintf("Recovery attempt %d/%d...", attempt, maxRalph))

				for _, taskID := range repairTasks {
					// Reload to check current status
					phases, _ := store.LoadPhases()
					if p, ok := phases[tag]; ok {
						if t := p.FindTask(taskID); t != nil && t.Status != model.Done {
							guidance := store.LoadGuidance()
							taskAttrs := attrMap[taskID]
							prompt := buildRepairPrompt(t, tag, guidance, p, taskAttrs, vr)

							ui.Info(fmt.Sprintf("[%s] Repair: %s %s", taskID, t.Title, ui.Dim(fmt.Sprintf("(model=%s)", cfg.Rho.SmartModel))))

							taskCtx, cancel := context.WithTimeout(ctx, time.Duration(taskTimeout)*time.Second)
							_, execErr := rho.Run(taskCtx, rho.Options{
								Prompt:     prompt,
								Model:      cfg.Rho.SmartModel,
								WorkingDir: store.Root(),
							})
							cancel()

							if execErr != nil {
								ui.Fail(fmt.Sprintf("Task %s recovery: %v", taskID, execErr))
							}

							// Validate after each task in ralph mode
							vr2 := RunValidation(ctx, store.Root(), cfg)
							if vr2.AllPassed {
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
				vr = RunValidation(ctx, store.Root(), cfg)
				if vr.AllPassed {
					ui.Success(fmt.Sprintf("Recovery succeeded on attempt %d", attempt))
					recovered = true
					break
				}
			}

			if !recovered {
				ui.Fail(fmt.Sprintf("Recovery exhausted after %d attempts", maxRalph))
			}
		}
	}
}

func executeTask(ctx context.Context, store TaskStore, cfg *config.Config, tag, taskID string, timeoutSecs int) error {
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

func executeTaskWithModel(ctx context.Context, store TaskStore, cfg *config.Config, tag, taskID, taskModel string, timeoutSecs int) error {
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

	ui.Info(fmt.Sprintf("[%s] %s %s", taskID, t.Title, ui.Dim(fmt.Sprintf("(model=%s)", taskModel))))

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
				ui.Success(fmt.Sprintf("[%s] Agent set status: %s", taskID, current.Status))
				return nil
			}
		}
	}

	// Determine status from rho result
	finalStatus := model.Done
	if err != nil || (result != nil && result.ExitCode != 0) {
		finalStatus = model.Failed
		if err != nil {
			ui.Fail(fmt.Sprintf("[%s] %v", taskID, err))
		} else {
			ui.Fail(fmt.Sprintf("[%s] exit code %d", taskID, result.ExitCode))
		}
	} else {
		ui.Success(fmt.Sprintf("[%s] Completed", taskID))
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

func buildRepairPrompt(t *model.Task, tag, guidance string, phase *model.Phase, attributions []Attribution, vr ValidationResult) string {
	var b strings.Builder

	b.WriteString(fmt.Sprintf("# REPAIR Task: %s\n\n", t.Title))
	b.WriteString(fmt.Sprintf("**Task ID:** %s\n", t.ID))
	b.WriteString(fmt.Sprintf("**Tag:** %s\n\n", tag))

	b.WriteString("## Error Context\n\n")
	b.WriteString("The previous implementation caused validation failures. Fix the issues below.\n\n")

	// Include validation error output
	for _, cr := range vr.Results {
		if !cr.Passed {
			b.WriteString(fmt.Sprintf("### Failed: `%s` (exit %d)\n", cr.Command, cr.ExitCode))
			if cr.Stderr != "" {
				b.WriteString("```\n")
				b.WriteString(firstLines(cr.Stderr, 20))
				b.WriteString("\n```\n\n")
			}
			if cr.Stdout != "" {
				b.WriteString("```\n")
				b.WriteString(firstLines(cr.Stdout, 20))
				b.WriteString("\n```\n\n")
			}
		}
	}

	// Include attribution details
	if len(attributions) > 0 {
		b.WriteString("### Attributed Error Locations\n")
		for _, a := range attributions {
			b.WriteString(fmt.Sprintf("- **%s** [confidence: %s]: %s\n", a.File, a.Confidence, a.Reason))
		}
		b.WriteString("\n")
	}

	if t.Description != "" {
		b.WriteString(fmt.Sprintf("## Original Description\n%s\n\n", t.Description))
	}
	if t.Details != "" {
		b.WriteString(fmt.Sprintf("## Implementation Details\n%s\n\n", t.Details))
	}

	if guidance != "" {
		b.WriteString(fmt.Sprintf("## Project Guidance\n%s\n\n", guidance))
	}

	b.WriteString("## Instructions\n")
	b.WriteString("Fix the validation failures caused by this task. Focus on the error locations above.\n")
	b.WriteString("When you are done, run:\n")
	b.WriteString(fmt.Sprintf("  scud set-status %s done -t %s\n\n", t.ID, tag))
	b.WriteString("If you cannot fix it, run:\n")
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
