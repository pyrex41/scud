package runtime

import (
	"context"
	"errors"
	"testing"

	"github.com/reuben/scud/v2/adapters"
	"github.com/reuben/scud/v2/adapters/memory"
	"github.com/reuben/scud/v2/core"
)

type fakeRunner struct {
	calls   int
	outcome string
	err     error
}

func (f *fakeRunner) Run(_ context.Context, req adapters.RunRequest, sink adapters.EventSink) (adapters.RunResult, error) {
	f.calls++
	sink(adapters.Event{Type: "progress", Data: []byte(req.Prompt)})
	return adapters.RunResult{RunID: req.RunID, Outcome: f.outcome, Text: "done"}, f.err
}

type fakePolicy struct {
	allowed bool
	calls   int
}

func (p *fakePolicy) Authorize(_ context.Context, _ adapters.PolicyInput) (adapters.Decision, error) {
	p.calls++
	return adapters.Decision{Allowed: p.allowed, Reason: "test policy"}, nil
}

func configFor(r adapters.Runner, p adapters.Policy, store adapters.EventStore) Config {
	return Config{RunID: "run-1", Goal: core.Goal{ID: "goal-1", Title: "test"}, Obligations: []core.Obligation{{ID: "a", GoalID: "goal-1", Description: "first"}, {ID: "b", GoalID: "goal-1", Description: "second", DependsOn: []core.ID{"a"}}}, Runner: r, Policy: p, Store: store}
}

func TestRunPersistsAndReplaysCanonicalState(t *testing.T) {
	store := memory.NewEventStore()
	runner := &fakeRunner{outcome: "success"}
	policy := &fakePolicy{allowed: true}
	r, err := New(configFor(runner, policy, store))
	if err != nil {
		t.Fatal(err)
	}
	if err := r.Run(context.Background()); err != nil {
		t.Fatal(err)
	}
	if !r.Terminal() || runner.calls != 2 || policy.calls != 2 {
		t.Fatalf("terminal=%v calls=%d policy=%d", r.Terminal(), runner.calls, policy.calls)
	}
	state := r.State()
	if state.Goals["goal-1"].Status != core.GoalSucceeded || state.Budget.UsedSteps != 2 {
		t.Fatalf("unexpected state: %#v", state)
	}
	replay, err := New(configFor(&fakeRunner{outcome: "success"}, &fakePolicy{allowed: true}, store))
	if err != nil {
		t.Fatal(err)
	}
	if err := replay.Replay(context.Background()); err != nil {
		t.Fatal(err)
	}
	if replay.State().Revision != state.Revision || replay.State().Goals["goal-1"].Status != core.GoalSucceeded {
		t.Fatalf("replay differs: %#v", replay.State())
	}
	events, err := store.List(context.Background(), "run-1", 0)
	if err != nil {
		t.Fatal(err)
	}
	if len(events) < 8 {
		t.Fatalf("expected canonical and progress events, got %d", len(events))
	}
}

func TestAuthorizationDenialBlocksWithoutRunning(t *testing.T) {
	store := memory.NewEventStore()
	runner := &fakeRunner{outcome: "success"}
	policy := &fakePolicy{allowed: false}
	r, err := New(configFor(runner, policy, store))
	if err != nil {
		t.Fatal(err)
	}
	d, err := r.Step(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if d.Kind != core.DecisionBlock || runner.calls != 0 {
		t.Fatalf("got %#v calls=%d", d, runner.calls)
	}
	if !r.Terminal() || r.State().Goals["goal-1"].Status != core.GoalBlocked {
		t.Fatalf("expected blocked terminal state")
	}
}

func TestRunnerFailureBecomesObservation(t *testing.T) {
	store := memory.NewEventStore()
	runner := &fakeRunner{outcome: "failure", err: errors.New("runner unavailable")}
	r, err := New(configFor(runner, nil, store))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := r.Step(context.Background()); err != nil {
		t.Fatal(err)
	}
	if got := r.State().Obligations["a"].Status; got != core.ObligationFailed {
		t.Fatalf("got %q", got)
	}
}

func TestConfigurationRejectsMissingStoreAndRunner(t *testing.T) {
	_, err := New(Config{RunID: "r", Goal: core.Goal{ID: "g"}})
	if !errors.Is(err, ErrInvalidConfig) {
		t.Fatalf("got %v", err)
	}
	_, err = New(Config{RunID: "r", Goal: core.Goal{ID: "g"}, Runner: &fakeRunner{}, Store: nil, Obligations: []core.Obligation{{ID: "o", GoalID: "g"}}})
	if !errors.Is(err, ErrStoreRequired) {
		t.Fatalf("got %v", err)
	}
}
