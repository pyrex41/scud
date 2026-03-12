package db

import (
	"fmt"
	"time"
)

// Event represents a session event.
type Event struct {
	ID        int
	SessionID string
	Type      string
	Data      string
	Timestamp string
}

// ValidationRun represents a set of validation commands executed together.
type ValidationRun struct {
	ID        int
	SessionID string
	AllPassed bool
	Timestamp string
	Commands  []ValidationCommand
}

// ValidationCommand represents a single validation command and its result.
type ValidationCommand struct {
	ID              int
	ValidationRunID int
	Command         string
	Passed          bool
	ExitCode        int
	Stdout          string
	Stderr          string
	DurationSec     float64
}

// AddEvent inserts an event record.
func (d *DB) AddEvent(e *Event) error {
	if e.Timestamp == "" {
		e.Timestamp = time.Now().UTC().Format(time.RFC3339)
	}
	_, err := d.db.Exec(
		`INSERT INTO events (session_id, type, data, timestamp)
		 VALUES (?, ?, ?, ?)`,
		e.SessionID, e.Type, e.Data, e.Timestamp,
	)
	if err != nil {
		return fmt.Errorf("adding event: %w", err)
	}
	return nil
}

// ListEvents returns events for a session, ordered by timestamp descending.
func (d *DB) ListEvents(sessionID string, limit int) ([]Event, error) {
	if limit <= 0 {
		limit = 100
	}
	rows, err := d.db.Query(
		`SELECT id, session_id, type, COALESCE(data,''), timestamp
		 FROM events WHERE session_id = ? ORDER BY timestamp DESC LIMIT ?`,
		sessionID, limit,
	)
	if err != nil {
		return nil, fmt.Errorf("listing events: %w", err)
	}
	defer rows.Close()

	var events []Event
	for rows.Next() {
		var e Event
		if err := rows.Scan(&e.ID, &e.SessionID, &e.Type, &e.Data, &e.Timestamp); err != nil {
			return nil, err
		}
		events = append(events, e)
	}
	return events, rows.Err()
}

// AddValidationRun inserts a validation run and its commands, returning the run ID.
func (d *DB) AddValidationRun(vr *ValidationRun) (int64, error) {
	if vr.Timestamp == "" {
		vr.Timestamp = time.Now().UTC().Format(time.RFC3339)
	}
	allPassed := 0
	if vr.AllPassed {
		allPassed = 1
	}
	res, err := d.db.Exec(
		`INSERT INTO validation_runs (session_id, all_passed, timestamp)
		 VALUES (?, ?, ?)`,
		vr.SessionID, allPassed, vr.Timestamp,
	)
	if err != nil {
		return 0, fmt.Errorf("adding validation run: %w", err)
	}
	runID, err := res.LastInsertId()
	if err != nil {
		return 0, err
	}

	for _, cmd := range vr.Commands {
		passed := 0
		if cmd.Passed {
			passed = 1
		}
		_, err := d.db.Exec(
			`INSERT INTO validation_commands (validation_run_id, command, passed, exit_code, stdout, stderr, duration_sec)
			 VALUES (?, ?, ?, ?, ?, ?, ?)`,
			runID, cmd.Command, passed, cmd.ExitCode, cmd.Stdout, cmd.Stderr, cmd.DurationSec,
		)
		if err != nil {
			return runID, fmt.Errorf("adding validation command: %w", err)
		}
	}
	return runID, nil
}

// GetValidationStats returns the total and passed validation run counts for a session.
func (d *DB) GetValidationStats(sessionID string) (total, passed int, err error) {
	err = d.db.QueryRow(
		`SELECT COUNT(*), COALESCE(SUM(all_passed), 0) FROM validation_runs WHERE session_id = ?`,
		sessionID,
	).Scan(&total, &passed)
	if err != nil {
		err = fmt.Errorf("getting validation stats: %w", err)
	}
	return
}
