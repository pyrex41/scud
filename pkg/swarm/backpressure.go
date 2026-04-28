package swarm

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/reuben/scud/internal/config"
)

// ValidationResult holds results of running backpressure validation.
type ValidationResult struct {
	AllPassed bool
	Results   []CommandResult
}

// CommandResult holds the result of a single validation command.
type CommandResult struct {
	Command     string
	Passed      bool
	ExitCode    int
	Stdout      string
	Stderr      string
	DurationSec float64
}

// RunValidation executes configured backpressure commands.
func RunValidation(ctx context.Context, workDir string, cfg *config.Config) ValidationResult {
	commands := cfg.Swarm.Backpressure.Commands
	if len(commands) == 0 {
		commands = autoDetectCommands(workDir)
	}
	if len(commands) == 0 {
		return ValidationResult{AllPassed: true}
	}

	timeout := time.Duration(cfg.Swarm.Backpressure.TimeoutSecs) * time.Second
	if timeout == 0 {
		timeout = 300 * time.Second
	}

	var result ValidationResult
	result.AllPassed = true

	for _, cmdStr := range commands {
		cmdCtx, cancel := context.WithTimeout(ctx, timeout)
		start := time.Now()

		cmd := exec.CommandContext(cmdCtx, "sh", "-c", cmdStr)
		cmd.Dir = workDir

		var stdout, stderr strings.Builder
		cmd.Stdout = &stdout
		cmd.Stderr = &stderr

		err := cmd.Run()
		cancel()

		cr := CommandResult{
			Command:     cmdStr,
			Stdout:      truncateStr(stdout.String(), 1000),
			Stderr:      truncateStr(stderr.String(), 1000),
			DurationSec: time.Since(start).Seconds(),
		}

		if err != nil {
			cr.Passed = false
			if cmd.ProcessState != nil {
				cr.ExitCode = cmd.ProcessState.ExitCode()
			} else {
				cr.ExitCode = -1
			}
			result.AllPassed = false
		} else {
			cr.Passed = true
			cr.ExitCode = 0
		}

		result.Results = append(result.Results, cr)

		if !cr.Passed && cfg.Swarm.Backpressure.StopOnFailure {
			break
		}
	}

	return result
}

// autoDetectCommands detects validation commands based on project files.
func autoDetectCommands(workDir string) []string {
	if fileExists(filepath.Join(workDir, "go.mod")) {
		return []string{"go build ./...", "go test ./..."}
	}
	if fileExists(filepath.Join(workDir, "Cargo.toml")) {
		return []string{"cargo build", "cargo test"}
	}
	if fileExists(filepath.Join(workDir, "package.json")) {
		return detectNpmCommands(workDir)
	}
	if fileExists(filepath.Join(workDir, "pyproject.toml")) {
		return []string{"pytest"}
	}
	return nil
}

func detectNpmCommands(workDir string) []string {
	data, err := os.ReadFile(filepath.Join(workDir, "package.json"))
	if err != nil {
		return nil
	}
	content := string(data)
	var cmds []string
	for _, script := range []string{"build", "test", "lint", "typecheck"} {
		if strings.Contains(content, fmt.Sprintf(`"%s"`, script)) {
			cmds = append(cmds, fmt.Sprintf("npm run %s", script))
		}
	}
	return cmds
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func truncateStr(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
