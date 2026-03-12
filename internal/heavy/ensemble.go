package heavy

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/rho"
	"github.com/reuben/scud/internal/ui"
	"golang.org/x/sync/errgroup"
)

// AgentOutput holds the result of a single agent's execution.
type AgentOutput struct {
	Name     string  `json:"name"`
	Domain   string  `json:"domain"`
	Output   string  `json:"output"`
	Duration float64 `json:"duration_secs"`
	Failed   bool    `json:"failed,omitempty"`
	Error    string  `json:"error,omitempty"`
}

// RunOpts configures a Heavy ensemble run.
type RunOpts struct {
	Query        string
	Model        string
	Concurrency  int
	DebateRounds int
	Verbose      bool
	JSON         bool
	WorkingDir   string
	TimeoutSecs  int
}

// Result holds the final output of a Heavy ensemble run.
type Result struct {
	Query        string        `json:"query"`
	Agents       []string      `json:"agents"`
	Outputs      []AgentOutput `json:"outputs,omitempty"`
	Synthesis    string        `json:"synthesis"`
	DebateRounds int           `json:"debate_rounds"`
}

// Run executes the full Heavy reasoning ensemble pipeline.
func Run(ctx context.Context, cfg *config.Config, opts RunOpts) (*Result, error) {
	// Resolve defaults
	model := resolveModel(opts.Model, cfg)
	concurrency := opts.Concurrency
	if concurrency <= 0 {
		concurrency = 4
		if cfg != nil && cfg.Heavy.Concurrency > 0 {
			concurrency = cfg.Heavy.Concurrency
		}
	}
	timeout := opts.TimeoutSecs
	if timeout <= 0 {
		timeout = 300
		if cfg != nil && cfg.Heavy.TimeoutSecs > 0 {
			timeout = cfg.Heavy.TimeoutSecs
		}
	}
	maxAgents := 0
	if cfg != nil {
		maxAgents = cfg.Heavy.MaxAgents
	}

	// Step 1: Route — Captain selects specialists
	if opts.Verbose {
		ui.Header("Heavy Ensemble", fmt.Sprintf("(model: %s)", model))
		ui.Phase(1, "Captain routing query to specialists...")
	}
	spin := ui.NewSpinner("Captain selecting agents...")
	selected, err := routeAgents(ctx, opts.Query, model, timeout)
	if err != nil {
		spin.Stop(false, fmt.Sprintf("Routing failed: %v", err))
		return nil, fmt.Errorf("routing: %w", err)
	}

	// Merge with core agents and dedup
	agents := mergeAgents(selected, maxAgents)
	agentNames := make([]string, len(agents))
	for i, a := range agents {
		agentNames[i] = a.Name
	}
	spin.Stop(true, fmt.Sprintf("Selected: %s (%d agents)", strings.Join(agentNames, ", "), len(agents)))

	// Step 2: Parallel execution (skip Captain)
	if opts.Verbose {
		fmt.Fprintln(os.Stderr)
		ui.Phase(2, fmt.Sprintf("Running %d agents (concurrency=%d)...", len(agents)-1, concurrency))
	}
	outputs := executeAgents(ctx, agents, opts.Query, model, concurrency, timeout, opts)

	// Step 3: Synthesize
	if opts.Verbose {
		fmt.Fprintln(os.Stderr)
		ui.Phase(3, "Captain synthesizing responses...")
	}
	spin = ui.NewSpinner(fmt.Sprintf("Synthesizing %d responses...", countSuccessful(outputs)))
	synthesis, err := synthesize(ctx, opts.Query, outputs, model, timeout)
	if err != nil {
		spin.Stop(false, fmt.Sprintf("Synthesis failed: %v", err))
		return nil, fmt.Errorf("synthesis: %w", err)
	}
	spin.Stop(true, "Synthesis complete")

	// Step 4: Debate rounds
	for round := 1; round <= opts.DebateRounds; round++ {
		if opts.Verbose {
			fmt.Fprintln(os.Stderr)
			ui.Phase(3+round, fmt.Sprintf("Debate round %d...", round))
		}
		spin = ui.NewSpinner(fmt.Sprintf("Collecting critiques (round %d)...", round))
		critiques := collectCritiques(ctx, agents, opts.Query, synthesis, model, concurrency, timeout, opts)
		nCritiques := countSuccessful(critiques)
		spin.Stop(true, fmt.Sprintf("%d critiques collected", nCritiques))

		if nCritiques > 0 {
			spin = ui.NewSpinner("Captain re-synthesizing...")
			newSynthesis, err := resynthesize(ctx, opts.Query, synthesis, critiques, model, timeout)
			if err != nil {
				spin.Stop(false, fmt.Sprintf("Re-synthesis failed: %v", err))
			} else {
				spin.Stop(true, "Re-synthesis complete")
				synthesis = newSynthesis
			}
		}
	}

	if opts.Verbose {
		ui.Complete("Heavy ensemble complete!")
	}

	result := &Result{
		Query:        opts.Query,
		Agents:       agentNames,
		Synthesis:    synthesis,
		DebateRounds: opts.DebateRounds,
	}
	if opts.JSON {
		result.Outputs = outputs
	}

	return result, nil
}

func resolveModel(override string, cfg *config.Config) string {
	if override != "" {
		return override
	}
	if cfg != nil {
		if cfg.Heavy.Model != "" {
			return cfg.Heavy.Model
		}
		if cfg.Rho.SmartModel != "" {
			return cfg.Rho.SmartModel
		}
	}
	return "grok-4.20-reasoning"
}

func routeAgents(ctx context.Context, query, model string, timeoutSecs int) ([]string, error) {
	routeCtx, cancel := context.WithTimeout(ctx, 60*time.Second)
	defer cancel()

	prompt := RoutingPrompt(query, Specialists())
	captain, _ := FindByName("Captain")

	names, err := rho.RunJSON[[]string](routeCtx, rho.Options{
		Prompt:       prompt,
		Model:        model,
		SystemPrompt: captain.SystemPrompt,
		TimeoutSecs:  timeoutSecs,
	})
	if err != nil {
		return nil, err
	}
	return names, nil
}

func mergeAgents(selected []string, maxAgents int) []Agent {
	seen := make(map[string]bool)
	var result []Agent

	// Core agents first
	for _, a := range CoreAgents() {
		seen[strings.ToLower(a.Name)] = true
		result = append(result, a)
	}

	// Add selected specialists
	for _, name := range selected {
		lower := strings.ToLower(name)
		if seen[lower] {
			continue
		}
		if a, ok := FindByName(name); ok && !a.IsCore {
			seen[lower] = true
			result = append(result, a)
		}
	}

	if maxAgents > 0 && len(result) > maxAgents {
		result = result[:maxAgents]
	}
	return result
}

func executeAgents(ctx context.Context, agents []Agent, query, model string, concurrency, timeoutSecs int, opts RunOpts) []AgentOutput {
	var mu sync.Mutex
	var outputs []AgentOutput

	g, gctx := errgroup.WithContext(ctx)
	g.SetLimit(concurrency)

	for _, agent := range agents {
		if agent.Name == "Captain" {
			continue
		}
		a := agent
		g.Go(func() error {
			start := time.Now()
			agentCtx, cancel := context.WithTimeout(gctx, time.Duration(timeoutSecs)*time.Second)
			defer cancel()

			result, err := rho.Run(agentCtx, rho.Options{
				Prompt:       query,
				Model:        model,
				SystemPrompt: a.SystemPrompt,
				AllowedTools: a.Tools,
				WorkingDir:   opts.WorkingDir,
				TimeoutSecs:  timeoutSecs,
			})

			duration := time.Since(start).Seconds()
			out := AgentOutput{
				Name:     a.Name,
				Domain:   a.Domain,
				Duration: duration,
			}

			if err != nil {
				out.Failed = true
				out.Error = err.Error()
				if opts.Verbose {
					ui.Fail(fmt.Sprintf("%s: %v (%.1fs)", a.Name, err, duration))
				}
			} else if result.ExitCode != 0 {
				out.Failed = true
				out.Error = fmt.Sprintf("exit code %d", result.ExitCode)
				out.Output = result.Stdout
				if opts.Verbose {
					ui.Fail(fmt.Sprintf("%s: exit code %d (%.1fs)", a.Name, result.ExitCode, duration))
				}
			} else {
				out.Output = result.Stdout
				if opts.Verbose {
					ui.Success(fmt.Sprintf("%s %s", a.Name, ui.Dim(fmt.Sprintf("(%.1fs)", duration))))
				}
			}

			mu.Lock()
			outputs = append(outputs, out)
			mu.Unlock()
			return nil // don't kill the group on individual failure
		})
	}

	_ = g.Wait()
	return outputs
}

func synthesize(ctx context.Context, query string, outputs []AgentOutput, model string, timeoutSecs int) (string, error) {
	synthCtx, cancel := context.WithTimeout(ctx, 120*time.Second)
	defer cancel()

	captain, _ := FindByName("Captain")
	prompt := SynthesisPrompt(query, outputs)

	result, err := rho.Run(synthCtx, rho.Options{
		Prompt:       prompt,
		Model:        model,
		SystemPrompt: captain.SystemPrompt,
		TimeoutSecs:  timeoutSecs,
	})
	if err != nil {
		return "", err
	}
	if result.ExitCode != 0 {
		return "", fmt.Errorf("synthesis exited with code %d: %s", result.ExitCode, result.Stderr)
	}
	return result.Stdout, nil
}

func collectCritiques(ctx context.Context, agents []Agent, query, synthesis, model string, concurrency, timeoutSecs int, opts RunOpts) []AgentOutput {
	var mu sync.Mutex
	var critiques []AgentOutput

	g, gctx := errgroup.WithContext(ctx)
	g.SetLimit(concurrency)

	prompt := CritiquePrompt(query, synthesis)

	for _, agent := range agents {
		if agent.Name == "Captain" {
			continue
		}
		a := agent
		g.Go(func() error {
			start := time.Now()
			agentCtx, cancel := context.WithTimeout(gctx, time.Duration(timeoutSecs)*time.Second)
			defer cancel()

			result, err := rho.Run(agentCtx, rho.Options{
				Prompt:       prompt,
				Model:        model,
				SystemPrompt: a.SystemPrompt,
				TimeoutSecs:  timeoutSecs,
			})

			duration := time.Since(start).Seconds()
			out := AgentOutput{
				Name:     a.Name,
				Domain:   a.Domain,
				Duration: duration,
			}

			if err != nil {
				out.Failed = true
				out.Error = err.Error()
			} else if result.ExitCode != 0 {
				out.Failed = true
				out.Error = fmt.Sprintf("exit code %d", result.ExitCode)
			} else {
				out.Output = result.Stdout
			}

			mu.Lock()
			critiques = append(critiques, out)
			mu.Unlock()
			return nil
		})
	}

	_ = g.Wait()
	return critiques
}

func resynthesize(ctx context.Context, query, synthesis string, critiques []AgentOutput, model string, timeoutSecs int) (string, error) {
	rsCtx, cancel := context.WithTimeout(ctx, 120*time.Second)
	defer cancel()

	captain, _ := FindByName("Captain")
	prompt := ResynthesisPrompt(query, synthesis, critiques)

	result, err := rho.Run(rsCtx, rho.Options{
		Prompt:       prompt,
		Model:        model,
		SystemPrompt: captain.SystemPrompt,
		TimeoutSecs:  timeoutSecs,
	})
	if err != nil {
		return "", err
	}
	if result.ExitCode != 0 {
		return "", fmt.Errorf("re-synthesis exited with code %d: %s", result.ExitCode, result.Stderr)
	}
	return result.Stdout, nil
}

func countSuccessful(outputs []AgentOutput) int {
	n := 0
	for _, o := range outputs {
		if !o.Failed {
			n++
		}
	}
	return n
}

// PrintResult outputs the result to stdout.
func PrintResult(result *Result, jsonOutput bool) error {
	if jsonOutput {
		enc := json.NewEncoder(os.Stdout)
		enc.SetIndent("", "  ")
		return enc.Encode(result)
	}
	fmt.Print(result.Synthesis)
	if !strings.HasSuffix(result.Synthesis, "\n") {
		fmt.Println()
	}
	return nil
}
