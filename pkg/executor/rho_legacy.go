package executor

import (
	"context"

	"github.com/reuben/scud/internal/rho"
)

// LegacyRho runs the pre-rho.run/v1 CLI contract. It keeps existing SCUD
// behavior intact while callers migrate to RhoV1.
type LegacyRho struct{}

func (LegacyRho) Run(ctx context.Context, req Request, _ EventHandler) (*Result, error) {
	result, err := rho.Run(ctx, rho.Options{
		Prompt:       req.Prompt,
		Model:        req.Model.ID,
		WorkingDir:   req.WorkingDir,
		SystemPrompt: req.SystemPrompt,
		AllowedTools: req.AllowedTools,
	})
	if result == nil {
		return nil, err
	}
	return &Result{
		RunID:    req.RunID,
		Text:     result.Stdout,
		ExitCode: result.ExitCode,
		Stderr:   result.Stderr,
	}, err
}
