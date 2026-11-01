package cmd

import (
	"context"
	"errors"
	"fmt"
	"io"
	"strings"
	"testing"
	"time"

	"github.com/reuben/scud/pkg/generate"
)

type ensembleStub struct{ failure bool }

func (c ensembleStub) CompleteTargetJSON(_ context.Context, _, _ string, output any) error {
	if c.failure {
		return fmt.Errorf("unavailable")
	}
	output.(*ensembleAnswer).Answer = "answer"
	return nil
}

func TestEnsembleTimeoutIncludesInput(t *testing.T) {
	r, w := io.Pipe()
	defer r.Close()
	defer w.Close()
	c := NewExperimentalCmd()
	c.SetIn(r)
	c.SetOut(io.Discard)
	c.SetErr(io.Discard)
	c.SetArgs([]string{"ensemble", "--timeout", "20ms"})
	start := time.Now()
	if err := c.Execute(); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("blocked input = %v", err)
	}
	if time.Since(start) > time.Second {
		t.Fatal("timeout did not include input")
	}
}

func TestEnsembleBoundsArgumentPrompt(t *testing.T) {
	c := NewExperimentalCmd()
	c.SetOut(io.Discard)
	c.SetErr(io.Discard)
	c.SetArgs([]string{"ensemble", strings.Repeat("x", (4<<20)+1)})
	if err := c.Execute(); err == nil || !strings.Contains(err.Error(), "4 MiB") {
		t.Fatalf("oversized argument = %v", err)
	}
}

func TestEnsembleNoHarnessAndExplicitFailure(t *testing.T) {
	factory := func(name string) (generate.TargetCaller, error) { return ensembleStub{failure: name == "bad"}, nil }
	r, err := runEnsemble(context.Background(), factory, "question", []string{"main", "fast"}, "main", 2)
	if err != nil || !r.Experimental || len(r.Members) != 2 || r.Synthesis != "answer" {
		t.Fatalf("result=%+v err=%v", r, err)
	}
	r, err = runEnsemble(context.Background(), factory, "question", []string{"main", "bad"}, "main", 2)
	if err == nil || r.Synthesis != "" || r.Members[1].Error == "" {
		t.Fatalf("result=%+v err=%v", r, err)
	}
}
