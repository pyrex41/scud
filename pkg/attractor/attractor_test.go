package attractor

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseDOTSimple(t *testing.T) {
	dot := `digraph pipeline {
		start -> build
		build -> finish
	}`

	g, err := ParseDOT(dot)
	if err != nil {
		t.Fatal(err)
	}

	if len(g.Nodes) != 3 {
		t.Errorf("node count = %d, want 3", len(g.Nodes))
	}
	if len(g.Edges) != 2 {
		t.Errorf("edge count = %d, want 2", len(g.Edges))
	}

	// Check auto-created nodes have correct IDs
	ids := make(map[string]bool)
	for _, n := range g.Nodes {
		ids[n.ID] = true
	}
	for _, want := range []string{"start", "build", "finish"} {
		if !ids[want] {
			t.Errorf("missing node %q", want)
		}
	}
}

func TestParseDOTWithProperties(t *testing.T) {
	dot := `digraph test {
		begin [label="Begin" type="start"]
		work [label="Do Work" type="codergen" model="grok"]
		end [label="Done" type="exit"]
		begin -> work
		work -> end
	}`

	g, err := ParseDOT(dot)
	if err != nil {
		t.Fatal(err)
	}

	begin := g.FindNode("begin")
	if begin == nil {
		t.Fatal("begin node not found")
	}
	if begin.Label != "Begin" {
		t.Errorf("begin.Label = %q, want Begin", begin.Label)
	}
	if begin.Type != "start" {
		t.Errorf("begin.Type = %q, want start", begin.Type)
	}

	work := g.FindNode("work")
	if work == nil {
		t.Fatal("work node not found")
	}
	if work.Type != "codergen" {
		t.Errorf("work.Type = %q, want codergen", work.Type)
	}
	if work.Properties["model"] != "grok" {
		t.Errorf("work.Properties[model] = %q, want grok", work.Properties["model"])
	}
}

func TestParseDOTNoDigraph(t *testing.T) {
	_, err := ParseDOT("not a graph")
	if err == nil {
		t.Fatal("expected error for non-digraph input")
	}
}

func TestValidateValid(t *testing.T) {
	g := &PipelineGraph{
		Nodes: []PipelineNode{
			{ID: "s", Type: "start"},
			{ID: "a", Type: "codergen"},
			{ID: "e", Type: "exit"},
		},
		Edges: []PipelineEdge{
			{From: "s", To: "a"},
			{From: "a", To: "e"},
		},
	}

	issues := Validate(g)
	if len(issues) != 0 {
		t.Errorf("expected no issues, got: %v", issues)
	}
}

func TestValidateNoStart(t *testing.T) {
	g := &PipelineGraph{
		Nodes: []PipelineNode{
			{ID: "a", Type: "codergen"},
			{ID: "e", Type: "exit"},
		},
		Edges: []PipelineEdge{
			{From: "a", To: "e"},
		},
	}

	issues := Validate(g)
	found := false
	for _, issue := range issues {
		if issue == "no start node found (need exactly one node with type=\"start\")" {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'no start node' issue, got: %v", issues)
	}
}

func TestValidateOrphan(t *testing.T) {
	g := &PipelineGraph{
		Nodes: []PipelineNode{
			{ID: "s", Type: "start"},
			{ID: "a", Type: "codergen"},
			{ID: "orphan", Type: "codergen"},
			{ID: "e", Type: "exit"},
		},
		Edges: []PipelineEdge{
			{From: "s", To: "a"},
			{From: "a", To: "e"},
		},
	}

	issues := Validate(g)
	hasOrphan := false
	for _, issue := range issues {
		if issue == `node "orphan" has no incoming edges (orphan)` {
			hasOrphan = true
		}
	}
	if !hasOrphan {
		t.Errorf("expected orphan issue, got: %v", issues)
	}
}

func TestPipelineContext(t *testing.T) {
	ctx := NewContext()

	ctx.Set("key1", "val1")
	ctx.Set("key2", "val2")

	if got := ctx.Get("key1"); got != "val1" {
		t.Errorf("Get(key1) = %q, want val1", got)
	}
	if got := ctx.Get("missing"); got != "" {
		t.Errorf("Get(missing) = %q, want empty", got)
	}

	all := ctx.All()
	if len(all) != 2 {
		t.Errorf("All() has %d entries, want 2", len(all))
	}

	// Load bulk
	ctx.Load(map[string]string{"key3": "val3", "key1": "overwritten"})
	if got := ctx.Get("key1"); got != "overwritten" {
		t.Errorf("after Load, key1 = %q, want overwritten", got)
	}
	if got := ctx.Get("key3"); got != "val3" {
		t.Errorf("after Load, key3 = %q, want val3", got)
	}
}

func TestEvalCondition(t *testing.T) {
	ctx := NewContext()
	ctx.Set("status", "success")

	tests := []struct {
		expr string
		want bool
	}{
		{"", true},
		{"status=success", true},
		{"status=failure", false},
		{"missing=value", false},
		{"invalid-no-equals", false},
	}

	for _, tt := range tests {
		got := EvalCondition(tt.expr, ctx)
		if got != tt.want {
			t.Errorf("EvalCondition(%q) = %v, want %v", tt.expr, got, tt.want)
		}
	}
}

func TestCheckpointRoundtrip(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "checkpoint.json")

	cp := &Checkpoint{
		GraphFile:   "test.dot",
		CurrentNode: "step2",
		Completed:   []string{"step1"},
		Outcomes: []Outcome{
			{NodeID: "step1", Status: "success", Output: "done"},
		},
		Context: map[string]string{"key": "val"},
	}

	if err := SaveCheckpoint(cp, path); err != nil {
		t.Fatal(err)
	}

	// Verify file exists
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("checkpoint file not created: %v", err)
	}

	loaded, err := LoadCheckpoint(path)
	if err != nil {
		t.Fatal(err)
	}

	if loaded.GraphFile != cp.GraphFile {
		t.Errorf("GraphFile = %q, want %q", loaded.GraphFile, cp.GraphFile)
	}
	if loaded.CurrentNode != cp.CurrentNode {
		t.Errorf("CurrentNode = %q, want %q", loaded.CurrentNode, cp.CurrentNode)
	}
	if len(loaded.Completed) != 1 || loaded.Completed[0] != "step1" {
		t.Errorf("Completed = %v, want [step1]", loaded.Completed)
	}
	if len(loaded.Outcomes) != 1 || loaded.Outcomes[0].NodeID != "step1" {
		t.Errorf("Outcomes = %v, want [{step1 success done}]", loaded.Outcomes)
	}
	if loaded.Context["key"] != "val" {
		t.Errorf("Context[key] = %q, want val", loaded.Context["key"])
	}
}

func TestGraphMethods(t *testing.T) {
	g := &PipelineGraph{
		Nodes: []PipelineNode{
			{ID: "s", Type: "start", Label: "Start"},
			{ID: "a", Type: "codergen", Label: "Build"},
			{ID: "b", Type: "codergen", Label: "Test"},
			{ID: "e1", Type: "exit", Label: "Exit1"},
			{ID: "e2", Type: "exit", Label: "Exit2"},
		},
		Edges: []PipelineEdge{
			{From: "s", To: "a"},
			{From: "s", To: "b"},
			{From: "a", To: "e1"},
			{From: "b", To: "e2"},
		},
	}

	// FindNode
	if n := g.FindNode("a"); n == nil || n.Label != "Build" {
		t.Error("FindNode(a) failed")
	}
	if n := g.FindNode("nonexistent"); n != nil {
		t.Error("FindNode(nonexistent) should return nil")
	}

	// Successors
	succ := g.Successors("s")
	if len(succ) != 2 {
		t.Errorf("Successors(s) = %d, want 2", len(succ))
	}

	// Predecessors
	pred := g.Predecessors("e1")
	if len(pred) != 1 || pred[0].From != "a" {
		t.Errorf("Predecessors(e1) = %v, want [{a->e1}]", pred)
	}

	// StartNode
	start := g.StartNode()
	if start == nil || start.ID != "s" {
		t.Error("StartNode() failed")
	}

	// ExitNodes
	exits := g.ExitNodes()
	if len(exits) != 2 {
		t.Errorf("ExitNodes() = %d, want 2", len(exits))
	}
}
