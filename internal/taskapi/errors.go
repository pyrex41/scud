package taskapi

import (
	"errors"
	"fmt"
)

var ErrRevisionConflict = errors.New("revision mismatch")

// RevisionConflictError provides structured optimistic-concurrency failure.
type RevisionConflictError struct {
	Expected string
	Actual   string
}

func (e *RevisionConflictError) Error() string {
	return fmt.Sprintf("revision mismatch: expected %s, current %s", e.Expected, e.Actual)
}

func (e *RevisionConflictError) Unwrap() error { return ErrRevisionConflict }
