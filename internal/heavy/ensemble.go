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
	"github.com/reuben/scud/internal/llm"
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
	Native       bool   // Use xAI native multi-agent model instead of rho ensemble
	NativeEffort string // "low"/"medium" (4 agents) or "high"/"xhigh" (16 agents)
	NativeTools  []string // Server-side tools: "web_search", "x_search", "code_execution"
}

// Result holds the final output of a Heavy ensemble run.
type Result struct {
	Query        string        `json:"query"`
	Agents       []string      `json:"agents"`
	Outputs      []AgentOutput `json:"outputs,omitempty"`
	Synthesis    string        `json:"synthesis"`
	DebateRounds int           `json:"debate_rounds"`
	Mode         string        `json:"mode"` // "ensemble" or "native"
	InputTokens  int           `json:"input_tokens,omitempty"`
	OutputTokens int           `json:"output_tokens,omitempty"`
	TotalTokens  int           `json:"total_tokens,omitempty"`
}

// RunNative executes a query using the xAI native multi-agent model.
func RunNative(ctx context.Context, cfg *config.Config, opts RunOpts) (*Result, error) {
	model := opts.Model
	if model == "" {
		if cfg != nil && cfg.LLM.MultiAgentModel != "" {
			model = cfg.LLM.MultiAgentModel
		} else {
			model = "grok-4.20-multi-agent-beta-0309"
		}
	}
	effort := opts.NativeEffort
	if effort == "" {
		if cfg != nil && cfg.LLM.MultiAgentEffort != "" {
			effort = cfg.LLM.MultiAgentEffort
		} else {
			effort = "low"
		}
	}

	if opts.Verbose {
		agentCount := "4"
		if effort == "high" || effort == "xhigh" {
			agentCount = "16"
		}
		ui.Header("Heavy Native Multi-Agent", fmt.Sprintf("(model: %s, effort: %s, agents: %s)", model, effort, agentCount))
	}

	provider, err := llm.NewMultiAgentProvider()
	if err != nil {
		return nil, fmt.Errorf("multi-agent provider: %w", err)
	}

	spin := ui.NewSpinner("Running multi-agent query...")
	start := time.Now()

	req := &llm.MultiAgentRequest{
		Model:  model,
		Prompt: opts.Query,
		Effort: effort,
		Tools:  opts.NativeTools,
	}

	resp, err := provider.CompleteMultiAgent(ctx, req)
	elapsed := time.Since(start)

	if err != nil {
		spin.Stop(false, fmt.Sprintf("Failed: %v", err))
		return nil, err
	}
	spin.Stop(true, fmt.Sprintf("Complete (%.1fs, %d tokens)", elapsed.Seconds(), resp.TotalTokens))

	if opts.Verbose {
		ui.Complete("Heavy native multi-agent complete!")
	}

	return &Result{
		Query:        opts.Query,
		Agents:       []string{"native-multi-agent"},
		Synthesis:    resp.Text,
		Mode:         "native",
		InputTokens:  resp.InputTokens,
		OutputTokens: resp.OutputTokens,
		TotalTokens:  resp.TotalTokens,
	}, nil
}

// Run executes the full Heavy reasoning ensemble pipeline.
func Run(ctx context.Context, cfg *config.Config, opts RunOpts) (*Result, error) {
	if opts.Native {
		return RunNative(ctx, cfg, opts)
	}

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
		Mode:         "ensemble",
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

	// Track per-agent status for progress display
	type agentStatus struct {
		mu     sync.Mutex
		status string // current activity description
	}
	statusMap := make(map[string]*agentStatus)

	for _, agent := range agents {
		if agent.Name == "Captain" {
			continue
		}
		a := agent
		as := &agentStatus{status: "starting..."}
		statusMap[a.Name] = as

		g.Go(func() error {
			start := time.Now()

			// Adaptive timeout: base timeout as configured, but extend up to 3x
			// if the agent is still actively producing output (idle timeout = 60s)
			baseDur := time.Duration(timeoutSecs) * time.Second
			idleDur := 60 * time.Second
			maxDur := baseDur * 3
			agentCtx, adaptive := rho.NewAdaptiveTimeout(gctx, baseDur, idleDur, maxDur)

			// Stream callback for heartbeat and progress
			eventCount := 0
			callback := func(ev rho.StreamEvent) {
				adaptive.Heartbeat()
				eventCount++

				// Update agent status for progress display
				var statusMsg string
				switch ev.Type {
				case "tool_use":
					if ev.ToolName != "" {
						statusMsg = fmt.Sprintf("using %s...", ev.ToolName)
					}
				case "text_delta":
					// Only update periodically to avoid spamming
					if eventCount%50 == 0 {
						elapsed := time.Since(start).Seconds()
						statusMsg = fmt.Sprintf("writing (%.0fs)...", elapsed)
					}
				case "complete":
					statusMsg = "finishing..."
				}

				if statusMsg != "" {
					as.mu.Lock()
					as.status = statusMsg
					as.mu.Unlock()
				}
			}

			result, err := rho.RunStreaming(agentCtx, rho.Options{
				Prompt:       query,
				Model:        model,
				SystemPrompt: a.SystemPrompt,
				AllowedTools: a.Tools,
				WorkingDir:   opts.WorkingDir,
				TimeoutSecs:  timeoutSecs,
			}, callback)

			duration := time.Since(start).Seconds()
			out := AgentOutput{
				Name:     a.Name,
				Domain:   a.Domain,
				Duration: duration,
			}

			if err != nil {
				out.Failed = true
				if adaptive.Extended() {
					out.Error = fmt.Sprintf("%v (deadline extended due to activity)", err)
				} else {
					out.Error = err.Error()
				}
				if opts.Verbose {
					msg := fmt.Sprintf("%s: %v (%.1fs)", a.Name, err, duration)
					if adaptive.Extended() {
						msg += " (was extended)"
					}
					ui.Fail(msg)
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
					extra := ""
					if adaptive.Extended() {
						extra = " [extended]"
					}
					ui.Success(fmt.Sprintf("%s %s%s", a.Name, ui.Dim(fmt.Sprintf("(%.1fs)", duration)), extra))
				}
			}

			mu.Lock()
			outputs = append(outputs, out)
			mu.Unlock()
			return nil // don't kill the group on individual failure
		})
	}

	// Progress reporter: periodically show what agents are doing
	if opts.Verbose {
		progressDone := make(chan struct{})
		go func() {
			ticker := time.NewTicker(15 * time.Second)
			defer ticker.Stop()
			for {
				select {
				case <-progressDone:
					return
				case <-ticker.C:
					mu.Lock()
					nDone := len(outputs)
					mu.Unlock()
					nTotal := 0
					for _, a := range agents {
						if a.Name != "Captain" {
							nTotal++
						}
					}
					if nDone >= nTotal {
						return
					}
					// Show active agents and what they're doing
					var active []string
					for name, as := range statusMap {
						as.mu.Lock()
						s := as.status
						as.mu.Unlock()
						// Check if this agent is still running
						found := false
						mu.Lock()
						for _, o := range outputs {
							if o.Name == name {
								found = true
								break
							}
						}
						mu.Unlock()
						if !found {
							active = append(active, fmt.Sprintf("%s: %s", name, s))
						}
					}
					if len(active) > 0 {
						ui.Info(fmt.Sprintf("Still running (%d/%d done): %s",
							nDone, nTotal, strings.Join(active, ", ")))
					}
				}
			}
		}()
		defer close(progressDone)
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
