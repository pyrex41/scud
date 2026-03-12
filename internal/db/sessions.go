package db

import (
	"database/sql"
	"fmt"
	"time"
)

// Session represents a coding session.
type Session struct {
	ID         string
	Model      string
	Provider   string
	StartedAt  string
	EndedAt    string
	Status     string
	Tag        string
	WorkingDir string
}

// AgentRun represents a single agent execution within a session.
type AgentRun struct {
	ID        string
	SessionID string
	AgentType string
	TaskID    string
	Model     string
	StartedAt string
	EndedAt   string
	Status    string
	ExitCode  int
}

// CreateSession inserts a new session record.
func (d *DB) CreateSession(s *Session) error {
	if s.StartedAt == "" {
		s.StartedAt = time.Now().UTC().Format(time.RFC3339)
	}
	if s.Status == "" {
		s.Status = "active"
	}
	_, err := d.db.Exec(
		`INSERT INTO sessions (id, model, provider, started_at, ended_at, status, tag, working_dir)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
		s.ID, s.Model, s.Provider, s.StartedAt, s.EndedAt, s.Status, s.Tag, s.WorkingDir,
	)
	if err != nil {
		return fmt.Errorf("creating session: %w", err)
	}
	return nil
}

// EndSession marks a session as ended.
func (d *DB) EndSession(id string) error {
	now := time.Now().UTC().Format(time.RFC3339)
	res, err := d.db.Exec(
		`UPDATE sessions SET ended_at = ?, status = 'ended' WHERE id = ?`,
		now, id,
	)
	if err != nil {
		return fmt.Errorf("ending session: %w", err)
	}
	n, _ := res.RowsAffected()
	if n == 0 {
		return fmt.Errorf("session %s not found", id)
	}
	return nil
}

// ListSessions returns the most recent sessions, ordered by start time descending.
func (d *DB) ListSessions(limit int) ([]Session, error) {
	if limit <= 0 {
		limit = 20
	}
	rows, err := d.db.Query(
		`SELECT id, model, provider, started_at, COALESCE(ended_at,''), status, COALESCE(tag,''), COALESCE(working_dir,'')
		 FROM sessions ORDER BY started_at DESC LIMIT ?`, limit,
	)
	if err != nil {
		return nil, fmt.Errorf("listing sessions: %w", err)
	}
	defer rows.Close()

	var sessions []Session
	for rows.Next() {
		var s Session
		if err := rows.Scan(&s.ID, &s.Model, &s.Provider, &s.StartedAt, &s.EndedAt, &s.Status, &s.Tag, &s.WorkingDir); err != nil {
			return nil, err
		}
		sessions = append(sessions, s)
	}
	return sessions, rows.Err()
}

// GetSession retrieves a session by ID.
func (d *DB) GetSession(id string) (*Session, error) {
	var s Session
	err := d.db.QueryRow(
		`SELECT id, model, provider, started_at, COALESCE(ended_at,''), status, COALESCE(tag,''), COALESCE(working_dir,'')
		 FROM sessions WHERE id = ?`, id,
	).Scan(&s.ID, &s.Model, &s.Provider, &s.StartedAt, &s.EndedAt, &s.Status, &s.Tag, &s.WorkingDir)
	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("session %s not found", id)
	}
	if err != nil {
		return nil, fmt.Errorf("getting session: %w", err)
	}
	return &s, nil
}

// CreateAgentRun inserts a new agent run record.
func (d *DB) CreateAgentRun(r *AgentRun) error {
	if r.StartedAt == "" {
		r.StartedAt = time.Now().UTC().Format(time.RFC3339)
	}
	_, err := d.db.Exec(
		`INSERT INTO agent_runs (id, session_id, agent_type, task_id, model, started_at, ended_at, status, exit_code)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		r.ID, r.SessionID, r.AgentType, r.TaskID, r.Model, r.StartedAt, r.EndedAt, r.Status, r.ExitCode,
	)
	if err != nil {
		return fmt.Errorf("creating agent run: %w", err)
	}
	return nil
}

// EndAgentRun marks an agent run as finished with the given status and exit code.
func (d *DB) EndAgentRun(id, status string, exitCode int) error {
	now := time.Now().UTC().Format(time.RFC3339)
	res, err := d.db.Exec(
		`UPDATE agent_runs SET ended_at = ?, status = ?, exit_code = ? WHERE id = ?`,
		now, status, exitCode, id,
	)
	if err != nil {
		return fmt.Errorf("ending agent run: %w", err)
	}
	n, _ := res.RowsAffected()
	if n == 0 {
		return fmt.Errorf("agent run %s not found", id)
	}
	return nil
}

// ListAgentRuns returns all agent runs for a given session.
func (d *DB) ListAgentRuns(sessionID string) ([]AgentRun, error) {
	rows, err := d.db.Query(
		`SELECT id, session_id, COALESCE(agent_type,''), COALESCE(task_id,''), COALESCE(model,''),
		        started_at, COALESCE(ended_at,''), COALESCE(status,''), COALESCE(exit_code,0)
		 FROM agent_runs WHERE session_id = ? ORDER BY started_at ASC`, sessionID,
	)
	if err != nil {
		return nil, fmt.Errorf("listing agent runs: %w", err)
	}
	defer rows.Close()

	var runs []AgentRun
	for rows.Next() {
		var r AgentRun
		if err := rows.Scan(&r.ID, &r.SessionID, &r.AgentType, &r.TaskID, &r.Model, &r.StartedAt, &r.EndedAt, &r.Status, &r.ExitCode); err != nil {
			return nil, err
		}
		runs = append(runs, r)
	}
	return runs, rows.Err()
}
