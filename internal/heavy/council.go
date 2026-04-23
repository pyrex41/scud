package heavy

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/ui"
)

// CouncilOpts configures a `scud council` run: one captain + N workers, each
// running a *different* model. Captain synthesizes the heterogeneous takes
// into a single answer.
type CouncilOpts struct {
	Query        string
	WorkerModels []string
	CaptainModel string
	Concurrency  int
	DebateRounds int
	Verbose      bool
	JSON         bool
	WorkingDir   string
	TimeoutSecs  int
}

// RunCouncil fans out one worker per entry in WorkerModels. Each uses a
// different model; the captain synthesizes the ensemble.
func RunCouncil(ctx context.Context, cfg *config.Config, opts CouncilOpts) (*Result, error) {
	if len(opts.WorkerModels) == 0 {
		return nil, fmt.Errorf("--workers is required: provide at least one model id")
	}
	if opts.CaptainModel == "" {
		if cfg != nil {
			opts.CaptainModel = cfg.HeavyModel("synthesis")
		}
		if opts.CaptainModel == "" {
			opts.CaptainModel = "grok-4.20-reasoning"
		}
	}

	concurrency := opts.Concurrency
	if concurrency <= 0 {
		concurrency = len(opts.WorkerModels)
	}
	timeout := opts.TimeoutSecs
	if timeout <= 0 {
		timeout = 300
		if cfg != nil && cfg.Heavy.TimeoutSecs > 0 {
			timeout = cfg.Heavy.TimeoutSecs
		}
	}

	workers := buildCouncilWorkers(opts.WorkerModels)

	if opts.Verbose {
		ui.Header("Scud Council", fmt.Sprintf(
			"(captain: %s, workers: %s)",
			opts.CaptainModel, strings.Join(opts.WorkerModels, ", "),
		))
		fmt.Fprintln(os.Stderr)
		ui.Phase(1, fmt.Sprintf("Running %d heterogeneous workers (concurrency=%d)...", len(workers), concurrency))
	}

	// The pipeline-level model arg is a fallback only; every worker has its
	// own Model set so the fallback is unused here.
	runOpts := RunOpts{
		Verbose:    opts.Verbose,
		WorkingDir: opts.WorkingDir,
	}
	outputs := executeAgents(ctx, workers, opts.Query, "", concurrency, timeout, runOpts)

	if opts.Verbose {
		fmt.Fprintln(os.Stderr)
		ui.Phase(2, "Captain synthesizing council…")
	}
	spin := ui.NewSpinner(fmt.Sprintf("Synthesizing %d responses...", countSuccessful(outputs)))
	synthesis, err := synthesize(ctx, opts.Query, outputs, opts.CaptainModel, timeout)
	if err != nil {
		spin.Stop(false, fmt.Sprintf("Synthesis failed: %v", err))
		return nil, fmt.Errorf("synthesis: %w", err)
	}
	spin.Stop(true, "Synthesis complete")

	for round := 1; round <= opts.DebateRounds; round++ {
		if opts.Verbose {
			fmt.Fprintln(os.Stderr)
			ui.Phase(2+round, fmt.Sprintf("Debate round %d...", round))
		}
		spin = ui.NewSpinner(fmt.Sprintf("Collecting critiques (round %d)...", round))
		critiques := collectCritiques(ctx, workers, opts.Query, synthesis, "", concurrency, timeout, runOpts)
		nCritiques := countSuccessful(critiques)
		spin.Stop(true, fmt.Sprintf("%d critiques collected", nCritiques))

		if nCritiques > 0 {
			spin = ui.NewSpinner("Captain re-synthesizing...")
			newSynthesis, err := resynthesize(ctx, opts.Query, synthesis, critiques, opts.CaptainModel, timeout)
			if err != nil {
				spin.Stop(false, fmt.Sprintf("Re-synthesis failed: %v", err))
			} else {
				spin.Stop(true, "Re-synthesis complete")
				synthesis = newSynthesis
			}
		}
	}

	if opts.Verbose {
		ui.Complete("Council complete!")
	}

	names := make([]string, len(workers))
	for i, a := range workers {
		names[i] = a.Name
	}
	result := &Result{
		Query:        opts.Query,
		Agents:       names,
		Synthesis:    synthesis,
		DebateRounds: opts.DebateRounds,
		Mode:         "council",
	}
	if opts.JSON {
		result.Outputs = outputs
	}
	return result, nil
}

const councilWorkerPrompt = "You are one member of a diverse council of reasoning agents. " +
	"Each council member runs on a different model and is expected to bring their own perspective. " +
	"Answer the user's query thoroughly and independently, laying out reasoning, evidence, assumptions, " +
	"and confidence level so a coordinator can combine the council's perspectives."

func buildCouncilWorkers(models []string) []Agent {
	out := make([]Agent, len(models))
	for i, m := range models {
		out[i] = Agent{
			Name:         fmt.Sprintf("Council%d", i+1),
			Domain:       m, // surface the model in output so the user knows who said what
			SystemPrompt: councilWorkerPrompt,
			Model:        m,
		}
	}
	return out
}
