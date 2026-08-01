// Package rho adapts the versioned rho.run/v1 JSONL protocol to the provider-
// neutral v2 adapters.Runner seam. No rho symbols are required by v2 core
// packages; this package is an optional integration.
package rho

import (
	"context"
	"fmt"
	"io"
	"strings"

	"github.com/reuben/scud/pkg/executor"
	"github.com/reuben/scud/v2/adapters"
)

// Runner invokes a rho.run/v1 producer. Command and Args are passed through
// to executor.RhoV1; an empty command uses rho-cli's normal lookup.
type Runner struct {
	Command   string
	Args      []string
	Grant     executor.Grant
	Authorize func(context.Context, []byte) (string, string, error)
}

func (r Runner) Run(ctx context.Context, req adapters.RunRequest, sink adapters.EventSink) (adapters.RunResult, error) {
	provider, model, err := splitModel(req.Model)
	if err != nil {
		return adapters.RunResult{RunID: req.RunID}, err
	}
	inner := executor.RhoV1{Command: r.Command, Args: r.Args, Grant: r.Grant, Authorize: r.Authorize}
	result, runErr := inner.Run(ctx, executor.Request{
		RunID:        req.RunID,
		Prompt:       req.Prompt,
		SystemPrompt: req.SystemPrompt,
		Model:        executor.ModelRef{Provider: provider, ID: model},
		WorkingDir:   req.WorkingDir,
		AllowedTools: req.AllowedTools,
		Limits: executor.Limits{
			MaxTurns:        req.Limits.MaxTurns,
			MaxInputTokens:  req.Limits.MaxInputTokens,
			MaxOutputTokens: req.Limits.MaxOutputTokens,
			MaxCostMicros:   req.Limits.MaxCostMicros,
			Deadline:        req.Limits.Deadline,
		},
		Context: req.Context,
	}, func(event executor.Event) {
		if sink == nil {
			return
		}
		sink(adapters.Event{
			RunID:    event.RunID,
			Sequence: event.Sequence,
			Time:     event.Time,
			Type:     event.Type,
			Data:     append([]byte(nil), event.Data...),
		})
	})
	if result == nil {
		return adapters.RunResult{RunID: req.RunID}, runErr
	}
	return adapters.RunResult{
		RunID:   result.RunID,
		Text:    result.Text,
		Outcome: result.Outcome,
		Failure: convertFailure(result.Failure),
		Usage: adapters.Usage{
			InputTokens:     result.Usage.InputTokens,
			OutputTokens:    result.Usage.OutputTokens,
			CacheReadTokens: result.Usage.CacheReadTokens,
			CostMicros:      result.Usage.CostMicros,
		},
		ExitCode: result.ExitCode,
		Stderr:   result.Stderr,
	}, runErr
}

// Consume validates a rho.run/v1 stream and translates events without
// starting a process. It is useful for conformance tests and alternate
// process transports.
func Consume(reader io.Reader, runID string, sink adapters.EventSink) (adapters.RunResult, error) {
	result, err := executor.ConsumeRhoV1(reader, runID, func(event executor.Event) {
		if sink != nil {
			sink(adapters.Event{RunID: event.RunID, Sequence: event.Sequence, Time: event.Time, Type: event.Type, Data: append([]byte(nil), event.Data...)})
		}
	})
	if result == nil {
		return adapters.RunResult{RunID: runID}, err
	}
	return adapters.RunResult{RunID: result.RunID, Text: result.Text, Outcome: result.Outcome, Failure: convertFailure(result.Failure), Usage: adapters.Usage{InputTokens: result.Usage.InputTokens, OutputTokens: result.Usage.OutputTokens, CacheReadTokens: result.Usage.CacheReadTokens, CostMicros: result.Usage.CostMicros}, ExitCode: result.ExitCode, Stderr: result.Stderr}, err
}

func splitModel(model string) (provider, id string, err error) {
	provider, id, ok := strings.Cut(model, "/")
	if !ok || provider == "" || id == "" || strings.Contains(id, "/") {
		return "", "", fmt.Errorf("rho model %q must use provider/model form", model)
	}
	return provider, id, nil
}

func convertFailure(failure *executor.Failure) *adapters.Failure {
	if failure == nil {
		return nil
	}
	return &adapters.Failure{Code: failure.Code, Message: failure.Message, Retryable: failure.Retryable, RetryAfterMS: failure.RetryAfterMS}
}

var _ adapters.Runner = Runner{}
