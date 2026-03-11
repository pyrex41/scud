package generate

import (
	"context"
	"fmt"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/storage"
)

// GenerateOpts configures the generate pipeline.
type GenerateOpts struct {
	NoExpand    bool
	NoCheckDeps bool
}

// Generate orchestrates the full pipeline: ParsePRD -> Expand -> CheckDeps.
func Generate(ctx context.Context, cfg *config.Config, store *storage.Storage, file, tag string, numTasks int, opts GenerateOpts) error {
	// Step 1: Parse PRD
	if err := ParsePRD(ctx, cfg, store, file, tag, numTasks); err != nil {
		return fmt.Errorf("parse-prd: %w", err)
	}

	// Step 2: Expand complex tasks
	if !opts.NoExpand {
		if err := Expand(ctx, cfg, store, tag, ""); err != nil {
			return fmt.Errorf("expand: %w", err)
		}
	}

	// Step 3: Check dependencies
	if !opts.NoCheckDeps {
		phases, err := store.LoadPhases()
		if err != nil {
			return fmt.Errorf("loading phases for check-deps: %w", err)
		}
		result := CheckDeps(phases, tag)
		if !result.OK {
			fmt.Printf("Dependency issues found:\n%s\n", FormatCheckResult(result))
		} else {
			fmt.Println("Dependency check passed.")
		}
	}

	fmt.Printf("Generation complete for tag '%s'.\n", tag)
	return nil
}
