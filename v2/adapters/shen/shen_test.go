package shen

import (
	"context"
	"testing"

	"github.com/reuben/scud/v2/adapters"
)

type fakeEngine struct{ seen Input }

func (f *fakeEngine) Evaluate(_ context.Context, input Input) (Output, error) {
	f.seen = input
	return Output{Allowed: true, Reason: "test", Constraints: map[string]string{"root": "/tmp"}}, nil
}

func TestPolicyTranslatesWithoutProviderFields(t *testing.T) {
	engine := &fakeEngine{}
	policy := Policy{Engine: engine}
	decision, err := policy.Authorize(context.Background(), adapters.PolicyInput{RunID: "r", Action: "read", Resource: "file", Attributes: map[string]string{"path": "x"}})
	if err != nil || !decision.Allowed || decision.Constraints["root"] != "/tmp" {
		t.Fatalf("unexpected decision: %+v, %v", decision, err)
	}
	if engine.seen.Attributes["path"] != "x" {
		t.Fatalf("input was not translated: %+v", engine.seen)
	}
}

func TestPolicyRequiresEngine(t *testing.T) {
	if _, err := (Policy{}).Authorize(context.Background(), adapters.PolicyInput{}); err == nil {
		t.Fatal("expected nil engine error")
	}
}
