package scg

import (
	"strings"
	"testing"

	"github.com/reuben/scud/internal/model"
)

func TestEscapeUnescapeRoundTrip(t *testing.T) {
	tests := []string{
		"simple text",
		"has | pipe",
		`has \ backslash`,
		"has\nnewline",
		`mixed | and \ and
newline`,
	}
	for _, input := range tests {
		escaped := EscapeField(input)
		unescaped := UnescapeField(escaped)
		if unescaped != input {
			t.Errorf("roundtrip failed: %q -> %q -> %q", input, escaped, unescaped)
		}
	}
}

func TestSplitByPipe(t *testing.T) {
	tests := []struct {
		input string
		want  int
	}{
		{"a | b | c", 3},
		{`a \| b | c`, 2}, // escaped pipe
		{"a", 1},
		{"a | b | c | d | e", 5},
	}
	for _, tt := range tests {
		got := SplitByPipe(tt.input)
		if len(got) != tt.want {
			t.Errorf("SplitByPipe(%q) = %d parts, want %d: %v", tt.input, len(got), tt.want, got)
		}
	}
}

func TestParseSinglePhase(t *testing.T) {
	input := `# SCUD Graph v1
# Phase: test

@meta {
  name test
  id_format sequential
}

@nodes
# id | title | status | complexity | priority
1 | Setup project | P | 3 | H
2 | Implement core | P | 8 | M
3 | Write tests | P | 5 | M

@edges
# dependent -> dependency
2 -> 1
3 -> 2

@agents
# id | agent_type | model_tier
1 | fast-builder | fast
2 | builder | smart
3 | tester | standard

@details
1 | description |
  Create the initial project structure
  with all necessary files
2 | description |
  Build the core functionality
`

	p := ParsePhase(input)
	if p.Name != "test" {
		t.Errorf("Name = %q, want %q", p.Name, "test")
	}
	if len(p.Tasks) != 3 {
		t.Fatalf("Tasks = %d, want 3", len(p.Tasks))
	}

	// Check task 1
	t1 := p.Tasks[0]
	if t1.ID != "1" || t1.Title != "Setup project" {
		t.Errorf("Task 1: ID=%q Title=%q", t1.ID, t1.Title)
	}
	if t1.Status != model.Pending {
		t.Errorf("Task 1 status = %q, want pending", t1.Status)
	}
	if t1.Complexity != 3 {
		t.Errorf("Task 1 complexity = %d, want 3", t1.Complexity)
	}
	if t1.AgentType != model.AgentFastBuilder {
		t.Errorf("Task 1 agent = %q, want fast-builder", t1.AgentType)
	}
	if t1.Description != "Create the initial project structure\nwith all necessary files" {
		t.Errorf("Task 1 description = %q", t1.Description)
	}

	// Check dependencies
	t2 := p.Tasks[1]
	if len(t2.Dependencies) != 1 || t2.Dependencies[0] != "1" {
		t.Errorf("Task 2 deps = %v, want [1]", t2.Dependencies)
	}
	t3 := p.Tasks[2]
	if len(t3.Dependencies) != 1 || t3.Dependencies[0] != "2" {
		t.Errorf("Task 3 deps = %v, want [2]", t3.Dependencies)
	}
}

func TestParseMultiPhase(t *testing.T) {
	input := `# SCUD Graph v1
# Phase: auth

@meta {
  name auth
}

@nodes
1 | Login | P | 3 | H

---

# SCUD Graph v1
# Phase: api

@meta {
  name api
}

@nodes
1 | REST endpoint | P | 5 | M
`

	phases := ParseMultiPhase(input)
	if len(phases) != 2 {
		t.Fatalf("phases = %d, want 2", len(phases))
	}
	if _, ok := phases["auth"]; !ok {
		t.Error("missing auth phase")
	}
	if _, ok := phases["api"]; !ok {
		t.Error("missing api phase")
	}
	if phases["auth"].Tasks[0].Title != "Login" {
		t.Errorf("auth task title = %q", phases["auth"].Tasks[0].Title)
	}
}

func TestParseParents(t *testing.T) {
	input := `# SCUD Graph v1
# Phase: test

@meta {
  name test
}

@nodes
1 | Parent task | X | 8 | H
1.1 | Subtask one | P | 0 | H
1.2 | Subtask two | P | 0 | H

@parents
1: 1.1, 1.2
`

	p := ParsePhase(input)
	parent := p.FindTask("1")
	if parent == nil {
		t.Fatal("parent not found")
	}
	if len(parent.Subtasks) != 2 {
		t.Errorf("subtasks = %v, want 2 items", parent.Subtasks)
	}
	sub1 := p.FindTask("1.1")
	if sub1 == nil || sub1.ParentID != "1" {
		t.Errorf("subtask 1.1 parent = %q, want 1", sub1.ParentID)
	}
}

func TestSerializeRoundTrip(t *testing.T) {
	original := &model.Phase{
		Name:     "test",
		IDFormat: "sequential",
		Tasks: []*model.Task{
			{
				ID: "1", Title: "First task", Status: model.Pending,
				Complexity: 3, Priority: model.High,
				AgentType: model.AgentBuilder, ModelTier: model.TierStandard,
				Description: "Do the first thing",
			},
			{
				ID: "2", Title: "Second task", Status: model.Pending,
				Complexity: 5, Priority: model.Medium,
				Dependencies: []string{"1"},
				Description:  "Do the second thing",
			},
		},
	}

	serialized := SerializePhase(original)
	parsed := ParsePhase(serialized)

	if parsed.Name != original.Name {
		t.Errorf("Name = %q, want %q", parsed.Name, original.Name)
	}
	if len(parsed.Tasks) != len(original.Tasks) {
		t.Fatalf("Tasks = %d, want %d", len(parsed.Tasks), len(original.Tasks))
	}

	for i, orig := range original.Tasks {
		got := parsed.Tasks[i]
		if got.ID != orig.ID {
			t.Errorf("task %d ID = %q, want %q", i, got.ID, orig.ID)
		}
		if got.Title != orig.Title {
			t.Errorf("task %d Title = %q, want %q", i, got.Title, orig.Title)
		}
		if got.Status != orig.Status {
			t.Errorf("task %d Status = %q, want %q", i, got.Status, orig.Status)
		}
		if got.Description != orig.Description {
			t.Errorf("task %d Description = %q, want %q", i, got.Description, orig.Description)
		}
	}
}

func TestSerializeMultiPhaseRoundTrip(t *testing.T) {
	phases := map[string]*model.Phase{
		"alpha": {
			Name: "alpha",
			Tasks: []*model.Task{
				{ID: "1", Title: "Alpha task", Status: model.Done, Complexity: 2, Priority: model.High},
			},
		},
		"beta": {
			Name: "beta",
			Tasks: []*model.Task{
				{ID: "1", Title: "Beta task", Status: model.Pending, Complexity: 5, Priority: model.Medium},
			},
		},
	}

	serialized := SerializeMultiPhase(phases)
	parsed := ParseMultiPhase(serialized)

	if len(parsed) != 2 {
		t.Fatalf("parsed phases = %d, want 2", len(parsed))
	}
	for name, orig := range phases {
		got, ok := parsed[name]
		if !ok {
			t.Errorf("missing phase %q", name)
			continue
		}
		if len(got.Tasks) != len(orig.Tasks) {
			t.Errorf("phase %q tasks = %d, want %d", name, len(got.Tasks), len(orig.Tasks))
		}
	}
}

func TestEscapedPipeInTitle(t *testing.T) {
	p := &model.Phase{
		Name: "test",
		Tasks: []*model.Task{
			{ID: "1", Title: "Task with | pipe", Status: model.Pending, Complexity: 1, Priority: model.Medium},
		},
	}

	serialized := SerializePhase(p)
	if !strings.Contains(serialized, `Task with \| pipe`) {
		t.Errorf("pipe not escaped in serialized output: %s", serialized)
	}

	parsed := ParsePhase(serialized)
	if parsed.Tasks[0].Title != "Task with | pipe" {
		t.Errorf("title = %q, want %q", parsed.Tasks[0].Title, "Task with | pipe")
	}
}
