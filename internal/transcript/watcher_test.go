package transcript

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/reuben/scud/internal/db"
)

func setupTestDB(t *testing.T) *db.DB {
	t.Helper()
	dir := t.TempDir()
	database, err := db.Open(dir)
	if err != nil {
		t.Fatalf("db.Open: %v", err)
	}
	t.Cleanup(func() { database.Close() })
	return database
}

func writeJSONL(t *testing.T, dir, name, content string) string {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	return path
}

func TestImportBasicEntries(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	writeJSONL(t, dir, "test.jsonl", `{"type":"session_start","session_id":"s1","model":"grok","timestamp":"2025-01-01T00:00:00Z","working_dir":"/tmp"}
{"type":"message","session_id":"s1","role":"user","content":"hello","timestamp":"2025-01-01T00:00:01Z"}
{"type":"message","session_id":"s1","role":"assistant","content":"hi there","timestamp":"2025-01-01T00:00:02Z"}
{"type":"session_end","session_id":"s1","timestamp":"2025-01-01T00:00:03Z"}
`)

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("Import: %v", err)
	}

	// Verify session was created
	sessions, err := database.ListSessions(10)
	if err != nil {
		t.Fatalf("ListSessions: %v", err)
	}
	if len(sessions) == 0 {
		t.Fatal("expected at least 1 session")
	}
}

func TestImportSkipsNonJSONL(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	// Write a non-JSONL file that should be ignored
	writeJSONL(t, dir, "notes.txt", "this is not jsonl")
	writeJSONL(t, dir, "test.jsonl", `{"type":"session_start","session_id":"s1","timestamp":"2025-01-01T00:00:00Z"}
`)

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("Import: %v", err)
	}
}

func TestImportSkipsMalformedLines(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	writeJSONL(t, dir, "test.jsonl", `not json at all
{"type":"session_start","session_id":"s1","timestamp":"2025-01-01T00:00:00Z"}
also not json {{{
{"type":"message","session_id":"s1","role":"user","content":"test","timestamp":"2025-01-01T00:00:01Z"}
`)

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("Import: %v", err)
	}
}

func TestImportIncrementalOffset(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	path := filepath.Join(dir, "test.jsonl")
	line1 := `{"type":"session_start","session_id":"s1","timestamp":"2025-01-01T00:00:00Z"}` + "\n"
	if err := os.WriteFile(path, []byte(line1), 0644); err != nil {
		t.Fatal(err)
	}

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("first Import: %v", err)
	}

	// Record offset
	offset1 := w.imported[path]
	if offset1 == 0 {
		t.Fatal("expected non-zero offset after first import")
	}

	// Append more data
	f, err := os.OpenFile(path, os.O_APPEND|os.O_WRONLY, 0644)
	if err != nil {
		t.Fatal(err)
	}
	line2 := `{"type":"message","session_id":"s1","role":"user","content":"new","timestamp":"2025-01-01T00:00:01Z"}` + "\n"
	f.WriteString(line2)
	f.Close()

	// Second import should only process new data
	if err := w.Import(); err != nil {
		t.Fatalf("second Import: %v", err)
	}

	offset2 := w.imported[path]
	if offset2 <= offset1 {
		t.Errorf("offset didn't advance: %d -> %d", offset1, offset2)
	}
}

func TestImportEmptyDirectory(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("Import empty dir: %v", err)
	}
}

func TestImportSkipsDirectories(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	// Create a subdirectory (should be skipped)
	if err := os.Mkdir(filepath.Join(dir, "subdir"), 0755); err != nil {
		t.Fatal(err)
	}

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("Import: %v", err)
	}
}

func TestImportToolCall(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	writeJSONL(t, dir, "test.jsonl", `{"type":"tool_call","session_id":"s1","tool_name":"bash","tool_input":"ls -la","timestamp":"2025-01-01T00:00:00Z"}
`)

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("Import: %v", err)
	}
}

func TestImportEvent(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	writeJSONL(t, dir, "test.jsonl", `{"type":"event","session_id":"s1","content":"something happened","timestamp":"2025-01-01T00:00:00Z"}
`)

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("Import: %v", err)
	}
}

func TestImportUnknownType(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	writeJSONL(t, dir, "test.jsonl", `{"type":"custom_event","session_id":"s1","content":"data","timestamp":"2025-01-01T00:00:00Z"}
`)

	w := NewWatcher(database, dir)
	// Unknown types should be stored as events, not error
	if err := w.Import(); err != nil {
		t.Fatalf("Import: %v", err)
	}
}

func TestImportEmptyLines(t *testing.T) {
	database := setupTestDB(t)
	dir := t.TempDir()

	writeJSONL(t, dir, "test.jsonl", `
{"type":"session_start","session_id":"s1","timestamp":"2025-01-01T00:00:00Z"}

{"type":"session_end","session_id":"s1","timestamp":"2025-01-01T00:00:01Z"}

`)

	w := NewWatcher(database, dir)
	if err := w.Import(); err != nil {
		t.Fatalf("Import with empty lines: %v", err)
	}
}
