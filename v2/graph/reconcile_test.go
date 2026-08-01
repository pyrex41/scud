package graph

import "testing"

func mustGraph(t *testing.T, nodes ...Node) Graph {
	t.Helper()
	g, err := Build(nodes)
	if err != nil {
		t.Fatal(err)
	}
	return g
}

func TestReconcileBudgetDefersAndPreservesInput(t *testing.T) {
	g := mustGraph(t, Node{ID: "a", Cost: 3}, Node{ID: "b", Cost: 3})
	s := NewSnapshot(g)
	res, err := Reconcile(s, Request{Action: Admit, ID: "a"}, Policy{MaxActive: 1, MaxCost: 3})
	if err != nil || res.Decision.Action != Admit {
		t.Fatalf("admit: decision=%+v err=%v", res.Decision, err)
	}
	if res.State.Budget.UsedCost != 3 || res.State.Budget.UsedSteps != 1 {
		t.Fatalf("budget = %+v", res.State.Budget)
	}
	// The original snapshot remains pending, demonstrating pure semantics.
	if n, _ := s.Graph.Node("a"); n.Status != Pending {
		t.Fatalf("input mutated: %q", n.Status)
	}
	res, err = Reconcile(res.State, Request{Action: Admit, ID: "b"}, Policy{MaxActive: 1, MaxCost: 3})
	if err != nil || res.Decision.Action != Defer {
		t.Fatalf("expected defer, decision=%+v err=%v", res.Decision, err)
	}
	if n, _ := res.State.Graph.Node("b"); n.Status != Deferred {
		t.Fatalf("b status = %q", n.Status)
	}
}

func TestReconcileProgressAndEvidenceGuards(t *testing.T) {
	g := mustGraph(t, Node{ID: "a", EvidenceRequired: true, MinProgress: .8})
	s := NewSnapshot(g)
	r, err := Reconcile(s, Request{Action: Admit, ID: "a"}, Policy{})
	if err != nil {
		t.Fatal(err)
	}
	r, err = Reconcile(r.State, Request{Action: Complete, ID: "a", Progress: .7}, Policy{RequireProgressForComplete: true})
	if err != nil || r.Decision.Action != Ask {
		t.Fatalf("progress guard: %+v err=%v", r.Decision, err)
	}
	r, err = Reconcile(r.State, Request{Action: Complete, ID: "a", Progress: .9}, Policy{RequireProgressForComplete: true})
	if err != nil || r.Decision.Action != Ask {
		t.Fatalf("evidence guard: %+v err=%v", r.Decision, err)
	}
	r, err = Reconcile(r.State, Request{Action: Complete, ID: "a", Progress: .9, Evidence: []Evidence{{Kind: "test", Value: "passed"}}}, Policy{RequireProgressForComplete: true})
	if err != nil || r.Decision.Action != Complete {
		t.Fatalf("complete: %+v err=%v", r.Decision, err)
	}
	if n, _ := r.State.Graph.Node("a"); n.Status != Succeeded || n.Progress != .9 || len(n.Evidence) != 1 {
		t.Fatalf("completed node = %+v", n)
	}
}

func TestReconcileActionsAndGuardedFeedback(t *testing.T) {
	g := mustGraph(t, Node{ID: "a"})
	s := NewSnapshot(g)
	r, err := Reconcile(s, Request{Action: Ask, ID: "a", Question: "which target?"}, Policy{})
	if err != nil || r.Decision.Action != Ask || len(r.State.Questions) != 1 {
		t.Fatalf("ask: %+v %+v", r.Decision, r.State.Questions)
	}
	r, err = Reconcile(r.State, Request{Action: Admit, ID: "a"}, Policy{})
	if err != nil || r.Decision.Action != Admit {
		t.Fatalf("admit after ask: %+v err=%v", r.Decision, err)
	}
	// First replan changes the graph; replaying it is blocked instead of
	// recursively feeding the same plan back into itself.
	rewrite := &Rewrite{AddNodes: []Node{{ID: "b"}}}
	r, err = Reconcile(r.State, Request{Action: Replan, Rewrite: rewrite}, Policy{MaxReplans: 2})
	if err != nil || r.Decision.Action != Replan {
		t.Fatalf("replan: %+v err=%v", r.Decision, err)
	}
	r, err = Reconcile(r.State, Request{Action: Replan, Rewrite: &Rewrite{RemoveIDs: []ID{"b"}}}, Policy{MaxReplans: 2})
	if err != nil || r.Decision.Action != Block {
		t.Fatalf("feedback guard: %+v err=%v", r.Decision, err)
	}
}
