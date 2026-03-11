package model

import (
	"testing"
)

func TestStatusCodeRoundTrip(t *testing.T) {
	statuses := []TaskStatus{Pending, InProgress, Done, Failed, Blocked, Review, Expanded, Deferred, Cancelled}
	for _, s := range statuses {
		code := StatusCode(s)
		got := StatusFromCode(code)
		if got != s {
			t.Errorf("StatusFromCode(StatusCode(%s)) = %s, want %s", s, got, s)
		}
	}
}

func TestPriorityCodeRoundTrip(t *testing.T) {
	priorities := []Priority{Critical, High, Medium, Low}
	for _, p := range priorities {
		code := PriorityCode(p)
		got := PriorityFromCode(code)
		if got != p {
			t.Errorf("PriorityFromCode(PriorityCode(%s)) = %s, want %s", p, got, p)
		}
	}
}

func TestIsReady(t *testing.T) {
	done := map[string]bool{"1": true, "2": true}

	tests := []struct {
		name   string
		task   Task
		want   bool
	}{
		{"no deps, pending", Task{Status: Pending}, true},
		{"deps met", Task{Status: Pending, Dependencies: []string{"1", "2"}}, true},
		{"deps not met", Task{Status: Pending, Dependencies: []string{"1", "3"}}, false},
		{"not pending", Task{Status: InProgress}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.task.IsReady(done); got != tt.want {
				t.Errorf("IsReady() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestGetEffectiveDeps(t *testing.T) {
	tasks := map[string]*Task{
		"1":   {ID: "1", Dependencies: []string{"0"}},
		"1.1": {ID: "1.1", ParentID: "1", Dependencies: []string{"x"}},
	}

	sub := tasks["1.1"]
	deps := sub.GetEffectiveDeps(tasks)
	// Should include own dep "x" and parent dep "0"
	want := map[string]bool{"x": true, "0": true}
	for _, d := range deps {
		if !want[d] {
			t.Errorf("unexpected dep: %s", d)
		}
		delete(want, d)
	}
	if len(want) > 0 {
		t.Errorf("missing deps: %v", want)
	}
}

func TestWouldCreateCycle(t *testing.T) {
	tasks := map[string]*Task{
		"1": {ID: "1", Dependencies: []string{}},
		"2": {ID: "2", Dependencies: []string{"1"}},
		"3": {ID: "3", Dependencies: []string{"2"}},
	}

	// Adding 1 -> 3 would create cycle: 1 -> 3 -> 2 -> 1
	if !tasks["1"].WouldCreateCycle("3", tasks) {
		t.Error("expected cycle detection")
	}

	// Adding 3 -> 1 is already there (transitive), no NEW cycle from 3's perspective
	// Actually: 3 depends on 2 depends on 1. If we add dep "1" to task 3, check if 1 can reach 3.
	// 1 -> (nothing) so no cycle.
	if tasks["3"].WouldCreateCycle("1", tasks) {
		t.Error("should not detect cycle for valid dep")
	}
}

func TestNeedsExpansion(t *testing.T) {
	tests := []struct {
		name string
		task Task
		want bool
	}{
		{"complexity 3", Task{Complexity: 3, Status: Pending}, false},
		{"complexity 5", Task{Complexity: 5, Status: Pending}, true},
		{"already expanded", Task{Complexity: 8, Status: Expanded}, false},
		{"has subtasks", Task{Complexity: 8, Status: Pending, Subtasks: []string{"1.1"}}, false},
		{"is subtask", Task{Complexity: 8, Status: Pending, ParentID: "1"}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.task.NeedsExpansion(); got != tt.want {
				t.Errorf("NeedsExpansion() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestPhaseActionableTasks(t *testing.T) {
	p := &Phase{
		Name: "test",
		Tasks: []*Task{
			{ID: "1", Status: Pending},
			{ID: "2", Status: Expanded, Subtasks: []string{"2.1", "2.2"}},
			{ID: "2.1", Status: Pending, ParentID: "2"},
			{ID: "2.2", Status: Done, ParentID: "2"},
			{ID: "3", Status: Done},
			{ID: "4", Status: Pending, ParentID: "5"}, // parent not expanded
		},
	}

	actionable := p.ActionableTasks()
	ids := make(map[string]bool)
	for _, t := range actionable {
		ids[t.ID] = true
	}

	if !ids["1"] {
		t.Error("expected task 1 to be actionable")
	}
	if ids["2"] {
		t.Error("expected expanded parent 2 to NOT be actionable")
	}
	if !ids["2.1"] {
		t.Error("expected subtask 2.1 to be actionable")
	}
	if ids["3"] {
		t.Error("expected done task 3 to NOT be actionable")
	}
	if ids["4"] {
		t.Error("expected task 4 with non-expanded parent to NOT be actionable")
	}
}

func TestPhaseStats(t *testing.T) {
	p := &Phase{
		Name: "test",
		Tasks: []*Task{
			{ID: "1", Status: Done, Complexity: 3},
			{ID: "2", Status: Expanded, Complexity: 8, Subtasks: []string{"2.1", "2.2"}},
			{ID: "2.1", Status: Done, Complexity: 0, ParentID: "2"},
			{ID: "2.2", Status: Done, Complexity: 0, ParentID: "2"},
			{ID: "3", Status: Pending, Complexity: 5},
		},
	}

	s := p.Stats()
	// Subtasks excluded from total. Expanded parent with all subs done = done.
	if s.Total != 3 { // 1, 2, 3
		t.Errorf("Total = %d, want 3", s.Total)
	}
	if s.Done != 2 { // 1, 2 (all subtasks done)
		t.Errorf("Done = %d, want 2", s.Done)
	}
	if s.Pending != 1 { // 3
		t.Errorf("Pending = %d, want 1", s.Pending)
	}
}
