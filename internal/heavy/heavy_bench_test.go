package heavy

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/rho"
)

// --- Unit tests (no API calls) ---

func TestResolveModelsPrecedence(t *testing.T) {
	tests := []struct {
		name string
		opts RunOpts
		cfg  *config.Config
		role string
		want string
	}{
		{
			name: "CLI per-role override wins",
			opts: RunOpts{ModelAgents: "cli-agents", ModelAll: "cli-all"},
			cfg:  config.Default(),
			role: "agents",
			want: "cli-agents",
		},
		{
			name: "CLI model-all fallback",
			opts: RunOpts{ModelAll: "cli-all"},
			cfg:  config.Default(),
			role: "agents",
			want: "cli-all",
		},
		{
			name: "per-role config",
			opts: RunOpts{},
			cfg: &config.Config{
				Heavy: config.HeavyConfig{
					Models: config.HeavyModelsConfig{Agents: "cfg-agents"},
					Model:  "cfg-all",
				},
				Rho: config.RhoConfig{SmartModel: "smart-model"},
			},
			role: "agents",
			want: "cfg-agents",
		},
		{
			name: "heavy.model fallback",
			opts: RunOpts{},
			cfg: &config.Config{
				Heavy: config.HeavyConfig{Model: "heavy-model"},
				Rho:   config.RhoConfig{SmartModel: "smart-model"},
			},
			role: "synthesis",
			want: "heavy-model",
		},
		{
			name: "rho smart model fallback",
			opts: RunOpts{},
			cfg: &config.Config{
				Heavy: config.HeavyConfig{},
				Rho:   config.RhoConfig{SmartModel: "smart-model"},
			},
			role: "routing",
			want: "smart-model",
		},
		{
			name: "nil config uses default",
			opts: RunOpts{},
			cfg:  nil,
			role: "agents",
			want: "grok-4.20-reasoning",
		},
		{
			name: "native role uses LLM config",
			opts: RunOpts{},
			cfg: &config.Config{
				LLM: config.LLMConfig{MultiAgentModel: "native-model"},
			},
			role: "native",
			want: "native-model",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			models := resolveModels(tt.opts, tt.cfg)
			var got string
			switch tt.role {
			case "routing":
				got = models.routing
			case "agents":
				got = models.agents
			case "synthesis":
				got = models.synthesis
			case "debate":
				got = models.debate
			case "native":
				got = models.native
			}
			if got != tt.want {
				t.Errorf("resolveModels().%s = %q, want %q", tt.role, got, tt.want)
			}
		})
	}
}

func TestMergeAgentsOrdering(t *testing.T) {
	// Core agents should always come first
	result := mergeAgents([]string{"Euler", "Curie"}, 0)
	if len(result) < 4 {
		t.Fatal("expected at least 4 core agents")
	}
	for i, a := range result[:4] {
		if !a.IsCore {
			t.Errorf("agent at index %d (%s) should be core", i, a.Name)
		}
	}
	// Specialists should follow
	for i := 4; i < len(result); i++ {
		if result[i].IsCore {
			t.Errorf("agent at index %d (%s) should be specialist", i, result[i].Name)
		}
	}
}

func TestMergeAgentsInvalidNames(t *testing.T) {
	// Invalid specialist names should be silently ignored
	result := mergeAgents([]string{"NotAnAgent", "AlsoFake", "Euler"}, 0)
	names := make(map[string]bool)
	for _, a := range result {
		names[a.Name] = true
	}
	if names["NotAnAgent"] || names["AlsoFake"] {
		t.Error("invalid agent names should be ignored")
	}
	if !names["Euler"] {
		t.Error("valid specialist Euler should be included")
	}
	// Should have 4 core + 1 valid specialist = 5
	if len(result) != 5 {
		t.Errorf("got %d agents, want 5", len(result))
	}
}

func TestMergeAgentsEmptySelected(t *testing.T) {
	result := mergeAgents(nil, 0)
	if len(result) != 4 {
		t.Errorf("empty selected should return only 4 core agents, got %d", len(result))
	}
}

func TestMergeAgentsMaxCapAtCoreMinimum(t *testing.T) {
	// MaxAgents=2 should still work (truncate to 2)
	result := mergeAgents([]string{"Euler", "Curie"}, 2)
	if len(result) != 2 {
		t.Errorf("maxAgents=2 should cap at 2, got %d", len(result))
	}
}

func TestCountSuccessful(t *testing.T) {
	outputs := []AgentOutput{
		{Name: "A", Failed: false},
		{Name: "B", Failed: true},
		{Name: "C", Failed: false},
		{Name: "D", Failed: true},
		{Name: "E", Failed: false},
	}
	if got := countSuccessful(outputs); got != 3 {
		t.Errorf("countSuccessful() = %d, want 3", got)
	}
}

func TestCountSuccessfulEmpty(t *testing.T) {
	if got := countSuccessful(nil); got != 0 {
		t.Errorf("countSuccessful(nil) = %d, want 0", got)
	}
}

func TestSynthesisPromptNoSuccessfulAgents(t *testing.T) {
	outputs := []AgentOutput{
		{Name: "A", Failed: true, Error: "timeout"},
		{Name: "B", Failed: true, Error: "error"},
	}
	prompt := SynthesisPrompt("query", outputs)
	// Should not contain any agent output sections
	if strings.Contains(prompt, "## A") || strings.Contains(prompt, "## B") {
		t.Error("synthesis prompt should not include failed agents")
	}
}

func TestSynthesisPromptMixedOutputLengths(t *testing.T) {
	short := "short output"
	exactly4000 := strings.Repeat("x", maxOutputLen)
	over4000 := strings.Repeat("y", maxOutputLen+500)

	outputs := []AgentOutput{
		{Name: "Short", Domain: "D1", Output: short},
		{Name: "Exact", Domain: "D2", Output: exactly4000},
		{Name: "Long", Domain: "D3", Output: over4000},
	}
	prompt := SynthesisPrompt("query", outputs)

	if !strings.Contains(prompt, short) {
		t.Error("short output should be included verbatim")
	}
	if !strings.Contains(prompt, exactly4000) {
		t.Error("exactly maxOutputLen output should be included verbatim")
	}
	if strings.Contains(prompt, over4000) {
		t.Error("over-limit output should be truncated")
	}
	if !strings.Contains(prompt, "truncated") {
		t.Error("truncated output should include truncation marker")
	}
}

func TestRoutingPromptFormat(t *testing.T) {
	prompt := RoutingPrompt("How do I build a REST API?", Specialists())
	// Should contain JSON array example
	if !strings.Contains(prompt, `["Captain"`) {
		t.Error("routing prompt should contain JSON array example")
	}
	// Should mention all specialists by name
	for _, s := range Specialists() {
		if !strings.Contains(prompt, s.Name) {
			t.Errorf("routing prompt should mention specialist %s", s.Name)
		}
		if !strings.Contains(prompt, s.Domain) {
			t.Errorf("routing prompt should mention domain %s", s.Domain)
		}
	}
}

func TestAgentToolsConsistency(t *testing.T) {
	validTools := map[string]bool{
		"Bash": true, "Read": true, "Grep": true, "Glob": true,
	}
	for _, a := range Registry() {
		for _, tool := range a.Tools {
			if !validTools[tool] {
				t.Errorf("agent %s has unknown tool %q", a.Name, tool)
			}
		}
	}
}

func TestAgentSystemPromptsNonEmpty(t *testing.T) {
	for _, a := range Registry() {
		if a.SystemPrompt == "" {
			t.Errorf("agent %s has empty system prompt", a.Name)
		}
		if len(a.SystemPrompt) < 50 {
			t.Errorf("agent %s system prompt too short (%d chars)", a.Name, len(a.SystemPrompt))
		}
	}
}

func TestAgentDomainsUnique(t *testing.T) {
	domains := make(map[string]string) // domain -> agent name
	for _, a := range Registry() {
		if prev, ok := domains[a.Domain]; ok {
			t.Errorf("duplicate domain %q: %s and %s", a.Domain, prev, a.Name)
		}
		domains[a.Domain] = a.Name
	}
}

func TestAgentNamesUnique(t *testing.T) {
	names := make(map[string]bool)
	for _, a := range Registry() {
		lower := strings.ToLower(a.Name)
		if names[lower] {
			t.Errorf("duplicate agent name: %s", a.Name)
		}
		names[lower] = true
	}
}

func TestPrintResultText(t *testing.T) {
	result := &Result{
		Query:     "test",
		Agents:    []string{"Captain"},
		Synthesis: "This is the answer.",
	}
	// Should not panic
	// (stdout capture not easy in a unit test, just verify no error)
	old := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	err := PrintResult(result, false)

	w.Close()
	os.Stdout = old

	if err != nil {
		t.Errorf("PrintResult text mode: %v", err)
	}

	buf := make([]byte, 1024)
	n, _ := r.Read(buf)
	output := string(buf[:n])
	if !strings.Contains(output, "This is the answer.") {
		t.Errorf("PrintResult text should contain synthesis, got: %s", output)
	}
}

func TestPrintResultJSON(t *testing.T) {
	result := &Result{
		Query:     "test query",
		Agents:    []string{"Captain", "Harper"},
		Synthesis: "Combined answer.",
		Outputs: []AgentOutput{
			{Name: "Harper", Domain: "Research", Output: "found stuff"},
		},
	}

	old := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	err := PrintResult(result, true)

	w.Close()
	os.Stdout = old

	if err != nil {
		t.Errorf("PrintResult JSON mode: %v", err)
	}

	buf := make([]byte, 8192)
	n, _ := r.Read(buf)
	output := string(buf[:n])

	var parsed Result
	if err := json.Unmarshal([]byte(output), &parsed); err != nil {
		t.Errorf("PrintResult JSON should be valid JSON: %v\nGot: %s", err, output)
	}
	if parsed.Query != "test query" {
		t.Errorf("parsed query = %q, want %q", parsed.Query, "test query")
	}
	if len(parsed.Outputs) != 1 {
		t.Errorf("parsed outputs count = %d, want 1", len(parsed.Outputs))
	}
}

func TestCritiquePromptContainsQueryAndSynthesis(t *testing.T) {
	p := CritiquePrompt("what is Go?", "Go is a programming language.")
	if !strings.Contains(p, "what is Go?") {
		t.Error("critique prompt should contain query")
	}
	if !strings.Contains(p, "Go is a programming language.") {
		t.Error("critique prompt should contain synthesis")
	}
	if !strings.Contains(p, "critique") || !strings.Contains(p, "weakness") {
		t.Error("critique prompt should instruct to critique")
	}
}

func TestResynthesisPromptSkipsFailedCritiques(t *testing.T) {
	critiques := []AgentOutput{
		{Name: "Good", Output: "valid feedback"},
		{Name: "Bad", Output: "ignored", Failed: true},
		{Name: "Also Good", Output: "more feedback"},
	}
	p := ResynthesisPrompt("q", "synth", critiques)
	if !strings.Contains(p, "Good critique") {
		t.Error("should include successful critique")
	}
	if strings.Contains(p, "Bad critique") {
		t.Error("should skip failed critique")
	}
	if !strings.Contains(p, "Also Good critique") {
		t.Error("should include second successful critique")
	}
}

// --- Integration tests (require XAI_API_KEY + rho-cli) ---

func skipWithoutAPI(t *testing.T) {
	t.Helper()
	if os.Getenv("XAI_API_KEY") == "" {
		t.Skip("XAI_API_KEY not set, skipping integration test")
	}
	if _, err := os.Stat("/Users/reuben/.cargo/bin/rho-cli"); err != nil {
		t.Skip("rho-cli not found, skipping integration test")
	}
}

func TestIntegrationRoutingOnly(t *testing.T) {
	skipWithoutAPI(t)

	start := time.Now()
	model := "grok-4.20-beta-0309-reasoning"

	selected, err := routeAgents(context.Background(), "Explain the P vs NP problem", model, 60)
	elapsed := time.Since(start)

	if err != nil {
		t.Fatalf("routeAgents: %v", err)
	}

	t.Logf("Routing took %s, selected %d specialists: %v", elapsed, len(selected), selected)

	// Should have at least selected some agents
	if len(selected) == 0 {
		t.Error("routing should select at least some agents")
	}

	// For a math/CS question, Euler should likely be selected
	hasEuler := false
	for _, name := range selected {
		if strings.EqualFold(name, "euler") {
			hasEuler = true
		}
	}
	t.Logf("Euler selected: %v (expected for P vs NP query)", hasEuler)
}

func TestIntegrationRoutingLatency(t *testing.T) {
	skipWithoutAPI(t)

	queries := []struct {
		name  string
		query string
	}{
		{"simple_math", "What is 2+2?"},
		{"code_question", "How do I parse JSON in Go?"},
		{"complex_analysis", "Compare the economic policies of Keynesianism vs Austrian economics"},
		{"creative", "Write a haiku about programming"},
	}

	for _, q := range queries {
		t.Run(q.name, func(t *testing.T) {
			start := time.Now()
			selected, err := routeAgents(context.Background(), q.query, "grok-code-fast-1", 60)
			elapsed := time.Since(start)

			if err != nil {
				t.Fatalf("routeAgents: %v", err)
			}
			t.Logf("Query=%q routing=%s specialists=%v", q.query, elapsed, selected)
		})
	}
}

func TestIntegrationSingleAgentExecution(t *testing.T) {
	skipWithoutAPI(t)

	model := "grok-code-fast-1"
	query := "What is 2+2? Reply with just the number."

	agent := Agent{
		Name:         "TestAgent",
		Domain:       "Math",
		SystemPrompt: "You are a math expert. Be extremely concise.",
		Tools:        nil,
	}

	start := time.Now()
	result, err := rho.Run(context.Background(), rho.Options{
		Prompt:       query,
		Model:        model,
		SystemPrompt: agent.SystemPrompt,
		TimeoutSecs:  30,
	})
	elapsed := time.Since(start)

	if err != nil {
		t.Fatalf("rho.Run: %v", err)
	}

	t.Logf("Single agent took %s, exit=%d, output=%q", elapsed, result.ExitCode, strings.TrimSpace(result.Stdout))

	if result.ExitCode != 0 {
		t.Errorf("expected exit code 0, got %d: stderr=%s", result.ExitCode, result.Stderr)
	}
	if !strings.Contains(result.Stdout, "4") {
		t.Errorf("expected output to contain '4', got %q", result.Stdout)
	}
}

func TestIntegrationParallelExecution(t *testing.T) {
	skipWithoutAPI(t)

	agents := []Agent{
		{Name: "A", Domain: "Math", SystemPrompt: "You are a math expert. Be concise.", Tools: nil},
		{Name: "Captain", Domain: "Coord", SystemPrompt: "Coordinate.", Tools: nil}, // Should be skipped
		{Name: "B", Domain: "Logic", SystemPrompt: "You are a logic expert. Be concise.", Tools: nil},
	}

	query := "What is 3+5? Just the number."
	opts := RunOpts{
		Query:       query,
		Verbose:     false,
		WorkingDir:  "/tmp",
		TimeoutSecs: 30,
	}

	start := time.Now()
	outputs := executeAgents(context.Background(), agents, query, "grok-code-fast-1", 3, 30, opts)
	elapsed := time.Since(start)

	t.Logf("Parallel execution of %d agents took %s", len(agents)-1, elapsed)

	// Captain should be skipped
	for _, o := range outputs {
		if o.Name == "Captain" {
			t.Error("Captain should not be executed")
		}
	}

	// Should have 2 outputs (A and B, not Captain)
	if len(outputs) != 2 {
		t.Errorf("expected 2 outputs (skipping Captain), got %d", len(outputs))
	}

	for _, o := range outputs {
		t.Logf("  %s: failed=%v duration=%.1fs output=%q",
			o.Name, o.Failed, o.Duration, strings.TrimSpace(o.Output)[:min(50, len(strings.TrimSpace(o.Output)))])
	}
}

func TestIntegrationFullEnsembleMinimal(t *testing.T) {
	skipWithoutAPI(t)

	cfg := config.Default()
	cfg.Heavy.Concurrency = 3
	cfg.Heavy.MaxAgents = 5 // Cap to keep it fast

	start := time.Now()
	result, err := Run(context.Background(), cfg, RunOpts{
		Query:        "What is the capital of France? One word answer.",
		ModelAll:     "grok-code-fast-1",
		Concurrency:  3,
		DebateRounds: 0,
		Verbose:      false,
		JSON:         true,
		WorkingDir:   "/tmp",
		TimeoutSecs:  60,
	})
	elapsed := time.Since(start)

	if err != nil {
		t.Fatalf("Run: %v", err)
	}

	t.Logf("Full ensemble took %s", elapsed)
	t.Logf("  Agents: %v", result.Agents)
	t.Logf("  Synthesis length: %d chars", len(result.Synthesis))

	if !strings.Contains(strings.ToLower(result.Synthesis), "paris") {
		t.Errorf("synthesis should mention Paris, got: %s", result.Synthesis[:min(200, len(result.Synthesis))])
	}

	// Check outputs are populated (JSON mode)
	successful := 0
	for _, o := range result.Outputs {
		if !o.Failed {
			successful++
		}
		t.Logf("  %s: failed=%v duration=%.1fs", o.Name, o.Failed, o.Duration)
	}

	if successful == 0 {
		t.Error("expected at least one successful agent output")
	}
}

func TestIntegrationFullEnsembleWithDebate(t *testing.T) {
	skipWithoutAPI(t)

	cfg := config.Default()

	start := time.Now()
	result, err := Run(context.Background(), cfg, RunOpts{
		Query:        "Is Go or Rust better for CLI tools? Give a brief opinionated take.",
		ModelAll:     "grok-code-fast-1",
		Concurrency:  3,
		DebateRounds: 1,
		Verbose:      false,
		JSON:         true,
		WorkingDir:   "/tmp",
		TimeoutSecs:  60,
	})
	elapsed := time.Since(start)

	if err != nil {
		t.Fatalf("Run with debate: %v", err)
	}

	t.Logf("Full ensemble with 1 debate round took %s", elapsed)
	t.Logf("  Agents: %v", result.Agents)
	t.Logf("  Debate rounds: %d", result.DebateRounds)
	t.Logf("  Synthesis length: %d chars", len(result.Synthesis))

	if result.DebateRounds != 1 {
		t.Errorf("expected 1 debate round, got %d", result.DebateRounds)
	}
	if result.Synthesis == "" {
		t.Error("synthesis should not be empty")
	}
}

func TestIntegrationTimeout(t *testing.T) {
	skipWithoutAPI(t)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	_, err := Run(ctx, config.Default(), RunOpts{
		Query:        "Explain quantum mechanics in detail",
		ModelAll:     "grok-code-fast-1",
		Concurrency:  1,
		DebateRounds: 0,
		Verbose:      false,
		WorkingDir:   "/tmp",
		TimeoutSecs:  3,
	})

	// Should either succeed fast or timeout — either is acceptable
	if err != nil {
		t.Logf("Run timed out as expected: %v", err)
	} else {
		t.Logf("Run completed within timeout")
	}
}

// --- Benchmarks ---

func BenchmarkRoutingPromptGeneration(b *testing.B) {
	specialists := Specialists()
	query := "How do I build a REST API in Go with authentication?"
	for i := 0; i < b.N; i++ {
		_ = RoutingPrompt(query, specialists)
	}
}

func BenchmarkSynthesisPromptGeneration(b *testing.B) {
	outputs := make([]AgentOutput, 8)
	for i := range outputs {
		outputs[i] = AgentOutput{
			Name:   fmt.Sprintf("Agent%d", i),
			Domain: fmt.Sprintf("Domain%d", i),
			Output: strings.Repeat("Analysis paragraph. ", 200),
		}
	}
	query := "Complex multi-domain question"
	for i := 0; i < b.N; i++ {
		_ = SynthesisPrompt(query, outputs)
	}
}

func BenchmarkMergeAgents(b *testing.B) {
	selected := []string{"Euler", "Curie", "Lovelace", "Turing", "Sappho"}
	for i := 0; i < b.N; i++ {
		_ = mergeAgents(selected, 0)
	}
}

func BenchmarkFindByName(b *testing.B) {
	names := []string{"Captain", "euler", "LOVELACE", "nonexistent", "Harper"}
	for i := 0; i < b.N; i++ {
		FindByName(names[i%len(names)])
	}
}

func BenchmarkCountSuccessful(b *testing.B) {
	outputs := make([]AgentOutput, 16)
	for i := range outputs {
		outputs[i] = AgentOutput{Failed: i%3 == 0}
	}
	for i := 0; i < b.N; i++ {
		countSuccessful(outputs)
	}
}

// --- Performance harness (only with API) ---

func TestBenchmarkLiveRouting(t *testing.T) {
	skipWithoutAPI(t)

	models := []string{"grok-code-fast-1"}
	queries := []string{
		"What is 2+2?",
		"Explain the security implications of JWT vs session tokens",
		"How does Go's garbage collector work?",
	}

	t.Logf("\n=== Live Routing Benchmark ===")
	t.Logf("%-15s %-60s %10s %s", "Model", "Query", "Latency", "Selected")
	t.Logf("%s", strings.Repeat("-", 120))

	for _, model := range models {
		for _, query := range queries {
			start := time.Now()
			selected, err := routeAgents(context.Background(), query, model, 60)
			elapsed := time.Since(start)

			if err != nil {
				t.Logf("%-15s %-60s %10s ERROR: %v", model, truncQuery(query, 55), elapsed, err)
				continue
			}
			t.Logf("%-15s %-60s %10s %v", model, truncQuery(query, 55), elapsed, selected)
		}
	}
}

func TestBenchmarkLiveSingleAgent(t *testing.T) {
	skipWithoutAPI(t)

	model := "grok-code-fast-1"
	query := "Name three Go standard library packages. Just the names, one per line."

	agents := []Agent{
		{Name: "NoTools", Domain: "General", SystemPrompt: "Be concise.", Tools: nil},
		{Name: "WithRead", Domain: "General", SystemPrompt: "Be concise.", Tools: []string{"Read"}},
	}

	t.Logf("\n=== Single Agent Benchmark ===")
	t.Logf("%-15s %-10s %10s %5s %s", "Agent", "Tools", "Latency", "Exit", "Output Preview")
	t.Logf("%s", strings.Repeat("-", 100))

	for _, agent := range agents {
		start := time.Now()
		result, err := rho.Run(context.Background(), rho.Options{
			Prompt:       query,
			Model:        model,
			SystemPrompt: agent.SystemPrompt,
			AllowedTools: agent.Tools,
			TimeoutSecs:  30,
		})
		elapsed := time.Since(start)

		if err != nil {
			t.Logf("%-15s %-10s %10s ERROR: %v", agent.Name, fmt.Sprint(agent.Tools), elapsed, err)
			continue
		}
		preview := strings.TrimSpace(result.Stdout)
		if len(preview) > 50 {
			preview = preview[:50] + "..."
		}
		t.Logf("%-15s %-10s %10s %5d %s", agent.Name, fmt.Sprint(agent.Tools), elapsed, result.ExitCode, preview)
	}
}

func TestBenchmarkLiveFullPipeline(t *testing.T) {
	skipWithoutAPI(t)

	cfg := config.Default()

	scenarios := []struct {
		name        string
		query       string
		concurrency int
		maxAgents   int
		debate      int
	}{
		{"minimal_no_debate", "What is 2+2?", 3, 5, 0},
		{"minimal_with_debate", "What is 2+2?", 3, 5, 1},
		{"moderate_no_debate", "Compare Go and Rust for CLI development", 3, 6, 0},
	}

	t.Logf("\n=== Full Pipeline Benchmark ===")
	t.Logf("%-25s %5s %5s %6s %10s %5s %s",
		"Scenario", "Conc", "MaxA", "Debate", "Total", "Succ", "Synthesis Preview")
	t.Logf("%s", strings.Repeat("-", 120))

	for _, s := range scenarios {
		cfg.Heavy.MaxAgents = s.maxAgents

		start := time.Now()
		result, err := Run(context.Background(), cfg, RunOpts{
			Query:        s.query,
			ModelAll:     "grok-code-fast-1",
			Concurrency:  s.concurrency,
			DebateRounds: s.debate,
			Verbose:      false,
			JSON:         true,
			WorkingDir:   "/tmp",
			TimeoutSecs:  120,
		})
		elapsed := time.Since(start)

		if err != nil {
			t.Logf("%-25s %5d %5d %6d %10s ERROR: %v",
				s.name, s.concurrency, s.maxAgents, s.debate, elapsed, err)
			continue
		}

		successful := countSuccessful(result.Outputs)
		preview := strings.TrimSpace(result.Synthesis)
		if len(preview) > 40 {
			preview = preview[:40] + "..."
		}
		t.Logf("%-25s %5d %5d %6d %10s %5d %s",
			s.name, s.concurrency, s.maxAgents, s.debate, elapsed, successful, preview)

		// Per-agent timing breakdown
		for _, o := range result.Outputs {
			status := "OK"
			if o.Failed {
				status = "FAIL"
			}
			t.Logf("  %-15s %s (%.1fs)", o.Name, status, o.Duration)
		}
	}
}

func truncQuery(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n-3] + "..."
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
