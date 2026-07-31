// Package executor defines the model-neutral boundary between SCUD scheduling
// and a process that performs one bounded agent run.
package executor

import (
	"context"
	"encoding/json"
)

const RhoRunV1 = "rho.run/v1"

// Runner performs one bounded agent run. Implementations may invoke Rho, a
// remote service, or another harness without exposing provider details to SCUD.
type Runner interface {
	Run(context.Context, Request, EventHandler) (*Result, error)
}

// EventHandler observes portable progress events. It must not retain or modify
// the event's Data bytes.
type EventHandler func(Event)

// Request is SCUD's harness-neutral execution request. A Runner is responsible
// for translating it to its native transport.
type Request struct {
	RunID        string
	Prompt       string
	SystemPrompt string
	Model        ModelRef
	WorkingDir   string
	AllowedTools []string
	Limits       Limits
	Context      map[string]any
}

type ModelRef struct {
	Provider string `json:"provider"`
	ID       string `json:"id"`
}

type Limits struct {
	MaxTurns        *uint32 `json:"max_turns,omitempty"`
	MaxInputTokens  *uint64 `json:"max_input_tokens,omitempty"`
	MaxOutputTokens *uint64 `json:"max_output_tokens,omitempty"`
	MaxCostMicros   *uint64 `json:"max_cost_micros,omitempty"`
	Deadline        string  `json:"deadline,omitempty"`
}

// Event preserves the open rho.run/v1 event namespace. Consumers can inspect
// known event types and safely ignore new ones while sequence validation remains
// active in the protocol adapter.
type Event struct {
	Protocol string          `json:"protocol"`
	RunID    string          `json:"run_id"`
	Sequence uint64          `json:"seq"`
	Time     string          `json:"time"`
	Type     string          `json:"type"`
	Data     json.RawMessage `json:"data"`
}

func (e Event) Terminal() bool {
	return e.Type == "run.completed" || e.Type == "run.failed" || e.Type == "run.cancelled"
}

type Result struct {
	RunID    string
	Text     string
	Outcome  string
	Failure  *Failure
	Usage    Usage
	ExitCode int
	Stderr   string
}

// Failed reports a terminal harness failure independently of process exit code.
func (r *Result) Failed() bool {
	return r != nil && (r.ExitCode != 0 || r.Outcome == "failed" || r.Outcome == "cancelled" || r.Failure != nil)
}

type Failure struct {
	Code         string  `json:"code"`
	Message      string  `json:"message"`
	Retryable    bool    `json:"retryable"`
	RetryAfterMS *uint64 `json:"retry_after_ms,omitempty"`
}

type Usage struct {
	InputTokens     uint64  `json:"input_tokens"`
	OutputTokens    uint64  `json:"output_tokens"`
	CacheReadTokens *uint64 `json:"cache_read_tokens,omitempty"`
	CostMicros      *uint64 `json:"cost_micros,omitempty"`
}
