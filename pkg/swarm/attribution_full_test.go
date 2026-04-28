package swarm

import (
	"context"
	"testing"
)

func TestAttributeFailureDirectTaskID(t *testing.T) {
	vr := ValidationResult{
		AllPassed: false,
		Results: []CommandResult{
			{
				Command: "go test ./...",
				Passed:  false,
				Stderr:  "FAIL [TASK-5] some test failed\nmore output here",
			},
		},
	}

	result := AttributeFailure(context.Background(), vr, "/tmp", []string{"TASK-5", "TASK-6"})
	if len(result) != 1 {
		t.Fatalf("got %d attributions, want 1", len(result))
	}
	if result[0].TaskID != "TASK-5" {
		t.Errorf("TaskID = %q, want %q", result[0].TaskID, "TASK-5")
	}
	if result[0].Confidence != "high" {
		t.Errorf("Confidence = %q, want %q", result[0].Confidence, "high")
	}
}

func TestAttributeFailureMultipleTaskIDs(t *testing.T) {
	vr := ValidationResult{
		AllPassed: false,
		Results: []CommandResult{
			{
				Command: "go build ./...",
				Passed:  false,
				Stderr:  "[TASK-1] undefined foo\n[TASK-3] undefined bar",
			},
		},
	}

	result := AttributeFailure(context.Background(), vr, "/tmp", []string{"TASK-1", "TASK-2", "TASK-3"})
	if len(result) != 2 {
		t.Fatalf("got %d attributions, want 2", len(result))
	}

	ids := make(map[string]bool)
	for _, a := range result {
		ids[a.TaskID] = true
		if a.Confidence != "medium" {
			t.Errorf("TaskID %s: Confidence = %q, want %q", a.TaskID, a.Confidence, "medium")
		}
	}
	if !ids["TASK-1"] || !ids["TASK-3"] {
		t.Errorf("expected TASK-1 and TASK-3, got %v", ids)
	}
}

func TestAttributeFailureTaskIDNotInWave(t *testing.T) {
	vr := ValidationResult{
		AllPassed: false,
		Results: []CommandResult{
			{
				Command: "go test ./...",
				Passed:  false,
				Stderr:  "[TASK-99] some error",
			},
		},
	}

	// TASK-99 is not in the wave tasks list
	result := AttributeFailure(context.Background(), vr, "/tmp", []string{"TASK-1", "TASK-2"})
	if result != nil {
		t.Errorf("expected nil attributions for non-wave task, got %v", result)
	}
}

func TestAttributeFailureNoisyOutput(t *testing.T) {
	vr := ValidationResult{
		AllPassed: false,
		Results: []CommandResult{
			{
				Command: "cargo build",
				Passed:  false,
				Stderr:  "error: could not compile\n   Compiling foo v0.1.0\nthread 'main' panicked at 'oh no'",
			},
		},
	}

	result := AttributeFailure(context.Background(), vr, "/tmp", []string{"1", "2"})
	if result != nil {
		t.Errorf("expected nil for noisy output with no task markers, got %v", result)
	}
}

func TestAttributeFailureFromStdout(t *testing.T) {
	vr := ValidationResult{
		AllPassed: false,
		Results: []CommandResult{
			{
				Command: "npm test",
				Passed:  false,
				Stdout:  "FAIL: [fix-auth] assertion error in login.test.js",
				Stderr:  "",
			},
		},
	}

	result := AttributeFailure(context.Background(), vr, "/tmp", []string{"fix-auth", "add-api"})
	if len(result) != 1 {
		t.Fatalf("got %d attributions, want 1", len(result))
	}
	if result[0].TaskID != "fix-auth" {
		t.Errorf("TaskID = %q, want %q", result[0].TaskID, "fix-auth")
	}
}

func TestAttributeFailurePassedCommandsIgnored(t *testing.T) {
	vr := ValidationResult{
		AllPassed: false,
		Results: []CommandResult{
			{
				Command: "go build ./...",
				Passed:  true, // passed — should be skipped
				Stderr:  "[TASK-1] this passed though",
			},
			{
				Command: "go test ./...",
				Passed:  false,
				Stderr:  "[TASK-2] real failure",
			},
		},
	}

	result := AttributeFailure(context.Background(), vr, "/tmp", []string{"TASK-1", "TASK-2"})
	if len(result) != 1 {
		t.Fatalf("got %d attributions, want 1", len(result))
	}
	if result[0].TaskID != "TASK-2" {
		t.Errorf("TaskID = %q, want %q", result[0].TaskID, "TASK-2")
	}
}

func TestAttributeFailureConfidenceSorting(t *testing.T) {
	vr := ValidationResult{
		AllPassed: false,
		Results: []CommandResult{
			{
				Command: "go test",
				Passed:  false,
				Stderr:  "[A] error\n[B] error\n[C] error\n[D] error",
			},
		},
	}

	result := AttributeFailure(context.Background(), vr, "/tmp", []string{"A", "B", "C", "D"})
	if len(result) != 4 {
		t.Fatalf("got %d attributions, want 4", len(result))
	}
	// All should be low confidence since 4 tasks matched
	for _, a := range result {
		if a.Confidence != "low" {
			t.Errorf("TaskID %s: Confidence = %q, want %q (4 matches = low)", a.TaskID, a.Confidence, "low")
		}
	}
}
