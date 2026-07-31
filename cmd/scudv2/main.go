// scudv2 is an opt-in, provider-blind view of the v2 core. It intentionally
// only reads a JSON goal/event document; execution and persistence remain
// adapter responsibilities.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/reuben/scud/v2/core"
)

type document struct {
	Goal   *core.Goal   `json:"goal,omitempty"`
	Budget core.Budget  `json:"budget,omitempty"`
	Events []core.Event `json:"events"`
}

func main() {
	if err := run(context.Background(), os.Args[1:], os.Stdout, os.Stderr); err != nil {
		fmt.Fprintln(os.Stderr, "scudv2:", err)
		os.Exit(1)
	}
}

func run(ctx context.Context, args []string, stdout, stderr io.Writer) error {
	return runWithStdin(ctx, args, os.Stdin, stdout, stderr)
}

func runWithStdin(ctx context.Context, args []string, stdin io.Reader, stdout, stderr io.Writer) error {
	if len(args) == 0 {
		return errors.New("usage: scudv2 <validate|plan|replay> <document.json>")
	}
	command := args[0]
	if command != "validate" && command != "plan" && command != "replay" {
		return fmt.Errorf("unknown command %q (want validate, plan, or replay)", command)
	}
	fs := flag.NewFlagSet(command, flag.ContinueOnError)
	fs.SetOutput(stderr)
	if err := fs.Parse(args[1:]); err != nil {
		return err
	}
	if fs.NArg() != 1 {
		return fmt.Errorf("usage: scudv2 %s <document.json>", command)
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	doc, err := readDocument(fs.Arg(0), stdin)
	if err != nil {
		return err
	}
	state, err := replay(doc)
	if err != nil {
		return err
	}
	if err := state.Validate(); err != nil {
		return fmt.Errorf("validate document: %w", err)
	}

	var output any
	switch command {
	case "validate":
		output = map[string]any{"valid": true, "revision": state.Revision}
	case "replay":
		output = state
	case "plan":
		decision, err := core.Reconcile(state)
		if err != nil {
			return fmt.Errorf("plan: %w", err)
		}
		output = decision
	}
	encoded, err := json.MarshalIndent(output, "", "  ")
	if err != nil {
		return err
	}
	_, err = fmt.Fprintf(stdout, "%s\n", encoded)
	return err
}

func readDocument(path string, stdin io.Reader) (document, error) {
	var data []byte
	var err error
	if path == "-" {
		data, err = io.ReadAll(stdin)
	} else {
		data, err = os.ReadFile(path)
	}
	if err != nil {
		return document{}, fmt.Errorf("read document: %w", err)
	}
	trimmed := strings.TrimSpace(string(data))
	if trimmed == "" {
		return document{}, errors.New("document is empty")
	}
	if strings.HasPrefix(trimmed, "[") {
		var events []core.Event
		if err := json.Unmarshal(data, &events); err != nil {
			return document{}, fmt.Errorf("decode events: %w", err)
		}
		return document{Events: events}, nil
	}
	var doc document
	if err := json.Unmarshal(data, &doc); err != nil {
		return document{}, fmt.Errorf("decode document: %w", err)
	}
	return doc, nil
}

func replay(doc document) (core.State, error) {
	events := append([]core.Event(nil), doc.Events...)
	if doc.Goal != nil {
		if len(events) == 0 || events[0].Kind != core.EventGoalCreated {
			events = append([]core.Event{{ID: core.ID("goal-" + string(doc.Goal.ID)), Kind: core.EventGoalCreated, Goal: *doc.Goal}}, events...)
		}
	}
	if doc.Budget.MaxSteps == 0 && doc.Budget.MaxCost == 0 {
		return core.Replay(events)
	}
	state := core.NewStateWithBudget(doc.Budget)
	for _, event := range events {
		var err error
		if event.Sequence == 0 {
			state, err = state.Append(event)
		} else {
			state, err = core.Reduce(state, event)
		}
		if err != nil {
			return core.State{}, err
		}
	}
	return state, nil
}
