package generate

import (
	"strings"
	"testing"

	"github.com/reuben/scud/internal/model"
)

func TestCheckDepsValid(t *testing.T) {
	phases := map[string]*model.Phase{
		"v1": {
			Name: "v1",
			Tasks: []*model.Task{
				{ID: "1", Title: "First", Status: model.Pending},
				{ID: "2", Title: "Second", Status: model.Pending, Dependencies: []string{"1"}},
				{ID: "3", Title: "Third", Status: model.Pending, Dependencies: []string{"2"}},
			},
		},
	}

	result := CheckDeps(phases, "v1")
	if !result.OK {
		t.Errorf("expected OK, got issues: selfRefs=%v, missing=%v, cycles=%v",
			result.SelfRefs, result.MissingDeps, result.Cycles)
	}
}

func TestCheckDepsSelfReference(t *testing.T) {
	phases := map[string]*model.Phase{
		"v1": {
			Name: "v1",
			Tasks: []*model.Task{
				{ID: "1", Title: "Self-ref", Status: model.Pending, Dependencies: []string{"1"}},
			},
		},
	}

	result := CheckDeps(phases, "v1")
	if result.OK {
		t.Fatal("expected not OK for self-reference")
	}
	if len(result.SelfRefs) != 1 || result.SelfRefs[0] != "1" {
		t.Errorf("SelfRefs = %v, want [1]", result.SelfRefs)
	}
}

func TestCheckDepsMissing(t *testing.T) {
	phases := map[string]*model.Phase{
		"v1": {
			Name: "v1",
			Tasks: []*model.Task{
				{ID: "1", Title: "Task", Status: model.Pending, Dependencies: []string{"nonexistent"}},
			},
		},
	}

	result := CheckDeps(phases, "v1")
	if result.OK {
		t.Fatal("expected not OK for missing dep")
	}
	if len(result.MissingDeps) != 1 {
		t.Fatalf("MissingDeps = %d, want 1", len(result.MissingDeps))
	}
	if result.MissingDeps[0].DepID != "nonexistent" {
		t.Errorf("missing dep = %q, want nonexistent", result.MissingDeps[0].DepID)
	}
}

func TestCheckDepsCycle(t *testing.T) {
	phases := map[string]*model.Phase{
		"v1": {
			Name: "v1",
			Tasks: []*model.Task{
				{ID: "a", Title: "A", Status: model.Pending, Dependencies: []string{"b"}},
				{ID: "b", Title: "B", Status: model.Pending, Dependencies: []string{"c"}},
				{ID: "c", Title: "C", Status: model.Pending, Dependencies: []string{"a"}},
			},
		},
	}

	result := CheckDeps(phases, "v1")
	if result.OK {
		t.Fatal("expected not OK for cycle")
	}
	if len(result.Cycles) == 0 {
		t.Fatal("expected at least one cycle")
	}
}

func TestCheckDepsNonexistentTag(t *testing.T) {
	phases := map[string]*model.Phase{}

	result := CheckDeps(phases, "missing")
	if !result.OK {
		t.Error("expected OK for nonexistent tag (no tasks to check)")
	}
}

func TestFormatCheckResultOK(t *testing.T) {
	result := &CheckDepsResult{OK: true}
	out := FormatCheckResult(result)
	if out != "All dependency checks passed." {
		t.Errorf("unexpected output: %q", out)
	}
}

func TestFormatCheckResultIssues(t *testing.T) {
	result := &CheckDepsResult{
		OK:       false,
		SelfRefs: []string{"1"},
		MissingDeps: []MissingDep{
			{TaskID: "2", DepID: "99"},
		},
		Cycles: [][]string{{"a", "b", "c"}},
	}

	out := FormatCheckResult(result)
	if !strings.Contains(out, "Self-references: 1") {
		t.Error("missing self-references in output")
	}
	if !strings.Contains(out, "Missing dep: task 2 depends on 99") {
		t.Error("missing dep info in output")
	}
	if !strings.Contains(out, "Cycle: a -> b -> c") {
		t.Error("missing cycle info in output")
	}
}
