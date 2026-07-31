package graph

import (
	"reflect"
	"testing"

	"github.com/reuben/scud/v2/core"
)

func TestCoreGraphRoundTrip(t *testing.T) {
	in := []core.Obligation{
		{ID: "b", GoalID: "g", Description: "build", CapabilityRef: "builder", PolicyRef: "safe", GrantRef: "grant-1", DependsOn: []core.ID{"a"}, Status: core.ObligationReady},
		{ID: "a", GoalID: "g", Status: core.ObligationSucceeded},
	}
	g, err := FromCore(in)
	if err != nil {
		t.Fatal(err)
	}
	out, err := ToCore(g, "g")
	if err != nil {
		t.Fatal(err)
	}
	if len(out) != len(in) {
		t.Fatalf("round trip count = %d", len(out))
	}
	for _, want := range in {
		var got core.Obligation
		for _, candidate := range out {
			if candidate.ID == want.ID {
				got = candidate
			}
		}
		if got.ID == "" || got.GoalID != want.GoalID || got.Description != want.Description || got.CapabilityRef != want.CapabilityRef || got.PolicyRef != want.PolicyRef || got.GrantRef != want.GrantRef || got.Status != want.Status || !reflect.DeepEqual(got.DependsOn, want.DependsOn) {
			t.Fatalf("round trip %q = %+v, want %+v", want.ID, got, want)
		}
	}
}

func TestCoreBridgeRejectsReconciliationOnlyStatuses(t *testing.T) {
	g := New(Node{ID: "a", Status: Deferred})
	if _, err := ToCore(g, "g"); err == nil {
		t.Fatal("expected deferred status conversion error")
	}
	if _, err := FromCore([]core.Obligation{{ID: "a", GoalID: "g", Status: core.ObligationStatus("waiting")}}); err == nil {
		t.Fatal("expected unknown core status error")
	}
}

func TestEvidenceRoundTrip(t *testing.T) {
	in := []core.Evidence{{ID: "e", Kind: "test", Value: "ok", URI: "file://x", Hash: "abc"}}
	got := ToCoreEvidence(FromCoreEvidence(in))
	if !reflect.DeepEqual(got, in) {
		t.Fatalf("evidence round trip = %+v, want %+v", got, in)
	}
}

func TestCoreStateBridgeCarriesBudget(t *testing.T) {
	coreState := core.NewState()
	coreState.Goals["g"] = core.Goal{ID: "g", Status: core.GoalRunning}
	coreState.Obligations["a"] = core.Obligation{ID: "a", GoalID: "g", Status: core.ObligationRunning}
	coreState.Budget.UsedCost, coreState.Budget.UsedSteps = 7, 2
	s, err := FromCoreState(coreState, "g")
	if err != nil {
		t.Fatal(err)
	}
	if s.Budget.UsedCost != 7 || s.Budget.UsedSteps != 2 {
		t.Fatalf("budget = %+v", s.Budget)
	}
	round, err := ToCoreState(s, core.Goal{ID: "g", Status: core.GoalRunning})
	if err != nil {
		t.Fatal(err)
	}
	if round.Obligations["a"].Status != core.ObligationRunning || round.Budget.UsedCost != 7 {
		t.Fatalf("core state = %+v", round)
	}
}
