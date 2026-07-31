// Package adapters contains the narrow seams used by the SCUD v2 runtime.
//
// These interfaces intentionally describe workflow concerns rather than an
// LLM vendor, policy implementation, database, or task CLI. Concrete
// integrations live in subpackages (for example adapters/rho and
// adapters/shen), so the v2 graph and scheduler can be tested without any of
// those dependencies.
package adapters

import "context"

// Runner executes one bounded agent run. Model is an opaque routing string;
// interpreting it (for example as provider/model) is an adapter concern.
type Runner interface {
	Run(context.Context, RunRequest, EventSink) (RunResult, error)
}

type RunRequest struct {
	RunID        string
	Prompt       string
	SystemPrompt string
	Model        string
	WorkingDir   string
	AllowedTools []string
	Limits       Limits
	Context      map[string]any
}

type Limits struct {
	MaxTurns        *uint32
	MaxInputTokens  *uint64
	MaxOutputTokens *uint64
	MaxCostMicros   *uint64
	Deadline        string
}

type RunResult struct {
	RunID    string
	Text     string
	Outcome  string
	Failure  *Failure
	Usage    Usage
	ExitCode int
	Stderr   string
}

type Failure struct {
	Code         string
	Message      string
	Retryable    bool
	RetryAfterMS *uint64
}

type Usage struct {
	InputTokens     uint64
	OutputTokens    uint64
	CacheReadTokens *uint64
	CostMicros      *uint64
}

// Event is the provider-neutral progress envelope. Data is opaque to the
// runtime and owned by the producer's event namespace.
type Event struct {
	RunID    string
	Sequence uint64
	Time     string
	Type     string
	Data     []byte
}

type EventSink func(Event)

// Policy makes an authorization decision before an adapter performs an
// operation. Action and Resource are stable SCUD vocabulary; Attributes are
// extension data interpreted by a policy adapter.
type Policy interface {
	Authorize(context.Context, PolicyInput) (Decision, error)
}

type PolicyInput struct {
	RunID      string
	Action     string
	Resource   string
	Attributes map[string]string
}

type Decision struct {
	Allowed     bool
	Reason      string
	Constraints map[string]string
}

// EventStore persists append-only run events. Implementations must reject a
// duplicate or out-of-order sequence for the same run.
type EventStore interface {
	Append(context.Context, Event) error
	List(context.Context, string, uint64) ([]Event, error)
}

// TaskDirectory is the v2 boundary around the legacy task database ("td").
// Task is deliberately a small projection; adapters can retain richer legacy
// fields without leaking them into the runtime.
type TaskDirectory interface {
	Get(context.Context, string) (Task, error)
	SetStatus(context.Context, string, string) error
	Ready(context.Context, string) ([]Task, error)
}

type Task struct {
	ID           string
	Title        string
	Status       string
	Dependencies []string
}
