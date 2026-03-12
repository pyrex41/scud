package db

import (
	"fmt"
)

// TranscriptMessage represents a single message in a transcript.
type TranscriptMessage struct {
	ID         int
	SessionID  string
	AgentRunID string
	Role       string
	Content    string
	Timestamp  string
	TokenCount int
}

// ToolCall represents a tool invocation within a message.
type ToolCall struct {
	ID        int
	MessageID int
	ToolName  string
	Input     string
	Timestamp string
}

// ToolResult represents the result of a tool call.
type ToolResult struct {
	ID         int
	ToolCallID int
	Output     string
	IsError    bool
	Timestamp  string
}

// AddMessage inserts a transcript message and returns its ID.
func (d *DB) AddMessage(m *TranscriptMessage) (int64, error) {
	res, err := d.db.Exec(
		`INSERT INTO transcript_messages (session_id, agent_run_id, role, content, timestamp, token_count)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		m.SessionID, m.AgentRunID, m.Role, m.Content, m.Timestamp, m.TokenCount,
	)
	if err != nil {
		return 0, fmt.Errorf("adding message: %w", err)
	}
	return res.LastInsertId()
}

// AddToolCall inserts a tool call record and returns its ID.
func (d *DB) AddToolCall(tc *ToolCall) (int64, error) {
	res, err := d.db.Exec(
		`INSERT INTO tool_calls (message_id, tool_name, input, timestamp)
		 VALUES (?, ?, ?, ?)`,
		tc.MessageID, tc.ToolName, tc.Input, tc.Timestamp,
	)
	if err != nil {
		return 0, fmt.Errorf("adding tool call: %w", err)
	}
	return res.LastInsertId()
}

// AddToolResult inserts a tool result record.
func (d *DB) AddToolResult(tr *ToolResult) error {
	isErr := 0
	if tr.IsError {
		isErr = 1
	}
	_, err := d.db.Exec(
		`INSERT INTO tool_results (tool_call_id, output, is_error, timestamp)
		 VALUES (?, ?, ?, ?)`,
		tr.ToolCallID, tr.Output, isErr, tr.Timestamp,
	)
	if err != nil {
		return fmt.Errorf("adding tool result: %w", err)
	}
	return nil
}

// GetTranscript returns all messages for a session, ordered by timestamp.
func (d *DB) GetTranscript(sessionID string) ([]TranscriptMessage, error) {
	rows, err := d.db.Query(
		`SELECT id, session_id, COALESCE(agent_run_id,''), role, content, timestamp, COALESCE(token_count,0)
		 FROM transcript_messages WHERE session_id = ? ORDER BY timestamp ASC`, sessionID,
	)
	if err != nil {
		return nil, fmt.Errorf("getting transcript: %w", err)
	}
	defer rows.Close()

	var msgs []TranscriptMessage
	for rows.Next() {
		var m TranscriptMessage
		if err := rows.Scan(&m.ID, &m.SessionID, &m.AgentRunID, &m.Role, &m.Content, &m.Timestamp, &m.TokenCount); err != nil {
			return nil, err
		}
		msgs = append(msgs, m)
	}
	return msgs, rows.Err()
}

// SearchMessages searches transcript messages using LIKE matching.
func (d *DB) SearchMessages(query string, limit int) ([]TranscriptMessage, error) {
	if limit <= 0 {
		limit = 50
	}
	pattern := "%" + query + "%"
	rows, err := d.db.Query(
		`SELECT id, session_id, COALESCE(agent_run_id,''), role, content, timestamp, COALESCE(token_count,0)
		 FROM transcript_messages WHERE content LIKE ? ORDER BY timestamp DESC LIMIT ?`,
		pattern, limit,
	)
	if err != nil {
		return nil, fmt.Errorf("searching messages: %w", err)
	}
	defer rows.Close()

	var msgs []TranscriptMessage
	for rows.Next() {
		var m TranscriptMessage
		if err := rows.Scan(&m.ID, &m.SessionID, &m.AgentRunID, &m.Role, &m.Content, &m.Timestamp, &m.TokenCount); err != nil {
			return nil, err
		}
		msgs = append(msgs, m)
	}
	return msgs, rows.Err()
}
