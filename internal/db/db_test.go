package db

import (
	"os"
	"path/filepath"
	"testing"
)

func openTestDB(t *testing.T) *DB {
	t.Helper()
	dir := t.TempDir()
	d, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { d.Close() })
	return d
}

func TestOpenCreatesMigratedDB(t *testing.T) {
	dir := t.TempDir()
	d, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer d.Close()

	// Check file exists
	dbPath := filepath.Join(dir, "transcripts.db")
	if _, err := os.Stat(dbPath); err != nil {
		t.Errorf("database file not created: %v", err)
	}

	// Verify migrations table was populated
	var version int
	err = d.db.QueryRow("SELECT MAX(version) FROM migrations").Scan(&version)
	if err != nil {
		t.Fatalf("reading migration version: %v", err)
	}
	if version != currentVersion {
		t.Errorf("version = %d, want %d", version, currentVersion)
	}
}

func TestSessionRoundtrip(t *testing.T) {
	d := openTestDB(t)

	s := &Session{
		ID:         "sess-001",
		Model:      "grok-4",
		Provider:   "xai",
		StartedAt:  "2025-01-15T10:00:00Z",
		Status:     "active",
		Tag:        "v1",
		WorkingDir: "/tmp/project",
	}

	if err := d.CreateSession(s); err != nil {
		t.Fatal(err)
	}

	// GetSession
	got, err := d.GetSession("sess-001")
	if err != nil {
		t.Fatal(err)
	}
	if got.Model != "grok-4" {
		t.Errorf("Model = %q, want grok-4", got.Model)
	}
	if got.Tag != "v1" {
		t.Errorf("Tag = %q, want v1", got.Tag)
	}

	// ListSessions
	sessions, err := d.ListSessions(10)
	if err != nil {
		t.Fatal(err)
	}
	if len(sessions) != 1 {
		t.Fatalf("ListSessions = %d, want 1", len(sessions))
	}
	if sessions[0].ID != "sess-001" {
		t.Errorf("session ID = %q, want sess-001", sessions[0].ID)
	}

	// GetSession not found
	_, err = d.GetSession("nonexistent")
	if err == nil {
		t.Error("expected error for nonexistent session")
	}
}

func TestMessageAndTranscript(t *testing.T) {
	d := openTestDB(t)

	d.CreateSession(&Session{ID: "s1", Model: "m", Provider: "p", StartedAt: "2025-01-01T00:00:00Z"})

	// Add messages
	_, err := d.AddMessage(&TranscriptMessage{
		SessionID: "s1",
		Role:      "user",
		Content:   "Hello world",
		Timestamp: "2025-01-01T00:01:00Z",
	})
	if err != nil {
		t.Fatal(err)
	}

	_, err = d.AddMessage(&TranscriptMessage{
		SessionID: "s1",
		Role:      "assistant",
		Content:   "Hi there",
		Timestamp: "2025-01-01T00:02:00Z",
	})
	if err != nil {
		t.Fatal(err)
	}

	// GetTranscript
	msgs, err := d.GetTranscript("s1")
	if err != nil {
		t.Fatal(err)
	}
	if len(msgs) != 2 {
		t.Fatalf("transcript length = %d, want 2", len(msgs))
	}
	if msgs[0].Role != "user" || msgs[0].Content != "Hello world" {
		t.Errorf("msg[0] = %+v", msgs[0])
	}
	if msgs[1].Role != "assistant" || msgs[1].Content != "Hi there" {
		t.Errorf("msg[1] = %+v", msgs[1])
	}
}

func TestSearchMessages(t *testing.T) {
	d := openTestDB(t)

	d.CreateSession(&Session{ID: "s1", Model: "m", Provider: "p", StartedAt: "2025-01-01T00:00:00Z"})

	d.AddMessage(&TranscriptMessage{
		SessionID: "s1", Role: "user", Content: "fix the bug in parser",
		Timestamp: "2025-01-01T00:01:00Z",
	})
	d.AddMessage(&TranscriptMessage{
		SessionID: "s1", Role: "assistant", Content: "I updated the serializer",
		Timestamp: "2025-01-01T00:02:00Z",
	})

	// Search for "parser"
	results, err := d.SearchMessages("parser", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(results) != 1 {
		t.Fatalf("search 'parser' = %d results, want 1", len(results))
	}
	if results[0].Content != "fix the bug in parser" {
		t.Errorf("content = %q", results[0].Content)
	}

	// Search for something not present
	results, err = d.SearchMessages("nonexistent-term", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(results) != 0 {
		t.Errorf("search 'nonexistent-term' = %d results, want 0", len(results))
	}
}

func TestEvents(t *testing.T) {
	d := openTestDB(t)

	d.CreateSession(&Session{ID: "s1", Model: "m", Provider: "p", StartedAt: "2025-01-01T00:00:00Z"})

	if err := d.AddEvent(&Event{
		SessionID: "s1",
		Type:      "task_start",
		Data:      `{"task_id":"1"}`,
		Timestamp: "2025-01-01T00:01:00Z",
	}); err != nil {
		t.Fatal(err)
	}

	if err := d.AddEvent(&Event{
		SessionID: "s1",
		Type:      "task_complete",
		Data:      `{"task_id":"1"}`,
		Timestamp: "2025-01-01T00:02:00Z",
	}); err != nil {
		t.Fatal(err)
	}

	events, err := d.ListEvents("s1", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(events) != 2 {
		t.Fatalf("events = %d, want 2", len(events))
	}
	// Listed in descending order
	if events[0].Type != "task_complete" {
		t.Errorf("events[0].Type = %q, want task_complete", events[0].Type)
	}
}

func TestAgentRuns(t *testing.T) {
	d := openTestDB(t)

	d.CreateSession(&Session{ID: "s1", Model: "m", Provider: "p", StartedAt: "2025-01-01T00:00:00Z"})

	if err := d.CreateAgentRun(&AgentRun{
		ID:        "run-1",
		SessionID: "s1",
		AgentType: "builder",
		TaskID:    "task-1",
		Model:     "grok",
		StartedAt: "2025-01-01T00:01:00Z",
		Status:    "running",
	}); err != nil {
		t.Fatal(err)
	}

	if err := d.CreateAgentRun(&AgentRun{
		ID:        "run-2",
		SessionID: "s1",
		AgentType: "tester",
		TaskID:    "task-2",
		Model:     "grok",
		StartedAt: "2025-01-01T00:02:00Z",
		Status:    "running",
	}); err != nil {
		t.Fatal(err)
	}

	runs, err := d.ListAgentRuns("s1")
	if err != nil {
		t.Fatal(err)
	}
	if len(runs) != 2 {
		t.Fatalf("runs = %d, want 2", len(runs))
	}
	if runs[0].ID != "run-1" {
		t.Errorf("runs[0].ID = %q, want run-1", runs[0].ID)
	}
	if runs[1].AgentType != "tester" {
		t.Errorf("runs[1].AgentType = %q, want tester", runs[1].AgentType)
	}

	// End agent run
	if err := d.EndAgentRun("run-1", "success", 0); err != nil {
		t.Fatal(err)
	}

	runs, _ = d.ListAgentRuns("s1")
	if runs[0].Status != "success" {
		t.Errorf("after end, status = %q, want success", runs[0].Status)
	}
}
