package executor

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"strings"
)

// RhoV1 invokes a rho.run/v1 producer. Command defaults to rho-cli and Args
// defaults to: run --request-file - --events jsonl.
type RhoV1 struct {
	Command string
	Args    []string
	Grant   Grant
}

type Grant struct {
	GrantID    string       `json:"grant_id"`
	ExpiresAt  string       `json:"expires_at"`
	Providers  []string     `json:"providers"`
	Models     []string     `json:"models"`
	Tools      []string     `json:"tools"`
	ReadRoots  []string     `json:"read_roots"`
	WriteRoots []string     `json:"write_roots"`
	Network    NetworkGrant `json:"network"`
	Witness    string       `json:"witness,omitempty"`
}

type NetworkGrant struct {
	Mode         string   `json:"mode"`
	Destinations []string `json:"destinations,omitempty"`
}

type wireRequest struct {
	Protocol string         `json:"protocol"`
	RunID    string         `json:"run_id"`
	Model    ModelRef       `json:"model"`
	Input    []wireMessage  `json:"input"`
	System   string         `json:"system,omitempty"`
	Limits   Limits         `json:"limits"`
	Grant    Grant          `json:"grant"`
	Context  map[string]any `json:"context,omitempty"`
}

type wireMessage struct {
	Role    string      `json:"role"`
	Content []wireBlock `json:"content"`
}

type wireBlock struct {
	Type string `json:"type"`
	Text string `json:"text"`
}

func (r RhoV1) Run(ctx context.Context, req Request, handler EventHandler) (*Result, error) {
	if req.RunID == "" || req.Model.Provider == "" || req.Model.ID == "" {
		return nil, errors.New("rho.run/v1 requires run ID, provider, and model ID")
	}
	grant := r.Grant
	if len(req.AllowedTools) > 0 {
		grant.Tools = append([]string(nil), req.AllowedTools...)
	}
	wire := wireRequest{
		Protocol: RhoRunV1,
		RunID:    req.RunID,
		Model:    req.Model,
		Input: []wireMessage{{
			Role:    "user",
			Content: []wireBlock{{Type: "text", Text: req.Prompt}},
		}},
		System:  req.SystemPrompt,
		Limits:  req.Limits,
		Grant:   grant,
		Context: req.Context,
	}
	payload, err := json.Marshal(wire)
	if err != nil {
		return nil, fmt.Errorf("marshal rho.run/v1 request: %w", err)
	}

	command := r.Command
	if command == "" {
		command = "rho-cli"
	}
	args := r.Args
	if len(args) == 0 {
		args = []string{"run", "--request-file", "-", "--events", "jsonl"}
	}
	cmd := exec.CommandContext(ctx, command, args...)
	cmd.Dir = req.WorkingDir
	cmd.Stdin = strings.NewReader(string(payload))
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("rho stdout pipe: %w", err)
	}
	var stderr strings.Builder
	cmd.Stderr = &stderr
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("start rho.run/v1 producer: %w", err)
	}

	result, streamErr := ConsumeRhoV1(stdout, req.RunID, handler)
	waitErr := cmd.Wait()
	result.ExitCode = -1
	result.Stderr = stderr.String()
	if cmd.ProcessState != nil {
		result.ExitCode = cmd.ProcessState.ExitCode()
	}
	if streamErr != nil {
		return result, streamErr
	}
	if waitErr != nil && ctx.Err() != nil {
		return result, ctx.Err()
	}
	if waitErr != nil {
		return result, fmt.Errorf("rho.run/v1 producer exited %d: %w", result.ExitCode, waitErr)
	}
	return result, nil
}

// ConsumeRhoV1 validates and reduces one rho.run/v1 JSONL stream.
func ConsumeRhoV1(reader io.Reader, expectedRunID string, handler EventHandler) (*Result, error) {
	result := &Result{RunID: expectedRunID}
	var text strings.Builder
	var previous uint64
	seen := false
	terminal := false
	scanner := bufio.NewScanner(reader)
	scanner.Buffer(make([]byte, 64*1024), 1024*1024)
	for scanner.Scan() {
		if strings.TrimSpace(scanner.Text()) == "" {
			continue
		}
		var event Event
		if err := json.Unmarshal(scanner.Bytes(), &event); err != nil {
			return result, fmt.Errorf("decode rho.run/v1 event: %w", err)
		}
		if event.Protocol != RhoRunV1 {
			return result, fmt.Errorf("unsupported protocol %q", event.Protocol)
		}
		if event.RunID != expectedRunID {
			return result, fmt.Errorf("event run ID %q does not match %q", event.RunID, expectedRunID)
		}
		if terminal {
			return result, errors.New("event received after terminal event")
		}
		if seen && event.Sequence <= previous {
			return result, fmt.Errorf("non-monotonic event sequence: %d follows %d", event.Sequence, previous)
		}
		seen, previous = true, event.Sequence
		if event.Type == "message.delta" {
			var delta struct {
				Text string `json:"text"`
			}
			if err := json.Unmarshal(event.Data, &delta); err != nil {
				return result, fmt.Errorf("decode message.delta: %w", err)
			}
			text.WriteString(delta.Text)
		}
		if handler != nil {
			handler(event)
		}
		terminal = event.Terminal()
		if terminal {
			if err := reduceTerminal(result, event); err != nil {
				return result, err
			}
		}
	}
	result.Text = text.String()
	if err := scanner.Err(); err != nil {
		return result, fmt.Errorf("read rho.run/v1 stream: %w", err)
	}
	if !terminal {
		return result, errors.New("rho.run/v1 stream ended without terminal event")
	}
	return result, nil
}

func reduceTerminal(result *Result, event Event) error {
	result.Outcome = strings.TrimPrefix(event.Type, "run.")
	switch event.Type {
	case "run.completed":
		var data struct {
			Status string `json:"status"`
			Usage  Usage  `json:"usage"`
		}
		if err := json.Unmarshal(event.Data, &data); err != nil || data.Status != "succeeded" {
			return errors.New("invalid run.completed payload")
		}
		result.Usage = data.Usage
	case "run.failed":
		var failure Failure
		if err := json.Unmarshal(event.Data, &failure); err != nil || failure.Code == "" || failure.Message == "" {
			return errors.New("invalid run.failed payload")
		}
		result.Failure = &failure
	case "run.cancelled":
		var data struct {
			Reason string `json:"reason"`
			Usage  Usage  `json:"usage"`
		}
		if err := json.Unmarshal(event.Data, &data); err != nil || data.Reason == "" {
			return errors.New("invalid run.cancelled payload")
		}
		result.Usage = data.Usage
	}
	return nil
}
