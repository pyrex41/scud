package core

import "testing"

func eventGoal(id ID) Event {
	return Event{ID: ID("event-" + string(id)), Kind: EventGoalCreated, Goal: Goal{ID: id}}
}

func TestReduceDoesNotMutateInputAndReplayMatches(t *testing.T) {
	s := NewState()
	var err error
	s, err = s.Append(eventGoal("g"))
	if err != nil {
		t.Fatal(err)
	}
	dep := []ID{"a"}
	s2, err := s.Append(Event{Kind: EventObligationAdded, Obligation: Obligation{ID: "a", GoalID: "g"}})
	if err != nil {
		t.Fatal(err)
	}
	s2, err = s2.Append(Event{Kind: EventObligationAdded, Obligation: Obligation{ID: "b", GoalID: "g", DependsOn: dep}})
	if err != nil {
		t.Fatal(err)
	}
	dep[0] = "changed"
	if got := s2.Obligations["b"].DependsOn[0]; got != "a" {
		t.Fatalf("reducer did not copy dependencies: %q", got)
	}
	s2, err = s2.Append(Event{Kind: EventDecisionIssued, Decision: Decision{ID: "d", GoalID: "g", ObligationID: "a", Kind: DecisionExecute}})
	if err != nil {
		t.Fatal(err)
	}
	s2, err = s2.Append(Event{Kind: EventObservationAdded, Observation: Observation{ID: "o", GoalID: "g", ObligationID: "a", Outcome: OutcomeSuccess}})
	if err != nil {
		t.Fatal(err)
	}
	s2, err = s2.Append(Event{Kind: EventDecisionIssued, Decision: Decision{ID: "d2", GoalID: "g", ObligationID: "b", Kind: DecisionExecute}})
	if err != nil {
		t.Fatal(err)
	}
	s2, err = s2.Append(Event{Kind: EventObservationAdded, Observation: Observation{ID: "o2", GoalID: "g", ObligationID: "b", Outcome: OutcomeSuccess}})
	if err != nil {
		t.Fatal(err)
	}
	replayed, err := Replay(s2.Events)
	if err != nil {
		t.Fatal(err)
	}
	if replayed.Revision != s2.Revision || replayed.Goals["g"].Status != GoalSucceeded {
		t.Fatalf("replay differs: %#v %#v", replayed, s2)
	}
	if s.Goals["g"].Status != GoalPending {
		t.Fatalf("append mutated prior state")
	}
}

func TestEventDataIsCopied(t *testing.T) {
	s := NewState()
	s, err := s.Append(eventGoal("g"))
	if err != nil {
		t.Fatal(err)
	}
	data := []byte("opaque")
	s, err = s.Append(Event{Kind: EventBudgetConsumed, Data: data, Budget: BudgetDelta{Steps: 1}})
	if err != nil {
		t.Fatal(err)
	}
	data[0] = 'X'
	if string(s.Events[1].Data) != "opaque" {
		t.Fatalf("event data was aliased: %q", s.Events[1].Data)
	}
}

func TestValidationRejectsCrossGoalAndCycles(t *testing.T) {
	s := NewState()
	var err error
	s, err = s.Append(eventGoal("g1"))
	if err != nil {
		t.Fatal(err)
	}
	s, err = s.Append(Event{Kind: EventGoalCreated, Goal: Goal{ID: "g2"}})
	if err != nil {
		t.Fatal(err)
	}
	s, err = s.Append(Event{Kind: EventObligationAdded, Obligation: Obligation{ID: "a", GoalID: "g1", DependsOn: []ID{"b"}}})
	if err == nil {
		t.Fatal("expected cycle/reference error")
	}
	// Direct mutation is rejected by final validation as well.
	s = NewState()
	s, _ = s.Append(eventGoal("g"))
	s, _ = s.Append(Event{Kind: EventObligationAdded, Obligation: Obligation{ID: "a", GoalID: "g"}})
	s.Obligations["a"] = Obligation{ID: "a", GoalID: "g", DependsOn: []ID{"a"}, Status: ObligationPending}
	if err := s.Validate(); err == nil {
		t.Fatal("expected self-cycle error")
	}
}

func TestReconcileIsDeterministicAndBudgetAware(t *testing.T) {
	s := NewState()
	var err error
	s, err = s.Append(eventGoal("g"))
	if err != nil {
		t.Fatal(err)
	}
	s, err = s.Append(Event{Kind: EventObligationAdded, Obligation: Obligation{ID: "z", GoalID: "g"}})
	if err != nil {
		t.Fatal(err)
	}
	s, err = s.Append(Event{Kind: EventObligationAdded, Obligation: Obligation{ID: "a", GoalID: "g"}})
	if err != nil {
		t.Fatal(err)
	}
	d, err := Reconcile(s)
	if err != nil {
		t.Fatal(err)
	}
	if d.Kind != DecisionExecute || d.ObligationID != "a" {
		t.Fatalf("got %#v", d)
	}
	s.Budget = Budget{MaxSteps: 1, UsedSteps: 1}
	d, err = Reconcile(s)
	if err != nil {
		t.Fatal(err)
	}
	if d.Kind != DecisionExhaust {
		t.Fatalf("got %#v", d)
	}
}

func TestReconcileBlocksWhenDependencyCannotComplete(t *testing.T) {
	s := NewState()
	var err error
	s, err = s.Append(eventGoal("g"))
	if err != nil {
		t.Fatal(err)
	}
	s, err = s.Append(Event{Kind: EventObligationAdded, Obligation: Obligation{ID: "a", GoalID: "g"}})
	if err != nil {
		t.Fatal(err)
	}
	s, err = s.Append(Event{Kind: EventObligationAdded, Obligation: Obligation{ID: "b", GoalID: "g", DependsOn: []ID{"a"}}})
	if err != nil {
		t.Fatal(err)
	}
	s.Obligations["a"] = Obligation{ID: "a", GoalID: "g", Status: ObligationFailed}
	d, err := Reconcile(s)
	if err != nil {
		t.Fatal(err)
	}
	if d.Kind != DecisionFail || d.ObligationID != "a" {
		t.Fatalf("expected failed prerequisite decision, got %#v", d)
	}
	s.Obligations["a"] = Obligation{ID: "a", GoalID: "g", Status: ObligationSucceeded}
	s.Obligations["b"] = Obligation{ID: "b", GoalID: "g", DependsOn: []ID{"a"}, Status: ObligationPending}
	d, err = Reconcile(s)
	if err != nil {
		t.Fatal(err)
	}
	if d.Kind != DecisionExecute || d.ObligationID != "b" {
		t.Fatalf("expected b execution, got %#v", d)
	}
}

func TestBudgetAndSequenceValidation(t *testing.T) {
	s := NewState()
	var err error
	s, err = s.Append(eventGoal("g"))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = Reduce(s, Event{Sequence: 4, Kind: EventGoalCreated, Goal: Goal{ID: "x"}}); err == nil {
		t.Fatal("expected sequence error")
	}
	s.Budget = Budget{MaxSteps: 1}
	if _, err = s.Append(Event{Kind: EventBudgetConsumed, Budget: BudgetDelta{Steps: 2}}); err == nil {
		t.Fatal("expected budget error")
	}
}

func TestTerminalStatuses(t *testing.T) {
	for _, status := range []GoalStatus{GoalSucceeded, GoalFailed, GoalBlocked, GoalCancelled, GoalExhausted} {
		if !status.terminal() {
			t.Errorf("%q not terminal", status)
		}
	}
	for _, status := range []ObligationStatus{ObligationSucceeded, ObligationFailed, ObligationBlocked, ObligationCancelled} {
		if !status.terminal() {
			t.Errorf("%q not terminal", status)
		}
	}
}
