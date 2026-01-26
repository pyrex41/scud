//! Database schema creation and migrations.

use anyhow::Result;
use rusqlite::Connection;

pub fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Swarm/spawn sessions
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            session_name TEXT NOT NULL,
            tag TEXT NOT NULL,
            terminal_mode TEXT NOT NULL,
            working_dir TEXT NOT NULL,
            round_size INTEGER,
            started_at TEXT NOT NULL,
            completed_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_tag ON sessions(tag);
        CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at);

        -- Agent runs (one per task execution attempt)
        CREATE TABLE IF NOT EXISTS agent_runs (
            id INTEGER PRIMARY KEY,
            session_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            wave_number INTEGER,
            round_number INTEGER,
            harness TEXT,
            model TEXT,
            prompt TEXT,
            window_name TEXT,
            spawned_at TEXT NOT NULL,
            completed_at TEXT,
            success INTEGER,
            duration_ms INTEGER,
            exit_code INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_agent_runs_task ON agent_runs(task_id);
        CREATE INDEX IF NOT EXISTS idx_agent_runs_session ON agent_runs(session_id);

        -- Events (lifecycle, tools, files, dependencies)
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY,
            timestamp TEXT NOT NULL,
            session_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            agent_run_id INTEGER,
            kind TEXT NOT NULL,
            success INTEGER,
            duration_ms INTEGER,
            tool_name TEXT,
            file_path TEXT,
            dependency_id TEXT,
            reason TEXT,
            data TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_events_session_task ON events(session_id, task_id);
        CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
        CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);

        -- Transcript messages (from Claude Code JSONL)
        CREATE TABLE IF NOT EXISTS transcript_messages (
            id INTEGER PRIMARY KEY,
            claude_session_id TEXT NOT NULL,
            scud_session_id TEXT,
            task_id TEXT,
            timestamp TEXT NOT NULL,
            uuid TEXT NOT NULL,
            parent_uuid TEXT,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            model TEXT,
            input_tokens INTEGER,
            output_tokens INTEGER,
            FOREIGN KEY (scud_session_id) REFERENCES sessions(session_id)
        );
        CREATE INDEX IF NOT EXISTS idx_transcript_claude_session ON transcript_messages(claude_session_id);
        CREATE INDEX IF NOT EXISTS idx_transcript_scud_session ON transcript_messages(scud_session_id);
        CREATE INDEX IF NOT EXISTS idx_transcript_timestamp ON transcript_messages(timestamp);
        CREATE INDEX IF NOT EXISTS idx_transcript_task ON transcript_messages(task_id);

        -- Tool calls (extracted from transcripts)
        CREATE TABLE IF NOT EXISTS tool_calls (
            id INTEGER PRIMARY KEY,
            message_id INTEGER NOT NULL,
            claude_session_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            tool_id TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            input_json TEXT,
            FOREIGN KEY (message_id) REFERENCES transcript_messages(id)
        );
        CREATE INDEX IF NOT EXISTS idx_tool_calls_session ON tool_calls(claude_session_id);
        CREATE INDEX IF NOT EXISTS idx_tool_calls_name ON tool_calls(tool_name);

        -- Tool results (extracted from transcripts)
        CREATE TABLE IF NOT EXISTS tool_results (
            id INTEGER PRIMARY KEY,
            message_id INTEGER NOT NULL,
            claude_session_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            tool_use_id TEXT NOT NULL,
            content TEXT,
            is_error INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (message_id) REFERENCES transcript_messages(id)
        );
        CREATE INDEX IF NOT EXISTS idx_tool_results_session ON tool_results(claude_session_id);
        CREATE INDEX IF NOT EXISTS idx_tool_results_tool_use ON tool_results(tool_use_id);

        -- Validation runs
        CREATE TABLE IF NOT EXISTS validation_runs (
            id INTEGER PRIMARY KEY,
            session_id TEXT NOT NULL,
            wave_number INTEGER NOT NULL,
            all_passed INTEGER NOT NULL,
            started_at TEXT NOT NULL,
            completed_at TEXT,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        );

        -- Validation command results
        CREATE TABLE IF NOT EXISTS validation_commands (
            id INTEGER PRIMARY KEY,
            validation_run_id INTEGER NOT NULL,
            command TEXT NOT NULL,
            passed INTEGER NOT NULL,
            exit_code INTEGER,
            stdout TEXT,
            stderr TEXT,
            duration_secs REAL,
            FOREIGN KEY (validation_run_id) REFERENCES validation_runs(id)
        );

        -- Salvo worktrees
        CREATE TABLE IF NOT EXISTS salvo_worktrees (
            id INTEGER PRIMARY KEY,
            tag TEXT NOT NULL UNIQUE,
            worktree_path TEXT NOT NULL,
            branch_name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_sync_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_salvo_worktrees_tag ON salvo_worktrees(tag);

        -- Schema version for migrations
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );
        INSERT OR IGNORE INTO schema_version (version) VALUES (1);
    "#,
    )?;
    Ok(())
}
