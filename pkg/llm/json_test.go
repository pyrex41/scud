package llm

import (
	"context"
	"errors"
	"testing"
)

type typedAnswer struct{ Text string }
type partialCaller struct{ err error }

func (c partialCaller) CompleteTargetJSON(_ context.Context, _, _ string, output any) error {
	output.(*typedAnswer).Text = "partial"
	return c.err
}

func TestTypedJSONDoesNotLeakPartialResult(t *testing.T) {
	failure := errors.New("truncated")
	value, err := RequestJSON[typedAnswer](t.Context(), partialCaller{failure}, "prompt", "system")
	if !errors.Is(err, failure) || value.Text != "" {
		t.Fatalf("partial result leaked: %+v, %v", value, err)
	}
	value, err = RequestJSON[typedAnswer](t.Context(), partialCaller{}, "prompt", "system")
	if err != nil || value.Text != "partial" {
		t.Fatalf("result=%+v err=%v", value, err)
	}
}
