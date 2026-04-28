package wave

import (
	"testing"

	"github.com/reuben/scud/pkg/model"
)

func TestPlanLinearChain(t *testing.T) {
	tasks := []*model.Task{
		{ID: "1"},
		{ID: "2", Dependencies: []string{"1"}},
		{ID: "3", Dependencies: []string{"2"}},
	}

	wr := Plan(tasks)
	if len(wr.Waves) != 3 {
		t.Fatalf("waves = %d, want 3", len(wr.Waves))
	}
	if wr.Waves[0].Tasks[0] != "1" {
		t.Errorf("wave 1 = %v, want [1]", wr.Waves[0].Tasks)
	}
	if wr.Waves[1].Tasks[0] != "2" {
		t.Errorf("wave 2 = %v, want [2]", wr.Waves[1].Tasks)
	}
	if wr.Waves[2].Tasks[0] != "3" {
		t.Errorf("wave 3 = %v, want [3]", wr.Waves[2].Tasks)
	}
}

func TestPlanParallelTasks(t *testing.T) {
	tasks := []*model.Task{
		{ID: "1"},
		{ID: "2"},
		{ID: "3"},
	}

	wr := Plan(tasks)
	if len(wr.Waves) != 1 {
		t.Fatalf("waves = %d, want 1", len(wr.Waves))
	}
	if len(wr.Waves[0].Tasks) != 3 {
		t.Errorf("wave 1 tasks = %d, want 3", len(wr.Waves[0].Tasks))
	}
}

func TestPlanDiamondShape(t *testing.T) {
	// 1 -> 2, 1 -> 3, 2 -> 4, 3 -> 4
	tasks := []*model.Task{
		{ID: "1"},
		{ID: "2", Dependencies: []string{"1"}},
		{ID: "3", Dependencies: []string{"1"}},
		{ID: "4", Dependencies: []string{"2", "3"}},
	}

	wr := Plan(tasks)
	if len(wr.Waves) != 3 {
		t.Fatalf("waves = %d, want 3", len(wr.Waves))
	}
	if len(wr.Waves[1].Tasks) != 2 {
		t.Errorf("wave 2 tasks = %v, want 2 tasks", wr.Waves[1].Tasks)
	}
}

func TestPlanCircularDeps(t *testing.T) {
	tasks := []*model.Task{
		{ID: "1", Dependencies: []string{"3"}},
		{ID: "2", Dependencies: []string{"1"}},
		{ID: "3", Dependencies: []string{"2"}},
	}

	wr := Plan(tasks)
	if len(wr.Waves) != 0 {
		t.Errorf("waves = %d, want 0 for circular deps", len(wr.Waves))
	}
	if len(wr.CircularDeps) != 3 {
		t.Errorf("circular = %v, want 3 tasks", wr.CircularDeps)
	}
}

func TestPlanExternalDepsIgnored(t *testing.T) {
	// Task 2 depends on "external" which is not in the task set
	tasks := []*model.Task{
		{ID: "1"},
		{ID: "2", Dependencies: []string{"1", "external"}},
	}

	wr := Plan(tasks)
	if len(wr.Waves) != 2 {
		t.Fatalf("waves = %d, want 2", len(wr.Waves))
	}
}

func TestPlanEmpty(t *testing.T) {
	wr := Plan(nil)
	if len(wr.Waves) != 0 {
		t.Errorf("waves = %d, want 0", len(wr.Waves))
	}
}
