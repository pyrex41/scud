// Package shen contains the optional Shen policy integration. The v2 runtime
// only depends on adapters.Policy; Shen-specific request/decision plumbing is
// kept here.
package shen

import (
	"context"
	"errors"

	"github.com/reuben/scud/v2/adapters"
)

// Evaluator is the small portion of a Shen engine needed by SCUD. Keeping it
// local avoids coupling the v2 module to a particular Shen SDK version.
type Evaluator interface {
	Evaluate(context.Context, Input) (Output, error)
}

type Input struct {
	RunID      string
	Action     string
	Resource   string
	Attributes map[string]string
}

type Output struct {
	Allowed     bool
	Reason      string
	Constraints map[string]string
}

type Policy struct{ Engine Evaluator }

func (p Policy) Authorize(ctx context.Context, in adapters.PolicyInput) (adapters.Decision, error) {
	if p.Engine == nil {
		return adapters.Decision{}, errors.New("shen policy engine is nil")
	}
	out, err := p.Engine.Evaluate(ctx, Input{RunID: in.RunID, Action: in.Action, Resource: in.Resource, Attributes: clone(in.Attributes)})
	if err != nil {
		return adapters.Decision{}, err
	}
	return adapters.Decision{Allowed: out.Allowed, Reason: out.Reason, Constraints: clone(out.Constraints)}, nil
}

func clone(values map[string]string) map[string]string {
	if len(values) == 0 {
		return nil
	}
	copy := make(map[string]string, len(values))
	for key, value := range values {
		copy[key] = value
	}
	return copy
}

var _ adapters.Policy = Policy{}
