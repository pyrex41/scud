package llm

import (
	"context"
	"fmt"
)

// JSONCaller is a narrow port implementable by providers and test doubles.
// Callers use RequestJSON[T] to select an output type rather than an any result.
type JSONCaller interface {
	CompleteTargetJSON(context.Context, string, string, any) error
}

// RequestJSON returns a typed value only on success, never a partially filled
// response from a failed provider call.
func RequestJSON[T any](ctx context.Context, caller JSONCaller, prompt, system string) (T, error) {
	var value T
	if err := ctx.Err(); err != nil {
		return value, err
	}
	if caller == nil {
		return value, fmt.Errorf("model caller is required")
	}
	if err := caller.CompleteTargetJSON(ctx, prompt, system, &value); err != nil {
		var zero T
		return zero, err
	}
	if err := ctx.Err(); err != nil {
		var zero T
		return zero, err
	}
	return value, nil
}
