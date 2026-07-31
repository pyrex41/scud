package rho

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/reuben/scud/v2/adapters"
)

func TestConsumeCompletedFixture(t *testing.T) {
	file, err := os.Open(filepath.Join("testdata", "completed.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	var events []adapters.Event
	result, err := Consume(file, "fixture-1", func(event adapters.Event) { events = append(events, event) })
	if err != nil {
		t.Fatal(err)
	}
	if result.Text != "hello" || result.Outcome != "completed" || result.Usage.InputTokens != 4 {
		t.Fatalf("unexpected result: %+v", result)
	}
	if len(events) != 3 || events[1].Type != "message.delta" {
		t.Fatalf("unexpected events: %+v", events)
	}
}

func TestConsumeRejectsAfterTerminal(t *testing.T) {
	file, err := os.Open(filepath.Join("testdata", "invalid_after_terminal.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	if _, err := Consume(file, "fixture-2", nil); err == nil {
		t.Fatal("expected terminal invariant error")
	}
}

func TestRunnerRejectsProviderlessModelBeforeProcess(t *testing.T) {
	_, err := (Runner{}).Run(context.Background(), adapters.RunRequest{RunID: "x", Model: "claude"}, nil)
	if err == nil {
		t.Fatal("expected provider/model validation error")
	}
}
