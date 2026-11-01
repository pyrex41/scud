package cmd

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/reuben/scud/internal/taskapi"
)

type taskLineWriter struct{ lines chan string }

func (w taskLineWriter) Write(p []byte) (int, error) { w.lines <- string(p); return len(p), nil }
func waitTaskLine(t *testing.T, ch <-chan string) string {
	t.Helper()
	select {
	case line := <-ch:
		return line
	case <-time.After(3 * time.Second):
		t.Fatal("timed out waiting for status")
		return ""
	}
}

func TestTaskCommandsJSONAndWatch(t *testing.T) {
	root := t.TempDir()
	if e := os.MkdirAll(filepath.Join(root, ".scud", "tasks"), 0755); e != nil {
		t.Fatal(e)
	}
	old, e := os.Getwd()
	if e != nil {
		t.Fatal(e)
	}
	if e = os.Chdir(root); e != nil {
		t.Fatal(e)
	}
	defer os.Chdir(old)
	input := `{"schema_version":1,"phases":[{"tag":"main","name":"Main","id_format":"sequential","tasks":[{"id":"1","title":"First","description":"","status":"pending","complexity":1,"priority":"medium","dependencies":[]}]}]}`
	prose := strings.Repeat("large prose ", 8000)
	input = strings.Replace(input, `"description":""`, `"description":"`+prose+`","model_override":"special"`, 1)
	var out bytes.Buffer
	c := NewTaskImportCmd()
	c.SetIn(strings.NewReader(input))
	c.SetOut(&out)
	c.SetArgs([]string{"--json"})
	if e = c.Execute(); e != nil {
		t.Fatal(e)
	}
	var snap taskapi.Snapshot
	if e = json.Unmarshal(out.Bytes(), &snap); e != nil {
		t.Fatalf("invalid JSON output: %v", e)
	}
	out.Reset()
	c = NewTaskListCmd()
	c.SetOut(&out)
	c.SetArgs([]string{"--json"})
	if e = c.Execute(); e != nil {
		t.Fatal(e)
	}
	var d taskapi.Document
	if e = json.Unmarshal(out.Bytes(), &d); e != nil || len(d.Phases) != 1 {
		t.Fatalf("list: %s %v", out.String(), e)
	}
	c = NewTaskStatusCmd()
	stream := make(chan string, 4)
	c.SetOut(taskLineWriter{stream})
	ctx, cancel := context.WithCancel(context.Background())
	c.SetContext(ctx)
	c.SetArgs([]string{"--json", "--watch", "--interval", "100ms"})
	done := make(chan error, 1)
	go func() { done <- c.Execute() }()
	first := waitTaskLine(t, stream)
	update := NewTaskUpdateCmd()
	var updated bytes.Buffer
	update.SetOut(&updated)
	update.SetArgs([]string{"1", "--status", "done", "--assigned-to", "alice", "--depends-on="})
	if e = update.Execute(); e != nil {
		cancel()
		<-done
		t.Fatal(e)
	}
	second := waitTaskLine(t, stream)
	cancel()
	select {
	case e = <-done:
		if e != nil {
			t.Fatal(e)
		}
	case <-time.After(time.Second):
		t.Fatal("watch did not cancel")
	}
	if !strings.HasSuffix(first, "\n") || !strings.HasSuffix(second, "\n") || json.Unmarshal([]byte(first), &taskapi.Snapshot{}) != nil || json.Unmarshal([]byte(second), &snap) != nil {
		t.Fatal("watch did not stream valid JSONL snapshots")
	}
	task := snap.Phases[0].Tasks[0]
	if task.Description != prose || task.ModelOverride != "special" || task.AssignedTo != "alice" || string(task.Status) != "done" || len(task.Dependencies) != 0 {
		t.Fatal("watch lost task fields")
	}
	dup := NewTaskUpdateCmd()
	dup.SilenceErrors = true
	dup.SilenceUsage = true
	dup.SetIn(strings.NewReader(`{"title":"x","title":"y"}`))
	dup.SetArgs([]string{"1", "--input", "-"})
	if e := dup.Execute(); e == nil {
		t.Fatal("duplicate patch keys accepted")
	}
	duplicateImport := NewTaskImportCmd()
	duplicateImport.SilenceErrors = true
	duplicateImport.SilenceUsage = true
	duplicateImport.SetIn(strings.NewReader(`{"schema_version":1,"schema_version":1,"phases":[]}`))
	duplicateImport.SetArgs([]string{})
	if e := duplicateImport.Execute(); e == nil {
		t.Fatal("duplicate import keys accepted")
	}
}
