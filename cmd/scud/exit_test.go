package main

import (
	"context"
	"errors"
	"fmt"
	"testing"

	"github.com/reuben/scud/internal/taskapi"
)

func TestScriptExitCodes(t *testing.T) {
	for _, tc := range []struct {
		err  error
		want int
	}{
		{errors.New("invalid document"), 1},
		{fmt.Errorf("wrapped: %w", &taskapi.RevisionConflictError{Expected: "old", Actual: "new"}), 3},
		{fmt.Errorf("wrapped: %w", context.DeadlineExceeded), 124},
		{context.Canceled, 130},
	} {
		if got := exitCode(tc.err); got != tc.want {
			t.Fatalf("exitCode(%v)=%d, want %d", tc.err, got, tc.want)
		}
	}
}
