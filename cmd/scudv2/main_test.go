package main

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func fixture(t *testing.T) string {
	t.Helper()
	doc := `{"events":[{"id":"goal-event","kind":"goal_created","goal":{"id":"g","title":"demo"}},{"id":"obligation-event","kind":"obligation_added","obligation":{"id":"a","goal_id":"g","description":"first"}}]}`
	path := filepath.Join(t.TempDir(), "goal.json")
	if err := os.WriteFile(path, []byte(doc), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestValidateAndPlan(t *testing.T) {
	path := fixture(t)
	var out, errOut bytes.Buffer
	if err := run(context.Background(), []string{"validate", path}, &out, &errOut); err != nil {
		t.Fatalf("validate: %v (%s)", err, errOut.String())
	}
	var summary map[string]any
	if err := json.Unmarshal(out.Bytes(), &summary); err != nil || summary["valid"] != true {
		t.Fatalf("unexpected validate output: %s", out.String())
	}
	out.Reset()
	if err := run(context.Background(), []string{"plan", path}, &out, &errOut); err != nil {
		t.Fatalf("plan: %v", err)
	}
	if !strings.Contains(out.String(), `"kind": "execute"`) || !strings.Contains(out.String(), `"obligation_id": "a"`) {
		t.Fatalf("unexpected plan output: %s", out.String())
	}
}

func TestReplayFromStdinAndInvalidDocument(t *testing.T) {
	path := fixture(t)
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var out, errOut bytes.Buffer
	if err := runWithStdin(context.Background(), []string{"replay", "-"}, bytes.NewReader(data), &out, &errOut); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out.String(), `"revision": 2`) {
		t.Fatalf("unexpected replay output: %s", out.String())
	}
	bad := filepath.Join(t.TempDir(), "bad.json")
	if err := os.WriteFile(bad, []byte(`{"events":[{"kind":"obligation_added","obligation":{"id":"x","goal_id":"missing"}}]}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := run(context.Background(), []string{"validate", bad}, &out, &errOut); err == nil {
		t.Fatal("expected invalid document error")
	}
}

func TestGoalShorthandPrependsGoalEvent(t *testing.T) {
	doc := `{"goal":{"id":"g","title":"shorthand"},"events":[{"kind":"obligation_added","obligation":{"id":"a","goal_id":"g"}}]}`
	path := filepath.Join(t.TempDir(), "goal-shorthand.json")
	if err := os.WriteFile(path, []byte(doc), 0o600); err != nil {
		t.Fatal(err)
	}
	var out, errOut bytes.Buffer
	if err := run(context.Background(), []string{"validate", path}, &out, &errOut); err != nil {
		t.Fatalf("validate shorthand: %v", err)
	}
	if !strings.Contains(out.String(), `"revision": 2`) {
		t.Fatalf("expected synthesized goal event: %s", out.String())
	}
}
