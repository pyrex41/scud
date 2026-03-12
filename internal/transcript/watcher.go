// Package transcript watches Claude Code JSONL directories for new entries
// and imports them into the transcript database.
package transcript

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/reuben/scud/internal/db"
)

// Watcher polls a directory for JSONL transcript files and imports them.
type Watcher struct {
	db       *db.DB
	watchDir string
	interval time.Duration
	// Track which files have been imported (by path).
	imported map[string]int64 // path -> last byte offset processed
}

// NewWatcher creates a watcher for the given directory.
func NewWatcher(database *db.DB, watchDir string) *Watcher {
	return &Watcher{
		db:       database,
		watchDir: watchDir,
		interval: 5 * time.Second,
		imported: make(map[string]int64),
	}
}

// jsonlEntry represents a single line from a Claude Code JSONL transcript.
type jsonlEntry struct {
	Type      string `json:"type"`
	SessionID string `json:"session_id"`
	Role      string `json:"role"`
	Content   string `json:"content"`
	Timestamp string `json:"timestamp"`
	Model     string `json:"model"`
	Provider  string `json:"provider"`
	ToolName  string `json:"tool_name"`
	ToolInput string `json:"tool_input"`
	Output    string `json:"output"`
	IsError   bool   `json:"is_error"`
	// For session start/end events
	WorkingDir string `json:"working_dir"`
	Tag        string `json:"tag"`
}

// Import reads all JSONL files in the watch directory and imports new entries.
func (w *Watcher) Import() error {
	return w.importDir(w.watchDir)
}

// ImportPath imports transcripts from a specific directory.
func (w *Watcher) ImportPath(dir string) error {
	return w.importDir(dir)
}

func (w *Watcher) importDir(dir string) error {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return fmt.Errorf("reading directory %s: %w", dir, err)
	}

	var imported int
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if !strings.HasSuffix(entry.Name(), ".jsonl") {
			continue
		}
		path := filepath.Join(dir, entry.Name())
		n, err := w.importFile(path)
		if err != nil {
			return fmt.Errorf("importing %s: %w", path, err)
		}
		imported += n
	}
	if imported > 0 {
		fmt.Printf("Imported %d entries\n", imported)
	}
	return nil
}

func (w *Watcher) importFile(path string) (int, error) {
	f, err := os.Open(path)
	if err != nil {
		return 0, err
	}
	defer f.Close()

	// Seek past already-processed bytes.
	offset := w.imported[path]
	if offset > 0 {
		if _, err := f.Seek(offset, 0); err != nil {
			return 0, err
		}
	}

	scanner := bufio.NewScanner(f)
	// Allow larger lines (some tool outputs can be big).
	scanner.Buffer(make([]byte, 0, 1024*1024), 1024*1024)

	var count int
	for scanner.Scan() {
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}
		var entry jsonlEntry
		if err := json.Unmarshal(line, &entry); err != nil {
			// Skip malformed lines.
			continue
		}
		if err := w.processEntry(&entry); err != nil {
			// Log but don't abort on individual entry errors.
			fmt.Fprintf(os.Stderr, "warning: skipping entry: %v\n", err)
			continue
		}
		count++
	}

	// Record new offset.
	pos, _ := f.Seek(0, 1) // current position
	if pos == 0 {
		// Scanner consumed everything, get file size.
		info, err := f.Stat()
		if err == nil {
			pos = info.Size()
		}
	}
	w.imported[path] = pos

	return count, scanner.Err()
}

func (w *Watcher) processEntry(entry *jsonlEntry) error {
	switch entry.Type {
	case "session_start":
		return w.db.CreateSession(&db.Session{
			ID:         entry.SessionID,
			Model:      entry.Model,
			Provider:   entry.Provider,
			StartedAt:  entry.Timestamp,
			Status:     "active",
			Tag:        entry.Tag,
			WorkingDir: entry.WorkingDir,
		})
	case "session_end":
		return w.db.EndSession(entry.SessionID)
	case "message":
		_, err := w.db.AddMessage(&db.TranscriptMessage{
			SessionID: entry.SessionID,
			Role:      entry.Role,
			Content:   entry.Content,
			Timestamp: entry.Timestamp,
		})
		return err
	case "tool_call":
		_, err := w.db.AddToolCall(&db.ToolCall{
			ToolName:  entry.ToolName,
			Input:     entry.ToolInput,
			Timestamp: entry.Timestamp,
		})
		return err
	case "tool_result":
		// Without a tool_call_id reference, store as a message.
		_, err := w.db.AddMessage(&db.TranscriptMessage{
			SessionID: entry.SessionID,
			Role:      "tool",
			Content:   entry.Output,
			Timestamp: entry.Timestamp,
		})
		return err
	case "event":
		return w.db.AddEvent(&db.Event{
			SessionID: entry.SessionID,
			Type:      entry.Type,
			Data:      entry.Content,
			Timestamp: entry.Timestamp,
		})
	default:
		// Unknown types are stored as events.
		if entry.SessionID != "" {
			return w.db.AddEvent(&db.Event{
				SessionID: entry.SessionID,
				Type:      entry.Type,
				Data:      entry.Content,
				Timestamp: entry.Timestamp,
			})
		}
		return nil
	}
}

// Watch polls for new JSONL files at the configured interval until the context is cancelled.
func (w *Watcher) Watch(ctx context.Context) error {
	// Do an initial import.
	if err := w.Import(); err != nil {
		return err
	}

	ticker := time.NewTicker(w.interval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
			if err := w.Import(); err != nil {
				fmt.Fprintf(os.Stderr, "warning: import error: %v\n", err)
			}
		}
	}
}
