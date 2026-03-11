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
	OutputFormat string // "json" or "" for text
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

// Run executes rho with the given options.
func Run(ctx context.Context, opts Options) (*Result, error) {
	rhoPath, err := exec.LookPath("rho")
	if err != nil {
		return nil, fmt.Errorf("rho not found in PATH: %w (install from https://github.com/reuben/rho)", err)
	}

	args := []string{"-p", opts.Prompt}

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
		args = append(args, "--system", opts.SystemPrompt)
	}
	for _, tool := range opts.AllowedTools {
		args = append(args, "--allowedTools", tool)
	}

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

// RunJSON executes rho and parses the JSON output into T.
func RunJSON[T any](ctx context.Context, opts Options) (T, error) {
	var zero T
	opts.OutputFormat = "json"
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

// extractJSON tries multiple strategies to find JSON in the output.
func extractJSON(output string) string {
	// Strategy 1: ```json fence
	if m := jsonFenceRe.FindStringSubmatch(output); len(m) > 1 {
		return strings.TrimSpace(m[1])
	}
	// Strategy 2: ``` fence
	if m := fenceRe.FindStringSubmatch(output); len(m) > 1 {
		return strings.TrimSpace(m[1])
	}
	// Strategy 3: first JSON array
	if m := jsonArrayRe.FindString(output); m != "" {
		return m
	}
	// Strategy 4: first JSON object
	if m := jsonObjRe.FindString(output); m != "" {
		return m
	}
	// Strategy 5: raw output
	trimmed := strings.TrimSpace(output)
	if (strings.HasPrefix(trimmed, "[") || strings.HasPrefix(trimmed, "{")) {
		return trimmed
	}
	return ""
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
