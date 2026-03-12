package swarm

import (
	"testing"
)

func TestFileLineRegex(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantN   int
		wantF   string
		wantL   string
	}{
		{
			name:  "go test error",
			input: "--- FAIL: TestFoo (0.00s)\n    main_test.go:42: expected 5, got 3",
			wantN: 1,
			wantF: "main_test.go",
			wantL: "42",
		},
		{
			name:  "compiler error",
			input: "src/parser.go:15:8: undefined: FooBar",
			wantN: 1,
			wantF: "src/parser.go",
			wantL: "15",
		},
		{
			name:  "multiple matches",
			input: "error in foo.go:10\nalso bar.go:20",
			wantN: 2,
			wantF: "foo.go",
			wantL: "10",
		},
		{
			name:  "no matches",
			input: "everything passed",
			wantN: 0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			matches := fileLineRe.FindAllStringSubmatch(tt.input, -1)
			if len(matches) != tt.wantN {
				t.Errorf("got %d matches, want %d", len(matches), tt.wantN)
				return
			}
			if tt.wantN > 0 {
				if matches[0][1] != tt.wantF {
					t.Errorf("file = %q, want %q", matches[0][1], tt.wantF)
				}
				if matches[0][2] != tt.wantL {
					t.Errorf("line = %q, want %q", matches[0][2], tt.wantL)
				}
			}
		})
	}
}

func TestTaskIDRegex(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{
			name:  "standard commit",
			input: "abc1234 ([TASK-5] Fix the parser)",
			want:  "TASK-5",
		},
		{
			name:  "numeric ID",
			input: "def5678 [42] Add feature",
			want:  "42",
		},
		{
			name:  "no match",
			input: "just a regular commit message",
			want:  "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			matches := taskIDRe.FindStringSubmatch(tt.input)
			if tt.want == "" {
				if len(matches) > 0 {
					t.Errorf("expected no match, got %v", matches)
				}
				return
			}
			if len(matches) < 2 {
				t.Fatal("expected match")
			}
			if matches[1] != tt.want {
				t.Errorf("taskID = %q, want %q", matches[1], tt.want)
			}
		})
	}
}

func TestAttributeFailureNoLocations(t *testing.T) {
	vr := ValidationResult{
		AllPassed: false,
		Results: []CommandResult{
			{
				Command: "go build ./...",
				Passed:  false,
				Stderr:  "something failed but no file:line info",
			},
		},
	}

	// When there are no file:line patterns, should return nil
	result := AttributeFailure(nil, vr, "/tmp", []string{"1", "2"})
	if result != nil {
		t.Errorf("expected nil attributions, got %v", result)
	}
}
