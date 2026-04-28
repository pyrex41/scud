package heavy

import (
	"context"
	"fmt"
	"os"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/ui"
)

// SquadOpts configures a `scud squad` run: one smart captain + N identical
// worker clones, each running the same (typically cheap or local) model.
//
// This is the "command and conquer" variant — the captain gets the hard job
// (routing-free synthesis from diverse takes on the same query) while the
// workers are a swarm of one model running in parallel.
type SquadOpts struct {
	Query        string
	Workers      int
	WorkerModel  string
	CaptainModel string
	Concurrency  int
	DebateRounds int
	Verbose      bool
	JSON         bool
	WorkingDir   string
	TimeoutSecs  int
}

// RunSquad fans out N clone workers and synthesizes their outputs. Unlike
// `scud heavy`, there is no LLM-driven routing step — the worker roster is
// static, so no call can flake before the real work begins.
func RunSquad(ctx context.Context, cfg *config.Config, opts SquadOpts) (*Result, error) {
	if opts.Workers <= 0 {
		opts.Workers = 4
	}
	if opts.WorkerModel == "" {
		return nil, fmt.Errorf("--worker-model is required")
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
		concurrency = opts.Workers // squad typically wants to saturate
	}
	timeout := opts.TimeoutSecs
	if timeout <= 0 {
		timeout = 300
		if cfg != nil && cfg.Heavy.TimeoutSecs > 0 {
			timeout = cfg.Heavy.TimeoutSecs
		}
	}

	workers := buildSquadWorkers(opts.Workers, opts.WorkerModel)

	if opts.Verbose {
		ui.Header("Scud Squad", fmt.Sprintf(
			"(captain: %s, workers: %d × %s)",
			opts.CaptainModel, opts.Workers, opts.WorkerModel,
		))
		fmt.Fprintln(os.Stderr)
		ui.Phase(1, fmt.Sprintf("Running %d worker clones (concurrency=%d)...", opts.Workers, concurrency))
	}

	// executeAgents honors each Agent.Model override, so the second arg
	// here is effectively a shared fallback (unused when every worker has
	// Model set, which they do).
	runOpts := RunOpts{
		Verbose:    opts.Verbose,
		WorkingDir: opts.WorkingDir,
	}
	outputs := executeAgents(ctx, workers, opts.Query, opts.WorkerModel, concurrency, timeout, runOpts)

	if opts.Verbose {
		fmt.Fprintln(os.Stderr)
		ui.Phase(2, "Captain synthesizing...")
	}
	spin := ui.NewSpinner(fmt.Sprintf("Synthesizing %d responses...", countSuccessful(outputs)))
	synthesis, err := synthesize(ctx, opts.Query, outputs, opts.CaptainModel, timeout)
	if err != nil {
		spin.Stop(false, fmt.Sprintf("Synthesis failed: %v", err))
		return nil, fmt.Errorf("synthesis: %w", err)
	}
	spin.Stop(true, "Synthesis complete")

	// Optional debate — reuses the existing machinery.
	for round := 1; round <= opts.DebateRounds; round++ {
		if opts.Verbose {
			fmt.Fprintln(os.Stderr)
			ui.Phase(2+round, fmt.Sprintf("Debate round %d...", round))
		}
		spin = ui.NewSpinner(fmt.Sprintf("Collecting critiques (round %d)...", round))
		critiques := collectCritiques(ctx, workers, opts.Query, synthesis, opts.WorkerModel, concurrency, timeout, runOpts)
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
		ui.Complete("Squad complete!")
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
		Mode:         "squad",
	}
	if opts.JSON {
		result.Outputs = outputs
	}
	return result, nil
}

const squadWorkerPrompt = "You are one member of a team of reasoning agents. " +
	"Answer the user's query thoroughly and independently — do not coordinate with other members. " +
	"Lay out your reasoning, evidence, assumptions, and final answer clearly so a coordinator can combine perspectives."

// squadWorkerTools is the default toolset for squad/council workers. Read-only
// plus Bash so workers can explore a codebase or check state, but can't write
// or edit — captain-driven synthesis expects the workers to *analyze*, not
// mutate. If a use case needs write access, fork the agent definition.
var squadWorkerTools = []string{"Read", "Grep", "Glob", "Bash"}

func buildSquadWorkers(n int, model string) []Agent {
	out := make([]Agent, n)
	for i := 0; i < n; i++ {
		out[i] = Agent{
			Name:         fmt.Sprintf("Worker%d", i+1),
			Domain:       "General reasoning",
			SystemPrompt: squadWorkerPrompt,
			Tools:        squadWorkerTools,
			Model:        model,
		}
	}
	return out
}
