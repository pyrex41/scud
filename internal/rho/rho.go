package rho

import (
	"context"
	"encoding/json"
	"fmt"
	"os/exec"
	"regexp"
	"strings"
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
