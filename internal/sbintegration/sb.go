// Package sbintegration wraps the public sb CLI without interpreting project
// configuration, Shen source, or private verification state.
// On Linux and macOS each invocation owns a process group, killed when its
// launcher exits or its context is canceled. Descendants that deliberately
// detach into a different process group are outside that ownership. Windows
// and other platforms currently cancel only the direct launcher; there is no
// descendant-cleanup guarantee there (Windows needs suspended Job Object launch).
package sbintegration

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"time"
)

const maxOutput = 2 << 20

// State reports availability, not a verification verdict. Reading it never
// starts sb or updates a task's user-reported workflow status.
type State struct {
	SchemaVersion int    `json:"schema_version"`
	Enabled       bool   `json:"enabled"`
	Available     bool   `json:"available"`
	Project       string `json:"project"`
	Executable    string `json:"executable,omitempty"`
	ReportPresent bool   `json:"report_present"`
	ReportStatus  string `json:"report_status"`
}

// Result is evidence of one command invocation, not proof of rule coverage.
type Result struct {
	SchemaVersion int       `json:"schema_version"`
	Operation     string    `json:"operation"`
	StartedAt     time.Time `json:"started_at"`
	FinishedAt    time.Time `json:"finished_at"`
	ExitCode      int       `json:"exit_code"`
	Success       bool      `json:"success"`
	Stdout        string    `json:"stdout"`
	Stderr        string    `json:"stderr"`
	Truncated     bool      `json:"output_truncated"`
	Error         string    `json:"error,omitempty"`
	ReportPresent bool      `json:"report_present"`
	ReportChanged bool      `json:"report_changed"`
	ReportStatus  string    `json:"report_status"`
	ReportSHA256  string    `json:"report_sha256,omitempty"`
}

// Client permits an explicit executable for tests/integrations. The CLI uses
// the sb executable on PATH, never a shell-interpolated command string.
type Client struct {
	Root       string
	Executable string
	Timeout    time.Duration
}

func FindRoot(start string) (string, error) {
	dir, err := filepath.Abs(start)
	if err != nil {
		return "", err
	}
	for {
		for _, marker := range []string{".scud", "sb.toml"} {
			if info, statErr := os.Stat(filepath.Join(dir, marker)); statErr == nil {
				if marker == ".scud" && info.IsDir() || marker == "sb.toml" && info.Mode().IsRegular() {
					return dir, nil
				}
			} else if !errors.Is(statErr, os.ErrNotExist) {
				return "", statErr
			}
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return filepath.Abs(start)
		}
		dir = parent
	}
}

func (c Client) State() (State, error) {
	s := State{SchemaVersion: 1, Project: c.Root, ReportStatus: "absent"}
	marker, err := os.Stat(filepath.Join(c.Root, "sb.toml"))
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return s, err
	}
	s.Enabled = err == nil && marker.Mode().IsRegular()
	if err == nil && !marker.Mode().IsRegular() {
		return s, fmt.Errorf("sb.toml must be a regular file")
	}
	bin, err := exec.LookPath(c.executable())
	if err == nil {
		s.Available, s.Executable = true, bin
	}
	_, s.ReportPresent, err = c.reportDigest()
	if s.ReportPresent {
		s.ReportStatus = "unverified"
	}
	return s, err
}

func (c Client) executable() string {
	if c.Executable != "" {
		return c.Executable
	}
	return "sb"
}

// Run permits only non-agent public verification/context commands. Passing
// gates is deliberately not equated with a fresh discharge report.
func (c Client) Run(ctx context.Context, operation string) (r Result, runErr error) {
	r = Result{SchemaVersion: 1, Operation: operation, ExitCode: -1, ReportStatus: "absent"}
	defer func() {
		if runErr != nil {
			r.Success = false
			r.Error = runErr.Error()
		}
	}()
	args := map[string][]string{
		"check": {"gates"}, "context": {"context"}, "derive": {"derive"},
		"audit": {"audit-report"}, "gen": {"gen"},
	}[operation]
	if args == nil {
		return r, fmt.Errorf("unsupported sb operation %q", operation)
	}
	state, err := c.State()
	if err != nil {
		return r, err
	}
	if !state.Enabled {
		return r, fmt.Errorf("sb is not configured: no sb.toml at %s (core task commands do not require sb)", c.Root)
	}
	if !state.Available {
		return r, fmt.Errorf("sb.toml exists but the sb executable is unavailable; install sb or fix PATH")
	}
	before, _, err := c.reportDigest()
	if err != nil {
		return r, err
	}
	timeout := c.Timeout
	if timeout <= 0 {
		timeout = 5 * time.Minute
	}
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()
	command := exec.CommandContext(ctx, state.Executable, args...)
	command.Dir = c.Root
	// Avoid hanging forever on inherited output pipes after cancellation.
	command.WaitDelay = time.Second
	stdout, stderr := &boundedBuffer{}, &boundedBuffer{}
	command.Stdout, command.Stderr = stdout, stderr
	r.StartedAt = time.Now().UTC()
	err = runProcess(command)
	r.FinishedAt = time.Now().UTC()
	r.Stdout, r.Stderr = stdout.String(), stderr.String()
	r.Truncated = stdout.truncated || stderr.truncated
	if command.ProcessState != nil {
		r.ExitCode = command.ProcessState.ExitCode()
	}
	if ctx.Err() != nil {
		err = ctx.Err()
	}
	r.Success = err == nil
	if err != nil {
		r.Error = err.Error()
	}
	after, present, reportErr := c.reportDigest()
	r.ReportPresent = present
	r.ReportChanged = present && before != after
	r.ReportSHA256 = after
	if present {
		// Changed content is not sufficient evidence of freshness, snapshot
		// identity or rule coverage. Keep that uncertainty explicit.
		r.ReportStatus = "unverified"
	}
	if reportErr != nil {
		return r, fmt.Errorf("reading optional sb report: %w", reportErr)
	}
	if err != nil {
		return r, fmt.Errorf("sb %s: %w", args[0], err)
	}
	return r, nil
}

func (c Client) reportDigest() (string, bool, error) {
	directory, err := os.Lstat(filepath.Join(c.Root, ".sb"))
	if errors.Is(err, os.ErrNotExist) {
		return "", false, nil
	}
	if err != nil {
		return "", false, err
	}
	if !directory.IsDir() || directory.Mode()&os.ModeSymlink != 0 {
		return "", false, fmt.Errorf(".sb must be a directory inside the project, not a symlink")
	}
	path := filepath.Join(c.Root, ".sb", "discharge_report.json")
	info, err := os.Lstat(path)
	if errors.Is(err, os.ErrNotExist) {
		return "", false, nil
	}
	if err != nil {
		return "", false, err
	}
	if !info.Mode().IsRegular() {
		return "", true, fmt.Errorf("sb report must be a regular file, not a symlink or directory")
	}
	f, err := os.Open(path)
	if err != nil {
		return "", true, err
	}
	defer f.Close()
	h := sha256.New()
	if _, err = io.Copy(h, f); err != nil {
		return "", true, err
	}
	return hex.EncodeToString(h.Sum(nil)), true, nil
}

type boundedBuffer struct {
	mu        sync.Mutex
	buf       bytes.Buffer
	truncated bool
}

func (b *boundedBuffer) Write(p []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	n := len(p)
	remaining := maxOutput - b.buf.Len()
	if len(p) > remaining {
		p = p[:remaining]
		b.truncated = true
	}
	_, _ = b.buf.Write(p)
	return n, nil
}

func (b *boundedBuffer) String() string {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.buf.String()
}

// WriteJSON deliberately does not truncate a JSON document into invalid JSON.
func WriteJSON(w io.Writer, value any) error {
	encoder := json.NewEncoder(w)
	encoder.SetEscapeHTML(false)
	return encoder.Encode(value)
}
