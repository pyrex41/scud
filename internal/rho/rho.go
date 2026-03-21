package rho

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os/exec"
	"regexp"
	"strings"
	"sync"
	"time"
)

// Options configures a rho invocation.
type Options struct {
	Prompt       string
	Model        string
	OutputFormat string // "stream-json" or "" for text
	WorkingDir   string
	TimeoutSecs  int
	SystemPrompt string
	AllowedTools []string
}

// Result holds the output of a rho invocation.
type Result struct {
	Stdout   string
	Stderr   string
	ExitCode int
}

// Run executes rho-cli with the given options.
func Run(ctx context.Context, opts Options) (*Result, error) {
	rhoPath, err := exec.LookPath("rho-cli")
	if err != nil {
		return nil, fmt.Errorf("rho-cli not found in PATH: %w", err)
	}

	var args []string

	if opts.Model != "" {
		args = append(args, "--model", opts.Model)
	}
	if opts.OutputFormat != "" {
		args = append(args, "--output-format", opts.OutputFormat)
	}
	if opts.WorkingDir != "" {
		args = append(args, "-C", opts.WorkingDir)
	}
	if opts.SystemPrompt != "" {
		args = append(args, "--system-append", opts.SystemPrompt)
	}
	if len(opts.AllowedTools) > 0 {
		args = append(args, "--tools", strings.Join(opts.AllowedTools, ","))
	}

	// Prompt is positional, must come last
	args = append(args, opts.Prompt)

	cmd := exec.CommandContext(ctx, rhoPath, args...)
	if opts.WorkingDir != "" {
		cmd.Dir = opts.WorkingDir
	}

	var stdout, stderr strings.Builder
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	err = cmd.Run()
	result := &Result{
		Stdout: stdout.String(),
		Stderr: stderr.String(),
	}

	if cmd.ProcessState != nil {
		result.ExitCode = cmd.ProcessState.ExitCode()
	}

	if err != nil {
		if ctx.Err() != nil {
			return result, fmt.Errorf("rho timed out: %w", ctx.Err())
		}
		return result, nil // non-zero exit is not an error per se
	}

	return result, nil
}

// StreamEvent represents a parsed event from rho-cli's stream-json output.
type StreamEvent struct {
	Type      string `json:"type"`       // "session", "text_delta", "tool_use", "tool_result", "complete"
	Text      string `json:"text"`       // for text_delta
	SessionID string `json:"session_id"` // present on most events
	Success   bool   `json:"success"`    // for complete
	ToolName  string `json:"tool_name"`  // for tool_use
}

// StreamCallback is called for each event during streaming execution.
// It receives the event and the time since the last event (useful for heartbeat tracking).
type StreamCallback func(event StreamEvent)

// RunStreaming executes rho-cli with stream-json output, calling the callback
// for each event. This enables heartbeat detection and progress reporting.
// The returned Result contains the full text output reassembled from deltas.
func RunStreaming(ctx context.Context, opts Options, cb StreamCallback) (*Result, error) {
	rhoPath, err := exec.LookPath("rho-cli")
	if err != nil {
		return nil, fmt.Errorf("rho-cli not found in PATH: %w", err)
	}

	var args []string
	if opts.Model != "" {
		args = append(args, "--model", opts.Model)
	}
	// Force stream-json for heartbeat support
	args = append(args, "--output-format", "stream-json")
	if opts.WorkingDir != "" {
		args = append(args, "-C", opts.WorkingDir)
	}
	if opts.SystemPrompt != "" {
		args = append(args, "--system-append", opts.SystemPrompt)
	}
	if len(opts.AllowedTools) > 0 {
		args = append(args, "--tools", strings.Join(opts.AllowedTools, ","))
	}
	args = append(args, opts.Prompt)

	cmd := exec.CommandContext(ctx, rhoPath, args...)
	if opts.WorkingDir != "" {
		cmd.Dir = opts.WorkingDir
	}

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("stdout pipe: %w", err)
	}
	var stderr strings.Builder
	cmd.Stderr = &stderr

	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("starting rho-cli: %w", err)
	}

	// Read stream events line by line
	var textBuf strings.Builder
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		scanner := bufio.NewScanner(stdout)
		scanner.Buffer(make([]byte, 256*1024), 256*1024) // 256KB line buffer
		for scanner.Scan() {
			line := scanner.Text()
			if line == "" {
				continue
			}
			var ev StreamEvent
			if err := json.Unmarshal([]byte(line), &ev); err != nil {
				continue // skip unparseable lines
			}
			if ev.Type == "text_delta" {
				textBuf.WriteString(ev.Text)
			}
			if cb != nil {
				cb(ev)
			}
		}
	}()

	wg.Wait()
	err = cmd.Wait()

	result := &Result{
		Stdout: textBuf.String(),
		Stderr: stderr.String(),
	}
	if cmd.ProcessState != nil {
		result.ExitCode = cmd.ProcessState.ExitCode()
	}

	if err != nil {
		if ctx.Err() != nil {
			return result, fmt.Errorf("rho timed out: %w", ctx.Err())
		}
		return result, nil
	}
	return result, nil
}

// AdaptiveTimeout manages a deadline that extends when activity is detected.
type AdaptiveTimeout struct {
	mu           sync.Mutex
	cancel       context.CancelFunc
	baseTimeout  time.Duration
	idleTimeout  time.Duration // max time without activity before giving up
	maxTimeout   time.Duration // absolute maximum duration
	lastActivity time.Time
	startTime    time.Time
	timer        *time.Timer
	extended     bool
}

// NewAdaptiveTimeout creates a context with a deadline that resets on heartbeats.
// baseTimeout is the initial deadline. idleTimeout is how long to wait without
// activity before cancelling. maxTimeout is the absolute cap.
func NewAdaptiveTimeout(parent context.Context, baseTimeout, idleTimeout, maxTimeout time.Duration) (context.Context, *AdaptiveTimeout) {
	ctx, cancel := context.WithCancel(parent)
	now := time.Now()
	at := &AdaptiveTimeout{
		cancel:       cancel,
		baseTimeout:  baseTimeout,
		idleTimeout:  idleTimeout,
		maxTimeout:   maxTimeout,
		lastActivity: now,
		startTime:    now,
	}
	// Start with the base timeout
	at.timer = time.AfterFunc(baseTimeout, func() {
		at.checkIdle()
	})
	go func() {
		<-ctx.Done()
		at.timer.Stop()
	}()
	return ctx, at
}

// Heartbeat signals that the agent is still active. Returns true if the
// timeout was extended, false if the max timeout has been reached.
func (at *AdaptiveTimeout) Heartbeat() bool {
	at.mu.Lock()
	defer at.mu.Unlock()
	at.lastActivity = time.Now()
	elapsed := time.Since(at.startTime)
	if elapsed >= at.maxTimeout {
		return false
	}
	at.extended = true
	// Reset the idle timer
	at.timer.Reset(at.idleTimeout)
	return true
}

// Extended reports whether the timeout was ever extended due to heartbeats.
func (at *AdaptiveTimeout) Extended() bool {
	at.mu.Lock()
	defer at.mu.Unlock()
	return at.extended
}

func (at *AdaptiveTimeout) checkIdle() {
	at.mu.Lock()
	defer at.mu.Unlock()
	idle := time.Since(at.lastActivity)
	elapsed := time.Since(at.startTime)
	if elapsed >= at.maxTimeout || idle >= at.idleTimeout {
		at.cancel()
		return
	}
	// Still within max, reset for remaining idle window
	remaining := at.idleTimeout - idle
	if remaining > at.maxTimeout-elapsed {
		remaining = at.maxTimeout - elapsed
	}
	at.timer.Reset(remaining)
}

// IdleDuration returns how long since the last heartbeat.
func (at *AdaptiveTimeout) IdleDuration() time.Duration {
	at.mu.Lock()
	defer at.mu.Unlock()
	return time.Since(at.lastActivity)
}

var (
	jsonFenceRe = regexp.MustCompile("(?s)```json\\s*\n(.*?)```")
	fenceRe     = regexp.MustCompile("(?s)```\\s*\n(.*?)```")
	jsonArrayRe = regexp.MustCompile(`(?s)\[.*\]`)
	jsonObjRe   = regexp.MustCompile(`(?s)\{.*\}`)
)

// extractJSON tries multiple strategies to find JSON in the output.
func extractJSON(output string) string {
	if m := jsonFenceRe.FindStringSubmatch(output); len(m) > 1 {
		return strings.TrimSpace(m[1])
	}
	if m := fenceRe.FindStringSubmatch(output); len(m) > 1 {
		return strings.TrimSpace(m[1])
	}
	if m := jsonArrayRe.FindString(output); m != "" {
		return m
	}
	if m := jsonObjRe.FindString(output); m != "" {
		return m
	}
	trimmed := strings.TrimSpace(output)
	if strings.HasPrefix(trimmed, "[") || strings.HasPrefix(trimmed, "{") {
		return trimmed
	}
	return ""
}

// RunJSON executes rho and parses the JSON output into T.
func RunJSON[T any](ctx context.Context, opts Options) (T, error) {
	var zero T
	// Don't set output-format; use default "text" and extract JSON from response
	opts.OutputFormat = ""
	result, err := Run(ctx, opts)
	if err != nil {
		return zero, err
	}
	if result.ExitCode != 0 {
		return zero, fmt.Errorf("rho exited with code %d: %s", result.ExitCode, result.Stderr)
	}

	jsonStr := extractJSON(result.Stdout)
	if jsonStr == "" {
		return zero, fmt.Errorf("no JSON found in rho output:\n%s", truncate(result.Stdout, 500))
	}

	var v T
	if err := json.Unmarshal([]byte(jsonStr), &v); err != nil {
		return zero, fmt.Errorf("parsing JSON from rho: %w\nraw: %s", err, truncate(jsonStr, 500))
	}
	return v, nil
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
