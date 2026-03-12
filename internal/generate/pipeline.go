package generate

import (
	"context"
	"fmt"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/internal/ui"
)

// GenerateOpts configures the generate pipeline.
type GenerateOpts struct {
	NoExpand    bool
	NoCheckDeps bool
}

// Generate orchestrates the full pipeline: ParsePRD -> Expand -> CheckDeps.
func Generate(ctx context.Context, cfg *config.Config, store *storage.Storage, file, tag string, numTasks int, opts GenerateOpts) error {
	ui.Header("Generate Pipeline", fmt.Sprintf("(tag: %s)", tag))

	// Phase 1: Parse PRD
	ui.Phase(1, "Parsing PRD into tasks...")
	ui.Info(fmt.Sprintf("File: %s → %d tasks with model %s", file, numTasks, cfg.Rho.FastModel))

	spin := ui.NewSpinner(fmt.Sprintf("Parsing %s with AI...", file))
	err := ParsePRD(ctx, cfg, store, file, tag, numTasks)
	if err != nil {
		spin.Stop(false, fmt.Sprintf("Parse failed: %v", err))
		return fmt.Errorf("parse-prd: %w", err)
	}

	// Show created tasks
	phases, _ := store.LoadPhases()
	if p, ok := phases[tag]; ok {
		spin.Stop(true, fmt.Sprintf("Parsed %d tasks", len(p.Tasks)))
		for _, t := range p.Tasks {
			ui.TaskLine(t.ID, t.Title)
		}
	} else {
		spin.Stop(true, "Parse complete")
	}
	fmt.Fprintln(ui.Stderr())

	// Phase 2: Expand complex tasks
	if opts.NoExpand {
		ui.PhaseSkipped(2, "Skipping expansion", "(--no-expand)")
	} else {
		ui.Phase(2, "Expanding complex tasks into subtasks...")
		err = Expand(ctx, cfg, store, tag, "")
		if err != nil {
			return fmt.Errorf("expand: %w", err)
		}
	}
	fmt.Fprintln(ui.Stderr())

	// Phase 3: Check dependencies
	if opts.NoCheckDeps {
		ui.PhaseSkipped(3, "Skipping dependency validation", "(--no-check-deps)")
	} else {
		ui.Phase(3, "Validating task dependencies...")

		phases, err := store.LoadPhases()
		if err != nil {
			return fmt.Errorf("loading phases for check-deps: %w", err)
		}
		result := CheckDeps(phases, tag)
		if !result.OK {
			ui.Warn(fmt.Sprintf("Dependency issues found:\n%s", FormatCheckResult(result)))
		} else {
			ui.Success("Dependency check passed")
		}
	}

	ui.Complete(fmt.Sprintf("Generate pipeline complete! (tag: %s)", tag))

	// Next steps
	fmt.Fprintln(ui.Stderr())
	fmt.Fprintf(ui.Stderr(), "%s\n", ui.Blue("Next steps:"))
	fmt.Fprintf(ui.Stderr(), "  1. Review tasks: scud list --tag %s\n", tag)
	fmt.Fprintf(ui.Stderr(), "  2. View waves:   scud waves --tag %s\n", tag)
	fmt.Fprintf(ui.Stderr(), "  3. Start work:   scud next --tag %s\n", tag)
	return nil
}
