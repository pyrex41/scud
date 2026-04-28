package heavy

import (
	"strings"
	"testing"
)

func TestRegistryCount(t *testing.T) {
	if got := len(Registry()); got != 16 {
		t.Errorf("Registry() = %d agents, want 16", got)
	}
}

func TestCoreAgentsCount(t *testing.T) {
	cores := CoreAgents()
	if len(cores) != 4 {
		t.Fatalf("CoreAgents() = %d, want 4", len(cores))
	}
	names := map[string]bool{}
	for _, a := range cores {
		names[a.Name] = true
	}
	for _, want := range []string{"Captain", "Harper", "Benjamin", "Lucas"} {
		if !names[want] {
			t.Errorf("CoreAgents() missing %s", want)
		}
	}
}

func TestSpecialistsCount(t *testing.T) {
	if got := len(Specialists()); got != 12 {
		t.Errorf("Specialists() = %d, want 12", got)
	}
	for _, s := range Specialists() {
		if s.IsCore {
			t.Errorf("Specialists() contains core agent %s", s.Name)
		}
	}
}

func TestFindByName(t *testing.T) {
	tests := []struct {
		name  string
		found bool
	}{
		{"Captain", true},
		{"captain", true},
		{"CAPTAIN", true},
		{"Harper", true},
		{"Euler", true},
		{"euler", true},
		{"nonexistent", false},
		{"", false},
	}
	for _, tt := range tests {
		a, ok := FindByName(tt.name)
		if ok != tt.found {
			t.Errorf("FindByName(%q) found=%v, want %v", tt.name, ok, tt.found)
		}
		if ok && !strings.EqualFold(a.Name, tt.name) {
			t.Errorf("FindByName(%q) returned %q", tt.name, a.Name)
		}
	}
}

func TestRoutingPromptContainsSpecialists(t *testing.T) {
	prompt := RoutingPrompt("test query", Specialists())
	if !strings.Contains(prompt, "test query") {
		t.Error("routing prompt missing query")
	}
	if !strings.Contains(prompt, "JSON array") {
		t.Error("routing prompt missing JSON instruction")
	}
	for _, s := range Specialists() {
		if !strings.Contains(prompt, s.Name) {
			t.Errorf("routing prompt missing specialist %s", s.Name)
		}
	}
}

func TestSynthesisPromptTruncatesLongOutputs(t *testing.T) {
	longOutput := strings.Repeat("x", 5000)
	outputs := []AgentOutput{
		{Name: "Harper", Domain: "Research", Output: longOutput},
		{Name: "Failed", Domain: "Test", Output: "ignored", Failed: true},
	}
	prompt := SynthesisPrompt("query", outputs)
	if strings.Contains(prompt, "Failed") {
		t.Error("synthesis prompt should skip failed agents")
	}
	if !strings.Contains(prompt, "truncated") {
		t.Error("synthesis prompt should truncate long outputs")
	}
	if strings.Contains(prompt, longOutput) {
		t.Error("synthesis prompt should not contain full 5000-char output")
	}
}

func TestMergeAgentsDedup(t *testing.T) {
	// Should include core 4 + selected specialists, deduped
	result := mergeAgents([]string{"Captain", "Harper", "Euler", "Curie", "euler"}, 0)
	names := map[string]bool{}
	for _, a := range result {
		if names[a.Name] {
			t.Errorf("duplicate agent: %s", a.Name)
		}
		names[a.Name] = true
	}
	if !names["Captain"] || !names["Harper"] || !names["Benjamin"] || !names["Lucas"] {
		t.Error("missing core agents")
	}
	if !names["Euler"] || !names["Curie"] {
		t.Error("missing selected specialists")
	}
	if len(result) != 6 {
		t.Errorf("got %d agents, want 6", len(result))
	}
}

func TestMergeAgentsMaxCap(t *testing.T) {
	all := make([]string, 0)
	for _, s := range Specialists() {
		all = append(all, s.Name)
	}
	result := mergeAgents(all, 6)
	if len(result) != 6 {
		t.Errorf("got %d agents, want 6 (max_agents cap)", len(result))
	}
}

func TestCritiquePrompt(t *testing.T) {
	p := CritiquePrompt("my query", "my synthesis")
	if !strings.Contains(p, "my query") || !strings.Contains(p, "my synthesis") {
		t.Error("critique prompt missing query or synthesis")
	}
}

func TestResynthesisPrompt(t *testing.T) {
	critiques := []AgentOutput{
		{Name: "Harper", Output: "needs more sources"},
		{Name: "Failed", Output: "ignored", Failed: true},
	}
	p := ResynthesisPrompt("query", "synthesis", critiques)
	if !strings.Contains(p, "Harper critique") {
		t.Error("resynthesis prompt missing critique")
	}
	if strings.Contains(p, "Failed critique") {
		t.Error("resynthesis prompt should skip failed critiques")
	}
}
