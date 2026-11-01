package cmd

import (
	"bytes"
	"context"
	"errors"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"syscall"
	"testing"
	"time"

	"github.com/reuben/scud/pkg/generate"
)

type neverProposalCaller struct{ calls int }

func (n *neverProposalCaller) CompleteTargetJSON(context.Context, string, string, any) error {
	n.calls++
	return nil
}

func TestPRDModelFlagOnlyAcceptsMainFastBeforeCaller(t *testing.T) {
	for _, model := range []string{"other", "primary", ""} {
		caller := &neverProposalCaller{}
		c := NewPRDCmd(caller)
		c.SetIn(strings.NewReader("PRD"))
		args := []string{"-", "--model", model}
		c.SetArgs(args)
		if e := c.Execute(); e == nil {
			t.Fatalf("accepted model %q", model)
		}
		if caller.calls != 0 {
			t.Fatalf("made %d calls for model %q", caller.calls, model)
		}
	}
	for _, model := range []string{"MAIN", "Fast"} {
		caller := &neverProposalCaller{}
		c := NewPRDCmd(caller)
		c.SetIn(strings.NewReader("PRD"))
		c.SetArgs([]string{"-", "--model", model})
		_ = c.Execute()
		if caller.calls != 1 {
			t.Fatalf("case-insensitive alias %q did not reach caller", model)
		}
	}
}

func TestPRDInputAndNumericBounds(t *testing.T) {
	for _, args := range [][]string{{"-", "--num", "-1"}, {"-", "--tag", "bad/tag"}} {
		caller := &neverProposalCaller{}
		c := NewPRDCmd(caller)
		c.SetArgs(args)
		c.SetIn(strings.NewReader("PRD"))
		if e := c.Execute(); e == nil {
			t.Fatalf("accepted args %q", args)
		}
		if caller.calls != 0 {
			t.Fatal("invalid invocation called model")
		}
	}
	caller := &neverProposalCaller{}
	c := NewPRDCmd(caller)
	c.SetArgs([]string{"-"})
	c.SetIn(strings.NewReader(strings.Repeat("x", (4<<20)+1)))
	if e := c.Execute(); e == nil || !strings.Contains(e.Error(), "4 MiB") {
		t.Fatalf("expected bounded input error, got %v", e)
	}
}

var _ generate.TargetCaller = (*neverProposalCaller)(nil)

func TestPRDCancelsBlockedInput(t *testing.T) {
	r, w := io.Pipe()
	defer r.Close()
	defer w.Close()
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	caller := &neverProposalCaller{}
	c := NewPRDCmd(caller)
	c.SetIn(r)
	c.SetOut(io.Discard)
	c.SetErr(io.Discard)
	c.SetArgs([]string{"-"})
	if err := c.ExecuteContext(ctx); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("blocked read = %v", err)
	}
	if caller.calls != 0 {
		t.Fatal("canceled input reached model")
	}
}

func TestCLIBlockedInputSIGTERM(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Unix signal regression")
	}
	binary := filepath.Join(t.TempDir(), "scud")
	build := exec.Command("go", "build", "-o", binary, "./cmd/scud")
	build.Dir = filepath.Join("..", "..")
	if b, err := build.CombinedOutput(); err != nil {
		t.Fatalf("build CLI: %v\n%s", err, b)
	}
	for _, args := range [][]string{{"parse", "-"}, {"experimental", "ensemble"}} {
		t.Run(strings.Join(args, "_"), func(t *testing.T) {
			r, w, err := os.Pipe()
			if err != nil {
				t.Fatal(err)
			}
			defer r.Close()
			defer w.Close()
			p := exec.Command(binary, args...)
			p.Stdin = r
			var stderr bytes.Buffer
			p.Stderr = &stderr
			if err := p.Start(); err != nil {
				t.Fatal(err)
			}
			done := make(chan error, 1)
			go func() { done <- p.Wait() }()
			writeDone := make(chan error, 1)
			go func() {
				_, err := w.Write(bytes.Repeat([]byte(" "), 256<<10))
				writeDone <- err
			}()
			waited, written := false, false
			defer func() {
				_ = w.Close()
				_ = p.Process.Kill()
				if !waited {
					<-done
				}
				if !written {
					<-writeDone
				}
			}()
			// Writing more than pipe capacity proves the CLI has started
			// consuming stdin after installing its signal context. Keep the
			// write end open afterward, so ReadAll still cannot reach EOF.
			select {
			case err := <-writeDone:
				written = true
				if err != nil {
					t.Fatal(err)
				}
			case err := <-done:
				waited = true
				t.Fatalf("CLI exited before reading input: %v stderr=%s", err, &stderr)
			case <-time.After(3 * time.Second):
				t.Fatal("CLI did not start consuming stdin")
			}
			if err := p.Process.Signal(syscall.SIGTERM); err != nil {
				t.Fatal(err)
			}
			select {
			case err := <-done:
				waited = true
				var exit *exec.ExitError
				if !errors.As(err, &exit) || exit.ExitCode() != 130 || !strings.Contains(stderr.String(), "context canceled") {
					t.Fatalf("SIGTERM result=%v stderr=%s", err, &stderr)
				}
			case <-time.After(3 * time.Second):
				t.Fatal("CLI ignored SIGTERM while stdin remained open")
			}
		})
	}
}
